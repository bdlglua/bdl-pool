//! BTC Stratum V1 authentication proxy and CKPool accepted-share collector.
//!
//! The proxy authenticates a participant before forwarding mining.authorize to
//! a private, pool-mode CKPool instance. CKPool remains the source of truth for
//! accepted shares: the collector reads only its -L JSON share logs and posts
//! result=true entries to the private coordinator API.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};
use url::Url;

const MAX_STRATUM_LINE: usize = 16 * 1024;
const MAX_SHARELOG_LINE: usize = 16 * 1024;
const MAX_LOG_FILES: usize = 20_000;
const MAX_RECORDS_PER_POLL: usize = 1_000;
const CONNECTION_WORKERS: usize = 16;
const CONNECTION_QUEUE: usize = 64;
const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone)]
struct IngestConfig {
    listen: SocketAddr,
    upstream: SocketAddr,
    coordinator_base: String,
    backend_token: String,
    share_log_root: PathBuf,
    cursor_path: PathBuf,
}

fn config_string(config: &Value, key: &str) -> io::Result<String> {
    config[key]
        .as_str()
        .filter(|value| !value.is_empty() && !value.contains(['\n', '\r', '\0']))
        .map(str::to_owned)
        .ok_or_else(|| io::Error::other(format!("missing or invalid {key}")))
}

fn valid_hex_secret(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn socket_address(value: &str, loopback_only: bool) -> io::Result<SocketAddr> {
    let address: SocketAddr = value.parse().map_err(|_| {
        io::Error::other("Stratum listen/upstream must be an IP:port socket address")
    })?;
    if address.port() == 0 || (loopback_only && !address.ip().is_loopback()) {
        return Err(io::Error::other("Invalid Stratum socket address"));
    }
    Ok(address)
}

fn coordinator_base(value: &str) -> io::Result<String> {
    let url = Url::parse(value).map_err(io::Error::other)?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || !url
            .port_or_known_default()
            .map(|port| port > 1024)
            .unwrap_or(false)
        || url.path() != "/api/v1/collective"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(io::Error::other(
            "coordinatorUrl must point to a local 127.0.0.1 coordinator on a non-privileged port",
        ));
    }
    Ok(value.trim_end_matches('/').to_string())
}

impl IngestConfig {
    fn from_value(config: &Value) -> io::Result<Self> {
        let listen = socket_address(&config_string(config, "stratumListen")?, false)?;
        let upstream = socket_address(&config_string(config, "stratumUpstream")?, true)?;
        if listen == upstream {
            return Err(io::Error::other(
                "Stratum proxy and CKPool cannot use the same socket",
            ));
        }
        let coordinator_base = coordinator_base(&config_string(config, "coordinatorUrl")?)?;
        let backend_token = config_string(config, "backendToken")?;
        if !valid_hex_secret(&backend_token) {
            return Err(io::Error::other(
                "backendToken must be a 32-byte lowercase hex token",
            ));
        }
        let share_log_root = PathBuf::from(config_string(config, "shareLogDirectory")?);
        if !share_log_root.is_absolute() {
            return Err(io::Error::other(
                "shareLogDirectory must be an absolute path",
            ));
        }
        let cursor_path = PathBuf::from(config_string(config, "cursorPath")?);
        if !cursor_path.is_absolute() {
            return Err(io::Error::other("cursorPath must be an absolute path"));
        }
        Ok(Self {
            listen,
            upstream,
            coordinator_base,
            backend_token,
            share_log_root,
            cursor_path,
        })
    }
}

/// Validate a BDLMinerV2 credential tuple. The user field is the participant
/// ID, optionally followed by a conventional CKPool worker suffix.
fn participant_from_credentials(username: &str, password: &str) -> Option<String> {
    if !valid_hex_secret(password) {
        return None;
    }
    let (participant, worker) = username.split_once('.').unwrap_or((username, ""));
    if participant.len() != 32
        || !participant
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || worker.len() > 48
        || worker.contains('.')
        || !worker
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return None;
    }
    Some(participant.to_string())
}

fn coordinator_authorize(
    config: &IngestConfig,
    participant_id: &str,
    stratum_password: &str,
) -> bool {
    let response = ureq::post(&format!("{}/internal/authorize", config.coordinator_base))
        .timeout(Duration::from_secs(3))
        .set("Authorization", &format!("Bearer {}", config.backend_token))
        .set("Content-Type", "application/json")
        .send_json(json!({
            "participantId":participant_id,
            "stratumPassword":stratum_password
        }));
    match response {
        Ok(response) => response
            .into_json::<Value>()
            .map(|value| {
                value["authorized"] == true
                    && value["participantId"] == participant_id
                    && value["coin"] == "btc"
            })
            .unwrap_or(false),
        Err(_) => false,
    }
}

fn unauthorized_response(id: &Value) -> Vec<u8> {
    let mut line = serde_json::to_vec(&json!({
        "id":id,
        "result":false,
        "error":[24,"Unauthorized worker",null]
    }))
    .unwrap_or_else(|_| {
        b"{\"id\":null,\"result\":false,\"error\":[24,\"Unauthorized worker\",null]}".to_vec()
    });
    line.push(b'\n');
    line
}

fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    output: &mut Vec<u8>,
    limit: usize,
) -> io::Result<bool> {
    output.clear();
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            return Ok(!output.is_empty());
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let count = newline.map(|index| index + 1).unwrap_or(buffer.len());
        if output.len().saturating_add(count) > limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Stratum line exceeds limit",
            ));
        }
        output.extend_from_slice(&buffer[..count]);
        reader.consume(count);
        if newline.is_some() {
            return Ok(true);
        }
    }
}

fn stratum_method(line: &[u8]) -> Option<(String, Value)> {
    let text = std::str::from_utf8(line)
        .ok()?
        .trim_end_matches(['\r', '\n']);
    let value: Value = serde_json::from_str(text).ok()?;
    let method = value.get("method")?.as_str()?.to_string();
    Some((method, value))
}

fn proxy_connection(
    mut client: TcpStream,
    upstream_address: SocketAddr,
    config: Arc<IngestConfig>,
) -> io::Result<()> {
    let mut upstream = match TcpStream::connect_timeout(&upstream_address, Duration::from_secs(3)) {
        Ok(stream) => stream,
        Err(_) => return Ok(()),
    };
    let _ = client.set_nodelay(true);
    let _ = upstream.set_nodelay(true);
    let _ = client.set_read_timeout(Some(Duration::from_secs(900)));
    let _ = upstream.set_read_timeout(Some(Duration::from_secs(900)));
    let mut client_reader = BufReader::new(client.try_clone()?);
    let mut upstream_writer = upstream.try_clone()?;
    let mut downstream_client = client.try_clone()?;
    let downstream = thread::spawn(move || {
        let _ = io::copy(&mut upstream, &mut downstream_client);
        let _ = downstream_client.shutdown(std::net::Shutdown::Write);
    });

    let mut authorized = false;
    let mut auth_attempted = false;
    let mut preauth_messages = 0_usize;
    let mut line = Vec::with_capacity(256);
    let mut client_writer = client;

    loop {
        if !read_bounded_line(&mut client_reader, &mut line, MAX_STRATUM_LINE)? {
            break;
        }
        let Some((method, value)) = stratum_method(&line) else {
            break;
        };
        if method == "mining.authorize" {
            if authorized || auth_attempted {
                let id = value.get("id").unwrap_or(&Value::Null);
                let _ = client_writer.write_all(&unauthorized_response(id));
                break;
            }
            auth_attempted = true;
            let params = value["params"].as_array();
            let username = params
                .and_then(|items| items.first())
                .and_then(Value::as_str);
            let password = params
                .and_then(|items| items.get(1))
                .and_then(Value::as_str);
            let participant_id = username
                .zip(password)
                .and_then(|(user, pass)| participant_from_credentials(user, pass));
            let valid = match (participant_id.as_deref(), password) {
                (Some(participant_id), Some(password)) => {
                    coordinator_authorize(&config, participant_id, password)
                }
                _ => false,
            };
            if !valid {
                let id = value.get("id").unwrap_or(&Value::Null);
                let _ = client_writer.write_all(&unauthorized_response(id));
                let _ = client_writer.flush();
                break;
            }
            authorized = true;
            upstream_writer.write_all(&line)?;
            upstream_writer.flush()?;
            continue;
        }

        if !authorized {
            preauth_messages += 1;
            if preauth_messages > 8
                || !matches!(
                    method.as_str(),
                    "mining.subscribe"
                        | "mining.configure"
                        | "mining.suggest_difficulty"
                        | "mining.extranonce.subscribe"
                )
            {
                let id = value.get("id").unwrap_or(&Value::Null);
                let _ = client_writer.write_all(&unauthorized_response(id));
                let _ = client_writer.flush();
                break;
            }
        }
        upstream_writer.write_all(&line)?;
        upstream_writer.flush()?;
    }

    let _ = upstream_writer.shutdown(std::net::Shutdown::Both);
    let _ = client_writer.shutdown(std::net::Shutdown::Both);
    let _ = downstream.join();
    Ok(())
}

pub fn serve_proxy(config: IngestConfig) -> io::Result<()> {
    let listener = TcpListener::bind(config.listen)?;
    listener.set_nonblocking(false)?;
    println!(
        "BDL BTC Stratum V1 collective proxy listening on {}",
        config.listen
    );
    let config = Arc::new(config);
    let (sender, receiver) = mpsc::sync_channel::<TcpStream>(CONNECTION_QUEUE);
    let receiver = Arc::new(Mutex::new(receiver));
    for _ in 0..CONNECTION_WORKERS {
        let receiver = receiver.clone();
        let config = config.clone();
        thread::spawn(move || loop {
            let stream = receiver.lock().unwrap().recv();
            let Ok(stream) = stream else {
                break;
            };
            let _ = proxy_connection(stream, config.upstream, config.clone());
        });
    }
    for stream in listener.incoming() {
        let stream = stream?;
        let _ = sender.try_send(stream);
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct AcceptedShare {
    share_id: String,
    participant_id: String,
    work_units: u64,
}

fn string_field<'a>(value: &'a Value, key: &str, max_len: usize) -> Option<&'a str> {
    value
        .get(key)?
        .as_str()
        .filter(|text| !text.is_empty() && text.len() <= max_len)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 15) as usize] as char);
    }
    output
}

fn parse_ckpool_share(value: &Value) -> Option<AcceptedShare> {
    if value["result"].as_bool() != Some(true) {
        return None;
    }
    let username = string_field(value, "username", 96)?;
    let participant_id = username.split('.').next()?;
    if participant_id.len() != 32
        || !participant_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let diff = value["diff"].as_f64()?;
    if !diff.is_finite() || diff <= 0.0 {
        return None;
    }
    let work = (diff * 1_000_000.0).round();
    if !work.is_finite() || work < 1.0 || work >= u64::MAX as f64 {
        return None;
    }
    let work_units = work as u64;

    let workinfo = value["workinfoid"]
        .as_u64()
        .map(|number| number.to_string())
        .or_else(|| string_field(value, "workinfoid", 32).map(str::to_owned))?;
    let client = value["clientid"].to_string();
    let nonce2 = string_field(value, "nonce2", 64)?;
    let nonce = string_field(value, "nonce", 16)?;
    let ntime = string_field(value, "ntime", 16)?;
    let hash = string_field(value, "hash", 64)?;
    let worker = string_field(value, "workername", 64).unwrap_or("");
    let identity = format!(
        "btc|{participant_id}|{workinfo}|{client}|{worker}|{nonce2}|{nonce}|{ntime}|{hash}"
    );
    let share_id = format!("ckp-{}", hex(&Sha256::digest(identity.as_bytes())));
    Some(AcceptedShare {
        share_id,
        participant_id: participant_id.to_string(),
        work_units,
    })
}

fn post_accepted_share(config: &IngestConfig, share: &AcceptedShare) -> io::Result<()> {
    let result = ureq::post(&format!("{}/internal/shares", config.coordinator_base))
        .timeout(Duration::from_secs(3))
        .set("Authorization", &format!("Bearer {}", config.backend_token))
        .set("Content-Type", "application/json")
        .send_json(json!({
            "coin":"btc",
            "shareId":share.share_id,
            "participantId":share.participant_id,
            "workUnits":share.work_units
        }));
    match result {
        Ok(response) if (200..300).contains(&response.status()) => Ok(()),
        // A 409 means this is an already-recorded share or an account which
        // was not enrolled. Both are permanent outcomes for this log record.
        Err(ureq::Error::Status(409, _)) => Ok(()),
        Err(ureq::Error::Status(400, _)) => Ok(()),
        Err(_) => Err(io::Error::other("coordinator share ingestion failed")),
        _ => Err(io::Error::other("unexpected coordinator response")),
    }
}

fn read_cursors(path: &Path) -> io::Result<BTreeMap<String, u64>> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(io::Error::other("share cursor path must be a regular file"));
    }
    let bytes = fs::read(path)?;
    let value: BTreeMap<String, u64> = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    Ok(value)
}

fn save_cursors(path: &Path, cursors: &BTreeMap<String, u64>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        #[cfg(unix)]
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }
    if path.exists() {
        let meta = fs::symlink_metadata(path)?;
        if meta.file_type().is_symlink() || !meta.is_file() {
            return Err(io::Error::other("share cursor path must be a regular file"));
        }
    }
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec(cursors)?;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let mut file = options.open(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    #[cfg(unix)]
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
    fs::rename(&temporary, path)?;
    Ok(())
}

fn collect_sharelog_files(
    directory: &Path,
    root: &Path,
    depth: usize,
    files: &mut Vec<PathBuf>,
) -> io::Result<()> {
    if depth > 8 || files.len() >= MAX_LOG_FILES {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_sharelog_files(&path, root, depth + 1, files)?;
        } else if file_type.is_file()
            && path.extension().and_then(|ext| ext.to_str()) == Some("sharelog")
        {
            if path.starts_with(root) {
                files.push(path);
                if files.len() >= MAX_LOG_FILES {
                    break;
                }
            }
        }
    }
    Ok(())
}

fn process_sharelog_file(
    config: &IngestConfig,
    root: &Path,
    path: &Path,
    cursors: &mut BTreeMap<String, u64>,
    records_left: &mut usize,
) -> io::Result<usize> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| io::Error::other("sharelog escaped its configured directory"))?
        .to_string_lossy()
        .to_string();
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(0);
    }
    let mut offset = *cursors.get(&relative).unwrap_or(&0);
    if offset > metadata.len() {
        offset = 0;
    }
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(offset))?;
    let mut bytes = Vec::with_capacity(512);
    let mut processed = 0;
    while *records_left > 0 {
        let before = reader.stream_position()?;
        bytes.clear();
        let size = reader.read_until(b'\n', &mut bytes)?;
        if size == 0 || !bytes.ends_with(b"\n") {
            break; // CKPool may still be writing the final line.
        }
        let after = reader.stream_position()?;
        if bytes.len() <= MAX_SHARELOG_LINE {
            if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                if let Some(share) = parse_ckpool_share(&value) {
                    post_accepted_share(config, &share)?;
                }
            }
        }
        offset = after;
        cursors.insert(relative.clone(), offset);
        *records_left -= 1;
        processed += 1;
        if processed % 32 == 0 {
            save_cursors(&config.cursor_path, cursors)?;
        }
        if after <= before {
            return Err(io::Error::other("sharelog offset did not advance"));
        }
    }
    cursors.insert(relative, offset);
    Ok(processed)
}

fn process_sharelogs(config: &IngestConfig) -> io::Result<usize> {
    let root_metadata = fs::symlink_metadata(&config.share_log_root)?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(io::Error::other(
            "shareLogDirectory must be a real directory",
        ));
    }
    let root = config.share_log_root.canonicalize()?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(io::Error::other(
            "shareLogDirectory must be a real directory",
        ));
    }
    let mut files = Vec::new();
    collect_sharelog_files(&root, &root, 0, &mut files)?;
    files.sort();

    let mut cursors = read_cursors(&config.cursor_path)?;
    if cursors.len() > MAX_LOG_FILES {
        return Err(io::Error::other(
            "share cursor index exceeds the configured file limit",
        ));
    }
    let mut remaining = MAX_RECORDS_PER_POLL;
    let mut processed = 0;
    for path in files {
        if remaining == 0 {
            break;
        }
        processed += process_sharelog_file(config, &root, &path, &mut cursors, &mut remaining)?;
    }
    save_cursors(&config.cursor_path, &cursors)?;
    Ok(processed)
}

fn sharelog_loop(config: IngestConfig) {
    loop {
        let _ = process_sharelogs(&config);
        thread::sleep(POLL_INTERVAL);
    }
}

/// Start the BTC Stratum V1 authentication proxy and the CKPool share-log
/// collector. CKPool's own listener must remain private on 127.0.0.1.
pub fn run(value: &Value) -> io::Result<()> {
    let config = IngestConfig::from_value(value)?;
    let root_meta = fs::symlink_metadata(&config.share_log_root)?;
    if root_meta.file_type().is_symlink() || !root_meta.is_dir() {
        return Err(io::Error::other(
            "shareLogDirectory must be a real directory",
        ));
    }
    let root = config.share_log_root.canonicalize()?;
    let mut collector_config = config.clone();
    collector_config.share_log_root = root;
    thread::spawn(move || sharelog_loop(collector_config));
    serve_proxy(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn base_config(root: &Path) -> Value {
        json!({
            "stratumListen":"127.0.0.1:3337",
            "stratumUpstream":"127.0.0.1:3338",
            "coordinatorUrl":"http://127.0.0.1:8082/api/v1/collective",
            "backendToken":"a".repeat(64),
            "shareLogDirectory":root,
            "cursorPath":root.join("cursor.json")
        })
    }

    #[test]
    fn only_a_registered_format_credential_tuple_is_accepted() {
        let id = "0123456789abcdef0123456789abcdef";
        assert_eq!(
            participant_from_credentials(id, &"a".repeat(64)).as_deref(),
            Some(id)
        );
        assert_eq!(
            participant_from_credentials(&format!("{id}.gamma601"), &"b".repeat(64)).as_deref(),
            Some(id)
        );
        assert!(participant_from_credentials(id, "x").is_none());
        assert!(participant_from_credentials("not-an-id", &"a".repeat(64)).is_none());
        assert!(
            participant_from_credentials(&format!("{id}.bad.worker"), &"a".repeat(64)).is_none()
        );
        assert!(
            participant_from_credentials(&format!("{id}.bad worker"), &"a".repeat(64)).is_none()
        );
    }

    #[test]
    fn ckpool_sharelog_counts_only_accepted_server_records_and_uses_assigned_diff() {
        let id = "0123456789abcdef0123456789abcdef";
        let record = json!({
            "workinfoid":12345,
            "clientid":88,
            "nonce2":"00000001",
            "nonce":"deadbeef",
            "ntime":"65abc123",
            "hash":"c".repeat(64),
            "diff":0.0002,
            "sdiff":0.00025,
            "result":true,
            "username":id,
            "workername":"gamma601"
        });
        let accepted = parse_ckpool_share(&record).unwrap();
        assert_eq!(accepted.participant_id, id);
        assert_eq!(accepted.work_units, 200);
        assert!(accepted.share_id.starts_with("ckp-"));
        assert_eq!(accepted.share_id.len(), 68);

        let mut rejected = record.clone();
        rejected["result"] = false.into();
        assert!(parse_ckpool_share(&rejected).is_none());
        let mut bad_diff = record.clone();
        bad_diff["diff"] = (-1.0).into();
        assert!(parse_ckpool_share(&bad_diff).is_none());
        let mut bad_user = record;
        bad_user["username"] = "stranger".into();
        assert!(parse_ckpool_share(&bad_user).is_none());
    }

    #[test]
    fn ingest_config_only_allows_the_loopback_coordinator_and_upstream() {
        let root = TempDir::new().unwrap();
        let config = base_config(root.path());
        assert!(IngestConfig::from_value(&config).is_ok());
        let mut unsafe_coordinator = config.clone();
        unsafe_coordinator["coordinatorUrl"] = "https://example.com/api/v1/collective".into();
        assert!(IngestConfig::from_value(&unsafe_coordinator).is_err());
        let mut unsafe_upstream = config;
        unsafe_upstream["stratumUpstream"] = "192.0.2.1:3338".into();
        assert!(IngestConfig::from_value(&unsafe_upstream).is_err());
    }

    fn read_http_request(stream: &mut TcpStream) -> io::Result<(String, Value)> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut request_line = String::new();
        reader.read_line(&mut request_line)?;
        let mut length = 0_usize;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 || line == "\r\n" {
                break;
            }
            if let Some((key, value)) = line.split_once(':') {
                if key.eq_ignore_ascii_case("content-length") {
                    length = value.trim().parse().unwrap_or(0);
                }
            }
        }
        let mut body = vec![0_u8; length];
        reader.read_exact(&mut body)?;
        let body: Value = serde_json::from_slice(&body).map_err(io::Error::other)?;
        Ok((request_line, body))
    }

    fn reply_http(stream: &mut TcpStream, body: Value) -> io::Result<()> {
        let encoded = serde_json::to_vec(&body)?;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            encoded.len()
        )?;
        stream.write_all(&encoded)?;
        stream.flush()
    }

    #[test]
    fn authenticated_v1_proxy_forwards_only_registered_miners() {
        let coordinator = TcpListener::bind("127.0.0.1:0").unwrap();
        let coordinator_port = coordinator.local_addr().unwrap().port();
        let coordinator_worker = thread::spawn(move || {
            let (mut stream, _) = coordinator.accept().unwrap();
            let (line, body) = read_http_request(&mut stream).unwrap();
            assert!(line.starts_with("POST /api/v1/collective/internal/authorize "));
            assert_eq!(body["participantId"], "0123456789abcdef0123456789abcdef");
            assert_eq!(body["stratumPassword"], "a".repeat(64));
            reply_http(
                &mut stream,
                json!({
                    "authorized":true,
                    "participantId":"0123456789abcdef0123456789abcdef",
                    "coin":"btc"
                }),
            )
            .unwrap();
        });

        let upstream = TcpListener::bind("127.0.0.1:0").unwrap();
        let upstream_address = upstream.local_addr().unwrap();
        let upstream_worker = thread::spawn(move || {
            let (stream, _) = upstream.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut writer = stream;
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.contains("mining.subscribe"));
            writer
                .write_all(b"{\"id\":1,\"result\":[[],\"abcd\",4],\"error\":null}\n")
                .unwrap();
            writer.flush().unwrap();

            line.clear();
            reader.read_line(&mut line).unwrap();
            assert!(line.contains("mining.authorize"));
            writer
                .write_all(b"{\"id\":2,\"result\":true,\"error\":null}\n")
                .unwrap();
            writer.flush().unwrap();

            line.clear();
            reader.read_line(&mut line).unwrap();
            assert!(line.contains("mining.submit"));
        });

        let proxy_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy_address = proxy_listener.local_addr().unwrap();
        let config = Arc::new(IngestConfig {
            listen: proxy_address,
            upstream: upstream_address,
            coordinator_base: format!("http://127.0.0.1:{coordinator_port}/api/v1/collective"),
            backend_token: "b".repeat(64),
            share_log_root: std::env::temp_dir(),
            cursor_path: std::env::temp_dir().join("bdl-collective-test-cursor.json"),
        });
        let (sender, receiver) = mpsc::sync_channel::<TcpStream>(1);
        let worker_config = config.clone();
        let worker = thread::spawn(move || {
            let stream = receiver.recv().unwrap();
            proxy_connection(stream, worker_config.upstream, worker_config).unwrap();
        });
        let listener_worker = thread::spawn(move || {
            let (stream, _) = proxy_listener.accept().unwrap();
            sender.send(stream).unwrap();
        });
        let client = TcpStream::connect(proxy_address).unwrap();
        listener_worker.join().unwrap();
        let mut reader = BufReader::new(client.try_clone().unwrap());
        let mut writer = client;
        writer
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        writer
            .write_all(b"{\"id\":1,\"method\":\"mining.subscribe\",\"params\":[\"BDLMinerV2\"]}\n")
            .unwrap();
        writer.flush().unwrap();
        let mut response = String::new();
        reader.read_line(&mut response).unwrap();
        assert!(response.contains("\"id\":1"));

        writer
            .write_all(
                format!(
                    "{{\"id\":2,\"method\":\"mining.authorize\",\"params\":[\"0123456789abcdef0123456789abcdef.gamma601\",\"{}\"]}}\n",
                    "a".repeat(64)
                )
                .as_bytes(),
            )
            .unwrap();
        writer.flush().unwrap();
        response.clear();
        reader.read_line(&mut response).unwrap();
        assert!(response.contains("\"result\":true"));

        writer
            .write_all(b"{\"id\":10,\"method\":\"mining.submit\",\"params\":[]}\n")
            .unwrap();
        writer.flush().unwrap();
        coordinator_worker.join().unwrap();
        upstream_worker.join().unwrap();
        writer.shutdown(std::net::Shutdown::Both).unwrap();
        worker.join().unwrap();
    }

    #[test]
    fn sharelog_collector_persists_offsets_and_does_not_double_submit_after_restart() {
        let root = TempDir::new().unwrap();
        let log_root = root.path().join("logs");
        fs::create_dir_all(log_root.join("00000001")).unwrap();
        let file = log_root.join("00000001/0000000000000001.sharelog");
        let id = "0123456789abcdef0123456789abcdef";
        let record = json!({
            "workinfoid":1,
            "clientid":1,
            "nonce2":"00000001",
            "nonce":"deadbeef",
            "ntime":"65abc123",
            "hash":"c".repeat(64),
            "diff":0.0002,
            "sdiff":0.00025,
            "result":false,
            "username":id,
            "workername":"gamma601"
        });
        fs::write(&file, serde_json::to_vec(&record).unwrap()).unwrap();
        let config = IngestConfig {
            listen: "127.0.0.1:3337".parse().unwrap(),
            upstream: "127.0.0.1:3338".parse().unwrap(),
            coordinator_base: "http://127.0.0.1:8082/api/v1/collective".into(),
            backend_token: "a".repeat(64),
            share_log_root: log_root.clone(),
            cursor_path: root.path().join("cursor.json"),
        };
        assert_eq!(process_sharelogs(&config).unwrap(), 0);
        let cursors = read_cursors(&config.cursor_path).unwrap();
        assert!(cursors.contains_key("00000001/0000000000000001.sharelog"));
        assert_eq!(
            cursors["00000001/0000000000000001.sharelog"],
            fs::metadata(&file).unwrap().len()
        );
    }
}

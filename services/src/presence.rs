//! Fixed-command CKPool IPC reader; raw UAs, IPs and extranonces never persist.
use crate::api::{self, MAX_FILE};
use hmac::{Hmac, Mac};
use serde_json::{json, Map, Value};
use sha2::Sha256;
#[cfg(unix)]
use std::os::unix::{
    fs::{OpenOptionsExt, PermissionsExt},
    net::UnixStream,
};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::Path,
    time::{Duration, Instant},
};

pub fn filter_clients(clients: &[Value], salt: &[u8], coin: &str) -> Vec<Value> {
    let mut result = Vec::new();
    for client in clients {
        if client["authorised"] != true {
            continue;
        }
        let Some(name) = client["workername"].as_str() else {
            continue;
        };
        let Some((address, _)) = api::worker_identity(name, coin) else {
            continue;
        };
        let id = api::number(&client["id"], true);
        if id.is_null() {
            continue;
        }
        let raw = format!(
            "{id}:{}:{name}",
            client
                .get("starttime")
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".into())
        );
        let mut hmac = Hmac::<Sha256>::new_from_slice(salt).unwrap();
        hmac.update(raw.as_bytes());
        let identifier = hmac
            .finalize()
            .into_bytes()
            .iter()
            .take(12)
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let mut rates = Map::new();
        for (period, key) in [
            ("1m", "dsps1"),
            ("5m", "dsps5"),
            ("1h", "dsps60"),
            ("1d", "dsps1440"),
            ("7d", "dsps10080"),
        ] {
            let rate = api::number(&client[key], false)
                .as_f64()
                .map(|v| v * 4294967296.0)
                .filter(|v| v.is_finite());
            rates.insert(period.into(), json!(rate));
        }
        result.push(json!({"id":identifier,"address":address,"name":name,"device":api::device_from_agent(&client["useragent"]),"hashrateHps":rates,"bestShare":api::number(&client["bestdiff"],false),"connectedAt":api::epoch(&client["starttime"]),"lastShareAt":api::epoch(&client["lastshare"])}));
    }
    result
}
#[cfg(unix)]
pub fn clients_from_socket(path: &Path) -> io::Result<Vec<Value>> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut stream = UnixStream::connect(path)?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    stream.write_all(&[7, 0, 0, 0, b'c', b'l', b'i', b'e', b'n', b't', b's'])?;
    fn receive(stream: &mut UnixStream, data: &mut [u8], deadline: Instant) -> io::Result<()> {
        let mut offset = 0;
        while offset < data.len() {
            let timeout = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::TimedOut, "CKPool response deadline")
                })?;
            stream.set_read_timeout(Some(timeout))?;
            let n = stream.read(&mut data[offset..])?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Truncated CKPool response",
                ));
            }
            offset += n;
        }
        Ok(())
    }
    let mut header = [0_u8; 4];
    receive(&mut stream, &mut header, deadline)?;
    let size = u32::from_le_bytes(header) as usize;
    if size == 0 || size > 8 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Oversized CKPool response",
        ));
    }
    let mut body = vec![0; size];
    receive(&mut stream, &mut body, deadline)?;
    let data: Value = serde_json::from_slice(&body)?;
    data["clients"]
        .as_array()
        .filter(|c| c.len() <= 2000)
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid CKPool clients"))
}
pub fn publish(path: &Path, data: &Value) -> io::Result<()> {
    let encoded = serde_json::to_vec(data)?;
    if encoded.len() as u64 > MAX_FILE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Presence snapshot too large",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing parent directory"))?;
    let mut random = [0; 16];
    getrandom::getrandom(&mut random).map_err(io::Error::other)?;
    let temporary = parent.join(format!(
        ".presence-{}",
        random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let result = (|| {
        let mut file = options.open(&temporary)?;
        file.write_all(&encoded)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        std::fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    result
}
#[cfg(unix)]
pub fn run(config: &Value, once: bool) -> io::Result<()> {
    let mut salt = [0; 32];
    getrandom::getrandom(&mut salt).map_err(io::Error::other)?;
    let mut profiles = vec![("btc", config)];
    if let Some(bch) = config["coins"].get("bch") {
        profiles.push(("bch", bch));
    }
    let mut successes = std::collections::HashMap::new();
    loop {
        for (coin, profile) in &profiles {
            let output = Path::new(
                profile["onlineSnapshot"]
                    .as_str()
                    .unwrap_or("/var/lib/bdl-pool-presence/online.json"),
            );
            let socket = Path::new(
                profile["ckpoolSocket"]
                    .as_str()
                    .unwrap_or("/run/ckpool/stratifier"),
            );
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
                fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
            }
            let now = api::epoch(&json!(api::now()));
            let result =
                clients_from_socket(socket).map(|clients| filter_clients(&clients, &salt, coin));
            let data = match result {
                Ok(clients) => {
                    successes.insert(*coin, now.clone());
                    json!({"schemaVersion":1,"coin":coin,"checkedAt":now,"updatedAt":now,"available":true,"clients":clients})
                }
                Err(_) => {
                    json!({"schemaVersion":1,"coin":coin,"checkedAt":now,"updatedAt":successes.get(coin),"available":false,"clients":[]})
                }
            };
            publish(output, &data)?;
        }
        if once {
            break;
        }
        std::thread::sleep(Duration::from_secs(15));
    }
    Ok(())
}

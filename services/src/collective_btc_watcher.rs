//! Bitcoin Core watcher for the separate BTC collective CKPool instance.
//!
//! It captures a durable share-sequence snapshot when a pool-paid block first
//! appears, then waits for 100 confirmations before creating its PPLNS plan.
//! Orphaned candidates are invalidated and rescanned. It never signs or
//! broadcasts a transaction.

use crate::address;
use serde_json::{json, Value};
#[cfg(unix)]
use std::os::{
    fd::AsRawFd,
    unix::fs::{OpenOptionsExt, PermissionsExt},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use url::{Host, Url};

const BTC_COINBASE_MATURITY: u64 = 100;
const MAX_BLOCKS_PER_POLL: usize = 128;
const MAX_CURSOR_BYTES: u64 = 16 * 1024;
const RPC_TIMEOUT: Duration = Duration::from_secs(8);

struct WatchConfig {
    rpc_url: String,
    rpc_user: String,
    rpc_password: String,
    coordinator_url: String,
    backend_token: String,
    payout_address: String,
    window_work_units: u64,
    start_height: u64,
    cursor_path: PathBuf,
    poll_interval: Duration,
}

fn valid_secret(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn config_string(config: &Value, key: &str, max_len: usize) -> io::Result<String> {
    config[key]
        .as_str()
        .filter(|value| {
            !value.is_empty()
                && value.len() <= max_len
                && !value.chars().any(|character| character.is_control())
        })
        .map(str::to_owned)
        .ok_or_else(|| io::Error::other(format!("missing or invalid {key}")))
}

fn loopback_url(value: &str, path: &str) -> io::Result<String> {
    let url = Url::parse(value).map_err(|_| io::Error::other("invalid local service URL"))?;
    let local = match url.host() {
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        None => false,
    };
    if url.scheme() != "http"
        || !local
        || url.port().filter(|port| *port > 0).is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != path
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(io::Error::other(
            "service URL must use HTTP on loopback with the expected path",
        ));
    }
    Ok(value.trim_end_matches('/').to_string())
}

impl WatchConfig {
    fn from_value(value: &Value) -> io::Result<Self> {
        let rpc_url = loopback_url(&config_string(value, "rpcUrl", 512)?, "/")?;
        let rpc_user = config_string(value, "rpcUser", 128)?;
        if rpc_user.contains(':') {
            return Err(io::Error::other("rpcUser must not contain a colon"));
        }
        let rpc_password = config_string(value, "rpcPassword", 256)?;
        let coordinator_url = loopback_url(
            &config_string(value, "coordinatorUrl", 512)?,
            "/api/v1/collective",
        )?;
        if !coordinator_url.starts_with("http://127.0.0.1:") {
            return Err(io::Error::other("coordinatorUrl must use 127.0.0.1"));
        }
        let backend_token = config_string(value, "backendToken", 64)?;
        if !valid_secret(&backend_token) {
            return Err(io::Error::other(
                "backendToken must be a 32-byte lowercase hex token",
            ));
        }
        let payout_address = config_string(value, "payoutAddress", 128)?;
        let payout_address = address::canonical_coin(&payout_address, "btc")
            .ok_or_else(|| io::Error::other("payoutAddress is not a valid BTC mainnet address"))?;
        let window_work_units = value["windowWorkUnits"]
            .as_u64()
            .filter(|units| *units > 0)
            .ok_or_else(|| io::Error::other("windowWorkUnits must be a positive integer"))?;
        let start_height = value["startHeight"]
            .as_u64()
            .ok_or_else(|| io::Error::other("startHeight must be a non-negative integer"))?;
        let cursor_path = PathBuf::from(config_string(value, "cursorPath", 1024)?);
        if !cursor_path.is_absolute() {
            return Err(io::Error::other("cursorPath must be an absolute path"));
        }
        let poll_secs = value["pollIntervalSeconds"].as_u64().unwrap_or(5);
        if !(5..=300).contains(&poll_secs) {
            return Err(io::Error::other(
                "pollIntervalSeconds must be between 5 and 300",
            ));
        }
        Ok(Self {
            rpc_url,
            rpc_user,
            rpc_password,
            coordinator_url,
            backend_token,
            payout_address,
            window_work_units,
            start_height,
            cursor_path,
            poll_interval: Duration::from_secs(poll_secs),
        })
    }
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = *chunk.get(1).unwrap_or(&0);
        let third = *chunk.get(2).unwrap_or(&0);
        output.push(TABLE[(first >> 2) as usize] as char);
        output.push(TABLE[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(third & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

fn rpc_call(config: &WatchConfig, method: &str, params: Value) -> io::Result<Value> {
    let credentials = format!("{}:{}", config.rpc_user, config.rpc_password);
    let authorization = format!("Basic {}", base64(credentials.as_bytes()));
    let response = ureq::post(&config.rpc_url)
        .timeout(RPC_TIMEOUT)
        .set("Authorization", &authorization)
        .set("Content-Type", "application/json")
        .send_json(json!({
            "jsonrpc":"1.0",
            "id":"bdl-collective-btc-watch",
            "method":method,
            "params":params
        }))
        .map_err(|error| match error {
            ureq::Error::Status(status, _) => {
                io::Error::other(format!("Bitcoin Core RPC returned HTTP {status}"))
            }
            _ => io::Error::other("Bitcoin Core RPC is unavailable"),
        })?;
    let body: Value = response
        .into_json()
        .map_err(|_| io::Error::other("Bitcoin Core RPC returned invalid JSON"))?;
    if !body.get("error").unwrap_or(&Value::Null).is_null() {
        return Err(io::Error::other(format!(
            "Bitcoin Core rejected RPC method {method}"
        )));
    }
    body.get("result")
        .cloned()
        .ok_or_else(|| io::Error::other("Bitcoin Core RPC response has no result"))
}

fn block_hash_at(config: &WatchConfig, height: u64) -> io::Result<String> {
    let result = rpc_call(config, "getblockhash", json!([height]))?;
    let hash = result
        .as_str()
        .filter(|value| value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| io::Error::other("Bitcoin Core returned an invalid block hash"))?;
    Ok(hash.to_ascii_lowercase())
}

fn btc_to_satoshis(value: &Value) -> io::Result<u64> {
    let btc = value
        .as_f64()
        .filter(|number| number.is_finite() && *number >= 0.0)
        .ok_or_else(|| io::Error::other("coinbase output contains an invalid BTC amount"))?;
    let satoshis = (btc * 100_000_000.0).round();
    if !satoshis.is_finite() || satoshis < 0.0 || satoshis > u64::MAX as f64 {
        return Err(io::Error::other(
            "coinbase output amount is outside the satoshi range",
        ));
    }
    Ok(satoshis as u64)
}

fn coinbase_reward_for_address(
    block: &Value,
    expected_hash: &str,
    expected_height: u64,
    payout_address: &str,
) -> io::Result<Option<u64>> {
    if block["hash"].as_str() != Some(expected_hash)
        || block["height"].as_u64() != Some(expected_height)
    {
        return Err(io::Error::other(
            "Bitcoin Core block hash/height changed while the block was inspected",
        ));
    }
    if block["confirmations"].as_i64().unwrap_or(0) < 1 {
        return Err(io::Error::other(
            "Bitcoin Core block is not in the active chain",
        ));
    }

    let transactions = block["tx"]
        .as_array()
        .filter(|items| !items.is_empty())
        .ok_or_else(|| io::Error::other("Bitcoin block has no decoded transactions"))?;
    let coinbase = &transactions[0];
    if !coinbase["vin"]
        .as_array()
        .map(|items| items.len() == 1 && items[0]["coinbase"].is_string())
        .unwrap_or(false)
    {
        return Err(io::Error::other(
            "first Bitcoin block transaction is not coinbase",
        ));
    }
    let outputs = coinbase["vout"]
        .as_array()
        .ok_or_else(|| io::Error::other("coinbase transaction has no outputs"))?;

    let mut found = false;
    let mut reward_atoms = 0_u64;
    for output in outputs {
        let script = &output["scriptPubKey"];
        let direct = script["address"].as_str() == Some(payout_address);
        let listed = script["addresses"]
            .as_array()
            .map(|addresses| {
                addresses
                    .iter()
                    .any(|address| address.as_str() == Some(payout_address))
            })
            .unwrap_or(false);
        if direct || listed {
            found = true;
            reward_atoms = reward_atoms
                .checked_add(btc_to_satoshis(&output["value"])?)
                .ok_or_else(|| io::Error::other("coinbase reward satoshi total overflow"))?;
        }
    }
    if !found {
        return Ok(None);
    }
    if reward_atoms == 0 {
        return Err(io::Error::other(
            "collective coinbase output has zero value",
        ));
    }
    Ok(Some(reward_atoms))
}

fn post_candidate(
    config: &WatchConfig,
    block_hash: &str,
    height: u64,
    reward_atoms: u64,
) -> io::Result<()> {
    let response = ureq::post(&format!("{}/internal/candidates", config.coordinator_url))
        .timeout(Duration::from_secs(5))
        .set("Authorization", &format!("Bearer {}", config.backend_token))
        .set("Content-Type", "application/json")
        .send_json(json!({
            "coin":"btc",
            "blockId":block_hash,
            "height":height,
            "rewardAtoms":reward_atoms
        }))
        .map_err(|error| match error {
            ureq::Error::Status(status, _) => io::Error::other(format!(
                "coordinator rejected BTC block candidate (HTTP {status}); retrying"
            )),
            _ => io::Error::other("coordinator is unavailable while recording block candidate"),
        })?;
    let result: Value = response
        .into_json()
        .map_err(|_| io::Error::other("coordinator returned invalid block-candidate response"))?;
    if result["status"].as_str() == Some("error") {
        return Err(io::Error::other("coordinator rejected BTC block candidate"));
    }
    Ok(())
}

fn pending_candidates(config: &WatchConfig) -> io::Result<Vec<Value>> {
    let response = ureq::get(&format!("{}/internal/candidates", config.coordinator_url))
        .timeout(Duration::from_secs(5))
        .set("Authorization", &format!("Bearer {}", config.backend_token))
        .call()
        .map_err(|_| io::Error::other("coordinator candidate list is unavailable"))?;
    let result: Value = response
        .into_json()
        .map_err(|_| io::Error::other("coordinator returned invalid candidate list"))?;
    result["candidates"]
        .as_array()
        .cloned()
        .ok_or_else(|| io::Error::other("coordinator candidate list is malformed"))
}

fn orphan_candidate(config: &WatchConfig, block_hash: &str) -> io::Result<()> {
    let response = ureq::post(&format!(
        "{}/internal/candidates/orphan",
        config.coordinator_url
    ))
    .timeout(Duration::from_secs(5))
    .set("Authorization", &format!("Bearer {}", config.backend_token))
    .set("Content-Type", "application/json")
    .send_json(json!({"coin":"btc","blockId":block_hash}))
    .map_err(|_| io::Error::other("coordinator could not mark orphaned block candidate"))?;
    let result: Value = response
        .into_json()
        .map_err(|_| io::Error::other("coordinator returned invalid orphan response"))?;
    if result["status"].as_str() == Some("error") {
        return Err(io::Error::other(
            "coordinator rejected orphaned block candidate",
        ));
    }
    Ok(())
}

fn block_confirmations(block: &Value) -> io::Result<u64> {
    block["confirmations"]
        .as_u64()
        .filter(|confirmations| *confirmations > 0)
        .ok_or_else(|| io::Error::other("Bitcoin Core did not return active-chain confirmations"))
}

fn post_mature_block(config: &WatchConfig, block_hash: &str, reward_atoms: u64) -> io::Result<()> {
    let response = ureq::post(&format!("{}/internal/blocks", config.coordinator_url))
        .timeout(Duration::from_secs(5))
        .set("Authorization", &format!("Bearer {}", config.backend_token))
        .set("Content-Type", "application/json")
        .send_json(json!({
            "coin":"btc",
            "blockId":block_hash,
            "rewardAtoms":reward_atoms,
            "windowWorkUnits":config.window_work_units,
            "mature":true
        }))
        .map_err(|error| match error {
            ureq::Error::Status(status, _) => io::Error::other(format!(
                "coordinator rejected mature block (HTTP {status}); retrying"
            )),
            _ => io::Error::other("coordinator is unavailable; retrying mature block"),
        })?;
    let result: Value = response
        .into_json()
        .map_err(|_| io::Error::other("coordinator returned invalid block response"))?;
    if result["status"].as_str() == Some("error") {
        return Err(io::Error::other(
            "coordinator returned a block error; retrying",
        ));
    }
    Ok(())
}

struct Cursor {
    next_height: u64,
    last_height: Option<u64>,
    last_hash: Option<String>,
}

struct CursorStore {
    path: PathBuf,
    cursor: Cursor,
    _lock_file: File,
}

impl CursorStore {
    fn open(path: &Path, start_height: u64) -> io::Result<Self> {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("cursor path has no parent"))?;
        fs::create_dir_all(parent)?;
        let parent_meta = fs::symlink_metadata(parent)?;
        if !parent_meta.is_dir() || parent_meta.file_type().is_symlink() {
            return Err(io::Error::other("cursor parent must be a real directory"));
        }
        #[cfg(unix)]
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;

        let lock_path = path.with_extension("lock");
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        let lock_file = options.open(&lock_path)?;
        #[cfg(unix)]
        {
            fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600))?;
            let result =
                unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if result != 0 {
                return Err(io::Error::other(
                    "another BTC block watcher already owns the cursor",
                ));
            }
        }

        let cursor = match fs::symlink_metadata(path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink()
                    || !metadata.is_file()
                    || metadata.len() > MAX_CURSOR_BYTES
                {
                    return Err(io::Error::other(
                        "block cursor must be a small regular file",
                    ));
                }
                let value: Value = serde_json::from_slice(&fs::read(path)?)
                    .map_err(|_| io::Error::other("block cursor JSON is invalid"))?;
                let next_height = value["nextHeight"]
                    .as_u64()
                    .ok_or_else(|| io::Error::other("block cursor nextHeight is invalid"))?;
                let last_height = value["lastHeight"].as_u64();
                let last_hash = value["lastHash"].as_str().map(str::to_owned);
                match (last_height, last_hash.as_deref()) {
                    (Some(height), Some(hash))
                        if height.checked_add(1) == Some(next_height)
                            && hash.len() == 64
                            && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) => {}
                    (None, None) => {}
                    _ => return Err(io::Error::other("block cursor checkpoint is inconsistent")),
                }
                Cursor {
                    next_height,
                    last_height,
                    last_hash,
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Cursor {
                next_height: start_height,
                last_height: None,
                last_hash: None,
            },
            Err(error) => return Err(error),
        };
        let mut store = Self {
            path: path.to_path_buf(),
            cursor,
            _lock_file: lock_file,
        };
        store.save(&store.cursor)?;
        Ok(store)
    }

    fn save(&self, cursor: &Cursor) -> io::Result<()> {
        let value = json!({
            "nextHeight":cursor.next_height,
            "lastHeight":cursor.last_height,
            "lastHash":cursor.last_hash
        });
        let bytes = serde_json::to_vec(&value)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| io::Error::other("invalid block cursor filename"))?;
        let temporary =
            self.path
                .with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), now));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        let mut file = options.open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        #[cfg(unix)]
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
        fs::rename(&temporary, &self.path)?;
        if let Some(parent) = self.path.parent() {
            File::open(parent)?.sync_all()?;
        }
        Ok(())
    }

    fn rewind_to(&mut self, height: u64, previous_hash: Option<&str>) -> io::Result<()> {
        if height >= self.cursor.next_height {
            return Ok(());
        }
        let next = Cursor {
            next_height: height,
            last_height: height.checked_sub(1),
            last_hash: previous_hash.map(str::to_ascii_lowercase),
        };
        if next.last_height.is_some() != next.last_hash.is_some() {
            return Err(io::Error::other("rewind checkpoint is incomplete"));
        }
        self.save(&next)?;
        self.cursor = next;
        Ok(())
    }

    fn advance(&mut self, height: u64, hash: &str) -> io::Result<()> {
        if height != self.cursor.next_height {
            return Err(io::Error::other("block cursor advanced out of sequence"));
        }
        let next_height = height
            .checked_add(1)
            .ok_or_else(|| io::Error::other("block height overflow"))?;
        let next = Cursor {
            next_height,
            last_height: Some(height),
            last_hash: Some(hash.to_ascii_lowercase()),
        };
        self.save(&next)?;
        self.cursor = next;
        Ok(())
    }
}

fn poll_once(config: &WatchConfig, cursor: &mut CursorStore) -> io::Result<usize> {
    if let (Some(height), Some(previous_hash)) = (
        cursor.cursor.last_height,
        cursor.cursor.last_hash.as_deref(),
    ) {
        let canonical_hash = block_hash_at(config, height)?;
        if canonical_hash != previous_hash {
            let previous = if height == 0 {
                None
            } else {
                Some(block_hash_at(config, height - 1)?)
            };
            cursor.rewind_to(height, previous.as_deref())?;
            println!(
                "Bitcoin chain reorganisation detected at height {height}; rewinding the candidate scan."
            );
        }
    }

    let tip = rpc_call(config, "getblockcount", json!([]))?
        .as_u64()
        .ok_or_else(|| io::Error::other("Bitcoin Core returned an invalid chain height"))?;
    let mut scanned = 0;

    // Discover candidates as soon as CKPool's coinbase transaction appears
    // in the active chain. The coordinator freezes the accepted-share
    // sequence at this point; later shares cannot dilute this block's PPLNS.
    while cursor.cursor.next_height <= tip && scanned < MAX_BLOCKS_PER_POLL {
        let height = cursor.cursor.next_height;
        let hash = block_hash_at(config, height)?;
        let block = rpc_call(config, "getblock", json!([hash, 2]))?;
        let reward = coinbase_reward_for_address(&block, &hash, height, &config.payout_address)?;
        if block_hash_at(config, height)? != hash {
            return Err(io::Error::other(
                "Bitcoin chain changed while a new block candidate was being inspected",
            ));
        }
        if let Some(reward_atoms) = reward {
            post_candidate(config, &hash, height, reward_atoms)?;
            println!("Captured BTC collective block candidate at height {height}.");
        }
        cursor.advance(height, &hash)?;
        scanned += 1;
    }

    // A mature candidate is accounted only if the same block is still
    // canonical and its coinbase still pays the configured pool address.
    for candidate in pending_candidates(config)? {
        let height = candidate["height"]
            .as_u64()
            .ok_or_else(|| io::Error::other("saved candidate has invalid block height"))?;
        let hash = candidate["blockId"]
            .as_str()
            .filter(|hash| hash.len() == 64)
            .ok_or_else(|| io::Error::other("saved candidate has invalid block hash"))?;
        let reward_atoms = candidate["rewardAtoms"]
            .as_u64()
            .filter(|reward| *reward > 0)
            .ok_or_else(|| io::Error::other("saved candidate has invalid reward amount"))?;

        if block_hash_at(config, height)? != hash {
            orphan_candidate(config, hash)?;
            let previous_hash = if height == 0 {
                None
            } else {
                Some(block_hash_at(config, height - 1)?)
            };
            cursor.rewind_to(height, previous_hash.as_deref())?;
            println!("Marked orphaned BTC collective block candidate at height {height}.");
            continue;
        }

        let block = rpc_call(config, "getblock", json!([hash, 2]))?;
        let current_reward =
            coinbase_reward_for_address(&block, hash, height, &config.payout_address)?;
        if current_reward != Some(reward_atoms) {
            return Err(io::Error::other(
                "saved candidate reward no longer matches the canonical coinbase output",
            ));
        }
        if block_confirmations(&block)? < BTC_COINBASE_MATURITY {
            continue;
        }
        if block_hash_at(config, height)? != hash {
            return Err(io::Error::other(
                "Bitcoin chain changed while a mature block was being verified",
            ));
        }
        post_mature_block(config, hash, reward_atoms)?;
        println!("Recorded mature BTC collective block at height {height}.");
    }
    Ok(scanned)
}

/// Poll the local Bitcoin Core node and prepare mature-block PPLNS allocations.
pub fn run(value: &Value) -> io::Result<()> {
    let config = WatchConfig::from_value(value)?;
    let mut cursor = CursorStore::open(&config.cursor_path, config.start_height)?;
    println!(
        "BDL BTC collective block watcher started; scanning from height {} with 100-confirmation maturity.",
        cursor.cursor.next_height
    );
    loop {
        if let Err(error) = poll_once(&config, &mut cursor) {
            eprintln!("BTC collective block watcher paused: {error}");
        }
        thread::sleep(config.poll_interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn config(root: &Path) -> Value {
        json!({
            "rpcUrl":"http://127.0.0.1:8332/",
            "rpcUser":"rpcuser",
            "rpcPassword":"rpcpassword",
            "coordinatorUrl":"http://127.0.0.1:8082/api/v1/collective",
            "backendToken":"a".repeat(64),
            "payoutAddress":"1BoatSLRHtKNngkdXEeobR76b53LETtpyT",
            "windowWorkUnits":1000000000_u64,
            "startHeight":900000_u64,
            "cursorPath":root.join("btc-block-cursor.json").display().to_string(),
            "pollIntervalSeconds":15
        })
    }

    #[test]
    fn config_rejects_non_loopback_nodes_and_invalid_settings() {
        let root = TempDir::new().unwrap();
        assert!(WatchConfig::from_value(&config(root.path())).is_ok());

        let mut remote = config(root.path());
        remote["rpcUrl"] = "http://203.0.113.10:8332/".into();
        assert!(WatchConfig::from_value(&remote).is_err());

        let mut bad_window = config(root.path());
        bad_window["windowWorkUnits"] = 0.into();
        assert!(WatchConfig::from_value(&bad_window).is_err());

        let mut bad_address = config(root.path());
        bad_address["payoutAddress"] = "not-a-bitcoin-address".into();
        assert!(WatchConfig::from_value(&bad_address).is_err());

        let mut wrong_coordinator = config(root.path());
        wrong_coordinator["coordinatorUrl"] = "http://example.com:8082/api/v1/collective".into();
        assert!(WatchConfig::from_value(&wrong_coordinator).is_err());
    }

    #[test]
    fn basic_auth_header_is_correct_and_does_not_require_a_new_dependency() {
        assert_eq!(base64(b"user:pass"), "dXNlcjpwYXNz");
    }

    #[test]
    fn coinbase_output_to_the_collective_address_is_detected_before_maturity() {
        let address = "1BoatSLRHtKNngkdXEeobR76b53LETtpyT";
        let block_hash = "b".repeat(64);
        let block = json!({
            "hash":block_hash,
            "height":900000,
            "confirmations":100,
            "tx":[{
                "vin":[{"coinbase":"03abcdef"}],
                "vout":[
                    {"value":3.125,"scriptPubKey":{"address":address}},
                    {"value":1.0,"scriptPubKey":{"address":"1BitcoinEaterAddressDontSendf59kuE"}},
                    {"value":0.0,"scriptPubKey":{"type":"nulldata"}}
                ]
            }]
        });
        let reward = coinbase_reward_for_address(&block, &block_hash, 900000, address).unwrap();
        assert_eq!(reward, Some(312_500_000));
    }

    #[test]
    fn immature_or_replaced_blocks_are_never_accounted() {
        let address = "1BoatSLRHtKNngkdXEeobR76b53LETtpyT";
        let block_hash = "c".repeat(64);
        let mut block = json!({
            "hash":block_hash,
            "height":900001,
            "confirmations":99,
            "tx":[{
                "vin":[{"coinbase":"03abcdef"}],
                "vout":[{"value":3.125,"scriptPubKey":{"address":address}}]
            }]
        });
        assert_eq!(
            coinbase_reward_for_address(&block, &block_hash, 900001, address).unwrap(),
            Some(312_500_000)
        );
        assert_eq!(block_confirmations(&block).unwrap(), 99);
        block["confirmations"] = 100.into();
        assert_eq!(block_confirmations(&block).unwrap(), 100);
        assert!(coinbase_reward_for_address(&block, &"d".repeat(64), 900001, address).is_err());
    }

    #[test]
    fn cursor_is_durable_and_holds_a_single_process_lock() {
        let root = TempDir::new().unwrap();
        let path = root.path().join("btc-block-cursor.json");
        let mut store = CursorStore::open(&path, 100).unwrap();
        assert_eq!(store.cursor.next_height, 100);
        assert!(CursorStore::open(&path, 100).is_err());
        store.advance(100, &"e".repeat(64)).unwrap();
        drop(store);

        let reopened = CursorStore::open(&path, 100).unwrap();
        assert_eq!(reopened.cursor.next_height, 101);
        assert_eq!(reopened.cursor.last_height, Some(100));
        assert_eq!(
            reopened.cursor.last_hash.as_deref(),
            Some("e".repeat(64).as_str())
        );
    }
}

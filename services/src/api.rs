use crate::http::Response;
use chrono::{DateTime, SecondsFormat, Utc};
use regex::Regex;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

// Shared compiled patterns keep large presence snapshots bounded in CPU time.
fn compiled(pattern: &str) -> std::result::Result<Regex, regex::Error> {
    static PATTERNS: LazyLock<Mutex<std::collections::HashMap<String, Regex>>> =
        LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));
    let mut patterns = PATTERNS.lock().unwrap();
    if let Some(regex) = patterns.get(pattern) {
        return Ok(regex.clone());
    }
    let regex = Regex::new(pattern)?;
    patterns.insert(pattern.into(), regex.clone());
    Ok(regex)
}
pub const MAX_FILE: u64 = 1_048_576;
pub const RATES: [(&str, &str); 5] = [
    ("1m", "hashrate1m"),
    ("5m", "hashrate5m"),
    ("1h", "hashrate1hr"),
    ("1d", "hashrate1d"),
    ("7d", "hashrate7d"),
];
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
pub struct Error {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}
impl Error {
    pub fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
    pub fn response(self) -> Response {
        Response::error(self.status, self.code, &self.message)
    }
}
fn unavailable() -> Error {
    Error::new(
        503,
        "statistics_unavailable",
        "Statistics are temporarily unavailable.",
    )
}
pub fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
pub fn number(value: &Value, integer: bool) -> Value {
    value
        .as_f64()
        .filter(|v| v.is_finite() && *v >= 0.0 && (!integer || v.fract() == 0.0))
        .map(|v| {
            if integer {
                json!(v as u64)
            } else {
                value.clone()
            }
        })
        .unwrap_or(Value::Null)
}
pub fn rate(value: &Value) -> Value {
    if value.is_number() {
        return number(value, false);
    }
    static RATE: LazyLock<Regex> = LazyLock::new(|| {
        compiled(r"^\s*(\d+(?:\.\d+)?)\s*([kKMGTPE]?)\s*(?:[hH](?:/s)?)?\s*$").unwrap()
    });
    let Some(text) = value.as_str().filter(|s| s.len() <= 40) else {
        return Value::Null;
    };
    let Some(c) = RATE.captures(text) else {
        return Value::Null;
    };
    let multiplier = " KMGTPE"
        .find(c[2].to_uppercase().chars().next().unwrap_or(' '))
        .unwrap_or(0);
    let parsed = c[1].parse::<f64>().unwrap_or(f64::NAN) * 1000_f64.powi(multiplier as i32);
    if parsed.is_finite() {
        json!(parsed)
    } else {
        Value::Null
    }
}
pub fn timestamp(value: &Value) -> Value {
    value
        .as_str()
        .filter(|s| s.len() <= 40)
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .filter(|d| d.timestamp_millis() as f64 <= now() * 1000.0 + 60000.0)
        .map(|d| {
            json!(d
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Secs, true))
        })
        .unwrap_or(Value::Null)
}
pub fn epoch(value: &Value) -> Value {
    number(value, false)
        .as_f64()
        .filter(|v| *v > 0.0 && *v < 253_402_300_800.0)
        .and_then(|v| DateTime::from_timestamp(v as i64, 0))
        .map(|d| timestamp(&json!(d.to_rfc3339())))
        .unwrap_or(Value::Null)
}
pub fn freshness(stamp: Value, max: f64) -> Value {
    let age = stamp
        .as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| (now() - d.timestamp() as f64).max(0.0) as u64);
    json!({"updatedAt":stamp,"ageSeconds":age,"stale":age.map(|age|age as f64>max).unwrap_or(true)})
}
pub fn canonical(address: &str, coin: &str) -> Option<String> {
    if coin == "btc" {
        static BTC: LazyLock<Regex> = LazyLock::new(|| {
            compiled(r"^(?:[13][1-9A-HJ-NP-Za-km-z]{25,34}|bc1[ac-hj-np-z02-9]{11,87})$").unwrap()
        });
        BTC.is_match(address).then(|| address.into())
    } else if coin == "bch" {
        crate::address::canonical(address, true)
    } else {
        None
    }
}
pub fn worker_identity(name: &str, coin: &str) -> Option<(String, Option<String>)> {
    if name.len() > 128 {
        return None;
    }
    if let Some(address) = canonical(name, coin) {
        return Some((address, None));
    }
    for (index, character) in name.char_indices() {
        if character != '.' && character != '_' {
            continue;
        }
        let (address, worker) = (&name[..index], &name[index + 1..]);
        if worker.is_empty()
            || worker.len() > 64
            || !worker
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            continue;
        }
        if let Some(address) = canonical(address, coin) {
            return Some((address, Some(worker.into())));
        }
    }
    None
}
pub fn device_from_agent(agent: &Value) -> Value {
    let mut result = json!({"model":null,"software":null,"version":null});
    let Some(agent) = agent.as_str().filter(|s| s.len() <= 256) else {
        return result;
    };
    for (pattern, label) in [
        (r"bdlminer\s*v?2", "BDLMiner V2"),
        (r"nerdminer\s*v?2?", "NerdMiner V2"),
        (r"esp[- ]?miner", "ESP-Miner"),
        (r"axeos", "AxeOS"),
        (r"nerdqaxe", "NerdQAxe"),
        (r"nerdaxe", "NerdAxe"),
        (r"cgminer", "cgminer"),
        (r"bfgminer", "BFGMiner"),
        (r"braiins(?:\s*os)?", "Braiins OS"),
        (r"cpuminer", "cpuminer"),
        (r"antminer", "Antminer"),
        (r"whatsminer", "Whatsminer"),
    ] {
        if let Some(found) = compiled(&format!("(?i){pattern}")).unwrap().find(agent) {
            result["software"] = label.into();
            if let Some(c) =
                compiled(r"(?i)^[ /v:_-]*(\d{1,3}(?:\.\d{1,3}){1,3}(?:[-+][A-Za-z0-9._-]{1,16})?)")
                    .unwrap()
                    .captures(&agent[found.end()..])
            {
                result["version"] = c[1].into();
            }
            break;
        }
    }
    for (pattern, label) in [
        (r"m5stickc\s*(?:plus\s*2|\+2)", "M5StickC Plus2"),
        (r"m5stickc\s*(?:plus|\+)", "M5StickC Plus"),
        (r"m5stack\s*core\s*s3", "M5Stack CoreS3"),
        (r"m5stack\s*core\s*2", "M5Stack Core2"),
    ] {
        if compiled(&format!("(?i){pattern}")).unwrap().is_match(agent) {
            result["model"] = label.into();
            break;
        }
    }
    if let Some(c) = compiled(r"(?i)\bbitaxe(?:[ /_-]+(gamma|supra|ultra|hex))?\b")
        .unwrap()
        .captures(agent)
    {
        let mut model = "Bitaxe".to_string();
        if let Some(name) = c.get(1) {
            let lower = name.as_str().to_lowercase();
            model += ' '.to_string().as_str();
            model += &lower[..1].to_uppercase();
            model += &lower[1..];
        }
        result["model"] = model.into();
    }
    for (pattern, prefix) in [
        (
            r"\bantminer[ /_-]+(s\d{1,2}[a-z]?(?:\s+(?:pro|xp|hydro|hyd))?)\b",
            "Antminer ",
        ),
        (
            r"\bwhatsminer[ /_-]+(m\d{1,3}[a-z]?(?:\+{1,2})?)\b",
            "Whatsminer ",
        ),
    ] {
        if let Some(c) = compiled(&format!("(?i){pattern}")).unwrap().captures(agent) {
            result["model"] = (prefix.to_string() + &c[1].to_uppercase()).into();
        }
    }
    result
}
fn clean_device(value: &Value) -> Value {
    const SOFTWARE: &[&str] = &[
        "BDLMiner V2",
        "NerdMiner V2",
        "ESP-Miner",
        "AxeOS",
        "NerdQAxe",
        "NerdAxe",
        "cgminer",
        "BFGMiner",
        "Braiins OS",
        "cpuminer",
        "Antminer",
        "Whatsminer",
    ];
    const MODELS: &[&str] = &[
        "M5StickC Plus2",
        "M5StickC Plus",
        "M5Stack CoreS3",
        "M5Stack Core2",
        "Bitaxe Gamma",
        "Bitaxe Supra",
        "Bitaxe Ultra",
        "Bitaxe Hex",
        "Bitaxe",
    ];
    let model=value["model"].as_str().filter(|s|MODELS.contains(s)||compiled(r"^(?:Antminer S\d{1,2}[A-Z]?(?: (?:PRO|XP|HYDRO|HYD))?|Whatsminer M\d{1,3}[A-Z]?\+{0,2})$").unwrap().is_match(s));
    let software = value["software"].as_str().filter(|s| SOFTWARE.contains(s));
    let version = value["version"].as_str().filter(|s| {
        compiled(r"^\d{1,3}(?:\.\d{1,3}){1,3}(?:[-+][A-Za-z0-9._-]{1,16})?$")
            .unwrap()
            .is_match(s)
    });
    json!({"model":model,"software":software,"version":version})
}
type Cached = (PathBuf, Instant, Value, f64);
static CACHE: LazyLock<Mutex<VecDeque<Cached>>> = LazyLock::new(|| Mutex::new(VecDeque::new()));
pub fn read_json(path: &Path) -> Result<(Value, f64)> {
    if let Some((_, _, value, time)) = CACHE
        .lock()
        .unwrap()
        .iter()
        .find(|(p, t, _, _)| p == path && t.elapsed() < Duration::from_secs(5))
    {
        return Ok((value.clone(), *time));
    }
    let mut result = None;
    for attempt in 0..2 {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        let file = options.open(path).map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                Error::new(404, "file_not_found", "File not found.")
            } else {
                unavailable()
            }
        })?;
        let before = file.metadata().map_err(|_| unavailable())?;
        if !before.is_file() || before.len() > MAX_FILE {
            return Err(unavailable());
        }
        let mut raw = Vec::new();
        let mut stream = file.take(MAX_FILE + 1);
        stream.read_to_end(&mut raw).map_err(|_| unavailable())?;
        let after = stream.into_inner().metadata().map_err(|_| unavailable())?;
        let stable = before.len() == after.len() && before.modified().ok() == after.modified().ok();
        #[cfg(unix)]
        let stable = stable && before.mtime_nsec() == after.mtime_nsec();
        let mut merged = Map::new();
        let mut valid = stable && raw.len() as u64 <= MAX_FILE;
        for value in serde_json::Deserializer::from_slice(&raw).into_iter::<Value>() {
            match value {
                Ok(Value::Object(value)) => merged.extend(value),
                _ => {
                    valid = false;
                    break;
                }
            }
        }
        if valid && !merged.is_empty() {
            let modified = before
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            result = Some((Value::Object(merged), modified));
            break;
        }
        if attempt == 0 {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    let (value, modified) = result.ok_or_else(unavailable)?;
    let mut cache = CACHE.lock().unwrap();
    cache.retain(|(p, _, _, _)| p != path);
    if cache.len() >= 32 {
        cache.pop_front();
    }
    cache.push_back((path.into(), Instant::now(), value.clone(), modified));
    Ok((value, modified))
}
fn merge(mut a: Value, b: Value) -> Value {
    if let (Some(a), Some(b)) = (a.as_object_mut(), b.as_object()) {
        a.extend(b.clone());
    }
    a
}
fn measurements(data: &Value) -> Value {
    let rates: Map<String, Value> = RATES
        .iter()
        .map(|(p, key)| (p.to_string(), rate(&data[*key])))
        .collect();
    json!({"hashrateHps":rates,"shares":number(&data["shares"],true),"bestShare":number(&data["bestshare"],false),"bestEver":number(&data["bestever"],false),"lastShareAt":epoch(&data["lastshare"])})
}
fn valid_public_key(value: &Value) -> bool {
    let Some(key) = value.as_str().filter(|s| (40..=70).contains(&s.len())) else {
        return false;
    };
    let Ok(raw) = bs58::decode(key).into_vec() else {
        return false;
    };
    raw.len() == 38
        && raw[..2] == [1, 0]
        && Sha256::digest(Sha256::digest(&raw[..34]))[..4] == raw[34..]
}
pub struct PoolApi {
    pub coin: String,
    pub config: Value,
    coins: Option<Box<PoolApi>>,
    snapshot: PathBuf,
    logs: PathBuf,
    key: PathBuf,
    online: PathBuf,
    host: String,
    api_host: String,
    v1: u16,
    v2: u16,
    api_port: u16,
}
impl PoolApi {
    pub fn new(config: Value, coin: &str) -> Result<Self> {
        let host = config["host"]
            .as_str()
            .unwrap_or("bdl-pool.duckdns.org")
            .to_string();
        let api_host = config["apiHost"].as_str().unwrap_or(&host).to_string();
        let pattern = compiled(r"^[a-z0-9](?:[a-z0-9.-]{0,251}[a-z0-9])?$").unwrap();
        if !pattern.is_match(&host) || !pattern.is_match(&api_host) {
            return Err(Error::new(400, "invalid_config", "Invalid public hostname"));
        }
        let port = |key: &str, default: u64| -> Result<u16> {
            let value = config
                .get(key)
                .map(|v| v.as_u64())
                .unwrap_or(Some(default))
                .filter(|n| (1..=65535).contains(n))
                .ok_or_else(|| Error::new(400, "invalid_config", "Invalid public port"))?;
            Ok(value as u16)
        };
        let v1 = port("v1Port", 3333)?;
        let v2 = port("v2Port", 3336)?;
        let api_port = port("apiPublicPort", 443)?;
        let path =
            |key: &str, default: &str| PathBuf::from(config[key].as_str().unwrap_or(default));
        let snapshot = path("snapshot", "");
        let logs = path("logs", "");
        let key = path("sv2Public", "");
        let online = path("onlineSnapshot", "/var/lib/bdl-pool-presence/online.json");
        if snapshot.as_os_str().is_empty()
            || logs.as_os_str().is_empty()
            || key.as_os_str().is_empty()
        {
            return Err(Error::new(
                400,
                "invalid_config",
                "Missing statistics paths",
            ));
        }
        let coins = if coin == "btc" && config["coins"].get("bch").is_some() {
            Some(Box::new(Self::new(config["coins"]["bch"].clone(), "bch")?))
        } else {
            None
        };
        Ok(Self {
            coin: coin.into(),
            config,
            coins,
            snapshot,
            logs,
            key,
            online,
            host,
            api_host,
            v1,
            v2,
            api_port,
        })
    }
    pub fn pool(&self) -> Result<Value> {
        let (data, _) = read_json(&self.snapshot).map_err(|e| {
            if e.status == 404 {
                Error::new(
                    503,
                    "snapshot_unavailable",
                    "No monitoring snapshot is available.",
                )
            } else {
                e
            }
        })?;
        let stamp = timestamp(&data["updatedAt"]);
        if stamp.is_null() || !matches!(data["source"].as_str(), Some("snapshot" | "live")) {
            return Err(Error::new(
                503,
                "snapshot_unavailable",
                "No valid monitoring snapshot is available.",
            ));
        }
        if data["coin"].as_str().unwrap_or("btc") != self.coin {
            return Err(Error::new(
                503,
                "wrong_network",
                "The snapshot belongs to another network.",
            ));
        }
        let metadata = freshness(stamp.clone(), 900.0);
        let raw = &data["node"];
        let checks = &data["checks"];
        let progress = number(&raw["verificationProgress"], false);
        let node = json!({"blocks":number(&raw["blocks"],true),"headers":number(&raw["headers"],true),"chain":raw["chain"].as_str().filter(|s|matches!(*s,"main"|"test"|"testnet4"|"signet"|"regtest")),"verificationProgress":progress.as_f64().filter(|p|*p<=1.0),"initialBlockDownload":raw["initialBlockDownload"].as_bool()});
        let mut node_stamp = timestamp(&raw["updatedAt"]);
        if node_stamp.is_null() && checks["bitcoin"] == "ok" {
            node_stamp = stamp.clone();
        }
        let node = merge(node, freshness(node_stamp, 900.0));
        let mut public = Map::new();
        for (key, allowed) in [
            ("bitcoin", &["ok", "stale", "unavailable"][..]),
            ("poolStats", &["ok", "missing", "invalid", "stale"][..]),
            (
                "stratum",
                &[
                    "job_received",
                    "subscribed_no_job",
                    "unavailable",
                    "not_checked",
                    "invalid",
                ][..],
            ),
        ] {
            public.insert(
                key.into(),
                json!(checks[key]
                    .as_str()
                    .filter(|s| allowed.contains(s))
                    .unwrap_or("unknown")),
            );
        }
        let raw = &data["pool"];
        let mut pool = Map::new();
        for key in ["hashrateHps", "workers", "bestShare", "blocksFound"] {
            pool.insert(
                key.into(),
                number(&raw[key], matches!(key, "workers" | "blocksFound")),
            );
        }
        let ready = raw["ready"] == true
            && metadata["stale"] == false
            && node["stale"] == false
            && ["chain", "blocks", "headers", "verificationProgress"]
                .iter()
                .all(|k| !node[k].is_null())
            && node["chain"] == "main"
            && node["initialBlockDownload"] == false
            && public["bitcoin"] == "ok"
            && public["stratum"] == "job_received";
        pool.insert("readyAtLastCheck".into(), ready.into());
        Ok(merge(
            json!({"apiVersion":"1","source":data["source"],"pool":pool,"node":node,"checks":public}),
            metadata,
        ))
    }
    fn history(&self) -> Result<Value> {
        self.pool()?;
        let (data, _) = read_json(&self.snapshot)?;
        let rows = data["history"].as_array();
        let points: Vec<_> = rows
            .into_iter()
            .flatten()
            .skip(rows.map(|r| r.len().saturating_sub(288)).unwrap_or(0))
            .filter_map(|item| {
                let stamp = timestamp(&item["time"]);
                let rate = number(&item["hashrateHps"], false);
                (!stamp.is_null() && !rate.is_null())
                    .then(|| json!({"time":stamp,"hashrateHps":rate}))
            })
            .collect();
        Ok(json!({"apiVersion":"1","updatedAt":timestamp(&data["updatedAt"]),"points":points}))
    }
    fn stratum(&self) -> Value {
        let key = read_json(&self.key)
            .ok()
            .and_then(|(data, _)| {
                (self.coin == "btc" && valid_public_key(&data["authorityPublicKey"]))
                    .then(|| data["authorityPublicKey"].clone())
            })
            .unwrap_or(Value::Null);
        json!({"apiVersion":"1","v1":{"host":self.host,"port":self.v1,"url":format!("stratum+tcp://{}:{}",self.host,self.v1)},"v2":{"configured":!key.is_null(),"host":if self.coin=="btc"{json!(self.host)}else{Value::Null},"port":if self.coin=="btc"{json!(self.v2)}else{Value::Null},"authorityPublicKey":key,"url":key.as_str().map(|key|format!("stratum2+tcp://{}:{}/{key}",self.host,self.v2)),"availability":"not_verified"}})
    }
    fn presence(&self) -> (Value, Vec<Value>) {
        let data = read_json(&self.online).map(|v| v.0).unwrap_or(Value::Null);
        let metadata = freshness(timestamp(&data["updatedAt"]), 45.0);
        let mut available = data["schemaVersion"] == 1
            && data["available"] == true
            && metadata["stale"] == false
            && data["coin"].as_str().unwrap_or("btc") == self.coin;
        let mut status = if available {
            "ok"
        } else if data["available"] == true {
            "stale"
        } else {
            "unavailable"
        };
        if !available {
            if let Ok(pool) = self.pool() {
                if pool["node"]["initialBlockDownload"] == true && pool["node"]["stale"] == false {
                    status = "syncing";
                }
            }
        }
        let mut clients = Vec::new();
        if available {
            if let Some(rows) = data["clients"].as_array().filter(|r| r.len() <= 2000) {
                for row in rows {
                    let Some(name) = row["name"].as_str() else {
                        continue;
                    };
                    let Some((address, worker)) = worker_identity(name, &self.coin) else {
                        continue;
                    };
                    let Some(id) = row["id"].as_str().filter(|s| {
                        s.len() == 24
                            && s.bytes()
                                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    }) else {
                        continue;
                    };
                    if row["address"] != address {
                        continue;
                    }
                    let rates: Map<String, Value> = RATES
                        .iter()
                        .map(|(p, _)| (p.to_string(), number(&row["hashrateHps"][*p], false)))
                        .collect();
                    clients.push(json!({"id":id,"address":address,"name":if self.coin=="btc"{name.to_string()}else{address.clone()+&worker.as_ref().map(|w|format!(".{w}")).unwrap_or_default()},"worker":worker,"device":clean_device(&row["device"]),"hashrateHps":rates,"bestShare":number(&row["bestShare"],false),"connectedAt":timestamp(&row["connectedAt"]),"lastShareAt":timestamp(&row["lastShareAt"])}));
                }
            } else {
                available = false;
                status = "unavailable";
            }
        }
        (
            json!({"available":available,"status":status,"updatedAt":metadata["updatedAt"],"checkedAt":timestamp(&data["checkedAt"]),"stale":!available}),
            clients,
        )
    }
    fn online(&self, limit: usize, offset: usize) -> Value {
        let (metadata, clients) = self.presence();
        let regex = compiled(if self.coin == "btc" {
            r"(?:[13][1-9A-HJ-NP-Za-km-z]{25,34}|bc1[ac-hj-np-z02-9]{11,87})"
        } else {
            r"(?i)(?:bitcoincash:)?[qp][a-z0-9]{41}"
        })
        .unwrap();
        let mask = |s: &str| format!("{}…{}", &s[..6], &s[s.len() - 4..]);
        let mut public: Vec<_> = clients
            .into_iter()
            .map(|mut client| {
                let address = client["address"].as_str().unwrap_or_default();
                client["addressMasked"] = mask(address).into();
                if let Some(worker) = client["worker"].as_str() {
                    client["worker"] = regex
                        .replace_all(worker, |c: &regex::Captures| mask(&c[0]))
                        .to_string()
                        .into();
                }
                client.as_object_mut().unwrap().remove("address");
                client.as_object_mut().unwrap().remove("name");
                client
            })
            .collect();
        public.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        let total = if metadata["available"] == true {
            json!(public.len())
        } else {
            Value::Null
        };
        let miners: Vec<_> = public.into_iter().skip(offset).take(limit).collect();
        merge(
            json!({"apiVersion":"1","total":total,"limit":limit,"offset":offset,"miners":miners}),
            metadata,
        )
    }
    fn miner(&self, address: &str) -> Result<(Value, Vec<Value>)> {
        let address = canonical(address, &self.coin).ok_or_else(|| {
            Error::new(
                400,
                "invalid_address",
                format!(
                    "Use a mainnet {} without a worker suffix.",
                    if self.coin == "btc" {
                        "Bitcoin address"
                    } else {
                        "Bitcoin Cash CashAddr"
                    }
                ),
            )
        })?;
        let directory = self.logs.join("users");
        if fs::symlink_metadata(&directory)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(unavailable());
        }
        let (data, modified) = read_json(&directory.join(&address)).map_err(|e| {
            if e.status == 404 {
                Error::new(
                    404,
                    "miner_not_found",
                    "No CKPool statistics are recorded for this address.",
                )
            } else {
                e
            }
        })?;
        let empty = Vec::new();
        let rows = data
            .get("worker")
            .map(|v| v.as_array())
            .unwrap_or(Some(&empty))
            .filter(|r| r.len() <= 5000)
            .ok_or_else(unavailable)?;
        let mut workers = Vec::new();
        for row in rows {
            if !row.is_object() {
                continue;
            }
            let Some(name) = row["workername"].as_str() else {
                continue;
            };
            let canonical_name = if self.coin == "btc" {
                if valid_worker(name)
                    && (name == address
                        || name.starts_with(&format!("{address}."))
                        || name.starts_with(&format!("{address}_")))
                {
                    Some(name.to_string())
                } else {
                    None
                }
            } else {
                worker_identity(name, &self.coin)
                    .filter(|(a, _)| a == &address)
                    .map(|(_, worker)| {
                        address.clone() + &worker.map(|w| format!(".{w}")).unwrap_or_default()
                    })
            };
            if let Some(name) = canonical_name {
                workers.push(merge(json!({"name":name}), measurements(row)));
            }
        }
        workers.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        let (presence, clients) = self.presence();
        let clients: Vec<_> = clients
            .into_iter()
            .filter(|c| c["address"] == address)
            .collect();
        let available = presence["available"] == true;
        for worker in &mut workers {
            let connected: Vec<_> = clients
                .iter()
                .filter(|c| c["name"] == worker["name"])
                .collect();
            worker["online"] = if available {
                json!(!connected.is_empty())
            } else {
                Value::Null
            };
            worker["connections"] = if available {
                json!(connected.len())
            } else {
                Value::Null
            };
            worker["devices"] = connected.iter().map(|c| c["device"].clone()).collect();
            worker["presenceUpdatedAt"] = presence["updatedAt"].clone();
        }
        let result = merge(
            merge(
                json!({"apiVersion":"1","address":address,"workersActive":number(&data["workers"],true),"workersRecorded":workers.len(),"connections":if available{json!(clients.len())}else{Value::Null},"presenceAvailable":available,"presenceUpdatedAt":presence["updatedAt"]}),
                freshness(epoch(&json!(modified)), 180.0),
            ),
            measurements(&data),
        );
        Ok((result, workers))
    }
    pub fn dispatch(&self, target: &str) -> Result<Value> {
        if target.len() > 2048 {
            return Err(Error::new(414, "uri_too_long", "Request URL is too long."));
        }
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        let path = percent_encoding::percent_decode_str(path)
            .decode_utf8()
            .map_err(|_| Error::new(400, "invalid_request", "Invalid request URL."))?;
        let path = path.trim_end_matches('/');
        let route = path.strip_prefix("/api/v1/").ok_or_else(not_found)?;
        if self.coin == "btc" && route.starts_with("coins/") {
            let parts: Vec<_> = route.splitn(3, '/').collect();
            if parts.len() != 3 || parts[1] != "bch" {
                return Err(Error::new(404, "not_found", "Unknown coin or endpoint."));
            }
            let coin = self.coins.as_ref().ok_or_else(|| {
                Error::new(
                    503,
                    "coin_not_configured",
                    "Bitcoin Cash is not configured on this server.",
                )
            })?;
            let mut result = coin.dispatch(&format!(
                "/api/v1/{}{}",
                parts[2],
                if query.is_empty() {
                    String::new()
                } else {
                    format!("?{query}")
                }
            ))?;
            result["coin"] = "bch".into();
            return Ok(result);
        }
        let parts: Vec<_> = route.split('/').collect();
        let paginated = route == "online"
            || (parts.len() == 3 && parts[0] == "miners" && parts[2] == "workers");
        let mut limit = 50;
        let mut offset = 0;
        if !query.is_empty() {
            if !paginated {
                return Err(Error::new(
                    400,
                    "invalid_query",
                    "This endpoint does not accept query parameters.",
                ));
            }
            if query.split('&').count() > 4 || query.split('&').any(|p| !p.contains('=')) {
                return Err(Error::new(400, "invalid_request", "Invalid request URL."));
            }
            let mut seen = Vec::new();
            for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
                if !matches!(key.as_ref(), "limit" | "offset")
                    || seen.contains(&key.to_string())
                    || value.is_empty()
                    || value.len() > 6
                    || !value.bytes().all(|b| b.is_ascii_digit())
                {
                    return Err(Error::new(
                        400,
                        "invalid_query",
                        "Use one integer limit and offset.",
                    ));
                }
                seen.push(key.to_string());
                let value = value.parse::<usize>().unwrap();
                if key == "limit" {
                    limit = value;
                } else {
                    offset = value;
                }
            }
        }
        if paginated && !(1..=200).contains(&limit) {
            return Err(Error::new(
                400,
                "invalid_query",
                "limit must be between 1 and 200.",
            ));
        }
        match route {
            "health" => return Ok(json!({"apiVersion":"1","status":"ok"})),
            "coins" => {
                return Ok(
                    json!({"apiVersion":"1","coins":[{"id":"btc","configured":true},{"id":"bch","configured":self.coins.is_some()}]}),
                )
            }
            "pool" => return self.pool(),
            "history" => return self.history(),
            "stratum" => return Ok(self.stratum()),
            "online" => return Ok(self.online(limit, offset)),
            "openapi.json" => {
                let mut spec: Value = serde_json::from_str(include_str!("../../api/openapi.json"))
                    .map_err(|_| unavailable())?;
                spec["servers"] = json!([{"url":format!("https://{}{}",self.api_host,if self.api_port==443{String::new()}else{format!(":{}",self.api_port)})}]);
                return Ok(spec);
            }
            _ => {}
        }
        if (2..=4).contains(&parts.len())
            && parts[0] == "miners"
            && (parts.len() == 2 || parts[2] == "workers")
        {
            let address = canonical(parts[1], &self.coin).unwrap_or_else(|| parts[1].into());
            let (miner, workers) = self.miner(&address)?;
            if parts.len() == 2 {
                return Ok(miner);
            }
            if parts.len() == 3 {
                let total = workers.len();
                let workers: Vec<_> = workers.into_iter().skip(offset).take(limit).collect();
                return Ok(
                    json!({"apiVersion":"1","address":address,"updatedAt":miner["updatedAt"],"stale":miner["stale"],"total":total,"limit":limit,"offset":offset,"workers":workers}),
                );
            }
            let name = parts[3];
            if !valid_worker(name)
                && !(self.coin == "bch" && worker_identity(name, "bch").is_some())
            {
                return Err(Error::new(400, "invalid_worker", "Invalid worker name."));
            }
            let matches: Vec<_> = workers
                .into_iter()
                .filter(|w| {
                    w["name"] == name
                        || w["name"] == format!("{address}.{name}")
                        || w["name"] == format!("{address}_{name}")
                })
                .collect();
            if matches.len() != 1 {
                return Err(Error::new(
                    if matches.is_empty() { 404 } else { 400 },
                    if matches.is_empty() {
                        "worker_not_found"
                    } else {
                        "ambiguous_worker"
                    },
                    "Use the full worker name returned by the workers endpoint.",
                ));
            }
            return Ok(
                json!({"apiVersion":"1","address":address,"updatedAt":miner["updatedAt"],"stale":miner["stale"],"worker":matches[0]}),
            );
        }
        Err(not_found())
    }
}
fn valid_worker(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn not_found() -> Error {
    Error::new(404, "not_found", "Endpoint not found.")
}

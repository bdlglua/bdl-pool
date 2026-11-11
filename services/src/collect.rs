//! Public monitoring snapshot: local RPC, bounded CKPool reads, passive subscribe.
use crate::api;
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{json, Map, Value};
use std::{
    fs,
    io::{self, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, UNIX_EPOCH},
};
pub fn run_bounded(command: &mut Command, timeout: Duration, limit: u64) -> io::Result<Vec<u8>> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("Missing subprocess output"))?;
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Local command deadline",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let output = reader
        .join()
        .map_err(|_| io::Error::other("Local command reader failed"))??;
    if !status.success() || output.len() as u64 > limit {
        return Err(io::Error::other("Local command failed"));
    }
    Ok(output)
}
fn utc(now: f64) -> Value {
    DateTime::from_timestamp(now as i64, 0)
        .map(|date| json!(date.to_rfc3339_opts(SecondsFormat::Secs, true)))
        .unwrap_or(Value::Null)
}
pub fn filtered_node(data: &Value) -> Option<Value> {
    let chain = data["chain"]
        .as_str()
        .filter(|s| matches!(*s, "main" | "test" | "testnet4" | "signet" | "regtest"))?;
    let blocks = api::number(&data["blocks"], true);
    let headers = api::number(&data["headers"], true);
    let progress = api::number(&data["verificationProgress"], false)
        .as_f64()
        .filter(|p| *p <= 1.0)?;
    let syncing = data["initialBlockDownload"].as_bool()?;
    if blocks.is_null() || headers.is_null() {
        return None;
    }
    Some(
        json!({"chain":chain,"blocks":blocks,"headers":headers,"verificationProgress":progress,"initialBlockDownload":syncing}),
    )
}
fn node_info(cli: &str, args: &[String]) -> (Value, &'static str) {
    let empty = json!({"chain":null,"blocks":null,"headers":null,"verificationProgress":null,"initialBlockDownload":null,"updatedAt":null});
    for attempt in 0..2 {
        let output = run_bounded(
            Command::new(cli)
                .args(args)
                .args(["-rpcclienttimeout=30", "getblockchaininfo"]),
            Duration::from_secs(35),
            1024 * 1024,
        );
        if let Ok(output) = output {
            if let Ok(data) = serde_json::from_slice::<Value>(&output) {
                let raw = json!({"chain":data["chain"],"blocks":data["blocks"],"headers":data["headers"],"verificationProgress":data["verificationprogress"],"initialBlockDownload":data["initialblockdownload"]});
                if let Some(mut node) = filtered_node(&raw) {
                    node["updatedAt"] = utc(api::now());
                    return (node, "ok");
                }
            }
        }
        if attempt == 0 {
            thread::sleep(Duration::from_secs(1));
        }
    }
    eprintln!("Bitcoin Core: local reading unavailable after retry.");
    (empty, "unavailable")
}
pub fn node_from_snapshot(previous: &Value, now: f64) -> Option<Value> {
    if !matches!(previous["source"].as_str(), Some("live" | "snapshot"))
        || previous["checks"]["bitcoin"] == "unavailable"
    {
        return None;
    }
    let mut node = filtered_node(&previous["node"])?;
    let stamp = previous["node"]["updatedAt"].as_str().or_else(|| {
        if previous["checks"]["bitcoin"] != "stale" {
            previous["updatedAt"].as_str()
        } else {
            None
        }
    })?;
    let stamp = DateTime::parse_from_rfc3339(stamp).ok()?;
    if stamp.timestamp() as f64 > now + 60.0 {
        return None;
    }
    node["updatedAt"] = json!(stamp
        .with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Secs, true));
    Some(node)
}
fn previous_node(path: &Path, now: f64) -> Option<Value> {
    if let Ok(raw) = fs::read(path) {
        if let Ok(previous) = serde_json::from_slice::<Value>(&raw) {
            if let Some(node) = node_from_snapshot(&previous, now) {
                return Some(node);
            }
        }
    }
    let parent = path.parent()?;
    if !parent.join(".git").is_dir() {
        return None;
    }
    let filename = path.file_name()?.to_str()?;
    let deadline = Instant::now() + Duration::from_secs(6);
    let commits = run_bounded(
        Command::new("git").arg("-C").arg(parent).args([
            "log",
            "-n",
            "32",
            "--format=%H",
            "--",
            filename,
        ]),
        Duration::from_secs(3),
        8192,
    )
    .ok()?;
    for commit in String::from_utf8(commits).ok()?.lines() {
        if Instant::now() >= deadline {
            break;
        }
        if !(40..=64).contains(&commit.len()) || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let output = run_bounded(
            Command::new("git")
                .arg("-C")
                .arg(parent)
                .args(["show", &format!("{commit}:{filename}")]),
            deadline.saturating_duration_since(Instant::now()),
            1024 * 1024,
        )
        .ok()?;
        if let Ok(previous) = serde_json::from_slice::<Value>(&output) {
            if let Some(node) = node_from_snapshot(&previous, now) {
                eprintln!("Recovered the last successful node reading from local history.");
                return Some(node);
            }
        }
    }
    None
}
fn pool_path(config: &Path, explicit: Option<&str>) -> PathBuf {
    if let Some(path) = explicit {
        return path.into();
    }
    let logdir = fs::read_to_string(config)
        .ok()
        .and_then(|s| {
            let re = regex::Regex::new(r#""logdir"\s*:\s*("(?:[^"\\]|\\.)*")"#).unwrap();
            let values = re.captures_iter(&s).collect::<Vec<_>>();
            if values.len() == 1 {
                serde_json::from_str::<String>(&values[0][1]).ok()
            } else {
                None
            }
        })
        .unwrap_or_else(|| "logs".into());
    let logdir = Path::new(&logdir);
    if logdir.is_absolute() {
        return logdir.join("pool/pool.status");
    }
    if let Ok(output) = run_bounded(
        Command::new("systemctl").args([
            "show",
            "ckpool.service",
            "--property=WorkingDirectory",
            "--value",
        ]),
        Duration::from_secs(5),
        4096,
    ) {
        let directory = String::from_utf8_lossy(&output);
        let directory = Path::new(directory.trim());
        if directory.is_absolute() {
            let candidate = directory.join(logdir).join("pool/pool.status");
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    config
        .parent()
        .unwrap_or(Path::new("."))
        .join(logdir)
        .join("pool/pool.status")
}
pub fn pool_info(path: &Path, now: f64) -> (Value, &'static str) {
    let mut pool = json!({"ready":false,"hashrateHps":null,"workers":null,"bestShare":null,"blocksFound":null});
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) => {
            return (
                pool,
                if error.kind() == io::ErrorKind::NotFound {
                    "missing"
                } else {
                    "invalid"
                },
            )
        }
    };
    let modified = file
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64());
    let mut bytes = Vec::new();
    if file.take(262145).read_to_end(&mut bytes).is_err() || bytes.len() > 262144 {
        return (pool, "invalid");
    }
    let mut data = Map::new();
    for value in serde_json::Deserializer::from_slice(&bytes).into_iter::<Value>() {
        if let Ok(Value::Object(object)) = value {
            data.extend(object);
        } else {
            return (pool, "invalid");
        }
    }
    let updated = api::number(data.get("lastupdate").unwrap_or(&Value::Null), false)
        .as_f64()
        .or(modified);
    if !updated
        .map(|at| (-60.0..=180.0).contains(&(now - at)))
        .unwrap_or(false)
    {
        return (pool, "stale");
    }
    pool["hashrateHps"] = api::rate(data.get("hashrate5m").unwrap_or(&Value::Null));
    pool["workers"] = api::number(
        data.get("Workers")
            .or_else(|| data.get("workers"))
            .unwrap_or(&Value::Null),
        true,
    );
    pool["bestShare"] = api::number(data.get("bestshare").unwrap_or(&Value::Null), false);
    (pool, "ok")
}
pub fn stratum_check(host: &str, port: u16, timeout: Duration) -> &'static str {
    let deadline = Instant::now() + timeout;
    let mut subscribed = false;
    let result = (|| -> io::Result<bool> {
        let addresses = (host, port).to_socket_addrs()?;
        let mut stream = None;
        for address in addresses {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            if let Ok(connection) = TcpStream::connect_timeout(&address, remaining) {
                stream = Some(connection);
                break;
            }
        }
        let mut stream =
            stream.ok_or_else(|| io::Error::other("Stratum connection unavailable"))?;
        stream.set_write_timeout(Some(timeout))?;
        stream.write_all(
            b"{\"id\":1,\"method\":\"mining.subscribe\",\"params\":[\"BDL-status/1\"]}\n",
        )?;
        let mut buffer = Vec::new();
        let mut job = false;
        while Instant::now() < deadline {
            stream.set_read_timeout(Some(
                deadline
                    .saturating_duration_since(Instant::now())
                    .max(Duration::from_millis(10)),
            ))?;
            let mut chunk = [0_u8; 16384];
            let n = stream.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..n]);
            if buffer.len() > 131072 {
                break;
            }
            while let Some(end) = buffer.iter().position(|b| *b == b'\n') {
                let line: Vec<_> = buffer.drain(..=end).collect();
                if line.iter().all(|b| b.is_ascii_whitespace()) {
                    continue;
                }
                let value: Value = serde_json::from_slice(&line)?;
                if !value.is_object() {
                    continue;
                }
                if value["id"] == 1 && value["error"].is_null() {
                    let result = &value["result"];
                    subscribed = result
                        .as_array()
                        .map(|r| {
                            r.len() == 3
                                && r[0].is_array()
                                && r[1].is_string()
                                && r[2].as_u64().map(|n| n > 0).unwrap_or(false)
                        })
                        .unwrap_or(false);
                }
                if value["method"] == "mining.notify" {
                    job = value["params"]
                        .as_array()
                        .map(|p| {
                            p.len() >= 9
                                && p[0].as_str().map(|s| !s.is_empty()).unwrap_or(false)
                                && p[1].as_str().map(|s| s.len() == 64).unwrap_or(false)
                        })
                        .unwrap_or(false);
                }
            }
            if subscribed && job {
                return Ok(true);
            }
        }
        Ok(false)
    })();
    if matches!(result, Ok(true)) {
        "job_received"
    } else if subscribed {
        "subscribed_no_job"
    } else {
        "unavailable"
    }
}
pub fn history(previous: &Value, now: f64) -> Vec<Value> {
    let rows = previous["history"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut points = rows
        .iter()
        .skip(rows.len().saturating_sub(288))
        .filter_map(|p| {
            let stamp = DateTime::parse_from_rfc3339(p["time"].as_str()?)
                .ok()?
                .timestamp() as f64;
            let rate = api::number(&p["hashrateHps"], false);
            ((now - 86400.0..=now).contains(&stamp) && !rate.is_null())
                .then(|| (stamp, json!({"time":utc(stamp),"hashrateHps":rate})))
        })
        .collect::<Vec<_>>();
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    points.into_iter().map(|(_, p)| p).collect()
}
pub fn collect(args: &[String]) -> io::Result<Value> {
    let option = |key: &str| args.windows(2).find(|p| p[0] == key).map(|p| p[1].clone());
    let mut cli = option("--bitcoin-cli").unwrap_or_else(|| "/usr/local/bin/bitcoin-cli".into());
    let mut cli_args = Vec::new();
    let mut config =
        option("--config").unwrap_or_else(|| "/home/bdl/mining/ckpool/local.conf".into());
    let mut explicit = option("--pool-status");
    let mut host = option("--stratum-host").unwrap_or_else(|| "192.168.1.2".into());
    let mut port = option("--stratum-port")
        .unwrap_or_else(|| "3333".into())
        .parse::<u16>()
        .map_err(io::Error::other)?;
    let mut coin = option("--coin").unwrap_or_else(|| "btc".into());
    if let Some(profile) = option("--profile") {
        let profile: Value = serde_json::from_slice(&fs::read(profile)?)?;
        if profile["coin"] != "bch" {
            return Err(io::Error::other("BCH profile must declare coin=bch"));
        }
        coin = "bch".into();
        cli = profile["bitcoinCli"]
            .as_str()
            .ok_or_else(|| io::Error::other("Missing Bitcoin CLI"))?
            .into();
        cli_args = profile["cliArgs"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|v| {
                        v.as_str()
                            .map(str::to_string)
                            .ok_or_else(|| io::Error::other("Invalid CLI arguments"))
                    })
                    .collect::<io::Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        config = profile["config"]
            .as_str()
            .ok_or_else(|| io::Error::other("Missing pool config"))?
            .into();
        explicit = Some(
            profile["poolStatus"]
                .as_str()
                .ok_or_else(|| io::Error::other("Missing pool status"))?
                .into(),
        );
        host = profile["stratumHost"]
            .as_str()
            .unwrap_or("127.0.0.1")
            .into();
        port = profile["stratumPort"]
            .as_u64()
            .filter(|p| (1..=65535).contains(p))
            .ok_or_else(|| io::Error::other("Invalid Stratum port"))? as u16;
    }
    if !matches!(coin.as_str(), "btc" | "bch") || port == 0 {
        return Err(io::Error::other("Invalid monitoring network or port"));
    }
    let previous_path = option("--previous").map(PathBuf::from);
    let previous = previous_path
        .as_ref()
        .and_then(|p| fs::read(p).ok())
        .and_then(|raw| serde_json::from_slice::<Value>(&raw).ok())
        .unwrap_or(Value::Null);
    if !previous.is_null() && previous["coin"].as_str().unwrap_or("btc") != coin {
        return Err(io::Error::other(
            "Previous snapshot belongs to another network",
        ));
    }
    let (mut node, mut bitcoin) = node_info(&cli, &cli_args);
    if bitcoin != "ok" {
        if let Some(previous) = previous_path
            .as_ref()
            .and_then(|p| previous_node(p, api::now()))
        {
            node = previous;
            bitcoin = "stale";
        }
    }
    let (mut pool, statistics) = pool_info(
        &pool_path(Path::new(&config), explicit.as_deref()),
        api::now(),
    );
    let stratum = if bitcoin == "ok" && node["initialBlockDownload"] == false {
        stratum_check(&host, port, Duration::from_secs(5))
    } else {
        "not_checked"
    };
    pool["ready"] = (bitcoin == "ok"
        && node["chain"] == "main"
        && node["initialBlockDownload"] == false
        && stratum == "job_received")
        .into();
    let now = api::now();
    let stamp = utc(now);
    let mut history = history(&previous, now);
    if !pool["hashrateHps"].is_null() {
        history.push(json!({"time":stamp,"hashrateHps":pool["hashrateHps"]}));
    }
    let history = history
        .into_iter()
        .rev()
        .take(288)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    eprintln!("Bitcoin: {bitcoin}; CKPool statistics: {statistics}; Stratum: {stratum}.");
    Ok(
        json!({"schemaVersion":1,"coin":coin,"source":"snapshot","updatedAt":stamp,"node":node,"pool":pool,"history":history,"checks":{"bitcoin":bitcoin,"poolStats":statistics,"stratum":stratum}}),
    )
}

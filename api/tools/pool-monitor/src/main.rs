use serde::Deserialize;
use std::{env, error::Error, thread, time::Duration};

const DEFAULT_API_URL: &str = "https://api.bdl-pool.com:36018/api/v1";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PoolSnapshot {
    api_version: String,
    updated_at: Option<String>,
    age_seconds: Option<u64>,
    stale: bool,
    pool: PoolStats,
    node: NodeStatus,
    checks: Checks,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PoolStats {
    hashrate_hps: Option<f64>,
    workers: Option<u64>,
    best_share: Option<f64>,
    blocks_found: Option<u64>,
    ready_at_last_check: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeStatus {
    initial_block_download: Option<bool>,
    stale: bool,
}

#[derive(Deserialize)]
struct Checks {
    stratum: String,
}

fn format_hashrate(value: Option<f64>) -> String {
    let Some(mut rate) = value else {
        return "unknown".to_owned();
    };
    let units = ["H/s", "kH/s", "MH/s", "GH/s", "TH/s", "PH/s", "EH/s"];
    let mut unit = 0;
    while rate >= 1000.0 && unit < units.len() - 1 {
        rate /= 1000.0;
        unit += 1;
    }
    format!("{rate:.2} {}", units[unit])
}

fn report(snapshot: PoolSnapshot) -> Result<(), Box<dyn Error>> {
    if snapshot.api_version != "1" {
        return Err("Unsupported BDL Pool API version".into());
    }

    let state = if snapshot.stale || snapshot.node.stale {
        "STALE"
    } else if snapshot.node.initial_block_download == Some(true) {
        "SYNCING"
    } else if snapshot.pool.ready_at_last_check {
        "READY"
    } else {
        "UNCONFIRMED"
    };
    let age = snapshot
        .age_seconds
        .map(|seconds| format!("{seconds}s old"))
        .or(snapshot.updated_at)
        .unwrap_or_else(|| "age unknown".to_owned());
    let workers = snapshot
        .pool
        .workers
        .map_or_else(|| "unknown".to_owned(), |value| value.to_string());
    let blocks = snapshot
        .pool
        .blocks_found
        .map_or_else(|| "unknown".to_owned(), |value| value.to_string());
    let best_share = snapshot
        .pool
        .best_share
        .map_or_else(|| "unknown".to_owned(), |value| format!("{value:.2}"));

    println!(
        "{state} | {} | workers: {workers} | blocks: {blocks} | best share: {best_share} | Stratum: {} | {age}",
        format_hashrate(snapshot.pool.hashrate_hps),
        snapshot.checks.stratum
    );
    Ok(())
}

fn poll(client: &reqwest::blocking::Client, url: &str) -> Result<(), Box<dyn Error>> {
    let response = client
        .get(format!("{url}/pool"))
        .send()?
        .error_for_status()?;
    let snapshot: PoolSnapshot = response.json()?;
    report(snapshot)
}

fn main() -> Result<(), Box<dyn Error>> {
    let base_url = env::var("BDL_POOL_API_URL").unwrap_or_else(|_| DEFAULT_API_URL.to_owned());
    let base_url = base_url.trim_end_matches('/');
    let interval = env::var("BDL_POOL_POLL_SECONDS")
        .ok()
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(60);
    if interval < 5 {
        return Err("BDL_POOL_POLL_SECONDS must be at least 5".into());
    }

    let once = env::args().skip(1).any(|argument| argument == "--once");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()?;

    loop {
        if let Err(error) = poll(&client, base_url) {
            eprintln!("Pool check failed: {error}");
            if once {
                return Err(error);
            }
        }
        if once {
            break;
        }
        thread::sleep(Duration::from_secs(interval));
    }
    Ok(())
}

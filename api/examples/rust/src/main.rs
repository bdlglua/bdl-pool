use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct Health {
    #[serde(rename = "apiVersion")]
    api_version: String,
    status: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_url = std::env::var("BDL_POOL_API_URL")
        .unwrap_or_else(|_| "https://api.bdl-pool.com:36018/api/v1".to_owned());
    let base_url = base_url.trim_end_matches('/');
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()?;
    let health: Health = client
        .get(format!("{base_url}/health"))
        .send()?
        .error_for_status()?
        .json()?;

    if health.api_version != "1" || health.status != "ok" {
        return Err("API returned an invalid health response".into());
    }

    println!("BDL Pool API {}: {}", health.api_version, health.status);
    Ok(())
}

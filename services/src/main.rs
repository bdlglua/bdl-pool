use bdl_pool_services::{api::PoolApi, collect, http, install, presence};
use serde_json::Value;
use std::{env, fs, io, path::PathBuf, sync::Arc};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("api");
    let option = |key: &str| args.windows(2).find(|p| p[0] == key).map(|p| p[1].clone());
    if command == "collect" {
        println!(
            "{}",
            serde_json::to_string_pretty(&collect::collect(&args)?)?
        );
        return Ok(());
    }
    if command == "install" {
        install::dispatch(&args[1..])?;
        return Ok(());
    }
    let config = option("--config").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env::var_os("HOME").unwrap_or_default())
            .join(".config/bdl-pool-api/config.json")
    });
    let config: Value = serde_json::from_slice(&fs::read(config)?)?;
    match command {
        "api" => {
            let port = option("--port")
                .unwrap_or_else(|| "8081".into())
                .parse::<u16>()?;
            if port == 0 {
                return Err("Invalid port".into());
            }
            let api =
                Arc::new(PoolApi::new(config, "btc").map_err(|e| io::Error::other(e.message))?);
            let address = format!("127.0.0.1:{port}");
            println!("BDL Pool Rust API listening on {address}");
            http::serve(&address, move |request| {
                match api.dispatch(&request.target) {
                    Ok(data) => http::Response::json(200, data),
                    Err(error) => error.response(),
                }
            })?;
        }
        "presence" => presence::run(&config, args.iter().any(|a| a == "--once"))?,
        _ => {
            return Err(
                "usage: bdl-pool-services api|presence --config PATH [--port PORT] [--once]".into(),
            )
        }
    }
    Ok(())
}

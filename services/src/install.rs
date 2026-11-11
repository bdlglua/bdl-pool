//! Installation helpers. Shell performs privilege changes; all configuration logic is Rust.
use crate::{api::PoolApi, collect::run_bounded};
use serde_json::{json, Value};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
fn invalid(message: &str) -> io::Error {
    io::Error::other(message)
}
fn read(path: &Path) -> io::Result<Value> {
    serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)
}
fn host(value: &str) -> bool {
    regex::Regex::new(r"^[a-z0-9](?:[a-z0-9.-]{0,251}[a-z0-9])?$")
        .unwrap()
        .is_match(value)
}
fn atomic(path: &Path, value: &Value) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(value)?)?;
    #[cfg(unix)]
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
    fs::rename(temporary, path)
}
fn free_port(port: u16, output: &[u8]) -> io::Result<()> {
    if output.iter().all(u8::is_ascii_whitespace) {
        Ok(())
    } else {
        Err(invalid(&format!(
            "Port {port} is occupied by another service; installation stopped."
        )))
    }
}
pub fn ports() -> io::Result<()> {
    for (port, unit) in [
        (8081, "bdl-pool-api.service"),
        (80, "caddy.service"),
        (443, "caddy.service"),
    ] {
        if Command::new("systemctl")
            .args(["is-active", "--quiet", unit])
            .status()?
            .success()
        {
            continue;
        }
        let output = run_bounded(
            Command::new("ss").args(["-H", "-ltn", &format!("sport = :{port}")]),
            Duration::from_secs(5),
            16384,
        )?;
        free_port(port, &output)?;
    }
    Ok(())
}
fn configure_relay(settings: &mut Value, relay: &Value) -> io::Result<String> {
    let domain = relay["domain"]
        .as_str()
        .filter(|s| host(s))
        .ok_or_else(|| invalid("Invalid public OpenPort domain"))?;
    let api_host = relay["apiDomain"].as_str().unwrap_or(domain);
    let directory = relay["tlsDirectory"]
        .as_str()
        .unwrap_or("/etc/caddy/bdl-pool-tls");
    if relay["managedTLS"] != true
        || !host(api_host)
        || !matches!(
            directory,
            "/etc/caddy/bdl-pool-tls" | "/etc/caddy/bdl-pool-domain-tls"
        )
        || ["v1Port", "v2Port", "apiPublicPort"].iter().any(|k| {
            !relay[*k]
                .as_u64()
                .map(|p| (1..=65535).contains(&p))
                .unwrap_or(false)
        })
    {
        return Err(invalid(
            "Invalid OpenPort/TLS configuration; existing certificate retained",
        ));
    }
    let configured = settings["host"].as_str().unwrap_or_default();
    let previous = relay["previousDomain"].as_str().unwrap_or_default();
    let related = |candidate: &str| {
        configured == candidate || configured.strip_prefix("stratum.") == Some(candidate)
    };
    if !related(domain) && (previous.is_empty() || !host(previous) || !related(previous)) {
        return Err(invalid(
            "Unrelated OpenPort domain; existing configuration retained",
        ));
    }
    settings["host"] = domain.into();
    settings["apiHost"] = api_host.into();
    settings["v1Port"] = relay["v1Port"].clone();
    settings["v2Port"] = relay["v2Port"].clone();
    settings["apiPublicPort"] = relay["apiPublicPort"].clone();
    let tls =
        format!("    tls {directory}/current/fullchain.pem {directory}/current/privkey.pem\n");
    Ok(tls)
}
pub fn prepare(source: &Path, destination: &Path, caddy: &Path) -> io::Result<()> {
    let home = Path::new("/home/bdl");
    let root = source
        .parent()
        .ok_or_else(|| invalid("Invalid source directory"))?;
    let mut settings = if destination.exists() {
        read(destination)?
    } else {
        let public = read(&root.join("config.json"))?;
        let host = public["publicHost"]
            .as_str()
            .filter(|s| host(s) && s.contains('.'))
            .ok_or_else(|| invalid("Invalid public host"))?;
        let local = home.join("mining/ckpool/local.conf");
        let raw = fs::read(&local)?;
        let config = serde_json::Deserializer::from_slice(&raw)
            .into_iter::<Value>()
            .next()
            .ok_or_else(|| invalid("Invalid CKPool configuration"))??;
        let logs = config["logdir"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("logs");
        let logs = if Path::new(logs).is_absolute() {
            PathBuf::from(logs)
        } else {
            local.parent().unwrap().join(logs)
        };
        fs::create_dir_all(&logs)?;
        let logs = logs.canonicalize()?;
        if !logs.starts_with(home)
            || logs == home
            || logs
                .to_string_lossy()
                .contains(['\r', '\n', '"', '\\', ':', '%'])
        {
            return Err(invalid("Unexpected CKPool statistics directory"));
        }
        json!({"snapshot":home.join(".local/state/bdl-pool-monitor/repository/status.json"),"logs":logs,"sv2Public":home.join(".local/state/bdl-pool-monitor/sv2-public.json"),"onlineSnapshot":"/var/lib/bdl-pool-presence/online.json","ckpoolSocket":"/run/ckpool/stratifier","host":host,"v1Port":3333,"v2Port":3336})
    };
    let bch = home.join(".config/bdl-pool-bch/api.json");
    if bch.exists() {
        settings["coins"] = json!({"bch":read(&bch)?});
    }
    let relay_file = Path::new("/etc/bdl-pool-openport/public.json");
    let tls = if relay_file.exists() {
        configure_relay(&mut settings, &read(relay_file)?)?
    } else {
        String::new()
    };
    PoolApi::new(settings.clone(), "btc").map_err(|e| invalid(&e.message))?;
    let logs = settings["logs"]
        .as_str()
        .filter(|s| !s.contains(['\r', '\n', '"', '\\', ':', '%']))
        .ok_or_else(|| invalid("Invalid statistics directory"))?;
    let api_host = settings["apiHost"]
        .as_str()
        .or_else(|| settings["host"].as_str())
        .unwrap();
    atomic(destination, &settings)?;
    fs::write(caddy,format!("{api_host} {{\n{tls}    import /etc/caddy/bdlminer-routes/*.caddy\n    handle /api/v1/* {{\n        reverse_proxy 127.0.0.1:8081 {{\n            transport http {{\n                dial_timeout 3s\n                response_header_timeout 10s\n            }}\n        }}\n    }}\n    handle {{\n        redir https://bdl-pool.com/ 302\n    }}\n}}\n"))?;
    let binary = root.join("services/target/release/bdl-pool-services");
    let binary = binary
        .to_str()
        .filter(|s| !s.contains(['\r', '\n', ' ', '%']))
        .ok_or_else(|| invalid("Invalid Rust binary path"))?;
    let config = destination
        .to_str()
        .filter(|s| !s.contains(['\r', '\n', ' ', '%']))
        .ok_or_else(|| invalid("Invalid public config path"))?;
    for (name, command, presence) in [
        ("bdl-pool-api", "api", false),
        ("bdl-pool-presence", "presence", true),
    ] {
        let read_only = if presence {
            String::new()
        } else {
            format!("BindReadOnlyPaths=/home/bdl/.local/state/bdl-pool-monitor/repository\nBindReadOnlyPaths=-/home/bdl/.local/state/bdl-pool-monitor/sv2-public.json\nBindReadOnlyPaths=\"{logs}\"\nBindReadOnlyPaths=-/home/bdl/mining/bch/logs\n")
        };
        let unit=format!("[Unit]\nDescription=BDL Pool Rust {}\nAfter=network.target\n\n[Service]\nType=simple\nUser=bdl\nGroup=bdl\nWorkingDirectory=/\nExecStart={binary} {command} --config {config}\nRestart=on-failure\nRestartSec=5\n{}UMask=0077\nNoNewPrivileges=true\nPrivateTmp=true\nPrivateDevices=true\nProtectSystem=strict\nProtectHome=tmpfs\nBindReadOnlyPaths={binary}\nBindReadOnlyPaths=/home/bdl/.config/bdl-pool-api\n{read_only}RestrictAddressFamilies={}\nMemoryMax={}M\nTasksMax={}\nLimitNOFILE=256\n\n[Install]\nWantedBy=multi-user.target\n",if presence{"filtered connection collector"}else{"read-only API"},if presence{"StateDirectory=bdl-pool-presence\nStateDirectoryMode=0700\n"}else{""},if presence{"AF_UNIX"}else{"AF_INET AF_INET6 AF_UNIX"},if presence{128}else{192},if presence{16}else{48});
        fs::write(caddy.with_file_name(format!("{name}.service")), unit)?;
    }
    Ok(())
}
pub fn health() -> io::Result<()> {
    for _ in 0..10 {
        if let Ok(response) = ureq::get("http://127.0.0.1:8081/api/v1/health")
            .timeout(Duration::from_secs(2))
            .call()
        {
            if let Ok(value) = response.into_json::<Value>() {
                if value["status"] == "ok" && value["apiVersion"] == "1" {
                    println!("Local Rust API: ok.");
                    return Ok(());
                }
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err(invalid(
        "The local API did not become healthy; inspect bdl-pool-api journal.",
    ))
}
pub fn caddy_import(path: &Path) -> io::Result<()> {
    let text = fs::read_to_string(path)?;
    let line = "import /etc/caddy/conf.d/*.caddy";
    if !text.lines().any(|l| l == line) {
        fs::write(path, format!("{}\n\n{line}\n", text.trim_end()))?;
    }
    Ok(())
}
pub fn describe(path: &Path) -> io::Result<()> {
    let settings = read(path)?;
    let host = settings["apiHost"]
        .as_str()
        .or_else(|| settings["host"].as_str())
        .unwrap_or_default();
    let port = settings["apiPublicPort"].as_u64().unwrap_or(443);
    println!(
        "Rust API installed. Public health: https://{host}{}/api/v1/health",
        if port == 443 {
            String::new()
        } else {
            format!(":{port}")
        }
    );
    println!(
        "Presence collection runs every 15 seconds. Bitcoin Core and CKPool were not restarted."
    );
    Ok(())
}
pub fn dispatch(args: &[String]) -> io::Result<()> {
    match args.first().map(String::as_str){Some("ports")=>ports(),Some("prepare")if args.len()==4=>prepare(Path::new(&args[1]),Path::new(&args[2]),Path::new(&args[3])),Some("health")=>health(),Some("caddy-import")if args.len()==2=>caddy_import(Path::new(&args[1])),Some("describe")if args.len()==2=>describe(Path::new(&args[1])),_=>Err(invalid("usage: install ports|prepare SOURCE CONFIG CADDY|health|caddy-import PATH|describe CONFIG"))}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn relay() -> Value {
        json!({"domain":"bdl-pool.duckdns.org","managedTLS":true,"v1Port":39991,"v2Port":36809,"apiPublicPort":36018})
    }
    #[test]
    fn relay_migration_preserves_ports_and_selects_its_own_certificate() {
        let mut settings = json!({"host":"bdl-pool.duckdns.org"});
        let tls = configure_relay(&mut settings, &relay()).unwrap();
        assert!(tls.contains("/etc/caddy/bdl-pool-tls/current/"));
        let mut current = relay();
        current["domain"] = "bdl-pool.com".into();
        current["previousDomain"] = "bdl-pool.duckdns.org".into();
        current["apiDomain"] = "api.bdl-pool.com".into();
        current["tlsDirectory"] = "/etc/caddy/bdl-pool-domain-tls".into();
        let tls = configure_relay(&mut settings, &current).unwrap();
        assert_eq!(settings["host"], "bdl-pool.com");
        assert_eq!(settings["apiHost"], "api.bdl-pool.com");
        assert_eq!(settings["v1Port"], 39991);
        assert_eq!(settings["v2Port"], 36809);
        assert!(tls.contains("/etc/caddy/bdl-pool-domain-tls/current/"));
        let mut fresh = json!({"host":"stratum.bdl-pool.com"});
        assert!(configure_relay(&mut fresh, &current).is_ok());
    }
    #[test]
    fn unrelated_domains_and_unexpected_certificates_cannot_replace_settings() {
        for change in [
            json!({"domain":"unrelated.example"}),
            json!({"tlsDirectory":"/tmp/certs"}),
            json!({"v1Port":0}),
            json!({"v2Port":65536}),
            json!({"managedTLS":false}),
            json!({"apiDomain":"bad\nhost"}),
        ] {
            let mut relay = relay();
            for (k, v) in change.as_object().unwrap() {
                relay[k] = v.clone();
            }
            let mut settings = json!({"host":"bdl-pool.duckdns.org"});
            let before = settings.clone();
            assert!(configure_relay(&mut settings, &relay).is_err());
            assert_eq!(settings, before);
        }
    }
    #[test]
    fn listening_ports_block_installation_without_privileged_binds() {
        assert!(free_port(80, b"\n  ").is_ok());
        assert!(free_port(80, b"LISTEN 0 128 0.0.0.0:80").is_err());
    }
}

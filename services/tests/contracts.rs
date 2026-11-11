use bdl_pool_services::{
    api::{self, PoolApi},
    presence,
};
use serde_json::{json, Value};
use std::{fs, path::Path};
use tempfile::TempDir;
const ADDRESS: &str = "1BoatSLRHtKNngkdXEeobR76b53LETtpyT";
const CASH: &str = "bitcoincash:qpm2qsznhks23z7629mms6s4cwef74vcwvy22gdx6a";
const KEY: &str = "9c9DhUCzDsSZaCgKJitKKcngq24FLrqfJ9kUQzhxLiiSvAGuhna";
fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
fn config(root: &Path) -> Value {
    json!({"snapshot":root.join("status.json"),"logs":root.join("logs"),"sv2Public":root.join("key.json"),"onlineSnapshot":root.join("online.json"),"host":"stratum.example.com","apiHost":"api.example.com","v1Port":39991,"v2Port":36809,"apiPublicPort":36018})
}
fn snapshot() -> Value {
    let stamp = api::epoch(&json!(api::now()));
    json!({"schemaVersion":1,"source":"snapshot","updatedAt":stamp,"node":{"chain":"main","blocks":975013,"headers":975013,"verificationProgress":1,"initialBlockDownload":false,"updatedAt":stamp,"rpcpassword":"PRIVATE_TEST"},"pool":{"ready":true,"hashrateHps":1e12,"workers":2,"bestShare":1000,"blocksFound":null,"private":"PRIVATE_TEST"},"checks":{"bitcoin":"ok","poolStats":"ok","stratum":"job_received"},"history":[{"time":stamp,"hashrateHps":1e12,"ip":"PRIVATE_TEST"}],"secret":"PRIVATE_TEST"})
}
fn fixture(sample: Value) -> (TempDir, PoolApi) {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("logs/users")).unwrap();
    write(&root.path().join("status.json"), &sample);
    write(
        &root.path().join("key.json"),
        &json!({"authorityPublicKey":KEY,"secret":"PRIVATE_TEST"}),
    );
    write(
        &root.path().join("logs/users").join(ADDRESS),
        &json!({"hashrate1m":"1.5T","hashrate5m":"1.25T","hashrate1hr":"950G","hashrate1d":"12M","hashrate7d":"7K","shares":1234,"bestshare":123.5,"bestever":1000,"workers":2,"ip":"PRIVATE_TEST","worker":[{"workername":format!("{ADDRESS}.bitaxe-01"),"hashrate5m":"750G","shares":100,"password":"PRIVATE_TEST"},{"workername":format!("{ADDRESS}.bitaxe-02"),"hashrate5m":"500G","shares":200},{"workername":"1BitcoinEaterAddressDontSendf59kuE.foreign","hashrate5m":"9T"}]}),
    );
    let api = PoolApi::new(config(root.path()), "btc").unwrap();
    (root, api)
}
#[test]
fn api_contract_filters_private_fields_and_preserves_units() {
    let (_root, api) = fixture(snapshot());
    let pool = api.dispatch("/api/v1/pool").unwrap();
    assert_eq!(pool["pool"]["readyAtLastCheck"], true);
    assert_eq!(pool["pool"]["hashrateHps"], 1e12);
    assert_eq!(pool["pool"]["blocksFound"], Value::Null);
    assert!(!pool.to_string().contains("PRIVATE_TEST"));
    let miner = api.dispatch(&format!("/api/v1/miners/{ADDRESS}")).unwrap();
    assert_eq!(miner["hashrateHps"]["5m"], 1.25e12);
    assert_eq!(miner["workersRecorded"], 2);
    assert_eq!(miner["shares"], 1234);
    assert!(!miner.to_string().contains("PRIVATE_TEST"));
    let workers = api
        .dispatch(&format!(
            "/api/v1/miners/{ADDRESS}/workers?limit=1&offset=1"
        ))
        .unwrap();
    assert_eq!(workers["total"], 2);
    assert_eq!(
        workers["workers"][0]["name"],
        format!("{ADDRESS}.bitaxe-02")
    );
    assert_eq!(
        api.dispatch(&format!("/api/v1/miners/{ADDRESS}/workers/bitaxe-01"))
            .unwrap()["worker"]["shares"],
        100
    );
    let stratum = api.dispatch("/api/v1/stratum").unwrap();
    assert_eq!(stratum["v2"]["authorityPublicKey"], KEY);
    assert_eq!(stratum["v1"]["port"], 39991);
    assert_eq!(stratum["v2"]["availability"], "not_verified");
}
#[test]
fn stale_or_incomplete_measurements_never_confirm_readiness() {
    for scenario in [
        "old_snapshot",
        "old_node",
        "retained",
        "no_job",
        "unknown_node",
    ] {
        let mut data = snapshot();
        match scenario {
            "old_snapshot" => data["updatedAt"] = api::epoch(&json!(api::now() - 1000.0)),
            "old_node" => data["node"]["updatedAt"] = api::epoch(&json!(api::now() - 1000.0)),
            "retained" => data["checks"]["bitcoin"] = "stale".into(),
            "no_job" => data["checks"]["stratum"] = "subscribed_no_job".into(),
            _ => data["node"]["blocks"] = Value::Null,
        }
        let (_root, api) = fixture(data);
        assert_eq!(
            api.pool().unwrap()["pool"]["readyAtLastCheck"],
            false,
            "{scenario}"
        );
    }
}
#[test]
fn invalid_routes_queries_and_missing_sources_remain_explicit() {
    let (root, api) = fixture(snapshot());
    for route in [
        "/api/v1/miners/%2e%2e",
        "/api/v1/miners/%2e%2e%2fprivate",
        "/api/v1/health?callback=alert",
        "/api/v1/miners/not-a-bitcoin-address",
    ] {
        assert!(matches!(api.dispatch(route).unwrap_err().status, 400 | 404));
    }
    for query in [
        "limit=0",
        "limit=201",
        "offset=-1",
        "limit=1&limit=2",
        "unknown=1",
        "limit=",
    ] {
        assert_eq!(
            api.dispatch(&format!("/api/v1/miners/{ADDRESS}/workers?{query}"))
                .unwrap_err()
                .status,
            400
        );
    }
    fs::remove_file(root.path().join("status.json")).unwrap();
    assert_eq!(api.dispatch("/api/v1/pool").unwrap_err().status, 503);
    assert_eq!(api.dispatch("/api/v1/health").unwrap()["status"], "ok");
    assert_eq!(api.dispatch("/api/v1/miners").unwrap_err().status, 404);
    assert_eq!(
        api.dispatch("/api/v1/coins/bch/pool").unwrap_err().status,
        503
    );
}
#[cfg(unix)]
#[test]
fn refuses_symlinks_devices_partial_json_and_oversized_statistics() {
    use std::os::unix::fs::symlink;
    for case in [
        "symlink",
        "oversize",
        "partial",
        "nan",
        "array",
        "directory",
    ] {
        let (root, api) = fixture(snapshot());
        let user = root.path().join("logs/users").join(ADDRESS);
        fs::remove_file(&user).unwrap();
        match case {
            "symlink" => {
                let private = root.path().join("private");
                fs::write(&private, b"{\"private\":1}").unwrap();
                symlink(private, &user).unwrap();
            }
            "oversize" => fs::write(&user, vec![b' '; api::MAX_FILE as usize + 1]).unwrap(),
            "partial" => fs::write(&user, b"{\"shares\":").unwrap(),
            "nan" => fs::write(&user, b"{\"shares\":NaN}").unwrap(),
            "array" => fs::write(&user, b"[]").unwrap(),
            _ => fs::create_dir(&user).unwrap(),
        }
        assert_eq!(
            api.dispatch(&format!("/api/v1/miners/{ADDRESS}"))
                .unwrap_err()
                .status,
            503,
            "{case}"
        );
    }
}
#[test]
fn bch_uses_only_its_own_sources_and_checksummed_addresses() {
    let (root, _) = fixture(snapshot());
    let bch = root.path().join("bch");
    fs::create_dir_all(bch.join("logs/users")).unwrap();
    let mut sample = snapshot();
    sample["coin"] = "bch".into();
    write(&bch.join("status.json"), &sample);
    write(
        &bch.join("logs/users").join(CASH),
        &json!({"shares":987,"worker":[{"workername":format!("{}.bitaxe",CASH.split(':').next_back().unwrap().to_uppercase()),"shares":12},{"workername":format!("{ADDRESS}.wrong-network"),"shares":555}]}),
    );
    let mut conf = config(root.path());
    conf["coins"] = json!({"bch":config(&bch)});
    let api = PoolApi::new(conf, "btc").unwrap();
    assert_eq!(
        api.dispatch(&format!("/api/v1/coins/bch/miners/{CASH}"))
            .unwrap()["shares"],
        987
    );
    let workers = api
        .dispatch(&format!("/api/v1/coins/bch/miners/{CASH}/workers"))
        .unwrap();
    assert_eq!(workers["total"], 1);
    assert_eq!(workers["workers"][0]["name"], format!("{CASH}.bitaxe"));
    assert_eq!(workers["coin"], "bch");
    assert_eq!(
        api.dispatch("/api/v1/coins/bch/stratum").unwrap()["v2"]["configured"],
        false
    );
    assert_eq!(
        api.dispatch(&format!("/api/v1/miners/{CASH}"))
            .unwrap_err()
            .status,
        400
    );
    assert_eq!(
        api.dispatch(&format!("/api/v1/coins/bch/miners/{ADDRESS}"))
            .unwrap_err()
            .status,
        400
    );
}
#[test]
fn presence_requires_fresh_authorized_connections_and_masks_public_addresses() {
    let (root, api) = fixture(snapshot());
    let client = json!({"id":123,"workername":format!("{ADDRESS}.bitaxe-01"),"authorised":true,"useragent":"BDLMinerV2/2.5.4 M5StickC Plus2","dsps5":2,"bestdiff":4321,"starttime":api::now()-10000.0,"lastshare":api::now()-5000.0,"ip":"PRIVATE_TEST","password":"PRIVATE_TEST"});
    let clients = presence::filter_clients(&[client], b"test-salt", "btc");
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0]["hashrateHps"]["5m"], 2.0 * 4294967296.0);
    assert!(!json!(clients).to_string().contains("PRIVATE_TEST"));
    write(
        &root.path().join("online.json"),
        &json!({"schemaVersion":1,"available":true,"updatedAt":api::epoch(&json!(api::now())),"clients":clients}),
    );
    let online = api.dispatch("/api/v1/online").unwrap();
    assert_eq!(online["total"], 1);
    assert!(!online.to_string().contains(ADDRESS));
    assert_eq!(online["miners"][0]["device"]["model"], "M5StickC Plus2");
    let worker = api
        .dispatch(&format!("/api/v1/miners/{ADDRESS}/workers/bitaxe-01"))
        .unwrap();
    assert_eq!(worker["worker"]["online"], true);
}
#[test]
fn user_agent_recognition_does_not_invent_hardware() {
    for (agent, model, software) in [
        ("BDLMinerV2/2.5.4", None, Some("BDLMiner V2")),
        ("NerdMinerV2", None, Some("NerdMiner V2")),
        (
            "ESP-Miner/2.7.0 Bitaxe Gamma",
            Some("Bitaxe Gamma"),
            Some("ESP-Miner"),
        ),
        (
            "cgminer/4.11.1 Antminer S19 Pro",
            Some("Antminer S19 PRO"),
            Some("cgminer"),
        ),
        ("<img src=x onerror=alert(1)> PRIVATE_TEST", None, None),
    ] {
        let device = api::device_from_agent(&json!(agent));
        assert_eq!(device["model"].as_str(), model);
        assert_eq!(device["software"].as_str(), software);
        assert!(!device.to_string().contains("PRIVATE_TEST"));
    }
}

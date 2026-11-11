use bdl_pool_services::{
    api::{self, PoolApi},
    collect, http, presence,
};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};
use tempfile::TempDir;
fn request(port: u16, data: &[u8]) -> (String, Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(data).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let index = response.windows(4).position(|b| b == b"\r\n\r\n").unwrap();
    (
        String::from_utf8(response[..index].to_vec()).unwrap(),
        response[index + 4..].to_vec(),
    )
}
#[test]
fn real_http_preserves_head_etag_cors_and_read_only_errors() {
    let root = TempDir::new().unwrap();
    let config = json!({"snapshot":root.path().join("missing"),"logs":root.path(),"sv2Public":root.path().join("key"),"host":"example.com"});
    let api = PoolApi::new(config, "btc").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        http::serve_listener(listener, move |request| {
            match api.dispatch(&request.target) {
                Ok(v) => http::Response::json(200, v),
                Err(e) => e.response(),
            }
        })
        .unwrap();
    });
    let (head, body) = request(
        port,
        b"GET /api/v1/health HTTP/1.1\r\nHost: localhost\r\n\r\n",
    );
    assert!(head.starts_with("HTTP/1.1 200"));
    assert!(head.contains("Access-Control-Allow-Origin: *"));
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["status"],
        "ok"
    );
    let etag = head
        .lines()
        .find_map(|line| line.strip_prefix("ETag: "))
        .unwrap()
        .trim();
    let (cached, body) = request(
        port,
        format!("GET /api/v1/health HTTP/1.1\r\nHost: localhost\r\nIf-None-Match: {etag}\r\n\r\n")
            .as_bytes(),
    );
    assert!(cached.starts_with("HTTP/1.1 304"));
    assert!(cached.contains(etag));
    assert!(body.is_empty());
    for (method, status) in [
        ("HEAD", 200),
        ("OPTIONS", 204),
        ("POST", 405),
        ("PUT", 405),
        ("PATCH", 405),
        ("DELETE", 405),
        ("TRACE", 405),
    ] {
        let (head, body) = request(
            port,
            format!("{method} /api/v1/health HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes(),
        );
        assert!(head.starts_with(&format!("HTTP/1.1 {status}")));
        if matches!(method, "HEAD" | "OPTIONS") {
            assert!(body.is_empty());
        } else {
            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap()["error"]["code"],
                "method_not_allowed"
            );
        }
    }
    for (data, status) in [
        (b"GET /api/v1/PRIVATE_TEST HTTP/2.0\r\n\r\n".to_vec(), 505),
        (b"POST /api/v1/PRIVATE_TEST\r\n\r\n".to_vec(), 400),
        (
            format!(
                "GET /{} HTTP/1.1\r\nHost: localhost\r\n\r\n",
                "x".repeat(70000)
            )
            .into_bytes(),
            414,
        ),
    ] {
        let (head, body) = request(port, &data);
        assert!(head.starts_with(&format!("HTTP/1.1 {status}")));
        assert!(!head.contains("PRIVATE_TEST"));
        assert!(!String::from_utf8_lossy(&body).contains("PRIVATE_TEST"));
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap()["error"]["code"],
            "invalid_request"
        );
    }
}
#[test]
fn passive_stratum_check_requires_both_a_subscription_and_a_job() {
    for (job, expected) in [(true, "job_received"), (false, "subscribed_no_job")] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = [0; 1024];
            let size = stream.read(&mut bytes).unwrap();
            let subscription: Value = serde_json::from_slice(&bytes[..size]).unwrap();
            assert_eq!(subscription["method"], "mining.subscribe");
            assert!(!String::from_utf8_lossy(&bytes[..size]).contains("mining.authorize"));
            stream
                .write_all(b"{\"id\":1,\"result\":[[],\"aa\",4],\"error\":null}\n")
                .unwrap();
            if job {
                let message = json!({"method":"mining.notify","params":["job","a".repeat(64),"","",[],"20000000","1d00ffff","12345678",true]});
                stream
                    .write_all((message.to_string() + "\n").as_bytes())
                    .unwrap();
            }
        });
        assert_eq!(
            collect::stratum_check("127.0.0.1", port, Duration::from_secs(2)),
            expected
        );
        worker.join().unwrap();
    }
}
#[cfg(unix)]
#[test]
fn ckpool_ipc_reads_fragmented_frames_and_publishes_only_filtered_data() {
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};
    let root = TempDir::new().unwrap();
    let socket = root.path().join("stratifier");
    let listener = UnixListener::bind(&socket).unwrap();
    let client = json!({"id":123,"authorised":true,"workername":"1BoatSLRHtKNngkdXEeobR76b53LETtpyT.bitaxe","useragent":"BDLMinerV2/2.5.4 M5StickC Plus2","dsps5":2,"starttime":api::now()-100.0,"ip":"PRIVATE_TEST"});
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut size = [0; 4];
        stream.read_exact(&mut size).unwrap();
        assert_eq!(u32::from_le_bytes(size), 7);
        let mut command = [0; 7];
        stream.read_exact(&mut command).unwrap();
        assert_eq!(&command, b"clients");
        let encoded = serde_json::to_vec(&json!({"clients":[client]})).unwrap();
        stream
            .write_all(&(encoded.len() as u32).to_le_bytes())
            .unwrap();
        stream.write_all(&encoded[..15]).unwrap();
        stream.write_all(&encoded[15..]).unwrap();
    });
    let clients = presence::clients_from_socket(&socket).unwrap();
    worker.join().unwrap();
    let filtered = presence::filter_clients(&clients, b"test-salt", "btc");
    assert_eq!(filtered[0]["hashrateHps"]["5m"], 2.0 * 4294967296.0);
    let output = root.path().join("online.json");
    presence::publish(&output, &json!({"clients":filtered})).unwrap();
    assert_eq!(
        std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(!std::fs::read_to_string(output)
        .unwrap()
        .contains("PRIVATE_TEST"));
}
#[test]
fn collector_retains_original_measurement_time_and_filters_bad_history() {
    let now = api::now();
    let stamp = api::epoch(&json!(now - 1000.0));
    let node = json!({"chain":"main","blocks":10,"headers":20,"verificationProgress":0.5,"initialBlockDownload":true,"updatedAt":stamp,"rpcpassword":"PRIVATE_TEST"});
    let previous = json!({"source":"snapshot","updatedAt":api::epoch(&json!(now)),"checks":{"bitcoin":"stale"},"node":node,"history":[{"time":stamp,"hashrateHps":1e12,"ip":"PRIVATE_TEST"},{"time":"broken","hashrateHps":1},null,{"time":stamp,"hashrateHps":true}]});
    let retained = collect::node_from_snapshot(&previous, now).unwrap();
    assert_eq!(retained["updatedAt"], stamp);
    assert!(!retained.to_string().contains("PRIVATE_TEST"));
    let history = collect::history(&previous, now);
    assert_eq!(history.len(), 1);
    assert!(!json!(history).to_string().contains("PRIVATE_TEST"));
}

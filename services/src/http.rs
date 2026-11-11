//! Bounded HTTP/1 server. Request paths, client IPs and payloads are never logged.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
}
pub struct Response {
    pub status: u16,
    pub data: Value,
    pub headers: Vec<(String, String)>,
}
impl Response {
    pub fn json(status: u16, data: Value) -> Self {
        Self {
            status,
            data,
            headers: Vec::new(),
        }
    }
    pub fn error(status: u16, code: &str, message: &str) -> Self {
        Self::json(status, json!({"error":{"code":code,"message":message}}))
    }
}
impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}
fn read(stream: &mut TcpStream) -> Result<Request, u16> {
    let mut raw = Vec::new();
    let mut buf = [0_u8; 1024];
    let mut end = None;
    while end.is_none() {
        let n = stream.read(&mut buf).map_err(|_| 400_u16)?;
        if n == 0 {
            return Err(400);
        }
        raw.extend_from_slice(&buf[..n]);
        end = raw.windows(4).position(|w| w == b"\r\n\r\n");
        if raw.len() > 16384 {
            let first = raw.windows(2).position(|w| w == b"\r\n");
            if first.map(|n| n > 2048).unwrap_or(true) {
                if end.is_some() || raw.len() > 100_000 {
                    return Err(414);
                }
            } else {
                return Err(431);
            }
        }
    }
    let text = std::str::from_utf8(&raw[..end.unwrap()]).map_err(|_| 400_u16)?;
    let mut lines = text.split("\r\n");
    let line = lines.next().ok_or(400_u16)?;
    let parts: Vec<_> = line.split_whitespace().collect();
    if parts.len() != 3 {
        return Err(400);
    }
    if parts[2].starts_with("HTTP/2.") {
        return Err(505);
    }
    if !matches!(parts[2], "HTTP/1.0" | "HTTP/1.1") {
        return Err(400);
    }
    if parts[1].len() > 2048 {
        return Err(414);
    }
    let mut headers = Vec::new();
    for line in lines {
        let (key, value) = line.split_once(':').ok_or(400_u16)?;
        if key.is_empty()
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
            || value.contains(['\0', '\n', '\r'])
        {
            return Err(400);
        }
        if headers.len() >= 100 {
            return Err(431);
        }
        headers.push((key.to_ascii_lowercase(), value.trim().into()));
    }
    Ok(Request {
        method: parts[0].into(),
        target: parts[1].into(),
        headers,
    })
}
fn send(stream: &mut TcpStream, request: Option<&Request>, response: Response) -> io::Result<()> {
    let body = serde_json::to_vec(&response.data)?;
    let etag = format!("\"{:x}\"", Sha256::digest(&body));
    let cached = response.status == 200
        && request.and_then(|r| r.header("if-none-match")) == Some(etag.as_str());
    let status = if cached { 304 } else { response.status };
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        304 => "Not Modified",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        414 => "URI Too Long",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    };
    write!(stream,"HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, HEAD, OPTIONS\r\nAccess-Control-Allow-Headers: If-None-Match\r\nAccess-Control-Expose-Headers: ETag\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\nCache-Control: {}\r\n",if response.status==200{"public, max-age=5, must-revalidate"}else{"no-store"})?;
    if status == 405 {
        write!(stream, "Allow: GET, HEAD, OPTIONS\r\n")?;
    }
    if status == 503 {
        write!(stream, "Retry-After: 5\r\n")?;
    }
    if status != 204 {
        write!(stream, "ETag: {etag}\r\n")?;
        if !cached {
            write!(stream, "Content-Length: {}\r\n", body.len())?;
        }
    }
    for (key, value) in response.headers {
        if !key.contains(['\r', '\n']) && !value.contains(['\r', '\n']) {
            write!(stream, "{key}: {value}\r\n")?;
        }
    }
    write!(stream, "\r\n")?;
    if !cached && status != 204 && request.map(|r| r.method != "HEAD").unwrap_or(true) {
        stream.write_all(&body)?;
    }
    stream.flush()
}
pub fn serve(
    address: &str,
    handler: impl Fn(&Request) -> Response + Send + Sync + 'static,
) -> io::Result<()> {
    serve_listener(TcpListener::bind(address)?, handler)
}
pub fn serve_listener(
    listener: TcpListener,
    handler: impl Fn(&Request) -> Response + Send + Sync + 'static,
) -> io::Result<()> {
    let handler = Arc::new(handler);
    let (sender, receiver) = mpsc::sync_channel::<TcpStream>(32);
    let receiver = Arc::new(Mutex::new(receiver));
    for _ in 0..16 {
        let receiver = receiver.clone();
        let handler = handler.clone();
        thread::spawn(move || loop {
            let stream = receiver.lock().unwrap().recv();
            let Ok(mut stream) = stream else {
                break;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
            match read(&mut stream) {
                Ok(request) => {
                    let response = if request.method == "OPTIONS" {
                        Response::json(204, json!({}))
                    } else if !matches!(request.method.as_str(), "GET" | "HEAD") {
                        Response::error(405, "method_not_allowed", "This API is read-only.")
                    } else {
                        handler(&request)
                    };
                    let _ = send(&mut stream, Some(&request), response);
                }
                Err(status) => {
                    let response =
                        Response::error(status, "invalid_request", "Invalid HTTP request.");
                    let _ = send(&mut stream, None, response);
                }
            }
        });
    }
    for stream in listener.incoming() {
        let stream = stream?;
        let _ = sender.try_send(stream);
    }
    Ok(())
}

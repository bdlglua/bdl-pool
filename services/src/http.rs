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

const MAX_HEADER_BYTES: usize = 16_384;
const MAX_BODY_BYTES: usize = 32_768;

pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
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
        if end.is_none() && raw.len() > MAX_HEADER_BYTES {
            return Err(431);
        }
    }
    let header_end = end.unwrap();
    if header_end > MAX_HEADER_BYTES {
        return Err(431);
    }
    let text = std::str::from_utf8(&raw[..header_end]).map_err(|_| 400_u16)?;
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
    let mut content_length = None;
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
        let key = key.to_ascii_lowercase();
        let value = value.trim().to_string();
        if key == "transfer-encoding" {
            // Chunked framing is intentionally unsupported to avoid request-smuggling ambiguity.
            return Err(400);
        }
        if key == "content-length" {
            if content_length.is_some() {
                return Err(400);
            }
            let parsed = value.parse::<usize>().map_err(|_| 400_u16)?;
            if parsed > MAX_BODY_BYTES {
                return Err(413);
            }
            content_length = Some(parsed);
        }
        headers.push((key, value));
    }
    let expected = content_length.unwrap_or(0);
    let body_start = header_end + 4;
    let initial = raw.len().saturating_sub(body_start);
    if initial > expected {
        return Err(400);
    }
    let mut body = raw[body_start..].to_vec();
    while body.len() < expected {
        let remaining = expected - body.len();
        let take = remaining.min(buf.len());
        let n = stream.read(&mut buf[..take]).map_err(|_| 400_u16)?;
        if n == 0 {
            return Err(400);
        }
        body.extend_from_slice(&buf[..n]);
    }
    Ok(Request {
        method: parts[0].into(),
        target: parts[1].into(),
        headers,
        body,
    })
}
fn send(
    stream: &mut TcpStream,
    request: Option<&Request>,
    response: Response,
    allow_post: bool,
) -> io::Result<()> {
    let body = serde_json::to_vec(&response.data)?;
    let etag = format!("\\"{:x}\\"", Sha256::digest(&body));
    let cached = response.status == 200
        && request.and_then(|r| r.header("if-none-match")) == Some(etag.as_str());
    let status = if cached { 304 } else { response.status };
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Content Too Large",
        414 => "URI Too Long",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        507 => "Insufficient Storage",
        _ => "Error",
    };
    let methods = if allow_post {
        "GET, HEAD, POST, OPTIONS"
    } else {
        "GET, HEAD, OPTIONS"
    };
    let allowed_headers = if allow_post {
        "If-None-Match, Authorization, Content-Type"
    } else {
        "If-None-Match"
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: {methods}\r\nAccess-Control-Allow-Headers: {allowed_headers}\r\nAccess-Control-Expose-Headers: ETag\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\nCache-Control: {}\r\n",
        if response.status == 200 { "no-store" } else { "no-store" }
    )?;
    if status == 405 {
        write!(
            stream,
            "Allow: {}\r\n",
            if allow_post { "GET, HEAD, POST, OPTIONS" } else { "GET, HEAD, OPTIONS" }
        )?;
    }
    if status == 401 {
        write!(stream, "WWW-Authenticate: Bearer\r\n")?;
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
pub fn serve_private(
    address: &str,
    handler: impl Fn(&Request) -> Response + Send + Sync + 'static,
) -> io::Result<()> {
    serve_listener_mode(TcpListener::bind(address)?, handler, true)
}
pub fn serve_listener(
    listener: TcpListener,
    handler: impl Fn(&Request) -> Response + Send + Sync + 'static,
) -> io::Result<()> {
    serve_listener_mode(listener, handler, false)
}
fn serve_listener_mode(
    listener: TcpListener,
    handler: impl Fn(&Request) -> Response + Send + Sync + 'static,
    allow_post: bool,
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
                    } else if !matches!(request.method.as_str(), "GET" | "HEAD")
                        && !(allow_post && request.method == "POST")
                    {
                        Response::error(405, "method_not_allowed", "This API does not allow this method.")
                    } else {
                        handler(&request)
                    };
                    let _ = send(&mut stream, Some(&request), response, allow_post);
                }
                Err(status) => {
                    let response =
                        Response::error(status, "invalid_request", "Invalid HTTP request.");
                    let _ = send(&mut stream, None, response, allow_post);
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

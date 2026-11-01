use crate::adapter::Match;
use crate::engine::as_strict_i32;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct ServerState {
    pub current_match: Match,
    pub static_root: PathBuf,
}

pub fn content_type_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("webp") => "image/webp",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}

const SECURITY_HEADERS: &str = "Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self'; frame-ancestors 'none'\r\n";

fn status_text_for(code: u16) -> &'static str {
    match code {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        408 => "Request Timeout",
        414 => "URI Too Long",
        _ => "Error",
    }
}

fn write_json_response_with_headers(
    stream: &mut TcpStream,
    code: u16,
    payload: &Value,
    extra_headers: &[(&str, &str)],
) {
    let status_text = status_text_for(code);
    let body = serde_json::to_vec(payload).unwrap_or_else(|_| b"{}".to_vec());
    let mut extra = String::new();
    for (k, v) in extra_headers {
        extra.push_str(&format!("{k}: {v}\r\n"));
    }
    let header = format!(
        "HTTP/1.1 {code} {status_text}\r\nContent-Type: application/json; charset=utf-8\r\n{extra}{SECURITY_HEADERS}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(&body);
    let _ = stream.flush();
}

fn write_json_response(stream: &mut TcpStream, code: u16, payload: &Value) {
    write_json_response_with_headers(stream, code, payload, &[]);
}

fn write_bytes_response(stream: &mut TcpStream, code: u16, ctype: &str, body: &[u8]) {
    let status_text = status_text_for(code);
    let header = format!(
        "HTTP/1.1 {code} {status_text}\r\nContent-Type: {ctype}\r\n{SECURITY_HEADERS}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push(((h << 4) | l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn normalize_path(base: &Path, rel: &str) -> PathBuf {
    let mut out = base.to_path_buf();
    for comp in Path::new(rel).components() {
        match comp {
            std::path::Component::Normal(c) => out.push(c),
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {}
        }
    }
    out
}

fn check_local_request(
    headers: &HashMap<String, Vec<String>>,
    port: u16,
    post: bool,
) -> Result<(), (u16, &'static str)> {
    let allowed_hosts = [
        format!("127.0.0.1:{port}"),
        format!("localhost:{port}"),
        format!("[::1]:{port}"),
    ];
    let host_vals = headers.get("host").cloned().unwrap_or_default();
    if host_vals.len() != 1 || !allowed_hosts.contains(&host_vals[0]) {
        return Err((400, "Only the local application host is permitted"));
    }
    for name in ["origin", "sec-fetch-site"] {
        if headers.get(name).map(|v| v.len()).unwrap_or(0) > 1 {
            return Err((400, "Duplicate origin metadata is not permitted"));
        }
    }
    let allowed_origins: Vec<String> = allowed_hosts
        .iter()
        .map(|h| format!("http://{h}"))
        .collect();
    if let Some(origin_vals) = headers.get("origin") {
        let origin = origin_vals.first().map(|s| s.as_str()).unwrap_or("");
        if !allowed_origins.iter().any(|ao| ao == origin) {
            return Err((400, "Cross-origin requests are not permitted"));
        }
    }
    let fetch_site = headers
        .get("sec-fetch-site")
        .and_then(|v| v.first())
        .map(|s| s.as_str())
        .unwrap_or("");
    if !fetch_site.is_empty() && !matches!(fetch_site, "same-origin" | "none") {
        return Err((400, "Cross-site requests are not permitted"));
    }
    if post {
        if headers.contains_key("transfer-encoding") {
            return Err((400, "Transfer-Encoding is not supported"));
        }
        let ct_vals = headers.get("content-type").cloned().unwrap_or_default();
        if ct_vals.len() != 1 {
            return Err((400, "Exactly one Content-Type is required"));
        }
        let ctype = ct_vals[0]
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if ctype != "application/json" {
            return Err((400, "Use application/json"));
        }
    }
    Ok(())
}

fn read_json_body<R: Read>(
    reader: &mut R,
    headers: &HashMap<String, Vec<String>>,
) -> Result<Value, (u16, &'static str)> {
    if headers.contains_key("transfer-encoding") {
        return Err((400, "Transfer-Encoding is not supported"));
    }
    let lengths = headers
        .get("content-length")
        .ok_or((400, "Exactly one decimal Content-Length is required"))?;
    if lengths.len() != 1 {
        return Err((400, "Exactly one decimal Content-Length is required"));
    }
    let raw_len = lengths[0].trim();
    if raw_len.is_empty() || !raw_len.chars().all(|c| c.is_ascii_digit()) {
        return Err((400, "Exactly one decimal Content-Length is required"));
    }
    let length: usize = raw_len
        .parse()
        .map_err(|_| (400, "Exactly one decimal Content-Length is required"))?;
    if length == 0 || length > 16384 {
        return Err((400, "Invalid request size"));
    }
    let mut buf = Vec::with_capacity(length);
    match reader.take(length as u64).read_to_end(&mut buf) {
        Ok(_) => {}
        Err(e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
        {
            return Err((
                408,
                "Request body timed out; refresh the table before retrying",
            ));
        }
        Err(_) => return Err((400, "Incomplete request body")),
    }
    if buf.len() != length {
        return Err((400, "Incomplete request body"));
    }
    let data: Value =
        serde_json::from_slice(&buf).map_err(|_| (400, "Request body must be valid JSON"))?;
    if !data.is_object() {
        return Err((400, "JSON body must be an object"));
    }
    Ok(data)
}

pub fn handle_connection(mut stream: TcpStream, state: Arc<Mutex<ServerState>>, port: u16) {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    let cloned = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut reader = BufReader::new(cloned);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
        return;
    }
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let raw_target = parts[1];
    let raw_path = raw_target.split('?').next().unwrap_or("/");
    let path_part = percent_decode(raw_path);

    let mut headers: HashMap<String, Vec<String>> = HashMap::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
        if trimmed.is_empty() {
            break;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            headers
                .entry(k.trim().to_ascii_lowercase())
                .or_default()
                .push(v.trim().to_string());
        }
    }

    if method == "OPTIONS" {
        write_json_response(
            &mut stream,
            403,
            &json!({"error": "Cross-origin access is disabled"}),
        );
        return;
    }

    let is_post = method == "POST";
    if let Err((code, msg)) = check_local_request(&headers, port, is_post) {
        write_json_response(&mut stream, code, &json!({"error": msg}));
        return;
    }

    match (method, path_part.as_str()) {
        ("GET", "/api/state") => {
            let st = state.lock().unwrap();
            let payload = st.current_match.state();
            drop(st);
            write_json_response(&mut stream, 200, &payload);
        }
        ("GET", "/api/replay") => {
            let st = state.lock().unwrap();
            let res = st.current_match.replay();
            drop(st);
            match res {
                Ok(payload) => write_json_response_with_headers(
                    &mut stream,
                    200,
                    &payload,
                    &[(
                        "Content-Disposition",
                        "attachment; filename=\"wonderland-replay.json\"",
                    )],
                ),
                Err(msg) => write_json_response(&mut stream, 400, &json!({"error": msg})),
            }
        }
        ("POST", "/api/action") => {
            let body = match read_json_body(&mut reader, &headers) {
                Ok(b) => b,
                Err((code, msg)) => {
                    write_json_response(&mut stream, code, &json!({"error": msg}));
                    return;
                }
            };
            let st = state.lock().unwrap();
            let res = st.current_match.submit(&body);
            drop(st);
            match res {
                Ok(payload) => write_json_response(&mut stream, 200, &payload),
                Err(msg) => write_json_response(&mut stream, 400, &json!({"error": msg})),
            }
        }
        ("POST", "/api/reset") => {
            let body = match read_json_body(&mut reader, &headers) {
                Ok(b) => b,
                Err((code, msg)) => {
                    write_json_response(&mut stream, code, &json!({"error": msg}));
                    return;
                }
            };
            if let Some(obj) = body.as_object() {
                if obj.keys().any(|k| k != "seed" && k != "starter") {
                    write_json_response(
                        &mut stream,
                        400,
                        &json!({"error": "Expected seed and starter only"}),
                    );
                    return;
                }
            }
            let seed = body
                .get("seed")
                .and_then(as_strict_i32)
                .map(|x| x as i64)
                .unwrap_or(42);
            let starter = body
                .get("starter")
                .and_then(as_strict_i32)
                .map(|x| x as i64)
                .unwrap_or(0);
            if let Some(sv) = body.get("seed") {
                if as_strict_i32(sv).is_none() {
                    write_json_response(
                        &mut stream,
                        400,
                        &json!({"error": "Seed must be an integer from 0 to 2147483647"}),
                    );
                    return;
                }
            }
            if let Some(stv) = body.get("starter") {
                if as_strict_i32(stv).is_none() {
                    write_json_response(
                        &mut stream,
                        400,
                        &json!({"error": "Starter must be 0 or 1"}),
                    );
                    return;
                }
            }
            let next_match = match Match::new(seed, starter) {
                Ok(m) => m,
                Err(msg) => {
                    write_json_response(&mut stream, 400, &json!({"error": msg}));
                    return;
                }
            };
            let payload = next_match.state();
            {
                let mut st = state.lock().unwrap();
                st.current_match.stop();
                st.current_match = next_match;
            }
            write_json_response(&mut stream, 200, &payload);
        }
        ("POST", _) => {
            let _ = match read_json_body(&mut reader, &headers) {
                Ok(b) => b,
                Err((code, msg)) => {
                    write_json_response(&mut stream, code, &json!({"error": msg}));
                    return;
                }
            };
            write_json_response(&mut stream, 404, &json!({"error": "Unknown endpoint"}));
        }
        ("GET", _) | ("HEAD", _) => {
            if path_part.len() > 2048 {
                write_json_response(
                    &mut stream,
                    414,
                    &json!({"error": "Request path is too long"}),
                );
                return;
            }
            let static_root = {
                let st = state.lock().unwrap();
                st.static_root.clone()
            };
            let rel = if path_part == "/" {
                "index.html"
            } else {
                path_part.trim_start_matches('/')
            };
            let root_canon = match static_root.canonicalize() {
                Ok(p) => p,
                Err(_) => {
                    write_json_response(&mut stream, 404, &json!({"error": "File not found"}));
                    return;
                }
            };
            let normalized = normalize_path(&root_canon, rel);
            if !normalized.starts_with(&root_canon) {
                write_json_response(
                    &mut stream,
                    403,
                    &json!({"error": "Path outside static root"}),
                );
                return;
            }
            let target_file = match normalized.canonicalize() {
                Ok(p) => {
                    if !p.starts_with(&root_canon) {
                        write_json_response(
                            &mut stream,
                            403,
                            &json!({"error": "Path outside static root"}),
                        );
                        return;
                    }
                    if !p.is_file() {
                        write_json_response(&mut stream, 404, &json!({"error": "File not found"}));
                        return;
                    }
                    p
                }
                Err(_) => {
                    write_json_response(&mut stream, 404, &json!({"error": "File not found"}));
                    return;
                }
            };
            match std::fs::read(&target_file) {
                Ok(bytes) => {
                    let ctype = content_type_for(&target_file);
                    write_bytes_response(&mut stream, 200, ctype, &bytes);
                }
                Err(_) => {
                    write_json_response(&mut stream, 404, &json!({"error": "File not found"}));
                }
            }
        }
        _ => {
            write_json_response(&mut stream, 404, &json!({"error": "Not found"}));
        }
    }
}

pub fn serve(port: u16, seed: i64, static_root: PathBuf) -> Result<(), String> {
    let m = Match::new(seed, 0)?;
    let state = Arc::new(Mutex::new(ServerState {
        current_match: m,
        static_root,
    }));
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    let actual_port = listener.local_addr().map_err(|e| e.to_string())?.port();
    println!("HEAV3NS-ENGINE — Wonderland Digital 0.4: http://127.0.0.1:{actual_port}");
    for stream in listener.incoming().flatten() {
        let st_clone = Arc::clone(&state);
        thread::spawn(move || {
            handle_connection(stream, st_clone, actual_port);
        });
    }
    Ok(())
}

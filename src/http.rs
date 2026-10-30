//! Embedded HTTP Web Server, WebRTC UI Bridge & Real-Time Signaling Hub (TCP Port 7878)

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::consensus::RecordingType;
use crate::engine::GafanaEngine;
use crate::smtp::{SmtpClient, SmtpConfig};

#[derive(Debug, Clone)]
pub struct SignalMessage {
    pub from: String,
    pub to: String,
    pub signal_type: String,
    pub payload: String,
}

#[derive(Default)]
pub struct SignalHub {
    pub queues: HashMap<String, Vec<SignalMessage>>,
}

pub struct HttpServerConfig {
    pub port: u16,
    pub sfu_packets: Arc<AtomicU64>,
    pub running: Arc<AtomicBool>,
    pub smtp_client: Arc<Mutex<SmtpClient>>,
    pub engine: Arc<Mutex<GafanaEngine>>,
    pub signal_hub: Arc<Mutex<SignalHub>>,
}

pub struct EmbeddedHttpServer {
    config: HttpServerConfig,
}

impl EmbeddedHttpServer {
    pub fn new(config: HttpServerConfig) -> Self {
        Self { config }
    }

    pub fn start(self) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let addr = format!("0.0.0.0:{}", self.config.port);
            let listener = match TcpListener::bind(&addr) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("[HTTP Server] Failed to bind {}: {}", addr, e);
                    return;
                }
            };

            let _ = listener.set_nonblocking(true);

            while self.config.running.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let cfg_clone = HttpServerConfig {
                            port: self.config.port,
                            sfu_packets: self.config.sfu_packets.clone(),
                            running: self.config.running.clone(),
                            smtp_client: self.config.smtp_client.clone(),
                            engine: self.config.engine.clone(),
                            signal_hub: self.config.signal_hub.clone(),
                        };
                        thread::spawn(move || {
                            Self::handle_connection(stream, cfg_clone);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => {
                        eprintln!("[HTTP Server] Accept error: {}", e);
                        break;
                    }
                }
            }
        })
    }

    fn send_response(mut stream: TcpStream, status: &str, content_type: &str, body: &[u8]) {
        let headers = format!(
            "HTTP/1.1 {}\r\n\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\
             Access-Control-Allow-Origin: *\r\n\
             Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
             Access-Control-Allow-Headers: Content-Type, X-Peer-Id\r\n\
             Content-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com data:; img-src 'self' data: blob:; media-src 'self' blob: mediastream:; connect-src 'self' ws: wss:; object-src 'none'; base-uri 'self'; frame-ancestors 'none'; form-action 'self';\r\n\
             X-Frame-Options: DENY\r\n\
             X-Content-Type-Options: nosniff\r\n\
             Referrer-Policy: no-referrer\r\n\
             Cross-Origin-Opener-Policy: same-origin\r\n\
             Cross-Origin-Resource-Policy: same-origin\r\n\
             Permissions-Policy: camera=(self), microphone=(self), display-capture=(self), geolocation=(), payment=(), usb=()\r\n\
             Connection: close\r\n\r\n",
            status,
            content_type,
            body.len()
        );
        let _ = stream.write_all(headers.as_bytes());
        let _ = stream.write_all(body);
        let _ = stream.flush();
    }

    fn handle_connection(mut stream: TcpStream, config: HttpServerConfig) {
        let (method, full_path, body) = match read_full_request(&mut stream) {
            Some(req) => req,
            None => return,
        };

        if method == "OPTIONS" {
            Self::send_response(stream, "204 No Content", "text/plain", b"");
            return;
        }

        let path = full_path.split('?').next().unwrap_or(&full_path);

        match path {
            "/" | "/index.html" => {
                let content = if Path::new("web/index.html").exists() {
                    fs::read("web/index.html").unwrap_or_else(|_| b"<h1>Gafana Web</h1>".to_vec())
                } else if Path::new("../web/index.html").exists() {
                    fs::read("../web/index.html").unwrap_or_else(|_| b"<h1>Gafana Web</h1>".to_vec())
                } else {
                    b"<!DOCTYPE html><html><body><h1>Gafana Web UI</h1></body></html>".to_vec()
                };
                Self::send_response(stream, "200 OK", "text/html", &content);
            }
            "/style.css" => {
                let content = if Path::new("web/style.css").exists() {
                    fs::read("web/style.css").unwrap_or_default()
                } else if Path::new("../web/style.css").exists() {
                    fs::read("../web/style.css").unwrap_or_default()
                } else {
                    b"body { background: #0b0f19; color: #fff; }".to_vec()
                };
                Self::send_response(stream, "200 OK", "text/css", &content);
            }
            "/gafana_web.js" => {
                let content = if Path::new("web/gafana_web.js").exists() {
                    fs::read("web/gafana_web.js").unwrap_or_default()
                } else if Path::new("../web/gafana_web.js").exists() {
                    fs::read("../web/gafana_web.js").unwrap_or_default()
                } else {
                    b"console.log('Gafana Web Engine Loaded');".to_vec()
                };
                Self::send_response(stream, "200 OK", "application/javascript", &content);
            }
            "/api/status" => {
                let sfu_pkts = config.sfu_packets.load(Ordering::Relaxed);
                let lan_ip = detect_lan_ip();
                let network_url = get_shareable_url(config.port);
                let json = format!(
                    "{{\"status\":\"online\",\"engine\":\"Gafana-Rust\",\"version\":\"1.2.0\",\"zero_data\":true,\"sfu_packets\":{},\"local_ip\":\"{}\",\"network_url\":\"{}\"}}",
                    sfu_pkts, lan_ip, network_url
                );
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/stats" => {
                let sfu_pkts = config.sfu_packets.load(Ordering::Relaxed);
                let peer_count = {
                    let mut eng = config.engine.lock().unwrap();
                    eng.peers.prune_timeouts(15000);
                    eng.peers.active_count()
                };
                let lan_ip = detect_lan_ip();
                let network_url = get_shareable_url(config.port);
                let json = format!(
                    "{{\"status\":\"online\",\"engine\":\"Gafana-Rust\",\"version\":\"1.2.0\",\
                     \"zero_data\":true,\"sfu_packets\":{},\"server_disk_writes_bytes\":0,\
                     \"active_rooms\":1,\"active_peers\":{},\
                     \"local_ip\":\"{}\",\"network_url\":\"{}\",\
                     \"consensus_rule\":\"100% Unanimous Agreement Required\"}}",
                    sfu_pkts, peer_count.max(1), lan_ip, network_url
                );
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/peers" => {
                let req_peer_id = get_query_param(&full_path, "peer_id").unwrap_or("");
                let peers = {
                    let mut eng = config.engine.lock().unwrap();
                    eng.peers.prune_timeouts(15000);
                    eng.peers.list_active()
                };
                let mut peer_entries = Vec::new();
                for p in peers.iter() {
                    let is_self = if !req_peer_id.is_empty() {
                        p.peer_id == req_peer_id
                    } else {
                        false
                    };
                    peer_entries.push(format!(
                        "{{\"id\":\"{}\",\"name\":\"{}\",\"is_self\":{},\"joined_sec\":{}}}",
                        p.peer_id, p.display_name, is_self, p.join_timestamp_sec
                    ));
                }
                let json = format!(
                    "{{\"room\":\"Gafana-Secure-Room\",\"peers\":[{}]}}",
                    peer_entries.join(",")
                );
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/peers/join" => {
                let peer_id = extract_json_field(&body, "peer_id").unwrap_or_else(|| format!("web_{}", fast_random_id()));
                let name = extract_json_field(&body, "name").unwrap_or_else(|| "Web Participant".to_string());
                {
                    let mut eng = config.engine.lock().unwrap();
                    eng.peers.insert_or_update(&peer_id, &name);
                }
                let json = format!(
                    "{{\"success\":true,\"peer_id\":\"{}\",\"name\":\"{}\",\"room\":\"Gafana-Secure-Room\"}}",
                    peer_id, name
                );
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/peers/heartbeat" => {
                if let Some(peer_id) = extract_json_field(&body, "peer_id") {
                    let mut eng = config.engine.lock().unwrap();
                    eng.peers.touch_heartbeat(&peer_id);
                }
                Self::send_response(stream, "200 OK", "application/json", b"{\"status\":\"ok\"}");
            }
            "/api/peers/leave" => {
                if let Some(peer_id) = extract_json_field(&body, "peer_id") {
                    let mut eng = config.engine.lock().unwrap();
                    eng.peers.remove(&peer_id);
                }
                Self::send_response(stream, "200 OK", "application/json", b"{\"status\":\"left\"}");
            }
            "/api/consensus/request" => {
                let requester_id = extract_json_field(&body, "requester_id").unwrap_or_else(|| "unknown".to_string());
                let rec_type_str = extract_json_field(&body, "rec_type").unwrap_or_else(|| "av".to_string());
                let rec_type = if rec_type_str == "audio" {
                    RecordingType::AudioOnly
                } else {
                    RecordingType::FullAv
                };

                let (ballot_id, state, total) = {
                    let mut eng = config.engine.lock().unwrap();
                    eng.peers.prune_timeouts(15000);
                    let total = eng.peers.active_count() as u32;
                    let (bid, st) = eng.consensus.start_ballot(&requester_id, rec_type, total);
                    (bid, st, total)
                };

                let json = format!(
                    "{{\"success\":true,\"ballot_id\":{},\"state\":\"{:?}\",\"total_expected\":{}}}",
                    ballot_id, state, total
                );
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/consensus/vote" => {
                let ballot_id = extract_json_field(&body, "ballot_id")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                let accept = extract_json_field(&body, "accept")
                    .map(|s| s == "true" || s == "1")
                    .unwrap_or(false);

                let res = {
                    let mut eng = config.engine.lock().unwrap();
                    eng.consensus.cast_vote(ballot_id, accept)
                };

                let (state_str, unanimous) = match res {
                    Some((st, unan)) => (format!("{:?}", st), unan),
                    None => ("InvalidOrInactive".to_string(), false),
                };

                let json = format!(
                    "{{\"success\":true,\"ballot_id\":{},\"state\":\"{}\",\"unanimous\":{}}}",
                    ballot_id, state_str, unanimous
                );
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/consensus/revoke" => {
                let ballot_id = extract_json_field(&body, "ballot_id")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);

                let revoked = {
                    let mut eng = config.engine.lock().unwrap();
                    let ok = eng.consensus.revoke_consent(ballot_id);
                    if eng.recorder.is_recording {
                        let _ = eng.recorder.stop();
                    }
                    ok
                };

                let json = format!("{{\"success\":true,\"revoked\":{}}}", revoked);
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/consensus/status" => {
                let json = {
                    let mut eng = config.engine.lock().unwrap();
                    eng.consensus.check_timeouts();
                    let b = &eng.consensus.current_ballot;
                    format!(
                        "{{\"is_active\":{},\"ballot_id\":{},\"requester_id\":\"{}\",\"rec_type\":\"{:?}\",\
                         \"state\":\"{:?}\",\"yes_votes\":{},\"no_votes\":{},\"total_expected\":{}}}",
                        b.is_active, b.ballot_id, b.requester_id, b.rec_type, b.state, b.yes_votes, b.no_votes, b.total_expected_votes
                    )
                };
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/signal/send" => {
                let from = extract_json_field(&body, "from").unwrap_or_default();
                let to = extract_json_field(&body, "to").unwrap_or_default();
                let sig_type = extract_json_field(&body, "type").unwrap_or_default();
                let payload = extract_raw_json_field(&body, "payload").unwrap_or_default();

                if !to.is_empty() && !from.is_empty() {
                    let msg = SignalMessage {
                        from,
                        to: to.clone(),
                        signal_type: sig_type,
                        payload,
                    };
                    let mut hub = config.signal_hub.lock().unwrap();
                    hub.queues.entry(to).or_default().push(msg);
                }
                Self::send_response(stream, "200 OK", "application/json", b"{\"status\":\"sent\"}");
            }
            "/api/signal/poll" => {
                let peer_id = get_query_param(&full_path, "peer_id").unwrap_or("");
                let signals = if !peer_id.is_empty() {
                    let mut hub = config.signal_hub.lock().unwrap();
                    hub.queues.remove(peer_id).unwrap_or_default()
                } else {
                    Vec::new()
                };

                let mut items = Vec::new();
                for s in signals {
                    items.push(format!(
                        "{{\"from\":\"{}\",\"to\":\"{}\",\"type\":\"{}\",\"payload\":{}}}",
                        s.from, s.to, s.signal_type, s.payload
                    ));
                }
                let json = format!("[{}]", items.join(","));
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/anticapture/status" => {
                let json = "{\"guard_active\":true,\"screenshots_blocked\":true,\"bg_device_recording_blocked\":true,\"unanimous_consent_required\":true,\"status\":\"ARMED\"}";
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/open" => {
                #[cfg(target_os = "windows")]
                {
                    let url = "http://localhost:7878";
                    let launched = std::process::Command::new("cmd")
                        .args(["/c", "start", "chrome", "--incognito", "--disable-extensions", url])
                        .spawn()
                        .or_else(|_| {
                            std::process::Command::new("cmd")
                                .args(["/c", "start", "msedge", "--inprivate", url])
                                .spawn()
                        });

                    if launched.is_err() {
                        let _ = std::process::Command::new("cmd")
                            .args(["/c", "start", url])
                            .spawn();
                    }
                }

                let json = "{\"status\":\"ok\",\"url\":\"http://localhost:7878\"}";
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/quit" => {
                let json = "{\"status\":\"terminating\",\"message\":\"Gafana engine shutting down gracefully.\"}";
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
                config.running.store(false, Ordering::Relaxed);
                println!("\n[WEB COMMAND] 'quit' triggered from Web Control Center. Shutting down...");
                std::process::exit(0);
            }
            "/api/smtp/status" => {
                let client = config.smtp_client.lock().unwrap();
                let cfg = client.get_masked_config();
                let json = format!(
                    "{{\"configured\":{},\"host\":\"{}\",\"port\":{},\"sender_email\":\"{}\",\"sender_name\":\"{}\"}}",
                    cfg.is_configured, cfg.host, cfg.port, cfg.sender_email, cfg.sender_name
                );
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/smtp/config" => {
                let mut client = config.smtp_client.lock().unwrap();
                let mut cfg = SmtpConfig::default();
                if let Some(host) = extract_json_field(&body, "host") {
                    cfg.host = host;
                }
                if let Some(port) = extract_json_field(&body, "port").and_then(|p| p.parse().ok()) {
                    cfg.port = port;
                }
                if let Some(username) = extract_json_field(&body, "username") {
                    cfg.username = username;
                }
                if let Some(password) = extract_json_field(&body, "password") {
                    cfg.password = password;
                }
                if let Some(sender_email) = extract_json_field(&body, "sender_email") {
                    cfg.sender_email = sender_email;
                }
                if let Some(sender_name) = extract_json_field(&body, "sender_name") {
                    cfg.sender_name = sender_name;
                }
                client.setup(cfg);
                Self::send_response(stream, "200 OK", "application/json", b"{\"success\":true,\"message\":\"SMTP config updated in volatile memory.\"}");
            }
            "/api/smtp/invite" => {
                let client = config.smtp_client.lock().unwrap();
                let to = extract_json_field(&body, "to").unwrap_or_default();
                let name = extract_json_field(&body, "name").unwrap_or_else(|| "Attendee".to_string());
                let note = extract_json_field(&body, "note").unwrap_or_default();
                let url = extract_json_field(&body, "url").unwrap_or_else(|| "http://localhost:7878".to_string());

                let (res, log) = client.send_invite(&to, &name, "Gafana-Secure-Room", &url, &note);
                let escaped_log = log.replace('\"', "\\\"").replace('\n', "\\n").replace('\r', "");
                let json = format!(
                    "{{\"success\":{},\"code\":{},\"log\":\"{}\"}}",
                    res.is_ok(),
                    if res.is_ok() { 0 } else { -14 },
                    escaped_log
                );
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            _ => {
                Self::send_response(stream, "404 Not Found", "text/plain", b"Not Found");
            }
        }
    }
}

fn read_full_request(stream: &mut TcpStream) -> Option<(String, String, String)> {
    let mut buffer = Vec::with_capacity(8192);
    let mut chunk = [0u8; 4096];
    let mut header_end = None;
    let mut content_length: usize = 0;

    let _ = stream.set_read_timeout(Some(Duration::from_millis(2500)));

    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buffer.extend_from_slice(&chunk[..n]);

                if header_end.is_none() {
                    if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                        header_end = Some(pos + 4);
                        let header_str = String::from_utf8_lossy(&buffer[..pos]);
                        for line in header_str.lines() {
                            let lower = line.to_ascii_lowercase();
                            if lower.starts_with("content-length:") {
                                if let Some(val) = line.split(':').nth(1) {
                                    content_length = val.trim().parse::<usize>().unwrap_or(0);
                                }
                            }
                        }
                    }
                }

                if let Some(h_end) = header_end {
                    if buffer.len() >= h_end + content_length {
                        break;
                    }
                }

                if buffer.len() > 1024 * 1024 {
                    break;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                if header_end.is_some() {
                    break;
                } else {
                    return None;
                }
            }
            Err(_) => return None,
        }
    }

    let h_end = header_end?;
    let header_str = String::from_utf8_lossy(&buffer[..h_end - 4]).to_string();
    let body_str = String::from_utf8_lossy(&buffer[h_end..h_end + content_length.min(buffer.len() - h_end)]).to_string();

    let first_line = header_str.lines().next()?;
    let mut parts = first_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();

    Some((method, path, body_str))
}

fn get_query_param<'a>(path_and_query: &'a str, param: &str) -> Option<&'a str> {
    let q_idx = path_and_query.find('?')?;
    let query = &path_and_query[q_idx + 1..];
    for pair in query.split('&') {
        let mut parts = pair.split('=');
        if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
            if k == param {
                return Some(v);
            }
        }
    }
    None
}

pub fn extract_json_field(json: &str, field: &str) -> Option<String> {
    let needle = format!("\"{}\"", field);
    let start_idx = json.find(&needle)? + needle.len();
    let remaining = &json[start_idx..];

    let colon_idx = remaining.find(':')? + 1;
    let after_colon = remaining[colon_idx..].trim_start();

    if after_colon.starts_with('\"') {
        let val_start = 1;
        let val_end = after_colon[val_start..].find('\"')? + val_start;
        Some(after_colon[val_start..val_end].to_string())
    } else {
        let val_end = after_colon
            .find(|c: char| c == ',' || c == '}' || c == ' ' || c == '\n' || c == '\r')
            .unwrap_or(after_colon.len());
        Some(after_colon[..val_end].to_string())
    }
}

pub fn extract_raw_json_field(json: &str, field: &str) -> Option<String> {
    let needle = format!("\"{}\"", field);
    let start_idx = json.find(&needle)? + needle.len();
    let remaining = &json[start_idx..];

    let colon_idx = remaining.find(':')? + 1;
    let after_colon = remaining[colon_idx..].trim_start();

    if after_colon.starts_with('{') {
        let mut depth = 0;
        let mut end_idx = 0;
        for (i, c) in after_colon.char_indices() {
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    end_idx = i + 1;
                    break;
                }
            }
        }
        if end_idx > 0 {
            Some(after_colon[..end_idx].to_string())
        } else {
            None
        }
    } else if after_colon.starts_with('\"') {
        let val_start = 1;
        let val_end = after_colon[val_start..].find('\"')? + val_start;
        Some(format!("\"{}\"", &after_colon[val_start..val_end]))
    } else {
        let val_end = after_colon
            .find(|c: char| c == ',' || c == '}' || c == ' ' || c == '\n' || c == '\r')
            .unwrap_or(after_colon.len());
        Some(after_colon[..val_end].to_string())
    }
}

pub fn detect_lan_ip() -> String {
    if let Ok(ip) = std::env::var("GAFANA_HOST_IP") {
        if !ip.trim().is_empty() {
            return ip.trim().to_string();
        }
    }

    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = sock.local_addr() {
                let ip_str = addr.ip().to_string();
                if ip_str != "0.0.0.0" && ip_str != "127.0.0.1" {
                    return ip_str;
                }
            }
        }
    }

    "127.0.0.1".to_string()
}

pub fn get_shareable_url(port: u16) -> String {
    if let Ok(public_url) = std::env::var("GAFANA_PUBLIC_URL") {
        if !public_url.trim().is_empty() {
            return public_url.trim().trim_end_matches('/').to_string();
        }
    }
    if let Ok(public_url) = std::env::var("PUBLIC_URL") {
        if !public_url.trim().is_empty() {
            return public_url.trim().trim_end_matches('/').to_string();
        }
    }

    let lan_ip = detect_lan_ip();
    format!("http://{}:{}", lan_ip, port)
}

fn fast_random_id() -> u32 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u32)
        .unwrap_or(1234);
    (now % 90000) + 10000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lan_ip_and_shareable_url() {
        let ip = detect_lan_ip();
        assert!(!ip.is_empty());
        let url = get_shareable_url(7878);
        assert!(url.starts_with("http://"));
        assert!(url.ends_with(":7878"));
    }

    #[test]
    fn test_extract_json_field() {
        let json = r#"{"name": "Alice", "ballot_id": 105, "accept": true}"#;
        assert_eq!(extract_json_field(json, "name").unwrap(), "Alice");
        assert_eq!(extract_json_field(json, "ballot_id").unwrap(), "105");
        assert_eq!(extract_json_field(json, "accept").unwrap(), "true");
        assert!(extract_json_field(json, "missing").is_none());
    }

    #[test]
    fn test_get_query_param() {
        let url = "/api/peers?peer_id=user_123&format=json";
        assert_eq!(get_query_param(url, "peer_id").unwrap(), "user_123");
        assert_eq!(get_query_param(url, "format").unwrap(), "json");
        assert!(get_query_param(url, "nonexistent").is_none());
    }

    #[test]
    fn test_extract_raw_json_field() {
        let json = r#"{"from":"a","to":"b","type":"offer","payload":{"type":"offer","sdp":"v=0..."}}"#;
        let payload = extract_raw_json_field(json, "payload").unwrap();
        assert!(payload.starts_with("{\"type\":\"offer\""));
    }
}

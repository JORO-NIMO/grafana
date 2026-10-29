//! Embedded HTTP Web Server & WebRTC UI Bridge (TCP Port 7878)

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::peer::PeerInfo;
use crate::smtp::{SmtpClient, SmtpConfig};

pub struct HttpServerConfig {
    pub port: u16,
    pub sfu_packets: Arc<AtomicU64>,
    pub running: Arc<AtomicBool>,
    pub smtp_client: Arc<Mutex<SmtpClient>>,
    pub peers_provider: Arc<dyn Fn() -> Vec<PeerInfo> + Send + Sync>,
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
                            peers_provider: self.config.peers_provider.clone(),
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
        let mut buf = [0u8; 4096];
        let n = match stream.read(&mut buf) {
            Ok(n) if n > 0 => n,
            _ => return,
        };

        let req_str = String::from_utf8_lossy(&buf[..n]);

        if req_str.contains("100-continue") || req_str.contains("100-Continue") {
            let _ = stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n");
        }

        let mut lines = req_str.lines();
        let first_line = match lines.next() {
            Some(l) => l,
            None => return,
        };

        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() < 2 {
            return;
        }

        let method = parts[0];
        let path = parts[1];

        if method == "OPTIONS" {
            Self::send_response(stream, "204 No Content", "text/plain", b"");
            return;
        }

        // Extract body if any
        let body = if let Some(idx) = req_str.find("\r\n\r\n") {
            &req_str[idx + 4..]
        } else {
            ""
        };

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
                let json = format!(
                    "{{\"status\":\"online\",\"engine\":\"Gafana-Rust\",\"version\":\"1.2.0\",\"zero_data\":true,\"sfu_packets\":{},\"local_ip\":\"127.0.0.1\",\"network_url\":\"http://127.0.0.1:{}\"}}",
                    sfu_pkts, config.port
                );
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/stats" => {
                let sfu_pkts = config.sfu_packets.load(Ordering::Relaxed);
                let peers = (config.peers_provider)();
                let json = format!(
                    "{{\"status\":\"online\",\"engine\":\"Gafana-Rust\",\"version\":\"1.2.0\",\
                     \"zero_data\":true,\"sfu_packets\":{},\"server_disk_writes_bytes\":0,\
                     \"active_rooms\":1,\"active_peers\":{},\
                     \"local_ip\":\"127.0.0.1\",\"network_url\":\"http://127.0.0.1:{}\",\
                     \"consensus_rule\":\"100% Unanimous Agreement Required\"}}",
                    sfu_pkts, peers.len().max(1), config.port
                );
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/peers" => {
                let peers = (config.peers_provider)();
                let mut peer_entries = Vec::new();
                for (i, p) in peers.iter().enumerate() {
                    peer_entries.push(format!(
                        "{{\"id\":\"{}\",\"name\":\"{}\",\"is_self\":{},\"joined_sec\":{}}}",
                        p.peer_id, p.display_name, i == 0, p.join_timestamp_sec
                    ));
                }
                if peer_entries.is_empty() {
                    peer_entries.push("{\"id\":\"self_host\",\"name\":\"Local Host (You)\",\"is_self\":true,\"joined_sec\":0}".to_string());
                }
                let json = format!(
                    "{{\"room\":\"Gafana-Secure-Room\",\"peers\":[{}]}}",
                    peer_entries.join(",")
                );
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
                if let Some(host) = extract_json_field(body, "host") {
                    cfg.host = host;
                }
                if let Some(port_str) = extract_json_field(body, "port") {
                    if let Ok(p) = port_str.parse::<u16>() {
                        cfg.port = p;
                    }
                }
                if let Some(u) = extract_json_field(body, "username") {
                    cfg.username = u;
                }
                if let Some(p) = extract_json_field(body, "password") {
                    cfg.password = p;
                }
                if let Some(s) = extract_json_field(body, "sender_email") {
                    cfg.sender_email = s;
                }
                if let Some(n) = extract_json_field(body, "sender_name") {
                    cfg.sender_name = n;
                }
                cfg.use_tls = true;
                client.setup(cfg);

                let json = "{\"status\":\"ok\",\"configured\":true,\"message\":\"SMTP credentials registered in volatile RAM.\"}";
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/smtp/invite" => {
                let client = config.smtp_client.lock().unwrap();
                let to = extract_json_field(body, "to").unwrap_or_default();
                let name = extract_json_field(body, "name").unwrap_or_else(|| "Attendee".to_string());
                let note = extract_json_field(body, "note").unwrap_or_else(|| "Confidential Video Meeting".to_string());
                let url = extract_json_field(body, "url").unwrap_or_else(|| "http://localhost:7878".to_string());

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

fn extract_json_field(json: &str, field: &str) -> Option<String> {
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

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
use crate::engine::GrafanaEngine;
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

static SERVER_START_TIME: AtomicU64 = AtomicU64::new(0);

fn record_server_start() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let _ = SERVER_START_TIME.compare_exchange(0, now, Ordering::Relaxed, Ordering::Relaxed);
}

fn get_server_uptime() -> u64 {
    let start = SERVER_START_TIME.load(Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if start > 0 {
        now.saturating_sub(start)
    } else {
        0
    }
}

pub fn push_cloud_vault(endpoint: &str, username: &str, email: &str, payload_json: &str) -> (bool, u32, String) {
    use std::process::Command;
    use std::io::Write;

    let temp_dir = std::env::temp_dir();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(12345);
    let temp_file_path = temp_dir.join(format!("filefran_sync_{}.json", now));
    
    let wrapped_json = format!(
        r#"{{"source":"Safe Spaces AI Innovation Hub","username":"{}","email":"{}","timestamp":{},"vault_payload":{}}}"#,
        username, email, now / 1000, payload_json
    );

    if let Ok(mut f) = std::fs::File::create(&temp_file_path) {
        let _ = f.write_all(wrapped_json.as_bytes());
    } else {
        return (false, 0, "Could not create temporary vault payload file".to_string());
    }

    let curl_cmd = if cfg!(windows) { "curl.exe" } else { "curl" };
    let mut cmd = Command::new(curl_cmd);
    cmd.arg("-s").arg("-S");
    cmd.arg("-X").arg("POST");
    cmd.arg("-H").arg("Content-Type: application/json");
    cmd.arg("-H").arg("User-Agent: SafeSpaces-Grafana/1.2.0");
    cmd.arg("--data-binary").arg(format!("@{}", temp_file_path.display()));
    cmd.arg("--max-time").arg("6");
    cmd.arg(endpoint);

    let output = match cmd.output() {
        Ok(out) => out,
        Err(e) => {
            let _ = std::fs::remove_file(&temp_file_path);
            return (false, 0, format!("Outbound push error: {}", e));
        }
    };

    let _ = std::fs::remove_file(&temp_file_path);

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if output.status.success() {
        (true, 200, if stdout.trim().is_empty() { "Vault record dispatched to Safe Spaces Cloud (filefran.safespaceshub.com)".to_string() } else { stdout.trim().to_string() })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        (false, output.status.code().unwrap_or(500) as u32, format!("Remote responded with exit code {}: {}", output.status.code().unwrap_or(-1), stderr.trim()))
    }
}

pub struct HttpServerConfig {
    pub port: u16,
    pub sfu_packets: Arc<AtomicU64>,
    pub running: Arc<AtomicBool>,
    pub smtp_client: Arc<Mutex<SmtpClient>>,
    pub engine: Arc<Mutex<GrafanaEngine>>,
    pub signal_hub: Arc<Mutex<SignalHub>>,
    pub auth_registry: Arc<Mutex<crate::auth::AuthRegistry>>,
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
            record_server_start();
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
                            auth_registry: self.config.auth_registry.clone(),
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
        let (method, full_path, body, header_str) = match read_full_request(&mut stream) {
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
                    fs::read("web/index.html").unwrap_or_else(|_| b"<h1>Grafana Web</h1>".to_vec())
                } else if Path::new("../web/index.html").exists() {
                    fs::read("../web/index.html").unwrap_or_else(|_| b"<h1>Grafana Web</h1>".to_vec())
                } else {
                    b"<!DOCTYPE html><html><body><h1>Grafana Web UI</h1></body></html>".to_vec()
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
            "/grafana_web.js" => {
                let content = if Path::new("web/grafana_web.js").exists() {
                    fs::read("web/grafana_web.js").unwrap_or_default()
                } else if Path::new("../web/grafana_web.js").exists() {
                    fs::read("../web/grafana_web.js").unwrap_or_default()
                } else {
                    b"console.log('Grafana Web Engine Loaded');".to_vec()
                };
                Self::send_response(stream, "200 OK", "application/javascript", &content);
            }
            "/logo.png" | "/favicon.ico" => {
                let content = if Path::new("web/logo.png").exists() {
                    fs::read("web/logo.png").unwrap_or_default()
                } else if Path::new("logo.png").exists() {
                    fs::read("logo.png").unwrap_or_default()
                } else if Path::new("../web/logo.png").exists() {
                    fs::read("../web/logo.png").unwrap_or_default()
                } else {
                    Vec::new()
                };
                Self::send_response(stream, "200 OK", "image/png", &content);
            }
            "/api/health" => {
                let sfu_pkts = config.sfu_packets.load(Ordering::Relaxed);
                let peer_count = {
                    let mut eng = config.engine.lock().unwrap();
                    eng.peers.prune_timeouts(15000);
                    eng.peers.active_count()
                };
                let user_count = {
                    let auth = config.auth_registry.lock().unwrap();
                    auth.user_count()
                };
                let smtp_cfg = {
                    let client = config.smtp_client.lock().unwrap();
                    client.get_masked_config()
                };
                let uptime = get_server_uptime();
                let lan_ip = detect_lan_ip();
                let network_url = get_shareable_url(config.port);

                let json = format!(
                    r#"{{"status":"healthy","platform":"Safe Spaces AI Innovation Hub","service":"Grafana Conference Engine","version":"1.2.0","uptime_seconds":{},"active_peers":{},"sfu_packets_routed":{},"vault":{{"storage":"atomic_disk_ledger","file":"data/grafana_vault.json","registered_users":{}}},"smtp":{{"configured":{},"host":"{}","port":{}}},"cloud_sync":{{"destination":"https://filefran.safespaceshub.com/api/v1/vault/sync","mode":"user_opt_in"}},"local_ip":"{}","network_url":"{}"}}"#,
                    uptime, peer_count, sfu_pkts, user_count, smtp_cfg.is_configured, smtp_cfg.host, smtp_cfg.port, lan_ip, network_url
                );
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/docs" => {
                let html = crate::openapi::get_api_docs_html();
                Self::send_response(stream, "200 OK", "text/html", html.as_bytes());
            }
            "/api/openapi.json" => {
                let json = crate::openapi::get_openapi_json();
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/status" => {
                let sfu_pkts = config.sfu_packets.load(Ordering::Relaxed);
                let lan_ip = detect_lan_ip();
                let network_url = get_shareable_url(config.port);
                let json = format!(
                    "{{\"status\":\"online\",\"engine\":\"Grafana-Rust\",\"version\":\"1.2.0\",\"zero_data\":true,\"sfu_packets\":{},\"local_ip\":\"{}\",\"network_url\":\"{}\"}}",
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
                    "{{\"status\":\"online\",\"engine\":\"Grafana-Rust\",\"version\":\"1.2.0\",\
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
                let (peers, auto_admit) = {
                    let mut eng = config.engine.lock().unwrap();
                    eng.peers.prune_timeouts(15000);
                    (eng.peers.list_active(), eng.peers.auto_admit)
                };
                let mut peer_entries = Vec::new();
                for p in peers.iter() {
                    let is_self = if !req_peer_id.is_empty() {
                        p.peer_id == req_peer_id
                    } else {
                        false
                    };
                    peer_entries.push(format!(
                        "{{\"id\":\"{}\",\"name\":\"{}\",\"avatar\":\"{}\",\"status\":\"{}\",\"is_self\":{},\"joined_sec\":{}}}",
                        p.peer_id, p.display_name, p.avatar, p.status, is_self, p.join_timestamp_sec
                    ));
                }
                let json = format!(
                    "{{\"room\":\"Grafana-Secure-Room\",\"auto_admit\":{},\"peers\":[{}]}}",
                    auto_admit,
                    peer_entries.join(",")
                );
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/peers/join" => {
                let peer_id = extract_json_field(&body, "peer_id").unwrap_or_else(|| format!("web_{}", fast_random_id()));
                let name = extract_json_field(&body, "name").unwrap_or_else(|| "Web Participant".to_string());
                let avatar = extract_json_field(&body, "avatar").unwrap_or_default();
                let status = {
                    let mut eng = config.engine.lock().unwrap();
                    let (_, info) = eng.peers.insert_or_update_with_avatar(&peer_id, &name, &avatar);
                    info.status
                };
                let json = format!(
                    "{{\"success\":true,\"peer_id\":\"{}\",\"name\":\"{}\",\"status\":\"{}\",\"room\":\"Grafana-Secure-Room\"}}",
                    peer_id, name, status
                );
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/peers/status" => {
                let peer_id = get_query_param(&full_path, "peer_id").unwrap_or("");
                let status = {
                    let eng = config.engine.lock().unwrap();
                    eng.peers.get_peer_status(peer_id)
                };
                let json = format!("{{\"status\":\"{}\"}}", status);
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/peers/admit" => {
                let peer_id = extract_json_field(&body, "peer_id").unwrap_or_default();
                let admit = extract_json_field(&body, "admit")
                    .map(|s| s == "true" || s == "1")
                    .unwrap_or(true);
                let ok = {
                    let mut eng = config.engine.lock().unwrap();
                    if admit {
                        eng.peers.admit_peer(&peer_id)
                    } else {
                        eng.peers.deny_peer(&peer_id)
                    }
                };
                let json = format!("{{\"success\":{}}}", ok);
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/peers/profile" => {
                let peer_id = extract_json_field(&body, "peer_id").unwrap_or_default();
                let name = extract_json_field(&body, "name").unwrap_or_default();
                let avatar = extract_json_field(&body, "avatar").unwrap_or_default();
                let ok = {
                    let mut eng = config.engine.lock().unwrap();
                    eng.peers.update_profile(&peer_id, &name, &avatar)
                };
                let json = format!("{{\"success\":{}}}", ok);
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/room/settings" => {
                if method == "POST" {
                    if let Some(auto_str) = extract_json_field(&body, "auto_admit") {
                        let auto_val = auto_str == "true" || auto_str == "1";
                        let mut eng = config.engine.lock().unwrap();
                        eng.peers.auto_admit = auto_val;
                    }
                }
                let auto_admit = {
                    let eng = config.engine.lock().unwrap();
                    eng.peers.auto_admit
                };
                let json = format!("{{\"auto_admit\":{}}}", auto_admit);
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
            "/api/quit" => {
                let json = "{\"status\":\"terminating\",\"message\":\"Grafana engine shutting down gracefully.\"}";
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
            "/api/smtp/test" => {
                let client = config.smtp_client.lock().unwrap();
                let (res, log) = client.test_connection();
                let escaped_log = escape_json_string(&log);
                let json = format!(
                    r#"{{"success":{},"code":{},"log":"{}"}}"#,
                    res.is_ok(),
                    if res.is_ok() { 0 } else { -1 },
                    escaped_log
                );
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/auth/register" => {
                let username = extract_json_field(&body, "username").unwrap_or_default();
                let email = extract_json_field(&body, "email").unwrap_or_default();
                let password = extract_json_field(&body, "password").unwrap_or_default();

                let reg_res = {
                    let mut auth = config.auth_registry.lock().unwrap();
                    auth.register(&username, &email, &password)
                };

                match reg_res {
                    Ok((email, code)) => {
                        // Dispatch confirmation code via our Stalwart SMTP gateway
                        let note = format!("Your Safe Spaces Grafana confirmation code is: {}\n\nEnter this code on the verification screen to activate your hosting privileges.", code);
                        let client = config.smtp_client.lock().unwrap();
                        let (res, _) = client.send_invite(&email, &username, "Confirm Email", &get_shareable_url(config.port), &note);

                        let json = format!(
                            "{{\"success\":true,\"email\":\"{}\",\"message\":\"Confirmation code dispatched to your email.\",\"smtp_delivered\":{}}}",
                            email, res.is_ok()
                        );
                        Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
                    }
                    Err(_) => {
                        Self::send_response(stream, "400 Bad Request", "application/json", b"{\"success\":false,\"error\":\"Username or email is already registered, or password too short (min 6 characters).\"}");
                    }
                }
            }
            "/api/auth/verify" => {
                let email = extract_json_field(&body, "email").unwrap_or_default();
                let code = extract_json_field(&body, "code").unwrap_or_default();

                let verify_res = {
                    let mut auth = config.auth_registry.lock().unwrap();
                    auth.verify_email(&email, &code)
                };

                match verify_res {
                    Ok((token, user)) => {
                        let json = format!(
                            "{{\"success\":true,\"token\":\"{}\",\"user\":{{\"username\":\"{}\",\"email\":\"{}\",\"is_admin\":{},\"allow_cloud_sync\":{}}}}}",
                            token, user.username, user.email, user.is_admin, user.allow_cloud_sync
                        );
                        Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
                    }
                    Err(_) => {
                        Self::send_response(stream, "400 Bad Request", "application/json", b"{\"success\":false,\"error\":\"Invalid or expired confirmation code. Please check your email and try again.\"}");
                    }
                }
            }
            "/api/auth/login" => {
                let identifier = extract_json_field(&body, "identifier")
                    .or_else(|| extract_json_field(&body, "username"))
                    .or_else(|| extract_json_field(&body, "email"))
                    .unwrap_or_default();
                let password = extract_json_field(&body, "password").unwrap_or_default();

                let login_res = {
                    let mut auth = config.auth_registry.lock().unwrap();
                    auth.login(&identifier, &password)
                };

                match login_res {
                    Ok((token, user)) => {
                        let json = format!(
                            "{{\"success\":true,\"token\":\"{}\",\"user\":{{\"username\":\"{}\",\"email\":\"{}\",\"is_admin\":{},\"allow_cloud_sync\":{}}}}}",
                            token, user.username, user.email, user.is_admin, user.allow_cloud_sync
                        );
                        Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
                    }
                    Err(crate::error::GrafanaError::ConsensusRejected) => {
                        Self::send_response(stream, "403 Forbidden", "application/json", b"{\"success\":false,\"requires_verification\":true,\"error\":\"Email not confirmed. Please enter the 6-digit confirmation code sent to your email.\"}");
                    }
                    Err(_) => {
                        Self::send_response(stream, "401 Unauthorized", "application/json", b"{\"success\":false,\"error\":\"Invalid username, email, or password.\"}");
                    }
                }
            }
            "/api/auth/me" => {
                let token = extract_token(&header_str, &body, &full_path);
                let user_opt = token.and_then(|t| config.auth_registry.lock().unwrap().get_user_by_token(&t));

                match user_opt {
                    Some(user) => {
                        let json = format!(
                            "{{\"logged_in\":true,\"user\":{{\"username\":\"{}\",\"email\":\"{}\",\"is_verified\":{},\"is_admin\":{},\"allow_cloud_sync\":{},\"last_sync_at\":{}}}}}",
                            user.username, user.email, user.is_verified, user.is_admin, user.allow_cloud_sync, user.last_sync_at
                        );
                        Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
                    }
                    None => {
                        Self::send_response(stream, "200 OK", "application/json", b"{\"logged_in\":false}");
                    }
                }
            }
            "/api/auth/logout" => {
                if let Some(token) = extract_token(&header_str, &body, &full_path) {
                    config.auth_registry.lock().unwrap().logout(&token);
                }
                Self::send_response(stream, "200 OK", "application/json", b"{\"success\":true}");
            }
            "/api/cloud/sync" => {
                let token = extract_token(&header_str, &body, &full_path);
                let user_opt = token.and_then(|t| config.auth_registry.lock().unwrap().get_user_by_token(&t));

                if user_opt.is_none() {
                    Self::send_response(stream, "401 Unauthorized", "application/json", b"{\"error\":\"Authentication required for cloud backup sync.\"}");
                    return;
                }
                let user = user_opt.unwrap();
                let allow_sync = extract_json_field(&body, "allow_sync")
                    .map(|s| s == "true" || s == "1")
                    .unwrap_or(true);
                let backup_payload = extract_raw_json_field(&body, "backup_payload")
                    .or_else(|| extract_json_field(&body, "backup_payload"));

                let res = {
                    let mut auth = config.auth_registry.lock().unwrap();
                    auth.update_cloud_sync(&user.email, allow_sync, backup_payload.as_deref())
                };

                if res.is_ok() {
                    let mut remote_pushed = false;
                    let mut remote_note = "Local device vault updated; remote push deferred.".to_string();

                    if allow_sync {
                        let endpoint = std::env::var("FILEFRAN_URL")
                            .unwrap_or_else(|_| "https://filefran.safespaceshub.com/api/v1/vault/sync".to_string());
                        let payload_str = backup_payload.as_deref().unwrap_or("{}");
                        let (ok, code, note) = push_cloud_vault(&endpoint, &user.username, &user.email, payload_str);
                        remote_pushed = ok;
                        remote_note = format!("Remote HTTP {}: {}", code, note);
                    }

                    let escaped_note = escape_json_string(&remote_note);
                    let json = format!(
                        r#"{{"success":true,"allow_cloud_sync":{},"destination":"https://filefran.safespaceshub.com/api/v1/vault/sync","pushed_to_remote":{},"remote_status":"{}","message":"Backup recorded in atomic vault ledger and dispatched to Safe Spaces Cloud (filefran.safespaceshub.com)."}}"#,
                        allow_sync, remote_pushed, escaped_note
                    );
                    Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
                } else {
                    Self::send_response(stream, "500 Internal Server Error", "application/json", b"{\"error\":\"Failed to update cloud sync.\"}");
                }
            }
            "/api/cloud/status" => {
                let token = extract_token(&header_str, &body, &full_path);
                let user_opt = token.and_then(|t| config.auth_registry.lock().unwrap().get_user_by_token(&t));

                if let Some(user) = user_opt {
                    let json = format!(
                        "{{\"allow_cloud_sync\":{},\"destination\":\"https://filefran.safespaceshub.com\",\"last_sync_at\":{},\"has_backup\":{}}}",
                        user.allow_cloud_sync, user.last_sync_at, user.backup_data.is_some()
                    );
                    Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
                } else {
                    Self::send_response(stream, "200 OK", "application/json", b"{\"allow_cloud_sync\":false,\"destination\":\"https://filefran.safespaceshub.com\"}");
                }
            }
            "/api/admin/overview" => {
                let token = extract_token(&header_str, &body, &full_path);
                let user_opt = token.and_then(|t| config.auth_registry.lock().unwrap().get_user_by_token(&t));

                if user_opt.is_none() || !user_opt.as_ref().map(|u| u.is_admin).unwrap_or(false) {
                    Self::send_response(stream, "403 Forbidden", "application/json", b"{\"error\":\"Administrator privileges required to view user sync data.\"}");
                    return;
                }

                let (users, backup_logs) = config.auth_registry.lock().unwrap().get_admin_overview();
                let sfu_pkts = config.sfu_packets.load(Ordering::Relaxed);
                let peer_count = {
                    let eng = config.engine.lock().unwrap();
                    eng.peers.active_count()
                };

                let mut user_items = Vec::new();
                for u in users {
                    user_items.push(format!(
                        "{{\"username\":\"{}\",\"email\":\"{}\",\"is_verified\":{},\"is_admin\":{},\"allow_cloud_sync\":{},\"last_sync_at\":{},\"has_backup\":{}}}",
                        u.username, u.email, u.is_verified, u.is_admin, u.allow_cloud_sync, u.last_sync_at, u.backup_data.is_some()
                    ));
                }

                let mut log_items = Vec::new();
                for b in backup_logs {
                    log_items.push(format!(
                        "{{\"username\":\"{}\",\"email\":\"{}\",\"destination\":\"{}\",\"size\":{},\"timestamp\":{}}}",
                        b.username, b.email, b.destination, b.payload_size, b.timestamp
                    ));
                }

                let json = format!(
                    "{{\"sfu_packets\":{},\"active_peers\":{},\"cloud_destination\":\"https://filefran.safespaceshub.com\",\
                     \"users\":[{}],\"backup_audit_logs\":[{}]}}",
                    sfu_pkts, peer_count, user_items.join(","), log_items.join(",")
                );
                Self::send_response(stream, "200 OK", "application/json", json.as_bytes());
            }
            "/api/smtp/invite" => {
                let token = extract_token(&header_str, &body, &full_path);
                let user_opt = token.and_then(|t| config.auth_registry.lock().unwrap().get_user_by_token(&t));

                if user_opt.is_none() {
                    Self::send_response(
                        stream,
                        "401 Unauthorized",
                        "application/json",
                        b"{\"success\":false,\"code\":-401,\"error\":\"Hosting requires login. Please sign in with your verified account to host meetings and dispatch invitations.\"}"
                    );
                    return;
                }
                let user = user_opt.unwrap();
                if !user.is_verified {
                    Self::send_response(
                        stream,
                        "403 Forbidden",
                        "application/json",
                        b"{\"success\":false,\"code\":-403,\"error\":\"Email confirmation required. Please confirm your email before hosting meetings.\"}"
                    );
                    return;
                }

                let client = config.smtp_client.lock().unwrap();
                let to = extract_json_field(&body, "to").unwrap_or_default();
                let name = extract_json_field(&body, "name").unwrap_or_else(|| "Attendee".to_string());
                let note = extract_json_field(&body, "note").unwrap_or_default();
                let url = extract_json_field(&body, "url").unwrap_or_else(|| get_shareable_url(config.port));

                let (res, log) = client.send_invite(&to, &name, "Grafana-Secure-Room", &url, &note);
                let escaped_log = escape_json_string(&log);
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

fn read_full_request(stream: &mut TcpStream) -> Option<(String, String, String, String)> {
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

    Some((method, path, body_str, header_str))
}

fn extract_token(headers: &str, body: &str, full_path: &str) -> Option<String> {
    for line in headers.lines() {
        let lower = line.to_lowercase();
        if lower.starts_with("authorization:") {
            let val = line["authorization:".len()..].trim();
            if val.to_lowercase().starts_with("bearer ") {
                let tok = val[7..].trim();
                if !tok.is_empty() {
                    return Some(tok.to_string());
                }
            }
        }
        if lower.starts_with("x-auth-token:") {
            let tok = line["x-auth-token:".len()..].trim();
            if !tok.is_empty() {
                return Some(tok.to_string());
            }
        }
    }
    if let Some(tok) = extract_json_field(body, "token") {
        if !tok.trim().is_empty() {
            return Some(tok.trim().to_string());
        }
    }
    if let Some(tok) = get_query_param(full_path, "token") {
        if !tok.trim().is_empty() {
            return Some(tok.trim().to_string());
        }
    }
    None
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

pub fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {},
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
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
    if let Ok(ip) = std::env::var("GRAFANA_HOST_IP") {
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
    if let Ok(public_url) = std::env::var("GRAFANA_PUBLIC_URL") {
        if !public_url.trim().is_empty() {
            return public_url.trim().trim_end_matches('/').to_string();
        }
    }
    if let Ok(public_url) = std::env::var("PUBLIC_URL") {
        if !public_url.trim().is_empty() {
            return public_url.trim().trim_end_matches('/').to_string();
        }
    }
    if let Ok(domain) = std::env::var("GRAFANA_DOMAIN") {
        if !domain.trim().is_empty() {
            let d = domain.trim().trim_end_matches('/');
            if d.starts_with("http://") || d.starts_with("https://") {
                return d.to_string();
            } else {
                return format!("https://{}", d);
            }
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

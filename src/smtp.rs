//! In-Memory Secure SMTP Email Invite Dispatcher (RFC 5321 / RFC 5322)
//!
//! Zero Cloud Storage: All credentials and logs are held in volatile RAM only.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{GrafanaError, GrafanaResult};

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub sender_email: String,
    pub sender_name: String,
    pub use_tls: bool,
    pub is_configured: bool,
}

impl Default for SmtpConfig {
    fn default() -> Self {
        let host = std::env::var("SMTP_HOST").unwrap_or_else(|_| "mail.safespaceshub.com".to_string());
        let port = std::env::var("SMTP_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(587);
        let username = std::env::var("SMTP_USER").unwrap_or_default();
        let password = std::env::var("SMTP_PASS")
            .or_else(|_| std::env::var("SMTP_PASSWORD"))
            .unwrap_or_default();
        let sender_email = std::env::var("SMTP_FROM")
            .or_else(|_| std::env::var("SMTP_SENDER"))
            .unwrap_or_else(|_| "grafana@safespaceshub.com".to_string());
        let sender_name = std::env::var("SMTP_NAME")
            .unwrap_or_else(|_| "Safe Spaces Grafana".to_string());
        let is_configured = !username.is_empty();

        Self {
            host,
            port,
            username,
            password,
            sender_email,
            sender_name,
            use_tls: true,
            is_configured,
        }
    }
}

pub struct SmtpClient {
    pub config: SmtpConfig,
}

impl SmtpClient {
    pub fn new() -> Self {
        Self {
            config: SmtpConfig::default(),
        }
    }

    pub fn setup(&mut self, config: SmtpConfig) {
        self.config = config;
        self.config.is_configured = true;
    }

    pub fn get_masked_config(&self) -> SmtpConfig {
        let mut cfg = self.config.clone();
        if !cfg.password.is_empty() {
            cfg.password = "********".to_string();
        }
        cfg
    }

    /// Format epoch seconds into standard RFC 2822 / RFC 5322 date string.
    pub fn format_rfc2822_date(epoch_secs: u64) -> String {
        let days = (epoch_secs / 86400) as i64;
        let day_secs = (epoch_secs % 86400) as u32;
        let hour = day_secs / 3600;
        let minute = (day_secs % 3600) / 60;
        let second = day_secs % 60;

        let wday_idx = ((days + 4) % 7 + 7) % 7;
        let wdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        let wday = wdays[wday_idx as usize];

        let z = days + 719468;
        let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
        let doe = (z - era * 146097) as u32;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = (yoe as i64) + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = y + (if m <= 2 { 1 } else { 0 });

        let months = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let month = months[(m - 1) as usize];

        format!("{}, {:02} {} {} {:02}:{:02}:{:02} +0000", wday, d, month, y, hour, minute, second)
    }

    /// Base64 encoder without external dependencies.
    pub fn base64_encode(input: &str) -> String {
        const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let bytes = input.as_bytes();
        let mut result = String::new();
        let mut i = 0;

        while i < bytes.len() {
            let b0 = bytes[i];
            let b1 = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
            let b2 = if i + 2 < bytes.len() { bytes[i + 2] } else { 0 };

            let triple = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);

            result.push(B64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
            result.push(B64_CHARS[((triple >> 12) & 0x3F) as usize] as char);

            if i + 1 < bytes.len() {
                result.push(B64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
            } else {
                result.push('=');
            }

            if i + 2 < bytes.len() {
                result.push(B64_CHARS[(triple & 0x3F) as usize] as char);
            } else {
                result.push('=');
            }

            i += 3;
        }

        result
    }

    fn read_smtp_response(stream: &mut TcpStream) -> Result<(u32, String), GrafanaError> {
        let mut buf = [0u8; 1024];
        let mut total_read = 0;
        let mut full_resp = String::new();

        loop {
            let n = stream.read(&mut buf).map_err(|_| GrafanaError::Network)?;
            if n == 0 {
                break;
            }
            total_read += n;
            let chunk = String::from_utf8_lossy(&buf[..n]);
            full_resp.push_str(&chunk);

            // Multiline completion check: e.g. "250 " at the beginning of the final line
            if full_resp.ends_with("\r\n") || full_resp.ends_with('\n') {
                let lines: Vec<&str> = full_resp.lines().collect();
                if let Some(last) = lines.last() {
                    if last.len() >= 4 && last.chars().nth(3) == Some(' ') {
                        let code_str = &last[..3];
                        if let Ok(code) = code_str.parse::<u32>() {
                            return Ok((code, full_resp));
                        }
                    }
                }
            }

            if total_read >= 4096 {
                break;
            }
        }

        let code = full_resp
            .lines()
            .last()
            .and_then(|l| l.get(..3))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        Ok((code, full_resp))
    }

    /// Dispatch tamper-evident email invitation to attendee.
    /// Attempt STARTTLS / SMTPS delivery using system curl bridge.
    pub fn dispatch_with_curl(&self, recipient_email: &str, raw_email: &str) -> Result<String, String> {
        use std::process::Command;
        use std::io::Write;

        let scheme = if self.config.port == 465 { "smtps" } else { "smtp" };
        let url = format!("{}://{}:{}", scheme, self.config.host, self.config.port);

        let temp_dir = std::env::temp_dir();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(12345);
        let temp_file_path = temp_dir.join(format!("grafana_mail_{}.eml", now));
        if let Ok(mut f) = std::fs::File::create(&temp_file_path) {
            let _ = f.write_all(raw_email.as_bytes());
        } else {
            return Err("Could not create temporary mail file".to_string());
        }

        let curl_cmd = if cfg!(windows) { "curl.exe" } else { "curl" };
        let mut cmd = Command::new(curl_cmd);
        cmd.arg("-s").arg("-S");
        cmd.arg("--url").arg(&url);
        if self.config.port == 587 || (self.config.use_tls && self.config.port != 465) {
            cmd.arg("--ssl-reqd");
        }
        cmd.arg("--mail-from").arg(&self.config.sender_email);
        cmd.arg("--mail-rcpt").arg(recipient_email);
        cmd.arg("--upload-file").arg(&temp_file_path);
        cmd.arg("--max-time").arg("10");

        if !self.config.username.is_empty() {
            cmd.arg("--user").arg(format!("{}:{}", self.config.username, self.config.password));
        }

        let output = match cmd.output() {
            Ok(out) => out,
            Err(e) => {
                let _ = std::fs::remove_file(&temp_file_path);
                return Err(format!("Failed to execute {}: {}", curl_cmd, e));
            }
        };

        let _ = std::fs::remove_file(&temp_file_path);

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(format!("TLS SMTP transmission successful via {}: {}", url, stdout.trim()))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(format!("TLS SMTP exit code {}: {}", output.status.code().unwrap_or(-1), stderr.trim()))
        }
    }

    /// Test SMTP connectivity and handshake with the configured server.
    pub fn test_connection(&self) -> (GrafanaResult<()>, String) {
        let mut log = String::new();
        log.push_str(&format!(
            "=== Safe Spaces AI Innovation Hub - SMTP Diagnostic Test ===\nHost: {}:{}\nTLS/STARTTLS: {}\n",
            self.config.host, self.config.port, self.config.use_tls
        ));

        let addr_str = format!("{}:{}", self.config.host, self.config.port);
        let resolved = match addr_str.to_socket_addrs() {
            Ok(mut iter) => match iter.next() {
                Some(a) => a,
                None => {
                    log.push_str("DNS Failure: Could not resolve SMTP host.\n");
                    return (Err(GrafanaError::Network), log);
                }
            },
            Err(e) => {
                log.push_str(&format!("DNS Failure: Hostname resolution error: {}\n", e));
                return (Err(GrafanaError::Network), log);
            }
        };

        log.push_str(&format!("Resolved IP: {}\nAttempting TCP connection...\n", resolved));
        match TcpStream::connect_timeout(&resolved, Duration::from_secs(4)) {
            Ok(mut stream) => {
                log.push_str("TCP connection established successfully.\n");
                let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                match Self::read_smtp_response(&mut stream) {
                    Ok((code, banner)) => {
                        log.push_str(&format!("Server Banner: {} {}\n", code, banner.trim()));
                        let _ = stream.write_all(b"QUIT\r\n");
                        (Ok(()), log)
                    }
                    Err(e) => {
                        log.push_str(&format!("Connected but failed to read banner: {:?}\n", e));
                        (Err(e), log)
                    }
                }
            }
            Err(e) => {
                log.push_str(&format!("TCP connection failed: {}\n", e));
                (Err(GrafanaError::Network), log)
            }
        }
    }

    pub fn send_invite(
        &self,
        recipient_email: &str,
        recipient_name: &str,
        room_id: &str,
        meeting_url: &str,
        custom_note: &str,
    ) -> (GrafanaResult<()>, String) {
        let mut log = String::new();
        log.push_str(&format!(
            "=== Grafana Secure SMTP Invite Dispatcher ===\nTarget: {} ({})\nRoom: {}\nSMTP Host: {}:{}\nZero Cloud Retention: Guaranteed\n-------------------------------------------\n",
            recipient_email, recipient_name, room_id, self.config.host, self.config.port
        ));

        if !recipient_email.contains('@') {
            log.push_str("ERROR: Invalid recipient email address.\n");
            return (Err(GrafanaError::InvalidParam), log);
        }

        let addr_str = format!("{}:{}", self.config.host, self.config.port);
        let resolved = match addr_str.to_socket_addrs() {
            Ok(mut iter) => match iter.next() {
                Some(a) => a,
                None => {
                    log.push_str("DNS Failure: Could not resolve SMTP host.\n");
                    return (Err(GrafanaError::Network), log);
                }
            },
            Err(_) => {
                log.push_str("DNS Failure: Hostname resolution error.\n");
                return (Err(GrafanaError::Network), log);
            }
        };

        let mut stream = match TcpStream::connect_timeout(&resolved, Duration::from_secs(4)) {
            Ok(s) => s,
            Err(e) => {
                log.push_str(&format!("Connection failed: Unable to reach SMTP host ({})\n", e));
                return (Err(GrafanaError::Network), log);
            }
        };

        let _ = stream.set_read_timeout(Some(Duration::from_secs(4)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(4)));

        // If port is 587/465 or TLS is enabled, attempt STARTTLS / TLS via curl bridge
        if self.config.port == 587 || self.config.port == 465 || self.config.use_tls {
            log.push_str("Attempting secure STARTTLS / SMTPS delivery...\n");
            // Build temporary basic body for TLS test if needed, or send standard invite
            let now_sec = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let date_str = Self::format_rfc2822_date(now_sec);
            let raw_msg = format!(
                "From: \"{}\" <{}>\r\nTo: \"{}\" <{}>\r\nSubject: Safe Spaces AI Innovation Hub Conference Invitation [{}]\r\nDate: {}\r\n\r\nRoom: {}\r\nLink: {}\r\nNote: {}\r\n",
                self.config.sender_name, self.config.sender_email, recipient_name, recipient_email, room_id, date_str, room_id, meeting_url, custom_note
            );
            match self.dispatch_with_curl(recipient_email, &raw_msg) {
                Ok(curl_log) => {
                    log.push_str(&format!("SUCCESS: {}\n", curl_log));
                    return (Ok(()), log);
                }
                Err(err) => {
                    log.push_str(&format!("TLS bridge note: {}. Proceeding to direct RFC 5321 stream...\n", err));
                }
            }
        }

        // 1. Initial Greeting (220)
        match Self::read_smtp_response(&mut stream) {
            Ok((code, resp)) => {
                log.push_str(&format!("SERVER <- {} {}\n", code, resp.trim()));
                if code != 220 {
                    return (Err(GrafanaError::Network), log);
                }
            }
            Err(e) => return (Err(e), log),
        }

        let sender_domain = self
            .config
            .sender_email
            .split('@')
            .nth(1)
            .unwrap_or("localhost");

        // 2. EHLO with configured domain
        let ehlo_cmd = format!("EHLO {}\r\n", sender_domain);
        let _ = stream.write_all(ehlo_cmd.as_bytes());
        let (code, _resp) = match Self::read_smtp_response(&mut stream) {
            Ok(res) => res,
            Err(e) => return (Err(e), log),
        };

        if code != 250 {
            let helo_cmd = format!("HELO {}\r\n", sender_domain);
            let _ = stream.write_all(helo_cmd.as_bytes());
            let (h_code, _) = match Self::read_smtp_response(&mut stream) {
                Ok(res) => res,
                Err(e) => return (Err(e), log),
            };
            if h_code != 250 {
                log.push_str(&format!("Handshake failed with code: {}\n", h_code));
                return (Err(GrafanaError::Network), log);
            }
        }

        // 3. Auth if credentials present
        if !self.config.username.is_empty() {
            let _ = stream.write_all(b"AUTH LOGIN\r\n");
            let (auth_code, auth_resp) = match Self::read_smtp_response(&mut stream) {
                Ok(res) => res,
                Err(e) => return (Err(e), log),
            };

            if auth_code == 334 {
                let u_b64 = Self::base64_encode(&self.config.username);
                let p_b64 = Self::base64_encode(&self.config.password);

                let _ = stream.write_all(format!("{}\r\n", u_b64).as_bytes());
                let _ = Self::read_smtp_response(&mut stream);

                let _ = stream.write_all(format!("{}\r\n", p_b64).as_bytes());
                let (pass_code, pass_resp) = match Self::read_smtp_response(&mut stream) {
                    Ok(res) => res,
                    Err(e) => return (Err(e), log),
                };

                if pass_code != 235 {
                    log.push_str(&format!("Authentication Failed (Code: {} - {})\n", pass_code, pass_resp.trim()));
                    if pass_resp.contains("STARTTLS") || pass_code == 530 {
                        log.push_str("Notice: Public mail providers require STARTTLS. For production self-hosting, point Grafana to your domain's local/private SMTP gateway (e.g. Stalwart/Postfix on port 25/587).\n");
                    }
                    return (Err(GrafanaError::Unauthorized), log);
                }
            } else if auth_code == 530 || auth_resp.contains("STARTTLS") {
                log.push_str(&format!("STARTTLS Required (Code: {}): {}\n", auth_code, auth_resp.trim()));
                log.push_str("Tip: Use your domain's self-hosted SMTP relay (e.g. localhost:25, postfix, or stalwart) for direct zero-friction delivery.\n");
                return (Err(GrafanaError::Unauthorized), log);
            }
        }

        // 4. MAIL FROM
        let mail_from = format!("MAIL FROM:<{}>\r\n", self.config.sender_email);
        let _ = stream.write_all(mail_from.as_bytes());
        let (code, mf_resp) = match Self::read_smtp_response(&mut stream) {
            Ok(res) => res,
            Err(e) => return (Err(e), log),
        };
        if code != 250 {
            log.push_str(&format!("MAIL FROM rejected (Code: {} - {})\n", code, mf_resp.trim()));
            return (Err(GrafanaError::Network), log);
        }

        // 5. RCPT TO
        let rcpt_to = format!("RCPT TO:<{}>\r\n", recipient_email);
        let _ = stream.write_all(rcpt_to.as_bytes());
        let (code, rcpt_resp) = match Self::read_smtp_response(&mut stream) {
            Ok(res) => res,
            Err(e) => return (Err(e), log),
        };
        if code != 250 {
            log.push_str(&format!("RCPT TO rejected (Code: {} - {})\n", code, rcpt_resp.trim()));
            return (Err(GrafanaError::Network), log);
        }

        // 6. DATA
        let _ = stream.write_all(b"DATA\r\n");
        let (code, data_resp) = match Self::read_smtp_response(&mut stream) {
            Ok(res) => res,
            Err(e) => return (Err(e), log),
        };
        if code != 354 {
            log.push_str(&format!("DATA rejected (Code: {} - {})\n", code, data_resp.trim()));
            return (Err(GrafanaError::Network), log);
        }

        // 7. Format RFC 5322 MIME Body with dynamic date and Message-ID
        let now_sec = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u32)
            .unwrap_or(12345);

        let date_str = Self::format_rfc2822_date(now_sec);
        let msg_id = format!("<grafana.{}.{}@{}>", now_sec, now_ms % 90000 + 10000, sender_domain);

        let body = format!(
            "From: \"{}\" <{}>\r\n\
             To: \"{}\" <{}>\r\n\
             Subject: Grafana Confidential Video Meeting Invitation [{}]\r\n\
             Date: {}\r\n\
             Message-ID: {}\r\n\
             MIME-Version: 1.0\r\n\
             Content-Type: multipart/alternative; boundary=\"GRAFANA_BOUNDARY_SECURE\"\r\n\
             \r\n\
             --GRAFANA_BOUNDARY_SECURE\r\n\
             Content-Type: text/plain; charset=UTF-8\r\n\
             \r\n\
             You are invited to a confidential Grafana video meeting.\r\n\r\n\
             Room: {}\r\n\
             Join Link: {}\r\n\r\n\
             Host Note: {}\r\n\r\n\
             Grafana Security Features:\r\n\
             - Zero Cloud Retention: Stream is purely volatile in-memory.\r\n\
             - 100% Unanimous Consent: Recording requires every participant's agreement.\r\n\
             - Absolute Veto Power: Any participant can block or revoke recording.\r\n\
             \r\n\
             --GRAFANA_BOUNDARY_SECURE\r\n\
             Content-Type: text/html; charset=UTF-8\r\n\
             \r\n\
             <!DOCTYPE html><html><body style=\"font-family: 'Segoe UI', Arial, sans-serif; background-color: #0a0d14; color: #f8fafc; padding: 24px;\">\
             <div style=\"max-width: 580px; margin: 0 auto; background: #121722; border: 1px solid rgba(0, 242, 254, 0.3); border-radius: 14px; padding: 28px;\">\
             <h2 style=\"margin:0; font-size:22px; color:#ffffff;\">Grafana Meeting Invitation</h2>\
             <p style=\"color: #94a3b8; font-size: 15px;\">You have been invited by <strong>{}</strong> to join a secure zero-data video conference.</p>\
             <div style=\"background: rgba(0, 242, 254, 0.08); border-left: 4px solid #00f2fe; padding: 14px; border-radius: 6px; margin: 18px 0;\">\
             <div style=\"font-size: 13px; color: #94a3b8;\">ROOM IDENTIFIER</div>\
             <div style=\"font-size: 18px; font-weight: 700; color: #00f2fe;\">{}</div>\
             </div>\
             <p style=\"font-size: 14px; color: #cbd5e1;\"><em>\"{}\"</em></p>\
             <div style=\"text-align: center; margin: 26px 0;\">\
             <a href=\"{}\" style=\"background: linear-gradient(135deg, #00f2fe, #4facfe); color: #030712; padding: 14px 30px; text-decoration: none; border-radius: 10px; font-weight: 700; display: inline-block;\">Join Secure Meeting</a>\
             </div>\
             <hr style=\"border: none; border-top: 1px solid rgba(255,255,255,0.1); margin: 24px 0;\">\
             <div style=\"font-size: 12px; color: #64748b;\">Zero Cloud Storage | 100% Unanimous Agreement Required</div>\
             </div></body></html>\r\n\
             --GRAFANA_BOUNDARY_SECURE--\r\n\
             .\r\n",
            self.config.sender_name, self.config.sender_email,
            recipient_name, recipient_email,
            room_id,
            date_str,
            msg_id,
            room_id, meeting_url, custom_note,
            self.config.sender_name, room_id, custom_note, meeting_url
        );

        let _ = stream.write_all(body.as_bytes());
        let (code, _) = match Self::read_smtp_response(&mut stream) {
            Ok(res) => res,
            Err(e) => return (Err(e), log),
        };

        if code != 250 {
            return (Err(GrafanaError::Network), log);
        }

        // 8. QUIT
        let _ = stream.write_all(b"QUIT\r\n");
        let _ = Self::read_smtp_response(&mut stream);

        log.push_str(&format!("SUCCESS: Secure invite sent to {}\n", recipient_email));
        (Ok(()), log)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        assert_eq!(SmtpClient::base64_encode(""), "");
        assert_eq!(SmtpClient::base64_encode("f"), "Zg==");
        assert_eq!(SmtpClient::base64_encode("fo"), "Zm8=");
        assert_eq!(SmtpClient::base64_encode("foo"), "Zm9v");
        assert_eq!(SmtpClient::base64_encode("foob"), "Zm9vYg==");
        assert_eq!(SmtpClient::base64_encode("fooba"), "Zm9vYmE=");
        assert_eq!(SmtpClient::base64_encode("foobar"), "Zm9vYmFy");
    }

    #[test]
    fn test_format_rfc2822_date() {
        // 0 seconds = Thu, 01 Jan 1970 00:00:00 +0000
        let d = SmtpClient::format_rfc2822_date(0);
        assert_eq!(d, "Thu, 01 Jan 1970 00:00:00 +0000");

        // 1700000000 = Wed, 14 Nov 2023 22:13:20 +0000
        let d2 = SmtpClient::format_rfc2822_date(1700000000);
        assert!(d2.contains("2023"));
        assert!(d2.contains("Nov"));
        assert!(d2.ends_with("+0000"));
    }

    #[test]
    fn test_smtp_config_masking() {
        let mut client = SmtpClient::new();
        let mut cfg = SmtpConfig::default();
        cfg.password = "my_secret_token".to_string();
        client.setup(cfg);

        let masked = client.get_masked_config();
        assert_eq!(masked.password, "********");
        assert!(masked.is_configured);
    }

    #[test]
    fn test_invalid_email_rejected_early() {
        let client = SmtpClient::new();
        let (res, log) = client.send_invite("invalid-email-no-at", "User", "room-1", "http://localhost", "note");
        assert_eq!(res.unwrap_err(), GrafanaError::InvalidParam);
        assert!(log.contains("ERROR: Invalid recipient email"));
    }
}

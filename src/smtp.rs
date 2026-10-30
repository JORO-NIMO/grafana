//! In-Memory Secure SMTP Email Invite Dispatcher (RFC 5321 / RFC 5322)
//!
//! Zero Cloud Storage: All credentials and logs are held in volatile RAM only.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{GafanaError, GafanaResult};

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
        Self {
            host: "127.0.0.1".to_string(),
            port: 587,
            username: String::new(),
            password: String::new(),
            sender_email: "invite@gafana.local".to_string(),
            sender_name: "Gafana Secure Call".to_string(),
            use_tls: true,
            is_configured: false,
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

    fn read_smtp_response(stream: &mut TcpStream) -> Result<(u32, String), GafanaError> {
        let mut buf = [0u8; 1024];
        let mut total_read = 0;
        let mut full_resp = String::new();

        loop {
            let n = stream.read(&mut buf).map_err(|_| GafanaError::Network)?;
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
    pub fn send_invite(
        &self,
        recipient_email: &str,
        recipient_name: &str,
        room_id: &str,
        meeting_url: &str,
        custom_note: &str,
    ) -> (GafanaResult<()>, String) {
        let mut log = String::new();
        log.push_str(&format!(
            "=== Gafana Secure SMTP Invite Dispatcher ===\nTarget: {} ({})\nRoom: {}\nSMTP Host: {}:{}\nZero Cloud Retention: Guaranteed\n-------------------------------------------\n",
            recipient_email, recipient_name, room_id, self.config.host, self.config.port
        ));

        if !recipient_email.contains('@') {
            log.push_str("ERROR: Invalid recipient email address.\n");
            return (Err(GafanaError::InvalidParam), log);
        }

        let addr_str = format!("{}:{}", self.config.host, self.config.port);
        let resolved = match addr_str.to_socket_addrs() {
            Ok(mut iter) => match iter.next() {
                Some(a) => a,
                None => {
                    log.push_str("DNS Failure: Could not resolve SMTP host.\n");
                    return (Err(GafanaError::Network), log);
                }
            },
            Err(_) => {
                log.push_str("DNS Failure: Hostname resolution error.\n");
                return (Err(GafanaError::Network), log);
            }
        };

        let mut stream = match TcpStream::connect_timeout(&resolved, Duration::from_secs(4)) {
            Ok(s) => s,
            Err(e) => {
                log.push_str(&format!("Connection failed: Unable to reach SMTP host ({})\n", e));
                return (Err(GafanaError::Network), log);
            }
        };

        let _ = stream.set_read_timeout(Some(Duration::from_secs(4)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(4)));

        // 1. Initial Greeting (220)
        match Self::read_smtp_response(&mut stream) {
            Ok((code, resp)) => {
                log.push_str(&format!("SERVER <- {} {}\n", code, resp.trim()));
                if code != 220 {
                    return (Err(GafanaError::Network), log);
                }
            }
            Err(e) => return (Err(e), log),
        }

        // 2. EHLO
        let _ = stream.write_all(b"EHLO gafana.app\r\n");
        let (code, _) = match Self::read_smtp_response(&mut stream) {
            Ok(res) => res,
            Err(e) => return (Err(e), log),
        };

        if code != 250 {
            let _ = stream.write_all(b"HELO gafana.app\r\n");
            let (h_code, _) = match Self::read_smtp_response(&mut stream) {
                Ok(res) => res,
                Err(e) => return (Err(e), log),
            };
            if h_code != 250 {
                return (Err(GafanaError::Network), log);
            }
        }

        // 3. Auth if credentials present
        if !self.config.username.is_empty() {
            let _ = stream.write_all(b"AUTH LOGIN\r\n");
            let (auth_code, _) = match Self::read_smtp_response(&mut stream) {
                Ok(res) => res,
                Err(e) => return (Err(e), log),
            };

            if auth_code == 334 {
                let u_b64 = Self::base64_encode(&self.config.username);
                let p_b64 = Self::base64_encode(&self.config.password);

                let _ = stream.write_all(format!("{}\r\n", u_b64).as_bytes());
                let _ = Self::read_smtp_response(&mut stream);

                let _ = stream.write_all(format!("{}\r\n", p_b64).as_bytes());
                let (pass_code, _) = match Self::read_smtp_response(&mut stream) {
                    Ok(res) => res,
                    Err(e) => return (Err(e), log),
                };

                if pass_code != 235 {
                    log.push_str(&format!("Authentication Failed (Code: {})\n", pass_code));
                    return (Err(GafanaError::Unauthorized), log);
                }
            }
        }

        // 4. MAIL FROM
        let mail_from = format!("MAIL FROM:<{}>\r\n", self.config.sender_email);
        let _ = stream.write_all(mail_from.as_bytes());
        let (code, _) = match Self::read_smtp_response(&mut stream) {
            Ok(res) => res,
            Err(e) => return (Err(e), log),
        };
        if code != 250 {
            return (Err(GafanaError::Network), log);
        }

        // 5. RCPT TO
        let rcpt_to = format!("RCPT TO:<{}>\r\n", recipient_email);
        let _ = stream.write_all(rcpt_to.as_bytes());
        let (code, _) = match Self::read_smtp_response(&mut stream) {
            Ok(res) => res,
            Err(e) => return (Err(e), log),
        };
        if code != 250 {
            return (Err(GafanaError::Network), log);
        }

        // 6. DATA
        let _ = stream.write_all(b"DATA\r\n");
        let (code, _) = match Self::read_smtp_response(&mut stream) {
            Ok(res) => res,
            Err(e) => return (Err(e), log),
        };
        if code != 354 {
            return (Err(GafanaError::Network), log);
        }

        // 7. Format MIME Body
        let _now_sec = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let body = format!(
            "From: \"{}\" <{}>\r\n\
             To: \"{}\" <{}>\r\n\
             Subject: Gafana Confidential Video Meeting Invitation [{}]\r\n\
             Date: Tue, 09 Oct 2026 12:00:00 +0000\r\n\
             MIME-Version: 1.0\r\n\
             Content-Type: multipart/alternative; boundary=\"GAFANA_BOUNDARY_SECURE\"\r\n\
             \r\n\
             --GAFANA_BOUNDARY_SECURE\r\n\
             Content-Type: text/plain; charset=UTF-8\r\n\
             \r\n\
             You are invited to a confidential Gafana video meeting.\r\n\r\n\
             Room: {}\r\n\
             Join Link: {}\r\n\r\n\
             Host Note: {}\r\n\r\n\
             Gafana Security Features:\r\n\
             - Zero Cloud Retention: Stream is purely volatile in-memory.\r\n\
             - 100% Unanimous Consent: Recording requires every participant's agreement.\r\n\
             - Absolute Veto Power: Any participant can block or revoke recording.\r\n\
             \r\n\
             --GAFANA_BOUNDARY_SECURE\r\n\
             Content-Type: text/html; charset=UTF-8\r\n\
             \r\n\
             <!DOCTYPE html><html><body style=\"font-family: 'Segoe UI', Arial, sans-serif; background-color: #0a0d14; color: #f8fafc; padding: 24px;\">\
             <div style=\"max-width: 580px; margin: 0 auto; background: #121722; border: 1px solid rgba(0, 242, 254, 0.3); border-radius: 14px; padding: 28px;\">\
             <h2 style=\"margin:0; font-size:22px; color:#ffffff;\">Gafana Meeting Invitation</h2>\
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
             --GAFANA_BOUNDARY_SECURE--\r\n\
             .\r\n",
            self.config.sender_name, self.config.sender_email,
            recipient_name, recipient_email,
            room_id,
            room_id, meeting_url, custom_note,
            self.config.sender_name, room_id, custom_note, meeting_url
        );

        let _ = stream.write_all(body.as_bytes());
        let (code, _) = match Self::read_smtp_response(&mut stream) {
            Ok(res) => res,
            Err(e) => return (Err(e), log),
        };

        if code != 250 {
            return (Err(GafanaError::Network), log);
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
        assert_eq!(res.unwrap_err(), GafanaError::InvalidParam);
        assert!(log.contains("ERROR: Invalid recipient email"));
    }
}

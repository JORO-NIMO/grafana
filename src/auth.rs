//! User Authentication, Email Verification & Voluntary Cloud Sync (filefran.safespaceshub.com)
//!
//! Enforces:
//! 1. Host Authentication: Username, Email & Salted Password with verification codes.
//! 2. Local-First Persistence: Secure local disk ledger (`data/grafana_vault.json`) with atomic writes.
//! 3. Voluntary Cloud Sync: Explicit opt-in backup pushed to `filefran.safespaceshub.com`.
//! 4. Admin Monitoring: Admin dashboard to view opted-in user backups and system health.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{GrafanaError, GrafanaResult};

// =========================================================================
// Pure Rust Standard SHA-256 (FIPS 180-4) Implementation
// Zero external dependencies.
// =========================================================================

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

pub fn sha256_hex(data: &[u8]) -> String {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut h_val = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h_val
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h_val = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(h_val);
    }

    let mut hex_str = String::with_capacity(64);
    for val in h {
        hex_str.push_str(&format!("{:08x}", val));
    }
    hex_str
}

pub fn hash_password_salted(password: &str, salt: &str) -> String {
    let combined = format!("{}:{}", salt, password);
    sha256_hex(combined.as_bytes())
}

pub fn generate_salt() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(123456789);
    let seed = format!("{:x}:salt:{}:grafana", now, std::process::id());
    sha256_hex(seed.as_bytes())[..16].to_string()
}

// =========================================================================
// Data Models
// =========================================================================

#[derive(Debug, Clone)]
pub struct UserAccount {
    pub username: String,
    pub email: String,
    pub salt: String,
    pub password_hash: String,
    pub is_verified: bool,
    pub verification_code: String,
    pub verification_expires: u64,
    pub is_admin: bool,
    pub created_at: u64,
    pub allow_cloud_sync: bool,
    pub sync_destination: String,
    pub backup_data: Option<String>,
    pub last_sync_at: u64,
}

#[derive(Debug, Clone)]
pub struct BackupAuditEntry {
    pub username: String,
    pub email: String,
    pub destination: String,
    pub payload_size: usize,
    pub timestamp: u64,
}

pub struct AuthRegistry {
    pub vault_path: Option<String>,
    users: HashMap<String, UserAccount>,     // Key: lowercase email
    usernames: HashMap<String, String>,     // Key: lowercase username -> lowercase email
    sessions: HashMap<String, String>,      // Key: session_token -> lowercase email
    backup_logs: Vec<BackupAuditEntry>,
}

impl Default for AuthRegistry {
    fn default() -> Self {
        let default_vault = std::env::var("GRAFANA_VAULT_PATH")
            .unwrap_or_else(|_| "data/grafana_vault.json".to_string());
        Self::with_vault_path(Some(default_vault))
    }
}

impl AuthRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_vault_path(vault_path: Option<String>) -> Self {
        let mut reg = Self {
            vault_path,
            users: HashMap::new(),
            usernames: HashMap::new(),
            sessions: HashMap::new(),
            backup_logs: Vec::new(),
        };

        // Attempt to load existing ledger from disk
        let loaded = reg.load_from_disk().unwrap_or(0);

        // If no records found on disk, seed initial administrator account
        if loaded == 0 {
            let admin_email = "admin@safespaceshub.com".to_string();
            let admin_user = "admin".to_string();
            let admin_pass = std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "SafeSpaces2026!".to_string());
            let salt = generate_salt();
            let pass_hash = hash_password_salted(&admin_pass, &salt);

            let admin_account = UserAccount {
                username: admin_user.clone(),
                email: admin_email.clone(),
                salt,
                password_hash: pass_hash,
                is_verified: true,
                verification_code: "000000".to_string(),
                verification_expires: u64::MAX,
                is_admin: true,
                created_at: Self::now_secs(),
                allow_cloud_sync: true,
                sync_destination: "https://filefran.safespaceshub.com".to_string(),
                backup_data: Some(r#"{"source":"initial_admin","opt_in":true}"#.to_string()),
                last_sync_at: Self::now_secs(),
            };

            reg.users.insert(admin_email.clone(), admin_account);
            reg.usernames.insert(admin_user, admin_email);
            let _ = reg.save_to_disk();
        }

        reg
    }

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn generate_code() -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(123456);
        let code = ((now % 900000) + 100000) as u32;
        code.to_string()
    }

    fn generate_token(email: &str) -> String {
        let now = Self::now_secs();
        let seed = format!("{}:{}:{}", email, now, Self::generate_code());
        let hash = sha256_hex(seed.as_bytes());
        format!("tok_{}", &hash[..32])
    }

    /// Register a new user with cryptographic salt and sha256 hashing.
    pub fn register(&mut self, username: &str, email: &str, password: &str) -> GrafanaResult<(String, String)> {
        let u_clean = username.trim().to_string();
        let e_clean = email.trim().to_lowercase();
        let p_clean = password.trim();

        if u_clean.len() < 2 {
            return Err(GrafanaError::InvalidParam);
        }
        if !e_clean.contains('@') || !e_clean.contains('.') {
            return Err(GrafanaError::InvalidParam);
        }
        if p_clean.len() < 6 {
            return Err(GrafanaError::InvalidParam);
        }

        if self.users.contains_key(&e_clean) || self.usernames.contains_key(&u_clean.to_lowercase()) {
            return Err(GrafanaError::InvalidParam);
        }

        let code = Self::generate_code();
        let expires = Self::now_secs() + 1800; // 30 minutes
        let salt = generate_salt();
        let pass_hash = hash_password_salted(p_clean, &salt);

        let account = UserAccount {
            username: u_clean.clone(),
            email: e_clean.clone(),
            salt,
            password_hash: pass_hash,
            is_verified: false,
            verification_code: code.clone(),
            verification_expires: expires,
            is_admin: false,
            created_at: Self::now_secs(),
            allow_cloud_sync: false,
            sync_destination: "https://filefran.safespaceshub.com".to_string(),
            backup_data: None,
            last_sync_at: 0,
        };

        self.usernames.insert(u_clean.to_lowercase(), e_clean.clone());
        self.users.insert(e_clean.clone(), account);
        let _ = self.save_to_disk();

        Ok((e_clean, code))
    }

    /// Confirm email using the 6-digit verification code.
    pub fn verify_email(&mut self, email: &str, code: &str) -> GrafanaResult<(String, UserAccount)> {
        let e_clean = email.trim().to_lowercase();
        let c_clean = code.trim();
        let now = Self::now_secs();

        let user_clone = {
            let user = self.users.get_mut(&e_clean).ok_or(GrafanaError::PeerNotFound)?;
            if user.verification_code != c_clean {
                return Err(GrafanaError::InvalidParam);
            }
            if now > user.verification_expires {
                return Err(GrafanaError::Timeout);
            }
            user.is_verified = true;
            user.clone()
        };

        let token = Self::generate_token(&e_clean);
        self.sessions.insert(token.clone(), e_clean);
        let _ = self.save_to_disk();

        Ok((token, user_clone))
    }

    /// Login via username or email + password. Returns session token if verified.
    pub fn login(&mut self, identifier: &str, password: &str) -> GrafanaResult<(String, UserAccount)> {
        let id_clean = identifier.trim().to_lowercase();

        let email = if id_clean.contains('@') {
            id_clean
        } else {
            self.usernames.get(&id_clean).cloned().ok_or(GrafanaError::PeerNotFound)?
        };

        let user = self.users.get(&email).ok_or(GrafanaError::PeerNotFound)?;

        let valid = if user.salt.is_empty() {
            // Legacy unsalted fallback for initial admin accounts
            user.password_hash == sha256_hex(password.trim().as_bytes())
        } else {
            user.password_hash == hash_password_salted(password.trim(), &user.salt)
        };

        if !valid {
            return Err(GrafanaError::PermissionDenied);
        }

        if !user.is_verified {
            return Err(GrafanaError::ConsensusRejected);
        }

        let token = Self::generate_token(&email);
        self.sessions.insert(token.clone(), email);

        Ok((token, user.clone()))
    }

    pub fn get_user_by_token(&self, token: &str) -> Option<UserAccount> {
        let email = self.sessions.get(token.trim())?;
        self.users.get(email).cloned()
    }

    pub fn logout(&mut self, token: &str) {
        self.sessions.remove(token.trim());
    }

    /// Update voluntary cloud sync preference & backup payload for `filefran.safespaceshub.com`.
    pub fn update_cloud_sync(&mut self, email: &str, allow_sync: bool, backup_payload: Option<&str>) -> GrafanaResult<()> {
        let e_clean = email.trim().to_lowercase();
        let now = Self::now_secs();
        let mut audit_log_entry = None;

        {
            let user = self.users.get_mut(&e_clean).ok_or(GrafanaError::PeerNotFound)?;
            user.allow_cloud_sync = allow_sync;
            if allow_sync {
                if let Some(payload) = backup_payload {
                    user.backup_data = Some(payload.to_string());
                    user.last_sync_at = now;
                    audit_log_entry = Some(BackupAuditEntry {
                        username: user.username.clone(),
                        email: user.email.clone(),
                        destination: user.sync_destination.clone(),
                        payload_size: payload.len(),
                        timestamp: now,
                    });
                }
            } else {
                user.backup_data = None;
            }
        }

        if let Some(entry) = audit_log_entry {
            self.backup_logs.push(entry);
        }

        let _ = self.save_to_disk();
        Ok(())
    }

    /// Admin inspection overview: Returns registered users, verification statuses, and backup records.
    pub fn user_count(&self) -> usize {
        self.users.len()
    }

    pub fn get_admin_overview(&self) -> (Vec<UserAccount>, Vec<BackupAuditEntry>) {
        let users: Vec<UserAccount> = self.users.values().cloned().collect();
        (users, self.backup_logs.clone())
    }

    /// Atomically persist user ledger to disk.
    pub fn save_to_disk(&self) -> std::io::Result<()> {
        let path_str = match &self.vault_path {
            Some(p) => p,
            None => return Ok(()),
        };

        let p = Path::new(path_str);
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }

        let mut json = String::from("{\n  \"version\": 1,\n  \"users\": [\n");
        let mut first = true;
        for user in self.users.values() {
            if !first {
                json.push_str(",\n");
            }
            first = false;
            json.push_str(&format!(
                "    {{\n      \"username\": \"{}\",\n      \"email\": \"{}\",\n      \"salt\": \"{}\",\n      \"password_hash\": \"{}\",\n      \"is_verified\": {},\n      \"verification_code\": \"{}\",\n      \"verification_expires\": {},\n      \"is_admin\": {},\n      \"created_at\": {},\n      \"allow_cloud_sync\": {},\n      \"sync_destination\": \"{}\",\n      \"backup_data\": {},\n      \"last_sync_at\": {}\n    }}",
                escape_json_str(&user.username),
                escape_json_str(&user.email),
                escape_json_str(&user.salt),
                escape_json_str(&user.password_hash),
                user.is_verified,
                escape_json_str(&user.verification_code),
                user.verification_expires,
                user.is_admin,
                user.created_at,
                user.allow_cloud_sync,
                escape_json_str(&user.sync_destination),
                match &user.backup_data {
                    Some(b) => format!("\"{}\"", escape_json_str(b)),
                    None => "null".to_string(),
                },
                user.last_sync_at
            ));
        }
        json.push_str("\n  ],\n  \"backup_logs\": [\n");
        let mut first_log = true;
        for log in &self.backup_logs {
            if !first_log {
                json.push_str(",\n");
            }
            first_log = false;
            json.push_str(&format!(
                "    {{\n      \"username\": \"{}\",\n      \"email\": \"{}\",\n      \"destination\": \"{}\",\n      \"payload_size\": {},\n      \"timestamp\": {}\n    }}",
                escape_json_str(&log.username),
                escape_json_str(&log.email),
                escape_json_str(&log.destination),
                log.payload_size,
                log.timestamp
            ));
        }
        json.push_str("\n  ]\n}\n");

        let tmp_path = format!("{}.tmp", path_str);
        {
            let mut f = File::create(&tmp_path)?;
            f.write_all(json.as_bytes())?;
            f.flush()?;
        }
        fs::rename(tmp_path, path_str)?;
        Ok(())
    }

    /// Load user accounts and backup records from local disk.
    pub fn load_from_disk(&mut self) -> std::io::Result<usize> {
        let path_str = match &self.vault_path {
            Some(p) => p,
            None => return Ok(0),
        };

        if !Path::new(path_str).exists() {
            return Ok(0);
        }

        let mut f = File::open(path_str)?;
        let mut content = String::new();
        f.read_to_string(&mut content)?;

        let mut count = 0;

        // Parse users array
        if let Some(users_idx) = content.find("\"users\":") {
            let after_users = &content[users_idx + 8..];
            if let Some(arr_start) = after_users.find('[') {
                let user_objs = extract_json_objects(&after_users[arr_start..]);
                for obj in user_objs {
                    let username = match extract_field_str(&obj, "username") {
                        Some(u) => u,
                        None => continue,
                    };
                    let email = match extract_field_str(&obj, "email") {
                        Some(e) => e,
                        None => continue,
                    };
                    let salt = extract_field_str(&obj, "salt").unwrap_or_default();
                    let password_hash = match extract_field_str(&obj, "password_hash") {
                        Some(h) => h,
                        None => continue,
                    };
                    let is_verified = extract_field_str(&obj, "is_verified").map(|v| v == "true").unwrap_or(false);
                    let verification_code = extract_field_str(&obj, "verification_code").unwrap_or_default();
                    let verification_expires = extract_field_str(&obj, "verification_expires").and_then(|v| v.parse().ok()).unwrap_or(0);
                    let is_admin = extract_field_str(&obj, "is_admin").map(|v| v == "true").unwrap_or(false);
                    let created_at = extract_field_str(&obj, "created_at").and_then(|v| v.parse().ok()).unwrap_or(0);
                    let allow_cloud_sync = extract_field_str(&obj, "allow_cloud_sync").map(|v| v == "true").unwrap_or(false);
                    let sync_destination = extract_field_str(&obj, "sync_destination").unwrap_or_else(|| "https://filefran.safespaceshub.com".to_string());
                    let backup_data = extract_field_str(&obj, "backup_data");
                    let last_sync_at = extract_field_str(&obj, "last_sync_at").and_then(|v| v.parse().ok()).unwrap_or(0);

                    let account = UserAccount {
                        username: username.clone(),
                        email: email.clone(),
                        salt,
                        password_hash,
                        is_verified,
                        verification_code,
                        verification_expires,
                        is_admin,
                        created_at,
                        allow_cloud_sync,
                        sync_destination,
                        backup_data,
                        last_sync_at,
                    };

                    self.usernames.insert(username.to_lowercase(), email.clone());
                    self.users.insert(email, account);
                    count += 1;
                }
            }
        }

        // Parse backup logs array
        if let Some(logs_idx) = content.find("\"backup_logs\":") {
            let after_logs = &content[logs_idx + 14..];
            if let Some(arr_start) = after_logs.find('[') {
                let log_objs = extract_json_objects(&after_logs[arr_start..]);
                for obj in log_objs {
                    if let (Some(u), Some(e), Some(d), Some(s), Some(t)) = (
                        extract_field_str(&obj, "username"),
                        extract_field_str(&obj, "email"),
                        extract_field_str(&obj, "destination"),
                        extract_field_str(&obj, "payload_size").and_then(|v| v.parse().ok()),
                        extract_field_str(&obj, "timestamp").and_then(|v| v.parse().ok()),
                    ) {
                        self.backup_logs.push(BackupAuditEntry {
                            username: u,
                            email: e,
                            destination: d,
                            payload_size: s,
                            timestamp: t,
                        });
                    }
                }
            }
        }

        Ok(count)
    }
}

// =========================================================================
// Pure Rust JSON Serialization & Parsing Utilities (Zero Crates)
// =========================================================================

fn escape_json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

fn unescape_json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn extract_field_str(json: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{}\"", field);
    let key_idx = json.find(&pattern)?;
    let after_key = &json[key_idx + pattern.len()..];
    let colon_idx = after_key.find(':')?;
    let val_part = after_key[colon_idx + 1..].trim_start();

    if val_part.starts_with('"') {
        let rest = &val_part[1..];
        let mut escaped = false;
        let mut end_idx = None;
        for (i, c) in rest.char_indices() {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                end_idx = Some(i);
                break;
            }
        }
        end_idx.map(|idx| unescape_json_str(&rest[..idx]))
    } else if val_part.starts_with("null") {
        None
    } else {
        let end = val_part
            .find(|c: char| c == ',' || c == '}' || c == ']' || c.is_whitespace())
            .unwrap_or(val_part.len());
        let slice = val_part[..end].trim();
        if slice.is_empty() {
            None
        } else {
            Some(slice.to_string())
        }
    }
}

fn extract_json_objects(array_json: &str) -> Vec<String> {
    let mut objs = Vec::new();
    let mut depth = 0;
    let mut start = None;
    let mut in_string = false;
    let mut escaped = false;

    for (i, c) in array_json.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' && in_string {
            escaped = true;
            continue;
        }
        if c == '"' {
            in_string = !in_string;
            continue;
        }
        if !in_string {
            if c == '{' {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            } else if c == '}' {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(s) = start {
                            objs.push(array_json[s..=i].to_string());
                        }
                        start = None;
                    }
                }
            } else if c == ']' && depth == 0 {
                break;
            }
        }
    }
    objs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_vector() {
        // Standard NIST test vectors
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_register_verify_login_lifecycle() {
        let mut reg = AuthRegistry::with_vault_path(None);

        // 1. Register
        let (email, code) = reg.register("testuser", "test@domain.com", "secret123").unwrap();
        assert_eq!(email, "test@domain.com");
        assert_eq!(code.len(), 6);

        // 2. Login before verify must fail
        let err = reg.login("testuser", "secret123").unwrap_err();
        assert_eq!(err, GrafanaError::ConsensusRejected);

        // 3. Verify
        let (token, user) = reg.verify_email("test@domain.com", &code).unwrap();
        assert!(token.starts_with("tok_"));
        assert!(user.is_verified);
        assert!(!user.salt.is_empty());

        // 4. Login succeeds after verification
        let (tok2, _) = reg.login("testuser", "secret123").unwrap();
        assert!(tok2.starts_with("tok_"));

        // 5. Cloud sync opt-in test
        reg.update_cloud_sync("test@domain.com", true, Some(r#"{"history":["call1"]}"#)).unwrap();
        let u = reg.get_user_by_token(&tok2).unwrap();
        assert!(u.allow_cloud_sync);
        assert!(u.backup_data.is_some());
    }

    #[test]
    fn test_disk_persistence_and_reload() {
        let test_vault_path = "target/test_vault.json".to_string();
        let _ = fs::remove_file(&test_vault_path);

        // 1. Create registry with test vault path
        let mut reg1 = AuthRegistry::with_vault_path(Some(test_vault_path.clone()));
        let (_, code) = reg1.register("persistent_user", "persist@safespaceshub.com", "MyPassword99").unwrap();
        reg1.verify_email("persist@safespaceshub.com", &code).unwrap();
        reg1.update_cloud_sync("persist@safespaceshub.com", true, Some(r#"{"preferences":{"mute_on_join":true}}"#)).unwrap();
        reg1.save_to_disk().unwrap();

        // 2. Create second fresh registry pointing to the same file
        let mut reg2 = AuthRegistry::with_vault_path(Some(test_vault_path.clone()));

        // 3. Verify user was restored from disk and can log in with salted password
        let (token, user) = reg2.login("persistent_user", "MyPassword99").unwrap();
        assert_eq!(user.email, "persist@safespaceshub.com");
        assert!(user.is_verified);
        assert!(user.allow_cloud_sync);
        assert!(user.backup_data.is_some());
        assert!(token.starts_with("tok_"));

        // Cleanup
        let _ = fs::remove_file(&test_vault_path);
    }
}

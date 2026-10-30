//! Peer Node and Room Registry

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    pub peer_id: String,
    pub display_name: String,
    pub is_speaking: bool,
    pub has_video: bool,
    pub has_screen: bool,
    pub has_presentation: bool,
    pub has_granted_recording: bool,
    pub join_timestamp_sec: u32,
    pub last_heartbeat_ms: u32,
}

impl PeerInfo {
    pub fn new(peer_id: &str, display_name: &str) -> Self {
        let now_sec = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as u32)
            .unwrap_or(0);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u32)
            .unwrap_or(0);

        Self {
            peer_id: peer_id.to_string(),
            display_name: display_name.to_string(),
            is_speaking: false,
            has_video: false,
            has_screen: false,
            has_presentation: false,
            has_granted_recording: false,
            join_timestamp_sec: now_sec,
            last_heartbeat_ms: now_ms,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PeerNode {
    pub info: PeerInfo,
    pub active: bool,
    pub last_seen_ms: u32,
    pub has_voted: bool,
    pub vote_accept: bool,
}

pub struct PeerRegistry {
    peers: HashMap<String, PeerNode>,
    self_peer_id: String,
}

impl PeerRegistry {
    pub fn new(self_peer_id: &str, self_display_name: &str) -> Self {
        let mut reg = Self {
            peers: HashMap::new(),
            self_peer_id: self_peer_id.to_string(),
        };

        let self_node = PeerNode {
            info: PeerInfo::new(self_peer_id, self_display_name),
            active: true,
            last_seen_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u32)
                .unwrap_or(0),
            has_voted: false,
            vote_accept: false,
        };
        reg.peers.insert(self_peer_id.to_string(), self_node);
        reg
    }

    pub fn insert_or_update(&mut self, peer_id: &str, display_name: &str) -> (bool, PeerInfo) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u32)
            .unwrap_or(0);

        if let Some(node) = self.peers.get_mut(peer_id) {
            node.last_seen_ms = now_ms;
            node.active = true;
            (false, node.info.clone())
        } else {
            let mut info = PeerInfo::new(peer_id, display_name);
            info.last_heartbeat_ms = now_ms;
            let node = PeerNode {
                info: info.clone(),
                active: true,
                last_seen_ms: now_ms,
                has_voted: false,
                vote_accept: false,
            };
            self.peers.insert(peer_id.to_string(), node);
            (true, info)
        }
    }

    pub fn touch_heartbeat(&mut self, peer_id: &str) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u32)
            .unwrap_or(0);
        if let Some(node) = self.peers.get_mut(peer_id) {
            node.last_seen_ms = now_ms;
        }
    }

    pub fn remove(&mut self, peer_id: &str) -> bool {
        if peer_id == self.self_peer_id {
            return false;
        }
        if let Some(node) = self.peers.get_mut(peer_id) {
            let was_active = node.active;
            node.active = false;
            self.peers.remove(peer_id);
            was_active
        } else {
            false
        }
    }

    pub fn active_count(&self) -> usize {
        self.peers.values().filter(|p| p.active).count()
    }

    pub fn list_active(&self) -> Vec<PeerInfo> {
        self.peers
            .values()
            .filter(|p| p.active)
            .map(|p| p.info.clone())
            .collect()
    }

    /// Prune peers that have missed heartbeats for over `timeout_ms` (default: 15,000 ms).
    pub fn prune_timeouts(&mut self, timeout_ms: u32) -> Vec<String> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u32)
            .unwrap_or(0);

        let mut timed_out = Vec::new();

        for (pid, node) in self.peers.iter_mut() {
            if pid == &self.self_peer_id {
                continue;
            }
            if node.active && (now_ms.saturating_sub(node.last_seen_ms) >= timeout_ms) {
                node.active = false;
                timed_out.push(pid.clone());
            }
        }

        for pid in &timed_out {
            self.peers.remove(pid);
        }

        timed_out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_registry_insert_and_heartbeat() {
        let mut reg = PeerRegistry::new("host_1", "Host User");
        assert_eq!(reg.active_count(), 1);

        let (inserted, info) = reg.insert_or_update("peer_bob", "Bob");
        assert!(inserted);
        assert_eq!(info.peer_id, "peer_bob");
        assert_eq!(reg.active_count(), 2);

        // Update existing peer
        let (inserted_again, _) = reg.insert_or_update("peer_bob", "Bob New");
        assert!(!inserted_again);
        assert_eq!(reg.active_count(), 2);

        reg.touch_heartbeat("peer_bob");

        let active = reg.list_active();
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn test_peer_registry_remove_and_timeouts() {
        let mut reg = PeerRegistry::new("host_1", "Host User");
        reg.insert_or_update("peer_bob", "Bob");
        reg.insert_or_update("peer_charlie", "Charlie");
        assert_eq!(reg.active_count(), 3);

        // Remove bob
        assert!(reg.remove("peer_bob"));
        assert_eq!(reg.active_count(), 2);

        // Cannot remove self
        assert!(!reg.remove("host_1"));
        assert_eq!(reg.active_count(), 2);

        // Pruning with 0 ms timeout should prune Charlie but NOT self
        let pruned = reg.prune_timeouts(0);
        assert!(pruned.contains(&"peer_charlie".to_string()));
        assert_eq!(reg.active_count(), 1); // Only host_1 left
    }
}

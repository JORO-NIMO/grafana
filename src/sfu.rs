//! Stateless In-Memory SFU Relay Server (UDP)
//!
//! Zero disk writes, zero database, sub-millisecond UDP packet forwarding.

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::protocol::*;

#[derive(Debug, Clone)]
pub struct SfuClient {
    pub peer_id: String,
    pub addr: SocketAddr,
    pub last_seen: Instant,
    pub active: bool,
}

#[derive(Debug, Default)]
pub struct SfuRoom {
    pub room_id: String,
    pub clients: HashMap<String, SfuClient>,
}

pub struct StatelessSfuRelay {
    pub port: u16,
    rooms: HashMap<String, SfuRoom>,
    pub total_packets_forwarded: Arc<AtomicU64>,
    pub total_bytes_forwarded: Arc<AtomicU64>,
    pub is_running: Arc<AtomicBool>,
}

impl StatelessSfuRelay {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            rooms: HashMap::new(),
            total_packets_forwarded: Arc::new(AtomicU64::new(0)),
            total_bytes_forwarded: Arc::new(AtomicU64::new(0)),
            is_running: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Prune inactive peers (> 15 seconds silent).
    pub fn cleanup_stale_peers(&mut self) {
        let now = Instant::now();
        let timeout = Duration::from_secs(15);

        self.rooms.retain(|_, room| {
            room.clients
                .retain(|_, client| now.duration_since(client.last_seen) < timeout);
            !room.clients.is_empty()
        });
    }

    /// Run the SFU UDP server loop until `running` flag becomes false.
    pub fn run_loop(&mut self, running: Arc<AtomicBool>) -> std::io::Result<()> {
        let bind_addr = format!("0.0.0.0:{}", self.port);
        let socket = UdpSocket::bind(&bind_addr)?;
        socket.set_nonblocking(true)?;

        println!(
            "[SFU Relay] Stateless SFU listening on UDP {} (Zero-Disk Retained)",
            bind_addr
        );

        let mut buf = [0u8; 2048];
        let mut last_cleanup = Instant::now();

        while running.load(Ordering::Relaxed) {
            match socket.recv_from(&mut buf) {
                Ok((n, src)) => {
                    if n < GAFANA_HEADER_SIZE {
                        continue;
                    }

                    let hdr = match NetPacketHeader::parse(&buf[..n]) {
                        Some(h) => h,
                        None => continue,
                    };

                    if hdr.magic != GAFANA_MAGIC {
                        continue;
                    }

                    // Periodic prune every 5 seconds
                    if last_cleanup.elapsed() > Duration::from_secs(5) {
                        self.cleanup_stale_peers();
                        last_cleanup = Instant::now();
                    }

                    let room = self
                        .rooms
                        .entry(hdr.room_id.clone())
                        .or_insert_with(|| SfuRoom {
                            room_id: hdr.room_id.clone(),
                            clients: HashMap::new(),
                        });

                    // Register / touch sending peer
                    let client = room
                        .clients
                        .entry(hdr.sender_id.clone())
                        .or_insert_with(|| SfuClient {
                            peer_id: hdr.sender_id.clone(),
                            addr: src,
                            last_seen: Instant::now(),
                            active: true,
                        });
                    client.addr = src;
                    client.last_seen = Instant::now();
                    client.active = true;

                    // Fanout packet in volatile memory to all other room participants
                    for (pid, peer) in &room.clients {
                        if pid != &hdr.sender_id && peer.active {
                            let _ = socket.send_to(&buf[..n], peer.addr);
                        }
                    }

                    self.total_packets_forwarded.fetch_add(1, Ordering::Relaxed);
                    self.total_bytes_forwarded
                        .fetch_add(n as u64, Ordering::Relaxed);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(e) => {
                    eprintln!("[SFU Relay] Socket recv error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }
}

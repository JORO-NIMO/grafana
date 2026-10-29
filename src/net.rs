//! UDP Transport Engine with Non-blocking I/O and MTU Chunking

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{GafanaError, GafanaResult};
use crate::protocol::*;

/// UDP Network Transport
pub struct NetTransport {
    socket: Option<UdpSocket>,
    pub relay_addr: SocketAddr,
    pub local_port: u16,
    pub next_seq: u32,
    pub is_connected: bool,
}

impl NetTransport {
    pub fn new() -> Self {
        Self {
            socket: None,
            relay_addr: "127.0.0.1:8765".parse().unwrap(),
            local_port: 0,
            next_seq: 1,
            is_connected: false,
        }
    }

    /// Initialize non-blocking UDP socket and resolve relay address.
    pub fn init(&mut self, relay_host: &str, relay_port: u16) -> GafanaResult<()> {
        let port = if relay_port > 0 { relay_port } else { 8765 };
        let host = if relay_host.is_empty() || relay_host == "local" {
            "127.0.0.1"
        } else {
            relay_host
        };

        let target_str = format!("{}:{}", host, port);
        let resolved_addr = target_str
            .to_socket_addrs()
            .map_err(|_| GafanaError::Network)?
            .next()
            .unwrap_or_else(|| "127.0.0.1:8765".parse().unwrap());

        self.relay_addr = resolved_addr;

        // Bind ephemeral local UDP port (0)
        let sock = UdpSocket::bind("0.0.0.0:0").map_err(|_| GafanaError::Socket)?;
        sock.set_nonblocking(true).map_err(|_| GafanaError::Socket)?;
        let _ = sock.set_broadcast(true);

        self.local_port = sock.local_addr().map(|a| a.port()).unwrap_or(0);
        self.socket = Some(sock);
        self.is_connected = true;
        self.next_seq = 1;

        Ok(())
    }

    pub fn close(&mut self) {
        self.socket = None;
        self.is_connected = false;
    }

    /// Transmit payload over UDP with automatic MTU fragmentation.
    pub fn send(
        &mut self,
        room_id: &str,
        sender_id: &str,
        opcode: u8,
        payload: &[u8],
    ) -> GafanaResult<()> {
        let sock = self.socket.as_ref().ok_or(GafanaError::NotInitialized)?;
        let max_chunk_payload = GAFANA_NET_MTU - GAFANA_HEADER_SIZE;
        let payload_len = payload.len();

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u32)
            .unwrap_or(0);

        if payload_len > max_chunk_payload {
            let total_chunks = ((payload_len + max_chunk_payload - 1) / max_chunk_payload) as u16;

            for c in 0..total_chunks {
                let start = (c as usize) * max_chunk_payload;
                let end = (start + max_chunk_payload).min(payload_len);
                let chunk_slice = &payload[start..end];
                let this_chunk_len = chunk_slice.len() as u16;

                let checksum = calc_fletcher32(chunk_slice);
                let seq = self.next_seq;
                self.next_seq = self.next_seq.wrapping_add(1);

                let hdr = NetPacketHeader::new(
                    opcode,
                    seq,
                    now_ms,
                    room_id,
                    sender_id,
                    this_chunk_len,
                    c,
                    total_chunks,
                    checksum,
                );

                let mut packet = vec![0u8; GAFANA_HEADER_SIZE + chunk_slice.len()];
                hdr.write_to(&mut packet[..GAFANA_HEADER_SIZE]);
                packet[GAFANA_HEADER_SIZE..].copy_from_slice(chunk_slice);

                let _ = sock.send_to(&packet, self.relay_addr);
            }
            Ok(())
        } else {
            let checksum = if !payload.is_empty() {
                calc_fletcher32(payload)
            } else {
                0
            };

            let seq = self.next_seq;
            self.next_seq = self.next_seq.wrapping_add(1);

            let hdr = NetPacketHeader::new(
                opcode,
                seq,
                now_ms,
                room_id,
                sender_id,
                payload_len as u16,
                0,
                1,
                checksum,
            );

            let mut packet = vec![0u8; GAFANA_HEADER_SIZE + payload.len()];
            hdr.write_to(&mut packet[..GAFANA_HEADER_SIZE]);
            if !payload.is_empty() {
                packet[GAFANA_HEADER_SIZE..].copy_from_slice(payload);
            }

            sock.send_to(&packet, self.relay_addr)
                .map(|_| ())
                .map_err(|_| GafanaError::Network)
        }
    }

    /// Non-blocking receive datagram. Returns `(header, payload, sender_addr)` on success.
    pub fn recv(&self) -> Option<(NetPacketHeader, Vec<u8>, SocketAddr)> {
        let sock = self.socket.as_ref()?;
        let mut buf = [0u8; GAFANA_MAX_PACKET_SIZE];

        let (n, src) = sock.recv_from(&mut buf).ok()?;
        if n < GAFANA_HEADER_SIZE {
            return None;
        }

        let hdr = NetPacketHeader::parse(&buf[..n])?;
        let plen = hdr.payload_len as usize;

        if n < GAFANA_HEADER_SIZE + plen {
            return None;
        }

        let payload = buf[GAFANA_HEADER_SIZE..GAFANA_HEADER_SIZE + plen].to_vec();

        // Verify Fletcher-32 checksum
        if plen > 0 {
            let calc = calc_fletcher32(&payload);
            if calc != hdr.checksum {
                // Checksum mismatch -> discard corrupted packet
                return None;
            }
        }

        Some((hdr, payload, src))
    }
}

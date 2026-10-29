//! Gafana Binary Wire Protocol Implementation
//!
//! Enforces exact 1:1 packet compatibility with C `gafana_net_packet_header_t`
//! and zero-overhead binary framing.

pub const GAFANA_MAGIC: u32 = 0x47414641;
pub const GAFANA_VERSION: u8 = 1;
pub const GAFANA_MAX_ID_LEN: usize = 64;
pub const GAFANA_MAX_NAME_LEN: usize = 128;
pub const GAFANA_NET_MTU: usize = 1400;
pub const GAFANA_MAX_PACKET_SIZE: usize = 1472;
pub const GAFANA_HEADER_SIZE: usize = 154;

pub const GAFANA_OP_JOIN: u8 = 0x01;
pub const GAFANA_OP_LEAVE: u8 = 0x02;
pub const GAFANA_OP_TRACK_ANNOUNCE: u8 = 0x03;
pub const GAFANA_OP_TRACK_REMOVE: u8 = 0x04;
pub const GAFANA_OP_CONSENT_REQUEST: u8 = 0x10;
pub const GAFANA_OP_CONSENT_VOTE: u8 = 0x11;
pub const GAFANA_OP_CONSENT_DECISION: u8 = 0x12;
pub const GAFANA_OP_CONSENT_REVOKE: u8 = 0x13;
pub const GAFANA_OP_MEDIA_VIDEO: u8 = 0x20;
pub const GAFANA_OP_MEDIA_AUDIO: u8 = 0x21;
pub const GAFANA_OP_PRES_EVENT: u8 = 0x30;
pub const GAFANA_OP_HEARTBEAT: u8 = 0x40;

/// Fast Fletcher-32 Checksum Algorithm matching C implementation bit-for-bit.
pub fn calc_fletcher32(data: &[u8]) -> u32 {
    let mut sum1: u32 = 0xffff;
    let mut sum2: u32 = 0xffff;
    let mut len = data.len();
    let mut idx = 0;

    while len > 0 {
        let tlen = if len > 359 { 359 } else { len };
        len -= tlen;
        for _ in 0..tlen {
            sum1 += data[idx] as u32;
            sum2 += sum1;
            idx += 1;
        }
        sum1 = (sum1 & 0xffff) + (sum1 >> 16);
        sum2 = (sum2 & 0xffff) + (sum2 >> 16);
    }
    sum1 = (sum1 & 0xffff) + (sum1 >> 16);
    sum2 = (sum2 & 0xffff) + (sum2 >> 16);
    (sum2 << 16) | sum1
}

/// Binary Packet Header matching C `gafana_net_packet_header_t`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetPacketHeader {
    pub magic: u32,
    pub version: u8,
    pub opcode: u8,
    pub flags: u16,
    pub seq: u32,
    pub timestamp_ms: u32,
    pub room_id: String,
    pub sender_id: String,
    pub payload_len: u16,
    pub chunk_idx: u16,
    pub total_chunks: u16,
    pub checksum: u32,
}

impl NetPacketHeader {
    pub fn new(
        opcode: u8,
        seq: u32,
        timestamp_ms: u32,
        room_id: &str,
        sender_id: &str,
        payload_len: u16,
        chunk_idx: u16,
        total_chunks: u16,
        checksum: u32,
    ) -> Self {
        Self {
            magic: GAFANA_MAGIC,
            version: GAFANA_VERSION,
            opcode,
            flags: 0,
            seq,
            timestamp_ms,
            room_id: room_id.to_string(),
            sender_id: sender_id.to_string(),
            payload_len,
            chunk_idx,
            total_chunks,
            checksum,
        }
    }

    /// Serialize header to 154-byte binary slice.
    pub fn write_to(&self, out: &mut [u8]) {
        assert!(out.len() >= GAFANA_HEADER_SIZE);
        out[0..4].copy_from_slice(&self.magic.to_le_bytes());
        out[4] = self.version;
        out[5] = self.opcode;
        out[6..8].copy_from_slice(&self.flags.to_le_bytes());
        out[8..12].copy_from_slice(&self.seq.to_le_bytes());
        out[12..16].copy_from_slice(&self.timestamp_ms.to_le_bytes());

        // Fixed 64-byte room_id
        out[16..80].fill(0);
        let room_bytes = self.room_id.as_bytes();
        let rlen = room_bytes.len().min(GAFANA_MAX_ID_LEN - 1);
        out[16..16 + rlen].copy_from_slice(&room_bytes[..rlen]);

        // Fixed 64-byte sender_id
        out[80..144].fill(0);
        let sender_bytes = self.sender_id.as_bytes();
        let slen = sender_bytes.len().min(GAFANA_MAX_ID_LEN - 1);
        out[80..80 + slen].copy_from_slice(&sender_bytes[..slen]);

        out[144..146].copy_from_slice(&self.payload_len.to_le_bytes());
        out[146..148].copy_from_slice(&self.chunk_idx.to_le_bytes());
        out[148..150].copy_from_slice(&self.total_chunks.to_le_bytes());
        out[150..154].copy_from_slice(&self.checksum.to_le_bytes());
    }

    /// Parse header from binary slice.
    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < GAFANA_HEADER_SIZE {
            return None;
        }

        let magic = u32::from_le_bytes(buf[0..4].try_into().ok()?);
        if magic != GAFANA_MAGIC {
            return None;
        }

        let version = buf[4];
        let opcode = buf[5];
        let flags = u16::from_le_bytes(buf[6..8].try_into().ok()?);
        let seq = u32::from_le_bytes(buf[8..12].try_into().ok()?);
        let timestamp_ms = u32::from_le_bytes(buf[12..16].try_into().ok()?);

        let room_id_slice = &buf[16..80];
        let room_len = room_id_slice.iter().position(|&b| b == 0).unwrap_or(64);
        let room_id = String::from_utf8_lossy(&room_id_slice[..room_len]).to_string();

        let sender_id_slice = &buf[80..144];
        let sender_len = sender_id_slice.iter().position(|&b| b == 0).unwrap_or(64);
        let sender_id = String::from_utf8_lossy(&sender_id_slice[..sender_len]).to_string();

        let payload_len = u16::from_le_bytes(buf[144..146].try_into().ok()?);
        let chunk_idx = u16::from_le_bytes(buf[146..148].try_into().ok()?);
        let total_chunks = u16::from_le_bytes(buf[148..150].try_into().ok()?);
        let checksum = u32::from_le_bytes(buf[150..154].try_into().ok()?);

        Some(Self {
            magic,
            version,
            opcode,
            flags,
            seq,
            timestamp_ms,
            room_id,
            sender_id,
            payload_len,
            chunk_idx,
            total_chunks,
            checksum,
        })
    }
}

/// Consent Request Payload
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PktConsentReq {
    pub ballot_id: u32,
    pub rec_type: u8,
    pub timeout_sec: u32,
    pub requester_id: String,
}

impl PktConsentReq {
    pub const SIZE: usize = 4 + 1 + 4 + GAFANA_MAX_ID_LEN; // 73 bytes

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = vec![0u8; Self::SIZE];
        buf[0..4].copy_from_slice(&self.ballot_id.to_le_bytes());
        buf[4] = self.rec_type;
        buf[5..9].copy_from_slice(&self.timeout_sec.to_le_bytes());
        let req_bytes = self.requester_id.as_bytes();
        let len = req_bytes.len().min(GAFANA_MAX_ID_LEN - 1);
        buf[9..9 + len].copy_from_slice(&req_bytes[..len]);
        buf
    }

    pub fn deserialize(buf: &[u8]) -> Option<Self> {
        if buf.len() < Self::SIZE {
            return None;
        }
        let ballot_id = u32::from_le_bytes(buf[0..4].try_into().ok()?);
        let rec_type = buf[4];
        let timeout_sec = u32::from_le_bytes(buf[5..9].try_into().ok()?);
        let id_slice = &buf[9..9 + GAFANA_MAX_ID_LEN];
        let id_len = id_slice.iter().position(|&b| b == 0).unwrap_or(GAFANA_MAX_ID_LEN);
        let requester_id = String::from_utf8_lossy(&id_slice[..id_len]).to_string();

        Some(Self {
            ballot_id,
            rec_type,
            timeout_sec,
            requester_id,
        })
    }
}

/// Consent Vote Payload
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PktConsentVote {
    pub ballot_id: u32,
    pub vote: u8, // 1 = YES, 0 = NO
    pub voter_id: String,
}

impl PktConsentVote {
    pub const SIZE: usize = 4 + 1 + GAFANA_MAX_ID_LEN; // 69 bytes

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = vec![0u8; Self::SIZE];
        buf[0..4].copy_from_slice(&self.ballot_id.to_le_bytes());
        buf[4] = self.vote;
        let v_bytes = self.voter_id.as_bytes();
        let len = v_bytes.len().min(GAFANA_MAX_ID_LEN - 1);
        buf[5..5 + len].copy_from_slice(&v_bytes[..len]);
        buf
    }

    pub fn deserialize(buf: &[u8]) -> Option<Self> {
        if buf.len() < Self::SIZE {
            return None;
        }
        let ballot_id = u32::from_le_bytes(buf[0..4].try_into().ok()?);
        let vote = buf[4];
        let id_slice = &buf[5..5 + GAFANA_MAX_ID_LEN];
        let id_len = id_slice.iter().position(|&b| b == 0).unwrap_or(GAFANA_MAX_ID_LEN);
        let voter_id = String::from_utf8_lossy(&id_slice[..id_len]).to_string();

        Some(Self {
            ballot_id,
            vote,
            voter_id,
        })
    }
}

/// Consent Revoke Payload
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PktConsentRevoke {
    pub ballot_id: u32,
    pub revoker_id: String,
}

impl PktConsentRevoke {
    pub const SIZE: usize = 4 + GAFANA_MAX_ID_LEN; // 68 bytes

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = vec![0u8; Self::SIZE];
        buf[0..4].copy_from_slice(&self.ballot_id.to_le_bytes());
        let r_bytes = self.revoker_id.as_bytes();
        let len = r_bytes.len().min(GAFANA_MAX_ID_LEN - 1);
        buf[4..4 + len].copy_from_slice(&r_bytes[..len]);
        buf
    }

    pub fn deserialize(buf: &[u8]) -> Option<Self> {
        if buf.len() < Self::SIZE {
            return None;
        }
        let ballot_id = u32::from_le_bytes(buf[0..4].try_into().ok()?);
        let id_slice = &buf[4..4 + GAFANA_MAX_ID_LEN];
        let id_len = id_slice.iter().position(|&b| b == 0).unwrap_or(GAFANA_MAX_ID_LEN);
        let revoker_id = String::from_utf8_lossy(&id_slice[..id_len]).to_string();

        Some(Self {
            ballot_id,
            revoker_id,
        })
    }
}

/// Presentation Event Payload
#[derive(Debug, Clone, PartialEq)]
pub struct PktPresentation {
    pub event_type: u8,
    pub current_slide: u32,
    pub total_slides: u32,
    pub pointer_x: f32,
    pub pointer_y: f32,
    pub color_hex: u32,
}

impl PktPresentation {
    pub const SIZE: usize = 1 + 4 + 4 + 4 + 4 + 4; // 21 bytes

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = vec![0u8; Self::SIZE];
        buf[0] = self.event_type;
        buf[1..5].copy_from_slice(&self.current_slide.to_le_bytes());
        buf[5..9].copy_from_slice(&self.total_slides.to_le_bytes());
        buf[9..13].copy_from_slice(&self.pointer_x.to_le_bytes());
        buf[13..17].copy_from_slice(&self.pointer_y.to_le_bytes());
        buf[17..21].copy_from_slice(&self.color_hex.to_le_bytes());
        buf
    }

    pub fn deserialize(buf: &[u8]) -> Option<Self> {
        if buf.len() < Self::SIZE {
            return None;
        }
        let event_type = buf[0];
        let current_slide = u32::from_le_bytes(buf[1..5].try_into().ok()?);
        let total_slides = u32::from_le_bytes(buf[5..9].try_into().ok()?);
        let pointer_x = f32::from_le_bytes(buf[9..13].try_into().ok()?);
        let pointer_y = f32::from_le_bytes(buf[13..17].try_into().ok()?);
        let color_hex = u32::from_le_bytes(buf[17..21].try_into().ok()?);

        Some(Self {
            event_type,
            current_slide,
            total_slides,
            pointer_x,
            pointer_y,
            color_hex,
        })
    }
}

//! Core Gafana Media & Consensus Engine

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::consensus::{ConsentState, ConsensusEngine, RecordingType};
use crate::error::{GafanaError, GafanaResult};
use crate::media::*;
use crate::net::NetTransport;
use crate::peer::{PeerInfo, PeerRegistry};
use crate::protocol::*;
use crate::recorder::LocalRecorder;

#[derive(Default)]
pub struct EngineCallbacks {
    pub on_peer_joined: Option<Box<dyn Fn(&PeerInfo) + Send + Sync>>,
    pub on_peer_left: Option<Box<dyn Fn(&str, &str) + Send + Sync>>,
    pub on_track_published: Option<Box<dyn Fn(&str, TrackType, u32) + Send + Sync>>,
    pub on_track_unpublished: Option<Box<dyn Fn(&str, TrackType, u32) + Send + Sync>>,
    pub on_recording_consent_requested: Option<Box<dyn Fn(u32, &str, RecordingType) + Send + Sync>>,
    pub on_recording_vote_update: Option<Box<dyn Fn(u32, u32, u32) + Send + Sync>>,
    pub on_recording_state_changed: Option<Box<dyn Fn(u32, ConsentState, &str) + Send + Sync>>,
    pub on_presentation_event: Option<Box<dyn Fn(&str, &PresentationEvent) + Send + Sync>>,
    pub on_connection_state: Option<Box<dyn Fn(bool, &str) + Send + Sync>>,
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub relay_host: String,
    pub relay_port: u16,
    pub max_peers: u32,
    pub ballot_timeout_sec: u32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            relay_host: "127.0.0.1".to_string(),
            relay_port: 8765,
            max_peers: 4096,
            ballot_timeout_sec: 30,
        }
    }
}

pub struct GafanaEngine {
    pub config: EngineConfig,
    pub room_id: String,
    pub self_peer_id: String,
    pub self_display_name: String,
    pub in_room: bool,
    pub net: NetTransport,
    pub peers: PeerRegistry,
    pub consensus: ConsensusEngine,
    pub recorder: LocalRecorder,
    pub presentation: PresentationState,
    pub tracks: Vec<TrackNode>,
    next_track_id: AtomicU32,
    last_heartbeat_sent: Instant,
    pub callbacks: EngineCallbacks,
}

impl GafanaEngine {
    pub fn new(config: EngineConfig, callbacks: EngineCallbacks) -> Self {
        Self {
            config: config.clone(),
            room_id: String::new(),
            self_peer_id: String::new(),
            self_display_name: String::new(),
            in_room: false,
            net: NetTransport::new(),
            peers: PeerRegistry::new("", ""),
            consensus: ConsensusEngine::new(config.ballot_timeout_sec),
            recorder: LocalRecorder::new(),
            presentation: PresentationState::default(),
            tracks: Vec::new(),
            next_track_id: AtomicU32::new(1),
            last_heartbeat_sent: Instant::now(),
            callbacks,
        }
    }

    /// Join an ephemeral room.
    pub fn join_room(
        &mut self,
        room_id: &str,
        self_peer_id: &str,
        display_name: &str,
    ) -> GafanaResult<()> {
        if self.in_room {
            return Err(GafanaError::AlreadyInRoom);
        }

        self.net.init(&self.config.relay_host, self.config.relay_port)?;

        self.room_id = room_id.to_string();
        self.self_peer_id = self_peer_id.to_string();
        self.self_display_name = if !display_name.is_empty() {
            display_name.to_string()
        } else {
            format!("User_{}", self_peer_id)
        };

        self.peers = PeerRegistry::new(&self.self_peer_id, &self.self_display_name);
        self.in_room = true;
        self.last_heartbeat_sent = Instant::now();

        // Broadcast JOIN packet over wire
        let _ = self.net.send(
            &self.room_id,
            &self.self_peer_id,
            GAFANA_OP_JOIN,
            self.self_display_name.as_bytes(),
        );

        if let Some(ref cb) = self.callbacks.on_connection_state {
            cb(true, "Joined ephemeral room");
        }

        Ok(())
    }

    /// Leave room and cleanly terminate connections.
    pub fn leave_room(&mut self) -> GafanaResult<()> {
        if !self.in_room {
            return Err(GafanaError::RoomNotJoined);
        }

        if self.recorder.is_recording {
            let _ = self.recorder.stop();
        }

        let _ = self.net.send(
            &self.room_id,
            &self.self_peer_id,
            GAFANA_OP_LEAVE,
            b"UserDeparted",
        );

        self.net.close();
        self.in_room = false;
        self.room_id.clear();

        if let Some(ref cb) = self.callbacks.on_connection_state {
            cb(false, "Disconnected from room");
        }

        Ok(())
    }

    /// Poll incoming network datagrams, heartbeats, and consent timeouts.
    pub fn poll(&mut self) -> GafanaResult<()> {
        if !self.in_room {
            return Err(GafanaError::RoomNotJoined);
        }

        // 1. Process network packets
        while let Some((hdr, payload, _)) = self.net.recv() {
            if hdr.sender_id == self.self_peer_id {
                continue; // Ignore our own packets
            }

            self.handle_packet(&hdr, &payload);
        }

        // 2. Periodic heartbeat every 3 seconds
        if self.last_heartbeat_sent.elapsed().as_secs() >= 3 {
            let _ = self.net.send(
                &self.room_id,
                &self.self_peer_id,
                GAFANA_OP_HEARTBEAT,
                &[],
            );
            self.last_heartbeat_sent = Instant::now();
        }

        // 3. Check consensus timeouts
        if let Some((bid, req_id)) = self.consensus.check_timeouts() {
            if let Some(ref cb) = self.callbacks.on_recording_state_changed {
                cb(bid, ConsentState::Expired, &req_id);
            }
        }

        // 4. Prune dead peers (timeout 15 seconds)
        let timed_out = self.peers.prune_timeouts(15000);
        for pid in timed_out {
            if let Some(ref cb) = self.callbacks.on_peer_left {
                cb(&pid, "HeartbeatTimeout");
            }
        }

        Ok(())
    }

    /// Internal packet dispatcher.
    pub fn handle_packet(&mut self, hdr: &NetPacketHeader, payload: &[u8]) {
        self.peers.touch_heartbeat(&hdr.sender_id);

        match hdr.opcode {
            GAFANA_OP_HEARTBEAT => {}

            GAFANA_OP_JOIN => {
                let name = String::from_utf8_lossy(payload);
                let (is_new, info) = self.peers.insert_or_update(&hdr.sender_id, &name);

                if is_new {
                    // Privacy Enforcement: If local recording is active when a new peer joins, halt capture!
                    if self.recorder.is_recording {
                        println!(
                            "[PRIVACY ENFORCER] New peer '{}' joined during active recording. Halting capture.",
                            hdr.sender_id
                        );
                        let _ = self.recorder.stop();
                    }

                    if self.consensus.current_ballot.is_active
                        && self.consensus.current_ballot.state == ConsentState::Requested
                    {
                        self.consensus.current_ballot.total_expected_votes += 1;
                    }

                    if let Some(ref cb) = self.callbacks.on_peer_joined {
                        cb(&info);
                    }
                }
            }

            GAFANA_OP_LEAVE => {
                if self.peers.remove(&hdr.sender_id) {
                    if let Some(ref cb) = self.callbacks.on_peer_left {
                        cb(&hdr.sender_id, "UserDisconnected");
                    }
                }
            }

            GAFANA_OP_TRACK_ANNOUNCE => {
                if payload.len() >= 8 {
                    let ttype_val = u32::from_le_bytes(payload[0..4].try_into().unwrap());
                    let tid = u32::from_le_bytes(payload[4..8].try_into().unwrap());
                    let ttype = match ttype_val {
                        1 => TrackType::Audio,
                        2 => TrackType::Video,
                        3 => TrackType::Screen,
                        4 => TrackType::Presentation,
                        _ => TrackType::Audio,
                    };
                    if let Some(ref cb) = self.callbacks.on_track_published {
                        cb(&hdr.sender_id, ttype, tid);
                    }
                }
            }

            GAFANA_OP_TRACK_REMOVE => {
                if payload.len() >= 4 {
                    let tid = u32::from_le_bytes(payload[0..4].try_into().unwrap());
                    if let Some(ref cb) = self.callbacks.on_track_unpublished {
                        cb(&hdr.sender_id, TrackType::Audio, tid);
                    }
                }
            }

            GAFANA_OP_CONSENT_REQUEST => {
                if let Some(req) = PktConsentReq::deserialize(payload) {
                    let rec_type = match req.rec_type {
                        1 => RecordingType::AudioOnly,
                        2 => RecordingType::FullAv,
                        _ => RecordingType::ScreenAudio,
                    };
                    self.consensus.current_ballot.ballot_id = req.ballot_id;
                    self.consensus.current_ballot.requester_id = req.requester_id.clone();
                    self.consensus.current_ballot.rec_type = rec_type;
                    self.consensus.current_ballot.state = ConsentState::Requested;
                    self.consensus.current_ballot.is_active = true;

                    if let Some(ref cb) = self.callbacks.on_recording_consent_requested {
                        cb(req.ballot_id, &req.requester_id, rec_type);
                    }
                }
            }

            GAFANA_OP_CONSENT_VOTE => {
                if let Some(v) = PktConsentVote::deserialize(payload) {
                    let accept = v.vote != 0;
                    if let Some((new_state, unanimous)) =
                        self.consensus.cast_vote(v.ballot_id, accept)
                    {
                        let bid = self.consensus.current_ballot.ballot_id;
                        let yes = self.consensus.current_ballot.yes_votes;
                        let total = self.consensus.current_ballot.total_expected_votes;
                        let req_id = self.consensus.current_ballot.requester_id.clone();

                        if let Some(ref cb) = self.callbacks.on_recording_vote_update {
                            cb(bid, yes, total);
                        }

                        if new_state == ConsentState::Rejected {
                            if let Some(ref cb) = self.callbacks.on_recording_state_changed {
                                cb(bid, ConsentState::Rejected, &req_id);
                            }
                        } else if unanimous {
                            if let Some(ref cb) = self.callbacks.on_recording_state_changed {
                                cb(bid, ConsentState::UnanimousOk, &req_id);
                            }
                        }
                    }
                }
            }

            GAFANA_OP_CONSENT_REVOKE => {
                if let Some(rev) = PktConsentRevoke::deserialize(payload) {
                    if self.recorder.is_recording {
                        let _ = self.recorder.stop();
                    }
                    self.consensus.current_ballot.state = ConsentState::Revoked;

                    if let Some(ref cb) = self.callbacks.on_recording_state_changed {
                        cb(rev.ballot_id, ConsentState::Revoked, &rev.revoker_id);
                    }
                }
            }

            GAFANA_OP_MEDIA_VIDEO => {
                if !payload.is_empty() && self.recorder.is_recording {
                    if self.consensus.current_ballot.state != ConsentState::UnanimousOk {
                        // Anti-Capture Guard: Immediately halt recording if consensus is no longer unanimous
                        let _ = self.recorder.stop();
                        eprintln!("[ANTI-CAPTURE GUARD] Video capture halted: Unanimous consent is not active.");
                    } else {
                        let _ = self.recorder.write_video_frame(payload, 1280, 720, hdr.timestamp_ms);
                    }
                }
            }

            GAFANA_OP_MEDIA_AUDIO => {
                if payload.len() >= 2 && self.recorder.is_recording {
                    if self.consensus.current_ballot.state != ConsentState::UnanimousOk {
                        // Anti-Capture Guard: Immediately halt recording if consensus is no longer unanimous
                        let _ = self.recorder.stop();
                        eprintln!("[ANTI-CAPTURE GUARD] Audio capture halted: Unanimous consent is not active.");
                    } else {
                        let samples: Vec<i16> = payload
                            .chunks_exact(2)
                            .map(|c| i16::from_le_bytes([c[0], c[1]]))
                            .collect();
                        let _ = self.recorder.write_audio_samples(&samples);
                    }
                }
            }

            GAFANA_OP_PRES_EVENT => {
                if let Some(p) = PktPresentation::deserialize(payload) {
                    let ev_type = match p.event_type {
                        1 => PresEventType::SlideChange,
                        2 => PresEventType::PointerMove,
                        3 => PresEventType::Annotate,
                        _ => PresEventType::Stop,
                    };
                    let ev = PresentationEvent {
                        event_type: ev_type,
                        current_slide: p.current_slide,
                        total_slides: p.total_slides,
                        pointer_x: p.pointer_x,
                        pointer_y: p.pointer_y,
                        color_hex: p.color_hex,
                    };
                    if let Some(ref cb) = self.callbacks.on_presentation_event {
                        cb(&hdr.sender_id, &ev);
                    }
                }
            }

            _ => {}
        }
    }

    /// Request consent to record.
    pub fn request_recording_consent(
        &mut self,
        rec_type: RecordingType,
    ) -> GafanaResult<(u32, ConsentState)> {
        if !self.in_room {
            return Err(GafanaError::RoomNotJoined);
        }
        if self.recorder.is_recording {
            return Err(GafanaError::RecordingActive);
        }

        let total = self.peers.active_count() as u32;
        let (ballot_id, state) =
            self.consensus
                .start_ballot(&self.self_peer_id, rec_type, total);

        if state == ConsentState::Requested {
            let req_pkt = PktConsentReq {
                ballot_id,
                rec_type: rec_type as u8,
                timeout_sec: self.config.ballot_timeout_sec,
                requester_id: self.self_peer_id.clone(),
            };
            let _ = self.net.send(
                &self.room_id,
                &self.self_peer_id,
                GAFANA_OP_CONSENT_REQUEST,
                &req_pkt.serialize(),
            );
        }

        if let Some(ref cb) = self.callbacks.on_recording_vote_update {
            cb(ballot_id, 1, total.max(1));
        }

        if let Some(ref cb) = self.callbacks.on_recording_state_changed {
            cb(ballot_id, state, &self.self_peer_id);
        }

        Ok((ballot_id, state))
    }

    /// Cast vote on a pending ballot.
    pub fn cast_vote(&mut self, ballot_id: u32, accept: bool) -> GafanaResult<()> {
        if !self.in_room {
            return Err(GafanaError::RoomNotJoined);
        }

        let vote_pkt = PktConsentVote {
            ballot_id,
            vote: if accept { 1 } else { 0 },
            voter_id: self.self_peer_id.clone(),
        };
        let _ = self.net.send(
            &self.room_id,
            &self.self_peer_id,
            GAFANA_OP_CONSENT_VOTE,
            &vote_pkt.serialize(),
        );

        if let Some((state, unanimous)) = self.consensus.cast_vote(ballot_id, accept) {
            let req_id = self.consensus.current_ballot.requester_id.clone();
            let yes = self.consensus.current_ballot.yes_votes;
            let total = self.consensus.current_ballot.total_expected_votes;

            if let Some(ref cb) = self.callbacks.on_recording_vote_update {
                cb(ballot_id, yes, total);
            }

            if state == ConsentState::Rejected {
                if let Some(ref cb) = self.callbacks.on_recording_state_changed {
                    cb(ballot_id, ConsentState::Rejected, &req_id);
                }
            } else if unanimous {
                if let Some(ref cb) = self.callbacks.on_recording_state_changed {
                    cb(ballot_id, ConsentState::UnanimousOk, &req_id);
                }
            }
        }

        Ok(())
    }

    /// Start writing media stream directly to local client disk (Zero Cloud copies).
    /// STRICTLY REJECTED unless 100% unanimous approval is active.
    pub fn start_recording(&mut self, ballot_id: u32, output_file: &str) -> GafanaResult<()> {
        if !self.in_room {
            return Err(GafanaError::RoomNotJoined);
        }

        if self.consensus.current_ballot.ballot_id != ballot_id {
            return Err(GafanaError::InvalidParam);
        }

        match self.consensus.current_ballot.state {
            ConsentState::UnanimousOk => {
                let rec_type = self.consensus.current_ballot.rec_type;
                self.recorder.start(ballot_id, rec_type, output_file)?;
                Ok(())
            }
            ConsentState::Requested => Err(GafanaError::ConsentPending),
            _ => Err(GafanaError::ConsentDenied),
        }
    }

    /// Stop local recording.
    pub fn stop_recording(&mut self) -> GafanaResult<()> {
        let ballot_id = self.recorder.ballot_id;
        self.recorder.stop()?;
        self.consensus.reset();

        if let Some(ref cb) = self.callbacks.on_recording_state_changed {
            cb(ballot_id, ConsentState::Idle, &self.self_peer_id);
        }

        Ok(())
    }

    /// Revoke consent mid-call.
    pub fn revoke_consent(&mut self, ballot_id: u32) -> GafanaResult<()> {
        if !self.in_room {
            return Err(GafanaError::RoomNotJoined);
        }

        let rev_pkt = PktConsentRevoke {
            ballot_id,
            revoker_id: self.self_peer_id.clone(),
        };
        let _ = self.net.send(
            &self.room_id,
            &self.self_peer_id,
            GAFANA_OP_CONSENT_REVOKE,
            &rev_pkt.serialize(),
        );

        if self.recorder.is_recording {
            let _ = self.recorder.stop();
        }

        self.consensus.revoke_consent(ballot_id);

        if let Some(ref cb) = self.callbacks.on_recording_state_changed {
            cb(ballot_id, ConsentState::Revoked, &self.self_peer_id);
        }

        Ok(())
    }

    /// Check whether screen capture or media recording is authorized under 100% unanimous consent.
    pub fn is_capture_authorized(&self) -> bool {
        self.consensus.current_ballot.state == ConsentState::UnanimousOk
    }

    /// Enforces the Anti-Capture and Device Background Recording Guard.
    /// Returns Ok(()) if authorized, or Err(GafanaError::ConsentDenied) if any user has not accepted.
    pub fn enforce_anti_capture_guard(&self) -> GafanaResult<()> {
        if self.consensus.current_ballot.state != ConsentState::UnanimousOk {
            Err(GafanaError::ConsentDenied)
        } else {
            Ok(())
        }
    }

    /// Broadcast audio frame.
    pub fn send_audio_frame(&mut self, _track_id: u32, frame: &AudioFrame) -> GafanaResult<()> {
        if !self.in_room {
            return Err(GafanaError::RoomNotJoined);
        }

        if self.recorder.is_recording {
            let _ = self.recorder.write_audio_samples(frame.pcm_samples);
        }

        let mut byte_buf = Vec::with_capacity(frame.pcm_samples.len() * 2);
        for &s in frame.pcm_samples {
            byte_buf.extend_from_slice(&s.to_le_bytes());
        }

        self.net.send(
            &self.room_id,
            &self.self_peer_id,
            GAFANA_OP_MEDIA_AUDIO,
            &byte_buf,
        )
    }

    /// Start screen sharing.
    pub fn start_screen_share(&mut self) -> GafanaResult<u32> {
        let tid = self.next_track_id.fetch_add(1, Ordering::Relaxed);
        let mut announce = [0u8; 8];
        announce[0..4].copy_from_slice(&(TrackType::Screen as u32).to_le_bytes());
        announce[4..8].copy_from_slice(&tid.to_le_bytes());
        let _ = self.net.send(
            &self.room_id,
            &self.self_peer_id,
            GAFANA_OP_TRACK_ANNOUNCE,
            &announce,
        );
        Ok(tid)
    }

    /// Stop screen sharing.
    pub fn stop_screen_share(&mut self, track_id: u32) -> GafanaResult<()> {
        let _ = self.net.send(
            &self.room_id,
            &self.self_peer_id,
            GAFANA_OP_TRACK_REMOVE,
            &track_id.to_le_bytes(),
        );
        Ok(())
    }

    /// Start presentation mode.
    pub fn start_presentation(&mut self, total_slides: u32) -> GafanaResult<u32> {
        let tid = self.next_track_id.fetch_add(1, Ordering::Relaxed);
        self.presentation.is_presenting = true;
        self.presentation.track_id = tid;
        self.presentation.presenter_id = self.self_peer_id.clone();
        self.presentation.current_slide = 1;
        self.presentation.total_slides = total_slides;

        let ev = PktPresentation {
            event_type: PresEventType::SlideChange as u8,
            current_slide: 1,
            total_slides,
            pointer_x: 0.5,
            pointer_y: 0.5,
            color_hex: 0xFF0000,
        };
        let _ = self.net.send(
            &self.room_id,
            &self.self_peer_id,
            GAFANA_OP_PRES_EVENT,
            &ev.serialize(),
        );
        Ok(tid)
    }

    /// Stop presentation mode.
    pub fn stop_presentation(&mut self, track_id: u32) -> GafanaResult<()> {
        self.presentation.is_presenting = false;
        let ev = PktPresentation {
            event_type: PresEventType::Stop as u8,
            current_slide: 0,
            total_slides: 0,
            pointer_x: 0.0,
            pointer_y: 0.0,
            color_hex: 0,
        };
        let _ = self.net.send(
            &self.room_id,
            &self.self_peer_id,
            GAFANA_OP_PRES_EVENT,
            &ev.serialize(),
        );
        let _ = self.net.send(
            &self.room_id,
            &self.self_peer_id,
            GAFANA_OP_TRACK_REMOVE,
            &track_id.to_le_bytes(),
        );
        Ok(())
    }
}

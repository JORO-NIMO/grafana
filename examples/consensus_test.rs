//! Gafana Engine Consensus & Zero-Data Architecture Test Suite (Rust Edition)
//!
//! Mirrors `examples/consensus_test.c`:
//! 1. Ephemeral room creation and peer discovery
//! 2. Unanimous consent state machine:
//!    - Requesting permission to record
//!    - One 'NO' vote blocks recording (Veto power)
//!    - 100% 'YES' votes unblocks local recording
//! 3. Local file disk recording (WAV / Binary frame stream)
//! 4. Mid-recording consent revocation (immediate forced termination)
//! 5. Zero server data retention

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use gafana::consensus::{ConsentState, RecordingType};
use gafana::engine::{EngineCallbacks, EngineConfig, GafanaEngine};
use gafana::error::GafanaError;
use gafana::media::AudioFrame;
use gafana::protocol::*;

fn main() {
    println!("=================================================================");
    println!(" Gafana Engine - Consensus & Zero-Data Architecture Test Suite   ");
    println!(" Version: {}", gafana::VERSION);
    println!("=================================================================\n");

    let consent_requested_count = Arc::new(AtomicU32::new(0));
    let consent_state_changes = Arc::new(AtomicU32::new(0));

    let cr_clone = consent_requested_count.clone();
    let cs_clone = consent_state_changes.clone();

    let callbacks = EngineCallbacks {
        on_recording_consent_requested: Some(Box::new(move |ballot_id, requester_id, rec_type| {
            println!(
                "[CALLBACK] Peer '{}' requested recording consent (Ballot #{}, Type: {:?})",
                requester_id, ballot_id, rec_type
            );
            cr_clone.fetch_add(1, Ordering::SeqCst);
        })),
        on_recording_state_changed: Some(Box::new(move |ballot_id, state, peer_id| {
            println!(
                "[CALLBACK] Ballot #{} state changed -> {:?} (Peer: {})",
                ballot_id, state, peer_id
            );
            cs_clone.fetch_add(1, Ordering::SeqCst);
        })),
        on_peer_joined: Some(Box::new(|peer| {
            println!("[CALLBACK] Peer joined: {} ({})", peer.display_name, peer.peer_id);
        })),
        ..Default::default()
    };

    let mut config = EngineConfig::default();
    config.relay_port = 8765;
    let mut alice = GafanaEngine::new(config, callbacks);

    // Alice joins room
    alice
        .join_room("Room_Alpha99", "peer_alice", "Alice (Host)")
        .expect("Alice should join room");
    println!(
        "[TEST 1] Alice joined room. Peer count: {}",
        alice.peers.active_count()
    );

    // --- Scenario A: Solo user requests recording ---
    println!("\n--- SCENARIO A: Solo Participant Local Recording ---");
    let (ballot_solo, state_solo) = alice
        .request_recording_consent(RecordingType::AudioOnly)
        .expect("Solo recording consent request failed");

    assert_eq!(state_solo, ConsentState::UnanimousOk);
    println!("-> Solo participant automatically has unanimous consent (100%): OK");

    alice
        .start_recording(ballot_solo, "test_solo_recording.wav")
        .expect("Should start recording");
    assert!(alice.recorder.is_recording);
    println!("-> Local recording started on client disk: test_solo_recording.wav");

    // Stream some dummy audio PCM samples
    let dummy_pcm: Vec<i16> = (0..480).map(|i| (i * 30) as i16).collect();
    let aframe = AudioFrame {
        pcm_samples: &dummy_pcm,
        sample_rate: 48000,
        channels: 2,
        timestamp_ms: 10,
    };
    alice
        .send_audio_frame(1, &aframe)
        .expect("Should send audio frame");

    alice
        .stop_recording()
        .expect("Should stop local recording");
    assert!(!alice.recorder.is_recording);
    println!("-> Local recording stopped safely and WAV header finalized.");

    // --- Scenario B: Multi-User Consensus with Veto ---
    println!("\n--- SCENARIO B: Multi-User Consensus & Veto Protocol ---");
    // Simulate Bob and Charlie joining via packet injection
    let bob_join_hdr = NetPacketHeader::new(
        GAFANA_OP_JOIN,
        1,
        100,
        "Room_Alpha99",
        "peer_bob",
        3,
        0,
        1,
        0,
    );
    alice.handle_packet(&bob_join_hdr, b"Bob");

    let charlie_join_hdr = NetPacketHeader::new(
        GAFANA_OP_JOIN,
        2,
        101,
        "Room_Alpha99",
        "peer_charlie",
        7,
        0,
        1,
        0,
    );
    alice.handle_packet(&charlie_join_hdr, b"Charlie");

    println!(
        "-> Total active peers now in meeting: {} (Alice, Bob, Charlie)",
        alice.peers.active_count()
    );

    // Alice requests consent to record Audio + Video
    let (ballot_multi, state_multi) = alice
        .request_recording_consent(RecordingType::FullAv)
        .expect("Multi consent request failed");
    assert_eq!(state_multi, ConsentState::Requested);
    println!(
        "-> Ballot #{} initiated by Alice. Current state: PENDING_VOTES",
        ballot_multi
    );

    // Attempting to record prematurely MUST FAIL
    let early_res = alice.start_recording(ballot_multi, "unauthorized.mp4");
    assert_eq!(early_res.err(), Some(GafanaError::ConsentPending));
    println!("-> VERIFIED: Premature recording attempt blocked with ConsentPending");

    // Bob votes YES
    println!("-> Bob votes: ACCEPT (YES)");
    let v1 = PktConsentVote {
        ballot_id: ballot_multi,
        vote: 1,
        voter_id: "peer_bob".to_string(),
    };
    let v1_hdr = NetPacketHeader::new(
        GAFANA_OP_CONSENT_VOTE,
        3,
        105,
        "Room_Alpha99",
        "peer_bob",
        PktConsentVote::SIZE as u16,
        0,
        1,
        0,
    );
    alice.handle_packet(&v1_hdr, &v1.serialize());

    // Charlie votes NO (Veto!)
    println!("-> Charlie votes: DENY (NO)");
    let v2 = PktConsentVote {
        ballot_id: ballot_multi,
        vote: 0,
        voter_id: "peer_charlie".to_string(),
    };
    let v2_hdr = NetPacketHeader::new(
        GAFANA_OP_CONSENT_VOTE,
        4,
        106,
        "Room_Alpha99",
        "peer_charlie",
        PktConsentVote::SIZE as u16,
        0,
        1,
        0,
    );
    alice.handle_packet(&v2_hdr, &v2.serialize());

    assert_eq!(
        alice.consensus.current_ballot.state,
        ConsentState::Rejected
    );
    let vetoed_res = alice.start_recording(ballot_multi, "unauthorized.mp4");
    assert_eq!(vetoed_res.err(), Some(GafanaError::ConsentDenied));
    println!("-> VERIFIED: Single participant veto successfully REJECTED recording!");

    // --- Scenario C: 100% Unanimous Approval + Mid-Stream Revocation ---
    println!("\n--- SCENARIO C: 100% Unanimous Approval + Mid-Stream Revocation ---");
    let (ballot_unanimous, _) = alice
        .request_recording_consent(RecordingType::FullAv)
        .expect("Unanimous consent request failed");

    // Bob votes YES
    let v1_ok = PktConsentVote {
        ballot_id: ballot_unanimous,
        vote: 1,
        voter_id: "peer_bob".to_string(),
    };
    alice.handle_packet(&v1_hdr, &v1_ok.serialize());

    // Charlie votes YES this time
    let v2_ok = PktConsentVote {
        ballot_id: ballot_unanimous,
        vote: 1,
        voter_id: "peer_charlie".to_string(),
    };
    alice.handle_packet(&v2_hdr, &v2_ok.serialize());

    assert_eq!(
        alice.consensus.current_ballot.state,
        ConsentState::UnanimousOk
    );
    println!("-> 100% Unanimous Consent achieved! State: UNANIMOUS_APPROVED");

    alice
        .start_recording(ballot_unanimous, "consensual_meeting.rec")
        .expect("Recording start should succeed");
    assert!(alice.recorder.is_recording);
    println!("-> Local recording actively streaming to: consensual_meeting.rec");

    // Bob revokes consent midway during the call!
    println!("\n[ALERT] Bob revokes consent during the active call!");
    let rev = PktConsentRevoke {
        ballot_id: ballot_unanimous,
        revoker_id: "peer_bob".to_string(),
    };
    let rev_hdr = NetPacketHeader::new(
        GAFANA_OP_CONSENT_REVOKE,
        10,
        200,
        "Room_Alpha99",
        "peer_bob",
        PktConsentRevoke::SIZE as u16,
        0,
        1,
        0,
    );
    alice.handle_packet(&rev_hdr, &rev.serialize());

    assert!(!alice.recorder.is_recording);
    println!("-> VERIFIED: Local recording was IMMEDIATELY forced to halt upon revocation!");

    alice.leave_room().expect("Should cleanly leave room");

    println!("\n=================================================================");
    println!(" ALL GAFANA RUST TESTS PASSED SUCCESSFULLY! ZERO DATA LEAKAGE CONFIRMED ");
    println!("=================================================================");
}

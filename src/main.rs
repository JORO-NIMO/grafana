//! Grafana Unified Single-Executable Application (Rust Edition)
//!
//! Encapsulates:
//! 1. In-Memory Stateless SFU Relay Server (UDP Port 8765)
//! 2. Embedded WebRTC UI Server (TCP Port 7878)
//! 3. Core Media Engine & 100% Unanimous Consensus Recorder
//! 4. Interactive Desktop Terminal CLI Console
//! 5. Auto-launch of the browser WebRTC interface

use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use grafana::consensus::{ConsentState, RecordingType};
use grafana::engine::{EngineCallbacks, EngineConfig, GrafanaEngine};
use grafana::http::{EmbeddedHttpServer, HttpServerConfig};
// Presentation events handled via WebRTC screen share
use grafana::sfu::StatelessSfuRelay;
use grafana::smtp::SmtpClient;

const GRAFANA_SFU_PORT: u16 = 8765;
const GRAFANA_HTTP_PORT: u16 = 7878;

fn main() {
    println!("=================================================================");
    println!("  GRAFANA ALL-IN-ONE VIDEO CALLING APPLIANCE (Rust Edition v1.2)  ");
    println!("  Zero-Cloud Data | 100% Unanimous Consensus | Pure Memory Mesh  ");
    println!("=================================================================\n");

    let running = Arc::new(AtomicBool::new(true));
    let sfu_packets = Arc::new(AtomicU64::new(0));

    // 1. Launch In-Memory Stateless SFU Relay Thread (UDP 8765)
    println!("[1/4] Starting In-Memory Stateless SFU Relay on UDP port {}...", GRAFANA_SFU_PORT);
    let sfu_running = running.clone();
    let sfu_pkts = sfu_packets.clone();
    thread::spawn(move || {
        let mut relay = StatelessSfuRelay::new(GRAFANA_SFU_PORT);
        relay.total_packets_forwarded = sfu_pkts;
        if let Err(e) = relay.run_loop(sfu_running) {
            eprintln!("[SFU Relay] Error: {}", e);
        }
    });

    let smtp_client = Arc::new(Mutex::new(SmtpClient::new()));

    // 2. Initialize Core Grafana Engine
    println!("[2/4] Initializing Core Media Engine...");
    let callbacks = EngineCallbacks {
        on_peer_joined: Some(Box::new(|peer| {
            println!("\n>>> [PEER JOINED] {} (ID: {})\n> ", peer.display_name, peer.peer_id);
            let _ = io::stdout().flush();
        })),
        on_peer_left: Some(Box::new(|peer_id, reason| {
            println!("\n<<< [PEER LEFT] {} (Reason: {})\n> ", peer_id, reason);
            let _ = io::stdout().flush();
        })),
        on_recording_consent_requested: Some(Box::new(|ballot_id, requester_id, rec_type| {
            println!("\n============================================================");
            println!(" [CONSENT REQUEST] Peer '{}' wants to record this meeting!", requester_id);
            println!(" Ballot ID: #{}", ballot_id);
            println!(" Recording Type: {:?}", rec_type);
            println!(" Consensus Rule: 100% Unanimous Agreement Required.");
            println!(" Type 'accept' or 'deny' to vote.");
            println!("============================================================\n> ");
            let _ = io::stdout().flush();
        })),
        on_recording_vote_update: Some(Box::new(|ballot_id, votes_granted, votes_total| {
            println!("\n[VOTE TALLY] Ballot #{}: {} of {} accepted.\n> ", ballot_id, votes_granted, votes_total);
            let _ = io::stdout().flush();
        })),
        on_recording_state_changed: Some(Box::new(|ballot_id, state, peer_id| {
            match state {
                ConsentState::UnanimousOk => {
                    println!("\n[CONSENT GRANTED] 100% approved! '{}' may now record locally.\n> ", peer_id);
                }
                ConsentState::Rejected => {
                    println!("\n[CONSENT REJECTED] Veto cast. Recording is blocked.\n> ");
                }
                ConsentState::Revoked => {
                    println!("\n[CONSENT REVOKED] Permission withdrawn! Recording halted immediately.\n> ");
                }
                ConsentState::Expired => {
                    println!("\n[CONSENT EXPIRED] Ballot #{} timed out.\n> ", ballot_id);
                }
                ConsentState::Idle => {
                    println!("\n[RECORDING STOPPED] Local capture finalized.\n> ");
                }
                _ => {}
            }
            let _ = io::stdout().flush();
        })),
        ..Default::default()
    };

    let mut config = EngineConfig::default();
    config.relay_port = GRAFANA_SFU_PORT;
    let engine = Arc::new(Mutex::new(GrafanaEngine::new(config, callbacks)));

    let random_id = format!("host_{}", (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() % 9000) + 1000);
    {
        let mut eng = engine.lock().unwrap();
        if let Err(e) = eng.join_room("Grafana-Secure-Room", &random_id, "Host User") {
            eprintln!("Failed to join room: {}", e);
        }
    }

    // 3. Launch Embedded Web UI Server (TCP 7878)
    println!("[3/4] Starting Embedded WebRTC UI Server on http://localhost:{}...", GRAFANA_HTTP_PORT);
    let signal_hub = Arc::new(Mutex::new(grafana::http::SignalHub::default()));
    let auth_registry = Arc::new(Mutex::new(grafana::auth::AuthRegistry::new()));
    let http_config = HttpServerConfig {
        port: GRAFANA_HTTP_PORT,
        sfu_packets: sfu_packets.clone(),
        running: running.clone(),
        smtp_client: smtp_client.clone(),
        engine: engine.clone(),
        signal_hub: signal_hub.clone(),
        auth_registry: auth_registry.clone(),
    };
    EmbeddedHttpServer::new(http_config).start();

    // Spawn engine polling thread
    let eng_poll = engine.clone();
    let poll_running = running.clone();
    thread::spawn(move || {
        while poll_running.load(Ordering::Relaxed) {
            {
                let mut eng = eng_poll.lock().unwrap();
                let _ = eng.poll();
            }
            thread::sleep(Duration::from_millis(20));
        }
    });

    // 4. Auto-launch the web UI in default browser
    println!("[4/4] Launching Web Interface in default browser...");
    #[cfg(target_os = "windows")]
    {
        let url = "http://localhost:7878";
        let launched = std::process::Command::new("cmd")
            .args(["/c", "start", "chrome", "--incognito", "--disable-extensions", url])
            .spawn()
            .or_else(|_| {
                std::process::Command::new("cmd")
                    .args(["/c", "start", "msedge", "--inprivate", url])
                    .spawn()
            });

        if launched.is_err() {
            let _ = std::process::Command::new("cmd")
                .args(["/c", "start", url])
                .spawn();
        }
    }

    let lan_ip = grafana::http::detect_lan_ip();
    let shareable_url = grafana::http::get_shareable_url(GRAFANA_HTTP_PORT);

    println!("\n>>> Grafana (Rust Edition) is LIVE!");
    println!(">>> Web UI (Local):        http://localhost:{}", GRAFANA_HTTP_PORT);
    println!(">>> Shareable Network URL: {}", shareable_url);
    println!(">>> UDP Relay (LAN):       {}:{}", lan_ip, GRAFANA_SFU_PORT);
    println!(">>> Ephemeral Room:        'Grafana-Secure-Room' (Zero Cloud Retention)\n");

    println!("Available Console Commands:");
    println!("  peers        - List active room participants");
    println!("  record [a|v] - Request consent to record (a=audio, v=full AV)");
    println!("  startrec     - Start writing to local disk (once approved)");
    println!("  stoprec      - Stop local recording");
    println!("  accept       - Vote YES on pending ballot");
    println!("  deny         - Vote NO (Veto) on pending ballot");
    println!("  revoke       - Revoke consent mid-call (emergency halt)");
    println!("  screen       - Toggle native screen sharing");
    println!("  stats        - Show relay in-memory packet throughput");
    println!("  quit         - Exit and terminate Grafana\n");

    print!("> ");
    let _ = io::stdout().flush();

    let stdin = io::stdin();
    let mut my_ballot = 0u32;
    let mut screen_track = 0u32;

    for line in stdin.lock().lines() {
        let cmd = match line {
            Ok(c) => c.trim().to_string(),
            Err(_) => break,
        };

        if cmd.is_empty() {
            print!("> ");
            let _ = io::stdout().flush();
            continue;
        }

        match cmd.as_str() {
            "quit" | "exit" => {
                break;
            }
            "peers" => {
                let eng = engine.lock().unwrap();
                let active = eng.peers.list_active();
                println!("Active Room Participants ({}):", active.len());
                for (i, p) in active.iter().enumerate() {
                    println!("  [{}] {} (ID: {})", i + 1, p.display_name, p.peer_id);
                }
            }
            c if c.starts_with("record") => {
                let rtype = if c.contains('a') {
                    RecordingType::AudioOnly
                } else {
                    RecordingType::FullAv
                };
                let mut eng = engine.lock().unwrap();
                match eng.request_recording_consent(rtype) {
                    Ok((bid, _)) => {
                        my_ballot = bid;
                        println!("Ballot #{} initiated. Waiting for 100% unanimous consent...", bid);
                    }
                    Err(e) => println!("Error requesting recording: {}", e),
                }
            }
            "accept" => {
                let mut eng = engine.lock().unwrap();
                let bid = if my_ballot > 0 { my_ballot } else { eng.consensus.current_ballot.ballot_id };
                let _ = eng.cast_vote(bid, true);
                println!("Voted: ACCEPT (YES)");
            }
            "deny" => {
                let mut eng = engine.lock().unwrap();
                let bid = if my_ballot > 0 { my_ballot } else { eng.consensus.current_ballot.ballot_id };
                let _ = eng.cast_vote(bid, false);
                println!("Voted: DENY (VETO)");
            }
            "startrec" => {
                let mut eng = engine.lock().unwrap();
                let out_file = "grafana_local_capture.wav";
                match eng.start_recording(my_ballot, out_file) {
                    Ok(_) => println!("Local recording started: {} (ZERO cloud copies)", out_file),
                    Err(e) => println!("Blocked: {}", e),
                }
            }
            "stoprec" => {
                let mut eng = engine.lock().unwrap();
                match eng.stop_recording() {
                    Ok(_) => println!("Local recording stopped and saved to disk."),
                    Err(e) => println!("Stop recording error: {}", e),
                }
            }
            "revoke" => {
                let mut eng = engine.lock().unwrap();
                let bid = if my_ballot > 0 { my_ballot } else { eng.consensus.current_ballot.ballot_id };
                let _ = eng.revoke_consent(bid);
                println!("Revocation broadcast! Any ongoing recording has been terminated.");
            }
            "screen" => {
                let mut eng = engine.lock().unwrap();
                if screen_track == 0 {
                    match eng.start_screen_share() {
                        Ok(tid) => {
                            screen_track = tid;
                            println!("Screen sharing track active (Track #{})", tid);
                        }
                        Err(e) => println!("Screen share error: {}", e),
                    }
                } else {
                    let _ = eng.stop_screen_share(screen_track);
                    println!("Screen sharing track stopped.");
                    screen_track = 0;
                }
            }

            "stats" => {
                let count = sfu_packets.load(Ordering::Relaxed);
                println!("Relay Stats: {} in-memory packets routed (0 bytes on server disk)", count);
            }

            other => {
                println!("Unknown command '{}'. Try: peers, record, accept, deny, startrec, stoprec, screen, stats, quit", other);
            }
        }

        print!("> ");
        let _ = io::stdout().flush();
    }

    println!("\nShutting down Grafana Appliance...");
    running.store(false, Ordering::Relaxed);
    {
        let mut eng = engine.lock().unwrap();
        let _ = eng.leave_room();
    }
    println!("Grafana safely terminated. All ephemeral session memory discarded.");
}

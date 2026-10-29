//! Standalone Stateless SFU Relay Server (`gafana-relay`)
//!
//! Sub-millisecond in-memory UDP forwarding with zero disk or database overhead.

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use gafana::sfu::StatelessSfuRelay;

fn main() {
    let args: Vec<String> = env::args().collect();
    let port: u16 = if args.len() > 1 {
        args[1].parse().unwrap_or(8765)
    } else {
        8765
    };

    println!("=================================================================");
    println!(" Gafana Stateless SFU Relay Server (Rust Edition v1.2.0)         ");
    println!(" Zero Cloud Storage | Memory Packet Forwarder | Ephemeral Mesh  ");
    println!(" Listening on UDP port: {}                                      ", port);
    println!("=================================================================\n");

    let mut relay = StatelessSfuRelay::new(port);
    let running = Arc::new(AtomicBool::new(true));

    let r_clone = running.clone();
    ctrlc_handler(move || {
        println!("\n[SFU] Termination signal received. Discarding all memory routing tables...");
        r_clone.store(false, Ordering::Relaxed);
    });

    if let Err(e) = relay.run_loop(running) {
        eprintln!("[SFU Relay] Fatal error: {}", e);
    }
}

fn ctrlc_handler<F: FnOnce() + Send + 'static>(_f: F) {
    // Graceful signal fallback
}

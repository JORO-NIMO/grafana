//! # Gafana: High-Performance, Privacy-First, Zero-Data Video Calling Engine in Rust
//!
//! Production Rust Library API (v1.2.0)
//!
//! Enforces:
//! 1. Zero Cloud Data: Stateless UDP routing, zero database, zero server disk caching.
//! 2. Unanimous Consensus Recording: 100% agreement required from all room participants.
//! 3. Dynamic Revocation: Mid-call revocation instantly halts local disk capture.
//! 4. Scalable Media Architecture: Multi-track Audio, Video, Screen Share, and Presentations.

pub mod consensus;
pub mod engine;
pub mod error;
pub mod http;
pub mod media;
pub mod net;
pub mod peer;
pub mod protocol;
pub mod recorder;
pub mod sfu;
pub mod smtp;

pub use consensus::{ConsentState, RecordingType};
pub use engine::{EngineCallbacks, EngineConfig, GafanaEngine};
pub use error::{GafanaError, GafanaResult};
pub use media::{AudioFrame, PresentationEvent, TrackType, VideoFrame};
pub use peer::PeerInfo;
pub use sfu::StatelessSfuRelay;
pub use smtp::{SmtpClient, SmtpConfig};

pub const VERSION: &str = "1.2.0";

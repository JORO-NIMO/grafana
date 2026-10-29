use std::fmt;

/// Gafana Error Codes mapping 1:1 with C `gafana_error_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum GafanaError {
    Ok = 0,
    InvalidParam = -1,
    NotInitialized = -2,
    AlreadyInitialized = -3,
    RoomNotJoined = -4,
    AlreadyInRoom = -5,
    PeerNotFound = -6,
    ConsentPending = -7,
    ConsentDenied = -8,
    ConsentRevoked = -9,
    ConsentTimeout = -10,
    RecordingActive = -11,
    RecordingInactive = -12,
    Unauthorized = -13,
    Network = -14,
    Socket = -15,
    Io = -16,
    OutOfMemory = -17,
    BufferOverflow = -18,
    Unsupported = -19,
}

impl GafanaError {
    pub fn as_i32(&self) -> i32 {
        *self as i32
    }
}

impl fmt::Display for GafanaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => write!(f, "Success (OK)"),
            Self::InvalidParam => write!(f, "Invalid parameter"),
            Self::NotInitialized => write!(f, "Engine not initialized"),
            Self::AlreadyInitialized => write!(f, "Engine already initialized"),
            Self::RoomNotJoined => write!(f, "Room not joined"),
            Self::AlreadyInRoom => write!(f, "Already in room"),
            Self::PeerNotFound => write!(f, "Peer not found"),
            Self::ConsentPending => write!(f, "Recording consent ballot pending"),
            Self::ConsentDenied => write!(f, "Recording consent denied (vetoed)"),
            Self::ConsentRevoked => write!(f, "Recording consent revoked mid-call"),
            Self::ConsentTimeout => write!(f, "Recording consent ballot timed out"),
            Self::RecordingActive => write!(f, "Recording is currently active"),
            Self::RecordingInactive => write!(f, "Recording is inactive"),
            Self::Unauthorized => write!(f, "Unauthorized operation"),
            Self::Network => write!(f, "Network transmission failure"),
            Self::Socket => write!(f, "Socket subsystem error"),
            Self::Io => write!(f, "Local disk I/O error"),
            Self::OutOfMemory => write!(f, "Out of memory / capacity reached"),
            Self::BufferOverflow => write!(f, "Buffer overflow"),
            Self::Unsupported => write!(f, "Operation unsupported"),
        }
    }
}

impl std::error::Error for GafanaError {}

pub type GafanaResult<T> = Result<T, GafanaError>;

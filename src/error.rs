use std::fmt;

/// Grafana Error Codes mapping 1:1 with C `grafana_error_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum GrafanaError {
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
    Timeout = -20,
    PermissionDenied = -21,
    ConsensusRejected = -22,
}

impl GrafanaError {
    pub fn as_i32(&self) -> i32 {
        *self as i32
    }
}

impl fmt::Display for GrafanaError {
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
            Self::Timeout => write!(f, "Operation timed out"),
            Self::PermissionDenied => write!(f, "Permission denied"),
            Self::ConsensusRejected => write!(f, "Consensus rejected or unverified account"),
        }
    }
}

impl std::error::Error for GrafanaError {}

pub type GrafanaResult<T> = Result<T, GrafanaError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes_and_display() {
        assert_eq!(GrafanaError::Ok.as_i32(), 0);
        assert_eq!(GrafanaError::InvalidParam.as_i32(), -1);
        assert_eq!(GrafanaError::ConsentDenied.as_i32(), -8);
        assert_eq!(GrafanaError::ConsentRevoked.as_i32(), -9);

        let err_str = format!("{}", GrafanaError::ConsentDenied);
        assert!(err_str.contains("vetoed"));

        let io_str = format!("{}", GrafanaError::Io);
        assert!(io_str.contains("disk I/O"));
    }
}

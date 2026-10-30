//! Consensus & Unanimous Agreement State Machine

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConsentState {
    Idle = 0,
    Requested = 1,
    UnanimousOk = 2,
    Rejected = 3,
    Revoked = 4,
    Expired = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordingType {
    AudioOnly = 1,
    FullAv = 2,
    ScreenAudio = 3,
}

#[derive(Debug, Clone)]
pub struct Ballot {
    pub ballot_id: u32,
    pub requester_id: String,
    pub rec_type: RecordingType,
    pub state: ConsentState,
    pub total_expected_votes: u32,
    pub yes_votes: u32,
    pub no_votes: u32,
    pub is_active: bool,
    pub start_time_ms: u32,
    pub expires_at_ms: u32,
}

impl Default for Ballot {
    fn default() -> Self {
        Self {
            ballot_id: 0,
            requester_id: String::new(),
            rec_type: RecordingType::AudioOnly,
            state: ConsentState::Idle,
            total_expected_votes: 0,
            yes_votes: 0,
            no_votes: 0,
            is_active: false,
            start_time_ms: 0,
            expires_at_ms: 0,
        }
    }
}

pub struct ConsensusEngine {
    pub current_ballot: Ballot,
    next_ballot_id: u32,
    default_timeout_sec: u32,
}

impl ConsensusEngine {
    pub fn new(default_timeout_sec: u32) -> Self {
        Self {
            current_ballot: Ballot::default(),
            next_ballot_id: 100,
            default_timeout_sec: if default_timeout_sec > 0 {
                default_timeout_sec
            } else {
                30
            },
        }
    }

    /// Reset ballot and peer voting flags.
    pub fn reset(&mut self) {
        self.current_ballot = Ballot::default();
    }

    /// Start a new recording consent ballot.
    pub fn start_ballot(
        &mut self,
        requester_id: &str,
        rec_type: RecordingType,
        total_participants: u32,
    ) -> (u32, ConsentState) {
        let ballot_id = self.next_ballot_id;
        self.next_ballot_id = self.next_ballot_id.wrapping_add(1);

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u32)
            .unwrap_or(0);

        let expires_ms = now_ms + (self.default_timeout_sec * 1000);

        let initial_state = if total_participants <= 1 {
            // Solo participant -> immediate unanimous agreement
            ConsentState::UnanimousOk
        } else {
            ConsentState::Requested
        };

        self.current_ballot = Ballot {
            ballot_id,
            requester_id: requester_id.to_string(),
            rec_type,
            state: initial_state,
            total_expected_votes: total_participants.max(1),
            yes_votes: 1, // Requester automatically votes yes
            no_votes: 0,
            is_active: true,
            start_time_ms: now_ms,
            expires_at_ms: expires_ms,
        };

        (ballot_id, initial_state)
    }

    /// Cast a vote. Returns `(new_state, is_unanimous)`.
    pub fn cast_vote(&mut self, ballot_id: u32, accept: bool) -> Option<(ConsentState, bool)> {
        if !self.current_ballot.is_active || self.current_ballot.ballot_id != ballot_id {
            return None;
        }

        if self.current_ballot.state != ConsentState::Requested {
            return Some((self.current_ballot.state, false));
        }

        if !accept {
            // SINGLE VETO REJECTS BALLOT
            self.current_ballot.no_votes += 1;
            self.current_ballot.state = ConsentState::Rejected;
            return Some((ConsentState::Rejected, false));
        }

        self.current_ballot.yes_votes += 1;
        let unanimous = self.current_ballot.yes_votes >= self.current_ballot.total_expected_votes;
        if unanimous {
            self.current_ballot.state = ConsentState::UnanimousOk;
        }

        Some((self.current_ballot.state, unanimous))
    }

    /// Revoke consent mid-call.
    pub fn revoke_consent(&mut self, ballot_id: u32) -> bool {
        if self.current_ballot.ballot_id == ballot_id || self.current_ballot.is_active {
            self.current_ballot.state = ConsentState::Revoked;
            true
        } else {
            false
        }
    }

    /// Check if the active ballot has expired due to timeout.
    pub fn check_timeouts(&mut self) -> Option<(u32, String)> {
        if self.current_ballot.is_active && self.current_ballot.state == ConsentState::Requested {
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u32)
                .unwrap_or(0);

            if now_ms >= self.current_ballot.expires_at_ms {
                self.current_ballot.state = ConsentState::Expired;
                self.current_ballot.is_active = false;
                return Some((
                    self.current_ballot.ballot_id,
                    self.current_ballot.requester_id.clone(),
                ));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solo_participant_auto_approval() {
        let mut engine = ConsensusEngine::new(30);
        let (bid, state) = engine.start_ballot("alice", RecordingType::FullAv, 1);
        assert_eq!(state, ConsentState::UnanimousOk);
        assert_eq!(engine.current_ballot.ballot_id, bid);
        assert_eq!(engine.current_ballot.state, ConsentState::UnanimousOk);
    }

    #[test]
    fn test_multi_participant_unanimous_approval() {
        let mut engine = ConsensusEngine::new(30);
        let (bid, state) = engine.start_ballot("alice", RecordingType::FullAv, 3);
        assert_eq!(state, ConsentState::Requested);
        assert_eq!(engine.current_ballot.yes_votes, 1); // Alice voted yes automatically
        assert_eq!(engine.current_ballot.total_expected_votes, 3);

        // Bob votes yes
        let (s1, unanimous1) = engine.cast_vote(bid, true).expect("bob vote");
        assert_eq!(s1, ConsentState::Requested);
        assert!(!unanimous1);

        // Charlie votes yes -> 100% agreement
        let (s2, unanimous2) = engine.cast_vote(bid, true).expect("charlie vote");
        assert_eq!(s2, ConsentState::UnanimousOk);
        assert!(unanimous2);
    }

    #[test]
    fn test_single_veto_blocks_recording() {
        let mut engine = ConsensusEngine::new(30);
        let (bid, state) = engine.start_ballot("alice", RecordingType::AudioOnly, 3);
        assert_eq!(state, ConsentState::Requested);

        // Bob votes yes
        let (s1, _) = engine.cast_vote(bid, true).expect("bob vote");
        assert_eq!(s1, ConsentState::Requested);

        // Charlie casts VETO (No)
        let (s2, unanimous) = engine.cast_vote(bid, false).expect("charlie veto");
        assert_eq!(s2, ConsentState::Rejected);
        assert!(!unanimous);
        assert_eq!(engine.current_ballot.no_votes, 1);
        assert_eq!(engine.current_ballot.state, ConsentState::Rejected);
    }

    #[test]
    fn test_revocation_mid_call() {
        let mut engine = ConsensusEngine::new(30);
        let (bid, _) = engine.start_ballot("alice", RecordingType::FullAv, 1);
        assert_eq!(engine.current_ballot.state, ConsentState::UnanimousOk);

        // Anyone revokes consent
        assert!(engine.revoke_consent(bid));
        assert_eq!(engine.current_ballot.state, ConsentState::Revoked);
    }

    #[test]
    fn test_reset_and_invalid_ballot_vote() {
        let mut engine = ConsensusEngine::new(30);
        let (bid, _) = engine.start_ballot("alice", RecordingType::FullAv, 2);
        // Vote for incorrect ballot id
        assert!(engine.cast_vote(bid + 999, true).is_none());

        engine.reset();
        assert_eq!(engine.current_ballot.state, ConsentState::Idle);
        assert!(!engine.current_ballot.is_active);
    }
}

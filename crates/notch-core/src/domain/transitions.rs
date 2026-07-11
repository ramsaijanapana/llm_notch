use notch_protocol::SessionStatus;

use crate::error::{CoreError, CoreResult};

/// Returns whether `to` is a legal successor of `from`.
pub fn is_valid_transition(from: SessionStatus, to: SessionStatus) -> bool {
    if from == to {
        return true;
    }

    use SessionStatus::*;
    match (from, to) {
        (Starting, Running | Failed | Stale) => true,
        (Running, Waiting | Paused | Completed | Failed | Stale) => true,
        (Waiting, Running | Paused | Completed | Failed | Stale) => true,
        (Paused, Running | Waiting | Completed | Failed | Stale) => true,
        (Completed | Failed | Stale, Stale) => true,
        _ => false,
    }
}

/// Validates a status change, allowing idempotent no-ops.
pub fn validate_transition(from: SessionStatus, to: SessionStatus) -> CoreResult<()> {
    if is_valid_transition(from, to) {
        Ok(())
    } else {
        Err(CoreError::InvalidTransition { from, to })
    }
}

/// Whether a status is terminal for lifecycle purposes.
pub fn is_terminal(status: SessionStatus) -> bool {
    matches!(
        status,
        SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Stale
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_protocol::SessionStatus::*;

    #[test]
    fn starting_may_become_running() {
        assert!(is_valid_transition(Starting, Running));
    }

    #[test]
    fn completed_cannot_return_to_running() {
        assert!(!is_valid_transition(Completed, Running));
        assert!(validate_transition(Completed, Running).is_err());
    }

    #[test]
    fn stale_may_repeat() {
        assert!(is_valid_transition(Running, Stale));
        assert!(is_valid_transition(Stale, Stale));
    }
}

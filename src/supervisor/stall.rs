//! Stall detection for the supervisor poll loop.
//!
//! Implements the stall-detection integration of the
//! `auto-approve-patterns` change: scan [`crate::broker::BrokerState`] for
//! agents whose status is `working` (or one of the still-active labels)
//! and whose `last_seen` exceeds the configured threshold, returning their
//! agent IDs for the auto-approver to act on.

use std::time::Duration;

use crate::broker::BrokerState;

/// Status labels considered "terminal" — these agents are NOT stalled even
/// if quiet, so the auto-approver MUST skip them per the `automatic-approval`
/// spec ("Skip terminal-status agents").
pub const TERMINAL_STATUSES: &[&str] = &["done", "verified", "blocked", "committed"];

/// Returns the agent IDs whose status is non-terminal but whose `last_seen`
/// is older than `threshold`.
///
/// "Working" here means anything not in [`TERMINAL_STATUSES`] — using a
/// negative match keeps the supervisor from missing newly introduced
/// active labels.
#[must_use]
pub fn detect_stalled_agents(state: &BrokerState, threshold: Duration) -> Vec<String> {
    let inner = state.read();
    inner
        .agents
        .values()
        .filter(|record| !TERMINAL_STATUSES.contains(&record.status.as_str()))
        .filter(|record| record.last_seen.elapsed() >= threshold)
        .map(|record| record.agent_id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::messages::{BrokerMessage, StatusPayload};
    use crate::broker::{AgentRecord, BrokerState};
    use std::time::{Duration, Instant};

    fn insert_record(state: &BrokerState, id: &str, status: &str, last_seen: Instant) {
        let mut inner = state.write();
        inner.agents.insert(
            id.to_string(),
            AgentRecord {
                agent_id: id.to_string(),
                status: status.to_string(),
                last_seen,
                last_message: Some(BrokerMessage::Status {
                    agent_id: id.to_string(),
                    payload: StatusPayload {
                        status: status.to_string(),
                        modified_files: Vec::new(),
                        message: None,
                    },
                }),
            },
        );
    }

    #[test]
    fn fresh_working_agent_is_not_stalled() {
        let state = BrokerState::new(None);
        insert_record(&state, "agent-fresh", "working", Instant::now());
        let stalled = detect_stalled_agents(&state, Duration::from_secs(30));
        assert!(
            stalled.is_empty(),
            "fresh working agent must not be stalled"
        );
    }

    #[test]
    fn stale_working_agent_is_stalled() {
        let state = BrokerState::new(None);
        let past = Instant::now().checked_sub(Duration::from_mins(2)).unwrap();
        insert_record(&state, "agent-stuck", "working", past);
        let stalled = detect_stalled_agents(&state, Duration::from_secs(30));
        assert_eq!(stalled, vec!["agent-stuck".to_string()]);
    }

    #[test]
    fn terminal_status_done_is_skipped_even_if_stale() {
        let state = BrokerState::new(None);
        let past = Instant::now().checked_sub(Duration::from_mins(10)).unwrap();
        insert_record(&state, "agent-done", "done", past);
        let stalled = detect_stalled_agents(&state, Duration::from_secs(30));
        assert!(stalled.is_empty(), "done is terminal — never stalled");
    }

    #[test]
    fn terminal_statuses_are_all_skipped() {
        let state = BrokerState::new(None);
        let past = Instant::now().checked_sub(Duration::from_mins(10)).unwrap();
        for status in TERMINAL_STATUSES {
            insert_record(&state, &format!("a-{status}"), status, past);
        }
        let stalled = detect_stalled_agents(&state, Duration::from_secs(30));
        assert!(stalled.is_empty());
    }

    #[test]
    fn unknown_status_label_treated_as_active() {
        // A future status label we have not seen before should be treated
        // as non-terminal so the supervisor still notices it stalled.
        let state = BrokerState::new(None);
        let past = Instant::now().checked_sub(Duration::from_mins(2)).unwrap();
        insert_record(&state, "agent-x", "researching", past);
        let stalled = detect_stalled_agents(&state, Duration::from_secs(30));
        assert_eq!(stalled, vec!["agent-x".to_string()]);
    }
}

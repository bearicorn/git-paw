//! Integration coverage for the reusable `coordination::inventory` helpers
//! (design D6, tasks 2.4 / 10.6 / 11.1).
//!
//! These tests live in the `tests/` crate — i.e. they exercise the helpers as
//! an *external* consumer, exactly as the future v1.0.0 MCP write tools'
//! `publish_agent_feedback` will. That they compile and pass proves the
//! inventory + validation API is a public library surface, not a private
//! helper buried inside the supervisor module.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use git_paw::config::TellMode;
use git_paw::coordination::inventory::{
    self, AgentInventory, InventoryCache, Mode, ValidationError,
};
use git_paw::coordination::tell::{self, DeliveryDecision};

/// Builds a two-agent inventory (`feat-a`, `feat-b`) the way an external
/// consumer would, from broker `/status` JSON + a tmux pane mapping.
fn external_inventory() -> AgentInventory {
    let status = r#"{
        "git_paw": true, "version": "0.6.0", "uptime_seconds": 1,
        "agents": [
            {"agent_id": "feat-a", "cli": "claude", "status": "working", "last_seen_seconds": 2, "summary": ""},
            {"agent_id": "feat-b", "cli": "codex", "status": "blocked", "last_seen_seconds": 40, "summary": ""}
        ]
    }"#;
    let agents = inventory::parse_status_agents(status).expect("status parses");
    let panes = inventory::parse_pane_paths("1 /w/proj-feat-b\n2 /w/proj-feat-a\n");
    let mut modes = HashMap::new();
    modes.insert(2usize, Mode::AcceptEdits); // feat-a
    let entries = inventory::join_inventory(agents, &panes, &modes);
    AgentInventory {
        entries,
        refreshed_at: Instant::now(),
    }
}

#[test]
fn validate_target_is_callable_from_outside_supervisor_module() {
    let inv = external_inventory();
    // Known target resolves; pane index is path-resolved (feat-a → pane 2).
    let entry = inventory::validate_target(&inv, "feat-a").expect("feat-a is a live agent");
    assert_eq!(entry.branch_id, "feat-a");
    assert_eq!(entry.pane_index, Some(2));
    assert_eq!(entry.mode, Mode::AcceptEdits);
}

#[test]
fn unknown_target_returns_documented_candidate_list() {
    let inv = external_inventory();
    let err = inventory::validate_target(&inv, "feat-ghost").unwrap_err();
    match err {
        ValidationError::UnknownTarget { target, candidates } => {
            assert_eq!(target, "feat-ghost");
            assert_eq!(candidates, vec!["feat-a".to_string(), "feat-b".to_string()]);
        }
    }
    // The Display form is also stable for any caller that just wants a string.
    let msg = inventory::validate_target(&inv, "feat-ghost")
        .unwrap_err()
        .to_string();
    assert!(msg.contains("feat-ghost") && msg.contains("feat-a, feat-b"));
}

#[test]
fn delivery_mode_selection_is_reusable() {
    let inv = external_inventory();
    let feat_a = inventory::validate_target(&inv, "feat-a").unwrap(); // accept-edits
    let feat_b = inventory::validate_target(&inv, "feat-b").unwrap(); // unknown mode

    // Default feedback mode always uses feedback.
    assert_eq!(
        tell::select_delivery_mode(TellMode::Feedback, feat_a.mode),
        DeliveryDecision::Feedback
    );
    // Configured send-keys + accept-edits target → send-keys.
    assert_eq!(
        tell::select_delivery_mode(TellMode::SendKeys, feat_a.mode),
        DeliveryDecision::SendKeys
    );
    // Configured send-keys + non-accept-edits target → feedback fallback.
    let decision = tell::select_delivery_mode(TellMode::SendKeys, feat_b.mode);
    assert_eq!(decision, DeliveryDecision::FeedbackFallback);
    assert!(decision.is_fallback());
}

#[test]
fn inventory_cache_reuses_within_window() {
    let mut cache = InventoryCache::from_seconds(60);
    let mut polls = 0u32;
    for _ in 0..3 {
        cache
            .get_or_refresh(Instant::now(), || {
                polls += 1;
                Ok::<_, ()>(external_inventory())
            })
            .unwrap();
    }
    assert_eq!(polls, 1, "fresh cache must be reused across rapid lookups");
    assert!(cache.snapshot().is_some());
    assert_eq!(cache.max_age(), Duration::from_mins(1));
}

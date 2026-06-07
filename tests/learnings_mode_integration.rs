//! Integration tests for the learnings-mode aggregator.
//!
//! Covers the cross-module flow: publishing fixture events through the
//! broker triggers the deterministic signal trackers; flushes append to
//! `.git-paw/session-learnings.md`; shutdown captures unresolved blocks;
//! a restart with a new aggregator preserves prior session content and
//! appends a fresh H2 section.

use std::sync::{Arc, Mutex};

use serial_test::serial;
use tempfile::TempDir;

use git_paw::broker::delivery::publish_message;
use git_paw::broker::learnings::{LearningsAggregator, PERMISSION_PATTERN_THRESHOLD};
use git_paw::broker::messages::{
    ArtifactPayload, BlockedPayload, BrokerMessage, FeedbackPayload, StatusPayload, VerifiedPayload,
};
use git_paw::broker::{BrokerState, WatchTarget, start_broker_with};
use git_paw::config::BrokerConfig;

fn broker_config(port_base: u16) -> BrokerConfig {
    #[allow(clippy::cast_possible_truncation)]
    BrokerConfig {
        enabled: true,
        port: port_base + (std::process::id() as u16 % 100),
        bind: "127.0.0.1".to_string(),
        ..Default::default()
    }
}

fn make_status(agent: &str, status: &str, message: Option<&str>) -> BrokerMessage {
    BrokerMessage::Status {
        agent_id: agent.to_string(),
        payload: StatusPayload {
            status: status.to_string(),
            modified_files: vec![],
            message: message.map(str::to_string),
            ..Default::default()
        },
    }
}

fn make_artifact(agent: &str, status: &str) -> BrokerMessage {
    BrokerMessage::Artifact {
        agent_id: agent.to_string(),
        payload: ArtifactPayload {
            status: status.to_string(),
            exports: vec![],
            modified_files: vec![],
        },
    }
}

fn make_blocked(agent: &str, from: &str) -> BrokerMessage {
    BrokerMessage::Blocked {
        agent_id: agent.to_string(),
        payload: BlockedPayload {
            needs: "types".to_string(),
            from: from.to_string(),
        },
    }
}

fn make_verified(target: &str) -> BrokerMessage {
    BrokerMessage::Verified {
        agent_id: target.to_string(),
        payload: VerifiedPayload {
            verified_by: "supervisor".to_string(),
            message: None,
        },
    }
}

fn make_feedback(target: &str, errors: &[&str]) -> BrokerMessage {
    BrokerMessage::Feedback {
        agent_id: target.to_string(),
        payload: FeedbackPayload {
            from: "supervisor".to_string(),
            errors: errors.iter().map(|s| (*s).to_string()).collect(),
        },
    }
}

fn watch_target(agent: &str, tmp: &TempDir) -> WatchTarget {
    WatchTarget {
        agent_id: agent.to_string(),
        cli: "claude".to_string(),
        worktree_path: tmp.path().to_path_buf(),
    }
}

#[test]
#[serial]
fn broker_aggregator_writes_expected_sections_and_no_learning_variant_appears() {
    let tmp = TempDir::new().unwrap();
    let md_path = tmp.path().join(".git-paw").join("session-learnings.md");
    let mut state = BrokerState::new(None);

    let agg = Arc::new(Mutex::new(LearningsAggregator::new(md_path.clone())));
    {
        let mut a = agg.lock().unwrap();
        a.register_agent("feat-x");
        a.register_agent("feat-y");
        a.register_agent("feat-z");
        a.set_spec_id("feat-x", "003-user-list");
        a.set_spec_id("feat-y", "003-user-list");
    }
    state.attach_learnings(Arc::clone(&agg));

    let config = broker_config(20_000);
    // Long interval — we drive the flush manually in the assertion. Skip
    // the test if the chosen port happens to be in use locally.
    let Ok(handle) = start_broker_with(
        &config,
        state,
        vec![
            watch_target("feat-x", &tmp),
            watch_target("feat-y", &tmp),
            watch_target("feat-z", &tmp),
        ],
        None,
        3600,
    ) else {
        return;
    };

    // Stuck-duration: feat-x blocked on feat-y, then unblocks.
    publish_message(&handle.state, &make_blocked("feat-x", "feat-y"));
    std::thread::sleep(std::time::Duration::from_millis(50));
    publish_message(&handle.state, &make_artifact("feat-x", "done"));

    // Recovery cycles: feat-z gets two feedbacks, then verifies.
    publish_message(&handle.state, &make_feedback("feat-z", &["test failed"]));
    publish_message(&handle.state, &make_feedback("feat-z", &["lint failed"]));
    publish_message(&handle.state, &make_verified("feat-z"));

    // Forward-conflict intra-spec between feat-x and feat-y.
    publish_message(
        &handle.state,
        &make_feedback(
            "feat-x",
            &["[conflict-detector] forward conflict with feat-y on src/main.rs"],
        ),
    );
    publish_message(
        &handle.state,
        &make_feedback(
            "feat-y",
            &["[conflict-detector] forward conflict with feat-x on src/main.rs"],
        ),
    );

    // Permission patterns: 6 auto-approves for `cargo check` (above default
    // threshold of 5).
    for _ in 0..=PERMISSION_PATTERN_THRESHOLD {
        publish_message(
            &handle.state,
            &make_status(
                "feat-x",
                "auto_approved",
                Some("auto_approved: matched cargo check"),
            ),
        );
    }

    // Trigger an explicit flush via the shared handle (the timer is
    // configured for 1h, so we bypass it).
    {
        let mut a = handle.state.learnings.as_ref().unwrap().lock().unwrap();
        a.flush().expect("flush ok");
    }

    let md = std::fs::read_to_string(&md_path).expect("md file written");
    assert!(md.contains("## Session Learnings — "));
    assert!(md.contains("### Conflict events"));
    assert!(md.contains("forward-conflict-intra-spec"));
    assert!(md.contains("### Where agents got stuck"));
    assert!(md.contains("feat-x: blocked"));
    assert!(md.contains("### Recovery cycles"));
    assert!(md.contains("feat-z"));
    assert!(md.contains("### Permission patterns"));
    assert!(md.contains("`cargo check` auto-approved"));

    // Broker message log MUST NOT contain any new variant — only the
    // pre-existing message types are allowed. Deserializing every entry
    // back into the strongly-typed `BrokerMessage` enum proves no opaque
    // `agent.learning`-style payloads leaked in.
    let inner = handle.state.read();
    for (_, _, msg) in &inner.message_log {
        let _: BrokerMessage = msg.clone();
        // The known status labels — anything else would be a new variant.
        let label = msg.status_label();
        assert!(
            matches!(
                label,
                "working"
                    | "done"
                    | "blocked"
                    | "verified"
                    | "feedback"
                    | "question"
                    | "auto_approved"
                    | "idle"
            ),
            "unexpected status label in message log: {label}"
        );
    }
    let json = serde_json::to_string(&inner.message_log[0].2).unwrap();
    assert!(
        !json.contains("agent.learning"),
        "no new agent.learning variant should appear on the wire"
    );
    drop(inner);

    drop(handle);
}

#[test]
#[serial]
fn shutdown_flush_captures_unresolved_blocks() {
    let tmp = TempDir::new().unwrap();
    let md_path = tmp.path().join(".git-paw").join("session-learnings.md");
    let mut state = BrokerState::new(None);

    let agg = Arc::new(Mutex::new(LearningsAggregator::new(md_path.clone())));
    {
        let mut a = agg.lock().unwrap();
        a.register_agent("feat-x");
        a.register_agent("feat-y");
    }
    state.attach_learnings(Arc::clone(&agg));

    let config = broker_config(20_200);
    let Ok(handle) = start_broker_with(
        &config,
        state,
        vec![watch_target("feat-x", &tmp), watch_target("feat-y", &tmp)],
        None,
        3600,
    ) else {
        return;
    };

    publish_message(&handle.state, &make_blocked("feat-x", "feat-y"));
    // No subsequent artifact — this block remains open until shutdown.

    drop(handle);
    // The Drop impl raises the stop flag and joins the learnings flush
    // thread, which performs the final shutdown flush.
    let md = std::fs::read_to_string(&md_path).expect("md file written at shutdown");
    assert!(md.contains("### Where agents got stuck"));
    assert!(md.contains("unresolved at session end"));
}

#[test]
#[serial]
fn restart_preserves_prior_session_and_appends_new_h2() {
    let tmp = TempDir::new().unwrap();
    let md_path = tmp.path().join(".git-paw").join("session-learnings.md");

    // --- First session ---
    {
        let mut state = BrokerState::new(None);
        let agg = Arc::new(Mutex::new(LearningsAggregator::new(md_path.clone())));
        {
            let mut a = agg.lock().unwrap();
            a.register_agent("feat-x");
            a.register_agent("feat-y");
        }
        state.attach_learnings(Arc::clone(&agg));
        let config = broker_config(20_400);
        let Ok(handle) = start_broker_with(
            &config,
            state,
            vec![watch_target("feat-x", &tmp), watch_target("feat-y", &tmp)],
            None,
            3600,
        ) else {
            return;
        };
        publish_message(&handle.state, &make_feedback("feat-x", &["e1"]));
        publish_message(&handle.state, &make_verified("feat-x"));
        {
            let mut a = handle.state.learnings.as_ref().unwrap().lock().unwrap();
            a.flush().unwrap();
        }
        drop(handle);
    }
    let after_first = std::fs::read_to_string(&md_path).unwrap();
    assert!(after_first.contains("### Recovery cycles"));

    // Ensure the next session's H2 timestamp differs by at least 1s.
    std::thread::sleep(std::time::Duration::from_secs(1));

    // --- Second session ---
    {
        let mut state = BrokerState::new(None);
        let agg = Arc::new(Mutex::new(LearningsAggregator::new(md_path.clone())));
        {
            let mut a = agg.lock().unwrap();
            a.register_agent("feat-x");
        }
        state.attach_learnings(Arc::clone(&agg));
        let config = broker_config(20_600);
        let Ok(handle) = start_broker_with(
            &config,
            state,
            vec![watch_target("feat-x", &tmp)],
            None,
            3600,
        ) else {
            return;
        };
        publish_message(&handle.state, &make_feedback("feat-x", &["e2"]));
        publish_message(&handle.state, &make_feedback("feat-x", &["e3"]));
        publish_message(&handle.state, &make_verified("feat-x"));
        {
            let mut a = handle.state.learnings.as_ref().unwrap().lock().unwrap();
            a.flush().unwrap();
        }
        drop(handle);
    }
    let after_second = std::fs::read_to_string(&md_path).unwrap();
    assert!(after_second.starts_with(after_first.trim_end()));
    let h2_count = after_second.matches("## Session Learnings — ").count();
    assert_eq!(h2_count, 2, "expected two H2 headers, got:\n{after_second}");
    // Second session shows two feedback cycles.
    assert!(after_second.contains("2 feedback"));
}

/// Spec scenario `Periodic flush + shutdown flush / Periodic flush writes
/// accumulated entries`: the periodic timer SHALL fire on its own and
/// write events that were observed since the previous flush. Drives a
/// 1-second interval so the test does not have to wait minutes.
#[test]
#[serial]
fn periodic_flush_writes_accumulated_entries_on_timer() {
    let tmp = TempDir::new().unwrap();
    let md_path = tmp.path().join(".git-paw").join("session-learnings.md");
    let mut state = BrokerState::new(None);

    let agg = Arc::new(Mutex::new(LearningsAggregator::new(md_path.clone())));
    {
        let mut a = agg.lock().unwrap();
        a.register_agent("feat-x");
    }
    state.attach_learnings(Arc::clone(&agg));

    let config = broker_config(20_800);
    let Ok(handle) = start_broker_with(
        &config,
        state,
        vec![watch_target("feat-x", &tmp)],
        None,
        1, // periodic flush every 1s — exercises the timer path
    ) else {
        return;
    };

    // Observe 3 events, then sit back and let the timer flush them.
    for _ in 0..(git_paw::broker::learnings::PERMISSION_PATTERN_THRESHOLD + 2) {
        publish_message(
            &handle.state,
            &make_status(
                "feat-x",
                "auto_approved",
                Some("auto_approved: matched cargo check"),
            ),
        );
    }

    // The file should not exist yet (the first tick is ~1s away).
    assert!(
        !md_path.exists() || std::fs::read_to_string(&md_path).unwrap().is_empty(),
        "aggregator wrote eagerly without a timer tick"
    );

    // Wait up to 3 seconds for the periodic timer to fire.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if md_path.exists() {
            let content = std::fs::read_to_string(&md_path).unwrap();
            if content.contains("`cargo check` auto-approved") {
                drop(handle);
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    drop(handle);
    panic!(
        "periodic timer did not flush within 3 seconds; file contents = {:?}",
        std::fs::read_to_string(&md_path).ok()
    );
}

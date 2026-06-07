//! Multi-session E2E for the learnings aggregator.
//!
//! Two sequential broker sessions write to the same
//! `.git-paw/session-learnings.md` file. Session 1 emits events that drive
//! all four deterministic categories (conflict events, stuck-duration,
//! recovery cycles, permission patterns) plus an unresolved-block bullet
//! captured by the shutdown flush. Session 2 emits a single event and
//! shuts down.
//!
//! Asserts:
//!
//! - The markdown file contains exactly two `## Session Learnings — ` H2
//!   headings (one per session).
//! - Session 1's content contains all four deterministic H3 sections and
//!   an `unresolved at session end` bullet.
//!
//! Maps to scenario `Multi-session H2 append + all-five-categories
//! round-trip` from `learnings-mode`. (test-coverage-v0-5-0 tasks 5.3 + 5.4)

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

fn watch_target(agent: &str, tmp: &TempDir) -> WatchTarget {
    WatchTarget {
        agent_id: agent.to_string(),
        cli: "claude".to_string(),
        worktree_path: tmp.path().to_path_buf(),
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

#[test]
#[serial]
fn multi_session_appends_h2_with_all_categories() {
    let tmp = TempDir::new().unwrap();
    let md_path = tmp.path().join(".git-paw").join("session-learnings.md");

    // --- Session 1: trigger all four deterministic categories plus one
    // unresolved block (captured by the shutdown flush as the fifth
    // category bullet).
    {
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

        let config = broker_config(22_000);
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
            // Port collision — skip without failing CI.
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

        // Forward-conflict (Conflict events category) intra-spec between feat-x
        // and feat-y.
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

        // Permission patterns: enough auto-approves to cross the default
        // threshold.
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

        // Fifth-category seed: register a block that has no subsequent
        // artifact in this session — the shutdown flush MUST capture it
        // under the `### Where agents got stuck` H3 with the
        // `unresolved at session end` annotation.
        publish_message(&handle.state, &make_blocked("feat-y", "feat-z"));

        drop(handle);
        // Drop joins the flush thread which runs the shutdown flush.
    }

    let after_first = std::fs::read_to_string(&md_path).expect("file written by session 1");
    for needle in [
        "## Session Learnings — ",
        "### Conflict events",
        "### Where agents got stuck",
        "### Recovery cycles",
        "### Permission patterns",
        "unresolved at session end",
    ] {
        assert!(
            after_first.contains(needle),
            "session 1 output should contain `{needle}`; got:\n{after_first}"
        );
    }
    let h2_after_first = after_first.matches("## Session Learnings — ").count();
    assert_eq!(
        h2_after_first, 1,
        "session 1 should write exactly one H2; got:\n{after_first}"
    );

    // Make sure session 2's H2 timestamp differs from session 1's by at
    // least 1s so the resulting file has two distinct headings even though
    // the human-readable timestamps render with seconds resolution.
    std::thread::sleep(std::time::Duration::from_secs(1));

    // --- Session 2: a single event and shutdown.
    {
        let mut state = BrokerState::new(None);
        let agg = Arc::new(Mutex::new(LearningsAggregator::new(md_path.clone())));
        {
            let mut a = agg.lock().unwrap();
            a.register_agent("feat-x");
        }
        state.attach_learnings(Arc::clone(&agg));

        let config = broker_config(22_200);
        let Ok(handle) = start_broker_with(
            &config,
            state,
            vec![watch_target("feat-x", &tmp)],
            None,
            3600,
        ) else {
            return;
        };

        publish_message(&handle.state, &make_feedback("feat-x", &["e1"]));
        // Trigger a flush so session 2 has at least one H2 of its own.
        {
            let mut a = handle.state.learnings.as_ref().unwrap().lock().unwrap();
            a.flush().unwrap();
        }
        drop(handle);
    }

    let after_second = std::fs::read_to_string(&md_path).expect("file written by session 2");
    let h2_count = after_second.matches("## Session Learnings — ").count();
    assert_eq!(
        h2_count, 2,
        "expected two H2 headings (one per session); got:\n{after_second}"
    );
    assert!(
        after_second.starts_with(after_first.trim_end()),
        "session 2 must append, not rewrite, the prior session's content"
    );
}

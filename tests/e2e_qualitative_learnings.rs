//! E2E for qualitative learnings ingestion + file rendering.
//!
//! The supervisor LLM publishes `agent.learning` records with the four
//! qualitative categories through the existing broker publish path. The
//! aggregator observes them and routes each into its own section of
//! `.git-paw/session-learnings.md`, while never re-publishing them (they
//! already came from the broker).
//!
//! Maps to the `qualitative-learnings` capability scenarios:
//! - `A recurring_failure_shape record appears under its section`
//! - `Each new category has its own section`
//! - within-session dedup (`Skill prose names the primary identifier ...`,
//!   reinforced code-side)
//! - file-only behaviour with no broker re-publish.
//!
//! Tasks 6.1, 6.2, 6.3.

use std::sync::{Arc, Mutex};

use serial_test::serial;
use tempfile::TempDir;

use git_paw::broker::delivery::{poll_messages, publish_message};
use git_paw::broker::learnings::{
    CATEGORY_ADR_DRIFT, CATEGORY_DOC_GAP, CATEGORY_RECURRING_FAILURE_SHAPE, CATEGORY_SCOPE_MISTAKE,
    LearningsAggregator,
};
use git_paw::broker::messages::{BrokerMessage, LearningPayload};
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

/// Builds an externally-published `agent.learning` envelope — the shape the
/// supervisor publishes via `POST /publish`. `id` and `timestamp` are
/// supplied because the broker's strict deserialiser requires them.
fn learning(id: &str, category: &str, title: &str, body: serde_json::Value) -> BrokerMessage {
    BrokerMessage::Learning {
        payload: LearningPayload {
            id: id.to_string(),
            agent_id: "supervisor".to_string(),
            branch_id: None,
            category: category.to_string(),
            title: title.to_string(),
            body,
            timestamp: "2026-06-05T14:32:00Z".to_string(),
        },
    }
}

fn one_of_each() -> Vec<BrokerMessage> {
    vec![
        learning(
            "rfs0000000000001",
            CATEGORY_RECURRING_FAILURE_SHAPE,
            "import cycle recurs across two branches",
            serde_json::json!({
                "shape": "import cycle between payments and billing modules",
                "instances": [
                    {"branch_id": "feat/a", "feedback_id": "f1", "excerpt": "cycle a"},
                    {"branch_id": "feat/b", "feedback_id": "f2", "excerpt": "cycle b"}
                ]
            }),
        ),
        learning(
            "dg00000000000001",
            CATEGORY_DOC_GAP,
            "lint-before-commit convention undocumented",
            serde_json::json!({
                "convention": "agents run the linter before committing",
                "evidence_paths": ["AGENTS.md"],
                "suggestion": "add a Conventions section to AGENTS.md"
            }),
        ),
        learning(
            "adr0000000000001",
            CATEGORY_ADR_DRIFT,
            "message queue dependency lacks an ADR",
            serde_json::json!({
                "decision_area": "background job scheduling",
                "observed_pattern": "a message-queue dependency added in the worker service",
                "configured_adr_path": "docs/adr",
                "candidate_adr_title": "ADR-0007: Adopt a message queue"
            }),
        ),
        learning(
            "sm00000000000001",
            CATEGORY_SCOPE_MISTAKE,
            "two branches over-coordinated on one handler",
            serde_json::json!({
                "branches": ["feat/a", "feat/b"],
                "shared_files": ["src/payments/handler"],
                "coordination_events": ["two feedback exchanges about ownership"],
                "suggestion": "merge the feat/a and feat/b scopes"
            }),
        ),
    ]
}

const FOUR_SECTIONS: [&str; 4] = [
    "### Recurring failure shapes",
    "### Documentation gaps",
    "### ADR / architectural drift",
    "### Scope-mistake signals",
];

// Task 6.1: broker on, one of each category — the broker receives four
// records (routed to the supervisor inbox, since they are cross-cutting) and
// the rendered file gains the four new sections.
#[test]
#[serial]
fn broker_on_routes_four_categories_to_four_sections() {
    let tmp = TempDir::new().unwrap();
    let md_path = tmp.path().join(".git-paw").join("session-learnings.md");

    let mut state = BrokerState::new(None);
    let agg = Arc::new(Mutex::new(LearningsAggregator::new(md_path.clone())));
    {
        let mut a = agg.lock().unwrap();
        a.set_broker_publish(true);
    }
    state.attach_learnings(Arc::clone(&agg));

    let config = broker_config(23_000);
    let Ok(handle) = start_broker_with(
        &config,
        state,
        vec![watch_target("feat-x", &tmp)],
        None,
        3600,
    ) else {
        // Port collision — skip without failing CI.
        return;
    };

    for msg in one_of_each() {
        publish_message(&handle.state, &msg);
    }

    // The broker received all four records; cross-cutting learnings (no
    // branch_id) land in the supervisor inbox.
    let (msgs, _) = poll_messages(&handle.state, "supervisor", 0);
    let learning_count = msgs
        .iter()
        .filter(|m| matches!(m, BrokerMessage::Learning { .. }))
        .count();
    assert_eq!(
        learning_count, 4,
        "broker should have received four agent.learning records; got {learning_count}"
    );

    // Flush and assert the four new sections rendered. No qualitative record
    // is ever re-queued for publish.
    {
        let mut a = handle.state.learnings.as_ref().unwrap().lock().unwrap();
        a.flush().unwrap();
        assert!(
            a.take_pending_publish().is_empty(),
            "ingested qualitative records must not be re-published"
        );
    }
    drop(handle);

    let md = std::fs::read_to_string(&md_path).expect("file written");
    for section in FOUR_SECTIONS {
        assert!(md.contains(section), "missing `{section}` in:\n{md}");
    }
    // Structured rendering landed, not just the headers.
    assert!(md.contains(
        "import cycle between payments and billing modules: 2 instances across feat/a, feat/b"
    ));
    assert!(md.contains(
        "- agents run the linter before committing — add a Conventions section to AGENTS.md"
    ));
    assert!(md.contains("- feat/a and feat/b — merge the feat/a and feat/b scopes"));
}

// Task 6.2: the same recurring_failure_shape published twice in a session is
// rendered once. The broker accepts both publishes; the aggregator's file
// dedup (same category + same `shape`) collapses them.
#[test]
#[serial]
fn dedup_same_recurring_shape_renders_once() {
    let tmp = TempDir::new().unwrap();
    let md_path = tmp.path().join(".git-paw").join("session-learnings.md");

    let mut state = BrokerState::new(None);
    let agg = Arc::new(Mutex::new(LearningsAggregator::new(md_path.clone())));
    state.attach_learnings(Arc::clone(&agg));

    let config = broker_config(23_200);
    let Ok(handle) = start_broker_with(
        &config,
        state,
        vec![watch_target("feat-x", &tmp)],
        None,
        3600,
    ) else {
        return;
    };

    let body = serde_json::json!({
        "shape": "import cycle between payments and billing modules",
        "instances": [{"branch_id": "feat/a"}, {"branch_id": "feat/b"}]
    });
    // Two publishes of the same shape with different ids + wording.
    publish_message(
        &handle.state,
        &learning(
            "dup0000000000001",
            CATEGORY_RECURRING_FAILURE_SHAPE,
            "first",
            body.clone(),
        ),
    );
    publish_message(
        &handle.state,
        &learning(
            "dup0000000000002",
            CATEGORY_RECURRING_FAILURE_SHAPE,
            "reworded",
            body,
        ),
    );

    {
        let mut a = handle.state.learnings.as_ref().unwrap().lock().unwrap();
        a.flush().unwrap();
    }
    drop(handle);

    let md = std::fs::read_to_string(&md_path).expect("file written");
    let occurrences = md
        .matches("import cycle between payments and billing modules")
        .count();
    assert_eq!(occurrences, 1, "shape rendered more than once:\n{md}");
}

// Task 6.3: with the aggregator in file-only mode (broker publish off), the
// same four-category scenario still renders the v0.5.0 sections + the four
// new ones, and the aggregator attempts NO broker publish.
#[test]
#[serial]
fn broker_off_file_only_still_renders_four_sections() {
    let tmp = TempDir::new().unwrap();
    let md_path = tmp.path().join(".git-paw").join("session-learnings.md");

    let mut state = BrokerState::new(None);
    let agg = Arc::new(Mutex::new(LearningsAggregator::new(md_path.clone())));
    {
        let mut a = agg.lock().unwrap();
        // File-only: the aggregator's dual-output is disabled (the default).
        assert!(!a.broker_publish_enabled());
        // A v0.5.0 deterministic signal so the file also carries a v0.5.0
        // section in the same session.
        for _ in 0..6 {
            a.record_auto_approve("git status");
        }
    }
    state.attach_learnings(Arc::clone(&agg));

    let config = broker_config(23_400);
    let Ok(handle) = start_broker_with(
        &config,
        state,
        vec![watch_target("feat-x", &tmp)],
        None,
        3600,
    ) else {
        return;
    };

    for msg in one_of_each() {
        publish_message(&handle.state, &msg);
    }

    {
        let mut a = handle.state.learnings.as_ref().unwrap().lock().unwrap();
        a.flush().unwrap();
        assert!(
            a.take_pending_publish().is_empty(),
            "no broker publish must be attempted in file-only mode"
        );
    }
    drop(handle);

    let md = std::fs::read_to_string(&md_path).expect("file written");
    // v0.5.0 deterministic section preserved.
    assert!(md.contains("### Permission patterns"), "{md}");
    assert!(md.contains("- `git status` auto-approved 6 times"), "{md}");
    // The four new sections present alongside it.
    for section in FOUR_SECTIONS {
        assert!(md.contains(section), "missing `{section}` in:\n{md}");
    }
}

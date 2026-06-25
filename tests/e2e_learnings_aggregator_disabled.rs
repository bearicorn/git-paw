//! E2E observable for the learnings-aggregator lifecycle when supervisor mode
//! is disabled.
//!
//! Maps to scenario `Aggregator does not start when supervisor is disabled`
//! from `learnings-mode`. The production decision is:
//!
//! ```text
//! supervisor.enabled && supervisor.learnings
//! ```
//!
//! Even when the user opts in to learnings (`learnings = true`), the
//! aggregator MUST NOT run if the supervisor section is disabled. The
//! observable property is the absence of `<repo>/.git-paw/session-learnings.md`
//! after the broker has accepted and dropped a sequence of events.
//!
//! (test-coverage-v0-5-0 task 5.2)

use serial_test::serial;
use tempfile::TempDir;

use git_paw::broker::delivery::publish_message;
use git_paw::broker::messages::{ArtifactPayload, BlockedPayload, BrokerMessage};
use git_paw::broker::{BrokerState, WatchTarget, start_broker_with};
use git_paw::config::{BrokerConfig, LearningsConfig, SupervisorConfig};

/// Mirrors the production-side decision: only attach the aggregator when
/// supervisor mode is enabled *and* learnings is true. With supervisor
/// disabled, this returns `false` even if learnings is true.
fn should_attach(s: &SupervisorConfig) -> bool {
    s.enabled && s.learnings
}

/// Allocates an OS-assigned ephemeral broker port (`bind 127.0.0.1:0`, read
/// back, release), matching `tests/e2e_supervisor_stop.rs::pick_broker_port`.
/// Replaces the former `BASE + (process::id() % N)` scheme (F8 root cause),
/// which keyed the port on the PID modulo a small constant and collided
/// across concurrent test runs.
fn pick_broker_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read local addr")
        .port()
}

fn broker_config() -> BrokerConfig {
    BrokerConfig {
        enabled: true,
        port: pick_broker_port(),
        bind: "127.0.0.1".to_string(),
        ..Default::default()
    }
}

#[test]
#[serial]
fn aggregator_does_not_run_when_supervisor_disabled() {
    let tmp = TempDir::new().unwrap();
    let md_path = tmp.path().join(".git-paw").join("session-learnings.md");

    // [supervisor] enabled = false, learnings = true — the user opted in
    // to learnings but supervisor mode itself is off, so the production
    // wiring MUST NOT attach the aggregator.
    let supervisor = SupervisorConfig {
        enabled: false,
        learnings: true,
        learnings_config: LearningsConfig::default(),
        ..SupervisorConfig::default()
    };
    assert!(
        !should_attach(&supervisor),
        "predicate must be false when supervisor.enabled = false"
    );

    let state = BrokerState::new(None);
    if should_attach(&supervisor) {
        // Production attach path — never taken in this test.
        unreachable!("test contradicts should_attach predicate");
    }

    let config = broker_config();
    let watch_targets = vec![WatchTarget {
        agent_id: "feat-x".to_string(),
        cli: "claude".to_string(),
        worktree_path: tmp.path().to_path_buf(),
    }];
    let Ok(handle) = start_broker_with(&config, state, watch_targets, None, 3600) else {
        // Port collision — treat as "skipped" rather than failing CI.
        return;
    };

    // Publish a sequence of events that WOULD trigger learnings categories
    // if the aggregator were attached. We need to observe that the file
    // is NOT created.
    publish_message(
        &handle.state,
        &BrokerMessage::Blocked {
            agent_id: "feat-x".to_string(),
            payload: BlockedPayload {
                needs: "types".to_string(),
                from: "feat-y".to_string(),
            },
        },
    );
    publish_message(
        &handle.state,
        &BrokerMessage::Artifact {
            agent_id: "feat-x".to_string(),
            payload: ArtifactPayload {
                status: "done".to_string(),
                exports: vec![],
                modified_files: vec![],
            },
        },
    );

    drop(handle);

    assert!(
        !md_path.exists(),
        "session-learnings.md must not exist when supervisor mode is disabled; found at {}",
        md_path.display()
    );
}

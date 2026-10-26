//! `git paw __broker` — the broker host.
//!
//! Hosts the session's broker in its own process, in a detached service
//! surface of the session (on tmux, a window named `broker`), so the broker
//! outlives — and is independent of — any view of it. It builds the broker
//! state (per-session log, verify-on-commit nudge, seeded orchestrator CLI,
//! role-gating context, learnings aggregator), starts the HTTP server with the
//! repository-identity probe, watchers and the conflict detector, and blocks
//! until shutdown. It renders nothing, needs no `$TMUX`, and sends no
//! keystrokes to any surface.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::broker;
use crate::config::{self, SupervisorConfig};
use crate::error::PawError;
use crate::git;
use crate::session::{self, Session};

/// How long one wait for a terminal hang-up lasts before the shutdown and
/// orphan checks run again.
const LIFECYCLE_TICK: Duration = Duration::from_millis(500);

/// Builds the broker host's learnings aggregator, seeded from the saved
/// session when there is one.
///
/// `session` is the receipt the host found under its session name. When it
/// is present, [`broker::learnings::LearningsAggregator::seed_from_session`]
/// registers every agent AND records the spec each was launched from —
/// seeding the roster alone leaves the agent → spec mapping empty, which
/// reports every forward conflict cross-spec even between two agents launched
/// from the same spec. On a fresh launch there is no receipt yet: the agents
/// and their spec ids arrive through `POST /watch` instead.
pub(crate) fn build_learnings_aggregator(
    learnings_path: PathBuf,
    supervisor: &SupervisorConfig,
    session: Option<&Session>,
    broker_enabled: bool,
) -> broker::learnings::LearningsAggregator {
    let mut aggregator = broker::learnings::LearningsAggregator::new(learnings_path);
    aggregator.set_broker_publish(
        supervisor
            .learnings_config
            .broker_publish
            .resolve(broker_enabled),
    );
    if let Some(session) = session {
        aggregator.seed_from_session(session);
    }
    aggregator
}

/// The saved session named `session_name`, when it exists AND belongs to
/// `repo_root`.
///
/// Session names are unique only among live terminal sessions, so a stopped
/// receipt of another repository with the same basename can carry this name;
/// seeding from it would bind that repository's port and watch its worktrees.
fn saved_session_for(
    session_name: &str,
    repo_root: &Path,
    dir: &Path,
) -> Result<Option<Session>, PawError> {
    Ok(session::load_session_from(session_name, dir)?
        .filter(|sess| session::session_belongs_to_repo(sess, repo_root)))
}

/// The watch targets of a saved session: one per worktree entry.
fn watch_targets_of(session: Option<&Session>) -> Vec<broker::WatchTarget> {
    session
        .map(|sess| {
            sess.worktrees
                .iter()
                .map(|wt| broker::WatchTarget {
                    agent_id: broker::messages::slugify_branch(&wt.branch),
                    cli: wt.cli.clone(),
                    worktree_path: wt.worktree_path.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Runs the broker host for session `session_name`, logging to `log_path`.
///
/// The launcher hands over the session name and log path because on a fresh
/// launch the session receipt is saved only after the surfaces exist. A
/// saved session with that name, when one exists (recovery, restart from
/// pause), seeds the watch targets and the agent → spec mapping.
///
/// # Errors
/// Returns an error when the repository or config cannot be read, or when the
/// broker cannot bind or probe its port — the host exits non-zero then
/// instead of retrying.
#[allow(clippy::too_many_lines)]
pub fn cmd_broker_host(session_name: &str, log_path: &Path) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;
    // A config that will not parse must not take the broker down with it: a
    // resume of a session whose config broke since launch still needs its
    // broker, and the saved session supplies the address. Defaults stand in,
    // as they do for the rest of a resume.
    let config = config::load_config(&repo_root, None).unwrap_or_else(|e| {
        eprintln!("git-paw broker: could not load config ({e}); continuing with defaults");
        config::PawConfig::default()
    });

    // The conflict detector subsystem runs only when supervisor mode is
    // enabled — its outputs flow through the supervisor inbox and are tagged
    // for the supervisor agent's attention.
    let conflict_cfg = config
        .supervisor
        .as_ref()
        .filter(|s| s.enabled)
        .map(|s| s.conflict.clone());

    // Learnings flush cadence (seconds), passed to start_broker_with so the
    // flush thread spawns with the user-configured cadence.
    let learnings_interval_seconds = config
        .supervisor
        .as_ref()
        .map_or(60, |s| s.learnings_config.flush_interval_seconds);

    // Per-commit verification nudge (default on; opt out with
    // `[supervisor] verify_on_commit_nudge = false`).
    let verify_on_commit_nudge = config
        .supervisor
        .as_ref()
        .is_none_or(SupervisorConfig::verify_on_commit_nudge_enabled);

    // Resolve the supervisor's CLI the same way the launcher did
    // (`[supervisor].cli` > `default_cli`) and seed it authoritatively: the
    // supervisor is not a filesystem watch target, so this is the only
    // deterministic source for its roster CLI column (W15-15).
    let supervisor_cli = config
        .supervisor
        .as_ref()
        .and_then(|s| s.cli.clone())
        .or_else(|| config.default_cli.clone())
        .unwrap_or_default();

    // A saved receipt under this name exists on recovery and restart from
    // pause; a fresh launch saves its receipt only after the surfaces exist,
    // and registers its agents through `POST /watch` instead.
    let saved_session = saved_session_for(session_name, &repo_root, &session::sessions_dir()?)?;
    let watch_targets = watch_targets_of(saved_session.as_ref());

    // A saved session's broker address wins over the current config: its
    // agents were handed that URL, which can differ from the config when
    // `[broker] port` changed since the session was saved.
    let mut broker_config = config.broker.clone();
    if let Some(sess) = saved_session.as_ref()
        && let (Some(port), Some(bind)) = (sess.broker_port, sess.broker_bind.as_ref())
    {
        broker_config.enabled = true;
        broker_config.port = port;
        broker_config.bind.clone_from(bind);
    }

    let mut broker_state = broker::BrokerState::new(Some(log_path.to_path_buf()))
        .with_repo_root(&repo_root)
        .with_verify_on_commit_nudge(verify_on_commit_nudge)
        .with_seeded_cli("supervisor", &supervisor_cli);

    // The opsx role-gating context, scoped to the OpenSpec engine. The roster
    // maps each known coding worktree plus the supervisor's repo root; an
    // agent registered later through `/watch` is unmatched, which the guard
    // treats exactly as a matched coding worktree.
    {
        let engine_is_openspec =
            crate::specs::resolved_spec_type(&config, &repo_root).as_deref() == Some("openspec");
        let mut roster: Vec<(String, PathBuf)> = watch_targets
            .iter()
            .map(|t| (t.agent_id.clone(), t.worktree_path.clone()))
            .collect();
        roster.push((
            crate::opsx::SUPERVISOR_AGENT_ID.to_string(),
            repo_root.clone(),
        ));
        broker_state = broker_state.with_role_gating(crate::opsx::RoleGatingContext {
            mode: config.role_gating_mode(),
            engine_is_openspec,
            roster,
        });
    }

    // The learnings aggregator, when supervisor mode + learnings are both
    // enabled.
    if let Some(sup) = config
        .supervisor
        .as_ref()
        .filter(|s| s.enabled && s.learnings)
    {
        let aggregator = build_learnings_aggregator(
            repo_root.join(".git-paw").join("session-learnings.md"),
            sup,
            saved_session.as_ref(),
            broker_config.enabled,
        );
        broker_state.attach_learnings(Arc::new(std::sync::Mutex::new(aggregator)));
    }

    // Ignore SIGINT before anything else: an accidental Ctrl+C in the
    // service surface must not take the broker down.
    ignore_sigint();

    // A bind/probe failure is terminal: emit a diagnostic and exit non-zero
    // rather than retry. The launcher's bounded `/status` wait reports it.
    let handle = match broker::start_broker_with(
        &broker_config,
        broker_state,
        watch_targets,
        conflict_cfg,
        learnings_interval_seconds,
    ) {
        Ok(handle) => handle,
        Err(err) => {
            eprintln!(
                "git-paw broker: failed to start on port {} ({err}); exiting",
                broker_config.port
            );
            return Err(err.into());
        }
    };
    // A broker for this repository already serving the port belongs to
    // another process: idling beside it would leave the launcher talking to a
    // broker this host can neither flush nor stop. Exit instead.
    if handle.is_reattached() {
        eprintln!(
            "git-paw broker: a broker for this repository already serves port {}; exiting",
            broker_config.port
        );
        return Err(PawError::SessionError(format!(
            "a broker for this repository already serves port {}",
            broker_config.port
        )));
    }
    eprintln!(
        "git-paw broker for session '{session_name}' serving {} (log: {})",
        handle.url,
        log_path.display()
    );

    // Set up a flag that SIGHUP sets to signal the host to exit gracefully.
    // tmux sends SIGHUP to a window's process when it kills the window or the
    // session. Without this, the process would be terminated before
    // BrokerHandle::drop runs, losing the final log flush.
    let shutdown = Arc::new(AtomicBool::new(false));

    #[cfg(unix)]
    {
        use std::sync::atomic::AtomicPtr;

        static SHUTDOWN_PTR: AtomicPtr<std::sync::atomic::AtomicBool> =
            AtomicPtr::new(std::ptr::null_mut());

        const SIGHUP: std::ffi::c_int = 1;

        unsafe extern "C" {
            fn signal(signum: std::ffi::c_int, handler: extern "C" fn(std::ffi::c_int)) -> usize;
        }

        extern "C" fn sighup_handler(_: std::ffi::c_int) {
            let ptr = SHUTDOWN_PTR.load(std::sync::atomic::Ordering::Acquire);
            if !ptr.is_null() {
                unsafe {
                    (*ptr).store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }

        // Store the flag pointer so the signal handler can access it.
        SHUTDOWN_PTR.store(
            std::sync::Arc::as_ptr(&shutdown).cast_mut(),
            std::sync::atomic::Ordering::Release,
        );

        // SAFETY: sighup_handler only sets an AtomicBool, which is
        // async-signal-safe. The pointer is valid for the process lifetime.
        unsafe {
            signal(SIGHUP, sighup_handler);
        }
    }

    // Block until shutdown: SIGHUP, reparented to init, or a controlling
    // terminal it was given hanging up. Each wait is bounded, so every
    // condition is re-checked at least once per tick.
    while !(shutdown.load(Ordering::Relaxed) || crate::dashboard::orphaned()) {
        if crate::dashboard::terminal_hung_up(LIFECYCLE_TICK) {
            break;
        }
    }

    // Dropping the handle runs the final broker-log flush and the learnings
    // shutdown flush.
    drop(handle);
    Ok(())
}

/// Makes `SIGINT` harmless to this process: it is caught by a handler that
/// does nothing, so it neither terminates the host nor triggers shutdown.
#[cfg(unix)]
fn ignore_sigint() {
    const SIGINT: std::ffi::c_int = 2;
    unsafe extern "C" {
        fn signal(signum: std::ffi::c_int, handler: extern "C" fn(std::ffi::c_int)) -> usize;
    }
    extern "C" fn sigint_handler(_: std::ffi::c_int) {}
    // SAFETY: the handler does nothing at all, which is async-signal-safe.
    unsafe {
        signal(SIGINT, sigint_handler);
    }
}

/// Non-unix stub: there is no `SIGINT` disposition to set.
#[cfg(not(unix))]
fn ignore_sigint() {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{SessionStatus, WorktreeEntry};
    use std::time::UNIX_EPOCH;
    use tempfile::TempDir;

    /// A session receipt whose worktree entries carry the given
    /// `(branch, spec_id)` pairs — the shape a spec-driven launch saves.
    fn receipt_with(entries: &[(&str, Option<&str>)]) -> Session {
        Session {
            session_name: "paw-repo".to_string(),
            repo_path: PathBuf::from("/repo"),
            project_name: "repo".to_string(),
            created_at: UNIX_EPOCH,
            status: SessionStatus::Active,
            worktrees: entries
                .iter()
                .map(|(branch, spec_id)| WorktreeEntry {
                    branch: (*branch).to_string(),
                    worktree_path: PathBuf::from(format!("/repo/wt/{branch}")),
                    cli: "claude".to_string(),
                    branch_created: false,
                    base_branch: None,
                    pending_boot_prompt: None,
                    pending_entry_point: None,
                    runtime_slot: None,
                    cli_session_id: None,
                    spec_id: spec_id.map(str::to_string),
                })
                .collect(),
            broker_port: None,
            broker_bind: None,
            broker_log_path: None,
            mode: crate::session::SessionMode::Bare,
            dashboard_pane: None,
            layout: None,
        }
    }

    /// A `[conflict-detector]`-tagged forward-conflict feedback message
    /// naming `from` and `other`, as the detector emits it.
    fn forward_conflict(from: &str, other: &str) -> broker::messages::BrokerMessage {
        broker::messages::BrokerMessage::Feedback {
            agent_id: from.to_string(),
            payload: broker::messages::FeedbackPayload {
                from: "conflict-detector".to_string(),
                errors: vec![format!(
                    "[conflict-detector] forward conflict with {other} on src/main.rs"
                )],
            },
        }
    }

    /// `supervisor-learnings`' "The spec mapping is seeded from the persisted
    /// session": the host's aggregator, built for a saved receipt, SHALL know
    /// the agent → spec mapping with no further call.
    ///
    /// This asserts the *wiring*, not the helper. `seed_from_session` has its
    /// own unit tests, but those would stay green if the host never called it.
    #[test]
    fn dashboard_aggregator_is_seeded_from_the_session_receipt() {
        let tmp = TempDir::new().expect("tempdir");
        let receipt = receipt_with(&[
            ("feat/x", Some("003-user-list")),
            ("feat/y", Some("003-user-list")),
        ]);
        let mut aggregator = build_learnings_aggregator(
            tmp.path().join("session-learnings.md"),
            &SupervisorConfig::default(),
            Some(&receipt),
            false,
        );

        aggregator.record_detector_message(&forward_conflict("feat-x", "feat-y"));
        aggregator.record_detector_message(&forward_conflict("feat-y", "feat-x"));
        aggregator.flush().expect("flush learnings");

        let md = std::fs::read_to_string(aggregator.file_path()).expect("learnings file written");
        assert!(
            md.contains("forward-conflict-intra-spec"),
            "the host must seed the agent -> spec mapping from the receipt, so a \
             same-spec forward conflict classifies intra-spec; got:\n{md}"
        );
        assert!(
            md.contains("003-user-list"),
            "the flushed bullet must name the spec both agents were launched from; got:\n{md}"
        );
    }

    /// No receipt means nothing to seed: the aggregator is still built and
    /// still classifies, falling back to cross-spec as it did before.
    #[test]
    fn dashboard_aggregator_without_a_receipt_falls_back_to_cross_spec() {
        let tmp = TempDir::new().expect("tempdir");
        let mut aggregator = build_learnings_aggregator(
            tmp.path().join("session-learnings.md"),
            &SupervisorConfig::default(),
            None,
            false,
        );

        aggregator.record_detector_message(&forward_conflict("feat-x", "feat-y"));
        aggregator.record_detector_message(&forward_conflict("feat-y", "feat-x"));
        aggregator.flush().expect("flush learnings");

        let md = std::fs::read_to_string(aggregator.file_path()).expect("learnings file written");
        assert!(
            !md.contains("forward-conflict-intra-spec"),
            "with no receipt there is no mapping, so nothing may classify intra-spec; got:\n{md}"
        );
    }

    /// A receipt with the host's session name but another repository's path
    /// is not the host's: it seeds nothing.
    #[test]
    fn a_same_named_receipt_of_another_repository_is_ignored() {
        let dir = TempDir::new().expect("tempdir");
        let ours = TempDir::new().expect("our repo");
        let theirs = TempDir::new().expect("their repo");
        let mut receipt = receipt_with(&[("feat/x", None)]);
        receipt.repo_path = theirs.path().to_path_buf();
        session::save_session_in(&receipt, dir.path()).expect("save");

        assert!(
            saved_session_for("paw-repo", ours.path(), dir.path())
                .expect("load")
                .is_none()
        );
        assert!(
            saved_session_for("paw-repo", theirs.path(), dir.path())
                .expect("load")
                .is_some()
        );
    }

    /// A saved receipt yields one watch target per worktree, keyed by the
    /// slugified branch; no receipt yields none.
    #[test]
    fn watch_targets_come_from_the_saved_receipt_only() {
        let receipt = receipt_with(&[("feat/x", None), ("feat/y", Some("003"))]);
        let targets = watch_targets_of(Some(&receipt));
        let ids: Vec<&str> = targets.iter().map(|t| t.agent_id.as_str()).collect();
        assert_eq!(ids, ["feat-x", "feat-y"]);
        assert!(watch_targets_of(None).is_empty());
    }
}

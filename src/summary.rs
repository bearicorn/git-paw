//! Session summary generation.
//!
//! Produces a human-readable Markdown summary of a completed supervisor
//! session, sourcing data from [`BrokerState`] (per-agent records and the
//! message log) and [`Session`] (project metadata). Written once at the end
//! of the session to `.git-paw/session-summary.md`.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::broker::{BrokerMessage, BrokerState};
use crate::error::PawError;
use crate::session::Session;

/// Result of running a test command for a single agent.
#[derive(Clone, Debug)]
pub struct TestResult {
    pub success: bool,
    pub output: String,
}

/// Writes a Markdown session summary to `output_path`.
///
/// Sources per-agent details (status, modified files, exports, blocked time)
/// from `state` and project metadata (name, start time) from `session`. The
/// `merge_order` parameter provides the sequence in which feature branches
/// were merged. The `test_results` parameter provides test execution results
/// for each agent.
///
/// Overwrites any existing file at `output_path`. Returns
/// [`PawError::SessionError`] if the write fails.
pub fn write_session_summary<S: std::hash::BuildHasher>(
    state: &BrokerState,
    session: &Session,
    merge_order: &[String],
    test_results: &std::collections::HashMap<String, TestResult, S>,
    output_path: &Path,
) -> Result<(), PawError> {
    let now = SystemTime::now();
    let total_duration = now
        .duration_since(session.created_at)
        .unwrap_or(Duration::ZERO);

    let inner = state.read();

    // Map slugified branch -> cli for per-agent header lookup.
    let cli_by_slug: HashMap<String, String> = session
        .worktrees
        .iter()
        .map(|wt| {
            (
                crate::broker::messages::slugify_branch(&wt.branch),
                wt.cli.clone(),
            )
        })
        .collect();

    let mut agent_ids: Vec<&String> = inner.agents.keys().collect();
    agent_ids.sort();

    let date = format_date(session.created_at);
    let mut out = String::new();

    let _ = writeln!(
        out,
        "# Session Summary \u{2014} {} \u{2014} {date}\n",
        session.project_name
    );

    // Overview section
    out.push_str("## Overview\n");
    let _ = writeln!(out, "- **Duration:** {}", format_duration(total_duration));
    let _ = writeln!(out, "- **Agents:** {}", agent_ids.len());
    let merge_list = if merge_order.is_empty() {
        "(none)".to_string()
    } else {
        merge_order.join(", ")
    };
    let _ = writeln!(out, "- **Merge order:** {merge_list}\n");

    // Per-agent section
    out.push_str("## Agents\n\n");
    for agent_id in &agent_ids {
        let record = &inner.agents[*agent_id];
        let cli = cli_by_slug.get(*agent_id).map_or("unknown", String::as_str);

        let _ = writeln!(out, "### {agent_id} ({cli})");
        let _ = writeln!(out, "- **Status:** {}", record.status);

        let (files, exports) = last_artifact_fields(&inner.message_log, agent_id);
        let _ = writeln!(
            out,
            "- **Files modified:** {}",
            format_list(files.as_deref())
        );
        let _ = writeln!(out, "- **Exports:** {}", format_list(exports.as_deref()));

        let blocked = estimated_blocked_time(&inner.message_log, agent_id);
        let blocked_str = if blocked.is_zero() {
            "none".to_string()
        } else {
            format_duration(blocked)
        };
        let _ = writeln!(out, "- **Estimated blocked time:** {blocked_str}\n");
    }

    // Totals section
    out.push_str("## Totals\n");
    let _ = writeln!(out, "- Total agents: {}", agent_ids.len());
    let _ = writeln!(out, "- Total time: {}", format_duration(total_duration));

    // Test results section
    if !test_results.is_empty() {
        out.push_str("\n## Test Results\n");
        for (branch, result) in test_results {
            let status = if result.success {
                "✓ PASS"
            } else {
                "✗ FAIL"
            };
            let _ = writeln!(out, "- **{branch}**: {status}");
            if !result.output.is_empty() {
                let _ = writeln!(out, "  ```\n{}\n  ```", result.output);
            }
        }
    }

    drop(inner);

    fs::write(output_path, out).map_err(|e| {
        PawError::SessionError(format!(
            "failed to write session summary to {}: {e}",
            output_path.display()
        ))
    })
}

/// Convenience wrapper that writes the session summary to a timestamped
/// file under `<repo_root>/.git-paw/sessions/<UTC-timestamp>.md`, creating
/// `.git-paw/sessions/` if it does not already exist.
///
/// Each supervisor run produces a fresh file so multiple sessions for the
/// same repo can coexist on disk without overwriting each other. The
/// timestamp format is `YYYY-MM-DDTHH-MM-SSZ` (filesystem-safe — colons in
/// ISO 8601 are replaced with hyphens) and is taken from the system clock
/// at write time.
///
/// Returns the path of the file that was written so callers can log it.
pub fn write_supervisor_summary<S: std::hash::BuildHasher>(
    state: &BrokerState,
    session: &Session,
    merge_order: &[String],
    test_results: &std::collections::HashMap<String, TestResult, S>,
    repo_root: &Path,
) -> Result<PathBuf, PawError> {
    let dir = repo_root.join(".git-paw").join("sessions");
    fs::create_dir_all(&dir).map_err(|e| {
        PawError::SessionError(format!(
            "failed to create {} for session summary: {e}",
            dir.display()
        ))
    })?;
    let filename = format!("{}.md", filesystem_safe_utc_timestamp());
    let path = dir.join(&filename);
    write_session_summary(state, session, merge_order, test_results, &path)?;
    Ok(path)
}

/// Returns the current UTC time in `YYYY-MM-DDTHH-MM-SSZ` format. Colons
/// in canonical ISO 8601 are replaced with hyphens so the string is safe
/// to use as a filename on every platform git-paw supports (macOS / Linux
/// / WSL).
fn filesystem_safe_utc_timestamp() -> String {
    use chrono::{SecondsFormat, Utc};
    let iso = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    iso.replace(':', "-")
}

/// Returns `(modified_files, exports)` from the most recent `agent.artifact`
/// message published by `agent_id`. Returns `(None, None)` if no artifact
/// message exists for that agent.
fn last_artifact_fields(
    log: &[(u64, SystemTime, BrokerMessage)],
    agent_id: &str,
) -> (Option<Vec<String>>, Option<Vec<String>>) {
    for (_seq, _ts, msg) in log.iter().rev() {
        if let BrokerMessage::Artifact {
            agent_id: id,
            payload,
        } = msg
            && id == agent_id
        {
            return (
                Some(payload.modified_files.clone()),
                Some(payload.exports.clone()),
            );
        }
    }
    (None, None)
}

/// Sums the time gaps between each `agent.blocked` message and the next
/// `agent.status` or `agent.artifact` message from the same agent.
fn estimated_blocked_time(log: &[(u64, SystemTime, BrokerMessage)], agent_id: &str) -> Duration {
    let mut total = Duration::ZERO;
    let mut blocked_at: Option<SystemTime> = None;

    for (_seq, ts, msg) in log {
        if msg.agent_id() != agent_id {
            continue;
        }
        match msg {
            BrokerMessage::Blocked { .. } if blocked_at.is_none() => {
                blocked_at = Some(*ts);
            }
            BrokerMessage::Status { .. } | BrokerMessage::Artifact { .. } => {
                if let Some(start) = blocked_at.take()
                    && let Ok(gap) = ts.duration_since(start)
                {
                    total += gap;
                }
            }
            _ => {}
        }
    }
    total
}

/// Formats a list as a comma-separated string, or `(none)` if empty/missing.
fn format_list(items: Option<&[String]>) -> String {
    match items {
        Some(list) if !list.is_empty() => list.join(", "),
        _ => "(none)".to_string(),
    }
}

/// Formats a duration as `XmYs` style (e.g. `2m 13s`, `45s`, `1h 5m`).
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

/// Formats a `SystemTime` as `YYYY-MM-DD` (UTC) using the same civil-date
/// algorithm as `session.rs`.
fn format_date(time: SystemTime) -> String {
    let secs = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    // Algorithm from Howard Hinnant (matches session.rs).
    #[allow(clippy::cast_possible_wrap)]
    let mut days = (secs / 86400) as i64;
    days += 719_468;
    let era = days.div_euclid(146_097);
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::messages::{ArtifactPayload, StatusPayload};
    use crate::session::{SessionStatus, WorktreeEntry};
    use std::path::PathBuf;
    use std::time::UNIX_EPOCH;
    use tempfile::TempDir;

    fn sample_session() -> Session {
        Session {
            session_name: "paw-demo".to_string(),
            repo_path: PathBuf::from("/tmp/demo"),
            project_name: "demo".to_string(),
            // Fixed unix epoch (2024-03-23 13:20:00 UTC); seconds is the
            // canonical unit for unix timestamps so the literal stays
            // human-readable as a date.
            #[allow(clippy::duration_suboptimal_units)]
            created_at: UNIX_EPOCH + Duration::from_secs(1_711_200_000),
            status: SessionStatus::Active,
            worktrees: vec![
                WorktreeEntry {
                    branch: "feat/config".to_string(),
                    worktree_path: PathBuf::from("/tmp/demo-feat-config"),
                    cli: "claude".to_string(),
                    branch_created: true,
                },
                WorktreeEntry {
                    branch: "feat/errors".to_string(),
                    worktree_path: PathBuf::from("/tmp/demo-feat-errors"),
                    cli: "gemini".to_string(),
                    branch_created: true,
                },
            ],
            broker_port: None,
            broker_bind: None,
            broker_log_path: None,
        }
    }

    fn populate_state(state: &BrokerState, agent_id: &str, status: &str) {
        use crate::broker::AgentRecord;
        use std::time::Instant;

        let mut inner = state.write();
        inner.agents.insert(
            agent_id.to_string(),
            AgentRecord {
                agent_id: agent_id.to_string(),
                status: status.to_string(),
                last_seen: Instant::now(),
                last_message: None,
            },
        );
    }

    fn push_log(state: &BrokerState, seq: u64, ts: SystemTime, msg: BrokerMessage) {
        let mut inner = state.write();
        inner.message_log.push((seq, ts, msg));
    }

    #[test]
    fn writes_file_at_specified_path() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session-summary.md");

        let state = BrokerState::new(None);
        populate_state(&state, "feat-config", "verified");
        populate_state(&state, "feat-errors", "verified");

        let session = sample_session();
        write_session_summary(
            &state,
            &session,
            &[],
            &std::collections::HashMap::new(),
            &path,
        )
        .unwrap();
        assert!(path.exists());
    }

    #[test]
    fn output_contains_project_name() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.md");
        let state = BrokerState::new(None);
        let session = sample_session();
        write_session_summary(
            &state,
            &session,
            &[],
            &std::collections::HashMap::new(),
            &path,
        )
        .unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("demo"));
    }

    #[test]
    fn output_contains_agent_count() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.md");

        let state = BrokerState::new(None);
        populate_state(&state, "a", "verified");
        populate_state(&state, "b", "verified");
        populate_state(&state, "c", "verified");

        let session = sample_session();
        write_session_summary(
            &state,
            &session,
            &[],
            &std::collections::HashMap::new(),
            &path,
        )
        .unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("**Agents:** 3"));
    }

    #[test]
    fn output_lists_merge_order_in_sequence() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.md");
        let state = BrokerState::new(None);
        let session = sample_session();
        let merge_order = vec![
            "feat-errors".to_string(),
            "feat-config".to_string(),
            "feat-detect".to_string(),
        ];
        write_session_summary(
            &state,
            &session,
            &merge_order,
            &std::collections::HashMap::new(),
            &path,
        )
        .unwrap();
        let content = fs::read_to_string(&path).unwrap();
        let line = content
            .lines()
            .find(|l| l.contains("Merge order"))
            .expect("merge order line");
        let errors_pos = line.find("feat-errors").unwrap();
        let config_pos = line.find("feat-config").unwrap();
        let detect_pos = line.find("feat-detect").unwrap();
        assert!(errors_pos < config_pos);
        assert!(config_pos < detect_pos);
    }

    #[test]
    fn agent_section_shows_none_when_no_artifact() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.md");
        let state = BrokerState::new(None);
        populate_state(&state, "feat-config", "working");
        let session = sample_session();
        write_session_summary(
            &state,
            &session,
            &[],
            &std::collections::HashMap::new(),
            &path,
        )
        .unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("**Files modified:** (none)"));
        assert!(content.contains("**Exports:** (none)"));
    }

    #[test]
    fn agent_section_shows_modified_files_from_last_artifact() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.md");
        let state = BrokerState::new(None);
        populate_state(&state, "feat-config", "verified");

        push_log(
            &state,
            1,
            SystemTime::now(),
            BrokerMessage::Artifact {
                agent_id: "feat-config".to_string(),
                payload: ArtifactPayload {
                    status: "done".to_string(),
                    exports: vec!["SupervisorConfig".to_string()],
                    modified_files: vec!["src/config.rs".to_string()],
                },
            },
        );

        let session = sample_session();
        write_session_summary(
            &state,
            &session,
            &[],
            &std::collections::HashMap::new(),
            &path,
        )
        .unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("src/config.rs"));
        assert!(content.contains("SupervisorConfig"));
    }

    #[test]
    fn last_artifact_wins_when_multiple_present() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.md");
        let state = BrokerState::new(None);
        populate_state(&state, "feat-config", "verified");

        push_log(
            &state,
            1,
            SystemTime::now(),
            BrokerMessage::Artifact {
                agent_id: "feat-config".to_string(),
                payload: ArtifactPayload {
                    status: "in-progress".to_string(),
                    exports: vec![],
                    modified_files: vec!["old.rs".to_string()],
                },
            },
        );
        push_log(
            &state,
            2,
            SystemTime::now(),
            BrokerMessage::Artifact {
                agent_id: "feat-config".to_string(),
                payload: ArtifactPayload {
                    status: "done".to_string(),
                    exports: vec![],
                    modified_files: vec!["new.rs".to_string()],
                },
            },
        );

        let session = sample_session();
        write_session_summary(
            &state,
            &session,
            &[],
            &std::collections::HashMap::new(),
            &path,
        )
        .unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("new.rs"));
        assert!(!content.contains("old.rs"));
    }

    #[test]
    fn existing_file_is_overwritten() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.md");
        fs::write(&path, "old garbage content that should be replaced").unwrap();

        let state = BrokerState::new(None);
        let session = sample_session();
        write_session_summary(
            &state,
            &session,
            &[],
            &std::collections::HashMap::new(),
            &path,
        )
        .unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("garbage"));
        assert!(content.contains("Session Summary"));
    }

    #[test]
    fn write_to_invalid_path_returns_err() {
        let state = BrokerState::new(None);
        let session = sample_session();
        let bad = Path::new("/nonexistent-dir-xyz/sub/session-summary.md");
        let result = write_session_summary(
            &state,
            &session,
            &[],
            &std::collections::HashMap::new(),
            bad,
        );
        assert!(result.is_err());
    }

    #[test]
    fn unused_status_payload_compiles() {
        // Sanity-check: StatusPayload is part of the public broker API used
        // when computing blocked time gaps.
        let _ = StatusPayload {
            status: "working".to_string(),
            modified_files: vec![],
            message: None,
        };
    }

    #[test]
    fn blocked_time_sums_gap_to_next_status() {
        let state = BrokerState::new(None);
        populate_state(&state, "feat-config", "working");

        let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let t1 = t0 + Duration::from_secs(30);

        push_log(
            &state,
            1,
            t0,
            BrokerMessage::Blocked {
                agent_id: "feat-config".to_string(),
                payload: crate::broker::messages::BlockedPayload {
                    needs: "types".to_string(),
                    from: "feat-errors".to_string(),
                },
            },
        );
        push_log(
            &state,
            2,
            t1,
            BrokerMessage::Status {
                agent_id: "feat-config".to_string(),
                payload: StatusPayload {
                    status: "working".to_string(),
                    modified_files: vec![],
                    message: None,
                },
            },
        );

        let inner = state.read();
        let blocked = estimated_blocked_time(&inner.message_log, "feat-config");
        assert_eq!(blocked, Duration::from_secs(30));
    }

    #[test]
    fn output_contains_totals_section() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.md");
        let state = BrokerState::new(None);
        let session = sample_session();
        write_session_summary(
            &state,
            &session,
            &[],
            &std::collections::HashMap::new(),
            &path,
        )
        .unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("## Totals"));
    }

    #[test]
    fn write_supervisor_summary_creates_timestamped_file_under_sessions_dir() {
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path();

        let state = BrokerState::new(None);
        populate_state(&state, "feat-config", "verified");
        let session = sample_session();

        let written = write_supervisor_summary(
            &state,
            &session,
            &[],
            &std::collections::HashMap::new(),
            repo_root,
        )
        .unwrap();

        // The returned path is under .git-paw/sessions/ and has a .md suffix.
        assert!(
            written.starts_with(repo_root.join(".git-paw").join("sessions")),
            "summary written outside .git-paw/sessions/: {}",
            written.display()
        );
        assert_eq!(written.extension().and_then(|s| s.to_str()), Some("md"));
        assert!(written.exists(), "summary file does not exist on disk");

        // Filename is the UTC timestamp; should look like 2026-05-07T12-34-56Z.md
        let filename = written.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            filename.ends_with("Z.md"),
            "expected ISO-8601 UTC suffix, got {filename}"
        );
        assert!(
            !filename.contains(':'),
            "filename must not contain colons: {filename}"
        );

        let content = fs::read_to_string(&written).unwrap();
        assert!(content.contains("# Session Summary"));
        assert!(content.contains("demo"));
    }

    #[test]
    fn write_supervisor_summary_creates_sessions_dir_when_missing() {
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path();
        assert!(!repo_root.join(".git-paw").exists());

        let state = BrokerState::new(None);
        let session = sample_session();
        let written = write_supervisor_summary(
            &state,
            &session,
            &[],
            &std::collections::HashMap::new(),
            repo_root,
        )
        .unwrap();

        assert!(repo_root.join(".git-paw").is_dir());
        assert!(repo_root.join(".git-paw").join("sessions").is_dir());
        assert!(written.exists());
    }

    #[test]
    fn write_supervisor_summary_two_sequential_calls_produce_distinct_files() {
        // Multiple supervisor runs against the same repo must coexist on disk.
        // The second-resolution timestamp guarantees we may see the same name
        // if both writes occur in the same second; sleep one second between
        // calls so we exercise the "two sessions, two files" contract.
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path();
        let state = BrokerState::new(None);
        let session = sample_session();

        let first = write_supervisor_summary(
            &state,
            &session,
            &[],
            &std::collections::HashMap::new(),
            repo_root,
        )
        .unwrap();
        std::thread::sleep(Duration::from_secs(1));
        let second = write_supervisor_summary(
            &state,
            &session,
            &[],
            &std::collections::HashMap::new(),
            repo_root,
        )
        .unwrap();

        assert_ne!(
            first,
            second,
            "back-to-back supervisor runs must produce distinct summary files; both wrote to {}",
            first.display()
        );
        assert!(first.exists() && second.exists());
    }

    #[test]
    fn format_duration_examples() {
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
        assert_eq!(format_duration(Duration::from_secs(125)), "2m 5s");
        assert_eq!(format_duration(Duration::from_secs(3700)), "1h 1m");
    }

    /// When `test_results` contains entries for one or more branches, the
    /// rendered Markdown must include a `## Test Results` section and one
    /// row per entry showing the branch name and a pass/fail status. Output
    /// from the test command must also be included verbatim.
    #[test]
    fn test_results_in_summary() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.md");

        let state = BrokerState::new(None);
        populate_state(&state, "feat-config", "verified");
        populate_state(&state, "feat-errors", "verified");
        let session = sample_session();

        let mut test_results: std::collections::HashMap<String, TestResult> =
            std::collections::HashMap::new();
        test_results.insert(
            "feat-config".to_string(),
            TestResult {
                success: true,
                output: "all 42 tests passed".to_string(),
            },
        );
        test_results.insert(
            "feat-errors".to_string(),
            TestResult {
                success: false,
                output: "thread 'main' panicked: oh no".to_string(),
            },
        );

        write_session_summary(&state, &session, &[], &test_results, &path).unwrap();
        let content = fs::read_to_string(&path).unwrap();

        // Section header.
        assert!(
            content.contains("## Test Results"),
            "summary should include Test Results section; got:\n{content}"
        );

        // Each branch appears with the right status marker.
        assert!(
            content.contains("**feat-config**: \u{2713} PASS"),
            "passing branch must render with check mark; got:\n{content}"
        );
        assert!(
            content.contains("**feat-errors**: \u{2717} FAIL"),
            "failing branch must render with cross mark; got:\n{content}"
        );

        // Test command output is included verbatim for each branch.
        assert!(
            content.contains("all 42 tests passed"),
            "passing output must appear in summary; got:\n{content}"
        );
        assert!(
            content.contains("thread 'main' panicked: oh no"),
            "failing output must appear in summary; got:\n{content}"
        );
    }

    /// An empty `test_results` map must skip the Test Results section
    /// entirely — no header, no rows.
    #[test]
    fn test_results_section_omitted_when_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.md");
        let state = BrokerState::new(None);
        let session = sample_session();
        write_session_summary(
            &state,
            &session,
            &[],
            &std::collections::HashMap::new(),
            &path,
        )
        .unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(
            !content.contains("## Test Results"),
            "Test Results section must be absent when no results provided; got:\n{content}"
        );
    }
}

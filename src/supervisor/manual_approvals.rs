//! Manual-approval pattern recording, aggregation, and promotion heuristic.
//!
//! Implements the `approval-pattern-surfacing` capability. When the user
//! manually approves a command that the auto-approve preset did *not* match,
//! that pattern is appended to a per-session JSONL log at
//! `.git-paw/sessions/<session>.manual-approvals.jsonl`. The
//! `git paw approvals` subcommand (see `cmd_approvals` in `main.rs`) reads the
//! log, aggregates by pattern, and suggests a promotion target so the user can
//! decide which patterns deserve promotion to the bundled preset or the
//! project-local allowlist.
//!
//! The module is split into three concerns, each independently testable:
//!
//! 1. **Recording** — [`ManualApproval`], [`log_path`], [`record`] and the
//!    in-memory [`SeenPatterns`] tracker that computes `first_seen`.
//! 2. **Aggregation** — [`aggregate`] groups the log lines by pattern and
//!    computes count + first/last seen.
//! 3. **Heuristic** — [`suggest_target`] classifies a pattern as a
//!    project-allowlist or bundled-preset candidate.
//!
//! The learnings dispatch ([`permission_pattern_learning`]) builds the
//! `agent.learning` record that the recording call site publishes on a
//! first-seen approval when learnings are enabled.

use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::broker::learnings::{CATEGORY_PERMISSION_PATTERN, LearningRecord};
use crate::broker::messages::BrokerMessage;

/// One manual-approval log entry — the in-memory shape of a single JSONL line.
///
/// Serialised one-per-line to
/// `.git-paw/sessions/<session>.manual-approvals.jsonl`. `timestamp` is an
/// ISO-8601 UTC instant (`2026-05-29T12:34:56Z`), `pattern` is the approved
/// command as captured, and `first_seen` is `true` the first time the pattern
/// is approved within the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualApproval {
    /// ISO-8601 UTC timestamp of the approval.
    pub timestamp: String,
    /// Agent whose pane surfaced the prompt (slugified branch name).
    pub agent_id: String,
    /// The approved command pattern, captured verbatim.
    pub pattern: String,
    /// `true` the first time this pattern is approved this session.
    pub first_seen: bool,
}

impl ManualApproval {
    /// Builds a [`ManualApproval`] stamped with the current UTC time.
    #[must_use]
    pub fn now(agent_id: &str, pattern: &str, first_seen: bool) -> Self {
        Self {
            timestamp: now_iso8601(),
            agent_id: agent_id.to_string(),
            pattern: pattern.to_string(),
            first_seen,
        }
    }
}

/// Returns the current instant formatted as ISO-8601 UTC seconds precision
/// (`2026-05-29T12:34:56Z`).
#[must_use]
pub fn now_iso8601() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Returns the manual-approvals log path for a session:
/// `<repo_root>/.git-paw/sessions/<session>.manual-approvals.jsonl`.
#[must_use]
pub fn log_path(repo_root: &Path, session: &str) -> PathBuf {
    repo_root
        .join(".git-paw")
        .join("sessions")
        .join(format!("{session}.manual-approvals.jsonl"))
}

/// Appends one JSON line for `approval` to `log_path`, returning any I/O error.
///
/// The parent directory is created if absent. Exposed (alongside the
/// best-effort [`record`]) so tests can assert the success and failure paths
/// deterministically.
pub fn append_line(approval: &ManualApproval, log_path: &Path) -> std::io::Result<()> {
    if let Some(parent) = log_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut line = serde_json::to_string(approval)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    line.push('\n');
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    file.write_all(line.as_bytes())
}

/// Best-effort append of `approval` to `log_path`.
///
/// On failure (disk full, permission denied, parent is a file) the error is
/// reported to stderr and swallowed — the recording path SHALL NOT block or
/// panic the sweep. Returns `true` when the line was written.
pub fn record(approval: &ManualApproval, log_path: &Path) -> bool {
    match append_line(approval, log_path) {
        Ok(()) => true,
        Err(e) => {
            eprintln!(
                "warning: failed to record manual approval to {}: {e}",
                log_path.display()
            );
            false
        }
    }
}

/// In-memory set of patterns already approved this session, used to compute
/// the `first_seen` flag without re-reading the log on the sweep hot path.
#[derive(Debug, Default)]
pub struct SeenPatterns {
    seen: HashSet<String>,
}

impl SeenPatterns {
    /// Creates an empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `pattern` as seen and returns `true` if this is the first time
    /// the pattern has been seen this session (i.e. the `first_seen` value for
    /// the entry being logged now).
    pub fn observe(&mut self, pattern: &str) -> bool {
        self.seen.insert(pattern.to_string())
    }

    /// Returns whether `pattern` has already been observed this session.
    #[must_use]
    pub fn contains(&self, pattern: &str) -> bool {
        self.seen.contains(pattern)
    }
}

// ---------------------------------------------------------------------------
// Promotion-target heuristic
// ---------------------------------------------------------------------------

/// Suggested promotion target for a manual-approval pattern.
///
/// The suggestion is a hint, not a rule — the heuristic is a string match and
/// is allowed to be wrong. The output column is labelled `SUGGEST` to set that
/// expectation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suggestion {
    /// The pattern looks project-specific (local script path, project name, or
    /// branch name) — suggest adding it to the project allowlist.
    ProjectAllowlist,
    /// The pattern looks general (`make <target>`, `pnpm <script>`, …) —
    /// suggest it as a candidate for the bundled dev-allowlist preset.
    BundledPresetCandidate,
}

impl Suggestion {
    /// Human-readable label for the text table `SUGGEST` column.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::ProjectAllowlist => "project allowlist",
            Self::BundledPresetCandidate => "bundled preset candidate",
        }
    }

    /// Machine-readable token for the `--json` `suggested_target` field.
    #[must_use]
    pub fn json_value(self) -> &'static str {
        match self {
            Self::ProjectAllowlist => "project-allowlist",
            Self::BundledPresetCandidate => "bundled-preset",
        }
    }
}

/// Classifies `pattern` into a promotion-target [`Suggestion`].
///
/// A pattern is suggested for the **project allowlist** when any project-
/// specific signal is present:
///
/// - a token starting with `./` (a project-local script path),
/// - the `project_name` appears as a substring (when non-empty),
/// - the `branch_name` appears as a substring (when non-empty),
/// - the `worktree_root` path appears as a substring (when provided).
///
/// Otherwise the pattern looks general and is suggested as a **bundled preset
/// candidate**. The checks are intentionally permissive (substring matches);
/// the suggestion is a starting point for the user.
#[must_use]
pub fn suggest_target(
    pattern: &str,
    project_name: &str,
    branch_name: &str,
    worktree_root: Option<&Path>,
) -> Suggestion {
    // A token like `./scripts/deploy.sh` is a project-local path.
    if pattern.split_whitespace().any(|tok| tok.starts_with("./")) {
        return Suggestion::ProjectAllowlist;
    }
    if !project_name.is_empty() && pattern.contains(project_name) {
        return Suggestion::ProjectAllowlist;
    }
    if !branch_name.is_empty() && pattern.contains(branch_name) {
        return Suggestion::ProjectAllowlist;
    }
    if let Some(root) = worktree_root {
        let root = root.to_string_lossy();
        if !root.is_empty() && pattern.contains(root.as_ref()) {
            return Suggestion::ProjectAllowlist;
        }
    }
    Suggestion::BundledPresetCandidate
}

// ---------------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------------

/// One aggregated pattern row: the pattern, how many times it was approved,
/// and the first/last approval timestamps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregatedApproval {
    /// The approved command pattern.
    pub pattern: String,
    /// Number of approvals recorded for this pattern.
    pub count: u64,
    /// Earliest approval timestamp (ISO-8601 UTC).
    pub first_seen: String,
    /// Latest approval timestamp (ISO-8601 UTC).
    pub last_seen: String,
}

/// Reads the manual-approval JSONL log at `jsonl_path`, groups entries by
/// pattern, and returns the aggregated rows sorted by descending count (ties
/// broken by pattern, ascending, for deterministic output).
///
/// A missing log file is not an error — it yields an empty vector (the session
/// simply recorded no manual approvals). Malformed lines are skipped so a
/// single corrupt line does not abort the report.
pub fn aggregate(jsonl_path: &Path) -> std::io::Result<Vec<AggregatedApproval>> {
    let contents = match std::fs::read_to_string(jsonl_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };

    // Preserve first-encounter order so equal-count patterns are stable before
    // the final sort.
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, AggregatedApproval> = HashMap::new();

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<ManualApproval>(line) else {
            continue;
        };
        if let Some(agg) = groups.get_mut(&entry.pattern) {
            agg.count += 1;
            if entry.timestamp < agg.first_seen {
                agg.first_seen.clone_from(&entry.timestamp);
            }
            if entry.timestamp > agg.last_seen {
                agg.last_seen.clone_from(&entry.timestamp);
            }
        } else {
            order.push(entry.pattern.clone());
            groups.insert(
                entry.pattern.clone(),
                AggregatedApproval {
                    pattern: entry.pattern,
                    count: 1,
                    first_seen: entry.timestamp.clone(),
                    last_seen: entry.timestamp,
                },
            );
        }
    }

    let mut rows: Vec<AggregatedApproval> = order
        .into_iter()
        .filter_map(|p| groups.remove(&p))
        .collect();
    rows.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.pattern.cmp(&b.pattern))
    });
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Pattern extraction from captured pane text
// ---------------------------------------------------------------------------

/// Lower-cased substrings that mark a captured line as prompt boilerplate
/// (the question / choices) rather than the command awaiting a decision.
const PROMPT_BOILERPLATE: &[&str] = &[
    "requires approval",
    "do you want",
    "bash command",
    "allow this command",
    "[y/n]",
    "(y/n)",
    "press ",
    "esc to",
    "1. yes",
    "2. no",
    "❯",
];

fn is_prompt_boilerplate(lower: &str) -> bool {
    PROMPT_BOILERPLATE.iter().any(|n| lower.contains(n)) || lower == "yes" || lower == "no"
}

/// Extracts the candidate command pattern from a forwarded prompt's captured
/// pane text.
///
/// This is a heuristic (the suggestion downstream is "allowed to be wrong"):
///
/// 1. A recognised file-operation prompt yields its target path (reusing the
///    auto-approve file-prompt extractor).
/// 2. Otherwise the first non-empty, non-boilerplate line is taken as the
///    command, with a leading `Running ` / `$ ` shell-echo prefix stripped.
///
/// Returns `None` when no command-like line can be found.
#[must_use]
pub fn extract_forwarded_pattern(captured: &str) -> Option<String> {
    if let Some(path) = super::auto_approve::extract_path_from_file_prompt(captured) {
        return Some(path);
    }
    for raw in captured.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if is_prompt_boilerplate(&line.to_ascii_lowercase()) {
            continue;
        }
        let cmd = line
            .strip_prefix("Running ")
            .or_else(|| line.strip_prefix("$ "))
            .unwrap_or(line)
            .trim();
        if !cmd.is_empty() {
            return Some(cmd.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Forwarded-prompt recorder (the §3 hook)
// ---------------------------------------------------------------------------

/// Records the commands the poll loop forwards for a manual decision.
///
/// One instance is held by the supervisor's forward-to-human path across poll
/// ticks so the per-session `first_seen` set persists. Each forwarded prompt
/// appends a JSONL line (best-effort); the first sighting of a pattern returns
/// a `permission_pattern` learning for the caller to publish (when learnings
/// are enabled).
///
/// Recording is the honest "this command required a manual decision" signal —
/// the supervisor cannot observe the in-pane Yes/No, so it records the forward
/// (design D2, option A). When [`Self::enabled`] is `false`
/// (`[supervisor] manual_approvals_log = false`) the recorder is inert: no log
/// write and no learning.
#[derive(Debug)]
pub struct ManualDecisionRecorder {
    log_path: PathBuf,
    enabled: bool,
    learnings_enabled: bool,
    project_name: String,
    cli: Option<String>,
    seen: SeenPatterns,
}

impl ManualDecisionRecorder {
    /// Builds a recorder writing to `log_path`.
    ///
    /// `enabled` mirrors `[supervisor] manual_approvals_log`; `learnings_enabled`
    /// mirrors `[supervisor] learnings`. `project_name` and `cli` feed the
    /// promotion heuristic and the learning body.
    #[must_use]
    pub fn new(
        log_path: PathBuf,
        enabled: bool,
        learnings_enabled: bool,
        project_name: String,
        cli: Option<String>,
    ) -> Self {
        Self {
            log_path,
            enabled,
            learnings_enabled,
            project_name,
            cli,
            seen: SeenPatterns::new(),
        }
    }

    /// Records a prompt the poll loop just forwarded for a manual decision.
    ///
    /// Returns `Some(learning)` to publish when this is the first sighting of
    /// the pattern this session AND learnings are enabled; otherwise `None`.
    /// A no-op (returns `None`) when recording is disabled or no command can be
    /// extracted from `captured`.
    pub fn record_forwarded(&mut self, agent_id: &str, captured: &str) -> Option<BrokerMessage> {
        if !self.enabled {
            return None;
        }
        let pattern = extract_forwarded_pattern(captured)?;
        let first_seen = self.seen.observe(&pattern);
        let approval = ManualApproval::now(agent_id, &pattern, first_seen);
        record(&approval, &self.log_path);

        if first_seen && self.learnings_enabled {
            let suggestion = suggest_target(&pattern, &self.project_name, "", None);
            Some(permission_pattern_learning(
                agent_id,
                &pattern,
                1,
                suggestion,
                self.cli.as_deref(),
                SystemTime::now(),
            ))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Learnings dispatch
// ---------------------------------------------------------------------------

/// Builds the `agent.learning` broker message for a first-seen manual approval.
///
/// Emitted (by the recording call site) only on the first sighting of a
/// pattern within a session, and only when `[supervisor] learnings = true` and
/// `manual_approvals_log = true`. The body carries the pattern, the running
/// count, the suggested promotion target, and the CLI (when known). The record
/// reuses the deterministic-id / hour-bucket dedup from the learnings
/// subsystem via [`LearningRecord`].
#[must_use]
pub fn permission_pattern_learning(
    agent_id: &str,
    pattern: &str,
    count_so_far: u64,
    suggested_target: Suggestion,
    cli: Option<&str>,
    timestamp: SystemTime,
) -> BrokerMessage {
    let mut body = serde_json::json!({
        "pattern": pattern,
        "count_so_far": count_so_far,
        "suggested_target": suggested_target.json_value(),
    });
    if let Some(cli) = cli {
        body["cli"] = serde_json::Value::String(cli.to_string());
    }
    let record = LearningRecord {
        category: CATEGORY_PERMISSION_PATTERN.to_string(),
        agent_id: agent_id.to_string(),
        branch_id: None,
        title: format!(
            "manual approval `{pattern}` (suggest {})",
            suggested_target.label()
        ),
        body,
        timestamp,
    };
    BrokerMessage::from(&record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn appr(agent: &str, pattern: &str, first_seen: bool, ts: &str) -> ManualApproval {
        ManualApproval {
            timestamp: ts.to_string(),
            agent_id: agent.to_string(),
            pattern: pattern.to_string(),
            first_seen,
        }
    }

    // --- log_path ---

    #[test]
    fn log_path_follows_session_template() {
        let p = log_path(Path::new("/repo"), "paw-proj");
        assert!(p.ends_with(".git-paw/sessions/paw-proj.manual-approvals.jsonl"));
    }

    // --- append / record (§2.5) ---

    #[test]
    fn append_creates_file_and_writes_one_line() {
        let tmp = TempDir::new().unwrap();
        let path = log_path(tmp.path(), "paw-x");
        assert!(!path.exists());
        append_line(
            &appr("feat/a", "make it", true, "2026-05-29T12:00:00Z"),
            &path,
        )
        .unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 1);
        let parsed: ManualApproval = serde_json::from_str(content.lines().next().unwrap()).unwrap();
        assert_eq!(parsed.pattern, "make it");
        assert!(parsed.first_seen);
    }

    #[test]
    fn append_to_existing_file_adds_a_line() {
        let tmp = TempDir::new().unwrap();
        let path = log_path(tmp.path(), "paw-x");
        append_line(
            &appr("feat/a", "make it", true, "2026-05-29T12:00:00Z"),
            &path,
        )
        .unwrap();
        append_line(
            &appr("feat/a", "make it", false, "2026-05-29T12:05:00Z"),
            &path,
        )
        .unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 2);
    }

    #[test]
    fn record_swallows_errors_when_parent_is_a_file() {
        // Make the parent of the log path a regular file so create_dir_all and
        // open both fail — record must not panic and must return false.
        let tmp = TempDir::new().unwrap();
        let blocker = tmp.path().join("blocker");
        std::fs::write(&blocker, b"x").unwrap();
        // log path nested under the file → cannot create dir
        let path = blocker.join("sessions").join("p.manual-approvals.jsonl");
        let ok = record(
            &appr("feat/a", "anything", true, "2026-05-29T12:00:00Z"),
            &path,
        );
        assert!(!ok, "write under a file parent must fail gracefully");
    }

    #[test]
    fn record_returns_true_on_success() {
        let tmp = TempDir::new().unwrap();
        let path = log_path(tmp.path(), "paw-x");
        assert!(record(
            &appr("feat/a", "ok", true, "2026-05-29T12:00:00Z"),
            &path
        ));
    }

    // --- first_seen tracking (§2.4) ---

    #[test]
    fn first_seen_toggles_correctly() {
        let mut seen = SeenPatterns::new();
        assert!(seen.observe("make integration-test"));
        assert!(!seen.observe("make integration-test"));
        assert!(seen.observe("podman build"));
        assert!(seen.contains("make integration-test"));
        assert!(!seen.contains("never"));
    }

    // --- heuristic (§4.3 / §4.4) ---

    #[test]
    fn dot_slash_path_suggests_project() {
        assert_eq!(
            suggest_target("./scripts/deploy-staging.sh", "", "", None),
            Suggestion::ProjectAllowlist
        );
    }

    #[test]
    fn generic_command_suggests_bundled_preset() {
        assert_eq!(
            suggest_target("make integration-test", "myproj", "feat/auth", None),
            Suggestion::BundledPresetCandidate
        );
    }

    #[test]
    fn project_name_substring_suggests_project() {
        assert_eq!(
            suggest_target("myproj-cli --build", "myproj", "", None),
            Suggestion::ProjectAllowlist
        );
    }

    #[test]
    fn branch_name_substring_suggests_project() {
        assert_eq!(
            suggest_target("deploy feat/auth", "", "feat/auth", None),
            Suggestion::ProjectAllowlist
        );
    }

    #[test]
    fn worktree_root_substring_suggests_project() {
        let root = Path::new("/home/me/wt/feature");
        assert_eq!(
            suggest_target("cat /home/me/wt/feature/notes", "", "", Some(root)),
            Suggestion::ProjectAllowlist
        );
    }

    #[test]
    fn multiple_project_signals_tie_still_project() {
        // A `./` token AND a project-name match — both fire; result is stable.
        assert_eq!(
            suggest_target("./run.sh myproj", "myproj", "feat/x", None),
            Suggestion::ProjectAllowlist
        );
    }

    #[test]
    fn empty_project_and_branch_do_not_false_match() {
        // Empty signals must not match an empty substring of every pattern.
        assert_eq!(
            suggest_target("npm test", "", "", None),
            Suggestion::BundledPresetCandidate
        );
    }

    // --- aggregate (§4.1) ---

    #[test]
    fn aggregate_missing_file_is_empty() {
        let tmp = TempDir::new().unwrap();
        let path = log_path(tmp.path(), "paw-none");
        let rows = aggregate(&path).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn aggregate_groups_counts_and_sorts_descending() {
        let tmp = TempDir::new().unwrap();
        let path = log_path(tmp.path(), "paw-x");
        // make integration-test x3, ./deploy.sh x1, podman build x2
        append_line(
            &appr("a", "make integration-test", true, "2026-05-29T12:00:00Z"),
            &path,
        )
        .unwrap();
        append_line(
            &appr("a", "podman build", true, "2026-05-29T12:01:00Z"),
            &path,
        )
        .unwrap();
        append_line(
            &appr("a", "make integration-test", false, "2026-05-29T12:02:00Z"),
            &path,
        )
        .unwrap();
        append_line(
            &appr("a", "./deploy.sh", true, "2026-05-29T12:03:00Z"),
            &path,
        )
        .unwrap();
        append_line(
            &appr("a", "make integration-test", false, "2026-05-29T12:04:00Z"),
            &path,
        )
        .unwrap();
        append_line(
            &appr("a", "podman build", false, "2026-05-29T12:05:00Z"),
            &path,
        )
        .unwrap();

        let rows = aggregate(&path).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].pattern, "make integration-test");
        assert_eq!(rows[0].count, 3);
        assert_eq!(rows[0].first_seen, "2026-05-29T12:00:00Z");
        assert_eq!(rows[0].last_seen, "2026-05-29T12:04:00Z");
        assert_eq!(rows[1].pattern, "podman build");
        assert_eq!(rows[1].count, 2);
        assert_eq!(rows[2].pattern, "./deploy.sh");
        assert_eq!(rows[2].count, 1);
    }

    #[test]
    fn aggregate_skips_malformed_lines() {
        let tmp = TempDir::new().unwrap();
        let path = log_path(tmp.path(), "paw-x");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "{not json}\n{\"timestamp\":\"2026-05-29T12:00:00Z\",\"agent_id\":\"a\",\"pattern\":\"ok\",\"first_seen\":true}\n\n",
        )
        .unwrap();
        let rows = aggregate(&path).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pattern, "ok");
    }

    // --- learnings dispatch (§6) ---

    #[test]
    fn learning_record_has_permission_pattern_category_and_body() {
        let msg = permission_pattern_learning(
            "feat/a",
            "make integration-test",
            1,
            Suggestion::BundledPresetCandidate,
            Some("claude"),
            SystemTime::UNIX_EPOCH,
        );
        let BrokerMessage::Learning { payload } = msg else {
            panic!("expected Learning");
        };
        assert_eq!(payload.category, CATEGORY_PERMISSION_PATTERN);
        assert_eq!(payload.body["pattern"], "make integration-test");
        assert_eq!(payload.body["count_so_far"], 1);
        assert_eq!(payload.body["suggested_target"], "bundled-preset");
        assert_eq!(payload.body["cli"], "claude");
    }

    // --- pattern extraction ---

    #[test]
    fn extract_shell_command_from_prompt() {
        let captured = "Bash command:\nmake integration-test\nrequires approval [y/N]";
        assert_eq!(
            extract_forwarded_pattern(captured).as_deref(),
            Some("make integration-test")
        );
    }

    #[test]
    fn extract_skips_boilerplate_and_returns_command() {
        let captured = "Do you want to proceed?\nrm -rf /tmp/foo\n[y/N]";
        assert_eq!(
            extract_forwarded_pattern(captured).as_deref(),
            Some("rm -rf /tmp/foo")
        );
    }

    #[test]
    fn extract_strips_running_prefix() {
        let captured = "do you want to proceed\nRunning ./scripts/deploy.sh";
        assert_eq!(
            extract_forwarded_pattern(captured).as_deref(),
            Some("./scripts/deploy.sh")
        );
    }

    #[test]
    fn extract_file_op_path() {
        let captured = "Do you want to allow this write to /etc/hosts?";
        assert_eq!(
            extract_forwarded_pattern(captured).as_deref(),
            Some("/etc/hosts")
        );
    }

    #[test]
    fn extract_returns_none_for_marker_only() {
        assert_eq!(extract_forwarded_pattern("requires approval\n[y/N]"), None);
    }

    // --- ManualDecisionRecorder (§3.3 / §3.4 / §6.3) ---

    fn recorder(tmp: &TempDir, enabled: bool, learnings: bool) -> ManualDecisionRecorder {
        ManualDecisionRecorder::new(
            log_path(tmp.path(), "paw-x"),
            enabled,
            learnings,
            "myproj".to_string(),
            Some("claude".to_string()),
        )
    }

    #[test]
    fn recorder_disabled_writes_nothing_and_emits_no_learning() {
        let tmp = TempDir::new().unwrap();
        let mut rec = recorder(&tmp, false, true);
        let learning = rec.record_forwarded("feat/a", "Bash command:\nmake foo\n[y/N]");
        assert!(learning.is_none(), "disabled recorder must not emit");
        assert!(
            !log_path(tmp.path(), "paw-x").exists(),
            "disabled recorder must not write the log"
        );
    }

    #[test]
    fn recorder_first_sighting_emits_one_learning_repeat_emits_none() {
        let tmp = TempDir::new().unwrap();
        let mut rec = recorder(&tmp, true, true);
        let first = rec.record_forwarded("feat/a", "Bash command:\nmake foo\n[y/N]");
        let second = rec.record_forwarded("feat/a", "Bash command:\nmake foo\n[y/N]");
        assert!(first.is_some(), "first sighting must emit a learning");
        assert!(second.is_none(), "repeat sighting must not emit");
        // Both forwards are logged (per-forward recording, first_seen toggles).
        let rows = aggregate(&log_path(tmp.path(), "paw-x")).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].count, 2);
    }

    #[test]
    fn recorder_logs_but_omits_learning_when_learnings_disabled() {
        let tmp = TempDir::new().unwrap();
        let mut rec = recorder(&tmp, true, false);
        let learning = rec.record_forwarded("feat/a", "Bash command:\n./run.sh\n[y/N]");
        assert!(
            learning.is_none(),
            "learnings disabled → no broker emission"
        );
        let rows = aggregate(&log_path(tmp.path(), "paw-x")).unwrap();
        assert_eq!(rows.len(), 1, "but the JSONL line is still written");
        assert_eq!(rows[0].pattern, "./run.sh");
    }

    #[test]
    fn learning_record_omits_cli_when_absent() {
        let msg = permission_pattern_learning(
            "feat/a",
            "./deploy.sh",
            2,
            Suggestion::ProjectAllowlist,
            None,
            SystemTime::UNIX_EPOCH,
        );
        let BrokerMessage::Learning { payload } = msg else {
            panic!("expected Learning");
        };
        assert!(payload.body.get("cli").is_none());
        assert_eq!(payload.body["suggested_target"], "project-allowlist");
    }
}

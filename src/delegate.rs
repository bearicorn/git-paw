//! Delegate runs: the pure decisions behind `git paw delegate`.
//!
//! A delegate profile (`[profiles.<name>]` with `kind = "delegate"`) runs one
//! headless, one-shot agent at the repository root for the orchestrator. This
//! module holds everything about a run that needs no process, file or clock:
//! the stdin document ([`document`]), the result truncation
//! ([`truncate_result`]), the ledger line ([`LedgerLine`]) and admission
//! ([`admit`]: four read-only runs or one writing run at a time). The shell
//! that spawns and records the run is [`crate::commands::delegate`].

use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::ApprovalLevel;

/// The environment variable marking a delegate run; its value is the
/// delegate profile's name. Helpers and root-only commands refuse under it.
pub const DELEGATE_ENV: &str = "GIT_PAW_DELEGATE";

/// The most read-only (`manual`) delegate runs admitted at once.
pub const MAX_READ_ONLY_RUNS: usize = 4;

/// The most bytes of a delegate's standard output returned as its result.
pub const RESULT_CAP_BYTES: usize = 65_536;

/// The bundled framing prose that opens every delegate document.
const FRAMING_TEMPLATE: &str = include_str!("../assets/delegate-framing.md");

/// The delegate run this process is part of, when `GIT_PAW_DELEGATE` is
/// present in its environment (any value, empty included, counts).
#[must_use]
pub fn current_delegate_run() -> Option<String> {
    std::env::var_os(DELEGATE_ENV).map(|v| v.to_string_lossy().into_owned())
}

/// The refusal a root-only command or helper prints inside a delegate run.
#[must_use]
pub fn refusal_inside_delegate(run: &str, what: &str) -> String {
    format!(
        "{what} is unavailable inside a delegate run ({DELEGATE_ENV}={run}): a delegate's \
         result is its standard output"
    )
}

/// The framing prose rendered for one run.
#[must_use]
pub fn framing(profile: &str, repo_root: &Path, level: ApprovalLevel) -> String {
    FRAMING_TEMPLATE
        .replace("{{PROFILE_NAME}}", profile)
        .replace("{{REPO_ROOT}}", &repo_root.display().to_string())
        .replace("{{APPROVAL_LEVEL}}", level.kebab_name())
}

/// The document a delegate reads on stdin: the framing, the rendered role
/// skill, then the task, in that order.
#[must_use]
pub fn document(framing: &str, role_skill: &str, task: &str) -> String {
    let mut doc = String::with_capacity(framing.len() + role_skill.len() + task.len() + 64);
    for (i, part) in [framing, role_skill].into_iter().enumerate() {
        if i > 0 {
            doc.push('\n');
        }
        doc.push_str(part.trim_end());
        doc.push_str("\n\n---\n");
    }
    doc.push_str("\n## Task\n\n");
    doc.push_str(task.trim_end());
    doc.push('\n');
    doc
}

/// The result `git paw delegate` prints: `stdout` as is when it fits in
/// [`RESULT_CAP_BYTES`], else its first bytes up to the cap (cut back to a
/// character boundary) followed by a line naming the full run log.
#[must_use]
pub fn truncate_result(stdout: &str, run_log: &Path) -> String {
    if stdout.len() <= RESULT_CAP_BYTES {
        return stdout.to_string();
    }
    let mut cut = RESULT_CAP_BYTES;
    while !stdout.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut out = stdout[..cut].to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    let _ = writeln!(
        out,
        "[output truncated at {RESULT_CAP_BYTES} bytes; the full output is in {}]",
        run_log.display()
    );
    out
}

/// How a delegate run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The process exited 0 within its timeout.
    Ok,
    /// The process exited non-zero, or was killed by a signal (`None`).
    Failed(Option<i32>),
    /// The timeout elapsed; its process group was killed.
    Timeout,
}

impl Outcome {
    /// The ledger's outcome word.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failed(_) => "failed",
            Self::Timeout => "timeout",
        }
    }
}

/// One line of `.git-paw/logs/delegates/runs.jsonl`: metadata only — never
/// the task or the result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerLine {
    /// The delegate profile's name.
    pub profile: String,
    /// The CLI id it ran.
    pub cli: String,
    /// The CLI session id git-paw assigned, when the CLI has a template.
    pub session_id: Option<String>,
    /// When the run started, in seconds since the Unix epoch.
    pub started_at: u64,
    /// How long it ran, in seconds.
    pub duration_seconds: f64,
    /// `ok`, `failed` or `timeout`.
    pub outcome: String,
    /// The exit code of a `failed` run (`None` when killed by a signal).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

impl LedgerLine {
    /// The ledger line for one finished run.
    #[must_use]
    pub fn new(
        profile: &str,
        cli: &str,
        session_id: Option<&str>,
        started_at: u64,
        duration_seconds: f64,
        outcome: Outcome,
    ) -> Self {
        Self {
            profile: profile.to_string(),
            cli: cli.to_string(),
            session_id: session_id.map(str::to_string),
            started_at,
            duration_seconds,
            outcome: outcome.as_str().to_string(),
            exit_code: match outcome {
                Outcome::Failed(code) => code,
                _ => None,
            },
        }
    }

    /// The JSON line, without its trailing newline.
    #[must_use]
    pub fn to_json_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// The ledger file of the repository at `repo_root`.
#[must_use]
pub fn ledger_path(repo_root: &Path) -> std::path::PathBuf {
    repo_root
        .join(".git-paw")
        .join("logs")
        .join("delegates")
        .join("runs.jsonl")
}

/// Every readable line of the repository's run ledger, in order. A missing
/// ledger, or a line that does not parse, contributes nothing.
#[must_use]
pub fn read_ledger(repo_root: &Path) -> Vec<LedgerLine> {
    std::fs::read_to_string(ledger_path(repo_root))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// One active run as its record under `.git-paw/logs/delegates/active/`
/// holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveRun {
    /// The delegate profile's name.
    pub profile: String,
    /// The run's level.
    pub level: ApprovalLevel,
    /// The process id of the `git paw delegate` process holding the run.
    pub pid: u32,
    /// The delegate's own process group (its leader's pid), once spawned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pgid: Option<u32>,
    /// When the run's timeout elapses, in seconds since the Unix epoch, once
    /// spawned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<u64>,
    /// The program the delegate's group leader runs, once spawned: the
    /// identity checked before the group is ever signalled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    /// The group leader's start time as `ps -o lstart=` reported it right
    /// after the spawn: with `program`, the identity a kill requires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leader_start: Option<String>,
}

/// The largest process or group id a signal can name: anything above
/// `i32::MAX` wraps to a negative `pid_t` (a group, or every process).
const MAX_PID: u32 = 0x7fff_ffff;

/// Whether a record's holder `pid` names a real, single process: `0` and `1`
/// (the caller's group, init) and ids above `i32::MAX` count as dead.
#[must_use]
pub fn valid_holder(pid: u32) -> bool {
    pid > 1 && pid <= MAX_PID
}

/// Whether `pgid` may be probed or signalled at all: a real group (`> 1`;
/// `0` means the caller's own group and `1` every process the user can
/// signal) that is not git-paw's own group (`own`, when known).
#[must_use]
pub fn valid_pgid(pgid: u32, own: Option<u32>) -> bool {
    pgid > 1 && pgid <= MAX_PID && own != Some(pgid)
}

/// Whether `ps -o pgid=,command= -p <pgid>` output (`ps_line`) shows the
/// group's leader still leading `pgid`, with the basename of its first or
/// second argv word equal to `program`'s — the second covering a script CLI
/// run through its interpreter (`node /…/agent …`). This is half of the
/// identity a kill requires; the leader's recorded start time is the other,
/// so a group id reused by an unrelated process is never killed.
#[must_use]
pub fn leader_matches(ps_line: &str, pgid: u32, program: &str) -> bool {
    let basename = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
    let mut words = ps_line.split_whitespace();
    words.next().and_then(|g| g.parse::<u32>().ok()) == Some(pgid)
        && !program.is_empty()
        && words
            .take(2)
            .any(|argv| basename(argv) == basename(program))
}

/// What an active-run record means at admission time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordState {
    /// The run is still going: its `git paw delegate` process or its
    /// delegate's process group is alive, within the deadline.
    Live,
    /// Nothing of the run is left; the record is stale.
    Dead,
    /// The delegate's group outlived its deadline — typically orphaned by a
    /// `git paw delegate` that was killed. Its group must be killed and the
    /// record dropped.
    Overdue,
}

/// Classifies one record at `now` from whether its holder process and its
/// delegate's process group are alive. Past its deadline a record is overdue
/// while its group lives and dead once it does not, whatever its holder's
/// state, so an orphaned delegate can never outlive its timeout or keep
/// counting toward the concurrency limit.
#[must_use]
pub fn classify(run: &ActiveRun, now: u64, holder_alive: bool, group_alive: bool) -> RecordState {
    if run.deadline.is_some_and(|d| now >= d) {
        return if group_alive {
            RecordState::Overdue
        } else {
            RecordState::Dead
        };
    }
    if holder_alive || group_alive {
        RecordState::Live
    } else {
        RecordState::Dead
    }
}

/// Decides whether a run at `level` may start next to the `active` (live)
/// runs: up to [`MAX_READ_ONLY_RUNS`] `manual` runs at once, and an `auto`
/// run only alone, excluding every other run while it is active. Never
/// queues.
///
/// # Errors
///
/// The refusal, naming the active runs' profiles.
pub fn admit(active: &[ActiveRun], level: ApprovalLevel) -> Result<(), String> {
    let names = || {
        active
            .iter()
            .map(|r| r.profile.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if let Some(writer) = active.iter().find(|r| r.level != ApprovalLevel::Manual) {
        return Err(format!(
            "a writing delegate run ('{}') is active and runs alone; try again when it ends",
            writer.profile
        ));
    }
    if level != ApprovalLevel::Manual && !active.is_empty() {
        return Err(format!(
            "a writing ({}) delegate runs alone, but {} delegate run(s) are active: {}",
            level.kebab_name(),
            active.len(),
            names()
        ));
    }
    if active.len() >= MAX_READ_ONLY_RUNS {
        return Err(format!(
            "at most {MAX_READ_ONLY_RUNS} read-only delegate runs may run at once; active: {}",
            names()
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "delegate_tests.rs"]
mod tests;

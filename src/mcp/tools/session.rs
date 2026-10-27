//! Session-state tools: `get_session_status`, `get_session_summary`,
//! `get_learnings`, and — for a root session only — the lifecycle tools
//! `start_session`, `pause_session`, `stop_session`, `purge_session`.
//!
//! The read tools read the active/most-recent session receipt and the
//! learnings file. A null session and empty learning sections are returned
//! (not errors) when no session is active.
//!
//! The lifecycle tools do exactly what the equivalent `git paw` command does,
//! by calling the library command surface ([`crate::commands`]) in-process
//! with a capturing output sink (design D2). They never prompt (a picker the
//! command would open is refused with the parameter to pass instead), never
//! attach a terminal, and never run the unattended drive loop
//! (`start_session` has no `unattended` parameter, design D3).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::agents::SIDECAR_REL_PATH;
use crate::commands::launch::LaunchOutcome;
use crate::commands::pause::{PauseOptions, PauseOutcome};
use crate::commands::purge::{BranchFate, PurgeOptions, PurgeOutcome};
use crate::commands::spec_mode::SpecMode;
use crate::commands::stop::{StopOptions, StopOutcome};
use crate::commands::supervisor::{SupervisorOptions, SupervisorPrompt, resolve_supervisor_mode};
use crate::commands::{CaptureOutput, CommandContext};
use crate::error::PawError;
use crate::interactive::{CliInfo, CliMode, Prompter};
use crate::mcp::query;
use crate::mcp::server::GitPawMcpServer;
use crate::specs::SpecEntry;

/// Response for `get_session_status`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SessionStatusResponse {
    /// Active session snapshot, or null.
    pub session: Option<query::session::SessionSnapshot>,
}

/// Compact session summary.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SessionSummary {
    /// Session name.
    pub name: String,
    /// Session status.
    pub status: String,
    /// Number of registered agents.
    pub agent_count: usize,
    /// Counts of agents per live status label.
    pub agents_by_status: BTreeMap<String, usize>,
}

/// Response for `get_session_summary`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SummaryResponse {
    /// Session summary, or null.
    pub summary: Option<SessionSummary>,
}

/// Response for `get_learnings`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct LearningsResponse {
    /// Parsed learning sections.
    pub sections: Vec<query::learnings::LearningSection>,
}

#[tool_router(router = session_router, vis = "pub(crate)")]
impl GitPawMcpServer {
    /// `get_session_status` — the active session snapshot.
    #[tool(
        description = "Return the active session snapshot: name, mode, status, agent count, broker \
                       URL, pause state, and per-agent status (live from the broker when reachable). \
                       { \"session\": null } when no session is active."
    )]
    pub(crate) fn get_session_status(&self) -> Json<SessionStatusResponse> {
        Json(SessionStatusResponse {
            session: query::session::session_status(&self.ctx),
        })
    }

    /// `get_session_summary` — a compact one-object session summary.
    #[tool(
        description = "Return a compact summary of the current session (name, status, agent count, \
                       and per-status agent counts), or { \"summary\": null } when none is active."
    )]
    pub(crate) fn get_session_summary(&self) -> Json<SummaryResponse> {
        let summary = query::session::session_status(&self.ctx).map(|s| {
            let mut by_status: BTreeMap<String, usize> = BTreeMap::new();
            for a in &s.agents {
                if !a.status.is_empty() {
                    *by_status.entry(a.status.clone()).or_default() += 1;
                }
            }
            SessionSummary {
                name: s.name,
                status: s.status,
                agent_count: s.agent_count,
                agents_by_status: by_status,
            }
        });
        Json(SummaryResponse { summary })
    }

    /// `get_learnings` — parsed session-learnings sections.
    #[tool(
        description = "Parse .git-paw/session-learnings.md into structured sections, each with a \
                       category and its entries. Returns the canonical sections as empty arrays \
                       when no learnings file exists."
    )]
    pub(crate) fn get_learnings(&self) -> Json<LearningsResponse> {
        Json(LearningsResponse {
            sections: query::learnings::learnings(&self.ctx),
        })
    }
}

/// Parameters for [`GitPawMcpServer::start_session`]: the `git paw start`
/// selection inputs. There is deliberately no `unattended` parameter.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct StartSessionParams {
    /// The coding agents' CLI (`--cli`). Absent: the repository's
    /// `default_cli`.
    #[serde(default)]
    pub cli: Option<String>,
    /// The branches to launch (`--branches`).
    #[serde(default)]
    pub branches: Option<Vec<String>>,
    /// Launch these discovered specs by name (`--specs NAME,...`) instead of
    /// branches.
    #[serde(default)]
    pub specs: Option<Vec<String>>,
    /// Launch every discovered spec (`--from-all-specs`).
    #[serde(default)]
    pub from_all_specs: bool,
    /// Supervisor mode (`--supervisor` / `--no-supervisor`). Absent: the
    /// repository's `[supervisor]` config decides, as a non-interactive
    /// `git paw start` does.
    #[serde(default)]
    pub supervisor: Option<bool>,
    /// Report the plan without creating anything (`--dry-run`).
    #[serde(default)]
    pub dry_run: bool,
}

/// Response for [`GitPawMcpServer::start_session`].
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct StartSessionResponse {
    /// `launched` (a session is up: launched fresh, restarted from pause, or
    /// recovered), `already_running`, `dry_run`, or `nothing_launched`.
    pub outcome: String,
    /// The tmux session, when one is up.
    pub session_name: Option<String>,
    /// The command a person runs to attach to the session.
    pub attach_command: Option<String>,
    /// Session-context files for the orchestrator's own agent session,
    /// relative to the repository root.
    pub orchestrator_context: Vec<String>,
    /// The command's progress lines, in order.
    pub output: Vec<String>,
    /// The command's warnings, in order.
    pub warnings: Vec<String>,
}

/// Response for [`GitPawMcpServer::pause_session`] and
/// [`GitPawMcpServer::stop_session`].
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SessionActionResponse {
    /// `true` when the repository has no session (nothing was done).
    pub no_session: bool,
    /// The session acted on.
    pub session_name: Option<String>,
    /// `paused`, `already_paused`, `already_stopped`, `stopped`, or
    /// `no_session`.
    pub outcome: String,
    /// The command's progress lines, in order.
    pub output: Vec<String>,
    /// The command's warnings, in order.
    pub warnings: Vec<String>,
}

/// Parameters for [`GitPawMcpServer::purge_session`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PurgeSessionParams {
    /// Must be `true`: purging removes the session, its worktrees (with any
    /// uncommitted changes) and its state. Branches with unmerged commits are
    /// always kept.
    pub confirm: bool,
}

/// A branch purge deleted.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DeletedBranch {
    /// The branch.
    pub branch: String,
    /// Its tip before deletion (short SHA, or `unknown`).
    pub tip: String,
}

/// A branch purge kept.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct RetainedBranch {
    /// The branch.
    pub branch: String,
    /// Its tip (short SHA, or `unknown`).
    pub tip: String,
    /// Why it was kept.
    pub reason: String,
}

/// Response for [`GitPawMcpServer::purge_session`].
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct PurgeSessionResponse {
    /// `true` when the repository has no session (nothing was done).
    pub no_session: bool,
    /// The session purged.
    pub session_name: Option<String>,
    /// Branches git-paw created that were merged and deleted.
    pub deleted: Vec<DeletedBranch>,
    /// Branches kept because they carry unmerged commits (or their merge
    /// state could not be determined).
    pub retained: Vec<RetainedBranch>,
    /// The command's progress lines, in order.
    pub output: Vec<String>,
    /// The command's warnings, in order.
    pub warnings: Vec<String>,
}

/// The picker over MCP: there is no one to ask, so every question is refused
/// with the parameter that answers it instead.
struct NoPicker;

impl NoPicker {
    fn refuse(what: &str, param: &str) -> PawError {
        PawError::McpError(format!(
            "start_session cannot ask which {what} to use over MCP — pass `{param}`"
        ))
    }
}

impl Prompter for NoPicker {
    fn select_mode(&self) -> Result<CliMode, PawError> {
        Err(Self::refuse("CLI", "cli"))
    }

    fn select_branches(&self, _branches: &[String]) -> Result<Vec<String>, PawError> {
        Err(Self::refuse("branches", "branches"))
    }

    fn select_cli(&self, _clis: &[CliInfo], _default: Option<&str>) -> Result<String, PawError> {
        Err(Self::refuse("CLI", "cli"))
    }

    fn select_cli_for_branch(
        &self,
        _branch: &str,
        _clis: &[CliInfo],
        _default: Option<&str>,
    ) -> Result<String, PawError> {
        Err(Self::refuse("CLI", "cli"))
    }

    fn select_specs(&self, _specs: &[SpecEntry]) -> Result<Vec<SpecEntry>, PawError> {
        Err(Self::refuse("specs", "specs"))
    }
}

/// The "Start in supervisor mode?" question over MCP answers no, as a
/// non-interactive terminal does.
struct AnswerNo;

impl SupervisorPrompt for AnswerNo {
    fn ask(&mut self) -> Result<bool, PawError> {
        Ok(false)
    }
}

/// Maps an internal error to an MCP protocol error.
#[allow(clippy::needless_pass_by_value)]
fn to_err(e: PawError) -> ErrorData {
    ErrorData::internal_error(e.to_string(), None)
}

/// One lifecycle call at a time, process-wide. An MCP client may send
/// requests concurrently — `start_session` beside `purge_session`, or a
/// timed-out client retrying `start_session` while the first launch is still
/// in its readiness wait — and every lifecycle call acts on the same session
/// receipt and worktrees.
static LIFECYCLE: Mutex<()> = Mutex::new(());

/// Runs a lifecycle `job` on a blocking thread, so a launch's readiness waits
/// never stall the server's async workers, holding [`LIFECYCLE`] for the
/// whole job so concurrent calls queue instead of overlapping. A lock
/// poisoned by an earlier call's panic is recovered, not propagated: that
/// call already failed, and the next one must still run.
async fn run_lifecycle<T, F>(job: F) -> Result<T, ErrorData>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, PawError> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let _turn = LIFECYCLE.lock().unwrap_or_else(PoisonError::into_inner);
        job()
    })
    .await
    .map_err(|e| ErrorData::internal_error(format!("lifecycle task failed: {e}"), None))?
    .map_err(to_err)
}

/// The spec selection `start_session`'s parameters ask for, refusing the
/// combinations `git paw start` refuses.
fn spec_mode_for(p: &StartSessionParams) -> Result<SpecMode, PawError> {
    let named = p.specs.as_ref();
    if p.from_all_specs && named.is_some() {
        return Err(PawError::McpError(
            "pass either `specs` or `from_all_specs`, not both".to_string(),
        ));
    }
    if named.is_some_and(Vec::is_empty) {
        return Err(PawError::McpError(
            "`specs` must name at least one spec (use `from_all_specs` to launch them all)"
                .to_string(),
        ));
    }
    if (p.from_all_specs || named.is_some()) && p.branches.is_some() {
        return Err(PawError::McpError(
            "pass either `branches` or a spec selection, not both".to_string(),
        ));
    }
    Ok(SpecMode::from_flags(
        p.from_all_specs,
        named.map(Vec::as_slice),
    ))
}

/// `start_session`: the `git paw start` dispatch, non-interactive and never
/// attaching.
fn start_session_blocking(
    root: &Path,
    p: &StartSessionParams,
) -> Result<StartSessionResponse, PawError> {
    let spec_mode = spec_mode_for(p)?;
    let ctx = CommandContext::new(root.to_path_buf());
    let config = ctx.config_or_default();
    let supervisor_enabled = resolve_supervisor_mode(
        p.supervisor == Some(false),
        p.supervisor == Some(true),
        p.dry_run,
        &config,
        &mut AnswerNo,
    )?;
    let mut out = CaptureOutput::default();
    let outcome = if supervisor_enabled {
        crate::commands::supervisor::supervisor(
            &ctx,
            SupervisorOptions {
                cli: p.cli.clone(),
                branches: p.branches.clone(),
                spec_mode,
                specs_format: None,
                dry_run: p.dry_run,
                no_rebase: false,
                unattended: false,
                prompter: &NoPicker,
                interactive: false,
            },
            &mut out,
        )?
    } else if spec_mode == SpecMode::None {
        crate::commands::start::start(
            &ctx,
            crate::commands::start::StartOptions {
                cli: p.cli.clone(),
                branches: p.branches.clone(),
                dry_run: p.dry_run,
                preset: None,
                no_supervisor: true,
                no_rebase: false,
                prompter: &NoPicker,
            },
            &mut out,
        )?
    } else {
        crate::commands::start::start_with_specs(
            &ctx,
            crate::commands::start::StartWithSpecsOptions {
                cli: p.cli.clone(),
                spec_mode,
                specs_format: None,
                dry_run: p.dry_run,
                force: false,
                no_rebase: false,
                prompter: &NoPicker,
                interactive: false,
            },
            &mut out,
        )?
    };

    let (outcome, session_name) = match outcome {
        LaunchOutcome::Attach { session_name } => ("launched", Some(session_name)),
        LaunchOutcome::AlreadyRunning { session_name } => ("already_running", Some(session_name)),
        LaunchOutcome::Done if p.dry_run => ("dry_run", None),
        // A supervised launch reports its own hint and returns `Done`; the
        // session it saved names what came up.
        LaunchOutcome::Done => match crate::session::find_session_for_repo(root)? {
            Some(session) => ("launched", Some(session.session_name)),
            None => ("nothing_launched", None),
        },
        LaunchOutcome::DriveFresh { .. } | LaunchOutcome::DriveExisting { .. } => {
            return Err(PawError::McpError(
                "start_session never runs the unattended drive loop".to_string(),
            ));
        }
    };
    let orchestrator_context = if session_name.is_some() && root.join(SIDECAR_REL_PATH).is_file() {
        vec![SIDECAR_REL_PATH.to_string()]
    } else {
        Vec::new()
    };
    Ok(StartSessionResponse {
        outcome: outcome.to_string(),
        attach_command: session_name
            .as_ref()
            .map(|name| format!("tmux attach -t {name}")),
        session_name,
        orchestrator_context,
        output: out.lines,
        warnings: out.warnings,
    })
}

/// `pause_session`: `git paw pause`.
fn pause_session_blocking(root: PathBuf) -> Result<SessionActionResponse, PawError> {
    let mut out = CaptureOutput::default();
    let outcome =
        crate::commands::pause::pause(&CommandContext::new(root), PauseOptions {}, &mut out)?;
    let (no_session, session_name, outcome) = match outcome {
        PauseOutcome::NoSession => (true, None, "no_session"),
        PauseOutcome::AlreadyPaused { session_name } => {
            (false, Some(session_name), "already_paused")
        }
        PauseOutcome::AlreadyStopped { session_name } => {
            (false, Some(session_name), "already_stopped")
        }
        PauseOutcome::Paused { session_name, .. } => (false, Some(session_name), "paused"),
    };
    Ok(SessionActionResponse {
        no_session,
        session_name,
        outcome: outcome.to_string(),
        output: out.lines,
        warnings: out.warnings,
    })
}

/// `stop_session`: `git paw stop`, which never asks for confirmation.
fn stop_session_blocking(root: PathBuf) -> Result<SessionActionResponse, PawError> {
    let mut out = CaptureOutput::default();
    let outcome =
        crate::commands::stop::stop(&CommandContext::new(root), StopOptions {}, &mut out)?;
    let (no_session, session_name, outcome) = match outcome {
        StopOutcome::NoSession => (true, None, "no_session"),
        StopOutcome::Stopped { session_name } => (false, Some(session_name), "stopped"),
    };
    Ok(SessionActionResponse {
        no_session,
        session_name,
        outcome: outcome.to_string(),
        output: out.lines,
        warnings: out.warnings,
    })
}

/// `purge_session`: a confirmed `git paw purge`, keeping unmerged branches.
fn purge_session_blocking(root: PathBuf) -> Result<PurgeSessionResponse, PawError> {
    let mut out = CaptureOutput::default();
    let outcome = crate::commands::purge::purge(
        &CommandContext::new(root),
        PurgeOptions {
            force: false,
            confirmed: true,
        },
        &mut out,
    )?;
    let mut response = PurgeSessionResponse {
        no_session: false,
        session_name: None,
        deleted: Vec::new(),
        retained: Vec::new(),
        output: Vec::new(),
        warnings: Vec::new(),
    };
    match outcome {
        PurgeOutcome::NoSession => response.no_session = true,
        PurgeOutcome::Cancelled => {
            return Err(PawError::McpError(
                "purge was not confirmed; nothing was changed".to_string(),
            ));
        }
        PurgeOutcome::Purged {
            session_name,
            branches,
        } => {
            response.session_name = Some(session_name);
            for fate in branches {
                match fate {
                    BranchFate::Deleted { branch, tip } => {
                        response.deleted.push(DeletedBranch { branch, tip });
                    }
                    BranchFate::Retained {
                        branch,
                        tip,
                        reason,
                    } => response.retained.push(RetainedBranch {
                        branch,
                        tip,
                        reason,
                    }),
                }
            }
        }
    }
    response.output = out.lines;
    response.warnings = out.warnings;
    Ok(response)
}

/// The lifecycle tools, registered for a root session only (see
/// [`GitPawMcpServer::new`]).
#[tool_router(router = lifecycle_router, vis = "pub(crate)")]
impl GitPawMcpServer {
    /// `start_session` — launch (or revive) the repository's session,
    /// detached.
    #[tool(
        description = "Start this repository's git-paw session, detached (root session only): \
                       exactly what `git paw start` does, without attaching a terminal and without \
                       the unattended drive loop. Pass `branches` or a spec selection (`specs` / \
                       `from_all_specs`), and `cli` unless the repository sets a default; nothing \
                       is asked interactively. An existing session is revived (restarted from \
                       pause, or recovered) rather than duplicated. Returns once the launch's \
                       readiness gates have run, with the session name, the attach command and \
                       the orchestrator-context file."
    )]
    pub(crate) async fn start_session(
        &self,
        Parameters(p): Parameters<StartSessionParams>,
    ) -> Result<Json<StartSessionResponse>, ErrorData> {
        let root = self.ctx.root.clone();
        run_lifecycle(move || start_session_blocking(&root, &p))
            .await
            .map(Json)
    }

    /// `pause_session` — `git paw pause`.
    #[tool(
        description = "Pause this repository's session (root session only): stops the broker and \
                       keeps the agent panes and the dashboard running, as `git paw pause` does. \
                       { \"no_session\": true } when there is no session."
    )]
    pub(crate) async fn pause_session(&self) -> Result<Json<SessionActionResponse>, ErrorData> {
        let root = self.ctx.root.clone();
        run_lifecycle(move || pause_session_blocking(root))
            .await
            .map(Json)
    }

    /// `stop_session` — `git paw stop`.
    #[tool(
        description = "Stop this repository's session (root session only): ends the tmux session \
                       and keeps the worktrees, branches and session state, so it can be recovered \
                       with start_session — as `git paw stop` does, without asking. \
                       { \"no_session\": true } when there is no session."
    )]
    pub(crate) async fn stop_session(&self) -> Result<Json<SessionActionResponse>, ErrorData> {
        let root = self.ctx.root.clone();
        run_lifecycle(move || stop_session_blocking(root))
            .await
            .map(Json)
    }

    /// `purge_session` — a confirmed `git paw purge`.
    #[tool(
        description = "Purge this repository's session (root session only): removes the tmux \
                       session, every worktree (including uncommitted changes in them) and the \
                       session state, as a confirmed `git paw purge` does. Requires \
                       `confirm: true`. Branches git-paw created are deleted only when merged; a \
                       branch with unmerged commits is kept and listed with its tip. \
                       { \"no_session\": true } when there is no session.",
        annotations(destructive_hint = true)
    )]
    pub(crate) async fn purge_session(
        &self,
        Parameters(p): Parameters<PurgeSessionParams>,
    ) -> Result<Json<PurgeSessionResponse>, ErrorData> {
        if !p.confirm {
            return Err(ErrorData::invalid_params(
                "purge_session requires `confirm: true`; nothing was changed",
                None,
            ));
        }
        let root = self.ctx.root.clone();
        run_lifecycle(move || purge_session_blocking(root))
            .await
            .map(Json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    /// Review finding: lifecycle calls must queue, never overlap. Two calls
    /// submitted together each count themselves in while they run; the peak
    /// is one only when the second waited for the first.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn lifecycle_calls_never_overlap() {
        let inside = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let job = |inside: Arc<AtomicUsize>, peak: Arc<AtomicUsize>| {
            move || {
                let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(150));
                inside.fetch_sub(1, Ordering::SeqCst);
                Ok::<(), PawError>(())
            }
        };
        let (a, b) = tokio::join!(
            run_lifecycle(job(Arc::clone(&inside), Arc::clone(&peak))),
            run_lifecycle(job(Arc::clone(&inside), Arc::clone(&peak))),
        );
        a.expect("first call");
        b.expect("second call");
        assert_eq!(
            peak.load(Ordering::SeqCst),
            1,
            "two lifecycle calls overlapped"
        );
    }

    /// A call that panics fails on its own and does not wedge the next one.
    #[tokio::test]
    async fn a_panicked_lifecycle_call_does_not_block_the_next() {
        let failed =
            run_lifecycle(|| -> Result<(), PawError> { panic!("lifecycle job panicked") }).await;
        assert!(failed.is_err());
        let next = run_lifecycle(|| Ok::<u8, PawError>(7)).await;
        assert_eq!(next.expect("the next call runs"), 7);
    }
}

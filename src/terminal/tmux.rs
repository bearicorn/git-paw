//! The tmux adapter for the terminal port.
//!
//! [`TmuxBackend`] answers every [`TerminalBackend`] operation by composing
//! the `src/tmux/` command builders and running them through the
//! [`CommandRunner`] seam. The argv it emits — content, order and stdio
//! disposition — is exactly what the orchestration path issued before the
//! port existed; the golden fixtures under `src/terminal/fixtures/` pin it.
//!
//! A [`SurfaceId`] made here is the pane's tmux target: `<session>:0.<index>`
//! for a surface addressed by its layout position (the index the session
//! records in `pane_index` / `dashboard_pane`), or the pane id `%N` for a
//! surface found by its owner. Both conversions live only in this file.

use std::path::Path;
use std::time::SystemTime;

use crate::command_runner::{CommandRunner, RealCommandRunner};
use crate::error::PawError;
use crate::session::SessionMode;
use crate::tmux as substrate;

use super::{
    GridGeometry, Key, SessionLiveness, SessionPlan, SurfaceId, SurfaceInfo, SurfacePlacement,
    SurfaceSpec, TerminalBackend,
};

/// The tmux [`TerminalBackend`]: production wires [`TmuxBackend::new`], tests
/// inject a recording runner.
#[derive(Debug, Clone, Copy, Default)]
pub struct TmuxBackend<R: CommandRunner = RealCommandRunner> {
    runner: R,
}

impl TmuxBackend {
    /// The production backend, running real `tmux` processes.
    #[must_use]
    pub fn new() -> Self {
        Self {
            runner: RealCommandRunner,
        }
    }
}

impl<R: CommandRunner> TmuxBackend<R> {
    /// A backend whose `tmux` invocations go through `runner`.
    #[cfg(test)]
    pub(crate) fn with_runner(runner: R) -> Self {
        Self { runner }
    }

    /// The pane target of the surface at `position` in `session`'s window.
    fn target_at(session: &str, position: usize) -> String {
        format!("{session}:0.{position}")
    }

    /// Runs `tmux args`, capturing output; `None` when it cannot spawn or
    /// exits non-zero.
    fn read(&self, args: &[&str]) -> Option<Vec<u8>> {
        let out = self.runner.run("tmux", args).ok()?;
        out.success.then_some(out.stdout)
    }

    /// Runs `tmux args` with git-paw's stdio inherited, mapping a spawn
    /// failure or a non-zero exit to `PawError::TmuxError` naming `what`.
    fn act(&self, args: &[&str], what: &str) -> Result<(), PawError> {
        let status = self
            .runner
            .run_inheriting_stdio("tmux", args)
            .map_err(|e| PawError::TmuxError(format!("{what} failed: {e}")))?;
        if status.success {
            Ok(())
        } else {
            Err(PawError::TmuxError(format!(
                "{what} exited with {}",
                status
                    .code
                    .map_or_else(|| "a signal".to_string(), |c| format!("exit status: {c}"))
            )))
        }
    }

    /// The `PaneSpec` the `src/tmux/` builders take for `spec`.
    fn pane_spec(spec: &SurfaceSpec) -> substrate::PaneSpec {
        substrate::PaneSpec {
            branch: spec.label.clone(),
            worktree: spec.owner.to_string_lossy().to_string(),
            cli_command: spec.launch_command.clone(),
        }
    }

    /// Builds the tmux session `plan` describes, without executing it.
    fn build_session(plan: &SessionPlan) -> Result<substrate::TmuxSession, PawError> {
        match plan.mode {
            SessionMode::Supervisor => {
                let dashboard = plan.dashboard.as_ref().ok_or_else(|| {
                    PawError::TmuxError("a supervised session needs a dashboard".to_string())
                })?;
                let agents: Vec<substrate::PaneSpec> =
                    plan.surfaces.iter().map(Self::pane_spec).collect();
                substrate::build_supervisor_session(
                    &plan.name,
                    Some(plan.name.clone()),
                    &Self::pane_spec(dashboard),
                    &agents,
                    crate::surface::supervisor_layout(agents.len(), true)?,
                    plan.mouse,
                    plan.show_labels,
                    &plan.env,
                )
            }
            SessionMode::Bare => {
                let mut builder = substrate::TmuxSessionBuilder::new(&plan.name)
                    .session_name(plan.name.clone())
                    .mouse_mode(plan.mouse)
                    .border_affordances(plan.show_labels);
                if let Some(dashboard) = &plan.dashboard {
                    builder = builder.dashboard(Self::pane_spec(dashboard));
                }
                for (key, value) in &plan.env {
                    builder = builder.set_environment(key, value);
                }
                for spec in &plan.surfaces {
                    builder = builder.add_pane(Self::pane_spec(spec));
                }
                builder.build()
            }
        }
    }
}

impl<R: CommandRunner> TerminalBackend for TmuxBackend<R> {
    fn open_session(&self, plan: &SessionPlan) -> Result<(), PawError> {
        Self::build_session(plan)?.execute_via(&self.runner)
    }

    fn session_alive(&self, session: &str) -> Result<bool, PawError> {
        substrate::is_session_alive_with(&self.runner, session)
    }

    fn close_session(&self, session: &str) -> Result<(), PawError> {
        substrate::kill_session_with(&self.runner, session)
    }

    fn set_session_env(&self, session: &str, key: &str, value: &str) -> Result<(), PawError> {
        let out = self
            .runner
            .run("tmux", &["set-environment", "-t", session, key, value])
            .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;
        if out.success {
            Ok(())
        } else {
            Err(PawError::TmuxError(
                String::from_utf8_lossy(&out.stderr).trim().to_owned(),
            ))
        }
    }

    fn liveness(
        &self,
        session: &str,
        created_at: SystemTime,
        first_agent: usize,
    ) -> SessionLiveness {
        substrate::session_liveness_for_with(
            &self.runner,
            session,
            created_at,
            first_agent,
            SystemTime::now(),
        )
    }

    fn surface_at(&self, session: &str, position: usize) -> SurfaceId {
        SurfaceId::new(Self::target_at(session, position))
    }

    fn list_surfaces(&self, session: &str) -> Result<Vec<SurfaceInfo>, PawError> {
        Ok(
            substrate::list_panes_with_indices_with(&self.runner, session)?
                .into_iter()
                .map(|(position, path)| SurfaceInfo {
                    id: SurfaceId::new(Self::target_at(session, position)),
                    position,
                    owner: Some(path.into()),
                })
                .collect(),
        )
    }

    fn surface_owned_by(&self, session: &str, owner: &Path) -> Result<Option<SurfaceId>, PawError> {
        Ok(
            substrate::resolve_pane_id_for_worktree_with(&self.runner, session, owner)?
                .map(SurfaceId::new),
        )
    }

    fn spawn_surface(
        &self,
        session: &str,
        spec: &SurfaceSpec,
        placement: &SurfacePlacement,
    ) -> Result<SurfaceId, PawError> {
        match placement {
            SurfacePlacement::AppendToGrid { grid, show_labels } => {
                let layout = crate::surface::layout_for(grid.agent_count + 1, grid.has_top_row)?;
                substrate::build_add_agent_commands(
                    session,
                    &Self::pane_spec(spec),
                    grid.first_agent,
                    grid.agent_count,
                    layout,
                    *show_labels,
                )
                .execute_via(&self.runner)?;
                Ok(self.surface_at(session, grid.first_agent + grid.agent_count))
            }
            SurfacePlacement::Before(anchor) => {
                // The new pane is split off before the anchor and takes the
                // anchor's index, so it is addressed by the anchor's target.
                let target = anchor.token();
                let owner = spec.owner.to_string_lossy().to_string();
                let split = self
                    .runner
                    .run_inheriting_stdio(
                        "tmux",
                        &["split-window", "-h", "-b", "-t", target, "-c", &owner],
                    )
                    .map_err(|e| {
                        PawError::TmuxError(format!("failed to spawn {} pane: {e}", spec.label))
                    })?;
                if !split.success {
                    return Err(PawError::TmuxError(format!(
                        "failed to recreate {} pane",
                        spec.label
                    )));
                }
                let _ = self.runner.run_inheriting_stdio(
                    "tmux",
                    &["select-pane", "-t", target, "-T", &spec.label],
                );
                let sent = self
                    .runner
                    .run_inheriting_stdio(
                        "tmux",
                        &["send-keys", "-t", target, &spec.launch_command, "Enter"],
                    )
                    .map_err(|e| {
                        PawError::TmuxError(format!("failed to send {} command: {e}", spec.label))
                    })?;
                if !sent.success {
                    return Err(PawError::TmuxError(format!(
                        "failed to send {} command",
                        spec.label
                    )));
                }
                Ok(anchor.clone())
            }
        }
    }

    fn kill(&self, surface: &SurfaceId) -> Result<(), PawError> {
        substrate::kill_pane_by_id_with(&self.runner, surface.token())
    }

    fn label_surface(&self, surface: &SurfaceId, label: &str) -> Result<(), PawError> {
        self.act(
            &["select-pane", "-t", surface.token(), "-T", label],
            "tmux select-pane",
        )
    }

    fn label_of(&self, surface: &SurfaceId) -> Option<String> {
        // Read regardless of exit status, as the inventory always has: a
        // vanished pane reads as an empty title, not an error.
        self.runner
            .run(
                "tmux",
                &[
                    "display-message",
                    "-t",
                    surface.token(),
                    "-p",
                    "#{pane_title}",
                ],
            )
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    }

    fn send_text(&self, surface: &SurfaceId, text: &str) -> Result<(), PawError> {
        // `-l` immediately before the payload, so no token inside free text is
        // resolved through tmux's key table.
        self.act(
            &["send-keys", "-t", surface.token(), "-l", text],
            "tmux send-keys -l",
        )
    }

    fn prefill_text(&self, surface: &SurfaceId, text: &str) -> Result<(), PawError> {
        self.act(
            &["send-keys", "-l", "-t", surface.token(), text],
            "tmux send-keys -l",
        )
    }

    fn relaunch_surface(&self, surface: &SurfaceId, launch_command: &str) -> Result<(), PawError> {
        substrate::relaunch_cli_into_pane(&self.runner, surface.token(), launch_command);
        Ok(())
    }

    fn answer_dialog(&self, surface: &SurfaceId, option: u8) -> Result<(), PawError> {
        substrate::answer_dialog_in_pane(&self.runner, surface.token(), option);
        Ok(())
    }

    fn send_key(&self, surface: &SurfaceId, key: Key) -> Result<(), PawError> {
        let name = match key {
            Key::Enter => "Enter",
            Key::ClearLine => "C-u",
        };
        self.act(
            &["send-keys", "-t", surface.token(), name],
            "tmux send-keys",
        )
    }

    fn capture_tail(&self, surface: &SurfaceId) -> Option<String> {
        let out = self.read(&["capture-pane", "-p", "-t", surface.token()])?;
        String::from_utf8(out).ok()
    }

    fn foreground_process(&self, surface: &SurfaceId) -> Option<String> {
        let out = self.read(&[
            "display-message",
            "-p",
            "-t",
            surface.token(),
            "#{pane_current_command}",
        ])?;
        Some(String::from_utf8_lossy(&out).trim().to_string())
    }

    fn stream_to(&self, surface: &SurfaceId, log: &Path) -> Result<(), PawError> {
        let mut session = substrate::TmuxSession::empty(surface.token());
        session.pipe_pane(surface.token(), log);
        session.execute_via(&self.runner)
    }

    fn layout(&self, session: &str, grid: &GridGeometry) -> Result<(), PawError> {
        substrate::rebalance_agent_rows_with(
            &self.runner,
            session,
            grid.first_agent,
            grid.agent_count,
        )
    }

    fn reflow_rows(&self, session: &str, grid: &GridGeometry) -> Result<(), PawError> {
        let layout = crate::surface::layout_for(grid.agent_count, grid.has_top_row)?;
        substrate::build_remove_retile_commands(session, grid.first_agent, grid.agent_count, layout)
            .execute_via(&self.runner)
    }

    fn attach(&self, session: &str) -> Result<(), PawError> {
        substrate::attach_with(&self.runner, session)
    }

    fn detach_clients(&self, session: &str) -> Result<(), PawError> {
        substrate::detach_client_with(&self.runner, session)
    }
}

#[cfg(test)]
#[path = "tmux_tests.rs"]
mod tests;

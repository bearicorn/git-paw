//! Small, pure leaf helpers shared across the command handlers.
//!
//! Extracted verbatim from `main.rs` (code-analysis-refactor R2a); behaviour is
//! unchanged.

use git_paw::config::PawConfig;
use git_paw::detect;
use git_paw::error::PawError;
use git_paw::interactive;
use git_paw::session::Session;

/// Convert the config's `[clis.*]` table into the detector's custom-CLI
/// definitions so a configured CLI is resolvable on `PATH` at launch time.
pub(crate) fn config_to_custom_defs(config: &PawConfig) -> Vec<detect::CustomCliDef> {
    config
        .clis
        .iter()
        .map(|(name, cli)| detect::CustomCliDef {
            name: name.clone(),
            command: cli.command.clone(),
            display_name: cli.display_name.clone(),
        })
        .collect()
}

/// Converts a detected CLI info into the format expected by the interactive module.
pub(crate) fn to_interactive_cli(cli: &detect::CliInfo) -> interactive::CliInfo {
    interactive::CliInfo {
        display_name: cli.display_name.clone(),
        binary_name: cli.binary_name.clone(),
    }
}

/// Index of the first coding-agent pane in a session's tmux window. See
/// [`Session::agent_pane_offset`].
pub(crate) fn agent_pane_offset(session: &Session) -> usize {
    session.agent_pane_offset()
}

/// Attaches `tmux pipe-pane` to each coding-agent pane so the session-logging
/// capture contract (spec `session-logging` → "Attach pipe-pane to capture
/// output": *when logging is enabled and a pane is created*) is honoured on
/// every launch path — bare `start`, `--from-specs`, and supervisor. A no-op
/// when `[logging]` is disabled.
///
/// Must be called after the session is built (`builder.build()` /
/// `build_supervisor_session`) and before `tmux_session.execute()`:
/// [`pipe_pane`](git_paw::tmux::TmuxSession::pipe_pane) queues into the
/// session's command list, applied in order at execute time (so the panes
/// exist by the time pipe-pane runs). `first_agent_pane` is the pane index of
/// the first coding agent (see [`agent_pane_offset`]); branch `i` maps to pane
/// `first_agent_pane + i` in window 0.
pub(crate) fn attach_session_logging(
    tmux_session: &mut git_paw::tmux::TmuxSession,
    config: &PawConfig,
    repo_root: &std::path::Path,
    branches: &[&str],
    first_agent_pane: usize,
) -> Result<(), PawError> {
    if !config.logging.as_ref().is_some_and(|l| l.enabled) {
        return Ok(());
    }
    git_paw::logging::ensure_log_dir(repo_root, &tmux_session.name)?;
    for (i, branch) in branches.iter().enumerate() {
        let log_path = git_paw::logging::log_file_path(repo_root, &tmux_session.name, branch);
        let pane_target = format!("{}:0.{}", tmux_session.name, first_agent_pane + i);
        tmux_session.pipe_pane(&pane_target, &log_path);
    }
    Ok(())
}

/// The command that launches the dashboard in its own tmux pane.
///
/// `tmux send-keys` types this into the pane, and the pane's **shell** parses
/// what it receives — so the installed-binary path from
/// [`std::env::current_exe`] is shell-quoted
/// ([`shell_quote`](git_paw::domain::shell_quote)). An unquoted path containing
/// a space word-splits and the shell tries to run the first fragment, so the
/// dashboard never starts; `send-keys -l` cannot help, because the splitting
/// happens in the shell rather than in tmux's key parsing.
///
/// Falls back to the bare `git-paw` name (resolved on `PATH`) when the current
/// executable cannot be read.
pub(crate) fn dashboard_command() -> String {
    dashboard_command_for(
        &std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("git-paw")),
    )
}

/// [`dashboard_command`] for an explicit executable path, so the quoting
/// contract is exercisable without reinstalling the binary.
pub(crate) fn dashboard_command_for(exe: &std::path::Path) -> String {
    format!(
        "{} __dashboard",
        git_paw::domain::shell_quote(&exe.display().to_string())
    )
}

/// The exposure warning a launch command must print before it creates the
/// session, or `None` when the broker is disabled or bound to loopback.
///
/// The broker process prints the same warning, but into the dashboard pane,
/// which switches to tmux's alternate screen moments later — so that copy is
/// effectively invisible. Every path that launches a session calls this and
/// prints the result on stderr *before* the session exists, while the
/// operator is still looking at their own terminal.
pub(crate) fn broker_exposure_warning(broker: &git_paw::config::BrokerConfig) -> Option<String> {
    (broker.enabled && !git_paw::broker::origin::is_loopback_bind(&broker.bind))
        .then(|| git_paw::broker::origin::non_loopback_bind_warning(&broker.bind))
}

/// Prints [`broker_exposure_warning`] on stderr when there is one.
pub(crate) fn warn_if_broker_is_exposed(broker: &git_paw::config::BrokerConfig) {
    if let Some(warning) = broker_exposure_warning(broker) {
        eprintln!("{warning}");
    }
}

/// Error returned when add/remove is invoked on a live session an earlier
/// binary built with the whole-window tiled layout.
///
/// `add` and `remove` work on every session built with the agent grid, bare
/// and supervised alike — the grid is what the incremental re-tile builders
/// can extend. The tiled layout cannot be extended in place, and re-tiling a
/// live window into a grid would move every running pane under the user, so
/// the refusal names the layout (not the session's mode) and points at the one
/// thing that fixes it: stop the session and start it again, which rebuilds it
/// as a grid. No release version is cited — the user cannot act on one.
pub(crate) fn legacy_tiled_layout_unsupported(session_name: &str, verb: &str) -> PawError {
    PawError::SessionError(format!(
        "`git paw {verb}` cannot change session '{session_name}': its panes are the earlier tiled \
         layout, which cannot be re-tiled while the session runs. Stop the session and run \
         `git paw start` to relaunch it in the agent grid layout, then `git paw {verb}` will work."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Spec: safe-process-invocation — "Commands sent via send-keys are sent
    // literally or shell-quoted".

    #[test]
    fn dashboard_command_shell_quotes_the_binary_path() {
        for (exe, expected) in [
            // A spaced install path is quoted so the pane's shell keeps it whole.
            (
                "/Users/My User/bin/git-paw",
                "'/Users/My User/bin/git-paw' __dashboard",
            ),
            // A plain path behaves as before: the shell strips the quotes.
            (
                "/usr/local/bin/git-paw",
                "'/usr/local/bin/git-paw' __dashboard",
            ),
            // The `current_exe` fallback still resolves on PATH when quoted.
            ("git-paw", "'git-paw' __dashboard"),
        ] {
            assert_eq!(
                dashboard_command_for(std::path::Path::new(exe)),
                expected,
                "exe: {exe}"
            );
        }
    }

    #[test]
    #[serial_test::serial]
    fn a_spaced_binary_path_launches_the_dashboard_command_in_a_pane() {
        if std::process::Command::new("tmux")
            .arg("-V")
            .output()
            .is_err()
        {
            return;
        }

        // Stand in for an installed git-paw under a path with a space. The
        // stub records the subcommand it was invoked with, which is what
        // proves the pane's shell ran the whole path rather than its first
        // word.
        let dir = tempfile::TempDir::new().unwrap();
        let bin_dir = dir.path().join("My Bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let exe = bin_dir.join("git-paw");
        let marker = dir.path().join("launched");
        std::fs::write(
            &exe,
            format!(
                "#!/bin/sh\nprintf %s \"$1\" > '{}'\n",
                marker.to_string_lossy()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let session = "paw-dashboard-send-probe";
        let target = format!("{session}:0.0");
        let _ = std::process::Command::new("tmux")
            .args(["kill-session", "-t", session])
            .output();
        let created = std::process::Command::new("tmux")
            .args(["new-session", "-d", "-s", session, "-x", "200", "-y", "50"])
            .status()
            .expect("create probe session");
        assert!(created.success());

        let sent = std::process::Command::new("tmux")
            .args([
                "send-keys",
                "-t",
                &target,
                &dashboard_command_for(&exe),
                "Enter",
            ])
            .status()
            .expect("send dashboard command");
        assert!(sent.success());

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut launched = String::new();
        while std::time::Instant::now() < deadline {
            launched = std::fs::read_to_string(&marker).unwrap_or_default();
            if !launched.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        let _ = std::process::Command::new("tmux")
            .args(["kill-session", "-t", session])
            .output();

        assert_eq!(
            launched, "__dashboard",
            "the pane's shell did not launch the spaced binary path with its subcommand"
        );
    }

    // `git-add-remove-branch` — "Hot-attach works on every grid session": the
    // only refusal left names the tiled LAYOUT, never the session's mode.
    // (Replaces the tests that pinned the removed bare-session refusal.)

    #[test]
    fn the_tiled_layout_refusal_names_the_layout_and_the_fix() {
        for (session, verb) in [("test-session", "add"), ("my-session", "remove")] {
            let msg = legacy_tiled_layout_unsupported(session, verb).to_string();

            assert!(msg.contains(session), "error must name the session: {msg}");
            assert!(msg.contains(verb), "error must name the command: {msg}");
            assert!(
                msg.contains("tiled"),
                "error must name the tiled layout as the reason: {msg}"
            );
            assert!(
                msg.contains("Stop the session") && msg.contains("git paw start"),
                "error must tell the user to stop and start the session again: {msg}"
            );
        }
    }

    #[test]
    fn the_tiled_layout_refusal_blames_the_layout_not_the_mode() {
        let msg = legacy_tiled_layout_unsupported("test-session", "add").to_string();

        // The removed requirement refused a session for being bare and sent
        // the user to supervisor mode. Both are gone: bare grid sessions are
        // hot-attachable, and there is no supervisor mode to switch to.
        assert!(
            !msg.contains("bare") && !msg.contains("supervisor"),
            "the refusal must not blame the session's mode: {msg}"
        );
        for version in ["v0.6.0", "v1.0.0", "v0.18"] {
            assert!(
                !msg.contains(version),
                "error must not cite a release version ({version}): {msg}"
            );
        }
    }

    // Gate review item 6: the broker's own warning lands in the dashboard
    // pane just before it switches to the alternate screen, so the launch
    // command must print it too — before the session is created.

    fn broker(enabled: bool, bind: &str) -> git_paw::config::BrokerConfig {
        git_paw::config::BrokerConfig {
            enabled,
            bind: bind.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn a_launch_warns_when_the_broker_is_bound_off_loopback() {
        for bind in ["0.0.0.0", "192.168.1.5", "::"] {
            let warning = broker_exposure_warning(&broker(true, bind))
                .unwrap_or_else(|| panic!("bind {bind} must warn"));
            assert!(warning.contains(bind), "the warning must name the bind");
            assert!(
                warning.contains("reachable from the network without authentication"),
                "the warning must state the exposure, got: {warning}"
            );
        }
    }

    #[test]
    fn a_launch_is_silent_on_a_loopback_bind_or_a_disabled_broker() {
        for bind in ["127.0.0.1", "localhost", "::1"] {
            assert_eq!(
                broker_exposure_warning(&broker(true, bind)),
                None,
                "bind {bind} must not warn"
            );
        }
        assert_eq!(
            broker_exposure_warning(&broker(false, "0.0.0.0")),
            None,
            "a disabled broker is never started, so it must not warn"
        );
    }
}

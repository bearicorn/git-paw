//! Small, pure leaf helpers shared across the command handlers.
//!
//! Extracted verbatim from `main.rs` (code-analysis-refactor R2a); behaviour is
//! unchanged.

use git_paw::config::PawConfig;
use git_paw::detect;
use git_paw::error::PawError;
use git_paw::interactive;
use git_paw::session::{Session, SessionMode};

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

/// Distinct config-declared `settings_path` files for the session's CLIs
/// (supervisor + agents), expanded and filtered to those whose parent
/// directory already exists.
///
/// CLI-agnostic: only a CLI with `[clis.<name>].settings_path` set
/// contributes a path; built-in CLIs (no custom entry) contribute nothing
/// here — the repo-local `.claude/settings.json` is seeded separately. The
/// parent-exists gate means git-paw never creates a CLI's config dir
/// (matching the dev-allowlist seeder's caution).
pub(crate) fn session_cli_settings_paths(
    config: &PawConfig,
    supervisor_cli: &str,
    agent_cli: &str,
) -> Vec<std::path::PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for cli in [supervisor_cli, agent_cli] {
        let base = cli.split_whitespace().next().unwrap_or(cli);
        if let Some(raw) = config
            .clis
            .get(base)
            .and_then(|c| c.settings_path.as_deref())
        {
            let expanded = expand_tilde(raw);
            let parent_exists = expanded.parent().is_some_and(std::path::Path::is_dir);
            if parent_exists && seen.insert(expanded.clone()) {
                out.push(expanded);
            }
        }
    }
    out
}

/// Every configured `[clis.<name>].settings_path` (tilde-expanded) whose
/// parent directory already exists, deduplicated.
///
/// Used to seed the dev-command allowlist into each registered CLI's
/// alternate settings file in a CLI-agnostic way — there is no hardcoded
/// path. The parent-exists filter preserves the "never create the directory"
/// guarantee: a configured target whose parent is absent is skipped rather
/// than created.
pub(crate) fn configured_settings_paths(config: &PawConfig) -> Vec<std::path::PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for custom in config.clis.values() {
        if let Some(raw) = custom.settings_path.as_deref() {
            let expanded = expand_tilde(raw);
            let parent_exists = expanded.parent().is_some_and(std::path::Path::is_dir);
            if parent_exists && seen.insert(expanded.clone()) {
                out.push(expanded);
            }
        }
    }
    out
}

/// Expand a leading `~` / `~/` in `path` to the home directory.
fn expand_tilde(path: &str) -> std::path::PathBuf {
    match git_paw::dirs::home_dir() {
        Some(home) if path == "~" => home,
        Some(home) => match path.strip_prefix("~/") {
            Some(rest) => home.join(rest),
            None => std::path::PathBuf::from(path),
        },
        None => std::path::PathBuf::from(path),
    }
}

/// Index of the first coding-agent pane in a session's tmux window.
///
/// Supervisor mode reserves pane 0 (supervisor) and pane 1 (dashboard), so
/// agents start at [`SUPERVISOR_PANE_OFFSET`](git_paw::supervisor::layout::SUPERVISOR_PANE_OFFSET).
/// Bare mode places the dashboard at pane 0 when the broker is enabled (agents
/// at pane 1), or has no dashboard pane at all (agents at pane 0).
pub(crate) fn agent_pane_offset(session: &Session) -> usize {
    match session.mode {
        SessionMode::Supervisor => git_paw::supervisor::layout::SUPERVISOR_PANE_OFFSET,
        SessionMode::Bare => usize::from(session.broker_port.is_some()),
    }
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

/// Error returned when add/remove is invoked on a bare-mode session.
pub(crate) fn bare_mode_unsupported(session_name: &str, verb: &str) -> PawError {
    PawError::SessionError(format!(
        "`git paw {verb}` supports supervisor-mode sessions (the default). Session \
         '{session_name}' was started in bare (no-supervisor) mode, whose tiled grid is \
         not re-tiled incrementally in v0.6.0. Stop and re-start with the full branch set, \
         or run the session in supervisor mode to use add/remove."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_paw::config::CustomCli;
    use std::collections::HashMap;

    fn config_with_settings_path(cli: &str, settings_path: Option<String>) -> PawConfig {
        let mut clis = HashMap::new();
        clis.insert(
            cli.to_string(),
            CustomCli {
                command: cli.to_string(),
                display_name: None,
                submit_delay_ms: None,
                settings_path,
                approval_args: HashMap::new(),
            },
        );
        PawConfig {
            clis,
            ..PawConfig::default()
        }
    }

    #[test]
    fn configured_settings_paths_returns_targets_with_existing_parents() {
        let dir = tempfile::TempDir::new().unwrap();
        let target = dir.path().join("settings.json");
        let cfg = config_with_settings_path("mycli", Some(target.to_string_lossy().into_owned()));
        let paths = configured_settings_paths(&cfg);
        assert_eq!(
            paths,
            vec![target],
            "configured path with existing parent is returned"
        );
    }

    #[test]
    fn configured_settings_paths_skips_targets_with_absent_parent() {
        let dir = tempfile::TempDir::new().unwrap();
        let target = dir.path().join("missing-subdir").join("settings.json");
        let cfg = config_with_settings_path("mycli", Some(target.to_string_lossy().into_owned()));
        assert!(
            configured_settings_paths(&cfg).is_empty(),
            "a configured path whose parent is absent must be skipped",
        );
    }

    #[test]
    fn configured_settings_paths_empty_when_no_clis() {
        assert!(configured_settings_paths(&PawConfig::default()).is_empty());
    }

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
}

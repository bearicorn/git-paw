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
}

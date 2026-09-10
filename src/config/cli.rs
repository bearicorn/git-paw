//! Custom CLI and preset definitions, plus programmatic add/remove of
//! custom CLIs in the global/repo config files.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::PawError;

use super::prompt_profile::CliPromptProfileOverride;
use super::{global_config_path, load_config_file, save_config_to};

/// A custom CLI definition from config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomCli {
    /// Command or path to the CLI binary.
    pub command: String,
    /// Optional human-readable display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Optional override for the boot-prompt settle delay (milliseconds)
    /// before the submit `Enter`.
    ///
    /// git-paw injects the boot block, waits this long for a paste-aware CLI
    /// to settle the paste, then sends `Enter` separately. The default
    /// ([`crate::DEFAULT_SUBMIT_DELAY_MS`]) suits most CLIs; raise it for a
    /// CLI whose large-paste handling needs longer before the submit lands.
    /// Set per-CLI rather than hardcoded so the launcher stays CLI-agnostic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submit_delay_ms: Option<u64>,
    /// Optional path to this CLI's claude-format settings file.
    ///
    /// git-paw does not write to this file. Its parent directory joins the
    /// agent-memory-isolation protected-path set
    /// ([`crate::supervisor::auto_approve::ProtectedPaths::derive`]), so a
    /// claude-family variant reading a non-default config dir (e.g.
    /// `~/.claude-oss/settings.json`) gets that directory protected too. A
    /// leading `~` is expanded to the home directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_path: Option<String>,
    /// Per-approval-level flag overrides, consulted BEFORE the built-in
    /// table by [`resolve_approval_flags`].
    ///
    /// Keys are the kebab-case approval-level names (`"manual"`, `"auto"`,
    /// `"full-auto"`); values are the flag string appended verbatim to the
    /// CLI launch command. This is the seam for custom or variant CLIs
    /// (e.g. a claude-oss entry launched via `CLAUDE_CONFIG_DIR`) to get
    /// native permission flags without a built-in table row. Unknown level
    /// keys are rejected at config load with an error naming the key.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub approval_args: HashMap<String, String>,
    /// Per-field prompt-shape overrides, consulted BEFORE the embedded
    /// Claude Code default by [`super::resolve_prompt_profile`].
    ///
    /// A `[clis.<name>].prompt_profile` table with any subset of fields is
    /// valid — every field left unset falls back to the embedded default
    /// rather than resolving empty (D2 in the `cli-prompt-profiles`
    /// capability). This describes how the CLI's terminal *looks* only; it
    /// carries no permission decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_profile: Option<CliPromptProfileOverride>,
}

/// A named preset defining branches and a CLI to use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Preset {
    /// Branches to open in this preset.
    pub branches: Vec<String>,
    /// CLI to use for all branches in this preset.
    pub cli: String,
}

/// Adds a custom CLI to the global config.
///
/// If `command` is not an absolute path, it is resolved via PATH using `which`.
pub fn add_custom_cli(
    name: &str,
    command: &str,
    display_name: Option<&str>,
) -> Result<(), PawError> {
    add_custom_cli_to(&global_config_path()?, name, command, display_name)
}

/// Adds a custom CLI to the config at the given path.
///
/// If `command` is not an absolute path, it is resolved via PATH using `which`.
pub fn add_custom_cli_to(
    config_path: &Path,
    name: &str,
    command: &str,
    display_name: Option<&str>,
) -> Result<(), PawError> {
    let resolved_command = if Path::new(command).is_absolute() {
        command.to_string()
    } else {
        which::which(command)
            .map_err(|_| PawError::ConfigError(format!("command '{command}' not found on PATH")))?
            .to_string_lossy()
            .into_owned()
    };

    let mut config = load_config_file(config_path)?.unwrap_or_default();

    config.clis.insert(
        name.to_string(),
        CustomCli {
            command: resolved_command,
            display_name: display_name.map(String::from),
            submit_delay_ms: None,
            settings_path: None,
            approval_args: HashMap::new(),
            prompt_profile: None,
        },
    );

    save_config_to(config_path, &config)
}

/// Removes a custom CLI from the global config.
///
/// Returns `PawError::CliNotFound` if the name is not present in the config.
pub fn remove_custom_cli(name: &str) -> Result<(), PawError> {
    remove_custom_cli_from(&global_config_path()?, name)
}

/// Removes a custom CLI from the config at the given path.
///
/// Returns `PawError::CliNotFound` if the name is not present in the config.
pub fn remove_custom_cli_from(config_path: &Path, name: &str) -> Result<(), PawError> {
    let mut config = load_config_file(config_path)?.unwrap_or_default();

    if config.clis.remove(name).is_none() {
        return Err(PawError::CliNotFound(name.to_string()));
    }

    save_config_to(config_path, &config)
}

/// Resolve the per-CLI settle delay (ms) for `cli` from `clis`, falling back
/// to [`crate::DEFAULT_SUBMIT_DELAY_MS`].
///
/// `cli` may carry flags (e.g. `"mycli --foo"`); the lookup keys on the
/// leading binary token. The delay is config-driven, never a hardcoded
/// CLI-name table, so callers stay CLI-agnostic — a CLI whose large-paste
/// handling needs more time sets `[clis.<name>].submit_delay_ms` rather than
/// requiring a code change (W15-1, 2026-05-31 dogfood).
///
/// Takes the CLI map directly (rather than a whole [`super::PawConfig`]) so a caller
/// that only carries the CLI table — like the drive loop's `DriveConfig`
/// (`drive-loop-actuator-robustness`) — can resolve a delay without
/// depending on the full config type. Shared by the boot-prompt injection
/// path and the drive loop's nudge path so both settle on the same delay for
/// a given CLI rather than each carrying its own copy.
#[must_use]
pub fn resolve_submit_delay_ms<S: std::hash::BuildHasher>(
    cli: &str,
    clis: &HashMap<String, CustomCli, S>,
) -> u64 {
    let base = cli.split_whitespace().next().unwrap_or(cli);
    clis.get(base)
        .and_then(|c| c.submit_delay_ms)
        .unwrap_or(crate::DEFAULT_SUBMIT_DELAY_MS)
}

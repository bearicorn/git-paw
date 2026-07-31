//! Custom CLI and preset definitions, plus programmatic add/remove of
//! custom CLIs in the global/repo config files.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::PawError;

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
    /// Optional path to this CLI's claude-format settings file
    /// (the file carrying `allowed_bash_prefixes`).
    ///
    /// When set and the broker is enabled, git-paw seeds the broker-curl
    /// allowlist into this path too, so the CLI's boot-time broker `curl`
    /// does not raise a permission prompt. Use for claude-family variants
    /// that read a non-default config dir (e.g. a CLI reading
    /// `~/.claude-oss/settings.json`). A leading `~` is expanded to the
    /// home directory. Left unset, only the repo-local `.claude/settings.json`
    /// is seeded.
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

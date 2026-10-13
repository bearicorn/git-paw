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
    /// **Parse-only.** git-paw never reads or writes this file. It records
    /// where a claude-family variant keeps its settings (e.g. a `claude-oss`
    /// entry on `~/.claude-oss/settings.json`) for an operator's own
    /// reference and for a later change that may act on it. A leading `~` is
    /// expanded to the home directory.
    ///
    /// It previously also seeded the classifier's protected-path set, which
    /// refused to auto-approve writes into operator territory. With nothing
    /// approving automatically, every such write already reaches a human, so
    /// that set and its derivation are gone. The field is kept because
    /// config keys are a frozen surface: an existing config must still load
    /// and round-trip.
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
    /// Template for the flag(s) that assign a fresh session identifier at
    /// launch, consulted BEFORE the built-in table by
    /// [`super::resolve_session_id_args`].
    ///
    /// MUST contain the `{{SESSION_ID}}` placeholder; a template lacking it
    /// is rejected at config load, naming the key. Mirrors the shape and
    /// resolution order of [`Self::approval_args`] — see
    /// `worker-session-id-capture` design D2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id_args: Option<String>,
    /// Template for the flag(s) that resume a previously assigned session,
    /// consulted BEFORE the built-in table by
    /// [`super::resolve_resume_args`].
    ///
    /// MUST contain the `{{SESSION_ID}}` placeholder; a template lacking it
    /// is rejected at config load, naming the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_args: Option<String>,
    /// Whether this CLI speaks git-paw's MCP protocol, overriding the
    /// built-in MCP-capable table consulted by [`crate::skills::cli_speaks_mcp`].
    ///
    /// Unset (the default, and omitted on serialization) means "ask the
    /// built-in table", which knows only `claude`, `codex`, `cursor-agent`,
    /// and `agy`; a custom variant therefore gets the helper-script boot
    /// form unless it opts in here.
    ///
    /// Setting `mcp = true` for a CLI that does NOT have git-paw's MCP server
    /// registered **strands the agent at boot**: its boot block names MCP
    /// publish tools the agent cannot call, so it can never report status.
    /// Opt in only after registering the server for that CLI's config dir.
    /// Capability is deliberately resolved from the CLI identity, never from
    /// the resolved launch command, so `command = "claude"` on a variant that
    /// reads a different config dir does not silently inherit `claude`'s form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<bool>,
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
            session_id_args: None,
            resume_args: None,
            mcp: None,
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

/// Characters a shell leaves alone, so a command built only from these needs
/// no quoting and stays readable in pane scrollback.
const SHELL_SAFE_EXTRA: &[char] = &['_', '-', '.', '/', '+', ',', ':', '@', '%'];

/// Renders `s` as exactly one shell word, quoting only when it needs it.
///
/// A string drawn entirely from the portable safe set
/// `A-Z a-z 0-9 _ - . / + , : @ %` is returned verbatim; anything else goes
/// through [`crate::domain::shell_quote`], so a path containing a space or a
/// shell metacharacter is neither word-split nor expanded.
fn shell_word(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || SHELL_SAFE_EXTRA.contains(&c))
    {
        s.to_string()
    } else {
        crate::domain::shell_quote(s)
    }
}

/// Resolve the shell command that launches `cli` into a pane.
///
/// The CLI *identity* (`cli`) and the *binary it launches* are two different
/// names. `cli` is the `[clis.<name>]` key — or a built-in / pass-through CLI
/// name — and stays the key for approval flags, session-id and resume
/// templates, prompt profiles, settle delays, MCP capability, and the `cli`
/// recorded in each saved worktree entry. This function answers only "what
/// does the pane actually run".
///
/// The leading whitespace-delimited token of `cli` is looked up in `clis`
/// (the same convention [`resolve_submit_delay_ms`] and the other per-CLI
/// resolvers use). When it names an entry, that token is replaced by the
/// entry's `command`, rendered as a single shell word (quoted only when it
/// contains a character outside the portable safe set), and any trailing text
/// is appended verbatim — it was already shell text before the substitution.
/// When it names no entry the identity is returned unchanged, so a built-in
/// detected CLI launches exactly as it did before.
///
/// Approval flags and session-id / resume arguments are appended by the
/// caller, after this command.
#[must_use]
pub fn launch_command<S: std::hash::BuildHasher>(
    cli: &str,
    clis: &HashMap<String, CustomCli, S>,
) -> String {
    // Trim first so the remainder slice below is taken from a string whose
    // leading token really does start at byte 0.
    let trimmed = cli.trim_start();
    let base = trimmed.split_whitespace().next().unwrap_or(trimmed);
    match clis.get(base) {
        Some(entry) => format!("{}{}", shell_word(&entry.command), &trimmed[base.len()..]),
        None => cli.to_string(),
    }
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

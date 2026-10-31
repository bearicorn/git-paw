//! Custom CLI and preset definitions, plus programmatic add/remove of
//! custom CLIs in the global/repo config files.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::PawError;

use super::prompt_profile::CliPromptProfileOverride;
use super::{global_config_path, load_config_file, save_config_to};

/// A custom CLI definition from config.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
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
    /// Whether this CLI speaks git-paw's MCP protocol, overriding its
    /// provider family's default ([`crate::agent_cli::CliProfile::mcp_capable`]).
    ///
    /// Unset (the default, and omitted on serialization) means "use the
    /// family default": the `claude`, `codex`, `agy` and `cursor-agent`
    /// families are MCP-capable. A custom entry with no `family` is
    /// `generic`, so it gets the helper-script boot form unless it opts in
    /// here or declares a family.
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
    /// The arguments that make this CLI run one non-interactive turn reading
    /// its prompt from stdin, consulted BEFORE the built-in table by
    /// [`resolve_headless_args`].
    ///
    /// Used by the drive loop's headless orchestrator hand-off
    /// (`orchestrator-escalation-handoff`). An empty string is rejected at
    /// config load, naming the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headless_args: Option<String>,
    /// The provider family this CLI is a variant of (`cli-provider`):
    /// `claude`, `codex`, `agy`, `qwen`, `cursor-agent` or `generic`.
    ///
    /// Supplies the family's built-in approval flags, session-id / resume /
    /// headless templates and MCP default wherever this entry sets no
    /// override. Unset, a known CLI name keeps its own family and any other
    /// name resolves `generic`; the family is never inferred from `command`.
    /// An unknown value is rejected at config load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// Arguments appended verbatim after the executable on every launch of
    /// this CLI, before the approval flags (`cli-provider`).
    ///
    /// Per-worker variation (model, effort) is one `[clis.*]` entry per worker
    /// profile, each with its own `args`. Put permission flags in
    /// [`Self::approval_args`], not here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
}

/// Resolves the headless (one-shot, stdin-reading) arguments for `cli`,
/// consulting (in order): the `[clis.<name>].headless_args` override, the
/// built-in row of the CLI's provider family (Claude Code's `-p`, Codex's
/// `exec`), then `None` — the CLI has no headless form.
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use git_paw::config::{resolve_headless_args, CustomCli};
///
/// let clis: HashMap<String, CustomCli> = HashMap::new();
/// assert_eq!(resolve_headless_args("claude", &clis).as_deref(), Some("-p"));
/// assert_eq!(resolve_headless_args("codex", &clis).as_deref(), Some("exec"));
/// assert_eq!(resolve_headless_args("my-agent", &clis), None);
/// ```
#[must_use]
pub fn resolve_headless_args<S: std::hash::BuildHasher>(
    cli: &str,
    clis: &HashMap<String, CustomCli, S>,
) -> Option<String> {
    crate::agent_cli::CliProfile::resolve(cli, clis).headless_args()
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
            headless_args: None,
            family: None,
            args: None,
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

/// Resolve the per-CLI settle delay (ms) for the CLI identity `cli` from
/// `clis`, falling back to [`crate::DEFAULT_SUBMIT_DELAY_MS`].
///
/// Delegates to the CLI's resolved record
/// ([`crate::agent_cli::CliProfile::submit_delay_ms`]), keyed by its id, so a
/// variant whose `command` differs from its key keeps its own delay. The delay is config-driven, never a hardcoded
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
    crate::agent_cli::CliProfile::resolve(cli, clis).submit_delay_ms()
}

//! Per-CLI prompt-shape profile (`cli-prompt-profiles` capability).
//!
//! A [`CliPromptProfile`] describes how one agent CLI's terminal surface
//! *looks* — readiness markers, approval markers, live-prompt markers, and so
//! on. It describes shape only: it has no field capable of expressing a
//! permission decision. The danger list, protected-path rules, worktree
//! boundary, broad-grant eligibility, and the approval send gate stay
//! compiled in [`crate::supervisor::auto_approve`] and are never sourced from
//! this type (design D1).
//!
//! The Claude Code profile is embedded as the compiled default
//! ([`claude_prompt_profile`]), populated verbatim from the literals that
//! were previously scattered across `src/tmux/readiness.rs`,
//! `src/supervisor/permission_prompt.rs`, `src/supervisor/drive.rs`,
//! `src/supervisor/manual_approvals.rs`, `src/supervisor/auto_approve.rs`,
//! and `src/coordination/inventory.rs`. [`resolve_prompt_profile`] merges a
//! per-CLI override from `[clis.<name>].prompt_profile` over that default,
//! field by field (D2) — an absent field never resolves empty.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::CustomCli;

/// A fully resolved per-CLI prompt-shape profile.
///
/// Every field is populated — either from a `[clis.<name>].prompt_profile`
/// override or from the embedded Claude Code default
/// ([`claude_prompt_profile`]). Marker lists are matched as
/// case-insensitive substrings by convention (callers lower-case before
/// comparing, matching the pre-profile behaviour of each site). Pattern
/// fields are regex source strings.
///
/// This type carries prompt SHAPE only. It has no boolean or enum field that
/// could express a permission decision — see the `cli-prompt-profiles`
/// capability spec, "Security decisions are never profile-sourced".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliPromptProfile {
    /// Substrings that positively identify a launched CLI's interactive
    /// ready state (was `tmux::readiness::CLI_READY_MARKERS`).
    pub readiness_markers: Vec<String>,
    /// Substrings indicating the CLI is waiting for an approval decision
    /// (was `supervisor::permission_prompt::APPROVAL_MARKERS`).
    pub approval_markers: Vec<String>,
    /// Textual markers of a LIVE permission prompt: the confirmation
    /// question lead-in and the cancel footer (was
    /// `supervisor::auto_approve::LIVE_PROMPT_PROCEED` /
    /// `LIVE_PROMPT_FOOTER`).
    pub live_prompt_markers: Vec<String>,
    /// Substrings identifying a pane actively producing a response, rather
    /// than sitting at its input box (was
    /// `supervisor::drive::MID_RESPONSE_MARKERS`).
    pub mid_response_markers: Vec<String>,
    /// Lower-cased substrings marking a captured line as prompt boilerplate
    /// (the question / choices) rather than the command awaiting a decision
    /// (was `supervisor::manual_approvals::PROMPT_BOILERPLATE`).
    pub prompt_boilerplate_markers: Vec<String>,
    /// Lead-in forms of a command-confirmation header, e.g. `Bash command`
    /// or the inline `Bash(` form (was the hardcoded strings in
    /// `supervisor::auto_approve::is_bash_command_header` /
    /// `extract_command_slice`).
    pub command_header_markers: Vec<String>,
    /// Regex source matching the lead-in of a filesystem-operation
    /// permission prompt, capturing the target path (was
    /// `supervisor::auto_approve::file_prompt_regex`).
    pub file_prompt_pattern: String,
    /// Regex source matching a numbered option line, e.g. `1. …` / `2) …`
    /// (was the hardcoded check in `supervisor::auto_approve::is_option_line`).
    pub option_line_pattern: String,
    /// Substring identifying a durable ("don't ask again") broad-grant
    /// option in a multi-option prompt (was the hardcoded string in
    /// `supervisor::auto_approve::detect_prompt_shape`).
    pub broad_grant_marker: String,
    /// Prefixes identifying the CLI's input-box sigil, e.g. `>` / `❯` (was
    /// the hardcoded strings in `supervisor::drive::input_box_text`).
    pub input_sigils: Vec<String>,
    /// Substrings identifying an accept-edits / bypass-permissions mode
    /// footer (was part of `coordination::inventory::detect_mode`).
    pub mode_accept_edits_markers: Vec<String>,
    /// Substrings identifying a visible interactive prompt for mode
    /// detection (was part of `coordination::inventory::detect_mode`).
    pub mode_interactive_markers: Vec<String>,
    /// Substrings/phrases indicating a CLI API stream failed mid-turn
    /// (transport error, timeout, disconnect). Consulted today only by the
    /// bundled `sweep.sh` helper's stuck-shape detector.
    pub stream_error_markers: Vec<String>,
    /// Regex source matching a context-bloat clear/compact hint, capturing
    /// the token count in thousands. Consulted today only by the bundled
    /// `sweep.sh` helper's stuck-shape detector.
    pub context_bloat_pattern: String,
    /// Regex source matching a paste-buffer marker, e.g. `Pasted text #1`.
    /// Consulted today only by the bundled `sweep.sh` helper's stuck-shape
    /// detector.
    pub paste_buffer_pattern: String,
}

/// A partial per-CLI prompt-shape profile override, configured under
/// `[clis.<name>].prompt_profile`.
///
/// Every field is optional. An absent field falls back to the embedded
/// Claude Code default field-by-field via [`resolve_prompt_profile`] (D2) —
/// a partial override can never silently disable detection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliPromptProfileOverride {
    /// See [`CliPromptProfile::readiness_markers`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness_markers: Option<Vec<String>>,
    /// See [`CliPromptProfile::approval_markers`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_markers: Option<Vec<String>>,
    /// See [`CliPromptProfile::live_prompt_markers`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_prompt_markers: Option<Vec<String>>,
    /// See [`CliPromptProfile::mid_response_markers`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mid_response_markers: Option<Vec<String>>,
    /// See [`CliPromptProfile::prompt_boilerplate_markers`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_boilerplate_markers: Option<Vec<String>>,
    /// See [`CliPromptProfile::command_header_markers`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_header_markers: Option<Vec<String>>,
    /// See [`CliPromptProfile::file_prompt_pattern`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_prompt_pattern: Option<String>,
    /// See [`CliPromptProfile::option_line_pattern`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub option_line_pattern: Option<String>,
    /// See [`CliPromptProfile::broad_grant_marker`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broad_grant_marker: Option<String>,
    /// See [`CliPromptProfile::input_sigils`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_sigils: Option<Vec<String>>,
    /// See [`CliPromptProfile::mode_accept_edits_markers`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_accept_edits_markers: Option<Vec<String>>,
    /// See [`CliPromptProfile::mode_interactive_markers`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_interactive_markers: Option<Vec<String>>,
    /// See [`CliPromptProfile::stream_error_markers`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_error_markers: Option<Vec<String>>,
    /// See [`CliPromptProfile::context_bloat_pattern`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_bloat_pattern: Option<String>,
    /// See [`CliPromptProfile::paste_buffer_pattern`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_buffer_pattern: Option<String>,
}

/// Returns the embedded Claude Code prompt-shape profile — the compiled
/// default for every field, populated verbatim from the literals every
/// marker site used before profiles existed.
///
/// Single-binary distribution is preserved: this is a plain Rust literal, so
/// it resolves with no accompanying profile file on disk.
#[must_use]
pub fn claude_prompt_profile() -> &'static CliPromptProfile {
    static PROFILE: std::sync::OnceLock<CliPromptProfile> = std::sync::OnceLock::new();
    PROFILE.get_or_init(|| CliPromptProfile {
        readiness_markers: vec![
            "? for shortcuts".to_string(),
            "? for help".to_string(),
            "Welcome to Claude Code".to_string(),
            "esc to interrupt".to_string(),
            "Bypassing Permissions".to_string(),
            "│ >".to_string(),
        ],
        approval_markers: vec![
            "requires approval".to_string(),
            "do you want to proceed".to_string(),
            "do you want to allow".to_string(),
            "(y/n)".to_string(),
            "[y/N]".to_string(),
            "Allow this command".to_string(),
        ],
        live_prompt_markers: vec!["do you want to".to_string(), "esc to cancel".to_string()],
        mid_response_markers: vec![
            "esc to interrupt".to_string(),
            "ctrl+c to interrupt".to_string(),
        ],
        prompt_boilerplate_markers: vec![
            "requires approval".to_string(),
            "do you want".to_string(),
            "bash command".to_string(),
            "allow this command".to_string(),
            "[y/n]".to_string(),
            "(y/n)".to_string(),
            "press ".to_string(),
            "esc to".to_string(),
            "1. yes".to_string(),
            "2. no".to_string(),
            "❯".to_string(),
        ],
        command_header_markers: vec!["Bash command".to_string(), "Bash(".to_string()],
        file_prompt_pattern: r"(?i)(?:allow this write to|allow this edit to|make this edit to|write to|create file|edit file|write file)\s+(?:the file\s+)?(.+?)\s*\??\s*$".to_string(),
        option_line_pattern: r"^[0-9][.)]".to_string(),
        broad_grant_marker: "don't ask again".to_string(),
        input_sigils: vec![">".to_string(), "❯".to_string()],
        mode_accept_edits_markers: vec![
            "accept edits".to_string(),
            "accept-edits".to_string(),
            "bypass permissions".to_string(),
        ],
        mode_interactive_markers: vec![
            "? for shortcuts".to_string(),
            "do you want to proceed".to_string(),
            "do you want to allow".to_string(),
            "(y/n)".to_string(),
            "[y/n]".to_string(),
            "❯ 1. yes".to_string(),
        ],
        stream_error_markers: vec![
            "Request timed out".to_string(),
            "Request timeout".to_string(),
            "stream error".to_string(),
            "stream timed out".to_string(),
            "stream disconnected".to_string(),
            "transport error".to_string(),
            "Connection error".to_string(),
            "error streaming".to_string(),
        ],
        context_bloat_pattern: r"/?(clear|compact) to save ([0-9]+)k tokens".to_string(),
        paste_buffer_pattern: r"Pasted text #[0-9]".to_string(),
    })
}

/// Resolves the leading binary token of `cli`, matching the convention
/// [`super::resolve_submit_delay_ms`] uses so a CLI invoked with trailing
/// flags (e.g. `"mycli --foo"`) still keys on its own `[clis.<name>]` entry.
fn base_token(cli: &str) -> &str {
    cli.split_whitespace().next().unwrap_or(cli)
}

/// Resolves the effective [`CliPromptProfile`] for `cli`, consulting (per
/// field, in order): the `[clis.<name>].prompt_profile` override, then the
/// embedded Claude Code default ([`claude_prompt_profile`]).
///
/// An absent override — either no `[clis.<name>]` entry at all, an entry
/// with no `prompt_profile` table, or a `prompt_profile` table missing a
/// given field — resolves that field to the embedded default rather than
/// empty (D2). A CLI with no profile of its own therefore degrades to
/// today's behaviour rather than to no detection at all (`cli-prompt-profiles`
/// capability, "An unrecognised CLI resolves a usable profile").
#[must_use]
pub fn resolve_prompt_profile<S: std::hash::BuildHasher>(
    cli: &str,
    clis: &HashMap<String, CustomCli, S>,
) -> CliPromptProfile {
    let default = claude_prompt_profile();
    let Some(override_) = clis
        .get(base_token(cli))
        .and_then(|c| c.prompt_profile.as_ref())
    else {
        return default.clone();
    };
    CliPromptProfile {
        readiness_markers: override_
            .readiness_markers
            .clone()
            .unwrap_or_else(|| default.readiness_markers.clone()),
        approval_markers: override_
            .approval_markers
            .clone()
            .unwrap_or_else(|| default.approval_markers.clone()),
        live_prompt_markers: override_
            .live_prompt_markers
            .clone()
            .unwrap_or_else(|| default.live_prompt_markers.clone()),
        mid_response_markers: override_
            .mid_response_markers
            .clone()
            .unwrap_or_else(|| default.mid_response_markers.clone()),
        prompt_boilerplate_markers: override_
            .prompt_boilerplate_markers
            .clone()
            .unwrap_or_else(|| default.prompt_boilerplate_markers.clone()),
        command_header_markers: override_
            .command_header_markers
            .clone()
            .unwrap_or_else(|| default.command_header_markers.clone()),
        file_prompt_pattern: override_
            .file_prompt_pattern
            .clone()
            .unwrap_or_else(|| default.file_prompt_pattern.clone()),
        option_line_pattern: override_
            .option_line_pattern
            .clone()
            .unwrap_or_else(|| default.option_line_pattern.clone()),
        broad_grant_marker: override_
            .broad_grant_marker
            .clone()
            .unwrap_or_else(|| default.broad_grant_marker.clone()),
        input_sigils: override_
            .input_sigils
            .clone()
            .unwrap_or_else(|| default.input_sigils.clone()),
        mode_accept_edits_markers: override_
            .mode_accept_edits_markers
            .clone()
            .unwrap_or_else(|| default.mode_accept_edits_markers.clone()),
        mode_interactive_markers: override_
            .mode_interactive_markers
            .clone()
            .unwrap_or_else(|| default.mode_interactive_markers.clone()),
        stream_error_markers: override_
            .stream_error_markers
            .clone()
            .unwrap_or_else(|| default.stream_error_markers.clone()),
        context_bloat_pattern: override_
            .context_bloat_pattern
            .clone()
            .unwrap_or_else(|| default.context_bloat_pattern.clone()),
        paste_buffer_pattern: override_
            .paste_buffer_pattern
            .clone()
            .unwrap_or_else(|| default.paste_buffer_pattern.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_default_covers_every_field_non_empty() {
        let p = claude_prompt_profile();
        assert!(!p.readiness_markers.is_empty());
        assert!(!p.approval_markers.is_empty());
        assert!(!p.live_prompt_markers.is_empty());
        assert!(!p.mid_response_markers.is_empty());
        assert!(!p.prompt_boilerplate_markers.is_empty());
        assert!(!p.command_header_markers.is_empty());
        assert!(!p.file_prompt_pattern.is_empty());
        assert!(!p.option_line_pattern.is_empty());
        assert!(!p.broad_grant_marker.is_empty());
        assert!(!p.input_sigils.is_empty());
        assert!(!p.mode_accept_edits_markers.is_empty());
        assert!(!p.mode_interactive_markers.is_empty());
        assert!(!p.stream_error_markers.is_empty());
        assert!(!p.context_bloat_pattern.is_empty());
        assert!(!p.paste_buffer_pattern.is_empty());
    }

    #[test]
    fn unknown_cli_resolves_the_embedded_default() {
        let clis: HashMap<String, CustomCli> = HashMap::new();
        assert_eq!(
            resolve_prompt_profile("some-agent", &clis),
            *claude_prompt_profile()
        );
    }

    #[test]
    fn no_config_file_present_resolves_the_embedded_default() {
        // Single-binary distribution: an empty map (no config loaded at
        // all) resolves the same as an unrecognised CLI.
        let clis: HashMap<String, CustomCli> = HashMap::new();
        assert_eq!(
            resolve_prompt_profile("claude", &clis),
            *claude_prompt_profile()
        );
    }

    #[test]
    fn partial_override_falls_back_per_field() {
        let mut clis: HashMap<String, CustomCli> = HashMap::new();
        clis.insert(
            "mycli".to_string(),
            CustomCli {
                command: "mycli".to_string(),
                display_name: None,
                submit_delay_ms: None,
                settings_path: None,
                approval_args: HashMap::new(),
                prompt_profile: Some(CliPromptProfileOverride {
                    approval_markers: Some(vec!["waiting for go-ahead".to_string()]),
                    ..Default::default()
                }),
            },
        );
        let resolved = resolve_prompt_profile("mycli", &clis);
        assert_eq!(
            resolved.approval_markers,
            vec!["waiting for go-ahead".to_string()]
        );
        // Every other field falls back to the embedded default rather than
        // resolving empty (D2).
        assert_eq!(
            resolved.readiness_markers,
            claude_prompt_profile().readiness_markers
        );
        assert_eq!(
            resolved.mid_response_markers,
            claude_prompt_profile().mid_response_markers
        );
        assert_eq!(
            resolved.broad_grant_marker,
            claude_prompt_profile().broad_grant_marker
        );
    }

    #[test]
    fn override_wins_over_default_when_present() {
        let mut clis: HashMap<String, CustomCli> = HashMap::new();
        clis.insert(
            "mycli".to_string(),
            CustomCli {
                command: "mycli".to_string(),
                display_name: None,
                submit_delay_ms: None,
                settings_path: None,
                approval_args: HashMap::new(),
                prompt_profile: Some(CliPromptProfileOverride {
                    readiness_markers: Some(vec!["ready>".to_string()]),
                    ..Default::default()
                }),
            },
        );
        assert_eq!(
            resolve_prompt_profile("mycli", &clis).readiness_markers,
            vec!["ready>".to_string()]
        );
    }

    #[test]
    fn resolution_keys_on_the_leading_binary_token() {
        let mut clis: HashMap<String, CustomCli> = HashMap::new();
        clis.insert(
            "mycli".to_string(),
            CustomCli {
                command: "mycli".to_string(),
                display_name: None,
                submit_delay_ms: None,
                settings_path: None,
                approval_args: HashMap::new(),
                prompt_profile: Some(CliPromptProfileOverride {
                    broad_grant_marker: Some("never ask again".to_string()),
                    ..Default::default()
                }),
            },
        );
        assert_eq!(
            resolve_prompt_profile("mycli --dangerously-skip-permissions", &clis)
                .broad_grant_marker,
            "never ask again"
        );
    }
}

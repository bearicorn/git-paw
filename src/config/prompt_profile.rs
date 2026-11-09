//! Per-CLI prompt-shape profile (`cli-prompt-profiles` capability).
//!
//! A [`CliPromptProfile`] describes how one agent CLI's terminal surface
//! *looks* — readiness markers, approval markers, live-prompt markers, and so
//! on. It describes shape only: it has no field capable of expressing a
//! permission decision — and after the classifier's retirement there is no
//! permission decision in git-paw for it to express. What stays compiled,
//! and is never sourced from this type, is the small security-relevant set:
//! the live-prompt gate's window sizes and structural rules
//! ([`crate::supervisor::permission_prompt`]), the pane-0 refusal, and
//! `sweep.sh approve`'s rule that it presses option 1 and nothing else
//! (design D1). A hostile profile can change what git-paw *recognises*; it
//! cannot change what git-paw *does*.
//!
//! The Claude Code profile is embedded as the compiled default
//! ([`claude_prompt_profile`]), populated verbatim from the literals that
//! were previously scattered across `src/tmux/readiness.rs`,
//! `src/supervisor/permission_prompt.rs`, `src/supervisor/drive.rs`,
//! `src/supervisor/manual_approvals.rs`, and
//! `src/coordination/inventory.rs`. [`resolve_prompt_profile`] merges a
//! per-CLI override from `[clis.<name>].prompt_profile` over that default,
//! field by field (D2) — an absent field never resolves empty.

use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::CustomCli;
use crate::static_regex::static_regex;

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
    /// `supervisor::permission_prompt::LIVE_PROMPT_PROCEED` /
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
    /// `supervisor::permission_prompt::is_bash_command_header` /
    /// `extract_command_slice`).
    pub command_header_markers: Vec<String>,
    /// Regex source matching the lead-in of a filesystem-operation
    /// permission prompt, capturing the target path (was
    /// `supervisor::permission_prompt::file_prompt_regex`).
    pub file_prompt_pattern: String,
    /// Regex source matching a numbered option line, e.g. `1. …` / `2) …`
    /// (was the hardcoded check in `supervisor::permission_prompt::is_option_line`).
    pub option_line_pattern: String,
    /// Substring identifying a durable ("don't ask again") broad-grant
    /// option in a multi-option prompt.
    ///
    /// Consumed by `sweep.sh approve`'s option-1 guard: the helper refuses to
    /// press option 1 when that line itself carries this marker, because
    /// answering would write a permanent permission rule the human did not
    /// ask for. Relaxing the marker can only make the guard refuse MORE
    /// often, so a profile cannot use it to widen what git-paw will press.
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
    /// Substrings/phrases indicating the CLI itself has stopped serving —
    /// a usage/session limit or an expired/missing login — rather than a
    /// worker being merely stuck. Matched tail-only against the live-prompt
    /// scan's capture (`drive-loop-exit-summary-truth`): a marker in an
    /// orchestrator pane is a loud, explicit `cli-unavailable` exit; in a
    /// coding pane it is a deduped escalation.
    pub cli_unavailable_markers: Vec<String>,
    /// Glyphs that mark a selection menu's selected option, e.g. `❯`.
    /// Consulted by `__send` before it presses a key.
    pub menu_cursor_glyphs: Vec<String>,
    /// Footer phrases that mark a selection menu, e.g. `enter to select`.
    /// Matched case-insensitively.
    pub menu_footer_markers: Vec<String>,
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
    /// See [`CliPromptProfile::cli_unavailable_markers`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli_unavailable_markers: Option<Vec<String>>,
    /// See [`CliPromptProfile::menu_cursor_glyphs`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu_cursor_glyphs: Option<Vec<String>>,
    /// See [`CliPromptProfile::menu_footer_markers`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu_footer_markers: Option<Vec<String>>,
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
        file_prompt_pattern: DEFAULT_FILE_PROMPT_PATTERN.to_string(),
        option_line_pattern: DEFAULT_OPTION_LINE_PATTERN.to_string(),
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
            // Mid-response connection drop: the conversation context survives
            // a transient drop like this — a nudge resumes the worker — so it
            // belongs here, never in `cli_unavailable_markers` (reserved for
            // terminal conditions requiring human intervention).
            "API Error".to_string(),
            "Connection closed mid-response".to_string(),
        ],
        context_bloat_pattern: DEFAULT_CONTEXT_BLOAT_PATTERN.to_string(),
        paste_buffer_pattern: DEFAULT_PASTE_BUFFER_PATTERN.to_string(),
        cli_unavailable_markers: vec![
            "You've hit your session limit".to_string(),
            "usage limit reached".to_string(),
            "Please run /login".to_string(),
            "Invalid API key".to_string(),
            "org's monthly spend limit".to_string(),
        ],
        menu_cursor_glyphs: vec!["❯".to_string(), "›".to_string(), ">".to_string()],
        menu_footer_markers: vec!["enter to select".to_string()],
    })
}

const DEFAULT_FILE_PROMPT_PATTERN: &str = r"(?i)(?:allow this write to|allow this edit to|make this edit to|write to|create file|edit file|write file)\s+(?:the file\s+)?(.+?)\s*\??\s*$";
const DEFAULT_OPTION_LINE_PATTERN: &str = r"^[0-9][.)]";
const DEFAULT_CONTEXT_BLOAT_PATTERN: &str = r"/?(clear|compact) to save ([0-9]+)k tokens";
const DEFAULT_PASTE_BUFFER_PATTERN: &str = r"Pasted text #[0-9]";

/// A pattern field that did not compile.
#[derive(Debug, Clone)]
pub struct InvalidPattern {
    /// The field's config key, e.g. `option_line_pattern`.
    pub field: &'static str,
    /// Why it did not compile.
    pub error: regex::Error,
}

/// A resolved [`CliPromptProfile`] with its pattern fields compiled once; a
/// pattern equal to the default's reuses the default's compiled regex. Every
/// detector takes the one for the CLI on the surface it inspects.
#[derive(Debug, Clone)]
pub struct CompiledPromptProfile {
    shape: CliPromptProfile,
    file_prompt: Regex,
    option_line: Regex,
    paste_buffer: Regex,
}

impl CompiledPromptProfile {
    /// Compiles `shape`'s pattern fields.
    ///
    /// # Errors
    ///
    /// The first pattern field that is not a valid regular expression.
    pub fn compile(shape: CliPromptProfile) -> Result<Self, InvalidPattern> {
        let default = default_compiled_prompt_profile();
        let compile = |field: &'static str, source: &str, fallback: &Regex| {
            if source == fallback.as_str() {
                Ok(fallback.clone())
            } else {
                Regex::new(source).map_err(|error| InvalidPattern { field, error })
            }
        };
        let file_prompt = compile(
            "file_prompt_pattern",
            &shape.file_prompt_pattern,
            &default.file_prompt,
        )?;
        let option_line = compile(
            "option_line_pattern",
            &shape.option_line_pattern,
            &default.option_line,
        )?;
        let paste_buffer = compile(
            "paste_buffer_pattern",
            &shape.paste_buffer_pattern,
            &default.paste_buffer,
        )?;
        if shape.context_bloat_pattern != DEFAULT_CONTEXT_BLOAT_PATTERN {
            Regex::new(&shape.context_bloat_pattern).map_err(|error| InvalidPattern {
                field: "context_bloat_pattern",
                error,
            })?;
        }
        Ok(Self {
            shape,
            file_prompt,
            option_line,
            paste_buffer,
        })
    }

    /// The compiled `file_prompt_pattern`.
    #[must_use]
    pub fn file_prompt_regex(&self) -> &Regex {
        &self.file_prompt
    }

    /// The compiled `option_line_pattern`.
    #[must_use]
    pub fn option_line_regex(&self) -> &Regex {
        &self.option_line
    }

    /// The compiled `paste_buffer_pattern`.
    #[must_use]
    pub fn paste_buffer_regex(&self) -> &Regex {
        &self.paste_buffer
    }
}

/// Two compiled profiles are equal when their sources are: every compiled
/// pattern is a function of its source string.
impl PartialEq for CompiledPromptProfile {
    fn eq(&self, other: &Self) -> bool {
        self.shape == other.shape
    }
}

impl Eq for CompiledPromptProfile {}

impl std::ops::Deref for CompiledPromptProfile {
    type Target = CliPromptProfile;

    fn deref(&self) -> &CliPromptProfile {
        &self.shape
    }
}

/// The embedded default. Read only by profile resolution, never by a detector.
pub(crate) fn default_compiled_prompt_profile() -> &'static CompiledPromptProfile {
    static PROFILE: std::sync::OnceLock<CompiledPromptProfile> = std::sync::OnceLock::new();
    PROFILE.get_or_init(|| CompiledPromptProfile {
        shape: claude_prompt_profile().clone(),
        file_prompt: static_regex(DEFAULT_FILE_PROMPT_PATTERN),
        option_line: static_regex(DEFAULT_OPTION_LINE_PATTERN),
        paste_buffer: static_regex(DEFAULT_PASTE_BUFFER_PATTERN),
    })
}

/// Rust-regex constructs `grep -E` (POSIX ERE) cannot run. A pattern the
/// bundled `sweep.sh` also matches must stay within the subset both accept.
const NON_ERE_CONSTRUCTS: &[&str] = &[
    "(?", "\\d", "\\D", "\\s", "\\S", "\\w", "\\W", "\\p", "\\P", "\\A", "\\z", "*?", "+?", "??",
    "}?",
];

/// Validates every field `override_` sets, naming the first bad one as
/// `[clis.<cli>].prompt_profile.<field>`:
///
/// - every pattern compiles as a Rust regex;
/// - a pattern `sweep.sh` also matches with `grep -E` (`option_line_pattern`,
///   `context_bloat_pattern`, `paste_buffer_pattern`) uses none of
///   [`NON_ERE_CONSTRUCTS`];
/// - no marker is empty or whitespace-only — under `grep -E` an empty
///   alternative matches every line.
///
/// # Errors
///
/// The message for the first invalid field.
pub(crate) fn validate_override_patterns(
    cli: &str,
    override_: &CliPromptProfileOverride,
) -> Result<(), String> {
    let fail =
        |field: &str, why: String| Err(format!("[clis.{cli}].prompt_profile.{field}: {why}"));
    for (field, set, shell) in [
        ("file_prompt_pattern", &override_.file_prompt_pattern, false),
        ("option_line_pattern", &override_.option_line_pattern, true),
        (
            "context_bloat_pattern",
            &override_.context_bloat_pattern,
            true,
        ),
        (
            "paste_buffer_pattern",
            &override_.paste_buffer_pattern,
            true,
        ),
    ] {
        let Some(source) = set else { continue };
        if let Err(error) = Regex::new(source) {
            return fail(field, error.to_string());
        }
        if shell && let Some(c) = NON_ERE_CONSTRUCTS.iter().find(|c| source.contains(**c)) {
            return fail(
                field,
                format!("`{c}` is not supported by grep -E, which sweep.sh matches it with"),
            );
        }
    }
    for (field, set) in [
        ("readiness_markers", &override_.readiness_markers),
        ("approval_markers", &override_.approval_markers),
        ("live_prompt_markers", &override_.live_prompt_markers),
        ("mid_response_markers", &override_.mid_response_markers),
        (
            "prompt_boilerplate_markers",
            &override_.prompt_boilerplate_markers,
        ),
        ("command_header_markers", &override_.command_header_markers),
        ("input_sigils", &override_.input_sigils),
        (
            "mode_accept_edits_markers",
            &override_.mode_accept_edits_markers,
        ),
        (
            "mode_interactive_markers",
            &override_.mode_interactive_markers,
        ),
        ("stream_error_markers", &override_.stream_error_markers),
        (
            "cli_unavailable_markers",
            &override_.cli_unavailable_markers,
        ),
        ("menu_cursor_glyphs", &override_.menu_cursor_glyphs),
        ("menu_footer_markers", &override_.menu_footer_markers),
    ] {
        if set.iter().flatten().any(|m| m.trim().is_empty()) {
            return fail(
                field,
                "a marker is empty, and would match every line".to_string(),
            );
        }
    }
    for (field, set) in [
        ("menu_cursor_glyphs", &override_.menu_cursor_glyphs),
        ("menu_footer_markers", &override_.menu_footer_markers),
    ] {
        if set.as_ref().is_some_and(Vec::is_empty) {
            return fail(
                field,
                "the list is empty, which would turn off the selection-menu guard".to_string(),
            );
        }
    }
    if override_
        .broad_grant_marker
        .as_ref()
        .is_some_and(|m| m.trim().is_empty())
    {
        return fail(
            "broad_grant_marker",
            "the marker is empty, and would match every line".to_string(),
        );
    }
    Ok(())
}

/// Resolves the effective [`CliPromptProfile`] for the CLI identity `cli`:
/// the `[clis.<id>].prompt_profile` override field by field over the
/// provider family's profile.
///
/// An absent override falls back to the default for that field, not to an
/// empty value, so a CLI with no profile still gets usable detection (D2).
#[must_use]
pub fn resolve_prompt_profile<S: std::hash::BuildHasher>(
    cli: &str,
    clis: &HashMap<String, CustomCli, S>,
) -> CliPromptProfile {
    crate::agent_cli::CliProfile::resolve(cli, clis).prompt_profile()
}

/// Overlays `override_` on `default` field by field: a field the override
/// sets wins, every other field keeps the default's value.
#[must_use]
pub(crate) fn merge_prompt_profile(
    default: &CliPromptProfile,
    override_: Option<&CliPromptProfileOverride>,
) -> CliPromptProfile {
    let Some(override_) = override_ else {
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
        cli_unavailable_markers: override_
            .cli_unavailable_markers
            .clone()
            .unwrap_or_else(|| default.cli_unavailable_markers.clone()),
        menu_cursor_glyphs: override_
            .menu_cursor_glyphs
            .clone()
            .unwrap_or_else(|| default.menu_cursor_glyphs.clone()),
        menu_footer_markers: override_
            .menu_footer_markers
            .clone()
            .unwrap_or_else(|| default.menu_footer_markers.clone()),
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
        assert!(!p.cli_unavailable_markers.is_empty());
    }

    /// `drive-loop-exit-summary-truth` task 3.3: the default resolves the
    /// session-limit marker observed in the 0.16.0 dogfood.
    #[test]
    fn default_resolves_the_session_limit_marker() {
        let clis: HashMap<String, CustomCli> = HashMap::new();
        let resolved = resolve_prompt_profile("claude", &clis);
        assert!(
            resolved
                .cli_unavailable_markers
                .iter()
                .any(|m| m == "You've hit your session limit"),
            "{:?}",
            resolved.cli_unavailable_markers
        );
    }

    /// A mid-response connection drop is a stream error, not CLI
    /// unavailability (amendment, 2026-09-15): the context survives and a
    /// nudge resumes the worker, so it must never fire the loud
    /// `cli-unavailable` exit reserved for conditions needing a human.
    #[test]
    fn connection_drop_matches_stream_error_not_cli_unavailable() {
        let p = claude_prompt_profile();
        let capture = "API Error: Connection closed mid-response";
        let lower = capture.to_ascii_lowercase();
        assert!(
            p.stream_error_markers
                .iter()
                .any(|m| lower.contains(&m.to_ascii_lowercase())),
            "{:?}",
            p.stream_error_markers
        );
        assert!(
            !p.cli_unavailable_markers
                .iter()
                .any(|m| lower.contains(&m.to_ascii_lowercase())),
            "{:?}",
            p.cli_unavailable_markers
        );
    }

    /// An override wins over the default for `cli_unavailable_markers`.
    #[test]
    fn cli_unavailable_markers_override_wins_over_default() {
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
                    cli_unavailable_markers: Some(vec!["out of tokens".to_string()]),
                    ..Default::default()
                }),
                session_id_args: None,
                resume_args: None,
                mcp: None,
                headless_args: None,
                family: None,
                args: None,
                sandbox: None,
                sandbox_args: None,
            },
        );
        assert_eq!(
            resolve_prompt_profile("mycli", &clis).cli_unavailable_markers,
            vec!["out of tokens".to_string()]
        );
    }

    /// A partial override lacking `cli_unavailable_markers` falls back to the
    /// embedded default for that field (D2), same as every other field.
    #[test]
    fn cli_unavailable_markers_falls_back_when_absent_from_override() {
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
                session_id_args: None,
                resume_args: None,
                mcp: None,
                headless_args: None,
                family: None,
                args: None,
                sandbox: None,
                sandbox_args: None,
            },
        );
        assert_eq!(
            resolve_prompt_profile("mycli", &clis).cli_unavailable_markers,
            claude_prompt_profile().cli_unavailable_markers
        );
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
                session_id_args: None,
                resume_args: None,
                mcp: None,
                headless_args: None,
                family: None,
                args: None,
                sandbox: None,
                sandbox_args: None,
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
                session_id_args: None,
                resume_args: None,
                mcp: None,
                headless_args: None,
                family: None,
                args: None,
                sandbox: None,
                sandbox_args: None,
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
                session_id_args: None,
                resume_args: None,
                mcp: None,
                headless_args: None,
                family: None,
                args: None,
                sandbox: None,
                sandbox_args: None,
            },
        );
        assert_eq!(
            resolve_prompt_profile("mycli --dangerously-skip-permissions", &clis)
                .broad_grant_marker,
            "never ask again"
        );
    }
}

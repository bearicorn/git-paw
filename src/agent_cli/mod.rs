//! What each agent CLI is and how to drive it (`cli-provider` capability).
//!
//! "Agent CLI" is the coding-agent program a pane runs — not git-paw's own
//! clap surface, which lives in [`crate::cli`]. Every per-CLI fact the code
//! encodes lives here, in one place:
//!
//! - [`CliFamily`] — the closed set of provider families, parsed from the
//!   `[clis.<name>].family` key.
//! - [`CliProvider`] — one in-tree implementation per family. Its default
//!   methods are the Null Object (no flags, no templates, not MCP-capable, the
//!   embedded prompt profile), so the `generic` family overrides nothing and an
//!   unrecognised CLI degrades to launch-and-observe.
//! - [`ROSTER`] — the binary names auto-detected on PATH, each mapped to
//!   exactly one family. No other list of known CLI names exists in the code.
//! - [`CliProfile`] — the record one agent CLI resolves into: its id, launch
//!   command and arguments, family, and every facet consumers read.
//!
//! [`CliFamily::provider`] is the only exhaustive match over the families, so a
//! new family is decided in one place.

mod profile;
pub mod usage;

use std::fmt;
use std::str::FromStr;

use crate::config::{ApprovalLevel, CliPromptProfile, claude_prompt_profile};
use crate::error::PawError;

pub use profile::{
    AgentCli, AgentProfiles, CliProfile, LaunchSession, Resolved, resolve_for_launch,
};
pub use usage::{UsageCache, UsageQuery, UsageReport, UsageSummary, UsageTotals};

/// A provider family: which in-tree [`CliProvider`] supplies a CLI's built-in
/// facts.
///
/// A known CLI resolves its own family; a `[clis.<name>]` entry resolves the
/// family it declares with `family`, else [`CliFamily::Generic`]. The family is
/// never inferred from the basename of `command`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CliFamily {
    /// Claude Code.
    Claude,
    /// Codex CLI.
    Codex,
    /// Antigravity CLI (`agy`).
    Antigravity,
    /// Qwen Code.
    Qwen,
    /// Cursor's agent CLI (`cursor-agent`).
    Cursor,
    /// No encoded facts: launch and observe with the embedded defaults.
    Generic,
}

impl CliFamily {
    /// Every family, in declaration order.
    pub const ALL: [Self; 6] = [
        Self::Claude,
        Self::Codex,
        Self::Antigravity,
        Self::Qwen,
        Self::Cursor,
        Self::Generic,
    ];

    /// The family's config name, as accepted by `[clis.<name>].family`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        self.provider().name()
    }

    /// The in-tree provider for this family. This is the only exhaustive
    /// match over [`CliFamily`].
    #[must_use]
    pub fn provider(self) -> &'static dyn CliProvider {
        match self {
            Self::Claude => &ClaudeProvider,
            Self::Codex => &CodexProvider,
            Self::Antigravity => &AntigravityProvider,
            Self::Qwen => &QwenProvider,
            Self::Cursor => &CursorProvider,
            Self::Generic => &GenericProvider,
        }
    }

    /// The family of a known CLI name from the [`ROSTER`], or `None` when
    /// `name` is not a known CLI.
    #[must_use]
    pub fn of_known(name: &str) -> Option<Self> {
        roster_entry(name).map(|entry| entry.family)
    }

    /// The accepted config names, comma-separated, for error messages.
    #[must_use]
    pub fn accepted_names() -> String {
        Self::ALL
            .iter()
            .map(|f| format!("\"{}\"", f.as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl fmt::Display for CliFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CliFamily {
    type Err = PawError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|family| family.as_str() == s)
            .ok_or_else(|| {
                PawError::ConfigError(format!(
                    "unknown CLI family '{s}' (expected one of: {})",
                    Self::accepted_names()
                ))
            })
    }
}

/// How a CLI's workers are sandboxed, as declared by `[clis.<name>].sandbox`.
///
/// A declaration is the user's assertion; git-paw never verifies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxMode {
    /// git-paw passes the CLI's own sandbox arguments at launch.
    Native,
    /// `command` is the user's FS-sandbox wrapper; nothing is added.
    Wrapper,
}

impl SandboxMode {
    /// Every mode, in declaration order.
    pub const ALL: [Self; 2] = [Self::Native, Self::Wrapper];

    /// The mode's config value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Wrapper => "wrapper",
        }
    }

    /// The accepted config values, comma-separated, for error messages.
    #[must_use]
    pub fn accepted_names() -> String {
        Self::ALL
            .iter()
            .map(|m| format!("\"{}\"", m.as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl FromStr for SandboxMode {
    type Err = PawError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|mode| mode.as_str() == s)
            .ok_or_else(|| {
                PawError::ConfigError(format!(
                    "unknown sandbox mode '{s}' (expected one of: {})",
                    Self::accepted_names()
                ))
            })
    }
}

/// The built-in facts of one provider family.
///
/// Every method but [`CliProvider::name`] has a default that is the Null
/// Object: no approval flags, no session-id / resume / headless template, not
/// MCP-capable, and the embedded prompt-shape profile every CLI received before
/// per-CLI providers existed. A provider overrides only the facts the code
/// actually encodes for it.
pub trait CliProvider: Sync {
    /// The family's config name (`claude`, `codex`, `agy`, `qwen`,
    /// `cursor-agent`, `generic`).
    fn name(&self) -> &'static str;

    /// Native permission flags for `level`, or `""` when the family has none.
    fn approval_flags(&self, _level: ApprovalLevel) -> &'static str {
        ""
    }

    /// The arguments that turn on the CLI's own sandbox, or `None` when the
    /// family has none encoded.
    fn native_sandbox_args(&self) -> Option<&'static str> {
        None
    }

    /// Flags that turn the CLI's own sandbox off, wherever they come from.
    fn sandbox_disabling_flags(&self) -> &'static [&'static str] {
        &[]
    }

    /// Template assigning a fresh session id at launch (contains
    /// [`crate::config::SESSION_ID_PLACEHOLDER`]), or `None`.
    fn session_id_args(&self) -> Option<&'static str> {
        None
    }

    /// Template resuming a recorded session (contains
    /// [`crate::config::SESSION_ID_PLACEHOLDER`]), or `None`.
    fn resume_args(&self) -> Option<&'static str> {
        None
    }

    /// Arguments that run one non-interactive turn reading the prompt from
    /// stdin, or `None` when the family has no headless form.
    fn headless_args(&self) -> Option<&'static str> {
        None
    }

    /// Whether the family speaks git-paw's MCP protocol, so a worker receives
    /// the MCP form of the boot block.
    fn mcp_capable(&self) -> bool {
        false
    }

    /// The prompt-shape profile the family's terminal is observed with.
    fn prompt_profile(&self) -> &'static CliPromptProfile {
        claude_prompt_profile()
    }

    /// Dialogs the launch gate answers for this family. Fixed in code, not
    /// configurable, because answering one grants a permission. A family
    /// without its own dialogs uses [`DEFAULT_DIALOG_MARKERS`].
    fn dialog_markers(&self) -> &'static [DialogMarker] {
        DEFAULT_DIALOG_MARKERS
    }

    /// The session's token usage as the CLI reports it, or unknown with a
    /// reason naming the CLI when the family declares no usage source.
    fn usage(&self, query: &UsageQuery<'_>) -> UsageReport {
        UsageReport::Unknown(usage::no_source_reason(query.cli_id))
    }

    /// Whether the family reports cost. Claude Code writes no cost figure, so
    /// no in-tree family overrides this.
    fn reports_cost(&self) -> bool {
        false
    }

    /// What keeps a worker from loading the operator's personal configuration
    /// and extensions and from writing memory the operator's sessions share.
    /// Empty for a family with no verified facts.
    fn operator_isolation(&self) -> OperatorIsolation {
        OperatorIsolation::NONE
    }
}

/// A family's operator-isolation launch facts (`cli-provider`): environment
/// assignments placed before the executable (through `env`) and arguments placed after the
/// CLI's `args`. Every value is a compile-time constant, so nothing in it
/// needs quoting. An entry with an empty value clears a variable the worker
/// would otherwise inherit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperatorIsolation {
    /// `(NAME, value)` pairs, in launch order.
    pub env: &'static [(&'static str, &'static str)],
    /// Arguments, or `""` for none.
    pub args: &'static str,
}

impl OperatorIsolation {
    /// No isolation: the Null Object every family without verified facts
    /// returns.
    pub const NONE: Self = Self { env: &[], args: "" };
}

/// Identifies a first-run acceptance/trust dialog (GP-03b) by a heading/body
/// substring, paired with the case-insensitive substring of its affirmative
/// option's OWN text.
///
/// The affirmative option is selected by matching its text, never a
/// hardcoded digit or a blind `Enter` — see [`DEFAULT_DIALOG_MARKERS`] for
/// why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogMarker {
    /// Substring identifying this dialog in a pane capture.
    pub heading: &'static str,
    /// Case-insensitive substring of the option line to select.
    pub affirmative: &'static str,
}

/// The embedded default first-run dialogs (Claude Code's). A dialog whose
/// heading matches none of a provider's markers classifies as not-a-dialog,
/// so an unrecognised CLI's dialog falls back to the readiness / relaunch /
/// fixed-budget behaviour.
///
/// - Claude Code's bypass-permissions acceptance dialog: shown the first time
///   `--dangerously-skip-permissions` runs against a config directory that
///   has never accepted it. Its DEFAULT-highlighted option is `No, exit` —
///   exactly the boot-exit failure mode GP-03 fixes — so the gate selects
///   the affirmative option by TEXT, never a hardcoded digit or position.
/// - Claude Code's "trust this folder?" dialog: shown the first time a CLI
///   launches against a directory it has not seen before.
pub const DEFAULT_DIALOG_MARKERS: &[DialogMarker] = &[
    DialogMarker {
        heading: "Bypass Permissions mode",
        affirmative: "yes, i accept",
    },
    DialogMarker {
        heading: "trust the files in this folder",
        affirmative: "yes, proceed",
    },
];

/// Claude Code. Approval rows verified against upstream 2026-07-15 and the
/// live `--permission-mode` enumeration 2026-09-04; session-id / resume flags
/// 2026-09-15; `-p` headless form 2026-10-29.
struct ClaudeProvider;

impl CliProvider for ClaudeProvider {
    fn name(&self) -> &'static str {
        "claude"
    }
    fn approval_flags(&self, level: ApprovalLevel) -> &'static str {
        match level {
            ApprovalLevel::FullAuto => "--dangerously-skip-permissions",
            ApprovalLevel::Auto => "--permission-mode acceptEdits",
            ApprovalLevel::Manual => "",
        }
    }
    fn session_id_args(&self) -> Option<&'static str> {
        Some("--session-id {{SESSION_ID}}")
    }
    fn resume_args(&self) -> Option<&'static str> {
        Some("--resume {{SESSION_ID}}")
    }
    fn headless_args(&self) -> Option<&'static str> {
        Some("-p")
    }
    fn mcp_capable(&self) -> bool {
        true
    }
    /// Verified against Claude Code 2.1.322 on 2026-11-12
    /// (`worker-operator-isolation` design D5): `--setting-sources
    /// project,local` drops the user settings layer, an inherited
    /// `CLAUDE_CODE_PLUGIN_DIRS` loads plugins whatever the setting sources
    /// unless it is emptied, and `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` turns off
    /// the auto memory every worktree of a repository shares.
    fn operator_isolation(&self) -> OperatorIsolation {
        OperatorIsolation {
            env: &[
                ("CLAUDE_CODE_DISABLE_AUTO_MEMORY", "1"),
                ("CLAUDE_CODE_PLUGIN_DIRS", ""),
            ],
            args: "--setting-sources project,local",
        }
    }
    /// Session transcripts, verified against Claude Code 2.1.322 on
    /// 2026-11-10 (`tests/fixtures/claude_usage/README.md`).
    fn usage(&self, query: &UsageQuery<'_>) -> UsageReport {
        usage::read_claude_usage(query)
    }
}

/// Codex CLI. Approval rows verified 2026-07-15; `exec` headless form
/// 2026-10-29.
struct CodexProvider;

impl CliProvider for CodexProvider {
    fn name(&self) -> &'static str {
        "codex"
    }
    fn approval_flags(&self, level: ApprovalLevel) -> &'static str {
        match level {
            ApprovalLevel::FullAuto => "--dangerously-bypass-approvals-and-sandbox",
            ApprovalLevel::Auto => "--sandbox workspace-write",
            ApprovalLevel::Manual => "",
        }
    }
    fn native_sandbox_args(&self) -> Option<&'static str> {
        Some("--sandbox workspace-write")
    }
    fn sandbox_disabling_flags(&self) -> &'static [&'static str] {
        &["--dangerously-bypass-approvals-and-sandbox"]
    }
    fn headless_args(&self) -> Option<&'static str> {
        Some("exec")
    }
    fn mcp_capable(&self) -> bool {
        true
    }
}

/// Antigravity CLI, which replaces the retired Gemini CLI. Its full-auto mode
/// shares Claude's `--dangerously-skip-permissions`, not Gemini's `--yolo`.
struct AntigravityProvider;

impl CliProvider for AntigravityProvider {
    fn name(&self) -> &'static str {
        "agy"
    }
    fn approval_flags(&self, level: ApprovalLevel) -> &'static str {
        match level {
            ApprovalLevel::FullAuto => "--dangerously-skip-permissions",
            ApprovalLevel::Auto | ApprovalLevel::Manual => "",
        }
    }
    fn mcp_capable(&self) -> bool {
        true
    }
}

/// Qwen Code.
struct QwenProvider;

impl CliProvider for QwenProvider {
    fn name(&self) -> &'static str {
        "qwen"
    }
    fn approval_flags(&self, level: ApprovalLevel) -> &'static str {
        match level {
            ApprovalLevel::FullAuto => "--yolo",
            ApprovalLevel::Auto | ApprovalLevel::Manual => "",
        }
    }
}

/// Cursor's agent CLI.
struct CursorProvider;

impl CliProvider for CursorProvider {
    fn name(&self) -> &'static str {
        "cursor-agent"
    }
    fn mcp_capable(&self) -> bool {
        true
    }
}

/// The Null Object: every default, nothing overridden.
struct GenericProvider;

impl CliProvider for GenericProvider {
    fn name(&self) -> &'static str {
        "generic"
    }
}

/// One auto-detected binary name and the family it resolves.
#[derive(Debug, Clone, Copy)]
pub struct RosterEntry {
    /// The binary name scanned for on PATH, which is also the CLI id.
    pub name: &'static str,
    /// Product name shown to the user when it differs from the capitalised
    /// binary name.
    pub display_name: Option<&'static str>,
    /// The provider family this CLI resolves.
    pub family: CliFamily,
}

const fn known(name: &'static str, family: CliFamily) -> RosterEntry {
    RosterEntry {
        name,
        display_name: None,
        family,
    }
}

/// The detection roster: every binary name git-paw auto-detects on PATH.
///
/// The twelve CLIs whose facts the code does not encode resolve the
/// `generic` family.
pub const ROSTER: &[RosterEntry] = &[
    known("claude", CliFamily::Claude),
    known("codex", CliFamily::Codex),
    RosterEntry {
        name: "agy",
        display_name: Some("Antigravity"),
        family: CliFamily::Antigravity,
    },
    known("aider", CliFamily::Generic),
    known("vibe", CliFamily::Generic),
    known("qwen", CliFamily::Qwen),
    known("amp", CliFamily::Generic),
    known("opencode", CliFamily::Generic),
    known("cline", CliFamily::Generic),
    known("droid", CliFamily::Generic),
    known("pi", CliFamily::Generic),
    known("junie", CliFamily::Generic),
    RosterEntry {
        name: "cursor-agent",
        display_name: Some("Cursor Agent"),
        family: CliFamily::Cursor,
    },
    known("copilot", CliFamily::Generic),
    known("cn", CliFamily::Generic),
    known("kilo", CliFamily::Generic),
    known("kimi", CliFamily::Generic),
];

/// The roster entry for a known CLI name, or `None`.
#[must_use]
pub fn roster_entry(name: &str) -> Option<&'static RosterEntry> {
    ROSTER.iter().find(|entry| entry.name == name)
}

#[cfg(test)]
mod tests;

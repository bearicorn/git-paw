//! Configuration file support.
//!
//! Parses TOML configuration from global (`~/.config/git-paw/config.toml`)
//! and per-repo (`.git-paw/config.toml`) files. Supports custom CLI definitions,
//! presets, and programmatic add/remove of custom CLIs.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::PawError;

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
}

/// A named preset defining branches and a CLI to use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Preset {
    /// Branches to open in this preset.
    pub branches: Vec<String>,
    /// CLI to use for all branches in this preset.
    pub cli: String,
}

/// Governance document paths.
///
/// Each field is a pointer to a user-maintained document or directory that
/// describes some aspect of the project's governance (ADRs, test strategy,
/// security checklist, Definition of Done, project constitution).
///
/// All fields are optional and stored as raw [`PathBuf`] values. Relative
/// paths are resolved against the repository root at *use time* by
/// downstream consumers, not at config-load time. Absolute paths are
/// preserved as-is. No filesystem existence check is performed during
/// config-load — pointing at a path that doesn't exist is a runtime
/// concern, not a parse error.
///
/// This struct is storage-only: nothing in `git_paw::config` reads the
/// referenced documents or enforces any rubric against them. The runtime
/// consumer lives in the parallel `governance-context` capability.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GovernanceConfig {
    /// Directory containing ADR files. Project chooses the convention
    /// (Nygard, MADR, `adr-tools`, custom). git-paw does not dictate one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adr: Option<PathBuf>,
    /// Single Markdown file describing the project's test strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_strategy: Option<PathBuf>,
    /// Single Markdown file containing the project's security checklist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<PathBuf>,
    /// Single Markdown file containing the project's Definition of Done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dod: Option<PathBuf>,
    /// Single Markdown file containing the project's constitution
    /// (`Spec Kit`'s `constitution.md` or any project's equivalent). May
    /// be auto-populated from `.specify/memory/constitution.md` when the
    /// `SpecKit` backend is active and the user has not set this field
    /// explicitly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constitution: Option<PathBuf>,
    /// Path to the repository README (e.g. `README.md`). Bring-your-own
    /// pointer surfaced by the MCP documentation tools; `None` by default,
    /// degrading the `get_readme` tool to a null result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme: Option<PathBuf>,
    /// Path to the documentation root directory (e.g. `docs/src`).
    /// Bring-your-own pointer surfaced by the MCP documentation tools
    /// (`list_docs`/`get_doc`); `None` by default, degrading those tools to
    /// empty results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs: Option<PathBuf>,
}

/// MCP server configuration.
///
/// Carries settings specific to the `git paw mcp` server. Currently a single
/// optional `name` field that overrides the identity the server advertises in
/// the `initialize` handshake's `serverInfo.name`.
///
/// Embedded as a plain (non-`Option`) field on [`PawConfig`] with
/// `#[serde(default)]`, so a config with no `[mcp]` section loads
/// [`McpConfig::default`] (`name: None`) and pre-existing configs round-trip
/// identically.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpConfig {
    /// Per-repo override for the MCP server's advertised identity
    /// (`serverInfo.name`). When `Some`, the server advertises this name in
    /// the `initialize` handshake; when `None` (the default), it advertises
    /// `"git-paw"`. This is independent of the client-side `mcpServers` key the
    /// user controls in their MCP client config — it lets multi-repo setups
    /// distinguish instances by the server's own identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Spec scanning configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpecsConfig {
    /// Directory containing spec files (relative to repo root).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    /// Spec format type: `"openspec"` or `"markdown"`.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub spec_type: Option<String>,
}

/// Enforcement mode for the opsx role-gating guard.
///
/// Governs how the broker reacts when a non-supervisor agent commits an
/// `OpenSpec` archive operation (see the `opsx-role-gating` capability). The
/// serde wire values are the lowercase strings `"warn"`, `"block"`, and
/// `"off"`; an absent `[opsx].role_gating` resolves to [`Self::Warn`].
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RoleGatingMode {
    /// Publish an `agent.feedback` to the offending agent and record an
    /// `agent.learning` with category `permission_pattern`. The default.
    #[default]
    Warn,
    /// Warn behaviour PLUS publish an `agent.feedback` targeted at the
    /// supervisor requesting it revert the offending commit via its
    /// merge-orchestration skill.
    Block,
    /// Disable the guard entirely — no classification, feedback, or learning.
    Off,
}

/// opsx (`OpenSpec`) integration configuration.
///
/// Currently carries the single `role_gating` knob. Embedded as
/// `Option<OpsxConfig>` on [`PawConfig`] so configs without an `[opsx]`
/// section round-trip identically.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpsxConfig {
    /// Enforcement mode for the role-gating guard. `None` (the absent
    /// default) resolves to [`RoleGatingMode::Warn`] via
    /// [`OpsxConfig::role_gating_mode`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_gating: Option<RoleGatingMode>,
}

impl OpsxConfig {
    /// Resolves the effective role-gating mode, defaulting to
    /// [`RoleGatingMode::Warn`] when the field is absent.
    #[must_use]
    pub fn role_gating_mode(&self) -> RoleGatingMode {
        self.role_gating.unwrap_or_default()
    }
}

/// Session logging configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoggingConfig {
    /// Whether session logging is enabled.
    #[serde(default)]
    pub enabled: bool,
}

/// Approval level governing how much autonomy an agent has when operating
/// on the repository.
///
/// The variants are ordered from most conservative to most permissive:
///
/// - `Manual` — the agent must ask the user to approve every file write or
///   shell command. Safest, but slowest.
/// - `Auto` — the agent may perform routine edits without asking, but still
///   defers for destructive or privileged operations. This is the default.
/// - `FullAuto` — the agent is granted full unattended permissions,
///   bypassing per-action approval. Only appropriate for trusted sandboxes.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalLevel {
    /// Prompt the user for every write or command.
    Manual,
    /// Allow routine edits without prompting, defer for destructive ops.
    #[default]
    Auto,
    /// Grant full unattended permissions (skip approvals entirely).
    FullAuto,
}

/// Dashboard configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DashboardConfig {
    /// Whether to show the legacy broker messages panel in the dashboard.
    ///
    /// Superseded by the type-filterable "Broker log" panel
    /// ([`DashboardConfig::broker_log`]); retained for source compatibility
    /// with v0.5.0 configs.
    #[serde(default)]
    pub show_message_log: bool,
    /// Configuration for the v0.6.0 "Broker log" panel — its ring-buffer cap
    /// and default visibility. An absent `[dashboard.broker_log]` section
    /// loads [`BrokerLogConfig::default`] so v0.5.0 configs parse unchanged.
    #[serde(default)]
    pub broker_log: BrokerLogConfig,
}

/// Configuration for the dashboard's "Broker log" panel.
///
/// All fields carry `#[serde(default)]` so a v0.5.0 `[dashboard]` section
/// with no `broker_log` table — or a `[dashboard.broker_log]` table that
/// sets only some fields — loads with the documented defaults for the rest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrokerLogConfig {
    /// Maximum number of messages retained in the panel's in-memory ring
    /// buffer. Older messages drop off the top as new ones arrive. Default:
    /// `500`.
    #[serde(default = "BrokerLogConfig::default_max_messages")]
    pub max_messages: usize,
    /// Whether the panel is visible when the dashboard first launches. The
    /// `l` hotkey toggles visibility at runtime regardless of this value.
    /// Default: `true`.
    #[serde(default = "BrokerLogConfig::default_visible")]
    pub default_visible: bool,
    /// Number of terminal rows the panel occupies when visible. Raised from
    /// the v0.6.0 fixed `12` so more broker messages are visible without
    /// scrolling; the agent table keeps a positive minimum and yields slack
    /// to the panel only on tall terminals. Default: `20`.
    #[serde(default = "BrokerLogConfig::default_height_lines")]
    pub height_lines: u16,
}

impl Default for BrokerLogConfig {
    fn default() -> Self {
        Self {
            max_messages: Self::default_max_messages(),
            default_visible: Self::default_visible(),
            height_lines: Self::default_height_lines(),
        }
    }
}

impl BrokerLogConfig {
    fn default_max_messages() -> usize {
        500
    }

    fn default_visible() -> bool {
        true
    }

    /// Default panel height in terminal rows. Strictly greater than the
    /// v0.6.0 fixed `12` so the panel shows materially more messages.
    fn default_height_lines() -> u16 {
        20
    }
}

/// Supervisor mode configuration.
///
/// Supervisor mode puts git-paw in front of the agent CLI as a coordinating
/// layer that can enforce approval policy and run a verification command
/// after each agent completes a task.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SupervisorConfig {
    /// Whether supervisor mode is enabled by default for this repo.
    #[serde(default)]
    pub enabled: bool,
    /// Override the CLI used when launching the supervisor (e.g. `"claude"`).
    /// `None` resolves to the normal CLI selection flow at runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli: Option<String>,
    /// Test command to run after each agent completes (e.g. `"just check"`).
    /// `None` skips the verification step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_command: Option<String>,
    /// Pre-stage lint invocation for the five-gate verification workflow.
    ///
    /// Drives gate 1's lint sub-step. Example values per common stack:
    /// `"cargo clippy -- -D warnings"` (Rust), `"npm run lint"` (Node),
    /// `"ruff check ."` (Python), `"golangci-lint run"` (Go). When `None`,
    /// the supervisor skill renders the placeholder as `(not configured)`
    /// and the supervisor agent skips the tooling invocation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lint_command: Option<String>,
    /// Compile-step command when build is distinct from test.
    ///
    /// Drives gate 1's compile sub-step. Example values: `"cargo build"`
    /// (Rust), `"npm run build"` (Node), `"mvn package"` (Java), `"go
    /// build ./..."` (Go). When `None`, the supervisor skill renders the
    /// placeholder as `(not configured)` and the supervisor agent skips
    /// the tooling invocation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_command: Option<String>,
    /// Documentation-build command for gate 4 (doc audit).
    ///
    /// Example values: `"mdbook build docs/"` (`mdBook`), `"sphinx-build"`
    /// (Sphinx), `"mkdocs build"` (`MkDocs`), `"npx typedoc"` (`TypeDoc`).
    /// When `None`, the supervisor skill renders the placeholder as
    /// `(not configured)` and the supervisor agent skips the tooling
    /// invocation; the manual doc-surface review still applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_build_command: Option<String>,
    /// API-doc generator command used during spec audit.
    ///
    /// Distinct from [`Self::doc_build_command`] (which builds the
    /// human-readable doc site): this one runs the per-language API-doc
    /// extractor against changed public items. Example values:
    /// `"cargo doc --no-deps"` (Rust), `"sphinx-build -W docs docs/_build"`
    /// (Python/Sphinx), `"npx typedoc"` (TypeScript), `"javadoc"` (Java),
    /// `"go doc"` (Go). When `None`, the supervisor skill renders the
    /// `{{DOC_TOOL_COMMAND}}` placeholder as an empty string and the
    /// surrounding prose is authored to read naturally without it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_tool_command: Option<String>,
    /// Spec-validator command for gate 3 (spec audit).
    ///
    /// Typically takes a change name as argument; the supervisor agent
    /// substitutes `{{CHANGE_ID}}` at verification time using the change
    /// it is currently auditing. Example values: `"openspec validate
    /// {{CHANGE_ID}} --strict"` (`OpenSpec`). When `None`, the supervisor
    /// skill renders the placeholder as `(not configured)` and the
    /// supervisor agent skips the tooling invocation; the manual
    /// scenario-coverage check still applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_validate_command: Option<String>,
    /// Formatter-check command for gate 1's pre-stage.
    ///
    /// Example values: `"cargo fmt --check"` (Rust), `"prettier --check
    /// ."` (Node), `"gofmt -l ."` (Go), `"black --check ."` (Python).
    /// When `None`, the supervisor skill renders the placeholder as
    /// `(not configured)` and the supervisor agent skips the tooling
    /// invocation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fmt_check_command: Option<String>,
    /// Security-audit tooling for gate 5.
    ///
    /// Example values: `"cargo audit"` (Rust), `"npm audit"` (Node),
    /// `"bandit -r ."` (Python), `"gosec ./..."` (Go). When `None`, the
    /// supervisor skill renders the placeholder as `(not configured)`
    /// and the supervisor agent skips the tooling invocation; the manual
    /// OWASP-category diff review still applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security_audit_command: Option<String>,
    /// Approval policy applied to agent actions.
    #[serde(default)]
    pub agent_approval: ApprovalLevel,
    /// Auto-approval configuration for safe permission prompts.
    ///
    /// When present, the supervisor automatically approves stalled agents
    /// whose pending command matches an entry in the safe-command whitelist.
    /// See [`AutoApproveConfig`] for the per-field semantics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_approve: Option<AutoApproveConfig>,
    /// Conflict detector configuration.
    ///
    /// Drives the broker-internal subsystem that auto-emits
    /// `agent.feedback` and `agent.question` for forward, in-flight, and
    /// ownership conflicts between agents. Active only when
    /// [`SupervisorConfig::enabled`] is `true`; otherwise the detector
    /// subsystem is not started and no auto-warnings fire.
    #[serde(default)]
    pub conflict: ConflictConfig,
    /// Opt-in flag for the learnings aggregator subsystem (learnings-mode).
    ///
    /// When `true` (and `[broker] enabled = true`), the broker starts a
    /// learnings aggregator that observes the session and appends
    /// human-readable summaries to `.git-paw/session-learnings.md`. Defaults
    /// to `false` — pre-v0.5 configs load without producing learnings.
    #[serde(default)]
    pub learnings: bool,
    /// Tuning knobs for the learnings aggregator.
    ///
    /// Honoured only when [`Self::learnings`] is `true`. Missing fields fall
    /// back to [`LearningsConfig::default`]. The TOML table key is
    /// `[supervisor.learnings_config]` to avoid colliding with the boolean
    /// `learnings` field.
    #[serde(default)]
    pub learnings_config: LearningsConfig,
    /// Common dev-command allowlist configuration.
    ///
    /// Controls whether the supervisor seeds a curated preset of
    /// dev-loop prefix patterns (`cargo build`, `git commit`, ...) into
    /// `.claude/settings.json::allowed_bash_prefixes` on session start.
    /// See [`CommonDevAllowlistConfig`] for field semantics.
    #[serde(default)]
    pub common_dev_allowlist: CommonDevAllowlistConfig,
    /// Whether the broker emits a `supervisor.verify-now` nudge to the
    /// supervisor inbox when an agent publishes an
    /// `agent.artifact { status: "committed" }`.
    ///
    /// The nudge makes per-commit verification fire on an explicit event
    /// rather than relying on the supervisor's sweep cadence to notice the
    /// commit, so each agent's commit is verified promptly instead of being
    /// batched with a slower agent's. `None` (the field omitted from config)
    /// resolves to `true`; set `verify_on_commit_nudge = false` to suppress
    /// the nudge and fall back to sweep-cadence verification. Resolve the
    /// effective value with [`Self::verify_on_commit_nudge_enabled`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_on_commit_nudge: Option<bool>,
    /// Whether the per-worktree pre-commit branch guard refuses commits that
    /// would advance a branch other than the worktree's assigned branch.
    ///
    /// `None` (the default) resolves to `true` via [`Self::strict_branch_guard`]
    /// — the guard is on unless explicitly disabled. Set
    /// `[supervisor] strict_branch_guard = false` to opt out of *enforcement*
    /// (the post-commit `agent.feedback` detection still fires; detection
    /// without enforcement). Guards against cross-worktree contamination where
    /// a commit advances the wrong branch because linked worktrees share
    /// `.git/refs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict_branch_guard: Option<bool>,
    /// Whether the supervisor reverts an opsx role-gating violation commit
    /// without first confirming with the user.
    ///
    /// Consumed by the supervisor skill's merge-orchestration revert flow: in
    /// `block` mode the guard publishes a revert-request `agent.feedback` to
    /// the supervisor, and the supervisor confirms with the user before
    /// running `git revert` UNLESS this is `true`. `None` (the default)
    /// resolves to `false` via [`Self::auto_revert`] — confirmation is
    /// required by default so a destructive revert never fires unattended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_revert: Option<bool>,
    /// Whether manual (user-decided) approval patterns are recorded to the
    /// per-session log at `.git-paw/sessions/<session>.manual-approvals.jsonl`
    /// and surfaced via `git paw approvals`.
    ///
    /// `None` (the field omitted from config) resolves to `true` via
    /// [`Self::manual_approvals_log_enabled`] — recording is on unless
    /// explicitly disabled. Set `[supervisor] manual_approvals_log = false` to
    /// suppress both the log writes AND the derived `permission_pattern`
    /// learnings emission. The opt-out affects writes only; `git paw approvals`
    /// still reads any pre-existing log. See the `approval-pattern-surfacing`
    /// change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_approvals_log: Option<bool>,
    /// No-progress detection window, in seconds, for the bundled `sweep.sh`
    /// stuck detector.
    ///
    /// An agent is flagged `no-progress` when BOTH its completed-task-checkbox
    /// count AND its branch commit count stay unchanged for at least this many
    /// seconds. Consumed only by `.git-paw/scripts/sweep.sh` (which reads it
    /// from `[supervisor]` config); when the field is absent the helper falls
    /// back to its documented default (~1500s / 25 min), longer than the
    /// stuck-on-prompt heartbeat threshold because real edits take minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_progress_window_seconds: Option<u64>,
    /// Context-bloat token threshold, in thousands of tokens, for the bundled
    /// `sweep.sh` stuck detector.
    ///
    /// When an agent's pane shows a `/clear to save <N>k tokens` hint whose `N`
    /// meets or exceeds this value, the detector proactively flags the agent
    /// `context-bloat` so the supervisor can pre-empt the eventual freeze.
    /// Consumed only by `.git-paw/scripts/sweep.sh`; when absent the helper
    /// falls back to its documented default (~250, matching the observed
    /// v0.8.0 freeze point).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_bloat_threshold_k: Option<u64>,
    /// Blocked-on-supervisor timeout window, in seconds, for the bundled
    /// `sweep.sh` stuck detector.
    ///
    /// An agent whose latest unanswered `agent.blocked` names the supervisor as
    /// the blocker is flagged `blocked-on-supervisor` once it has waited longer
    /// than this window, forcing the supervisor (or the unattended drive loop)
    /// to answer rather than leaving the agent stalled. Consumed only by
    /// `.git-paw/scripts/sweep.sh`; when absent the helper falls back to its
    /// documented default (~900s / 15 min).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_on_supervisor_window_seconds: Option<u64>,
    /// Configuration for the `/tell` user→agent routing command.
    ///
    /// Carries the default delivery mode and the inventory-cache max age. The
    /// TOML table key is `[supervisor.tell]`. An absent table — every v0.5.0
    /// config — loads [`TellConfig::default`] (mode `feedback`, max age 60s)
    /// and round-trips identically because [`TellConfig::is_default`] skips
    /// serialising the all-default table.
    #[serde(default, skip_serializing_if = "TellConfig::is_default")]
    pub tell: TellConfig,
}

/// Delivery mode for the supervisor `/tell` routing command.
///
/// Selects the default channel by which a user-typed prompt reaches the named
/// agent. The serde wire values are the kebab-case strings `"feedback"` and
/// `"send-keys"`; an absent `[supervisor.tell] mode` resolves to
/// [`Self::Feedback`].
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TellMode {
    /// Queue an `agent.feedback` broker message — the agent consumes it on its
    /// next inbox poll. Safe by default: the prompt is recorded, not race-y.
    #[default]
    Feedback,
    /// Inject the prompt directly into the target pane via `tmux send-keys`.
    /// Faster, but only safe for agents in accept-edits mode; `/tell` falls
    /// back to [`Self::Feedback`] when the target's detected mode is not
    /// `accept-edits`.
    SendKeys,
}

/// Configuration for the supervisor `/tell` user→agent routing command.
///
/// Embedded as a plain (non-`Option`) field on [`SupervisorConfig`] with
/// `#[serde(default)]`, so a `[supervisor]` section with no `[supervisor.tell]`
/// table loads the documented defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TellConfig {
    /// Default delivery mode for `/tell`. Default: [`TellMode::Feedback`].
    #[serde(default)]
    pub mode: TellMode,
    /// Maximum age (seconds) of the cached inventory snapshot before
    /// `/tell` / `/agents` rebuild it on demand. Default: `60`.
    #[serde(default = "TellConfig::default_inventory_max_age_seconds")]
    pub inventory_max_age_seconds: u64,
}

impl Default for TellConfig {
    fn default() -> Self {
        Self {
            mode: TellMode::default(),
            inventory_max_age_seconds: Self::default_inventory_max_age_seconds(),
        }
    }
}

impl TellConfig {
    fn default_inventory_max_age_seconds() -> u64 {
        60
    }

    /// Returns `true` when this config equals [`TellConfig::default`].
    ///
    /// Used as the `skip_serializing_if` predicate so an all-default
    /// `[supervisor.tell]` table is omitted on save, keeping v0.5.0 configs
    /// byte-stable round-trips.
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

impl SupervisorConfig {
    /// Resolves whether the pre-commit branch guard enforces (blocks) on a
    /// branch mismatch. Defaults to `true` when the config field is absent.
    #[must_use]
    pub fn strict_branch_guard(&self) -> bool {
        self.strict_branch_guard.unwrap_or(true)
    }

    /// Resolves whether the supervisor reverts an opsx role-gating violation
    /// commit without user confirmation. Defaults to `false` when the config
    /// field is absent — a revert always asks first unless explicitly opted in.
    #[must_use]
    pub fn auto_revert(&self) -> bool {
        self.auto_revert.unwrap_or(false)
    }

    /// Resolves whether manual-approval pattern recording is enabled.
    ///
    /// Returns the configured [`Self::manual_approvals_log`] value, or `true`
    /// when the field is unset — recording is on by default.
    #[must_use]
    pub fn manual_approvals_log_enabled(&self) -> bool {
        self.manual_approvals_log.unwrap_or(true)
    }

    /// Borrowed view of the seven gate-command templates suitable for
    /// passing to [`crate::skills::render`]. Each field maps directly to
    /// the matching `Option<String>` on this struct.
    #[must_use]
    pub fn gate_commands(&self) -> crate::skills::GateCommands<'_> {
        crate::skills::GateCommands {
            test_command: self.test_command.as_deref(),
            lint_command: self.lint_command.as_deref(),
            build_command: self.build_command.as_deref(),
            doc_build_command: self.doc_build_command.as_deref(),
            spec_validate_command: self.spec_validate_command.as_deref(),
            fmt_check_command: self.fmt_check_command.as_deref(),
            security_audit_command: self.security_audit_command.as_deref(),
            doc_tool_command: self.doc_tool_command.as_deref(),
        }
    }

    /// Resolves whether the broker should emit a `supervisor.verify-now`
    /// nudge on each committed artifact.
    ///
    /// Returns the configured [`Self::verify_on_commit_nudge`] value, or
    /// `true` when the field is unset — per-commit verification nudging is on
    /// by default.
    #[must_use]
    pub fn verify_on_commit_nudge_enabled(&self) -> bool {
        self.verify_on_commit_nudge.unwrap_or(true)
    }
}

/// Configuration for the common dev-command allowlist preset.
///
/// The universal preset is a curated set of stack-neutral, repeatedly-
/// prompted dev-loop commands (non-destructive git verbs plus read-only
/// `find` / `grep` / `sed -n`) that the supervisor seeds into Claude's
/// `allowed_bash_prefixes` so agents do not hit a permission prompt for
/// each variant of these commands. Stack-specific grants are opt-in via
/// `stacks` (named presets `rust` / `node` / `python` / `go`) and/or
/// the free-form `extra` list. See `src/supervisor/dev_allowlist.rs`
/// for the preset constants and the merge implementation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommonDevAllowlistConfig {
    /// Whether the dev-allowlist seeder runs on supervisor start.
    ///
    /// Defaults to `true` — the v0.5.0 dogfood evidence makes the
    /// feature most useful when on by default. Opt out with
    /// `[supervisor.common_dev_allowlist] enabled = false`.
    #[serde(default = "CommonDevAllowlistConfig::default_enabled")]
    pub enabled: bool,
    /// Named, curated stack presets the repository opts into.
    ///
    /// Each entry names a built-in stack preset (`rust` / `node` /
    /// `python` / `go`) whose curated prefix bundle is seeded in
    /// addition to the universal preset. Unknown names contribute
    /// nothing. Defaults to empty — a fresh repo seeds only the
    /// universal preset, never a toolchain it does not use. See
    /// `src/supervisor/dev_allowlist.rs::stack_preset`.
    #[serde(default)]
    pub stacks: Vec<String>,
    /// Additional project-specific prefix patterns appended to the
    /// built-in preset (and to any selected stack presets).
    ///
    /// Each entry is a raw string consumed by Claude's prefix matcher;
    /// the seeder does not validate the strings. Duplicates of preset
    /// or stack entries are silently de-duplicated.
    #[serde(default)]
    pub extra: Vec<String>,
}

impl Default for CommonDevAllowlistConfig {
    fn default() -> Self {
        Self {
            enabled: Self::default_enabled(),
            stacks: Vec::new(),
            extra: Vec::new(),
        }
    }
}

impl CommonDevAllowlistConfig {
    fn default_enabled() -> bool {
        true
    }
}

/// Tuning knobs for the learnings aggregator.
///
/// The aggregator periodically flushes accumulated learnings to
/// `.git-paw/session-learnings.md` plus one final flush at broker shutdown.
/// `flush_interval_seconds` controls the periodic cadence; bursts of activity
/// may flush sooner if the in-memory queue grows past the soft cap.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LearningsConfig {
    /// Interval between periodic flushes to disk. Default: `60`.
    #[serde(default = "LearningsConfig::default_flush_interval_seconds")]
    pub flush_interval_seconds: u64,
    /// Whether flushed learnings are also published to the broker as
    /// `agent.learning` messages (in addition to the markdown file).
    ///
    /// Default [`BrokerPublish::Auto`] follows `[broker] enabled`: publish
    /// when the broker is running, file-only when it is not. Set to
    /// [`BrokerPublish::ForceOff`] to keep file-only output even with an
    /// active broker. See the `agent-learning-variant` change.
    #[serde(default)]
    pub broker_publish: BrokerPublish,
}

impl Default for LearningsConfig {
    fn default() -> Self {
        Self {
            flush_interval_seconds: Self::default_flush_interval_seconds(),
            broker_publish: BrokerPublish::default(),
        }
    }
}

impl LearningsConfig {
    fn default_flush_interval_seconds() -> u64 {
        60
    }
}

/// Whether the learnings aggregator publishes flushed records to the broker.
///
/// The markdown file output (`.git-paw/session-learnings.md`) is unconditional
/// — this knob only governs the additional `agent.learning` broker publish.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BrokerPublish {
    /// Follow `[broker] enabled`: publish to the broker when it is running,
    /// file-only when it is not. This is the default.
    #[default]
    Auto,
    /// Never publish to the broker, even when it is running (file-only).
    ForceOff,
}

impl BrokerPublish {
    /// Resolves the effective publish decision against whether the broker is
    /// enabled for this session.
    #[must_use]
    pub fn resolve(self, broker_enabled: bool) -> bool {
        match self {
            Self::Auto => broker_enabled,
            Self::ForceOff => false,
        }
    }
}

/// Configuration for the broker-internal conflict detector.
///
/// The detector observes `agent.intent` and `agent.status` events as they
/// pass through the publish pipeline and emits `agent.feedback` /
/// `agent.question` when one of three failure shapes triggers (forward,
/// in-flight, ownership). All fields have defaults; an entirely absent
/// `[supervisor.conflict]` section loads [`ConflictConfig::default`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConflictConfig {
    /// Window after which an unresolved in-flight conflict escalates to
    /// the supervisor inbox via `agent.question`. Default: `120`.
    #[serde(default = "ConflictConfig::default_window_seconds")]
    pub window_seconds: u64,
    /// Master switch for forward-conflict warnings. When `false`, no
    /// `agent.feedback` is emitted for overlapping `agent.intent`
    /// declarations, but the tracker SHALL still record intents (so
    /// in-flight and ownership detection remain functional). Default:
    /// `true`.
    #[serde(default = "ConflictConfig::default_true")]
    pub warn_on_intent_overlap: bool,
    /// Whether ownership violations escalate to the supervisor inbox via
    /// `agent.question`. The violator-bound `agent.feedback` always fires
    /// regardless of this flag — only the supervisor follow-up is gated.
    /// Default: `true`.
    #[serde(default = "ConflictConfig::default_true")]
    pub escalate_on_violation: bool,
}

impl Default for ConflictConfig {
    fn default() -> Self {
        Self {
            window_seconds: Self::default_window_seconds(),
            warn_on_intent_overlap: true,
            escalate_on_violation: true,
        }
    }
}

impl ConflictConfig {
    fn default_window_seconds() -> u64 {
        120
    }

    fn default_true() -> bool {
        true
    }
}

/// Coarse-grained policy preset that maps onto a known [`AutoApproveConfig`]
/// shape.
///
/// The presets exist so users do not have to hand-craft a whitelist when
/// they just want a sensible default for the project. The mapping is:
///
/// - `Off` — auto-approval is disabled regardless of other fields.
/// - `Conservative` — auto-approve `cargo`/`git commit` style commands but
///   strip `git push` and `curl` from the effective whitelist.
/// - `Safe` — the built-in default; auto-approve everything in
///   [`default_safe_commands()`](crate::supervisor::auto_approve::default_safe_commands).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalLevelPreset {
    /// Disable auto-approval entirely.
    Off,
    /// Approve only the most uncontroversial commands (no push/curl).
    Conservative,
    /// Approve every entry in the built-in safe-command list.
    #[default]
    Safe,
}

/// Configuration for the supervisor auto-approval feature.
///
/// Auto-approval detects permission prompts in stalled agent panes via
/// `tmux capture-pane`, classifies the pending command, and dispatches the
/// `BTab Down Enter` keystroke sequence when the command matches the
/// whitelist.
///
/// Embedded as `Option<AutoApproveConfig>` on [`SupervisorConfig`] so
/// existing configs without an `[supervisor.auto_approve]` table continue
/// to round-trip identically.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutoApproveConfig {
    /// Master enable flag. When `false`, no detection or approval runs.
    #[serde(default = "AutoApproveConfig::default_enabled")]
    pub enabled: bool,
    /// Project-specific safe-command prefixes appended to the built-in
    /// defaults from
    /// [`default_safe_commands()`](crate::supervisor::auto_approve::default_safe_commands).
    #[serde(default)]
    pub safe_commands: Vec<String>,
    /// Threshold (in seconds) of `last_seen` staleness before an agent in
    /// `working` status is treated as stalled by the poll loop.
    #[serde(default = "AutoApproveConfig::default_stall_threshold_seconds")]
    pub stall_threshold_seconds: u64,
    /// Coarse policy preset applied on top of the explicit fields.
    ///
    /// When the preset is `Off`, [`Self::enabled`] is forced to `false` by
    /// [`Self::resolved`]. When the preset is `Conservative`, the effective
    /// whitelist is the built-in defaults minus `git push` and `curl`
    /// entries.
    #[serde(default)]
    pub approval_level: ApprovalLevelPreset,
    /// Whether filesystem write / edit / create prompts whose target path
    /// resolves *inside* the agent's own worktree are auto-approved.
    ///
    /// `None` (the absent default) resolves to `true` via
    /// [`Self::approve_worktree_writes`] — worktrees are isolated, so
    /// confining auto-approval to the worktree boundary is safe by
    /// construction. Set to `false` to revert to the manual-prompt flow for
    /// all file operations. Out-of-worktree paths always require manual
    /// approval regardless of this flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approve_worktree_writes: Option<bool>,
}

impl Default for AutoApproveConfig {
    fn default() -> Self {
        Self {
            enabled: Self::default_enabled(),
            safe_commands: Vec::new(),
            stall_threshold_seconds: Self::default_stall_threshold_seconds(),
            approval_level: ApprovalLevelPreset::Safe,
            approve_worktree_writes: None,
        }
    }
}

impl AutoApproveConfig {
    /// Minimum stall threshold in seconds. Anything lower is clamped to
    /// avoid pathological poll loops.
    pub const MIN_STALL_THRESHOLD_SECONDS: u64 = 5;

    fn default_enabled() -> bool {
        true
    }

    fn default_stall_threshold_seconds() -> u64 {
        30
    }

    /// Returns a copy of this config with preset rules applied and the
    /// stall threshold floor enforced.
    ///
    /// - When `approval_level == Off`, `enabled` is forced to `false`.
    /// - When `stall_threshold_seconds < MIN_STALL_THRESHOLD_SECONDS`, the
    ///   value is clamped and a warning is written to stderr.
    #[must_use]
    pub fn resolved(&self) -> Self {
        let mut out = self.clone();
        if out.approval_level == ApprovalLevelPreset::Off {
            out.enabled = false;
        }
        if out.stall_threshold_seconds < Self::MIN_STALL_THRESHOLD_SECONDS {
            eprintln!(
                "warning: [supervisor.auto_approve] stall_threshold_seconds = {} clamped to {}s minimum",
                out.stall_threshold_seconds,
                Self::MIN_STALL_THRESHOLD_SECONDS
            );
            out.stall_threshold_seconds = Self::MIN_STALL_THRESHOLD_SECONDS;
        }
        out
    }

    /// Returns whether worktree-confined file operations are auto-approved.
    ///
    /// Resolves the optional [`Self::approve_worktree_writes`] field to its
    /// effective boolean: an absent value (the common case — no
    /// `[supervisor.auto_approve]` section, or the field omitted) defaults to
    /// `true`.
    #[must_use]
    pub fn approve_worktree_writes(&self) -> bool {
        self.approve_worktree_writes.unwrap_or(true)
    }

    /// Returns the effective whitelist for this config, applying the preset
    /// to the union of built-in defaults and user-configured `safe_commands`.
    ///
    /// - `Off` and `Safe` both return defaults plus configured extras.
    /// - `Conservative` returns the same union with `git push` and any
    ///   `curl` entries filtered out.
    #[must_use]
    pub fn effective_whitelist(&self) -> Vec<String> {
        let mut out: Vec<String> = crate::supervisor::auto_approve::default_safe_commands()
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        for extra in &self.safe_commands {
            if !out.iter().any(|e| e == extra) {
                out.push(extra.clone());
            }
        }
        if self.approval_level == ApprovalLevelPreset::Conservative {
            out.retain(|cmd| !cmd.starts_with("git push") && !cmd.starts_with("curl"));
        }
        out
    }
}

/// Returns the CLI-specific permission flag for `cli` at the given approval
/// `level`, or an empty string if the combination has no mapped flag.
///
/// # Examples
///
/// ```
/// use git_paw::config::{approval_flags, ApprovalLevel};
///
/// assert_eq!(
///     approval_flags("claude", &ApprovalLevel::FullAuto),
///     "--dangerously-skip-permissions",
/// );
/// assert_eq!(
///     approval_flags("codex", &ApprovalLevel::Auto),
///     "--approval-mode=auto-edit",
/// );
/// assert_eq!(approval_flags("claude", &ApprovalLevel::Manual), "");
/// assert_eq!(approval_flags("some-agent", &ApprovalLevel::FullAuto), "");
/// ```
#[must_use]
pub fn approval_flags(cli: &str, level: &ApprovalLevel) -> &'static str {
    match (cli, level) {
        ("claude", ApprovalLevel::FullAuto) => "--dangerously-skip-permissions",
        ("codex", ApprovalLevel::FullAuto) => "--approval-mode=full-auto",
        ("codex", ApprovalLevel::Auto) => "--approval-mode=auto-edit",
        _ => "",
    }
}

/// Configuration for the broker filesystem watcher.
///
/// The watcher publishes `agent.status: working` from git-status changes.
/// Bug 8 (`auto-approve-scope-v0-6-x`) adds a post-commit re-entry: after an
/// `agent.artifact status: "committed"` event, a subsequent file modification
/// observed within [`Self::republish_working_ttl_seconds`] re-publishes
/// `working` so the dashboard reflects the agent's continued activity.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WatcherConfig {
    /// TTL (seconds) after a `committed` event during which a file write
    /// re-publishes `working`.
    ///
    /// `None` resolves to [`Self::DEFAULT_REPUBLISH_TTL_SECONDS`] (60) via
    /// [`Self::republish_working_ttl_seconds`]. A value of `0` disables the
    /// auto-republish entirely (restoring the v0.5.0 "committed is terminal
    /// until explicit republish" model). Non-zero values below
    /// [`Self::MIN_REPUBLISH_TTL_SECONDS`] (5) are clamped to that floor with
    /// a stderr warning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub republish_working_ttl_seconds: Option<u64>,
}

impl WatcherConfig {
    /// Default post-commit re-entry TTL in seconds.
    pub const DEFAULT_REPUBLISH_TTL_SECONDS: u64 = 60;
    /// Minimum non-zero TTL; smaller positive values clamp up to this floor.
    pub const MIN_REPUBLISH_TTL_SECONDS: u64 = 5;

    /// Returns the effective post-commit re-entry TTL in seconds.
    ///
    /// - `None` → [`Self::DEFAULT_REPUBLISH_TTL_SECONDS`].
    /// - `Some(0)` → `0` (auto-republish disabled).
    /// - `Some(n)` with `0 < n < 5` → clamped to
    ///   [`Self::MIN_REPUBLISH_TTL_SECONDS`] with a stderr warning.
    /// - `Some(n)` with `n >= 5` → `n`.
    #[must_use]
    pub fn republish_working_ttl_seconds(&self) -> u64 {
        match self.republish_working_ttl_seconds {
            None => Self::DEFAULT_REPUBLISH_TTL_SECONDS,
            Some(0) => 0,
            Some(n) if n < Self::MIN_REPUBLISH_TTL_SECONDS => {
                eprintln!(
                    "warning: [broker.watcher] republish_working_ttl_seconds = {n} clamped to {}s minimum",
                    Self::MIN_REPUBLISH_TTL_SECONDS
                );
                Self::MIN_REPUBLISH_TTL_SECONDS
            }
            Some(n) => n,
        }
    }
}

/// HTTP broker configuration for agent coordination.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrokerConfig {
    /// Whether the broker is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// TCP port the broker listens on.
    #[serde(default = "BrokerConfig::default_port")]
    pub port: u16,
    /// Bind address for the broker.
    #[serde(default = "BrokerConfig::default_bind")]
    pub bind: String,
    /// Filesystem watcher tuning.
    #[serde(default)]
    pub watcher: WatcherConfig,
}

impl Default for BrokerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 9119,
            bind: "127.0.0.1".to_string(),
            watcher: WatcherConfig::default(),
        }
    }
}

impl BrokerConfig {
    /// Returns the full URL for the broker endpoint.
    pub fn url(&self) -> String {
        format!("http://{}:{}", self.bind, self.port)
    }

    fn default_port() -> u16 {
        9119
    }

    fn default_bind() -> String {
        "127.0.0.1".to_string()
    }
}

/// Layout configuration for git-paw-managed tmux sessions.
///
/// Controls the optional pane "affordances" — heavy borders, per-pane title
/// labels, and active-pane highlighting — applied to `paw-*` sessions.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LayoutConfig {
    /// Whether to apply the border affordances (heavy borders, dim/active
    /// border styling, per-pane label strip, and per-pane titles) to
    /// git-paw-managed sessions.
    ///
    /// `None` (the default, including when the `[layout]` section is absent)
    /// resolves to `true` via [`LayoutConfig::border_affordances_enabled`].
    /// Set to `false` to opt out and inherit the user's default tmux styling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_affordances: Option<bool>,
}

impl LayoutConfig {
    /// Resolve the border-affordances setting, defaulting to `true` when unset.
    #[must_use]
    pub fn border_affordances_enabled(&self) -> bool {
        self.border_affordances.unwrap_or(true)
    }
}

/// Placement of agent worktrees relative to the repository.
///
/// Selects where [`crate::git::create_worktree`] creates a worktree:
///
/// - `Sibling` — the v0.7.0 layout: `<repo_parent>/<project>-<branch-slug>`,
///   beside the repository in its parent directory. This is the
///   default-on-absent value so pre-existing configs (and sessions created
///   before this field existed) behave identically to v0.7.0.
/// - `Child` — the contained layout: `<repo_root>/.git-paw/worktrees/<branch-slug>`,
///   inside the repository. New repos opt into this via `git paw init`,
///   enabling a project-scoped permission model (one grant for
///   `.git-paw/worktrees/` instead of scattered sibling directories).
///
/// The serde wire values are the lowercase strings `"child"` and `"sibling"`.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WorktreePlacement {
    /// Create worktrees beside the repository at
    /// `<repo_parent>/<project>-<branch-slug>` (the v0.7.0 layout). The
    /// default when `worktree_placement` is absent.
    #[default]
    Sibling,
    /// Create worktrees inside the repository at
    /// `<repo_root>/.git-paw/worktrees/<branch-slug>`.
    Child,
}

/// Top-level git-paw configuration.
///
/// All fields are optional — absent config files produce empty defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PawConfig {
    /// Default CLI to use when none is specified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_cli: Option<String>,

    /// Default CLI for `--from-specs` (bypasses picker when set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_spec_cli: Option<String>,

    /// Prefix for spec-derived branch names (default: `"spec/"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_prefix: Option<String>,

    /// Whether to enable tmux mouse mode for sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mouse: Option<bool>,

    /// Custom CLI definitions keyed by name.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub clis: HashMap<String, CustomCli>,

    /// Named presets keyed by name.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub presets: HashMap<String, Preset>,

    /// Spec scanning configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specs: Option<SpecsConfig>,

    /// Session logging configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingConfig>,

    /// Dashboard configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dashboard: Option<DashboardConfig>,

    /// HTTP broker configuration.
    #[serde(default)]
    pub broker: BrokerConfig,

    /// Supervisor mode configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supervisor: Option<SupervisorConfig>,

    /// Governance document path pointers.
    ///
    /// All sub-fields are optional. Absence is equivalent to an empty
    /// `[governance]` section; v0.4 configs (no `[governance]` at all) load
    /// with `GovernanceConfig::default()` here.
    #[serde(default)]
    pub governance: GovernanceConfig,

    /// Layout configuration for git-paw-managed tmux sessions.
    ///
    /// Absent `[layout]` (v0.5.0 and earlier configs) loads as `None`, which
    /// [`PawConfig::border_affordances_enabled`] resolves to the default
    /// (affordances on).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutConfig>,

    /// opsx (`OpenSpec`) integration configuration.
    ///
    /// Absent `[opsx]` (v0.5.0 and earlier configs) loads as `None`, which
    /// [`PawConfig::role_gating_mode`] resolves to the default
    /// ([`RoleGatingMode::Warn`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opsx: Option<OpsxConfig>,

    /// MCP server configuration.
    ///
    /// Absent `[mcp]` (v0.6.0 and earlier configs) loads as
    /// [`McpConfig::default`] (`name: None`), so the MCP server advertises the
    /// default `"git-paw"` identity and pre-existing configs round-trip
    /// unchanged.
    #[serde(default)]
    pub mcp: McpConfig,

    /// Placement of agent worktrees relative to the repository
    /// (`"child"` or `"sibling"`).
    ///
    /// Absent (every v0.7.0 and earlier config) resolves to
    /// [`WorktreePlacement::Sibling`] via [`PawConfig::worktree_placement`],
    /// preserving the v0.7.0 sibling layout exactly. `git paw init` writes
    /// `"child"` for new repos. Serialised with `skip_serializing_if` so a
    /// default value never appears in round-tripped configs, keeping
    /// pre-existing configs byte-stable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_placement: Option<WorktreePlacement>,
}

impl PawConfig {
    /// Returns a new config that merges `overlay` on top of `self`.
    ///
    /// Scalar fields from `overlay` take precedence when present.
    /// Map fields are merged with `overlay` entries winning on key collisions.
    #[must_use]
    pub fn merged_with(&self, overlay: &Self) -> Self {
        let mut clis = self.clis.clone();
        for (k, v) in &overlay.clis {
            clis.insert(k.clone(), v.clone());
        }

        let mut presets = self.presets.clone();
        for (k, v) in &overlay.presets {
            presets.insert(k.clone(), v.clone());
        }

        Self {
            default_cli: overlay
                .default_cli
                .clone()
                .or_else(|| self.default_cli.clone()),
            default_spec_cli: overlay
                .default_spec_cli
                .clone()
                .or_else(|| self.default_spec_cli.clone()),
            branch_prefix: overlay
                .branch_prefix
                .clone()
                .or_else(|| self.branch_prefix.clone()),
            mouse: overlay.mouse.or(self.mouse),
            clis,
            presets,
            specs: overlay.specs.clone().or_else(|| self.specs.clone()),
            logging: overlay.logging.clone().or_else(|| self.logging.clone()),
            dashboard: overlay.dashboard.clone().or_else(|| self.dashboard.clone()),
            broker: if overlay.broker == BrokerConfig::default() {
                self.broker.clone()
            } else {
                overlay.broker.clone()
            },
            supervisor: overlay
                .supervisor
                .clone()
                .or_else(|| self.supervisor.clone()),
            governance: GovernanceConfig {
                adr: overlay
                    .governance
                    .adr
                    .clone()
                    .or_else(|| self.governance.adr.clone()),
                test_strategy: overlay
                    .governance
                    .test_strategy
                    .clone()
                    .or_else(|| self.governance.test_strategy.clone()),
                security: overlay
                    .governance
                    .security
                    .clone()
                    .or_else(|| self.governance.security.clone()),
                dod: overlay
                    .governance
                    .dod
                    .clone()
                    .or_else(|| self.governance.dod.clone()),
                constitution: overlay
                    .governance
                    .constitution
                    .clone()
                    .or_else(|| self.governance.constitution.clone()),
                readme: overlay
                    .governance
                    .readme
                    .clone()
                    .or_else(|| self.governance.readme.clone()),
                docs: overlay
                    .governance
                    .docs
                    .clone()
                    .or_else(|| self.governance.docs.clone()),
            },
            layout: overlay.layout.clone().or_else(|| self.layout.clone()),
            opsx: overlay.opsx.clone().or_else(|| self.opsx.clone()),
            mcp: McpConfig {
                name: overlay.mcp.name.clone().or_else(|| self.mcp.name.clone()),
            },
            worktree_placement: overlay.worktree_placement.or(self.worktree_placement),
        }
    }

    /// Resolves the effective worktree placement for this config, defaulting
    /// to [`WorktreePlacement::Sibling`] when `worktree_placement` is absent.
    #[must_use]
    pub fn worktree_placement(&self) -> WorktreePlacement {
        self.worktree_placement.unwrap_or_default()
    }

    /// Resolves the effective opsx role-gating mode for this config,
    /// defaulting to [`RoleGatingMode::Warn`] when `[opsx]` or its
    /// `role_gating` field is absent.
    #[must_use]
    pub fn role_gating_mode(&self) -> RoleGatingMode {
        self.opsx
            .as_ref()
            .map(OpsxConfig::role_gating_mode)
            .unwrap_or_default()
    }

    /// Resolve whether the border affordances should be applied, defaulting to
    /// `true` when the `[layout]` section or its `border_affordances` field is
    /// absent.
    #[must_use]
    pub fn border_affordances_enabled(&self) -> bool {
        self.layout
            .as_ref()
            .is_none_or(LayoutConfig::border_affordances_enabled)
    }

    /// Resolves the effective MCP server identity advertised in the
    /// `initialize` handshake's `serverInfo.name`.
    ///
    /// Returns the configured `[mcp].name` when set, otherwise the default
    /// `"git-paw"`.
    #[must_use]
    pub fn mcp_server_name(&self) -> String {
        self.mcp
            .name
            .clone()
            .unwrap_or_else(|| "git-paw".to_string())
    }

    /// Returns a preset by name, if it exists.
    pub fn get_preset(&self, name: &str) -> Option<&Preset> {
        self.presets.get(name)
    }

    /// Returns the dashboard configuration, if it exists.
    pub fn get_dashboard(&self) -> Option<&DashboardConfig> {
        self.dashboard.as_ref()
    }
}

/// Returns the path to the global config file (`~/.config/git-paw/config.toml`).
pub fn global_config_path() -> Result<PathBuf, PawError> {
    crate::dirs::config_dir()
        .map(|d| d.join("git-paw").join("config.toml"))
        .ok_or_else(|| PawError::ConfigError("could not determine config directory".into()))
}

/// Returns the path to a repo-level config file (`.git-paw/config.toml`).
pub fn repo_config_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".git-paw").join("config.toml")
}

/// Loads a [`PawConfig`] from a TOML file, returning `Ok(None)` if the file does not exist.
fn load_config_file(path: &Path) -> Result<Option<PawConfig>, PawError> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let config: PawConfig = toml::from_str(&contents)
                .map_err(|e| PawError::ConfigError(format!("{}: {e}", path.display())))?;
            Ok(Some(config))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(PawError::ConfigError(format!("{}: {e}", path.display()))),
    }
}

/// Loads only the repo-level configuration (`.git-paw/config.toml`).
///
/// Returns defaults if the file does not exist. Useful when you need to
/// update and save repo-level settings without clobbering global values.
///
/// Applies post-deserialise auto-wiring for governance documents (see
/// [`auto_wire_governance`]).
pub fn load_repo_config(repo_root: &Path) -> Result<PawConfig, PawError> {
    let mut config = load_config_file(&repo_config_path(repo_root))?.unwrap_or_default();
    auto_wire_governance(&mut config, repo_root);
    Ok(config)
}

/// Populates `config.governance.constitution` from
/// `git_paw::specs::speckit::detect_constitution` when:
///
/// 1. The user has not set `governance.constitution` explicitly
///    (i.e. it is `None` after TOML deserialisation), AND
/// 2. A `[specs]` section is present, AND
/// 3. `specs.type == "speckit"`.
///
/// Explicit user values always win — even if the explicit value points
/// at a path that does not exist. The check is `is_some()`, not
/// `is_some_and(|p| p.exists())`, so an empty-string or invalid path
/// still suppresses auto-wiring. This lets users disable the auto-wiring
/// without deleting the constitution slot.
///
/// This function is intentionally a no-op when the `SpecKit` backend
/// is not active. It is also a no-op when the configured `specs.dir`'s
/// parent does not contain `memory/constitution.md`.
fn auto_wire_governance(config: &mut PawConfig, repo_root: &Path) {
    if config.governance.constitution.is_some() {
        return;
    }
    let Some(specs_cfg) = config.specs.as_ref() else {
        return;
    };
    let Some(spec_type) = specs_cfg.spec_type.as_deref() else {
        return;
    };
    if spec_type != "speckit" {
        return;
    }
    let dir = specs_cfg.dir.as_deref().unwrap_or("specs");
    let specs_dir = repo_root.join(dir);
    if let Some(detected) = crate::specs::speckit::detect_constitution(&specs_dir) {
        config.governance.constitution = Some(detected);
    }
}

/// Loads the merged configuration for a repository.
///
/// Reads the user-level (global) config and the per-repo config, merging
/// them with repo settings taking precedence. Returns defaults if neither
/// file exists.
///
/// # Parameters
///
/// - `repo_root` — the repository root whose `.git-paw/config.toml` is the
///   repo-level config.
/// - `user_config_path` — controls which file is read as the user-level
///   (global) config:
///   - `None` resolves the user-level path via [`global_config_path`]
///     (platform default: `crate::dirs::config_dir().join("git-paw/config.toml")`).
///     This preserves v0.4 production behaviour and is what every internal
///     caller passes.
///   - `Some(p)` pins the user-level read to `p`. If `p` does not exist on
///     disk, the user-level side of the merge is the default `PawConfig`,
///     exactly as if no file existed at the platform-default path. This is
///     the discoverable test-isolation hook — pass an unused `TempDir`-rooted
///     path so the dev machine's real user-level config cannot leak into
///     the merged result.
///
/// See [`load_config_from`] for the lower-level primitive that takes both
/// paths explicitly (without the `Option` ergonomics).
pub fn load_config(
    repo_root: &Path,
    user_config_path: Option<&Path>,
) -> Result<PawConfig, PawError> {
    let global_path = match user_config_path {
        Some(p) => p.to_path_buf(),
        None => global_config_path()?,
    };
    load_config_from(&global_path, repo_root)
}

/// Loads merged config from an explicit global path and repo root.
///
/// Applies post-merge auto-wiring for governance documents (see
/// [`auto_wire_governance`]).
pub fn load_config_from(global_path: &Path, repo_root: &Path) -> Result<PawConfig, PawError> {
    let global = load_config_file(global_path)?.unwrap_or_default();
    let repo = load_config_file(&repo_config_path(repo_root))?.unwrap_or_default();
    let mut merged = global.merged_with(&repo);
    auto_wire_governance(&mut merged, repo_root);
    Ok(merged)
}

/// Saves a [`PawConfig`] to the repo-level config file (`.git-paw/config.toml`).
pub fn save_repo_config(repo_root: &Path, config: &PawConfig) -> Result<(), PawError> {
    save_config_to(&repo_config_path(repo_root), config)
}

/// Writes a [`PawConfig`] to a TOML file atomically (temp file + rename).
fn save_config_to(path: &Path, config: &PawConfig) -> Result<(), PawError> {
    let dir = path
        .parent()
        .ok_or_else(|| PawError::ConfigError("invalid config path".into()))?;
    fs::create_dir_all(dir)
        .map_err(|e| PawError::ConfigError(format!("create config dir: {e}")))?;

    let contents =
        toml::to_string_pretty(config).map_err(|e| PawError::ConfigError(e.to_string()))?;

    // Atomic write: temp file + rename
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, &contents)
        .map_err(|e| PawError::ConfigError(format!("write temp config: {e}")))?;
    fs::rename(&tmp, path).map_err(|e| PawError::ConfigError(format!("rename config: {e}")))?;

    Ok(())
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
        },
    );

    save_config_to(config_path, &config)
}

/// Returns a default `config.toml` string with sensible defaults and
/// commented-out v0.2.0 fields for discoverability.
#[allow(clippy::too_many_lines)] // single big string literal of example config
pub fn generate_default_config() -> String {
    r#"# git-paw configuration
# See https://github.com/bearicorn/git-paw for documentation.

# Pre-select a CLI in the interactive picker (user can still change).
# Omit to show the full picker with no default.
# default_cli = ""

# Enable tmux mouse mode for sessions (default: true).
# mouse = true

# Bypass the CLI picker entirely for --from-specs mode.
# Omit to prompt or use per-spec paw_cli fields.
# default_spec_cli = ""

# Prefix for spec-derived branch names (default: "spec/" ).
# branch_prefix = "spec/"

# Where agent worktrees are created relative to the repository.
#   "child"   — inside the repo at .git-paw/worktrees/<branch-slug> (contained
#               layout; enables a project-scoped permission grant). New repos
#               default to this. Requires .git-paw/worktrees/ in .gitignore
#               (git paw init seeds it).
#   "sibling" — beside the repo at ../<project>-<branch-slug> (v0.7.0 layout).
# Omit the field to default to "sibling".
worktree_placement = "child"

# Dashboard message log configuration.
# [dashboard]
# show_message_log = false

# Spec scanning configuration.
# [specs]
# dir = "specs"
#
# OpenSpec format (directory-based, default):
# type = "openspec"
#
# Markdown format (frontmatter-based):
# type = "markdown"
# Each .md file uses YAML frontmatter fields:
#   paw_status  — "pending" | "done" | "in-progress" (required)
#   paw_branch  — branch name suffix (optional, falls back to filename)
#   paw_cli     — CLI override for this spec (optional)

# Session logging configuration.
# [logging]
# enabled = false

# HTTP broker for agent coordination (requires --broker flag on start).
# [broker]
# enabled = true
# port = 9119
# bind = "127.0.0.1"

# Supervisor mode — git-paw acts as a coordinating layer in front of the
# agent CLI, enforcing approval policy and running configured gate
# commands during the five-gate verification workflow.
#
# Gate command templates feed the supervisor skill's five gates: gate 1
# Testing (fmt_check / lint / build / test), gate 3 Spec audit
# (spec_validate), gate 4 Doc audit (doc_build), gate 5 Security audit
# (security_audit). When a key is omitted, the matching placeholder
# renders as `(not configured)` in the supervisor skill and the agent
# skips that tooling step (the gate's manual review still applies).
# `{{CHANGE_ID}}` inside spec_validate_command is substituted by the
# supervisor agent at verification time with the change name.
# [supervisor]
# enabled = true
# cli = "claude"
# test_command = "just check"                                  # or: "cargo test", "npm test", "pytest"
# lint_command = "cargo clippy -- -D warnings"                 # or: "npm run lint", "ruff check .", "golangci-lint run"
# build_command = "cargo build"                                # or: "npm run build", "mvn package", "go build ./..."
# fmt_check_command = "cargo fmt --check"                      # or: "prettier --check .", "gofmt -l ."
# doc_build_command = "mdbook build docs/"                     # or: "sphinx-build", "mkdocs build"
# doc_tool_command = "cargo doc --no-deps"                     # or: "sphinx-build -W docs docs/_build", "javadoc", "npx typedoc"
# spec_validate_command = "openspec validate {{CHANGE_ID}} --strict"  # OpenSpec only
# security_audit_command = "cargo audit"                       # or: "npm audit", "bandit -r ."
# agent_approval = "auto"  # one of: "manual", "auto", "full-auto"
# verify_on_commit_nudge = true  # broker nudges the supervisor to verify each commit promptly (default true)
#
# Stuck/bloat detection thresholds, read by .git-paw/scripts/sweep.sh. Each is
# optional; omit to use the documented default shown.
# no_progress_window_seconds = 1500           # flag no-progress after ~25 min with no checkbox/commit movement
# context_bloat_threshold_k = 250             # flag context-bloat when the CLI hints at clearing >= this many k tokens
# blocked_on_supervisor_window_seconds = 900  # flag a supervisor-targeted block unanswered past ~15 min
#
# Routing through the supervisor (the /tell and /agents commands). The user
# types in the supervisor pane and the supervisor routes the prompt to the
# named agent. `mode` selects the default delivery channel:
#   "feedback"  (default) — queue an agent.feedback; the agent picks it up on
#                           its next inbox poll. Safe for mixed-mode sessions.
#   "send-keys"           — inject the prompt directly into the target pane;
#                           used only when the target is in accept-edits mode,
#                           otherwise /tell falls back to feedback.
# `inventory_max_age_seconds` is how stale the cached /agents inventory may be
# before /tell or /agents re-polls the broker (default 60).
# [supervisor.tell]
# mode = "feedback"
# inventory_max_age_seconds = 60
#
# Conflict detector tuning. Active only when supervisor mode is enabled.
# [supervisor.conflict]
# window_seconds = 120          # escalate unresolved in-flight conflicts after this many seconds
# warn_on_intent_overlap = true # emit feedback when two agent.intent declarations overlap
# escalate_on_violation = true  # also publish agent.question to supervisor on ownership violations

# Common dev-command allowlist. When supervisor mode starts a session,
# git-paw seeds .claude/settings.json::allowed_bash_prefixes with the
# universal preset (non-destructive git verbs + find / grep / sed -n) so
# agents do not hit a permission prompt for each variant. Opt into a
# toolchain's curated grants with stacks (named presets: rust / node /
# python / go); extend with project-specific prefixes via extra. Opt out
# entirely by setting enabled = false.
# [supervisor.common_dev_allowlist]
# enabled = true
# stacks = ["rust"]
# extra = ["just", "mdbook build", "openspec validate"]

# opsx (OpenSpec) role gating. When the session's spec engine is OpenSpec,
# git-paw's post-commit guard detects archive activity (`/opsx:archive` /
# `openspec archive`) by a non-supervisor agent and reacts per this mode:
#   "warn"  (default) — feedback to the offending agent + a permission_pattern
#                       learning the user sees in learnings.
#   "block"           — warn behaviour PLUS a feedback to the supervisor
#                       requesting it revert the offending commit.
#   "off"             — guard disabled entirely.
# The guard is inert under non-OpenSpec engines (speckit, markdown).
# [opsx]
# role_gating = "warn"

# Custom CLI definitions.
# [clis.my-agent]
# command = "/usr/local/bin/my-agent"
# display_name = "My Agent"

# Named presets for quick launches.
# [presets.my-preset]
# branches = ["feat/api", "fix/db"]
# cli = ""
"#
    .to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    // --- Parsing behavior ---

    #[test]
    fn parses_config_with_all_fields() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            r#"
default_cli = "claude"
mouse = false
default_spec_cli = "gemini"
branch_prefix = "spec/"

[clis.my-agent]
command = "/usr/local/bin/my-agent"
display_name = "My Agent"

[clis.local-llm]
command = "ollama-code"

[presets.backend]
branches = ["feature/api", "fix/db"]
cli = "claude"

[specs]
dir = "my-specs"
type = "openspec"

[logging]
enabled = true
"#,
        );

        let config = load_config_file(&path).unwrap().unwrap();
        assert_eq!(config.default_cli.as_deref(), Some("claude"));
        assert_eq!(config.mouse, Some(false));
        assert_eq!(config.default_spec_cli.as_deref(), Some("gemini"));
        assert_eq!(config.branch_prefix.as_deref(), Some("spec/"));
        assert_eq!(config.clis.len(), 2);
        assert_eq!(
            config.clis["my-agent"].display_name.as_deref(),
            Some("My Agent")
        );
        assert_eq!(config.clis["local-llm"].command, "ollama-code");
        assert_eq!(config.presets["backend"].cli, "claude");
        assert_eq!(
            config.presets["backend"].branches,
            vec!["feature/api", "fix/db"]
        );
        let specs = config.specs.unwrap();
        assert_eq!(specs.dir.as_deref(), Some("my-specs"));
        assert_eq!(specs.spec_type.as_deref(), Some("openspec"));
        let logging = config.logging.unwrap();
        assert!(logging.enabled);
    }

    #[test]
    fn all_fields_are_optional() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "default_cli = \"gemini\"\n");

        let config = load_config_file(&path).unwrap().unwrap();
        assert_eq!(config.default_cli.as_deref(), Some("gemini"));
        assert_eq!(config.mouse, None);
        assert!(config.clis.is_empty());
        assert!(config.presets.is_empty());
    }

    #[test]
    fn returns_defaults_when_no_files_exist() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("nonexistent").join("config.toml");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        let config = load_config_from(&global_path, &repo_root).unwrap();
        assert_eq!(config.default_cli, None);
        assert_eq!(config.mouse, None);
        assert!(config.clis.is_empty());
        assert!(config.presets.is_empty());
    }

    #[test]
    fn reports_error_for_invalid_toml() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("bad.toml");
        write_file(&path, "this is not [valid toml");

        let err = load_config_file(&path).unwrap_err();
        assert!(err.to_string().contains("bad.toml"));
    }

    // --- Merge behavior (through file I/O) ---

    #[test]
    fn repo_config_overrides_global_scalars() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("global").join("config.toml");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        write_file(&global_path, "default_cli = \"claude\"\nmouse = true\n");
        write_file(
            &repo_config_path(&repo_root),
            "default_cli = \"gemini\"\n", // mouse intentionally absent
        );

        let config = load_config_from(&global_path, &repo_root).unwrap();
        assert_eq!(config.default_cli.as_deref(), Some("gemini")); // repo wins
        assert_eq!(config.mouse, Some(true)); // global preserved when repo absent
    }

    #[test]
    fn repo_config_merges_cli_maps() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("global").join("config.toml");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        write_file(&global_path, "[clis.agent-a]\ncommand = \"/bin/a\"\n");
        write_file(
            &repo_config_path(&repo_root),
            "[clis.agent-b]\ncommand = \"/bin/b\"\n",
        );

        let config = load_config_from(&global_path, &repo_root).unwrap();
        assert_eq!(config.clis.len(), 2);
        assert!(config.clis.contains_key("agent-a"));
        assert!(config.clis.contains_key("agent-b"));
    }

    #[test]
    fn repo_cli_overrides_global_cli_with_same_name() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("global").join("config.toml");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        write_file(&global_path, "[clis.my-agent]\ncommand = \"/old/path\"\n");
        write_file(
            &repo_config_path(&repo_root),
            "[clis.my-agent]\ncommand = \"/new/path\"\ndisplay_name = \"Overridden\"\n",
        );

        let config = load_config_from(&global_path, &repo_root).unwrap();
        assert_eq!(config.clis["my-agent"].command, "/new/path");
        assert_eq!(
            config.clis["my-agent"].display_name.as_deref(),
            Some("Overridden")
        );
    }

    #[test]
    fn load_config_from_reads_global_file_when_no_repo() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("global").join("config.toml");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        write_file(&global_path, "default_cli = \"claude\"\nmouse = false\n");
        // No .git-paw/config.toml in repo_root

        let config = load_config_from(&global_path, &repo_root).unwrap();
        assert_eq!(config.default_cli.as_deref(), Some("claude"));
        assert_eq!(config.mouse, Some(false));
    }

    #[test]
    fn load_config_from_reads_repo_file_when_no_global() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("nonexistent").join("config.toml");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        write_file(&repo_config_path(&repo_root), "default_cli = \"codex\"\n");

        let config = load_config_from(&global_path, &repo_root).unwrap();
        assert_eq!(config.default_cli.as_deref(), Some("codex"));
    }

    // --- Preset behavior ---

    #[test]
    fn preset_accessible_by_name() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("global").join("config.toml");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        write_file(
            &repo_config_path(&repo_root),
            "[presets.backend]\nbranches = [\"feat/api\", \"fix/db\"]\ncli = \"claude\"\n",
        );

        let config = load_config_from(&global_path, &repo_root).unwrap();
        let preset = config.get_preset("backend").unwrap();
        assert_eq!(preset.cli, "claude");
        assert_eq!(preset.branches, vec!["feat/api", "fix/db"]);
    }

    #[test]
    fn preset_returns_none_when_not_in_config() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("config.toml");
        write_file(&global_path, "default_cli = \"claude\"\n");

        let config = load_config_file(&global_path).unwrap().unwrap();
        assert!(config.get_preset("nonexistent").is_none());
    }

    // --- add_custom_cli behavior ---

    #[test]
    fn add_cli_writes_to_config_file() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("git-paw").join("config.toml");

        // Add a CLI with an absolute path (no PATH resolution needed)
        add_custom_cli_to(
            &config_path,
            "my-agent",
            "/usr/local/bin/my-agent",
            Some("My Agent"),
        )
        .unwrap();

        // Verify by loading the file back
        let config = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(config.clis.len(), 1);
        assert_eq!(config.clis["my-agent"].command, "/usr/local/bin/my-agent");
        assert_eq!(
            config.clis["my-agent"].display_name.as_deref(),
            Some("My Agent")
        );
    }

    #[test]
    fn add_cli_preserves_existing_entries() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("git-paw").join("config.toml");

        add_custom_cli_to(&config_path, "first", "/bin/first", None).unwrap();
        add_custom_cli_to(&config_path, "second", "/bin/second", None).unwrap();

        let config = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(config.clis.len(), 2);
        assert!(config.clis.contains_key("first"));
        assert!(config.clis.contains_key("second"));
    }

    #[test]
    fn add_cli_errors_when_command_not_on_path() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let err = add_custom_cli_to(&config_path, "bad", "surely-nonexistent-binary-xyz", None)
            .unwrap_err();
        assert!(err.to_string().contains("not found on PATH"));
    }

    // --- remove_custom_cli behavior ---

    #[test]
    fn remove_cli_deletes_entry_from_config_file() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("git-paw").join("config.toml");

        // Set up: add two CLIs
        add_custom_cli_to(&config_path, "keep-me", "/bin/keep", None).unwrap();
        add_custom_cli_to(&config_path, "remove-me", "/bin/remove", None).unwrap();

        // Act: remove one
        remove_custom_cli_from(&config_path, "remove-me").unwrap();

        // Verify: only the kept CLI remains
        let config = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(config.clis.len(), 1);
        assert!(config.clis.contains_key("keep-me"));
        assert!(!config.clis.contains_key("remove-me"));
    }

    #[test]
    fn remove_nonexistent_cli_returns_cli_not_found_error() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        // Empty config file
        write_file(&config_path, "");

        let err = remove_custom_cli_from(&config_path, "nonexistent").unwrap_err();
        match err {
            PawError::CliNotFound(name) => assert_eq!(name, "nonexistent"),
            other => panic!("expected CliNotFound, got: {other}"),
        }
    }

    #[test]
    fn remove_cli_from_empty_config_returns_error() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        // No file at all

        let err = remove_custom_cli_from(&config_path, "ghost").unwrap_err();
        match err {
            PawError::CliNotFound(name) => assert_eq!(name, "ghost"),
            other => panic!("expected CliNotFound, got: {other}"),
        }
    }

    // --- Round-trip: config survives write + read ---

    // --- default_spec_cli behavior ---

    #[test]
    fn parses_default_spec_cli_when_present() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "default_spec_cli = \"claude\"\n");

        let config = load_config_file(&path).unwrap().unwrap();
        assert_eq!(config.default_spec_cli.as_deref(), Some("claude"));
    }

    #[test]
    fn default_spec_cli_defaults_to_none() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "default_cli = \"claude\"\n");

        let config = load_config_file(&path).unwrap().unwrap();
        assert_eq!(config.default_spec_cli, None);
    }

    #[test]
    fn repo_overrides_global_default_spec_cli() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("global").join("config.toml");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        write_file(&global_path, "default_spec_cli = \"claude\"\n");
        write_file(
            &repo_config_path(&repo_root),
            "default_spec_cli = \"gemini\"\n",
        );

        let config = load_config_from(&global_path, &repo_root).unwrap();
        assert_eq!(config.default_spec_cli.as_deref(), Some("gemini"));
    }

    #[test]
    fn global_default_spec_cli_preserved_when_repo_absent() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("global").join("config.toml");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        write_file(&global_path, "default_spec_cli = \"claude\"\n");

        let config = load_config_from(&global_path, &repo_root).unwrap();
        assert_eq!(config.default_spec_cli.as_deref(), Some("claude"));
    }

    // --- Round-trip: config survives write + read ---

    #[test]
    fn config_survives_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let original = PawConfig {
            default_cli: Some("claude".into()),
            default_spec_cli: None,
            branch_prefix: None,
            mouse: Some(true),
            clis: HashMap::from([(
                "test".into(),
                CustomCli {
                    command: "/bin/test".into(),
                    display_name: Some("Test CLI".into()),
                    submit_delay_ms: None,
                    settings_path: None,
                },
            )]),
            presets: HashMap::from([(
                "dev".into(),
                Preset {
                    branches: vec!["main".into()],
                    cli: "claude".into(),
                },
            )]),
            specs: None,
            logging: None,
            dashboard: None,
            broker: BrokerConfig::default(),
            supervisor: None,
            governance: GovernanceConfig::default(),
            layout: None,
            opsx: None,
            mcp: McpConfig::default(),
            worktree_placement: Some(WorktreePlacement::Child),
        };

        save_config_to(&config_path, &original).unwrap();
        let loaded = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(original, loaded);
    }

    // --- Gap #1: Parse [specs] section with populated fields ---

    #[test]
    fn parses_specs_section_with_populated_fields() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[specs]\ndir = \"my-specs\"\ntype = \"openspec\"\n");

        let config = load_config_file(&path).unwrap().unwrap();
        let specs = config.specs.unwrap();
        assert_eq!(specs.dir.as_deref(), Some("my-specs"));
        assert_eq!(specs.spec_type.as_deref(), Some("openspec"));
    }

    // --- Gap #2: Parse [logging] section with enabled ---

    #[test]
    fn parses_logging_section_with_enabled() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[logging]\nenabled = true\n");

        let config = load_config_file(&path).unwrap().unwrap();
        let logging = config.logging.unwrap();
        assert!(logging.enabled);
    }

    // --- Gap #3: Round-trip with specs and logging populated ---

    #[test]
    fn round_trip_with_specs_and_logging() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let original = PawConfig {
            specs: Some(SpecsConfig {
                dir: Some("specs".into()),
                spec_type: Some("openspec".into()),
            }),
            logging: Some(LoggingConfig { enabled: true }),
            ..Default::default()
        };

        save_config_to(&config_path, &original).unwrap();
        let loaded = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(original, loaded);
        assert_eq!(loaded.specs.unwrap().dir.as_deref(), Some("specs"));
        assert!(loaded.logging.unwrap().enabled);
    }

    // --- Gap #4: Generated config is valid TOML ---

    #[test]
    fn generated_default_config_is_valid_toml() {
        let raw = generate_default_config();
        let stripped: String = raw
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect::<Vec<&str>>()
            .join("\n");

        let parsed: Result<PawConfig, _> = toml::from_str(&stripped);
        assert!(
            parsed.is_ok(),
            "generated config with comments stripped should be valid TOML, got: {:?}",
            parsed.unwrap_err()
        );
    }

    // --- Gap #5: branch_prefix merge ---

    #[test]
    fn branch_prefix_repo_overrides_global() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("global").join("config.toml");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        write_file(&global_path, "branch_prefix = \"feat/\"\n");
        write_file(&repo_config_path(&repo_root), "branch_prefix = \"spec/\"\n");

        let config = load_config_from(&global_path, &repo_root).unwrap();
        assert_eq!(config.branch_prefix.as_deref(), Some("spec/"));
    }

    #[test]
    fn generated_default_config_contains_commented_examples() {
        let output = generate_default_config();
        assert!(
            output.contains("default_spec_cli"),
            "should contain default_spec_cli"
        );
        assert!(
            output.contains("branch_prefix"),
            "should contain branch_prefix"
        );
        assert!(output.contains("[specs]"), "should contain [specs]");
        assert!(output.contains("[logging]"), "should contain [logging]");
        assert!(output.contains("[broker]"), "should contain [broker]");
    }

    #[test]
    fn generated_default_config_contains_child_worktree_placement() {
        let output = generate_default_config();
        assert!(
            output.contains("worktree_placement = \"child\""),
            "generated config must set child worktree placement for new repos"
        );
        // The line must be active (not commented) so it actually takes effect.
        let parsed: PawConfig = toml::from_str(&output).expect("generated config parses");
        assert_eq!(
            parsed.worktree_placement(),
            WorktreePlacement::Child,
            "generated config must resolve to child placement"
        );
    }

    // --- BrokerConfig ---

    #[test]
    fn broker_config_defaults() {
        let config = BrokerConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.port, 9119);
        assert_eq!(config.bind, "127.0.0.1");
    }

    #[test]
    fn broker_config_url() {
        let config = BrokerConfig::default();
        assert_eq!(config.url(), "http://127.0.0.1:9119");

        let custom = BrokerConfig {
            enabled: true,
            port: 8080,
            bind: "0.0.0.0".to_string(),
            ..Default::default()
        };
        assert_eq!(custom.url(), "http://0.0.0.0:8080");
    }

    #[test]
    fn empty_config_gets_broker_defaults() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "");

        let config = load_config_file(&path).unwrap().unwrap();
        assert!(!config.broker.enabled);
        assert_eq!(config.broker.port, 9119);
        assert_eq!(config.broker.bind, "127.0.0.1");
    }

    #[test]
    fn parses_full_broker_section() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[broker]\nenabled = true\nport = 8080\nbind = \"0.0.0.0\"\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        assert!(config.broker.enabled);
        assert_eq!(config.broker.port, 8080);
        assert_eq!(config.broker.bind, "0.0.0.0");
    }

    #[test]
    fn parses_partial_broker_section() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[broker]\nenabled = true\n");

        let config = load_config_file(&path).unwrap().unwrap();
        assert!(config.broker.enabled);
        assert_eq!(config.broker.port, 9119);
        assert_eq!(config.broker.bind, "127.0.0.1");
    }

    // --- SupervisorConfig ---

    #[test]
    fn supervisor_is_none_when_section_absent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "default_cli = \"claude\"\n");

        let config = load_config_file(&path).unwrap().unwrap();
        assert!(config.supervisor.is_none());
    }

    #[test]
    fn parses_full_supervisor_section() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\n\
             enabled = true\n\
             cli = \"claude\"\n\
             test_command = \"just check\"\n\
             agent_approval = \"full-auto\"\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert!(supervisor.enabled);
        assert_eq!(supervisor.cli.as_deref(), Some("claude"));
        assert_eq!(supervisor.test_command.as_deref(), Some("just check"));
        assert_eq!(supervisor.agent_approval, ApprovalLevel::FullAuto);
    }

    #[test]
    fn parses_partial_supervisor_section() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[supervisor]\nenabled = true\n");

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert!(supervisor.enabled);
        assert_eq!(supervisor.cli, None);
        assert_eq!(supervisor.test_command, None);
        assert_eq!(supervisor.agent_approval, ApprovalLevel::Auto);
    }

    // --- verify_on_commit_nudge (per-commit-verification-v0-6-x) ---

    #[test]
    fn verify_on_commit_nudge_defaults_true_when_absent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[supervisor]\nenabled = true\n");

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert_eq!(
            supervisor.verify_on_commit_nudge, None,
            "an omitted field must deserialise as None"
        );
        assert!(
            supervisor.verify_on_commit_nudge_enabled(),
            "an unset verify_on_commit_nudge must resolve to true (default on)"
        );
    }

    #[test]
    fn verify_on_commit_nudge_explicit_false_disables() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\nenabled = true\nverify_on_commit_nudge = false\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert_eq!(supervisor.verify_on_commit_nudge, Some(false));
        assert!(
            !supervisor.verify_on_commit_nudge_enabled(),
            "an explicit `false` must disable the nudge"
        );
    }

    #[test]
    fn verify_on_commit_nudge_explicit_true_enables() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\nenabled = true\nverify_on_commit_nudge = true\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert_eq!(supervisor.verify_on_commit_nudge, Some(true));
        assert!(supervisor.verify_on_commit_nudge_enabled());
    }

    #[test]
    fn rejects_invalid_approval_level() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[supervisor]\nagent_approval = \"yolo\"\n");

        let err = load_config_file(&path).unwrap_err();
        assert!(
            err.to_string().contains("yolo"),
            "error should mention invalid value, got: {err}"
        );
    }

    #[test]
    fn supervisor_round_trips_through_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let original = PawConfig {
            supervisor: Some(SupervisorConfig {
                enabled: true,
                cli: Some("claude".into()),
                test_command: Some("just check".into()),
                lint_command: None,
                build_command: None,
                doc_build_command: None,
                doc_tool_command: None,
                spec_validate_command: None,
                fmt_check_command: None,
                security_audit_command: None,
                agent_approval: ApprovalLevel::FullAuto,
                auto_approve: None,
                conflict: ConflictConfig::default(),
                learnings: false,
                learnings_config: LearningsConfig::default(),
                common_dev_allowlist: CommonDevAllowlistConfig::default(),
                verify_on_commit_nudge: None,
                strict_branch_guard: None,
                auto_revert: None,
                manual_approvals_log: None,
                no_progress_window_seconds: None,
                context_bloat_threshold_k: None,
                blocked_on_supervisor_window_seconds: None,
                tell: TellConfig::default(),
            }),
            ..Default::default()
        };

        save_config_to(&config_path, &original).unwrap();
        let loaded = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(loaded.supervisor, original.supervisor);
    }

    // --- manual_approvals_log (approval-pattern-surfacing) ---

    #[test]
    fn manual_approvals_log_defaults_to_true_when_absent() {
        // [supervisor] present without the field → recording on by default.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[supervisor]\nenabled = true\n");
        let cfg = load_config_file(&path).unwrap().unwrap();
        let sup = cfg.supervisor.unwrap();
        assert_eq!(sup.manual_approvals_log, None);
        assert!(
            sup.manual_approvals_log_enabled(),
            "absent field must resolve to true"
        );
    }

    #[test]
    fn manual_approvals_log_explicit_false_opts_out() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\nenabled = true\nmanual_approvals_log = false\n",
        );
        let cfg = load_config_file(&path).unwrap().unwrap();
        let sup = cfg.supervisor.unwrap();
        assert_eq!(sup.manual_approvals_log, Some(false));
        assert!(!sup.manual_approvals_log_enabled());
    }

    #[test]
    fn pre_v050_config_parses_with_manual_approvals_log_absent() {
        // A config produced before this change (no `manual_approvals_log`
        // field) parses cleanly and the resolver still yields true.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\nenabled = true\ncli = \"claude\"\nlearnings = true\n",
        );
        let cfg = load_config_file(&path).unwrap().unwrap();
        let sup = cfg.supervisor.unwrap();
        assert_eq!(sup.manual_approvals_log, None);
        assert!(sup.manual_approvals_log_enabled());
    }

    // --- Gate-command fields (supervisor-gate-templating-v0-5-x) ---

    #[test]
    fn strict_branch_guard_defaults_to_true_and_honours_opt_out() {
        // Absent field → enforcement on by default.
        let on = TempDir::new().unwrap();
        let on_path = on.path().join("config.toml");
        write_file(&on_path, "[supervisor]\nenabled = true\n");
        let cfg = load_config_file(&on_path).unwrap().unwrap();
        let sup = cfg.supervisor.unwrap();
        assert_eq!(sup.strict_branch_guard, None);
        assert!(sup.strict_branch_guard(), "default must resolve to true");

        // Explicit opt-out → enforcement off (detection still applies).
        let off = TempDir::new().unwrap();
        let off_path = off.path().join("config.toml");
        write_file(
            &off_path,
            "[supervisor]\nenabled = true\nstrict_branch_guard = false\n",
        );
        let cfg = load_config_file(&off_path).unwrap().unwrap();
        let sup = cfg.supervisor.unwrap();
        assert_eq!(sup.strict_branch_guard, Some(false));
        assert!(!sup.strict_branch_guard());
    }

    #[test]
    fn gate_command_fields_default_to_none() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[supervisor]\nenabled = true\n");

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert_eq!(supervisor.test_command, None);
        assert_eq!(supervisor.lint_command, None);
        assert_eq!(supervisor.build_command, None);
        assert_eq!(supervisor.doc_build_command, None);
        assert_eq!(supervisor.doc_tool_command, None);
        assert_eq!(supervisor.spec_validate_command, None);
        assert_eq!(supervisor.fmt_check_command, None);
        assert_eq!(supervisor.security_audit_command, None);
    }

    #[test]
    fn gate_command_fields_round_trip() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let original = PawConfig {
            supervisor: Some(SupervisorConfig {
                enabled: true,
                cli: Some("claude".into()),
                test_command: Some("just check".into()),
                lint_command: Some("cargo clippy -- -D warnings".into()),
                build_command: Some("cargo build".into()),
                doc_build_command: Some("mdbook build docs/".into()),
                doc_tool_command: Some("cargo doc --no-deps".into()),
                spec_validate_command: Some("openspec validate {{CHANGE_ID}} --strict".into()),
                fmt_check_command: Some("cargo fmt --check".into()),
                security_audit_command: Some("cargo audit".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        save_config_to(&config_path, &original).unwrap();
        let loaded = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(loaded.supervisor, original.supervisor);
    }

    #[test]
    fn gate_command_fields_omit_from_toml_when_none() {
        let supervisor = SupervisorConfig {
            enabled: true,
            test_command: None,
            lint_command: None,
            build_command: None,
            doc_build_command: None,
            doc_tool_command: None,
            spec_validate_command: None,
            fmt_check_command: None,
            security_audit_command: None,
            ..Default::default()
        };
        let serialized = toml::to_string_pretty(&supervisor).unwrap();
        for key in [
            "test_command",
            "lint_command",
            "build_command",
            "doc_build_command",
            "doc_tool_command",
            "spec_validate_command",
            "fmt_check_command",
            "security_audit_command",
        ] {
            assert!(
                !serialized.contains(key),
                "TOML serialised with None gate fields should omit `{key}`; got:\n{serialized}",
            );
        }
    }

    // --- stuck/bloat detection thresholds (supervisor-stuck-bloat-detection) ---

    #[test]
    fn stuck_detection_fields_default_to_none() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[supervisor]\nenabled = true\n");

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert_eq!(supervisor.no_progress_window_seconds, None);
        assert_eq!(supervisor.context_bloat_threshold_k, None);
        assert_eq!(supervisor.blocked_on_supervisor_window_seconds, None);
    }

    #[test]
    fn stuck_detection_fields_round_trip() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let original = PawConfig {
            supervisor: Some(SupervisorConfig {
                enabled: true,
                no_progress_window_seconds: Some(1800),
                context_bloat_threshold_k: Some(300),
                blocked_on_supervisor_window_seconds: Some(600),
                ..Default::default()
            }),
            ..Default::default()
        };

        save_config_to(&config_path, &original).unwrap();
        let loaded = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(loaded.supervisor, original.supervisor);
        let supervisor = loaded.supervisor.unwrap();
        assert_eq!(supervisor.no_progress_window_seconds, Some(1800));
        assert_eq!(supervisor.context_bloat_threshold_k, Some(300));
        assert_eq!(supervisor.blocked_on_supervisor_window_seconds, Some(600));
    }

    #[test]
    fn stuck_detection_fields_omit_from_toml_when_none() {
        let supervisor = SupervisorConfig {
            enabled: true,
            no_progress_window_seconds: None,
            context_bloat_threshold_k: None,
            blocked_on_supervisor_window_seconds: None,
            ..Default::default()
        };
        let serialized = toml::to_string_pretty(&supervisor).unwrap();
        for key in [
            "no_progress_window_seconds",
            "context_bloat_threshold_k",
            "blocked_on_supervisor_window_seconds",
        ] {
            assert!(
                !serialized.contains(key),
                "TOML serialised with None stuck-detection fields should omit `{key}`; got:\n{serialized}",
            );
        }
    }

    #[test]
    fn stuck_detection_fields_pre_existing_config_loads() {
        // A config authored before these fields existed SHALL load cleanly with
        // the new fields defaulting to None (backward compatibility).
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\n\
             enabled = true\n\
             test_command = \"just check\"\n\
             strict_branch_guard = true\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert_eq!(supervisor.no_progress_window_seconds, None);
        assert_eq!(supervisor.context_bloat_threshold_k, None);
        assert_eq!(supervisor.blocked_on_supervisor_window_seconds, None);
        assert_eq!(supervisor.test_command.as_deref(), Some("just check"));
    }

    #[test]
    fn stuck_detection_fields_explicit_values_preserved() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\n\
             enabled = true\n\
             no_progress_window_seconds = 900\n\
             context_bloat_threshold_k = 200\n\
             blocked_on_supervisor_window_seconds = 1200\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert_eq!(supervisor.no_progress_window_seconds, Some(900));
        assert_eq!(supervisor.context_bloat_threshold_k, Some(200));
        assert_eq!(supervisor.blocked_on_supervisor_window_seconds, Some(1200));
    }

    // --- doc_tool_command (lang-agnostic-skills) ---

    #[test]
    fn doc_tool_command_default_none() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[supervisor]\nenabled = true\n");

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert_eq!(supervisor.doc_tool_command, None);
    }

    #[test]
    fn doc_tool_command_explicit_value_preserved() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\n\
             enabled = true\n\
             doc_tool_command = \"sphinx-build -W docs docs/_build\"\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert_eq!(
            supervisor.doc_tool_command.as_deref(),
            Some("sphinx-build -W docs docs/_build"),
            "explicit doc_tool_command value (including all whitespace) must be preserved verbatim",
        );
    }

    #[test]
    fn doc_tool_command_v0_5_config_parses_without_field() {
        // A v0.5.0 config that predates the doc_tool_command field SHALL
        // load cleanly with the field defaulting to None.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\n\
             enabled = true\n\
             test_command = \"just check\"\n\
             lint_command = \"cargo clippy -- -D warnings\"\n\
             build_command = \"cargo build\"\n\
             doc_build_command = \"mdbook build docs/\"\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert_eq!(supervisor.doc_tool_command, None);
        assert_eq!(supervisor.test_command.as_deref(), Some("just check"));
    }

    #[test]
    fn doc_tool_command_flows_into_gate_commands() {
        let supervisor = SupervisorConfig {
            doc_tool_command: Some("javadoc -d docs/api src/**/*.java".into()),
            ..Default::default()
        };
        let gates = supervisor.gate_commands();
        assert_eq!(
            gates.doc_tool_command,
            Some("javadoc -d docs/api src/**/*.java"),
        );
    }

    // --- CommonDevAllowlistConfig ---

    #[test]
    fn supervisor_common_dev_allowlist_defaults_when_section_absent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[supervisor]\nenabled = true\n");

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert!(supervisor.common_dev_allowlist.enabled);
        assert!(supervisor.common_dev_allowlist.stacks.is_empty());
        assert!(supervisor.common_dev_allowlist.extra.is_empty());
    }

    #[test]
    fn supervisor_common_dev_allowlist_stacks_parsed() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\nenabled = true\n\
             [supervisor.common_dev_allowlist]\nstacks = [\"rust\", \"node\"]\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert_eq!(
            supervisor.common_dev_allowlist.stacks,
            vec!["rust".to_string(), "node".to_string()],
        );
        // extra still defaults to empty; enabled stays true.
        assert!(supervisor.common_dev_allowlist.extra.is_empty());
        assert!(supervisor.common_dev_allowlist.enabled);
    }

    #[test]
    fn supervisor_common_dev_allowlist_disabled_opt_out() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\nenabled = true\n\
             [supervisor.common_dev_allowlist]\nenabled = false\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert!(!supervisor.common_dev_allowlist.enabled);
        // extra still defaults to empty.
        assert!(supervisor.common_dev_allowlist.extra.is_empty());
    }

    #[test]
    fn supervisor_common_dev_allowlist_extra_parsed() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\nenabled = true\n\
             [supervisor.common_dev_allowlist]\nextra = [\"pnpm test\", \"deno fmt\"]\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert_eq!(
            supervisor.common_dev_allowlist.extra,
            vec!["pnpm test".to_string(), "deno fmt".to_string()],
        );
        // enabled stays at default true.
        assert!(supervisor.common_dev_allowlist.enabled);
    }

    #[test]
    fn supervisor_common_dev_allowlist_round_trips_through_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let original = PawConfig {
            supervisor: Some(SupervisorConfig {
                enabled: true,
                common_dev_allowlist: CommonDevAllowlistConfig {
                    enabled: false,
                    stacks: vec!["rust".into(), "node".into()],
                    extra: vec!["pnpm test".into(), "uv pip install".into()],
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        save_config_to(&config_path, &original).unwrap();
        let loaded = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(loaded.supervisor, original.supervisor);
    }

    #[test]
    fn existing_pre_v05_config_loads_with_default_common_dev_allowlist() {
        // A pre-v0.5 supervisor config that omits the new sub-table must
        // still load and yield the documented defaults.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\n\
             enabled = true\n\
             cli = \"claude\"\n\
             test_command = \"just check\"\n\
             agent_approval = \"auto\"\n\
             [supervisor.conflict]\n\
             window_seconds = 60\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert!(supervisor.common_dev_allowlist.enabled);
        assert!(supervisor.common_dev_allowlist.extra.is_empty());
    }

    #[test]
    fn generated_default_config_template_contains_common_dev_allowlist_section() {
        let template = generate_default_config();
        assert!(
            template.contains("[supervisor.common_dev_allowlist]"),
            "default template should document the new sub-table",
        );
        assert!(
            template.contains("enabled = true"),
            "template should show the enabled default",
        );
        assert!(
            template.contains("extra ="),
            "template should illustrate the extra field",
        );
        assert!(
            template.contains("stacks ="),
            "template should illustrate the stacks field",
        );
    }

    // --- LearningsConfig (learnings-mode) ---

    #[test]
    fn learnings_defaults_to_false_when_supervisor_section_absent_field() {
        // [supervisor] present without `learnings` → learnings = false
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[supervisor]\nenabled = true\n");

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert!(!supervisor.learnings);
        assert_eq!(supervisor.learnings_config.flush_interval_seconds, 60);
    }

    #[test]
    fn learnings_true_loads() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[supervisor]\nenabled = true\nlearnings = true\n");

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert!(supervisor.learnings);
        // Defaults still applied for the nested table.
        assert_eq!(supervisor.learnings_config.flush_interval_seconds, 60);
    }

    #[test]
    fn learnings_config_custom_flush_interval_is_honoured() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\n\
             enabled = true\n\
             learnings = true\n\
             [supervisor.learnings_config]\n\
             flush_interval_seconds = 30\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert!(supervisor.learnings);
        assert_eq!(supervisor.learnings_config.flush_interval_seconds, 30);
    }

    #[test]
    fn learnings_config_defaults_when_table_absent() {
        // [supervisor.learnings_config] omitted → flush_interval_seconds = 60
        let cfg = LearningsConfig::default();
        assert_eq!(cfg.flush_interval_seconds, 60);
    }

    #[test]
    fn pre_v050_config_loads_with_learnings_false() {
        // A config produced before v0.5.0 (no `learnings` field, no
        // `[supervisor.learnings_config]` table) parses cleanly and yields
        // `learnings = false`.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "default_cli = \"claude\"\n\
             [supervisor]\n\
             enabled = true\n\
             agent_approval = \"auto\"\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert!(!supervisor.learnings);
        assert_eq!(supervisor.learnings_config.flush_interval_seconds, 60);
    }

    #[test]
    fn learnings_round_trips_through_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let original = PawConfig {
            supervisor: Some(SupervisorConfig {
                enabled: true,
                learnings: true,
                learnings_config: LearningsConfig {
                    flush_interval_seconds: 90,
                    broker_publish: BrokerPublish::ForceOff,
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        save_config_to(&config_path, &original).unwrap();
        let loaded = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(loaded.supervisor, original.supervisor);
        let supervisor = loaded.supervisor.unwrap();
        assert!(supervisor.learnings);
        assert_eq!(supervisor.learnings_config.flush_interval_seconds, 90);
    }

    #[test]
    fn existing_v030_config_loads_without_supervisor() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "default_cli = \"claude\"\n\
             mouse = true\n\
             [broker]\n\
             enabled = true\n\
             [logging]\n\
             enabled = false\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        assert_eq!(config.default_cli.as_deref(), Some("claude"));
        assert!(config.broker.enabled);
        assert!(config.supervisor.is_none());
    }

    #[test]
    fn generated_default_config_contains_commented_supervisor_section() {
        let output = generate_default_config();
        assert!(output.contains("[supervisor]"));
        assert!(output.contains("enabled"));
        assert!(output.contains("test_command"));
        assert!(output.contains("agent_approval"));
        // Stuck/bloat detection thresholds are listed with example values.
        assert!(output.contains("no_progress_window_seconds = 1500"));
        assert!(output.contains("context_bloat_threshold_k = 250"));
        assert!(output.contains("blocked_on_supervisor_window_seconds = 900"));
    }

    // --- DashboardConfig ---

    #[test]
    fn dashboard_config_defaults_to_disabled() {
        let config = DashboardConfig::default();
        assert!(!config.show_message_log);
    }

    #[test]
    fn parses_dashboard_section_with_show_message_log() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[dashboard]\nshow_message_log = true\n");

        let config = load_config_file(&path).unwrap().unwrap();
        let dashboard = config.dashboard.unwrap();
        assert!(dashboard.show_message_log);
    }

    #[test]
    fn dashboard_is_none_when_section_absent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "default_cli = \"claude\"\n");

        let config = load_config_file(&path).unwrap().unwrap();
        assert!(config.dashboard.is_none());
    }

    #[test]
    fn dashboard_merge_repo_wins() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("global").join("config.toml");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        write_file(&global_path, "[dashboard]\nshow_message_log = false\n");
        write_file(
            &repo_config_path(&repo_root),
            "[dashboard]\nshow_message_log = true\n",
        );

        let config = load_config_from(&global_path, &repo_root).unwrap();
        let dashboard = config.dashboard.unwrap();
        assert!(dashboard.show_message_log);
    }

    #[test]
    fn dashboard_round_trip_through_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let original = PawConfig {
            dashboard: Some(DashboardConfig {
                show_message_log: true,
                ..Default::default()
            }),
            ..Default::default()
        };

        save_config_to(&config_path, &original).unwrap();
        let loaded = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(loaded.dashboard, original.dashboard);
        assert!(loaded.dashboard.unwrap().show_message_log);
    }

    // --- BrokerLogConfig (dashboard-broker-log task 1.3) ---

    #[test]
    fn broker_log_config_defaults() {
        // Task 1.3: default load — cap 500, visible on, height > 12.
        let cfg = BrokerLogConfig::default();
        assert_eq!(cfg.max_messages, 500);
        assert!(cfg.default_visible);
        assert!(
            cfg.height_lines > 12,
            "default height_lines must be strictly greater than the v0.6.0 fixed 12, got {}",
            cfg.height_lines,
        );
    }

    #[test]
    fn dashboard_config_default_includes_broker_log_defaults() {
        // An entirely default DashboardConfig carries the documented
        // broker-log defaults so a bare `[dashboard]` section behaves
        // predictably.
        let cfg = DashboardConfig::default();
        assert_eq!(cfg.broker_log.max_messages, 500);
        assert!(cfg.broker_log.default_visible);
        assert!(cfg.broker_log.height_lines > 12);
    }

    #[test]
    fn parses_broker_log_section_with_explicit_overrides() {
        // Task 1.3: explicit override load.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[dashboard.broker_log]\nmax_messages = 100\ndefault_visible = false\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        let dashboard = config.dashboard.unwrap();
        assert_eq!(dashboard.broker_log.max_messages, 100);
        assert!(!dashboard.broker_log.default_visible);
    }

    #[test]
    fn broker_log_partial_section_fills_remaining_defaults() {
        // A `[dashboard.broker_log]` table that sets only one field still
        // loads the documented default for the other (per-field
        // `#[serde(default)]`).
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[dashboard.broker_log]\nmax_messages = 42\n");

        let config = load_config_file(&path).unwrap().unwrap();
        let broker_log = config.dashboard.unwrap().broker_log;
        assert_eq!(broker_log.max_messages, 42);
        assert!(
            broker_log.default_visible,
            "default_visible must fall back to true when omitted"
        );
        assert_eq!(
            broker_log.height_lines,
            BrokerLogConfig::default_height_lines(),
            "height_lines must fall back to the documented default when omitted"
        );
    }

    #[test]
    fn height_lines_parses_explicit_value() {
        // Configuration scenario "height_lines explicitly configured": an
        // explicit `[dashboard.broker_log] height_lines = 24` loads as 24.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[dashboard.broker_log]\nheight_lines = 24\n");

        let config = load_config_file(&path).unwrap().unwrap();
        let broker_log = config.dashboard.unwrap().broker_log;
        assert_eq!(broker_log.height_lines, 24);
    }

    #[test]
    fn height_lines_absent_uses_default() {
        // Configuration scenario "height_lines absent uses the default": a
        // `[dashboard.broker_log]` table that omits the field loads the
        // documented default, which is strictly greater than 12.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[dashboard.broker_log]\ndefault_visible = true\n");

        let config = load_config_file(&path).unwrap().unwrap();
        let broker_log = config.dashboard.unwrap().broker_log;
        assert_eq!(
            broker_log.height_lines,
            BrokerLogConfig::default_height_lines()
        );
        assert!(broker_log.height_lines > 12);
    }

    #[test]
    fn v050_dashboard_section_without_broker_log_still_parses() {
        // Task 1.3: a v0.5.0 config that predates the broker_log table must
        // load unchanged, with the new section materialising at its default.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[dashboard]\nshow_message_log = true\n");

        let config = load_config_file(&path).unwrap().unwrap();
        let dashboard = config.dashboard.unwrap();
        assert!(dashboard.show_message_log);
        assert_eq!(dashboard.broker_log, BrokerLogConfig::default());
    }

    #[test]
    fn broker_log_round_trips_through_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let original = PawConfig {
            dashboard: Some(DashboardConfig {
                show_message_log: false,
                broker_log: BrokerLogConfig {
                    max_messages: 250,
                    default_visible: false,
                    height_lines: 30,
                },
            }),
            ..Default::default()
        };

        save_config_to(&config_path, &original).unwrap();
        let loaded = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(loaded.dashboard, original.dashboard);
        // Configuration scenario "height_lines round-trips through save and
        // load": the re-parsed value matches what was written.
        assert_eq!(loaded.dashboard.unwrap().broker_log.height_lines, 30);
    }

    #[test]
    fn get_dashboard_returns_none_when_not_configured() {
        let config = PawConfig::default();
        assert!(config.get_dashboard().is_none());
    }

    #[test]
    fn get_dashboard_returns_config_when_present() {
        let config = PawConfig {
            dashboard: Some(DashboardConfig {
                show_message_log: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let dashboard = config.get_dashboard().unwrap();
        assert!(dashboard.show_message_log);
    }

    // --- approval_flags mapping ---

    #[test]
    fn approval_flags_claude_full_auto() {
        assert_eq!(
            approval_flags("claude", &ApprovalLevel::FullAuto),
            "--dangerously-skip-permissions"
        );
    }

    #[test]
    fn approval_flags_codex_auto() {
        assert_eq!(
            approval_flags("codex", &ApprovalLevel::Auto),
            "--approval-mode=auto-edit"
        );
    }

    #[test]
    fn approval_flags_codex_full_auto() {
        assert_eq!(
            approval_flags("codex", &ApprovalLevel::FullAuto),
            "--approval-mode=full-auto"
        );
    }

    #[test]
    fn approval_flags_unknown_cli_is_empty() {
        assert_eq!(approval_flags("some-agent", &ApprovalLevel::FullAuto), "");
    }

    #[test]
    fn approval_flags_manual_is_empty() {
        assert_eq!(approval_flags("claude", &ApprovalLevel::Manual), "");
        assert_eq!(approval_flags("codex", &ApprovalLevel::Manual), "");
    }

    #[test]
    fn approval_flags_is_deterministic() {
        let first = approval_flags("claude", &ApprovalLevel::FullAuto);
        let second = approval_flags("claude", &ApprovalLevel::FullAuto);
        assert_eq!(first, second);
    }

    #[test]
    fn supervisor_merge_repo_wins() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("global").join("config.toml");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        write_file(
            &global_path,
            "[supervisor]\nenabled = false\nagent_approval = \"manual\"\n",
        );
        write_file(
            &repo_config_path(&repo_root),
            "[supervisor]\nenabled = true\nagent_approval = \"full-auto\"\n",
        );

        let config = load_config_from(&global_path, &repo_root).unwrap();
        let supervisor = config.supervisor.unwrap();
        assert!(supervisor.enabled);
        assert_eq!(supervisor.agent_approval, ApprovalLevel::FullAuto);
    }

    #[test]
    fn broker_config_round_trip() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let original = PawConfig {
            broker: BrokerConfig {
                enabled: true,
                port: 9200,
                bind: "127.0.0.1".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        save_config_to(&config_path, &original).unwrap();
        let loaded = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(loaded.broker.enabled, original.broker.enabled);
        assert_eq!(loaded.broker.port, original.broker.port);
        assert_eq!(loaded.broker.bind, original.broker.bind);
    }

    // --- AutoApproveConfig (auto-approve-patterns / approval-configuration) ---

    #[test]
    fn auto_approve_defaults_match_spec() {
        let cfg = AutoApproveConfig::default();
        assert!(cfg.enabled, "enabled defaults to true");
        assert!(
            cfg.safe_commands.is_empty(),
            "safe_commands defaults to empty"
        );
        assert_eq!(cfg.stall_threshold_seconds, 30);
        assert_eq!(cfg.approval_level, ApprovalLevelPreset::Safe);
    }

    #[test]
    fn auto_approve_section_absent_keeps_supervisor_simple() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[supervisor]\nenabled = true\n");
        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert!(supervisor.auto_approve.is_none());
    }

    #[test]
    fn auto_approve_section_parses_full_body() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\n\
             enabled = true\n\
             [supervisor.auto_approve]\n\
             enabled = false\n\
             safe_commands = [\"just smoke\"]\n\
             stall_threshold_seconds = 60\n\
             approval_level = \"conservative\"\n",
        );
        let config = load_config_file(&path).unwrap().unwrap();
        let aa = config.supervisor.unwrap().auto_approve.unwrap();
        assert!(!aa.enabled);
        assert_eq!(aa.safe_commands, vec!["just smoke".to_string()]);
        assert_eq!(aa.stall_threshold_seconds, 60);
        assert_eq!(aa.approval_level, ApprovalLevelPreset::Conservative);
    }

    #[test]
    fn auto_approve_enabled_defaults_to_true_when_omitted() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\n[supervisor.auto_approve]\nstall_threshold_seconds = 30\n",
        );
        let config = load_config_file(&path).unwrap().unwrap();
        let aa = config.supervisor.unwrap().auto_approve.unwrap();
        assert!(aa.enabled, "enabled should default to true");
    }

    #[test]
    fn auto_approve_off_preset_forces_disabled() {
        let cfg = AutoApproveConfig {
            enabled: true,
            approval_level: ApprovalLevelPreset::Off,
            ..AutoApproveConfig::default()
        };
        let resolved = cfg.resolved();
        assert!(!resolved.enabled, "Off preset must force enabled = false");
    }

    // --- Bug 8: [broker.watcher] republish_working_ttl_seconds ---

    #[test]
    fn watcher_ttl_defaults_to_sixty_when_absent() {
        let cfg = WatcherConfig::default();
        assert_eq!(cfg.republish_working_ttl_seconds(), 60);
    }

    #[test]
    fn watcher_ttl_zero_disables() {
        let cfg = WatcherConfig {
            republish_working_ttl_seconds: Some(0),
        };
        assert_eq!(cfg.republish_working_ttl_seconds(), 0);
    }

    #[test]
    fn watcher_ttl_below_floor_clamps_to_five() {
        let cfg = WatcherConfig {
            republish_working_ttl_seconds: Some(2),
        };
        assert_eq!(
            cfg.republish_working_ttl_seconds(),
            WatcherConfig::MIN_REPUBLISH_TTL_SECONDS
        );
    }

    #[test]
    fn watcher_ttl_explicit_non_zero_is_preserved() {
        let cfg = WatcherConfig {
            republish_working_ttl_seconds: Some(120),
        };
        assert_eq!(cfg.republish_working_ttl_seconds(), 120);
    }

    #[test]
    fn watcher_ttl_parses_from_broker_table() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[broker]\nenabled = true\n[broker.watcher]\nrepublish_working_ttl_seconds = 0\n",
        );
        let config = load_config_file(&path).unwrap().unwrap();
        assert_eq!(config.broker.watcher.republish_working_ttl_seconds, Some(0));
        assert_eq!(config.broker.watcher.republish_working_ttl_seconds(), 0);
    }

    #[test]
    fn approve_worktree_writes_defaults_to_true_when_absent() {
        // Spec scenario: default true auto-approves (field unset).
        let cfg = AutoApproveConfig::default();
        assert!(
            cfg.approve_worktree_writes(),
            "absent approve_worktree_writes must resolve to true"
        );
    }

    #[test]
    fn approve_worktree_writes_explicit_false_resolves_false() {
        // Spec scenario: explicit false reverts to manual.
        let cfg = AutoApproveConfig {
            approve_worktree_writes: Some(false),
            ..AutoApproveConfig::default()
        };
        assert!(!cfg.approve_worktree_writes());
    }

    #[test]
    fn approve_worktree_writes_parses_from_toml() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\nenabled = true\n[supervisor.auto_approve]\napprove_worktree_writes = false\n",
        );
        let config = load_config_file(&path).unwrap().unwrap();
        let aa = config.supervisor.unwrap().auto_approve.unwrap();
        assert_eq!(aa.approve_worktree_writes, Some(false));
        assert!(!aa.approve_worktree_writes());
    }

    #[test]
    fn auto_approve_threshold_floor_clamps() {
        let cfg = AutoApproveConfig {
            stall_threshold_seconds: 0,
            ..AutoApproveConfig::default()
        };
        let resolved = cfg.resolved();
        assert_eq!(
            resolved.stall_threshold_seconds,
            AutoApproveConfig::MIN_STALL_THRESHOLD_SECONDS
        );
    }

    #[test]
    fn auto_approve_safe_preset_keeps_defaults() {
        let cfg = AutoApproveConfig {
            approval_level: ApprovalLevelPreset::Safe,
            ..AutoApproveConfig::default()
        };
        let wl = cfg.effective_whitelist();
        assert!(wl.iter().any(|c| c == "cargo test"));
        assert!(wl.iter().any(|c| c == "git push"));
        assert!(wl.iter().any(|c| c.starts_with("curl")));
    }

    #[test]
    fn auto_approve_conservative_drops_push_and_curl() {
        let cfg = AutoApproveConfig {
            approval_level: ApprovalLevelPreset::Conservative,
            ..AutoApproveConfig::default()
        };
        let wl = cfg.effective_whitelist();
        assert!(wl.iter().any(|c| c == "cargo test"));
        assert!(
            !wl.iter().any(|c| c.starts_with("git push")),
            "conservative drops git push"
        );
        assert!(
            !wl.iter().any(|c| c.starts_with("curl")),
            "conservative drops curl"
        );
    }

    #[test]
    fn auto_approve_extras_are_unioned_with_defaults() {
        let cfg = AutoApproveConfig {
            safe_commands: vec!["just lint".to_string(), "just test".to_string()],
            ..AutoApproveConfig::default()
        };
        let wl = cfg.effective_whitelist();
        assert!(wl.iter().any(|c| c == "cargo fmt"));
        assert!(wl.iter().any(|c| c == "just lint"));
        assert!(wl.iter().any(|c| c == "just test"));
    }

    #[test]
    fn auto_approve_empty_extras_keep_defaults() {
        let cfg = AutoApproveConfig::default();
        let wl = cfg.effective_whitelist();
        assert!(wl.iter().any(|c| c == "cargo test"));
    }

    /// Spec scenario `auto-approve-patterns/safe-command-classification`:
    /// "Config adds project-specific patterns" — a TOML config with
    /// `safe_commands = ["just smoke"]` must yield an effective whitelist
    /// such that `is_safe_command("just smoke -v", &whitelist)` is true.
    /// "Config does not weaken defaults" — `safe_commands = []` must keep
    /// the built-in defaults available to `is_safe_command`.
    #[test]
    fn toml_extras_classify_via_is_safe_command_and_empty_extras_keep_defaults() {
        use crate::supervisor::auto_approve::is_safe_command;

        // (1) Extras case: a project-specific entry parsed from TOML must
        //     classify a command using that prefix as safe.
        let tmp = TempDir::new().unwrap();
        let extras_path = tmp.path().join("extras.toml");
        write_file(
            &extras_path,
            "[supervisor]\n\
             enabled = true\n\
             [supervisor.auto_approve]\n\
             safe_commands = [\"just smoke\"]\n",
        );
        let extras_config = load_config_file(&extras_path).unwrap().unwrap();
        let extras_aa = extras_config.supervisor.unwrap().auto_approve.unwrap();
        let extras_whitelist = extras_aa.effective_whitelist();
        assert!(
            is_safe_command("just smoke -v", &extras_whitelist),
            "TOML extra `just smoke` must accept `just smoke -v`"
        );
        // The defaults must still be present alongside the extra.
        assert!(
            is_safe_command("cargo test", &extras_whitelist),
            "extras must not displace built-in defaults"
        );

        // (2) Empty extras: the effective whitelist must still classify the
        //     built-in defaults (e.g. `cargo test`) as safe.
        let empty_path = tmp.path().join("empty.toml");
        write_file(
            &empty_path,
            "[supervisor]\n\
             enabled = true\n\
             [supervisor.auto_approve]\n\
             safe_commands = []\n",
        );
        let empty_config = load_config_file(&empty_path).unwrap().unwrap();
        let empty_aa = empty_config.supervisor.unwrap().auto_approve.unwrap();
        let empty_whitelist = empty_aa.effective_whitelist();
        assert!(
            is_safe_command("cargo test", &empty_whitelist),
            "empty safe_commands must keep built-in defaults"
        );
        assert!(
            is_safe_command("cargo fmt --check", &empty_whitelist),
            "empty safe_commands must keep `cargo fmt` default"
        );
        // A command outside the defaults must still be rejected.
        assert!(
            !is_safe_command("rm -rf /tmp/foo", &empty_whitelist),
            "empty safe_commands must not whitelist arbitrary commands"
        );
    }

    // --- ConflictConfig (supervisor.conflict sub-table) ---

    #[test]
    fn conflict_config_defaults_match_spec() {
        let cfg = ConflictConfig::default();
        assert_eq!(cfg.window_seconds, 120);
        assert!(cfg.warn_on_intent_overlap);
        assert!(cfg.escalate_on_violation);
    }

    #[test]
    fn supervisor_with_no_conflict_section_loads_defaults() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[supervisor]\nenabled = true\n");
        let supervisor = load_config_file(&path)
            .unwrap()
            .unwrap()
            .supervisor
            .unwrap();
        assert_eq!(supervisor.conflict.window_seconds, 120);
        assert!(supervisor.conflict.warn_on_intent_overlap);
        assert!(supervisor.conflict.escalate_on_violation);
    }

    #[test]
    fn conflict_section_with_all_fields_overrides_defaults() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\n\
             enabled = true\n\
             [supervisor.conflict]\n\
             window_seconds = 300\n\
             warn_on_intent_overlap = false\n\
             escalate_on_violation = false\n",
        );
        let conflict = load_config_file(&path)
            .unwrap()
            .unwrap()
            .supervisor
            .unwrap()
            .conflict;
        assert_eq!(conflict.window_seconds, 300);
        assert!(!conflict.warn_on_intent_overlap);
        assert!(!conflict.escalate_on_violation);
    }

    #[test]
    fn conflict_section_with_partial_fields_keeps_other_defaults() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[supervisor]\n[supervisor.conflict]\nwindow_seconds = 60\n",
        );
        let conflict = load_config_file(&path)
            .unwrap()
            .unwrap()
            .supervisor
            .unwrap()
            .conflict;
        assert_eq!(conflict.window_seconds, 60);
        assert!(conflict.warn_on_intent_overlap);
        assert!(conflict.escalate_on_violation);
    }

    #[test]
    fn pre_v05_config_without_conflict_section_loads() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        // A v0.4-style config: supervisor enabled but no [supervisor.conflict].
        write_file(
            &path,
            "default_cli = \"claude\"\n\
             [supervisor]\n\
             enabled = true\n\
             agent_approval = \"auto\"\n",
        );
        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert!(supervisor.enabled);
        // The conflict sub-table defaults to ConflictConfig::default().
        assert_eq!(supervisor.conflict, ConflictConfig::default());
    }

    #[test]
    fn conflict_config_round_trips_through_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        let original = PawConfig {
            supervisor: Some(SupervisorConfig {
                enabled: true,
                conflict: ConflictConfig {
                    window_seconds: 90,
                    warn_on_intent_overlap: false,
                    escalate_on_violation: true,
                },
                ..Default::default()
            }),
            ..Default::default()
        };
        save_config_to(&config_path, &original).unwrap();
        let loaded = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(loaded.supervisor, original.supervisor);
    }

    #[test]
    fn v030_config_loads_without_auto_approve() {
        // Backward-compat: an existing v0.3.0 config that has neither
        // [supervisor] nor [supervisor.auto_approve] must parse cleanly.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "default_cli = \"claude\"\nmouse = true\n[broker]\nenabled = true\n",
        );
        let config = load_config_file(&path).unwrap().unwrap();
        assert!(config.supervisor.is_none());
        assert!(config.broker.enabled);
    }

    // --- GovernanceConfig (governance-config v0.5.0) ---

    /// Helper: lays out a repo with `.git-paw/config.toml` and an optional
    /// `SpecKit` `memory/constitution.md` so the `load_config_from`
    /// auto-wiring path can be exercised end-to-end.
    fn write_repo_config(repo_root: &Path, toml: &str) {
        write_file(&repo_config_path(repo_root), toml);
    }

    fn missing_global(tmp: &TempDir) -> PathBuf {
        tmp.path().join("nonexistent-global").join("config.toml")
    }

    // 3.1 No [governance] section → all paths None.
    #[test]
    fn governance_defaults_to_all_none_when_section_absent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "default_cli = \"claude\"\n");

        let config = load_config_file(&path).unwrap().unwrap();
        assert!(config.governance.adr.is_none());
        assert!(config.governance.test_strategy.is_none());
        assert!(config.governance.security.is_none());
        assert!(config.governance.dod.is_none());
        assert!(config.governance.constitution.is_none());
    }

    // 3.2 All paths populated.
    #[test]
    fn governance_all_paths_populated() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[governance]\n\
             adr = \"docs/adr\"\n\
             test_strategy = \"docs/test-strategy.md\"\n\
             security = \"docs/security-checklist.md\"\n\
             dod = \"docs/definition-of-done.md\"\n\
             constitution = \".specify/memory/constitution.md\"\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        assert_eq!(
            config.governance.adr.as_deref(),
            Some(Path::new("docs/adr"))
        );
        assert_eq!(
            config.governance.test_strategy.as_deref(),
            Some(Path::new("docs/test-strategy.md"))
        );
        assert_eq!(
            config.governance.security.as_deref(),
            Some(Path::new("docs/security-checklist.md"))
        );
        assert_eq!(
            config.governance.dod.as_deref(),
            Some(Path::new("docs/definition-of-done.md"))
        );
        assert_eq!(
            config.governance.constitution.as_deref(),
            Some(Path::new(".specify/memory/constitution.md"))
        );
    }

    // 3.3 Partial paths.
    #[test]
    fn governance_partial_paths_only_some_fields_populated() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[governance]\n\
             dod = \"docs/dod.md\"\n\
             security = \"docs/security.md\"\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        assert_eq!(
            config.governance.dod.as_deref(),
            Some(Path::new("docs/dod.md"))
        );
        assert_eq!(
            config.governance.security.as_deref(),
            Some(Path::new("docs/security.md"))
        );
        assert!(config.governance.adr.is_none());
        assert!(config.governance.test_strategy.is_none());
        assert!(config.governance.constitution.is_none());
    }

    // 3.4 Absolute path preserved as-is.
    #[test]
    fn governance_absolute_path_preserved_as_is() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[governance]\nadr = \"/absolute/path/to/adr\"\n");

        let config = load_config_file(&path).unwrap().unwrap();
        assert_eq!(
            config.governance.adr,
            Some(PathBuf::from("/absolute/path/to/adr"))
        );
    }

    // 3.5 Non-existent path loads cleanly without error.
    #[test]
    fn governance_nonexistent_path_loads_cleanly() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[governance]\ndod = \"docs/never-existed.md\"\n");

        let config = load_config_file(&path).unwrap().unwrap();
        assert_eq!(
            config.governance.dod,
            Some(PathBuf::from("docs/never-existed.md"))
        );
    }

    // 3.6 Round-trip via save → load.
    #[test]
    fn governance_round_trips_through_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let original = PawConfig {
            governance: GovernanceConfig {
                adr: Some(PathBuf::from("docs/adr")),
                test_strategy: Some(PathBuf::from("docs/test-strategy.md")),
                security: Some(PathBuf::from("docs/security.md")),
                dod: Some(PathBuf::from("docs/dod.md")),
                constitution: Some(PathBuf::from(".specify/memory/constitution.md")),
                readme: Some(PathBuf::from("README.md")),
                docs: Some(PathBuf::from("docs/src")),
            },
            ..Default::default()
        };

        save_config_to(&config_path, &original).unwrap();
        let loaded = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(loaded.governance, original.governance);
    }

    // 3.7 v0.4 fixture (no [governance]) loads with defaults.
    #[test]
    fn governance_v04_config_without_section_loads_with_defaults() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "default_cli = \"claude\"\n\
             mouse = true\n\
             [broker]\n\
             enabled = true\n\
             [supervisor]\n\
             enabled = true\n\
             [specs]\n\
             dir = \"specs\"\n\
             type = \"openspec\"\n\
             [clis.foo]\n\
             command = \"/bin/foo\"\n",
        );

        let config = load_config_file(&path).unwrap().unwrap();
        assert_eq!(config.governance, GovernanceConfig::default());
        assert!(config.governance.adr.is_none());
        assert!(config.governance.test_strategy.is_none());
        assert!(config.governance.security.is_none());
        assert!(config.governance.dod.is_none());
        assert!(config.governance.constitution.is_none());
        assert!(config.governance.readme.is_none());
        assert!(config.governance.docs.is_none());
    }

    // 3.8 GovernanceConfig::default() exposes only the documented path fields
    // (no `gates` field) — compile-time-style assertion via destructuring.
    #[test]
    fn governance_default_has_only_path_fields() {
        // If a future change adds a `gates` (or any other) field, this
        // destructure stops compiling, forcing the change author to
        // revisit the capability boundary explicitly.
        let GovernanceConfig {
            adr,
            test_strategy,
            security,
            dod,
            constitution,
            readme,
            docs,
        } = GovernanceConfig::default();
        assert!(adr.is_none());
        assert!(test_strategy.is_none());
        assert!(security.is_none());
        assert!(dod.is_none());
        assert!(constitution.is_none());
        assert!(readme.is_none());
        assert!(docs.is_none());
    }

    // governance-config delta: readme + docs parse from [governance].
    #[test]
    fn governance_parses_readme_and_docs_fields() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(
            &path,
            "[governance]\n\
             readme = \"README.md\"\n\
             docs = \"docs/src\"\n",
        );
        let config = load_config_file(&path).unwrap().unwrap();
        assert_eq!(config.governance.readme, Some(PathBuf::from("README.md")));
        assert_eq!(config.governance.docs, Some(PathBuf::from("docs/src")));
    }

    // governance-config delta: readme + docs default to None when omitted.
    #[test]
    fn governance_readme_and_docs_default_to_none_when_omitted() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, "[governance]\ndod = \"docs/dod.md\"\n");
        let config = load_config_file(&path).unwrap().unwrap();
        assert!(config.governance.readme.is_none());
        assert!(config.governance.docs.is_none());
        assert_eq!(config.governance.dod, Some(PathBuf::from("docs/dod.md")));
    }

    // governance-config delta: readme + docs survive round-trip serialization.
    #[test]
    fn governance_readme_and_docs_round_trip() {
        let original = GovernanceConfig {
            readme: Some(PathBuf::from("README.md")),
            docs: Some(PathBuf::from("docs/src")),
            ..Default::default()
        };
        let toml_str = toml::to_string(&original).unwrap();
        let reparsed: GovernanceConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(reparsed.readme, original.readme);
        assert_eq!(reparsed.docs, original.docs);
    }

    // 4.1 Auto-wires constitution when SpecKit detected + field unset.
    #[test]
    fn governance_auto_wires_constitution_when_speckit_detected() {
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path().join("repo");
        let specify = repo_root.join(".specify");
        let specs = specify.join("specs");
        let memory = specify.join("memory");
        fs::create_dir_all(&specs).unwrap();
        fs::create_dir_all(&memory).unwrap();
        let constitution = memory.join("constitution.md");
        fs::write(&constitution, "# Constitution\n").unwrap();

        write_repo_config(
            &repo_root,
            "[specs]\n\
             type = \"speckit\"\n\
             dir = \".specify/specs\"\n",
        );

        let config = load_config_from(&missing_global(&tmp), &repo_root).unwrap();
        assert_eq!(
            config.governance.constitution.as_deref(),
            Some(constitution.as_path())
        );
    }

    // 4.2 Explicit governance.constitution preserved unchanged.
    #[test]
    fn governance_explicit_constitution_preserved_over_auto_wiring() {
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path().join("repo");
        let specify = repo_root.join(".specify");
        let specs = specify.join("specs");
        let memory = specify.join("memory");
        fs::create_dir_all(&specs).unwrap();
        fs::create_dir_all(&memory).unwrap();
        fs::write(memory.join("constitution.md"), "# Constitution\n").unwrap();

        write_repo_config(
            &repo_root,
            "[specs]\n\
             type = \"speckit\"\n\
             dir = \".specify/specs\"\n\
             [governance]\n\
             constitution = \"docs/principles.md\"\n",
        );

        let config = load_config_from(&missing_global(&tmp), &repo_root).unwrap();
        assert_eq!(
            config.governance.constitution,
            Some(PathBuf::from("docs/principles.md"))
        );
    }

    // 4.3 Auto-wiring skipped for non-speckit backends.
    #[test]
    fn governance_auto_wiring_skipped_when_specs_type_is_openspec() {
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path().join("repo");
        let specify = repo_root.join(".specify");
        let memory = specify.join("memory");
        fs::create_dir_all(&memory).unwrap();
        fs::write(memory.join("constitution.md"), "# Constitution\n").unwrap();
        fs::create_dir_all(repo_root.join("specs")).unwrap();

        write_repo_config(
            &repo_root,
            "[specs]\n\
             type = \"openspec\"\n\
             dir = \"specs\"\n",
        );

        let config = load_config_from(&missing_global(&tmp), &repo_root).unwrap();
        assert!(config.governance.constitution.is_none());
    }

    // 4.4 Auto-wiring skipped when [specs] is absent entirely.
    #[test]
    fn governance_auto_wiring_skipped_when_specs_section_absent() {
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path().join("repo");
        let memory = repo_root.join(".specify").join("memory");
        fs::create_dir_all(&memory).unwrap();
        fs::write(memory.join("constitution.md"), "# Constitution\n").unwrap();
        fs::create_dir_all(repo_root.join(".git-paw")).unwrap();

        write_repo_config(&repo_root, "default_cli = \"claude\"\n");

        let config = load_config_from(&missing_global(&tmp), &repo_root).unwrap();
        assert!(config.governance.constitution.is_none());
    }

    // 4.5 SpecKit active but constitution.md absent → stays None, no error.
    #[test]
    fn governance_auto_wiring_skipped_when_constitution_md_absent() {
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path().join("repo");
        let specs = repo_root.join(".specify").join("specs");
        fs::create_dir_all(&specs).unwrap();
        // No memory/constitution.md.

        write_repo_config(
            &repo_root,
            "[specs]\n\
             type = \"speckit\"\n\
             dir = \".specify/specs\"\n",
        );

        let config = load_config_from(&missing_global(&tmp), &repo_root).unwrap();
        assert!(config.governance.constitution.is_none());
    }

    // 4.6 Explicit empty-string constitution preserved as Some("").
    #[test]
    fn governance_explicit_empty_string_constitution_suppresses_auto_wiring() {
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path().join("repo");
        let specify = repo_root.join(".specify");
        let specs = specify.join("specs");
        let memory = specify.join("memory");
        fs::create_dir_all(&specs).unwrap();
        fs::create_dir_all(&memory).unwrap();
        fs::write(memory.join("constitution.md"), "# Constitution\n").unwrap();

        write_repo_config(
            &repo_root,
            "[specs]\n\
             type = \"speckit\"\n\
             dir = \".specify/specs\"\n\
             [governance]\n\
             constitution = \"\"\n",
        );

        let config = load_config_from(&missing_global(&tmp), &repo_root).unwrap();
        assert_eq!(config.governance.constitution, Some(PathBuf::from("")));
    }

    // Merge: global and repo each contribute independent paths.
    #[test]
    fn governance_merge_fields_independently_across_global_and_repo() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("global").join("config.toml");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        write_file(&global_path, "[governance]\nadr = \"docs/adr\"\n");
        write_file(
            &repo_config_path(&repo_root),
            "[governance]\ndod = \"docs/dod.md\"\n",
        );

        let config = load_config_from(&global_path, &repo_root).unwrap();
        assert_eq!(config.governance.adr, Some(PathBuf::from("docs/adr")));
        assert_eq!(config.governance.dod, Some(PathBuf::from("docs/dod.md")));
    }

    // Merge precedence: repo wins per-field when both set.
    #[test]
    fn governance_merge_repo_wins_per_field_when_both_set() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("global").join("config.toml");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        write_file(&global_path, "[governance]\nadr = \"docs/global-adr\"\n");
        write_file(
            &repo_config_path(&repo_root),
            "[governance]\nadr = \"docs/repo-adr\"\n",
        );

        let config = load_config_from(&global_path, &repo_root).unwrap();
        assert_eq!(config.governance.adr, Some(PathBuf::from("docs/repo-adr")));
    }

    // load_repo_config also applies auto-wiring.
    #[test]
    fn governance_load_repo_config_also_auto_wires_constitution() {
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path().join("repo");
        let specify = repo_root.join(".specify");
        let specs = specify.join("specs");
        let memory = specify.join("memory");
        fs::create_dir_all(&specs).unwrap();
        fs::create_dir_all(&memory).unwrap();
        let constitution = memory.join("constitution.md");
        fs::write(&constitution, "# Constitution\n").unwrap();

        write_repo_config(
            &repo_root,
            "[specs]\n\
             type = \"speckit\"\n\
             dir = \".specify/specs\"\n",
        );

        let config = load_repo_config(&repo_root).unwrap();
        assert_eq!(
            config.governance.constitution.as_deref(),
            Some(constitution.as_path())
        );
    }

    // --- load_config user_config_path override (config-test-isolation) ---

    #[test]
    fn load_config_with_some_pins_global_to_override_path() {
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        let global_a = tmp.path().join("global-A.toml");
        let global_b = tmp.path().join("global-B.toml");
        write_file(&global_a, "[clis.cli-A]\ncommand = \"/bin/a\"\n");
        write_file(&global_b, "[clis.cli-B]\ncommand = \"/bin/b\"\n");

        let config = load_config(&repo_root, Some(&global_a)).unwrap();
        assert!(config.clis.contains_key("cli-A"));
        assert!(!config.clis.contains_key("cli-B"));
    }

    #[test]
    fn load_config_with_some_nonexistent_returns_defaults() {
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();
        let missing = tmp.path().join("does-not-exist.toml");

        let config = load_config(&repo_root, Some(&missing)).unwrap();
        assert_eq!(config, PawConfig::default());
    }

    // Note: a `load_config_with_none_reads_platform_default_global` test is
    // intentionally omitted. Asserting that `None` resolves to
    // `global_config_path()` would require either writing to the dev
    // machine's real `~/Library/Application Support/git-paw/config.toml`
    // (polluting it) or `serial_test` + env-var manipulation of `HOME` /
    // `XDG_CONFIG_HOME` (brittle, slows the suite). The `None` branch is
    // covered behaviourally by the 8 production call sites in `src/main.rs`
    // and the v0.4 test suite that continues to pass.

    #[test]
    fn load_config_override_does_not_affect_repo_resolution() {
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();
        write_file(&repo_config_path(&repo_root), "default_cli = \"claude\"\n");

        let global_path = tmp.path().join("global.toml");
        write_file(&global_path, "default_cli = \"gemini\"\n");

        let config = load_config(&repo_root, Some(&global_path)).unwrap();
        assert_eq!(config.default_cli.as_deref(), Some("claude"));
    }

    // Maps to scenario "GovernanceConfig has no gates field" from
    // governance-config. The struct does not enable `deny_unknown_fields`, so
    // unknown sections deserialise silently; this test asserts the round-trip
    // representation omits any `[governance.gates]` section and the loaded
    // governance config keeps only the documented document-pointer fields.
    // (test-coverage-v0-5-0 task 9.1)
    #[test]
    fn governance_config_rejects_gates_field() {
        let toml_input = "[governance]\ndod = \"docs/dod.md\"\n[governance.gates]\ndod = true\n";
        let cfg: PawConfig = toml::from_str(toml_input).expect("toml parse");
        let gov = cfg.governance;
        assert_eq!(gov.dod.as_deref(), Some(Path::new("docs/dod.md")));

        let round_trip = toml::to_string(&gov).expect("serialise gov");
        assert!(
            !round_trip.contains("gates"),
            "GovernanceConfig must not round-trip a `gates` field; got: {round_trip}"
        );
        assert!(
            !round_trip.contains("[governance.gates]"),
            "GovernanceConfig must not round-trip a `[governance.gates]` section; got: {round_trip}"
        );
    }

    // -----------------------------------------------------------------------
    // supervisor-pane-affordances: `[layout].border_affordances` config field
    // (spec requirement "border_affordances config field").
    // -----------------------------------------------------------------------

    /// Scenario: Default true applies all affordances — absent `[layout]`
    /// section resolves to `true`.
    #[test]
    fn border_affordances_defaults_to_true_when_layout_absent() {
        let cfg: PawConfig = toml::from_str("default_cli = \"claude\"\n").expect("toml parse");
        assert!(
            cfg.layout.is_none(),
            "no [layout] section should parse as None"
        );
        assert!(
            cfg.border_affordances_enabled(),
            "border affordances default to on when [layout] is absent"
        );
    }

    /// Scenario: Default true — `[layout]` present but `border_affordances`
    /// unset still resolves to `true`.
    #[test]
    fn border_affordances_defaults_to_true_when_field_unset() {
        let cfg: PawConfig = toml::from_str("[layout]\n").expect("toml parse");
        assert!(
            cfg.border_affordances_enabled(),
            "border affordances default to on when the field is unset"
        );
    }

    /// Scenario: Explicit false skips all affordances.
    #[test]
    fn border_affordances_explicit_false_resolves_off() {
        let cfg: PawConfig =
            toml::from_str("[layout]\nborder_affordances = false\n").expect("toml parse");
        assert_eq!(cfg.layout.as_ref().unwrap().border_affordances, Some(false));
        assert!(
            !cfg.border_affordances_enabled(),
            "explicit false must resolve to off"
        );
    }

    /// Scenario: Explicit true round-trips and resolves on.
    #[test]
    fn border_affordances_explicit_true_resolves_on() {
        let cfg: PawConfig =
            toml::from_str("[layout]\nborder_affordances = true\n").expect("toml parse");
        assert!(cfg.border_affordances_enabled());
    }

    /// Backward compatibility: a representative v0.5.0 config (no `[layout]`
    /// section at all) still parses and defaults affordances on.
    #[test]
    fn v0_5_0_config_without_layout_parses() {
        let v0_5_0 = "default_cli = \"claude\"\nmouse = true\n\n[broker]\nenabled = true\nport = 9119\n\n[supervisor]\nenabled = true\n";
        let cfg: PawConfig = toml::from_str(v0_5_0).expect("v0.5.0 config must still parse");
        assert!(cfg.layout.is_none());
        assert!(cfg.border_affordances_enabled());
    }

    /// `merged_with`: an overlay `[layout]` wins over the base layout.
    #[test]
    fn layout_overlay_wins_in_merge() {
        let base: PawConfig =
            toml::from_str("[layout]\nborder_affordances = true\n").expect("base");
        let overlay: PawConfig =
            toml::from_str("[layout]\nborder_affordances = false\n").expect("overlay");
        let merged = base.merged_with(&overlay);
        assert!(
            !merged.border_affordances_enabled(),
            "overlay [layout] must win in the merge"
        );
    }

    /// `merged_with`: an absent overlay `[layout]` preserves the base layout.
    #[test]
    fn layout_base_preserved_when_overlay_absent() {
        let base: PawConfig =
            toml::from_str("[layout]\nborder_affordances = false\n").expect("base");
        let overlay: PawConfig = toml::from_str("default_cli = \"claude\"\n").expect("overlay");
        let merged = base.merged_with(&overlay);
        assert!(
            !merged.border_affordances_enabled(),
            "base [layout] must survive when the overlay has none"
        );
    }

    // --- opsx role-gating config (opsx-role-gating 1.4) ---

    #[test]
    fn role_gating_defaults_to_warn_when_section_absent() {
        // A v0.5.0-shaped config with no `[opsx]` section still parses and
        // resolves to the default Warn mode.
        let config: PawConfig = toml::from_str("default_cli = \"claude\"\n").expect("parses");
        assert!(config.opsx.is_none());
        assert_eq!(config.role_gating_mode(), RoleGatingMode::Warn);
    }

    #[test]
    fn role_gating_section_present_but_field_absent_resolves_warn() {
        let config: PawConfig = toml::from_str("[opsx]\n").expect("parses");
        assert_eq!(config.role_gating_mode(), RoleGatingMode::Warn);
    }

    #[test]
    fn role_gating_explicit_warn() {
        let config: PawConfig = toml::from_str("[opsx]\nrole_gating = \"warn\"\n").expect("parses");
        assert_eq!(config.role_gating_mode(), RoleGatingMode::Warn);
    }

    #[test]
    fn role_gating_explicit_block() {
        let config: PawConfig =
            toml::from_str("[opsx]\nrole_gating = \"block\"\n").expect("parses");
        assert_eq!(config.role_gating_mode(), RoleGatingMode::Block);
    }

    #[test]
    fn role_gating_explicit_off() {
        let config: PawConfig = toml::from_str("[opsx]\nrole_gating = \"off\"\n").expect("parses");
        assert_eq!(config.role_gating_mode(), RoleGatingMode::Off);
    }

    #[test]
    fn role_gating_invalid_value_is_a_parse_error() {
        let err = toml::from_str::<PawConfig>("[opsx]\nrole_gating = \"loud\"\n").unwrap_err();
        assert!(
            err.to_string().contains("role_gating") || err.to_string().contains("variant"),
            "got: {err}"
        );
    }

    #[test]
    fn role_gating_mode_round_trips_through_toml() {
        let config = PawConfig {
            opsx: Some(OpsxConfig {
                role_gating: Some(RoleGatingMode::Block),
            }),
            ..Default::default()
        };
        let serialized = toml::to_string(&config).expect("serializes");
        assert!(
            serialized.contains("role_gating = \"block\""),
            "got: {serialized}"
        );
        let reparsed: PawConfig = toml::from_str(&serialized).expect("re-parses");
        assert_eq!(reparsed.role_gating_mode(), RoleGatingMode::Block);
    }

    #[test]
    fn opsx_section_merges_with_overlay_winning() {
        let base: PawConfig =
            toml::from_str("[opsx]\nrole_gating = \"warn\"\n").expect("base parses");
        let overlay: PawConfig =
            toml::from_str("[opsx]\nrole_gating = \"block\"\n").expect("overlay parses");
        let merged = base.merged_with(&overlay);
        assert_eq!(merged.role_gating_mode(), RoleGatingMode::Block);
    }

    #[test]
    fn opsx_section_base_preserved_when_overlay_absent() {
        let base: PawConfig =
            toml::from_str("[opsx]\nrole_gating = \"off\"\n").expect("base parses");
        let overlay: PawConfig = toml::from_str("default_cli = \"claude\"\n").expect("overlay");
        let merged = base.merged_with(&overlay);
        assert_eq!(merged.role_gating_mode(), RoleGatingMode::Off);
    }

    #[test]
    fn supervisor_auto_revert_defaults_false() {
        let config: PawConfig = toml::from_str("[supervisor]\nenabled = true\n").expect("parses");
        let sup = config.supervisor.expect("supervisor present");
        assert!(!sup.auto_revert(), "auto_revert defaults to false");
    }

    #[test]
    fn supervisor_auto_revert_explicit_true() {
        let config: PawConfig =
            toml::from_str("[supervisor]\nenabled = true\nauto_revert = true\n").expect("parses");
        let sup = config.supervisor.expect("supervisor present");
        assert!(sup.auto_revert());
    }

    // --- [supervisor.tell] (supervisor-tell change) ---

    #[test]
    fn tell_config_defaults_when_table_absent() {
        // A v0.5.0 `[supervisor]` with no `[supervisor.tell]` table loads the
        // documented defaults: feedback mode, 60s inventory max age.
        let config: PawConfig = toml::from_str("[supervisor]\nenabled = true\n").expect("parses");
        let sup = config.supervisor.expect("supervisor present");
        assert_eq!(sup.tell.mode, TellMode::Feedback);
        assert_eq!(sup.tell.inventory_max_age_seconds, 60);
        assert!(sup.tell.is_default());
    }

    #[test]
    fn tell_config_explicit_feedback_loads() {
        let config: PawConfig = toml::from_str(
            "[supervisor]\nenabled = true\n[supervisor.tell]\nmode = \"feedback\"\n",
        )
        .expect("parses");
        let sup = config.supervisor.expect("supervisor present");
        assert_eq!(sup.tell.mode, TellMode::Feedback);
        // mode set explicitly to the default still resolves to default values.
        assert_eq!(sup.tell.inventory_max_age_seconds, 60);
    }

    #[test]
    fn tell_config_explicit_send_keys_loads() {
        let config: PawConfig = toml::from_str(
            "[supervisor]\nenabled = true\n[supervisor.tell]\nmode = \"send-keys\"\ninventory_max_age_seconds = 15\n",
        )
        .expect("parses");
        let sup = config.supervisor.expect("supervisor present");
        assert_eq!(sup.tell.mode, TellMode::SendKeys);
        assert_eq!(sup.tell.inventory_max_age_seconds, 15);
        assert!(!sup.tell.is_default());
    }

    #[test]
    fn tell_config_rejects_unknown_mode() {
        let err = toml::from_str::<PawConfig>(
            "[supervisor]\nenabled = true\n[supervisor.tell]\nmode = \"shout\"\n",
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("shout") || err.to_string().contains("mode"),
            "unknown mode should be a parse error; got {err}"
        );
    }

    #[test]
    fn tell_config_all_default_table_round_trips_without_emitting_tell() {
        // An all-default tell table is skipped on serialize so v0.5.0 configs
        // stay byte-stable.
        let sup = SupervisorConfig {
            enabled: true,
            ..SupervisorConfig::default()
        };
        let config = PawConfig {
            supervisor: Some(sup),
            ..PawConfig::default()
        };
        let serialized = toml::to_string_pretty(&config).expect("serializes");
        assert!(
            !serialized.contains("[supervisor.tell]"),
            "all-default tell table must be omitted; got:\n{serialized}"
        );
        let reparsed: PawConfig = toml::from_str(&serialized).expect("re-parses");
        assert_eq!(config, reparsed);
    }

    // --- [mcp] configuration section (mcp-server-identity) ---

    // configuration delta — Scenario: Config with [mcp] name parses the field.
    #[test]
    fn mcp_name_parses_to_some() {
        let config: PawConfig = toml::from_str("[mcp]\nname = \"my-project\"\n").expect("parses");
        assert_eq!(config.mcp.name, Some("my-project".to_string()));
        assert_eq!(config.mcp_server_name(), "my-project");
    }

    // configuration delta — Scenario: Config without [mcp] section loads with
    // defaults (name = None) and does not error.
    #[test]
    fn mcp_section_absent_defaults_to_none() {
        let config: PawConfig = toml::from_str("default_cli = \"claude\"\n").expect("parses");
        assert_eq!(config.mcp, McpConfig::default());
        assert!(config.mcp.name.is_none());
        assert_eq!(config.mcp_server_name(), "git-paw");
    }

    // Backward compatibility: a representative pre-v0.7.0 config (no [mcp]
    // section) still parses unchanged.
    #[test]
    fn pre_existing_config_without_mcp_loads() {
        let prior = "default_cli = \"claude\"\nmouse = true\n\n[broker]\nenabled = true\nport = 9119\n\n[supervisor]\nenabled = true\n";
        let config: PawConfig = toml::from_str(prior).expect("prior config must still parse");
        assert_eq!(config.mcp, McpConfig::default());
    }

    // configuration delta — Scenario: MCP config survives round-trip
    // serialization.
    #[test]
    fn mcp_config_round_trips_through_toml() {
        let config = PawConfig {
            mcp: McpConfig {
                name: Some("my-project".to_string()),
            },
            ..PawConfig::default()
        };
        let serialized = toml::to_string(&config).expect("serializes");
        let reparsed: PawConfig = toml::from_str(&serialized).expect("re-parses");
        assert_eq!(reparsed.mcp, config.mcp);
    }

    // An all-default [mcp] table (name = None) is omitted on serialize so
    // pre-existing configs stay byte-stable.
    #[test]
    fn mcp_default_omits_name_on_serialize() {
        let config = PawConfig::default();
        let serialized = toml::to_string_pretty(&config).expect("serializes");
        assert!(
            !serialized.contains("name ="),
            "default [mcp] must not emit a name; got:\n{serialized}"
        );
        let reparsed: PawConfig = toml::from_str(&serialized).expect("re-parses");
        assert_eq!(config, reparsed);
    }

    // merged_with: a repo-level [mcp].name wins over the global one.
    #[test]
    fn mcp_overlay_name_wins_in_merge() {
        let base: PawConfig = toml::from_str("[mcp]\nname = \"global-name\"\n").expect("base");
        let overlay: PawConfig = toml::from_str("[mcp]\nname = \"repo-name\"\n").expect("overlay");
        let merged = base.merged_with(&overlay);
        assert_eq!(merged.mcp.name, Some("repo-name".to_string()));
    }

    // merged_with: an absent overlay [mcp].name preserves the base name.
    #[test]
    fn mcp_base_name_preserved_when_overlay_absent() {
        let base: PawConfig = toml::from_str("[mcp]\nname = \"global-name\"\n").expect("base");
        let overlay: PawConfig = toml::from_str("default_cli = \"claude\"\n").expect("overlay");
        let merged = base.merged_with(&overlay);
        assert_eq!(merged.mcp.name, Some("global-name".to_string()));
    }

    // --- worktree_placement (worktree-embedded-placement) ---

    #[test]
    fn worktree_placement_parses_child() {
        let cfg: PawConfig =
            toml::from_str("worktree_placement = \"child\"\n").expect("parse child");
        assert_eq!(cfg.worktree_placement, Some(WorktreePlacement::Child));
        assert_eq!(cfg.worktree_placement(), WorktreePlacement::Child);
    }

    #[test]
    fn worktree_placement_parses_sibling() {
        let cfg: PawConfig =
            toml::from_str("worktree_placement = \"sibling\"\n").expect("parse sibling");
        assert_eq!(cfg.worktree_placement, Some(WorktreePlacement::Sibling));
        assert_eq!(cfg.worktree_placement(), WorktreePlacement::Sibling);
    }

    #[test]
    fn worktree_placement_absent_defaults_to_sibling() {
        let cfg: PawConfig = toml::from_str("default_cli = \"claude\"\n").expect("parse");
        assert_eq!(cfg.worktree_placement, None);
        assert_eq!(cfg.worktree_placement(), WorktreePlacement::Sibling);
    }

    #[test]
    fn worktree_placement_repo_overrides_global() {
        let tmp = TempDir::new().unwrap();
        let global_path = tmp.path().join("global").join("config.toml");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        write_file(&global_path, "worktree_placement = \"sibling\"\n");
        write_file(
            &repo_config_path(&repo_root),
            "worktree_placement = \"child\"\n",
        );

        let config = load_config_from(&global_path, &repo_root).unwrap();
        assert_eq!(config.worktree_placement(), WorktreePlacement::Child);
    }

    #[test]
    fn worktree_placement_survives_round_trip() {
        let cfg = PawConfig {
            worktree_placement: Some(WorktreePlacement::Child),
            ..PawConfig::default()
        };
        let serialized = toml::to_string_pretty(&cfg).expect("serialize");
        let reparsed: PawConfig = toml::from_str(&serialized).expect("reparse");
        assert_eq!(reparsed.worktree_placement(), WorktreePlacement::Child);
    }

    #[test]
    fn worktree_placement_default_skipped_on_serialize() {
        // A default (absent) placement must not appear in serialized output so
        // pre-existing configs round-trip byte-stably.
        let cfg = PawConfig::default();
        let serialized = toml::to_string_pretty(&cfg).expect("serialize");
        assert!(
            !serialized.contains("worktree_placement"),
            "absent placement must not be serialized; got:\n{serialized}"
        );
    }

    #[test]
    fn preexisting_config_without_placement_loads_without_error() {
        // A v0.7.0 config (no worktree_placement field) must load and resolve
        // to sibling.
        let prior = "default_cli = \"claude\"\nmouse = true\n[broker]\nenabled = true\n";
        let cfg: PawConfig = toml::from_str(prior).expect("v0.7.0 config must load");
        assert_eq!(cfg.worktree_placement(), WorktreePlacement::Sibling);
    }
}

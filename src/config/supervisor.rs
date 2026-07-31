//! Supervisor-mode configuration: approval policy, auto-approval, conflict
//! detection, learnings aggregation, `/tell` routing, and the common
//! dev-command allowlist.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::CustomCli;

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

impl ApprovalLevel {
    /// The valid kebab-case level names — the accepted key set of
    /// [`CustomCli::approval_args`].
    pub const KEBAB_NAMES: [&'static str; 3] = ["manual", "auto", "full-auto"];

    /// Returns this level's kebab-case wire name (`"manual"`, `"auto"`,
    /// `"full-auto"`), matching the serde serialization and the key format
    /// of [`CustomCli::approval_args`].
    #[must_use]
    pub fn kebab_name(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Auto => "auto",
            Self::FullAuto => "full-auto",
        }
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
    /// The SUPERVISOR pane's own approval level, decoupled from
    /// [`Self::agent_approval`].
    ///
    /// `None` (the default — every pre-v0.11.0 config) makes the supervisor
    /// pane inherit `agent_approval`, preserving the pre-v0.11.0 behavior
    /// exactly. Setting `approval = "full-auto"` launches only the
    /// supervisor pane with its CLI's native skip-permissions flags while
    /// coding agents keep resolving from `agent_approval` (trusted-pane
    /// semantics: the supervisor runs in the repo root, not a worktree, so
    /// relaxing it is a deliberate, supervisor-scoped choice).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<ApprovalLevel>,
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
/// - `Conservative` — auto-approve the composed whitelist but strip
///   `git push` and `curl` entries AFTER composition, so the strip governs
///   built-ins, stack patterns, and configured extras alike.
/// - `Safe` — the built-in default; auto-approve the whole composed
///   whitelist (see [`AutoApproveConfig::effective_whitelist`]).
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
/// resolved option digit + `Enter` keystrokes when the command matches the
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
    /// Project-specific safe-command prefixes appended to the composed
    /// whitelist defaults — the stack-neutral built-ins from
    /// [`default_safe_commands()`](crate::supervisor::auto_approve::default_safe_commands)
    /// plus the resolved `[supervisor.common_dev_allowlist]` patterns (see
    /// [`Self::effective_whitelist`]).
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

    /// Returns the effective whitelist for this config, composed from three
    /// sources in order, de-duplicated:
    ///
    /// 1. the **stack-neutral built-ins**
    ///    ([`default_safe_commands()`](crate::supervisor::auto_approve::default_safe_commands));
    /// 2. the **resolved dev-allowlist patterns** —
    ///    [`effective_patterns`](crate::supervisor::dev_allowlist::effective_patterns)
    ///    over `[supervisor.common_dev_allowlist]` `stacks` + `extra`, so a
    ///    project's declared stack contributes its toolchain verbs (e.g. the
    ///    `rust` stack contributes `cargo test`) from the same declaration
    ///    that seeds the CLI allowlist;
    /// 3. the **configured extension** `[supervisor.auto_approve]
    ///    safe_commands`.
    ///
    /// The `Conservative` preset strip (`git push` and `curl` entries) is
    /// applied AFTER composition so it governs the whole composed set.
    /// `Off` and `Safe` return the composed union unchanged.
    #[must_use]
    pub fn effective_whitelist(&self, dev_allowlist: &CommonDevAllowlistConfig) -> Vec<String> {
        let mut out: Vec<String> = crate::supervisor::auto_approve::default_safe_commands()
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        for pat in crate::supervisor::dev_allowlist::effective_patterns(
            &dev_allowlist.stacks,
            &dev_allowlist.extra,
        ) {
            if !out.contains(&pat) {
                out.push(pat);
            }
        }
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

/// Returns the built-in CLI-specific permission flag for `cli` at the given
/// approval `level`, or an empty string if the combination has no mapped
/// flag.
///
/// This is the built-in-table step of [`resolve_approval_flags`]; prefer
/// that function when a loaded config (and thus its `[clis.<name>]`
/// overrides) is available. Rows verified against upstream CLI docs
/// 2026-07-15.
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
///     "--sandbox workspace-write",
/// );
/// assert_eq!(approval_flags("agy", &ApprovalLevel::FullAuto), "--dangerously-skip-permissions");
/// assert_eq!(approval_flags("claude", &ApprovalLevel::Manual), "");
/// assert_eq!(approval_flags("some-agent", &ApprovalLevel::FullAuto), "");
/// ```
#[must_use]
pub fn approval_flags(cli: &str, level: &ApprovalLevel) -> &'static str {
    match (cli, level) {
        ("claude" | "agy", ApprovalLevel::FullAuto) => "--dangerously-skip-permissions",
        ("codex", ApprovalLevel::FullAuto) => "--dangerously-bypass-approvals-and-sandbox",
        ("codex", ApprovalLevel::Auto) => "--sandbox workspace-write",
        ("qwen", ApprovalLevel::FullAuto) => "--yolo",
        _ => "",
    }
}

/// Resolves the permission flags for `cli` at `level`, consulting (in
/// order): the per-CLI `[clis.<name>].approval_args` override, the built-in
/// [`approval_flags`] table, then `""` (no flags).
///
/// The override is the seam for custom or variant CLIs (e.g. a claude-oss
/// entry launched via `CLAUDE_CONFIG_DIR`) to get native flags without a
/// built-in table row. Resolution is deterministic: the same
/// `(cli, level, clis)` triple always yields the same value.
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use git_paw::config::{resolve_approval_flags, ApprovalLevel, CustomCli};
///
/// let mut clis: HashMap<String, CustomCli> = HashMap::new();
/// // No override: the built-in table answers.
/// assert_eq!(
///     resolve_approval_flags("claude", &ApprovalLevel::FullAuto, &clis),
///     "--dangerously-skip-permissions",
/// );
/// // An override wins over the built-in row.
/// clis.insert(
///     "claude".to_string(),
///     CustomCli {
///         command: "claude".to_string(),
///         display_name: None,
///         submit_delay_ms: None,
///         settings_path: None,
///         approval_args: HashMap::from([(
///             "full-auto".to_string(),
///             "--my-custom-flag".to_string(),
///         )]),
///     },
/// );
/// assert_eq!(
///     resolve_approval_flags("claude", &ApprovalLevel::FullAuto, &clis),
///     "--my-custom-flag",
/// );
/// ```
#[must_use]
pub fn resolve_approval_flags<S: std::hash::BuildHasher>(
    cli: &str,
    level: &ApprovalLevel,
    clis: &HashMap<String, CustomCli, S>,
) -> String {
    if let Some(args) = clis
        .get(cli)
        .and_then(|c| c.approval_args.get(level.kebab_name()))
    {
        return args.clone();
    }
    approval_flags(cli, level).to_string()
}

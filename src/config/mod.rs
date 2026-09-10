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

mod broker;
mod cli;
mod dashboard;
mod layout;
mod prompt_profile;
mod specs;
mod supervisor;

#[cfg(test)]
mod tests;

pub use broker::{BrokerConfig, WatcherConfig};
pub use cli::{
    CustomCli, Preset, add_custom_cli, add_custom_cli_to, remove_custom_cli,
    remove_custom_cli_from, resolve_submit_delay_ms,
};
pub use dashboard::{BrokerLogConfig, DashboardConfig};
pub use layout::LayoutConfig;
pub use prompt_profile::{
    CliPromptProfile, CliPromptProfileOverride, claude_prompt_profile, resolve_prompt_profile,
};
pub use specs::SpecsConfig;
pub use supervisor::{
    ApprovalLevel, ApprovalLevelPreset, AutoApproveConfig, BrokerPublish, CommonDevAllowlistConfig,
    ConflictConfig, CorrectionConfig, LearningsConfig, OnExhausted, SupervisorConfig, TellConfig,
    TellMode, approval_flags, resolve_approval_flags,
};

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

/// Env-file provisioning for agent worktrees (`[worktree.env]`).
///
/// `copy` names files, relative to the repository root, that git-paw copies
/// verbatim into each new worktree after the worktree is created and before the
/// agent CLI launches. The propagated file is an independent copy, never a
/// symlink, so a worktree's edits never reach the source or a sibling worktree.
///
/// git-paw supplies no default filename — an absent table (or an empty `copy`)
/// copies nothing.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreeEnvConfig {
    /// Repository-root-relative paths to copy into each new worktree.
    ///
    /// An entry with no matching source file is skipped with a warning; the
    /// worktree creation still succeeds. Absolute paths and paths escaping the
    /// repository root (`..`) are rejected with a warning.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copy: Vec<String>,
}

/// Per-worktree port allocation (`[worktree.ports]`).
///
/// Each worktree is assigned a port *block* derived from its runtime slot:
/// `base + slot * stride`. The Nth entry of `vars` receives
/// `base + slot * stride + N`, written into the worktree's generated
/// `.env.local`.
///
/// Assignment is deterministic, not probed — git-paw does not scan the host for
/// free ports, so an allocated port can still be occupied by an unrelated
/// process. Resolve that by changing `base` or `stride`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreePortsConfig {
    /// First port of the first worktree's block.
    #[serde(default)]
    pub base: u16,
    /// Block size per worktree. SHOULD be at least `vars.len()`, otherwise
    /// adjacent worktrees' blocks would overlap; see
    /// [`WorktreePortsConfig::effective_stride`].
    #[serde(default)]
    pub stride: u16,
    /// Environment-variable names that receive sequential ports within the
    /// block. Empty (the default) writes no port lines — git-paw supplies no
    /// default port variables.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vars: Vec<String>,
}

impl WorktreePortsConfig {
    /// Returns the stride actually used for block arithmetic: the configured
    /// `stride` clamped up to `vars.len()`.
    ///
    /// A `stride` smaller than the number of `vars` would make adjacent
    /// worktrees' blocks overlap, handing two live agents the same port.
    /// Clamping up keeps blocks disjoint; [`Self::stride_warning`] surfaces the
    /// misconfiguration to the operator.
    #[must_use]
    pub fn effective_stride(&self) -> u16 {
        let needed = u16::try_from(self.vars.len()).unwrap_or(u16::MAX);
        self.stride.max(needed)
    }

    /// Returns a human-readable warning when `stride` is smaller than the
    /// number of `vars` (blocks would overlap), otherwise `None`.
    #[must_use]
    pub fn stride_warning(&self) -> Option<String> {
        let needed = u16::try_from(self.vars.len()).unwrap_or(u16::MAX);
        (self.stride < needed).then(|| {
            format!(
                "[worktree.ports] stride = {} is smaller than the {} declared vars; \
                 adjacent worktree port blocks would overlap. Using stride = {needed}.",
                self.stride,
                self.vars.len()
            )
        })
    }
}

/// Per-worktree lifecycle hooks (`[worktree.hooks]`).
///
/// Both commands are consumer-authored and optional. `on_create` runs after the
/// env copies and port allocation, before the agent's CLI launches; `on_remove`
/// runs when the worktree is removed or purged, before the directory is deleted.
/// Each is executed with the worktree as its working directory, and
/// `{worktree_id}` / `{worktree_path}` are substituted first.
///
/// The hooks exist because the remaining runtime collision — a shared database,
/// container, or tenant — is irreducibly stack-specific. git-paw ships the
/// mechanism (run a command, capture its `KEY=value` stdout, substitute the
/// worktree's identity) and never a driver or a policy: no database, stack, or
/// default command is assumed.
///
/// The commands run with the operator's own privileges, exactly like
/// [`CustomCli::command`] and the `[supervisor]` commands. git-paw never builds a
/// hook command from broker, network, or agent input — only from this config.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreeHooksConfig {
    /// Command run in a freshly provisioned worktree, before the agent starts.
    ///
    /// Its stdout is parsed as `KEY=value` lines and merged into the same
    /// `.env.local` managed block port allocation writes, so a hook that
    /// provisions an isolated resource can hand its connection details to the
    /// agent. A non-zero exit fails that worktree's provisioning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_create: Option<String>,
    /// Command run at `git paw remove` / `git paw purge`, before the worktree
    /// directory is deleted, so per-worktree resources are torn down.
    ///
    /// A non-zero exit is surfaced as a warning and does NOT block removal — a
    /// failed teardown must never strand a worktree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_remove: Option<String>,
}

/// Per-worktree runtime provisioning (`[worktree]`).
///
/// Every sub-table is optional and opt-in: with none present git-paw creates
/// worktrees exactly as it did before this table existed — copying no files,
/// generating no `.env.local`, and running no hook.
///
/// This table is independent of the top-level `worktree_placement` field, which
/// is deliberately left where it is so pre-existing configs keep parsing
/// unchanged.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreeConfig {
    /// Env files copied into each new worktree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<WorktreeEnvConfig>,
    /// Per-worktree port block allocation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ports: Option<WorktreePortsConfig>,
    /// Consumer commands run at worktree create and remove.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<WorktreeHooksConfig>,
}

impl WorktreeConfig {
    /// Returns `true` when no sub-table is configured, i.e. provisioning is a
    /// no-op and worktree creation behaves exactly as in prior versions.
    #[must_use]
    pub fn is_inert(&self) -> bool {
        self.env.is_none() && self.ports.is_none() && self.hooks.is_none()
    }

    /// Returns the configured `on_create` command, or `None` when no hook is
    /// declared.
    #[must_use]
    pub fn on_create_hook(&self) -> Option<&str> {
        self.hooks.as_ref()?.on_create.as_deref()
    }

    /// Returns the configured `on_remove` command, or `None` when no hook is
    /// declared.
    #[must_use]
    pub fn on_remove_hook(&self) -> Option<&str> {
        self.hooks.as_ref()?.on_remove.as_deref()
    }
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

    /// Base URL of the documentation site the bundled `docs-fetch` helper
    /// targets for discovery (`llms.txt`) and page retrieval.
    ///
    /// Absent (every config without the field) resolves to
    /// [`DEFAULT_DOCS_BASE_URL`] via [`PawConfig::docs_base_url`] — git-paw's
    /// published documentation site. A fork or mirror sets this to retarget
    /// the helper. Serialised with `skip_serializing_if` so the default never
    /// appears in round-tripped configs, keeping pre-existing configs
    /// byte-stable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs_base_url: Option<String>,

    /// Per-worktree runtime provisioning (`[worktree.env]` /
    /// `[worktree.ports]`).
    ///
    /// Absent (every config written before this table existed) loads as `None`,
    /// which [`PawConfig::worktree_runtime`] resolves to an inert
    /// [`WorktreeConfig::default`] — no files copied, no `.env.local`
    /// generated. Declared last in the struct so the TOML serializer emits this
    /// table after every scalar key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<WorktreeConfig>,
}

/// Default documentation site the `docs-fetch` helper targets when
/// `docs_base_url` is unset. Kept in sync with the fallback baked into
/// `assets/scripts/docs-fetch.sh`.
pub const DEFAULT_DOCS_BASE_URL: &str = "https://bearicorn.github.io/git-paw";

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
            docs_base_url: overlay
                .docs_base_url
                .clone()
                .or_else(|| self.docs_base_url.clone()),
            worktree: overlay.worktree.clone().or_else(|| self.worktree.clone()),
        }
    }

    /// Resolves the effective per-worktree runtime provisioning config,
    /// returning an inert [`WorktreeConfig::default`] when `[worktree]` is
    /// absent.
    #[must_use]
    pub fn worktree_runtime(&self) -> WorktreeConfig {
        self.worktree.clone().unwrap_or_default()
    }

    /// Resolves the effective docs base URL for the `docs-fetch` helper,
    /// defaulting to [`DEFAULT_DOCS_BASE_URL`] when `docs_base_url` is absent.
    #[must_use]
    pub fn docs_base_url(&self) -> &str {
        self.docs_base_url
            .as_deref()
            .unwrap_or(DEFAULT_DOCS_BASE_URL)
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
pub(crate) fn load_config_file(path: &Path) -> Result<Option<PawConfig>, PawError> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let config: PawConfig = toml::from_str(&contents)
                .map_err(|e| PawError::ConfigError(format!("{}: {e}", path.display())))?;
            validate_approval_args(&config, path)?;
            for warning in worktree_runtime_warnings(&config) {
                eprintln!("warning: {}: {warning}", path.display());
            }
            Ok(Some(config))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(PawError::ConfigError(format!("{}: {e}", path.display()))),
    }
}

/// Rejects `[clis.<name>].approval_args` maps whose keys are not one of the
/// three kebab-case approval-level names.
///
/// A typo'd level key would otherwise be silently ignored at flag
/// resolution, downgrading the security posture the operator explicitly
/// requested — so an unknown key fails the load, naming the bad key.
fn validate_approval_args(config: &PawConfig, path: &Path) -> Result<(), PawError> {
    for (name, cli) in &config.clis {
        for key in cli.approval_args.keys() {
            if !ApprovalLevel::KEBAB_NAMES.contains(&key.as_str()) {
                return Err(PawError::ConfigError(format!(
                    "{}: [clis.{name}] approval_args has invalid level key '{key}' \
                     (expected one of: \"manual\", \"auto\", \"full-auto\")",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

/// Collects non-fatal warnings about the `[worktree]` runtime-provisioning
/// tables.
///
/// A misconfiguration here degrades gracefully (see
/// [`WorktreePortsConfig::effective_stride`]) rather than failing the load, so
/// these are surfaced as warnings on the config-validation path instead of
/// errors — a stale `stride` should never stop a session from starting.
#[must_use]
pub fn worktree_runtime_warnings(config: &PawConfig) -> Vec<String> {
    config
        .worktree
        .as_ref()
        .and_then(|w| w.ports.as_ref())
        .and_then(WorktreePortsConfig::stride_warning)
        .into_iter()
        .collect()
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
pub(crate) fn save_config_to(path: &Path, config: &PawConfig) -> Result<(), PawError> {
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

# Documentation site the bundled docs-fetch helper consults on demand.
# Defaults to git-paw's published docs; set this to point a fork or mirror
# at its own site.
# docs_base_url = "https://bearicorn.github.io/git-paw"

# Where agent worktrees are created relative to the repository.
#   "child"   — inside the repo at .git-paw/worktrees/<branch-slug> (contained
#               layout; enables a project-scoped permission grant). New repos
#               default to this. Requires .git-paw/worktrees/ in .gitignore
#               (git paw init seeds it).
#   "sibling" — beside the repo at ../<project>-<branch-slug> (v0.7.0 layout).
# Omit the field to default to "sibling".
worktree_placement = "child"

# Per-worktree runtime isolation. Every table is opt-in: omit them and worktree
# creation copies nothing, generates no .env.local, and runs no command.
#
# [worktree.env] copy — files copied verbatim from the repo root into each new
# worktree, after it is created and before the agent CLI starts. The copy is
# independent (never a symlink), so one agent's edits never reach the source or
# a sibling worktree. A declared file that does not exist is skipped with a
# warning.
# [worktree.env]
# copy = [".env", ".env.development"]
#
# [worktree.ports] — each worktree gets its own port block starting at
# base + slot * stride; the Nth listed var receives base + slot * stride + N.
# The assignments are written into a git-paw-managed block in the worktree's
# .env.local, so a copied .env is never modified. Keep stride >= the number of
# vars or adjacent worktrees' blocks would overlap. Ports are deterministic,
# not probed: git-paw does not scan for free ports, so change base/stride if a
# block clashes with something else on the host.
# [worktree.ports]
# base   = 3000
# stride = 10
# vars   = ["PORT", "VITE_PORT"]
#
# [worktree.hooks] — your own commands, run per worktree, for the isolation
# git-paw cannot do generically (a database branch, a container, a tenant).
# Each runs through /bin/sh with the worktree as its working directory, with
# {worktree_id} (the branch-derived slug) and {worktree_path} (absolute)
# substituted. on_create runs after the copies and ports above and before the
# agent CLI starts; every KEY=value line it prints on stdout is merged into the
# same managed block of .env.local, so a script that provisions a database can
# hand back its DATABASE_URL. Other output is ignored. on_remove runs at
# `git paw remove` / `git paw purge` before the worktree is deleted.
# A non-zero on_create fails that worktree; a non-zero on_remove only warns and
# the removal continues, so write teardown to be idempotent.
# These run arbitrary commands with your privileges — same trust model as
# [clis.*].command.
# [worktree.hooks]
# on_create = "scripts/paw-db-branch.sh {worktree_id}"
# on_remove = "scripts/paw-db-drop.sh {worktree_id}"

# Dashboard message log configuration.
# [dashboard]
# show_message_log = false
#
# Broker log panel — a scrolling, type-filterable view of recent broker
# messages. An absent table loads these defaults.
# [dashboard.broker_log]
# max_messages = 500
# default_visible = true
# height_lines = 20

# Pane affordances for git-paw-managed (paw-*) sessions: heavy pane borders,
# a per-pane label strip, and dim-inactive / cyan-bold-active borders.
# Set false to inherit your own tmux styling (default: true).
# [layout]
# border_affordances = true

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
#
# Filesystem watcher. After a `committed` event, a file write within this
# TTL re-publishes `working` so the dashboard reflects continued activity.
# 0 disables; non-zero values below 5 clamp to 5 (default: 60).
# [broker.watcher]
# republish_working_ttl_seconds = 60

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
# approval = "manual"  # supervisor pane's own level: "manual" | "auto" | "full-auto"
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
#
# Auto-approve known-safe permission prompts in stalled agent panes so the
# supervisor need not dismiss each by hand. The whitelist is composed from
# stack-neutral built-ins plus the [supervisor.common_dev_allowlist] stacks /
# extra patterns below (declare your toolchain there — e.g. stacks = ["rust"]
# makes `cargo test` auto-approve). `approval_level` is a coarse preset
# ("off" | "conservative" | "safe"); `safe_commands` are extra prefixes
# appended to the composed whitelist; `stall_threshold_seconds` is the
# last_seen lag before a stalled pane is polled (minimum 5);
# `approve_worktree_writes` auto-approves file writes whose target resolves
# inside the agent's worktree.
# [supervisor.auto_approve]
# enabled = true
# safe_commands = ["just lint", "just test"]
# stall_threshold_seconds = 30
# approval_level = "safe"
# approve_worktree_writes = true
#
# Learnings subsystem flush cadence. The master switch is the `learnings`
# field on [supervisor] above; this sub-table only tunes the flush. Set
# broker_publish = "force_off" to keep file-only output even when the broker
# is running ("auto" follows [broker] enabled).
# [supervisor.learnings_config]
# flush_interval_seconds = 60
# broker_publish = "auto"
#
# Correction loop. When a supervisor gate fails, the --unattended drive loop
# re-engages that worker's pane with the gate's feedback instead of parking it
# in an inbox the blocked worker is not polling. Recommended for unattended
# runs — but `auto_loopback` defaults to FALSE so an existing setup keeps its
# behaviour after an upgrade; set it true to opt in. Cycles are bounded by
# max_cycles, after which on_exhausted applies ("escalate" flags the branch as
# unrecoverable, "abandon" marks it failed); escalate_after_cycles emits one
# early heads-up before the budget is spent (no early flag when it is >=
# max_cycles).
# [supervisor.correction]
# auto_loopback = true
# max_cycles = 5
# on_exhausted = "escalate"  # one of: "escalate", "abandon"
# escalate_after_cycles = 3

# Common dev-command allowlist. The universal preset (non-destructive git
# verbs + find / grep / sed -n) is always composed into the supervisor's
# auto-approve whitelist and its rendered skill guidance, so agents do not
# hit a permission prompt for each variant. Opt into a toolchain's curated
# patterns with stacks (named presets: rust / node / python / go); extend
# with project-specific prefixes via extra.
# [supervisor.common_dev_allowlist]
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

# Pointers to your project's existing governance docs so the supervisor can
# read them as context. All fields optional — list only the docs you have.
# [governance]
# adr = "docs/adr"                                  # directory of ADR files
# test_strategy = "docs/test-strategy.md"           # single Markdown file
# security = "docs/security-checklist.md"           # single Markdown file
# dod = "docs/definition-of-done.md"                # single Markdown file
# constitution = ".specify/memory/constitution.md"  # single Markdown file
# readme = "README.md"                              # repository README
# docs = "docs/src"                                 # documentation root directory

# MCP server identity for `git paw mcp` (advertised as serverInfo.name in the
# initialize handshake). Set to distinguish multiple repos each running a
# server; defaults to "git-paw".
# [mcp]
# name = "git-paw"

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

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
}

/// A named preset defining branches and a CLI to use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Preset {
    /// Branches to open in this preset.
    pub branches: Vec<String>,
    /// CLI to use for all branches in this preset.
    pub cli: String,
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
    /// Whether to show the broker messages panel in the dashboard.
    #[serde(default)]
    pub show_message_log: bool,
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
}

impl Default for AutoApproveConfig {
    fn default() -> Self {
        Self {
            enabled: Self::default_enabled(),
            safe_commands: Vec::new(),
            stall_threshold_seconds: Self::default_stall_threshold_seconds(),
            approval_level: ApprovalLevelPreset::Safe,
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
}

impl Default for BrokerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 9119,
            bind: "127.0.0.1".to_string(),
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
        }
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
pub fn load_repo_config(repo_root: &Path) -> Result<PawConfig, PawError> {
    Ok(load_config_file(&repo_config_path(repo_root))?.unwrap_or_default())
}

/// Loads the merged configuration for a repository.
///
/// Reads the global config and the per-repo config, merging them with
/// repo settings taking precedence. Returns defaults if neither file exists.
pub fn load_config(repo_root: &Path) -> Result<PawConfig, PawError> {
    let global_path = global_config_path()?;
    load_config_from(&global_path, repo_root)
}

/// Loads merged config from an explicit global path and repo root.
pub fn load_config_from(global_path: &Path, repo_root: &Path) -> Result<PawConfig, PawError> {
    let global = load_config_file(global_path)?.unwrap_or_default();
    let repo = load_config_file(&repo_config_path(repo_root))?.unwrap_or_default();
    Ok(global.merged_with(&repo))
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
        },
    );

    save_config_to(config_path, &config)
}

/// Returns a default `config.toml` string with sensible defaults and
/// commented-out v0.2.0 fields for discoverability.
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
# agent CLI, enforcing approval policy and optionally running a test
# command after each agent completes.
# [supervisor]
# enabled = true
# cli = "claude"
# test_command = "just check"
# agent_approval = "auto"  # one of: "manual", "auto", "full-auto"

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
                agent_approval: ApprovalLevel::FullAuto,
                auto_approve: None,
            }),
            ..Default::default()
        };

        save_config_to(&config_path, &original).unwrap();
        let loaded = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(loaded.supervisor, original.supervisor);
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
            }),
            ..Default::default()
        };

        save_config_to(&config_path, &original).unwrap();
        let loaded = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(loaded.dashboard, original.dashboard);
        assert!(loaded.dashboard.unwrap().show_message_log);
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
}

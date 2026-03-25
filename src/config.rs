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

/// Top-level git-paw configuration.
///
/// All fields are optional — absent config files produce empty defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PawConfig {
    /// Default CLI to use when none is specified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_cli: Option<String>,

    /// Whether to enable tmux mouse mode for sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mouse: Option<bool>,

    /// Custom CLI definitions keyed by name.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub clis: HashMap<String, CustomCli>,

    /// Named presets keyed by name.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub presets: HashMap<String, Preset>,
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
            mouse: overlay.mouse.or(self.mouse),
            clis,
            presets,
        }
    }

    /// Returns a preset by name, if it exists.
    pub fn get_preset(&self, name: &str) -> Option<&Preset> {
        self.presets.get(name)
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

[clis.my-agent]
command = "/usr/local/bin/my-agent"
display_name = "My Agent"

[clis.local-llm]
command = "ollama-code"

[presets.backend]
branches = ["feature/api", "fix/db"]
cli = "claude"
"#,
        );

        let config = load_config_file(&path).unwrap().unwrap();
        assert_eq!(config.default_cli.as_deref(), Some("claude"));
        assert_eq!(config.mouse, Some(false));
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

    #[test]
    fn config_survives_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let original = PawConfig {
            default_cli: Some("claude".into()),
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
        };

        save_config_to(&config_path, &original).unwrap();
        let loaded = load_config_file(&config_path).unwrap().unwrap();
        assert_eq!(original, loaded);
    }
}

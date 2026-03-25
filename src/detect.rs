//! AI CLI detection.
//!
//! Scans PATH for known AI coding CLI binaries and merges with custom CLIs
//! from the user's configuration. Provides the combined list for interactive
//! selection or direct use.

use std::fmt;
use std::path::{Path, PathBuf};

/// Known AI CLI binary names to scan for on PATH.
const KNOWN_CLIS: &[&str] = &["claude", "codex", "gemini", "aider", "vibe", "qwen", "amp"];

/// How a CLI was discovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliSource {
    /// Auto-detected on PATH.
    Detected,
    /// Defined in user configuration.
    Custom,
}

impl fmt::Display for CliSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Detected => write!(f, "detected"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

/// Information about an available AI CLI.
#[derive(Debug, Clone)]
pub struct CliInfo {
    /// Human-readable name for display in prompts and status output.
    pub display_name: String,
    /// The binary name (used for deduplication and identification).
    pub binary_name: String,
    /// Absolute path to the binary.
    pub path: PathBuf,
    /// How this CLI was discovered.
    pub source: CliSource,
}

/// A custom CLI definition provided by the user's configuration.
///
/// This is the input type that the config module will supply. Defined here
/// so that `detect.rs` has no dependency on the config module's types.
#[derive(Debug, Clone)]
pub struct CustomCliDef {
    /// Identifier name (e.g., `"my-agent"`).
    pub name: String,
    /// Command or path to the binary (e.g., `"/usr/local/bin/my-agent"` or `"my-agent"`).
    pub command: String,
    /// Optional human-readable display name. Defaults to `name` if not set.
    pub display_name: Option<String>,
}

/// Derives a display name from a binary name by capitalising the first letter.
fn derive_display_name(binary_name: &str) -> String {
    let mut chars = binary_name.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// Resolves a command string to an absolute path.
///
/// If the command is an absolute path and the file exists, returns it directly.
/// Otherwise attempts a PATH lookup via `which`.
fn resolve_command(command: &str) -> Option<PathBuf> {
    let path = Path::new(command);
    if path.is_absolute() && path.exists() {
        return Some(path.to_path_buf());
    }
    which::which(command).ok()
}

/// Scans PATH for known AI CLI binaries.
///
/// Returns a [`CliInfo`] for each known binary found on PATH.
pub fn detect_known_clis() -> Vec<CliInfo> {
    KNOWN_CLIS
        .iter()
        .filter_map(|&name| {
            which::which(name).ok().map(|path| CliInfo {
                display_name: derive_display_name(name),
                binary_name: name.to_string(),
                path,
                source: CliSource::Detected,
            })
        })
        .collect()
}

/// Resolves custom CLI definitions to [`CliInfo`] entries.
///
/// For each custom CLI, the command is resolved as follows:
/// 1. If the command looks like an absolute path and the file exists, use it directly.
/// 2. Otherwise, look it up on PATH via `which`.
///
/// CLIs whose binary cannot be found are excluded with a warning on stderr.
pub fn resolve_custom_clis(custom: &[CustomCliDef]) -> Vec<CliInfo> {
    custom
        .iter()
        .filter_map(|def| {
            if let Some(p) = resolve_command(&def.command) {
                let display = def
                    .display_name
                    .clone()
                    .unwrap_or_else(|| derive_display_name(&def.name));
                Some(CliInfo {
                    display_name: display,
                    binary_name: def.name.clone(),
                    path: p,
                    source: CliSource::Custom,
                })
            } else {
                eprintln!(
                    "warning: custom CLI '{}' not found at '{}', skipping",
                    def.name, def.command
                );
                None
            }
        })
        .collect()
}

/// Detects all available AI CLIs by combining auto-detected and custom CLIs.
///
/// Custom CLIs override auto-detected ones when they share the same `binary_name`.
/// The returned list is sorted by display name.
pub fn detect_clis(custom: &[CustomCliDef]) -> Vec<CliInfo> {
    let detected = detect_known_clis();
    let custom_resolved = resolve_custom_clis(custom);

    let mut by_name = std::collections::HashMap::new();
    for cli in detected {
        by_name.insert(cli.binary_name.clone(), cli);
    }
    for cli in custom_resolved {
        by_name.insert(cli.binary_name.clone(), cli);
    }

    let mut result: Vec<CliInfo> = by_name.into_values().collect();
    result.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    /// Creates a temp directory with fake executable binaries for the given names.
    /// Returns the `TempDir` (must be held alive) and its path.
    fn fake_path_with_binaries(names: &[&str]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        for name in names {
            let bin_path = dir.path().join(name);
            fs::write(&bin_path, "#!/bin/sh\n").expect("failed to write fake binary");
            fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755))
                .expect("failed to set permissions");
        }
        let path = dir.path().to_path_buf();
        (dir, path)
    }

    /// Runs a closure with PATH set to only the given directory.
    /// Restores the original PATH afterward. Tests using this must run
    /// serially (via `serial_test`) to avoid races.
    fn with_path<F, R>(path_dir: &Path, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let original = std::env::var("PATH").unwrap_or_default();
        // SAFETY: tests using this helper are serialized via #[serial_test::serial]
        // so no concurrent threads are reading PATH.
        unsafe {
            std::env::set_var("PATH", path_dir);
        }
        let result = f();
        unsafe {
            std::env::set_var("PATH", original);
        }
        result
    }

    // --- Acceptance: Auto-detects all 8 known CLIs when present ---

    #[test]
    #[serial_test::serial]
    fn all_known_clis_detected_when_present() {
        let all_names = ["claude", "codex", "gemini", "aider", "vibe", "qwen", "amp"];
        let (_dir, path) = fake_path_with_binaries(&all_names);

        let result = with_path(&path, detect_known_clis);

        assert_eq!(result.len(), all_names.len());
        for name in &all_names {
            assert!(
                result.iter().any(|c| c.binary_name == *name),
                "expected '{name}' to be detected"
            );
        }
        for cli in &result {
            assert_eq!(cli.source, CliSource::Detected);
            assert!(!cli.display_name.is_empty());
            assert!(cli.path.exists());
        }
    }

    // --- Acceptance: Returns empty vec when none found ---

    #[test]
    #[serial_test::serial]
    fn returns_empty_when_no_known_clis_on_path() {
        let (_dir, path) = fake_path_with_binaries(&[]);

        let result = with_path(&path, detect_known_clis);

        assert!(result.is_empty());
    }

    // --- Acceptance: Partial detection ---

    #[test]
    #[serial_test::serial]
    fn detects_subset_of_known_clis() {
        let (_dir, path) = fake_path_with_binaries(&["claude", "aider"]);

        let result = with_path(&path, detect_known_clis);

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|c| c.binary_name == "claude"));
        assert!(result.iter().any(|c| c.binary_name == "aider"));
    }

    // --- Acceptance: Loads custom CLIs from config and merges with detected ---

    #[test]
    #[serial_test::serial]
    fn custom_clis_merged_with_detected() {
        let (_dir, path) = fake_path_with_binaries(&["claude", "my-agent"]);
        let custom = vec![CustomCliDef {
            name: "my-agent".to_string(),
            command: "my-agent".to_string(),
            display_name: Some("My Agent".to_string()),
        }];

        let result = with_path(&path, || detect_clis(&custom));

        assert_eq!(result.len(), 2);
        assert!(
            result
                .iter()
                .any(|c| c.binary_name == "claude" && c.source == CliSource::Detected)
        );
        assert!(
            result
                .iter()
                .any(|c| c.binary_name == "my-agent" && c.source == CliSource::Custom)
        );
    }

    // --- Acceptance: Excludes custom CLIs with missing binaries (with warning) ---

    #[test]
    #[serial_test::serial]
    fn custom_cli_excluded_when_binary_missing() {
        let (_dir, path) = fake_path_with_binaries(&[]);
        let custom = vec![CustomCliDef {
            name: "ghost-agent".to_string(),
            command: "/nonexistent/ghost-agent".to_string(),
            display_name: None,
        }];

        let result = with_path(&path, || detect_clis(&custom));

        assert!(result.is_empty());
    }

    // --- Acceptance: Deduplicates — custom wins over detected ---

    #[test]
    #[serial_test::serial]
    fn custom_cli_overrides_detected_with_same_binary_name() {
        let (_dir, path) = fake_path_with_binaries(&["claude"]);
        let custom = vec![CustomCliDef {
            name: "claude".to_string(),
            command: "claude".to_string(),
            display_name: Some("My Custom Claude".to_string()),
        }];

        let result = with_path(&path, || detect_clis(&custom));

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].binary_name, "claude");
        assert_eq!(result[0].source, CliSource::Custom);
        assert_eq!(result[0].display_name, "My Custom Claude");
    }

    // --- Acceptance: Each result has display_name, binary_name, path, source ---

    #[test]
    #[serial_test::serial]
    fn detected_cli_has_all_fields() {
        let (_dir, path) = fake_path_with_binaries(&["gemini"]);

        let result = with_path(&path, detect_known_clis);

        assert_eq!(result.len(), 1);
        let cli = &result[0];
        assert_eq!(cli.binary_name, "gemini");
        assert_eq!(cli.display_name, "Gemini");
        assert!(cli.path.exists());
        assert_eq!(cli.source, CliSource::Detected);
    }

    #[test]
    #[serial_test::serial]
    fn custom_cli_has_all_fields() {
        let (_dir, path) = fake_path_with_binaries(&["my-tool"]);
        let custom = vec![CustomCliDef {
            name: "my-tool".to_string(),
            command: "my-tool".to_string(),
            display_name: Some("My Tool".to_string()),
        }];

        let result = with_path(&path, || resolve_custom_clis(&custom));

        assert_eq!(result.len(), 1);
        let cli = &result[0];
        assert_eq!(cli.binary_name, "my-tool");
        assert_eq!(cli.display_name, "My Tool");
        assert!(cli.path.exists());
        assert_eq!(cli.source, CliSource::Custom);
    }

    // --- Custom CLI with absolute path ---

    #[test]
    fn custom_cli_resolved_by_absolute_path() {
        let (_dir, path) = fake_path_with_binaries(&["my-agent"]);
        let abs = path.join("my-agent");
        let custom = vec![CustomCliDef {
            name: "my-agent".to_string(),
            command: abs.to_string_lossy().to_string(),
            display_name: Some("My Agent".to_string()),
        }];

        let result = resolve_custom_clis(&custom);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, abs);
    }

    // --- Display name defaults to capitalised binary name ---

    #[test]
    #[serial_test::serial]
    fn custom_cli_display_name_defaults_to_capitalised_name() {
        let (_dir, path) = fake_path_with_binaries(&["my-agent"]);
        let custom = vec![CustomCliDef {
            name: "my-agent".to_string(),
            command: "my-agent".to_string(),
            display_name: None,
        }];

        let result = with_path(&path, || resolve_custom_clis(&custom));

        assert_eq!(result[0].display_name, "My-agent");
    }

    // --- Results are sorted by display name ---

    #[test]
    #[serial_test::serial]
    fn results_sorted_by_display_name() {
        let (_dir, path) = fake_path_with_binaries(&["qwen", "aider", "zebra"]);
        let custom = vec![CustomCliDef {
            name: "zebra".to_string(),
            command: "zebra".to_string(),
            display_name: Some("Zebra".to_string()),
        }];

        let result = with_path(&path, || detect_clis(&custom));

        let names: Vec<&str> = result.iter().map(|c| c.display_name.as_str()).collect();
        assert_eq!(names, vec!["Aider", "Qwen", "Zebra"]);
    }

    // --- CliSource display ---

    #[test]
    fn cli_source_display_format() {
        assert_eq!(format!("{}", CliSource::Detected), "detected");
        assert_eq!(format!("{}", CliSource::Custom), "custom");
    }
}

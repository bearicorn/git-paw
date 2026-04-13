//! AI CLI detection.
//!
//! Scans PATH for known AI coding CLI binaries and merges with custom CLIs
//! from the user's configuration. Provides the combined list for interactive
//! selection or direct use.
//!
//! # Thread-Safe Architecture
//!
//! This module uses a thread-safe approach for PATH-based command resolution:
//!
//! 1. **No Global Environment Mutation**: Unlike the previous implementation that used
//!    `unsafe { std::env::set_var("PATH", ...) }`, this version never modifies
//!    the global environment.
//!
//! 2. **Process-Isolated PATH**: The `resolve_command_in` function and related
//!    internal functions use `std::process::Command::env()` to set PATH only
//!    for individual process executions. This ensures thread safety and allows
//!    true parallel test execution.
//!
//! 3. **Test Parallelization**: All tests can run in parallel without
//!    `#[serial_test::serial]` attributes because there's no shared mutable state.
//!
//! 4. **Robust PATH Construction**: When testing with custom PATH directories,
//!    the implementation automatically includes system paths (`/usr/bin:/bin:/usr/local/bin`)
//!    to ensure the `which` command itself can be found.

use std::fmt;
use std::path::{Path, PathBuf};

/// Known AI CLI binary names to scan for on PATH.
const KNOWN_CLIS: &[&str] = &[
    "claude", "codex", "gemini", "aider", "vibe", "qwen", "amp", "opencode", "cline", "droid",
    "pi", "junie", "cursor", "copilot", "cn", "kilo", "kimi",
];

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
///
/// # Architecture Note
/// This function uses `resolve_command_in` which executes the `which` command
/// with a custom PATH environment variable set only for that process execution.
/// This approach is thread-safe and avoids global environment mutation, making
/// it suitable for parallel test execution without race conditions.
#[allow(dead_code)]
fn resolve_command(command: &str) -> Option<PathBuf> {
    resolve_command_in(command, std::env::var_os("PATH").as_ref())
}

fn resolve_command_in(command: &str, path: Option<&std::ffi::OsString>) -> Option<PathBuf> {
    let path_obj = Path::new(command);
    if path_obj.is_absolute() && path_obj.exists() {
        return Some(path_obj.to_path_buf());
    }

    // Use Command with custom PATH instead of modifying global environment
    let mut cmd = std::process::Command::new("which");
    cmd.arg(command);

    // Build the PATH to use: custom path + system paths to ensure 'which' itself is found
    let final_path = if let Some(path_str) = path {
        // Convert OsString to String for manipulation
        let path_string = path_str.to_string_lossy().into_owned();
        // Include system paths to ensure 'which' and other binaries are found
        format!("{path_string}:/usr/bin:/bin:/usr/local/bin")
    } else {
        // Use system PATH if no custom path provided
        "/usr/bin:/bin:/usr/local/bin".to_string()
    };

    cmd.env("PATH", final_path);

    match cmd.output() {
        Ok(output) if output.status.success() => {
            let path_str = String::from_utf8_lossy(&output.stdout);
            let path_str = path_str.trim();
            if !path_str.is_empty() {
                return Some(PathBuf::from(path_str));
            }
        }
        _ => {}
    }

    None
}

/// Scans PATH for known AI CLI binaries.
///
/// Returns a [`CliInfo`] for each known binary found on PATH.
pub fn detect_known_clis() -> Vec<CliInfo> {
    detect_known_clis_in(std::env::var_os("PATH").as_ref())
}

fn detect_known_clis_in(path: Option<&std::ffi::OsString>) -> Vec<CliInfo> {
    KNOWN_CLIS
        .iter()
        .filter_map(|&name| {
            resolve_command_in(name, path).map(|path| CliInfo {
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
    resolve_custom_clis_in(custom, std::env::var_os("PATH").as_ref())
}

fn resolve_custom_clis_in(
    custom: &[CustomCliDef],
    path: Option<&std::ffi::OsString>,
) -> Vec<CliInfo> {
    custom
        .iter()
        .filter_map(|def| {
            if let Some(p) = resolve_command_in(&def.command, path) {
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
    detect_clis_in(custom, std::env::var_os("PATH").as_ref())
}

fn detect_clis_in(custom: &[CustomCliDef], path: Option<&std::ffi::OsString>) -> Vec<CliInfo> {
    let detected = detect_known_clis_in(path);
    let custom_resolved = resolve_custom_clis_in(custom, path);

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

    // --- Acceptance: Auto-detects all 8 known CLIs when present ---

    #[test]
    fn all_known_clis_detected_when_present() {
        let all_names = [
            "claude", "codex", "gemini", "aider", "vibe", "qwen", "amp", "opencode", "cline",
            "droid", "pi", "junie", "cursor", "copilot", "cn", "kilo", "kimi",
        ];
        let (_dir, path) = fake_path_with_binaries(&all_names);

        let result = detect_known_clis_in(Some(&path.as_os_str().to_os_string()));

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
    fn returns_empty_when_no_known_clis_on_path() {
        let (_dir, path) = fake_path_with_binaries(&[]);

        let result = detect_known_clis_in(Some(&path.as_os_str().to_os_string()));

        assert!(result.is_empty());
    }

    // --- Acceptance: Partial detection ---

    #[test]
    fn detects_subset_of_known_clis() {
        let (_dir, path) = fake_path_with_binaries(&["claude", "aider"]);

        let result = detect_known_clis_in(Some(&path.as_os_str().to_os_string()));

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|c| c.binary_name == "claude"));
        assert!(result.iter().any(|c| c.binary_name == "aider"));
    }

    // --- Acceptance: Loads custom CLIs from config and merges with detected ---

    #[test]
    fn custom_clis_merged_with_detected() {
        let (_dir, path) = fake_path_with_binaries(&["claude", "my-agent"]);
        let custom = vec![CustomCliDef {
            name: "my-agent".to_string(),
            command: "my-agent".to_string(),
            display_name: Some("My Agent".to_string()),
        }];

        let result = detect_clis_in(&custom, Some(&path.as_os_str().to_os_string()));

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
    fn custom_cli_excluded_when_binary_missing() {
        let (_dir, path) = fake_path_with_binaries(&[]);
        let custom = vec![CustomCliDef {
            name: "ghost-agent".to_string(),
            command: "/nonexistent/ghost-agent".to_string(),
            display_name: None,
        }];

        let result = detect_clis_in(&custom, Some(&path.as_os_str().to_os_string()));

        assert!(result.is_empty());
    }

    // --- Acceptance: Deduplicates — custom wins over detected ---

    #[test]
    fn custom_cli_overrides_detected_with_same_binary_name() {
        let (_dir, path) = fake_path_with_binaries(&["claude"]);
        let custom = vec![CustomCliDef {
            name: "claude".to_string(),
            command: "claude".to_string(),
            display_name: Some("My Custom Claude".to_string()),
        }];

        let result = detect_clis_in(&custom, Some(&path.as_os_str().to_os_string()));

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].binary_name, "claude");
        assert_eq!(result[0].source, CliSource::Custom);
        assert_eq!(result[0].display_name, "My Custom Claude");
    }

    // --- Acceptance: Each result has display_name, binary_name, path, source ---

    #[test]
    fn detected_cli_has_all_fields() {
        let (_dir, path) = fake_path_with_binaries(&["gemini"]);

        let result = detect_known_clis_in(Some(&path.as_os_str().to_os_string()));

        assert_eq!(result.len(), 1);
        let cli = &result[0];
        assert_eq!(cli.binary_name, "gemini");
        assert_eq!(cli.display_name, "Gemini");
        assert!(cli.path.exists());
        assert_eq!(cli.source, CliSource::Detected);
    }

    #[test]
    fn custom_cli_has_all_fields() {
        let (_dir, path) = fake_path_with_binaries(&["my-tool"]);
        let custom = vec![CustomCliDef {
            name: "my-tool".to_string(),
            command: "my-tool".to_string(),
            display_name: Some("My Tool".to_string()),
        }];

        let result = resolve_custom_clis_in(&custom, Some(&path.as_os_str().to_os_string()));

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
    fn custom_cli_display_name_defaults_to_capitalised_name() {
        let (_dir, path) = fake_path_with_binaries(&["my-agent"]);
        let custom = vec![CustomCliDef {
            name: "my-agent".to_string(),
            command: "my-agent".to_string(),
            display_name: None,
        }];

        let result = resolve_custom_clis_in(&custom, Some(&path.as_os_str().to_os_string()));

        assert_eq!(result[0].display_name, "My-agent");
    }

    // --- Results are sorted by display name ---

    #[test]
    fn results_sorted_by_display_name() {
        let (_dir, path) = fake_path_with_binaries(&["qwen", "aider", "zebra"]);
        let custom = vec![CustomCliDef {
            name: "zebra".to_string(),
            command: "zebra".to_string(),
            display_name: Some("Zebra".to_string()),
        }];

        let result = detect_clis_in(&custom, Some(&path.as_os_str().to_os_string()));

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

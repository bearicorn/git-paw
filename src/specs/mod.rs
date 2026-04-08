//! Spec scanning and discovery.
//!
//! Defines the `SpecBackend` trait for format-specific spec scanning,
//! `SpecEntry` as the universal spec representation, and `scan_specs()`
//! as the entry point for discovering pending specs.

mod markdown;
mod openspec;

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use crate::config::PawConfig;
use crate::error::PawError;
use openspec::OpenSpecBackend;

/// A discovered spec ready for session launch.
///
/// Represents a single pending spec with all the information needed
/// to create a worktree and launch an AI coding session.
#[derive(Debug)]
pub struct SpecEntry {
    /// Unique identifier (folder name or filename).
    pub id: String,
    /// Derived branch name: `branch_prefix` + `id`.
    pub branch: String,
    /// Per-spec CLI override (from `paw_cli` frontmatter).
    pub cli: Option<String>,
    /// Content to inject into the worktree `AGENTS.md`.
    pub prompt: String,
    /// File ownership if declared by the spec.
    pub owned_files: Option<Vec<String>>,
}

/// Trait for format-specific spec scanning backends.
///
/// Each spec format (`OpenSpec`, `Markdown`) implements this trait to provide
/// discovery of pending specs within a directory.
pub trait SpecBackend: fmt::Debug {
    /// Scans `dir` for pending specs and returns them as `SpecEntry` values.
    fn scan(&self, dir: &Path) -> Result<Vec<SpecEntry>, PawError>;
}

use markdown::MarkdownBackend;

/// Parses YAML frontmatter delimited by `---` lines.
///
/// Returns `(Some(fields), body)` if frontmatter is found, or `(None, content)` if not.
pub(crate) fn parse_frontmatter(content: &str) -> (Option<HashMap<String, String>>, &str) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content);
    }

    // Find the opening `---` line end
    let after_open = match trimmed.strip_prefix("---") {
        Some(rest) => {
            // Skip to end of line
            match rest.find('\n') {
                Some(idx) => &rest[idx + 1..],
                None => return (None, content),
            }
        }
        None => return (None, content),
    };

    // Find the closing `---`
    let close_pos = after_open
        .lines()
        .enumerate()
        .find(|(_, line)| line.trim() == "---");

    let (frontmatter_str, body) = match close_pos {
        Some((line_idx, _)) => {
            let byte_offset: usize = after_open.lines().take(line_idx).map(|l| l.len() + 1).sum();
            let fm = &after_open[..byte_offset];
            let after_close = &after_open[byte_offset..];
            // Skip the closing `---` line
            let body = match after_close.find('\n') {
                Some(idx) => &after_close[idx + 1..],
                None => "",
            };
            (fm, body)
        }
        None => return (None, content),
    };

    let mut fields = HashMap::new();
    for line in frontmatter_str.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            fields.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    (Some(fields), body)
}

/// Returns the appropriate backend for the given spec format type.
fn backend_for_type(spec_type: &str) -> Result<Box<dyn SpecBackend>, PawError> {
    match spec_type {
        "openspec" => Ok(Box::new(OpenSpecBackend)),
        "markdown" => Ok(Box::new(MarkdownBackend)),
        _ => Err(PawError::SpecError(format!(
            "unknown spec type: {spec_type}"
        ))),
    }
}

/// Derives a branch name by concatenating `prefix` and `id`.
///
/// Inserts a `/` separator if `prefix` does not already end with one.
fn derive_branch(prefix: &str, id: &str) -> String {
    if prefix.ends_with('/') {
        format!("{prefix}{id}")
    } else {
        format!("{prefix}/{id}")
    }
}

/// Scans for pending specs using the configuration from `[specs]`.
///
/// Reads the spec directory and format type from `config`, selects the
/// appropriate backend, scans for pending specs, and derives branch names.
///
/// Returns an error if:
/// - No `[specs]` section exists in config
/// - The spec directory does not exist or is not a directory
/// - The spec type is unknown
pub fn scan_specs(config: &PawConfig, repo_root: &Path) -> Result<Vec<SpecEntry>, PawError> {
    let specs_config = config
        .specs
        .as_ref()
        .ok_or_else(|| PawError::SpecError("no [specs] section in config".to_string()))?;

    let dir = specs_config.dir.as_deref().unwrap_or("specs");
    let specs_dir = repo_root.join(dir);

    if !specs_dir.exists() {
        return Err(PawError::SpecError(format!(
            "specs directory does not exist: {}",
            specs_dir.display()
        )));
    }
    if !specs_dir.is_dir() {
        return Err(PawError::SpecError(format!(
            "specs path is not a directory: {}",
            specs_dir.display()
        )));
    }

    let spec_type = specs_config.spec_type.as_deref().unwrap_or("openspec");
    let backend = backend_for_type(spec_type)?;

    let branch_prefix = config.branch_prefix.as_deref().unwrap_or("spec/");
    let mut entries = backend.scan(&specs_dir)?;

    for entry in &mut entries {
        entry.branch = derive_branch(branch_prefix, &entry.id);
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SpecsConfig;
    use std::fs;

    #[test]
    fn spec_entry_all_fields() {
        let entry = SpecEntry {
            id: "add-auth".to_string(),
            branch: "spec/add-auth".to_string(),
            cli: Some("claude".to_string()),
            prompt: "implement auth".to_string(),
            owned_files: Some(vec!["src/auth.rs".to_string()]),
        };
        assert_eq!(entry.id, "add-auth");
        assert_eq!(entry.branch, "spec/add-auth");
        assert_eq!(entry.cli.as_deref(), Some("claude"));
        assert_eq!(entry.prompt, "implement auth");
        assert_eq!(entry.owned_files.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn spec_entry_optional_fields_absent() {
        let entry = SpecEntry {
            id: "fix-bug".to_string(),
            branch: "spec/fix-bug".to_string(),
            cli: None,
            prompt: "fix the bug".to_string(),
            owned_files: None,
        };
        assert!(entry.cli.is_none());
        assert!(entry.owned_files.is_none());
    }

    #[test]
    fn derive_branch_default_prefix() {
        assert_eq!(derive_branch("spec/", "add-auth"), "spec/add-auth");
    }

    #[test]
    fn derive_branch_custom_prefix_with_trailing_slash() {
        assert_eq!(derive_branch("feat/", "login"), "feat/login");
    }

    #[test]
    fn derive_branch_custom_prefix_without_trailing_slash() {
        assert_eq!(derive_branch("feat", "login"), "feat/login");
    }

    #[test]
    fn backend_for_type_openspec() {
        assert!(backend_for_type("openspec").is_ok());
    }

    #[test]
    fn backend_for_type_markdown() {
        assert!(backend_for_type("markdown").is_ok());
    }

    #[test]
    fn backend_for_type_unknown() {
        let err = backend_for_type("unknown").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown spec type"), "got: {msg}");
    }

    #[test]
    fn scan_specs_no_specs_config() {
        let config = PawConfig::default();
        let tmp = tempfile::tempdir().unwrap();
        let err = scan_specs(&config, tmp.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("[specs]"), "got: {msg}");
    }

    #[test]
    fn scan_specs_nonexistent_directory() {
        let config = PawConfig {
            specs: Some(SpecsConfig {
                dir: Some("nonexistent".to_string()),
                spec_type: Some("openspec".to_string()),
            }),
            ..Default::default()
        };
        let tmp = tempfile::tempdir().unwrap();
        let err = scan_specs(&config, tmp.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("does not exist"), "got: {msg}");
        assert!(msg.contains("nonexistent"), "got: {msg}");
    }

    #[test]
    fn scan_specs_file_instead_of_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("specs");
        fs::write(&file_path, "not a directory").unwrap();
        let config = PawConfig {
            specs: Some(SpecsConfig {
                dir: Some("specs".to_string()),
                spec_type: Some("openspec".to_string()),
            }),
            ..Default::default()
        };
        let err = scan_specs(&config, tmp.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not a directory"), "got: {msg}");
    }

    #[test]
    fn scan_specs_valid_config_stub_backend() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("specs")).unwrap();
        let config = PawConfig {
            specs: Some(SpecsConfig {
                dir: Some("specs".to_string()),
                spec_type: Some("openspec".to_string()),
            }),
            ..Default::default()
        };
        let entries = scan_specs(&config, tmp.path()).unwrap();
        assert!(entries.is_empty());
    }
}

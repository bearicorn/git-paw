//! Spec scanning and discovery.
//!
//! Defines the `SpecBackend` trait for format-specific spec scanning,
//! `SpecEntry` as the universal spec representation, and `scan_specs()`
//! as the entry point for discovering pending specs.

mod markdown;
mod openspec;
pub mod resolve;
pub mod speckit;
pub mod superpowers;

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use crate::config::PawConfig;
use crate::error::PawError;
use openspec::OpenSpecBackend;
use speckit::SpecKitBackend;
use superpowers::SuperpowersBackend;

/// A discovered spec ready for session launch.
///
/// Represents a single pending spec with all the information needed
/// to create a worktree and launch an AI coding session. The `backend`
/// field identifies the `SpecBackend` implementation that produced the
/// entry, so downstream consumers (notably `build_task_prompt`) can
/// dispatch behaviour per backend without re-reading configuration.
#[derive(Debug, Clone)]
pub struct SpecEntry {
    /// Unique identifier (folder name or filename).
    pub id: String,
    /// The `SpecBackend` implementation that produced this entry.
    pub backend: SpecBackendKind,
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

/// The per-entry tag a `SpecBackend` implementation sets on every
/// `SpecEntry` it returns.
///
/// Downstream consumers (notably `build_task_prompt`) dispatch on this
/// field so per-backend behaviour does not have to re-read configuration
/// or maintain a parallel map of entry → backend identity.
// NOTE: tasks.md 1.3 of the `openspec-apply-boot-prompt` change predicted
// that the `SpecKit` variant would be added by the `spec-kit-format`
// change. That change shipped before this one and did not extend the
// enum, so we add the variant here to keep the field non-optional across
// every backend the codebase actually carries today. The Spec Kit branch
// of `build_task_prompt` falls through to the generic AGENTS.md pointer
// (same shape as `Markdown`); the `/speckit:apply` slash-command shape,
// if it ever lands, will replace that branch in a follow-up change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecBackendKind {
    /// Produced by `OpenSpecBackend` (`openspec/changes/<id>/` layout).
    OpenSpec,
    /// Produced by `MarkdownBackend` (flat `.md` files with frontmatter).
    Markdown,
    /// Produced by `SpecKitBackend` (`.specify/specs/<feature>/` layout).
    SpecKit,
    /// Produced by `SuperpowersBackend` (`docs/superpowers/plans/*.md` files).
    Superpowers,
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
        "speckit" => Ok(Box::new(SpecKitBackend)),
        "superpowers" => Ok(Box::new(SuperpowersBackend)),
        _ => Err(PawError::SpecError(format!(
            "unknown spec type: {spec_type} (known: openspec, markdown, speckit, superpowers)"
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

/// Resolves the effective spec configuration with auto-detection and CLI
/// override applied.
///
/// Precedence (highest to lowest):
/// 1. `format_override` (typically the `--specs-format` CLI value).
/// 2. Explicit `[specs]` section in TOML config.
/// 3. Auto-detection of `.specify/specs/` at the repo root → Spec Kit defaults.
///
/// Returns `None` when no source resolves a usable configuration.
fn resolve_specs_config(
    config: &PawConfig,
    repo_root: &Path,
    format_override: Option<&str>,
) -> Option<crate::config::SpecsConfig> {
    if let Some(format) = format_override {
        let mut base = config.specs.clone().unwrap_or_default();
        base.spec_type = Some(format.to_string());
        if base.dir.is_none() {
            if format == "speckit" {
                base.dir = Some(".specify/specs".to_string());
            } else if format == "superpowers" {
                base.dir = Some(superpowers::PLANS_DIR.to_string());
            }
        }
        return Some(base);
    }

    if config.specs.is_some() {
        return config.specs.clone();
    }

    // Auto-detect Spec Kit when `.specify/specs/` exists at the repo root.
    let specify = repo_root.join(".specify");
    if specify.is_dir() && specify.join("specs").is_dir() {
        return Some(crate::config::SpecsConfig {
            dir: Some(".specify/specs".to_string()),
            spec_type: Some("speckit".to_string()),
        });
    }

    // Auto-detect Superpowers when `docs/superpowers/plans/` holds at least one
    // `.md` plan. Deterministic precedence: Spec Kit (above) wins when both
    // layouts are present, so this only fires when `.specify/specs/` is absent.
    let plans = repo_root.join(superpowers::PLANS_DIR);
    if plans.is_dir() && dir_has_md(&plans) {
        return Some(crate::config::SpecsConfig {
            dir: Some(superpowers::PLANS_DIR.to_string()),
            spec_type: Some("superpowers".to_string()),
        });
    }

    None
}

/// Returns `true` when `dir` contains at least one regular `*.md` file.
fn dir_has_md(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|rd| {
        rd.filter_map(Result::ok).any(|e| {
            let p = e.path();
            p.is_file() && p.extension().is_some_and(|ext| ext == "md")
        })
    })
}

/// Resolves the effective spec engine type for a repo, or `None` when no
/// spec source is configured or auto-detected.
///
/// Applies the same precedence as [`scan_specs`] (explicit `[specs]` config,
/// then `.specify/` auto-detection) and resolves a present-but-untyped
/// `[specs]` section to the `"openspec"` default that `scan_specs` would use.
/// Consumers that need to gate a capability on the `OpenSpec` engine — notably
/// the `opsx-role-gating` guard — call this and compare against `"openspec"`.
#[must_use]
pub fn resolved_spec_type(config: &PawConfig, repo_root: &Path) -> Option<String> {
    resolve_specs_config(config, repo_root, None)
        .map(|c| c.spec_type.unwrap_or_else(|| "openspec".to_string()))
}

/// Scans for pending specs using the configuration from `[specs]`.
///
/// Reads the spec directory and format type from `config`, selects the
/// appropriate backend, scans for pending specs, and derives branch names.
///
/// Returns an error if:
/// - No `[specs]` section exists in config and no `.specify/` is auto-detected
/// - The spec directory does not exist or is not a directory
/// - The spec type is unknown
pub fn scan_specs(config: &PawConfig, repo_root: &Path) -> Result<Vec<SpecEntry>, PawError> {
    scan_specs_with_override(config, repo_root, None)
}

/// Like [`scan_specs`], but honours a CLI `--specs-format` override.
pub fn scan_specs_with_override(
    config: &PawConfig,
    repo_root: &Path,
    format_override: Option<&str>,
) -> Result<Vec<SpecEntry>, PawError> {
    let specs_config = resolve_specs_config(config, repo_root, format_override)
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

    // Backends that set their own branch name (e.g. SpecKit's `task/` and
    // `phase/` prefixes) keep it. Backends that leave `branch` empty get the
    // `<branch_prefix><id>` convention applied here.
    for entry in &mut entries {
        if entry.branch.is_empty() {
            entry.branch = derive_branch(branch_prefix, &entry.id);
        }
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
            backend: SpecBackendKind::OpenSpec,
            branch: "spec/add-auth".to_string(),
            cli: Some("claude".to_string()),
            prompt: "implement auth".to_string(),
            owned_files: Some(vec!["src/auth.rs".to_string()]),
        };
        assert_eq!(entry.id, "add-auth");
        assert_eq!(entry.backend, SpecBackendKind::OpenSpec);
        assert_eq!(entry.branch, "spec/add-auth");
        assert_eq!(entry.cli.as_deref(), Some("claude"));
        assert_eq!(entry.prompt, "implement auth");
        assert_eq!(entry.owned_files.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn spec_entry_optional_fields_absent() {
        let entry = SpecEntry {
            id: "fix-bug".to_string(),
            backend: SpecBackendKind::Markdown,
            branch: "spec/fix-bug".to_string(),
            cli: None,
            prompt: "fix the bug".to_string(),
            owned_files: None,
        };
        assert_eq!(entry.backend, SpecBackendKind::Markdown);
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
    fn backend_for_type_speckit() {
        assert!(backend_for_type("speckit").is_ok());
    }

    #[test]
    fn backend_for_type_superpowers() {
        assert!(backend_for_type("superpowers").is_ok());
    }

    #[test]
    fn backend_for_type_unknown() {
        let err = backend_for_type("unknown").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown spec type"), "got: {msg}");
        assert!(
            msg.contains("superpowers"),
            "unknown-type error lists known types incl superpowers; got: {msg}"
        );
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

    // --- Auto-detection of .specify/ ---

    #[test]
    fn auto_detect_specify_activates_speckit() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".specify").join("specs")).unwrap();
        let config = PawConfig::default();
        // The path exists but has no features — backend returns empty Vec.
        let entries = scan_specs(&config, tmp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn auto_detect_skipped_when_specs_section_present() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".specify").join("specs")).unwrap();
        fs::create_dir(tmp.path().join("my-specs")).unwrap();
        let config = PawConfig {
            specs: Some(SpecsConfig {
                dir: Some("my-specs".to_string()),
                spec_type: Some("markdown".to_string()),
            }),
            ..Default::default()
        };
        let resolved = resolve_specs_config(&config, tmp.path(), None).unwrap();
        assert_eq!(resolved.spec_type.as_deref(), Some("markdown"));
        assert_eq!(resolved.dir.as_deref(), Some("my-specs"));
    }

    #[test]
    fn auto_detect_skipped_when_no_specify_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let config = PawConfig::default();
        assert!(resolve_specs_config(&config, tmp.path(), None).is_none());
    }

    #[test]
    fn auto_detect_skipped_when_specify_missing_specs_subdir() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".specify").join("memory")).unwrap();
        let config = PawConfig::default();
        assert!(resolve_specs_config(&config, tmp.path(), None).is_none());
    }

    // Maps to scenario `Explicit config in TOML wins over auto-detection`
    // from spec-kit-format. The repo has BOTH a `.specify/specs/` directory
    // (which would normally auto-activate the SpecKit backend) AND an
    // explicit `[specs] type = "markdown"` config. The explicit config
    // must win: the Markdown backend is selected, not SpecKit.
    // (test-coverage-v0-5-0 task 11.5)
    #[test]
    fn explicit_config_wins_over_auto_detection() {
        let tmp = tempfile::tempdir().unwrap();
        // Seed `.specify/specs/` so the auto-detection branch *would* fire.
        fs::create_dir_all(tmp.path().join(".specify").join("specs")).unwrap();
        // Seed a markdown specs directory the explicit config points at.
        let md_dir = tmp.path().join("specs");
        fs::create_dir(&md_dir).unwrap();

        let config = PawConfig {
            specs: Some(SpecsConfig {
                dir: Some("specs".to_string()),
                spec_type: Some("markdown".to_string()),
            }),
            ..Default::default()
        };

        // resolve_specs_config must select the explicit config without
        // falling through to auto-detection.
        let resolved = resolve_specs_config(&config, tmp.path(), None)
            .expect("explicit config should resolve");
        assert_eq!(
            resolved.spec_type.as_deref(),
            Some("markdown"),
            "explicit type = markdown must win over the auto-detected speckit"
        );
        assert_eq!(
            resolved.dir.as_deref(),
            Some("specs"),
            "explicit dir = specs must win over the auto-detected .specify/specs"
        );

        // End-to-end: scan_specs must run the Markdown backend and NOT the
        // SpecKit backend. With an empty markdown specs/ dir the result is
        // an empty entry list; with SpecKit on the `.specify/specs/` dir
        // we would similarly get zero entries — but a SpecKit-routed scan
        // would set up the `.specify/specs/` dir as its source. We assert
        // success on the markdown path explicitly.
        let entries = scan_specs(&config, tmp.path()).unwrap();
        assert!(
            entries.is_empty(),
            "empty markdown specs dir should produce no entries; got: {entries:?}"
        );
    }

    #[test]
    fn format_override_wins_over_specs_config_and_auto_detection() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".specify").join("specs")).unwrap();
        let config = PawConfig {
            specs: Some(SpecsConfig {
                dir: Some("my-specs".to_string()),
                spec_type: Some("markdown".to_string()),
            }),
            ..Default::default()
        };
        let resolved = resolve_specs_config(&config, tmp.path(), Some("openspec")).unwrap();
        assert_eq!(resolved.spec_type.as_deref(), Some("openspec"));
        assert_eq!(resolved.dir.as_deref(), Some("my-specs"));
    }

    #[test]
    fn format_override_speckit_supplies_default_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let config = PawConfig::default();
        let resolved = resolve_specs_config(&config, tmp.path(), Some("speckit")).unwrap();
        assert_eq!(resolved.spec_type.as_deref(), Some("speckit"));
        assert_eq!(resolved.dir.as_deref(), Some(".specify/specs"));
    }

    // --- Superpowers auto-detection + override ---

    #[test]
    fn auto_detect_superpowers_activates_when_plans_present() {
        let tmp = tempfile::tempdir().unwrap();
        let plans = tmp.path().join("docs").join("superpowers").join("plans");
        fs::create_dir_all(&plans).unwrap();
        fs::write(plans.join("2026-07-20-x.md"), "### Task 1: X\n- [ ] do\n").unwrap();
        let resolved = resolve_specs_config(&PawConfig::default(), tmp.path(), None).unwrap();
        assert_eq!(resolved.spec_type.as_deref(), Some("superpowers"));
        assert_eq!(resolved.dir.as_deref(), Some("docs/superpowers/plans"));
    }

    #[test]
    fn auto_detect_speckit_wins_over_superpowers() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".specify").join("specs")).unwrap();
        let plans = tmp.path().join("docs").join("superpowers").join("plans");
        fs::create_dir_all(&plans).unwrap();
        fs::write(plans.join("p.md"), "### Task 1: X\n- [ ] do\n").unwrap();
        let resolved = resolve_specs_config(&PawConfig::default(), tmp.path(), None).unwrap();
        assert_eq!(
            resolved.spec_type.as_deref(),
            Some("speckit"),
            "speckit precedes superpowers when both layouts are present"
        );
    }

    #[test]
    fn auto_detect_superpowers_skipped_when_plans_dir_empty() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("docs").join("superpowers").join("plans")).unwrap();
        assert!(resolve_specs_config(&PawConfig::default(), tmp.path(), None).is_none());
    }

    #[test]
    fn format_override_superpowers_supplies_default_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let resolved =
            resolve_specs_config(&PawConfig::default(), tmp.path(), Some("superpowers")).unwrap();
        assert_eq!(resolved.spec_type.as_deref(), Some("superpowers"));
        assert_eq!(resolved.dir.as_deref(), Some("docs/superpowers/plans"));
    }

    #[test]
    fn scan_specs_with_override_routes_to_speckit() {
        let tmp = tempfile::tempdir().unwrap();
        let specify = tmp.path().join(".specify").join("specs");
        let feat = specify.join("001-feature");
        fs::create_dir_all(&feat).unwrap();
        fs::write(
            feat.join("tasks.md"),
            "## Phase 1: Setup\n- [ ] T001 do thing\n",
        )
        .unwrap();

        let config = PawConfig::default();
        let entries = scan_specs_with_override(&config, tmp.path(), Some("speckit")).unwrap();
        assert_eq!(entries.len(), 1);
        // SpecKit-supplied branch name is preserved (not overwritten with `spec/...`).
        assert!(
            entries[0].branch.starts_with("phase/"),
            "got branch: {}",
            entries[0].branch
        );
    }

    #[test]
    fn scan_specs_openspec_still_gets_branch_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let specs_dir = tmp.path().join("specs");
        let change = specs_dir.join("add-auth");
        fs::create_dir_all(&change).unwrap();
        fs::write(change.join("tasks.md"), "implement auth").unwrap();

        let config = PawConfig {
            specs: Some(SpecsConfig {
                dir: Some("specs".to_string()),
                spec_type: Some("openspec".to_string()),
            }),
            ..Default::default()
        };
        let entries = scan_specs(&config, tmp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].branch, "spec/add-auth");
    }
}

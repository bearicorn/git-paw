//! Spec scanning and discovery.
//!
//! Defines the `SpecBackend` trait for format-specific spec scanning,
//! `SpecEntry` as the universal spec representation, and `scan_specs()`
//! as the entry point for discovering pending specs.

#[cfg(test)]
mod backend_pin_tests;
mod markdown;
pub mod openspec;
pub mod resolve;
pub mod speckit;
pub mod superpowers;

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::config::PawConfig;
use crate::error::PawError;
use crate::task_prompts::{TaskPrompt, TaskPromptTemplates};
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

/// One file belonging to a spec, as the MCP `get_spec` tool serves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecArtifact {
    /// Artifact name (e.g. `proposal`, `design`, `tasks`, or a filename).
    pub name: String,
    /// Full content.
    pub content: String,
}

/// One task parsed from a spec's task list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecTask {
    /// Task id (e.g. `T009`, or a derived sequence number).
    pub id: String,
    /// Phase number (0 for the implicit leading phase).
    pub phase: u32,
    /// Whether the task carries a `[P]` parallel marker.
    pub parallel: bool,
    /// Description text.
    pub description: String,
    /// Completion state.
    pub complete: bool,
}

/// Everything that differs between spec formats. Each format implements it
/// once, deciding every method itself; code outside `src/specs/` uses a
/// format only through [`SpecBackendKind::backend`].
pub trait SpecBackend: fmt::Debug {
    /// The identity tag this backend sets on every entry it scans.
    fn kind(&self) -> SpecBackendKind;

    /// The format's conventional specs directory, relative to the repository
    /// root. Supplied when the format comes from `--specs-format` and no
    /// `[specs] dir` is set, and written by `git paw init`.
    fn default_dir(&self) -> &'static str;

    /// Scans `dir` for pending specs and returns them as `SpecEntry` values.
    fn scan(&self, dir: &Path) -> Result<Vec<SpecEntry>, PawError>;

    /// The one-sentence spec-path doctrine rendered into the supervisor and
    /// orchestrator skills (the bundled asset, verbatim).
    fn path_doctrine(&self) -> &'static str;

    /// The bundled task-prompt templates, placeholders unfilled.
    fn task_prompt_templates(&self) -> TaskPromptTemplates;

    /// The per-agent task prompt for `entry`, placeholders filled.
    fn task_prompt(&self, entry: &SpecEntry) -> TaskPrompt {
        self.task_prompt_templates().render(&entry.id)
    }

    /// The file inside a spec's directory whose first `# ` heading titles the
    /// spec, if the format has one.
    fn title_artifact(&self) -> Option<&'static str>;

    /// The artifacts served for `entry`, whose directory is `spec_dir`.
    fn artifacts(&self, spec_dir: &Path, entry: &SpecEntry) -> Vec<SpecArtifact>;

    /// Parses a spec's task list.
    fn parse_tasks(&self, content: &str) -> Vec<SpecTask>;

    /// Whether the format runs the opsx workflow, which gates the opsx
    /// role-gating skill regions and the role-gating guard.
    fn uses_opsx_workflow(&self) -> bool;

    /// The format's governance constitution beside `specs_dir`, auto-wired
    /// when `[governance] constitution` is unset.
    fn constitution(&self, specs_dir: &Path) -> Option<PathBuf>;

    /// Whether the MCP spec-authoring tools (create, write, validate) can
    /// author specs in this format.
    fn supports_spec_authoring(&self) -> bool;
}

/// A single-file spec's artifacts: one artifact named after the entry,
/// carrying the assembled entry prompt.
pub(crate) fn single_file_artifacts(entry: &SpecEntry) -> Vec<SpecArtifact> {
    vec![SpecArtifact {
        name: entry.id.clone(),
        content: entry.prompt.clone(),
    }]
}

/// Parses a flat Markdown checkbox task list, tracking `## ` headings as
/// phases and numbering tasks in order.
pub(crate) fn parse_checkbox_tasks(content: &str) -> Vec<SpecTask> {
    let mut phase = 0u32;
    let mut seq = 0u32;
    let mut out = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("## ") {
            phase += 1;
            continue;
        }
        let Some(rest) = t.strip_prefix("- [").or_else(|| t.strip_prefix("* [")) else {
            continue;
        };
        let Some(mark) = rest.chars().next() else {
            continue;
        };
        let desc = rest.get(2..).unwrap_or("").trim().to_string();
        seq += 1;
        out.push(SpecTask {
            id: format!("{seq}"),
            phase,
            parallel: false,
            description: desc,
            complete: mark == 'x' || mark == 'X',
        });
    }
    out
}

/// Reads `dir/file` as an artifact named after the file without its `.md`
/// extension, or `None` when it cannot be read.
pub(crate) fn read_named_artifact(dir: &Path, file: &str) -> Option<SpecArtifact> {
    let content = std::fs::read_to_string(dir.join(file)).ok()?;
    Some(SpecArtifact {
        name: file.trim_end_matches(".md").to_string(),
        content,
    })
}

/// The identity of a spec format: the tag every `SpecEntry` carries, parsed
/// from and rendered to the `[specs] type` / `--specs-format` / MCP
/// `backend` name. Behaviour lives on the format's [`SpecBackend`], reached
/// through [`SpecBackendKind::backend`].
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

impl SpecBackendKind {
    /// Every variant, in canonical order — the single source of truth for
    /// which `[specs].type` values are supported.
    pub const ALL: [SpecBackendKind; 4] = [
        SpecBackendKind::OpenSpec,
        SpecBackendKind::Markdown,
        SpecBackendKind::SpecKit,
        SpecBackendKind::Superpowers,
    ];

    /// The format's wire name: the `[specs] type` value, the
    /// `--specs-format` value and the MCP `backend` string.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenSpec => "openspec",
            Self::Markdown => "markdown",
            Self::SpecKit => "speckit",
            Self::Superpowers => "superpowers",
        }
    }

    /// The format's backend — the one exhaustive mapping from a kind to its
    /// behaviour. Adding a format adds a variant, its arm here and its
    /// [`SpecBackend`] implementation.
    #[must_use]
    pub fn backend(self) -> &'static dyn SpecBackend {
        match self {
            Self::OpenSpec => &OpenSpecBackend,
            Self::Markdown => &MarkdownBackend,
            Self::SpecKit => &SpecKitBackend,
            Self::Superpowers => &SuperpowersBackend,
        }
    }
}

impl std::str::FromStr for SpecBackendKind {
    type Err = PawError;

    /// Parses a wire name; an unknown name fails with the
    /// "unknown spec type" error listing every accepted name.
    fn from_str(spec_type: &str) -> Result<Self, PawError> {
        Self::ALL
            .into_iter()
            .find(|k| k.as_str() == spec_type)
            .ok_or_else(|| {
                let known = Self::ALL.map(Self::as_str).join(", ");
                PawError::SpecError(format!("unknown spec type: {spec_type} (known: {known})"))
            })
    }
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

/// Slugifies one segment of a branch name the spec providers build from spec
/// text (a task id plus its description, a feature dir plus a phase name, a
/// plan file stem).
///
/// Steps, in order:
///
/// 1. Convert ASCII uppercase letters to lowercase
/// 2. Replace every character not in `[a-z0-9_]` with `-`
/// 3. Collapse runs of `-` into a single `-`
/// 4. Trim leading and trailing `-`
/// 5. If the result is empty, return `"agent"`
///
/// This deliberately keeps `_`: a branch is durable git state (existing
/// worktrees, recovery and `--resume` find work by branch name) and `_` is
/// legal in a ref, so the providers do not borrow the broker's stricter
/// [`crate::broker::messages::slugify_branch`]. That function maps `_` to `-`
/// when the same branch is turned into an `agent_id`, so a branch keeping `_`
/// never yields an id the broker rejects.
///
/// # Examples
///
/// - `"T009-Add login form"` → `"t009-add-login-form"`
/// - `"T001-Create ROOM_SETUP.md"` → `"t001-create-room_setup-md"`
/// - `""` → `"agent"`
#[must_use]
pub fn branch_segment_slug(input: &str) -> String {
    let replaced: String = input
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();

    let mut collapsed = String::with_capacity(replaced.len());
    let mut prev_dash = false;
    for c in replaced.chars() {
        if c == '-' {
            if !prev_dash {
                collapsed.push('-');
            }
            prev_dash = true;
        } else {
            collapsed.push(c);
            prev_dash = false;
        }
    }

    let trimmed = collapsed.trim_matches('-');
    if trimmed.is_empty() {
        "agent".to_string()
    } else {
        trimmed.to_string()
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

/// Resolves the effective spec configuration from EXPLICIT sources only.
///
/// Precedence (highest to lowest):
/// 1. `format_override` (the `--specs-format` CLI value).
/// 2. Explicit `[specs]` section in TOML config.
///
/// git-paw does NOT probe the filesystem to guess the spec system: the config
/// `[specs]` section or `--specs-format` is the sole source of truth. When the
/// override names a format but no `dir` is set, the format's conventional
/// directory is supplied; a `[specs]` section without a `dir` scans `specs`
/// whatever its type. An untyped `[specs]` section selects `OpenSpec`.
///
/// Returns `Ok(None)` when neither source is set — the caller turns that into
/// an actionable error.
///
/// # Errors
///
/// Returns `PawError::SpecError` ("unknown spec type") when `[specs] type`
/// names no supported format.
pub fn resolve_specs_config(
    config: &PawConfig,
    format_override: Option<SpecBackendKind>,
) -> Result<Option<ResolvedSpecs>, PawError> {
    let configured_dir = config.specs.as_ref().and_then(|s| s.dir.clone());
    if let Some(kind) = format_override {
        return Ok(Some(ResolvedSpecs {
            kind,
            dir: configured_dir.unwrap_or_else(|| kind.backend().default_dir().to_string()),
        }));
    }
    let Some(specs) = config.specs.as_ref() else {
        return Ok(None);
    };
    let kind = match specs.spec_type.as_deref() {
        Some(spec_type) => spec_type.parse()?,
        None => SpecBackendKind::OpenSpec,
    };
    Ok(Some(ResolvedSpecs {
        kind,
        dir: configured_dir.unwrap_or_else(|| "specs".to_string()),
    }))
}

/// The spec format and directory a repository's configuration selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSpecs {
    /// The selected format.
    pub kind: SpecBackendKind,
    /// The specs directory, relative to the repository root.
    pub dir: String,
}

/// Resolves the configured spec type for display, or `None` when no
/// `[specs]` section is configured.
///
/// A present-but-untyped `[specs]` section resolves to the `OpenSpec` default
/// that `scan_specs` would use; an unknown type is returned verbatim so a
/// diagnostic can show what was configured. There is no filesystem
/// auto-detection: an unconfigured repo returns `None`.
#[must_use]
pub fn resolved_spec_type(config: &PawConfig, _repo_root: &Path) -> Option<String> {
    config.specs.as_ref().map(|c| {
        c.spec_type
            .clone()
            .unwrap_or_else(|| SpecBackendKind::OpenSpec.as_str().to_string())
    })
}

/// The spec format `config` selects, or `None` when no `[specs]` section is
/// configured or its type is unknown.
#[must_use]
pub fn resolved_spec_kind(config: &PawConfig) -> Option<SpecBackendKind> {
    resolve_specs_config(config, None)
        .ok()
        .flatten()
        .map(|r| r.kind)
}

/// Whether the opsx role-gating guard applies to `config`'s spec format —
/// true only when the resolved format runs the opsx workflow.
#[must_use]
pub fn opsx_role_gating_applies(config: &PawConfig) -> bool {
    resolved_spec_kind(config).is_some_and(|k| k.backend().uses_opsx_workflow())
}

/// Scans for pending specs using the configuration from `[specs]`.
///
/// Reads the spec directory and format type from `config`, selects the
/// appropriate backend, scans for pending specs, and derives branch names.
///
/// Returns an error if:
/// - No `[specs]` section is configured and no `--specs-format` is passed
/// - The spec directory does not exist or is not a directory
/// - The spec type is unknown
pub fn scan_specs(config: &PawConfig, repo_root: &Path) -> Result<Vec<SpecEntry>, PawError> {
    scan_specs_with_override(config, repo_root, None)
}

/// Like [`scan_specs`], but honours a CLI `--specs-format` override.
pub fn scan_specs_with_override(
    config: &PawConfig,
    repo_root: &Path,
    format_override: Option<SpecBackendKind>,
) -> Result<Vec<SpecEntry>, PawError> {
    let resolved = resolve_specs_config(config, format_override)?.ok_or_else(|| {
        PawError::SpecError(
            "no spec system configured: add a [specs] section to .git-paw/config.toml \
             (type = \"openspec\" | \"markdown\" | \"speckit\" | \"superpowers\") or pass \
             --specs-format"
                .to_string(),
        )
    })?;

    let specs_dir = repo_root.join(&resolved.dir);

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

    let branch_prefix = config.branch_prefix.as_deref().unwrap_or("spec/");
    let mut entries = resolved.kind.backend().scan(&specs_dir)?;

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

    /// `spec-providers`: "Every accepted name round-trips".
    #[test]
    fn every_accepted_name_round_trips() {
        for name in ["openspec", "markdown", "speckit", "superpowers"] {
            let kind: SpecBackendKind = name.parse().unwrap();
            assert_eq!(kind.as_str(), name);
        }
        let names: Vec<&str> = SpecBackendKind::ALL.map(SpecBackendKind::as_str).to_vec();
        assert_eq!(names, ["openspec", "markdown", "speckit", "superpowers"]);
    }

    #[test]
    fn single_file_formats_serve_the_entry_prompt_as_one_artifact() {
        let entry = SpecEntry {
            id: "fix-bug".to_string(),
            backend: SpecBackendKind::Markdown,
            branch: String::new(),
            cli: None,
            prompt: "# Fix the bug\n".to_string(),
            owned_files: None,
        };
        for kind in [SpecBackendKind::Markdown, SpecBackendKind::Superpowers] {
            let artifacts = kind.backend().artifacts(Path::new("unused"), &entry);
            assert_eq!(
                artifacts,
                [SpecArtifact {
                    name: "fix-bug".to_string(),
                    content: "# Fix the bug\n".to_string(),
                }],
                "{kind:?}"
            );
        }
    }

    #[test]
    fn only_openspec_supports_spec_authoring() {
        for kind in SpecBackendKind::ALL {
            assert_eq!(
                kind.backend().supports_spec_authoring(),
                kind == SpecBackendKind::OpenSpec,
                "{kind:?}"
            );
        }
    }

    #[test]
    fn every_backend_reports_its_own_kind() {
        for kind in SpecBackendKind::ALL {
            assert_eq!(kind.backend().kind(), kind);
        }
    }

    #[test]
    fn unknown_name_fails_listing_the_accepted_names() {
        let err = "unknown".parse::<SpecBackendKind>().unwrap_err();
        assert!(matches!(err, PawError::SpecError(_)));
        assert_eq!(
            err.to_string(),
            PawError::SpecError(
                "unknown spec type: unknown (known: openspec, markdown, speckit, superpowers)"
                    .to_string()
            )
            .to_string()
        );
    }

    /// `spec-providers`: "An unknown type keeps its error".
    #[test]
    fn scan_with_unknown_type_fails_with_spec_error() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("specs")).unwrap();
        let config = PawConfig {
            specs: Some(SpecsConfig {
                dir: Some("specs".to_string()),
                spec_type: Some("unknown".to_string()),
            }),
            ..Default::default()
        };
        let err = scan_specs(&config, tmp.path()).unwrap_err();
        assert!(matches!(err, PawError::SpecError(_)));
        let msg = err.to_string();
        assert!(msg.contains("unknown spec type: unknown"), "got: {msg}");
        for kind in SpecBackendKind::ALL {
            assert!(msg.contains(kind.as_str()), "lists {kind:?}; got: {msg}");
        }
    }

    /// `spec-providers`: "opsx role gating follows the backend" (the guard).
    #[test]
    fn opsx_guard_applies_only_to_openspec() {
        let with = |ty: &str| PawConfig {
            specs: Some(SpecsConfig {
                dir: None,
                spec_type: Some(ty.to_string()),
            }),
            ..Default::default()
        };
        assert!(opsx_role_gating_applies(&with("openspec")));
        assert!(!opsx_role_gating_applies(&with("speckit")));
        assert!(!opsx_role_gating_applies(&PawConfig::default()));
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

    // --- Explicit config / CLI are the only spec sources (no auto-detection) ---

    #[test]
    fn unconfigured_repo_resolves_to_none_even_with_layouts_present() {
        // Neither a [specs] section nor --specs-format: resolve to None even
        // though `.specify/specs/` and `docs/superpowers/plans/` exist on disk.
        // git-paw never probes the filesystem to guess the spec system.
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".specify").join("specs")).unwrap();
        let plans = tmp.path().join("docs").join("superpowers").join("plans");
        fs::create_dir_all(&plans).unwrap();
        fs::write(plans.join("p.md"), "### Task 1: X\n- [ ] do\n").unwrap();

        assert!(
            resolve_specs_config(&PawConfig::default(), None)
                .unwrap()
                .is_none(),
            "no [specs] and no --specs-format must resolve to None (no auto-detection)"
        );
    }

    #[test]
    fn unconfigured_scan_errors_with_actionable_message() {
        let tmp = tempfile::tempdir().unwrap();
        let err = scan_specs(&PawConfig::default(), tmp.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("[specs]"), "got: {msg}");
        assert!(
            msg.contains("--specs-format"),
            "error names the CLI fix; got: {msg}"
        );
    }

    #[test]
    fn config_specs_section_is_used_verbatim() {
        // The config [specs] section is the source of truth — returned as-is,
        // with no filesystem probing even when `.specify/specs/` exists.
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".specify").join("specs")).unwrap();
        let config = PawConfig {
            specs: Some(SpecsConfig {
                dir: Some("my-specs".to_string()),
                spec_type: Some("markdown".to_string()),
            }),
            ..Default::default()
        };
        let resolved = resolve_specs_config(&config, None).unwrap().unwrap();
        assert_eq!(resolved.kind, SpecBackendKind::Markdown);
        assert_eq!(resolved.dir, "my-specs");
    }

    /// A configured type with no `dir` scans `specs` for every format; the
    /// conventional directory is supplied only on the `--specs-format` path.
    #[test]
    fn configured_type_without_dir_resolves_to_specs_dir() {
        for kind in SpecBackendKind::ALL {
            let config = PawConfig {
                specs: Some(SpecsConfig {
                    dir: None,
                    spec_type: Some(kind.as_str().to_string()),
                }),
                ..Default::default()
            };
            let resolved = resolve_specs_config(&config, None).unwrap().unwrap();
            assert_eq!(resolved.dir, "specs", "{kind:?}");
        }
    }

    #[test]
    fn format_override_wins_over_config() {
        let config = PawConfig {
            specs: Some(SpecsConfig {
                dir: Some("my-specs".to_string()),
                spec_type: Some("markdown".to_string()),
            }),
            ..Default::default()
        };
        let resolved = resolve_specs_config(&config, Some(SpecBackendKind::OpenSpec))
            .unwrap()
            .unwrap();
        assert_eq!(resolved.kind, SpecBackendKind::OpenSpec);
        // The override keeps the configured dir when one is set.
        assert_eq!(resolved.dir, "my-specs");
    }

    #[test]
    fn format_override_speckit_supplies_default_dir() {
        let resolved = resolve_specs_config(&PawConfig::default(), Some(SpecBackendKind::SpecKit))
            .unwrap()
            .unwrap();
        assert_eq!(resolved.kind, SpecBackendKind::SpecKit);
        assert_eq!(resolved.dir, ".specify/specs");
    }

    #[test]
    fn format_override_superpowers_supplies_default_dir() {
        let resolved =
            resolve_specs_config(&PawConfig::default(), Some(SpecBackendKind::Superpowers))
                .unwrap()
                .unwrap();
        assert_eq!(resolved.kind, SpecBackendKind::Superpowers);
        assert_eq!(resolved.dir, "docs/superpowers/plans");
    }

    #[test]
    fn format_override_openspec_and_markdown_supply_specs() {
        for kind in [SpecBackendKind::OpenSpec, SpecBackendKind::Markdown] {
            let resolved = resolve_specs_config(&PawConfig::default(), Some(kind))
                .unwrap()
                .unwrap();
            assert_eq!(resolved.dir, "specs", "{kind:?}");
        }
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
        let entries =
            scan_specs_with_override(&config, tmp.path(), Some(SpecBackendKind::SpecKit)).unwrap();
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

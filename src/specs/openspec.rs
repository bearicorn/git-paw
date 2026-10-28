//! OpenSpec-format backend for spec scanning.
//!
//! Scans `openspec/changes/` for pending changes, extracts prompt content
//! from `tasks.md` and supplementary `specs/` files, and parses optional
//! frontmatter fields and file ownership declarations.

use std::fmt::Write;
use std::fs;
use std::path::Path;

use rmcp::schemars;
use serde::Serialize;

use crate::error::PawError;
use crate::specs::{SpecBackend, SpecBackendKind, SpecEntry};

/// Backend for the `OpenSpec` directory-based spec format.
///
/// Each pending change lives in its own subdirectory under the scanned
/// directory. The `tasks.md` file provides the primary prompt, and any
/// `specs/<capability>/spec.md` files are appended as supplementary content.
#[derive(Debug)]
pub struct OpenSpecBackend;

impl SpecBackend for OpenSpecBackend {
    fn scan(&self, dir: &Path) -> Result<Vec<SpecEntry>, PawError> {
        let entries = fs::read_dir(dir).map_err(|e| {
            PawError::SpecError(format!("cannot read directory {}: {e}", dir.display()))
        })?;

        let mut specs = Vec::new();

        for entry in entries {
            let entry = entry
                .map_err(|e| PawError::SpecError(format!("error reading directory entry: {e}")))?;

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = entry.file_name();
            let id = name.to_string_lossy().to_string();

            // Skip archive directory.
            if id == "archive" {
                continue;
            }

            let tasks_path = path.join("tasks.md");
            if !tasks_path.exists() {
                eprintln!("warning: skipping change {id}: no tasks.md found");
                continue;
            }

            let tasks_content = fs::read_to_string(&tasks_path).map_err(|e| {
                PawError::SpecError(format!("cannot read {}: {e}", tasks_path.display()))
            })?;

            let (frontmatter, body) = parse_frontmatter(&tasks_content);
            let cli = frontmatter
                .iter()
                .find(|(k, _)| k == "paw_cli")
                .map(|(_, v)| v.clone());

            let owned_files = extract_owned_files(&tasks_content);

            let mut prompt = body.to_string();

            // Append supplementary specs if present.
            let specs_dir = path.join("specs");
            if specs_dir.is_dir()
                && let Ok(spec_entries) = fs::read_dir(&specs_dir)
            {
                let mut cap_dirs: Vec<_> = spec_entries
                    .filter_map(Result::ok)
                    .filter(|e| e.path().is_dir())
                    .collect();
                cap_dirs.sort_by_key(std::fs::DirEntry::file_name);

                for cap_entry in cap_dirs {
                    let spec_file = cap_entry.path().join("spec.md");
                    if spec_file.exists() {
                        let cap_name = cap_entry.file_name().to_string_lossy().to_string();
                        if let Ok(spec_content) = fs::read_to_string(&spec_file) {
                            let _ = write!(prompt, "\n\n## Spec: {cap_name}\n\n{spec_content}");
                        }
                    }
                }
            }

            specs.push(SpecEntry {
                id,
                backend: SpecBackendKind::OpenSpec,
                branch: String::new(), // Filled in by scan_specs.
                cli,
                prompt,
                owned_files,
            });
        }

        Ok(specs)
    }
}

/// Parses YAML-style frontmatter delimited by `---` lines.
///
/// Returns a list of key-value pairs and the remaining content after
/// the closing `---`. If no frontmatter is present, returns an empty
/// list and the original content.
fn parse_frontmatter(content: &str) -> (Vec<(String, String)>, &str) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (vec![], content);
    }

    // Find the opening delimiter and start searching for the closing one.
    let after_open = match trimmed.strip_prefix("---") {
        Some(rest) => rest.trim_start_matches('-'),
        None => return (vec![], content),
    };

    // Skip the newline after the opening ---.
    let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);

    let Some(close_pos) = after_open.find("\n---") else {
        return (vec![], content);
    };

    let front = &after_open[..close_pos];
    let rest_start = close_pos + 4; // skip "\n---"
    let rest = &after_open[rest_start..];
    // Skip the newline after the closing ---.
    let rest = rest.strip_prefix('\n').unwrap_or(rest);

    let mut fields = Vec::new();
    for line in front.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            fields.push((key.trim().to_string(), value.trim().to_string()));
        }
    }

    (fields, rest)
}

/// Extracts file ownership declarations from content.
///
/// Looks for a line containing `Files owned:` or `Owned files:` (case-insensitive)
/// followed by a markdown list of file paths (lines starting with `- `).
fn extract_owned_files(content: &str) -> Option<Vec<String>> {
    let lower = content.to_lowercase();
    let pattern_pos = lower
        .find("files owned:")
        .or_else(|| lower.find("owned files:"))?;

    // Find the end of the header line.
    let after_header = &content[pattern_pos..];
    let newline_pos = after_header.find('\n')?;
    let list_start = &after_header[newline_pos + 1..];

    let mut files = Vec::new();
    for line in list_start.lines() {
        let trimmed = line.trim();
        if let Some(path) = trimmed.strip_prefix("- ") {
            files.push(path.trim().trim_matches('`').to_string());
        } else if !trimmed.is_empty() {
            break;
        }
    }

    if files.is_empty() { None } else { Some(files) }
}

/// File name of an `OpenSpec` change's metadata.
pub const CHANGE_METADATA_FILE: &str = ".openspec.yaml";

/// Whether `name` is a valid `OpenSpec` change name: non-empty and made only
/// of lowercase ASCII letters, digits and hyphens.
#[must_use]
pub fn is_valid_change_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The [`CHANGE_METADATA_FILE`] content `openspec new change` writes for a
/// change created on `created`.
#[must_use]
pub fn change_metadata(created: chrono::NaiveDate) -> String {
    format!(
        "schema: spec-driven\ncreated: {}\n",
        created.format("%Y-%m-%d")
    )
}

/// One structural finding reported by [`check_change`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ChangeFinding {
    /// The file the finding concerns, relative to the change directory.
    pub file: String,
    /// What is wrong.
    pub message: String,
}

impl ChangeFinding {
    fn new(file: &str, message: String) -> Self {
        Self {
            file: file.to_string(),
            message,
        }
    }
}

/// The delta-section headers a delta spec may carry.
const DELTA_HEADERS: [&str; 4] = [
    "## ADDED Requirements",
    "## MODIFIED Requirements",
    "## REMOVED Requirements",
    "## RENAMED Requirements",
];

/// Whether `path` is a delta spec: exactly `specs/<capability>/spec.md`.
fn is_delta_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    matches!(parts.as_slice(), ["specs", cap, "spec.md"] if !cap.is_empty())
}

/// The lines of `content` outside fenced code blocks; fenced lines (and the
/// fences themselves) are replaced by `None` so headers inside them are
/// ignored.
fn unfenced_lines(content: &str) -> Vec<Option<&str>> {
    let mut in_fence = false;
    content
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("```") {
                in_fence = !in_fence;
                return None;
            }
            if in_fence { None } else { Some(line) }
        })
        .collect()
}

/// Checks one delta file's ADDED and MODIFIED requirements.
fn check_delta(path: &str, content: &str, findings: &mut Vec<ChangeFinding>) {
    let lines = unfenced_lines(content);
    let mut in_checked_section = false;
    let mut i = 0;
    while i < lines.len() {
        let Some(line) = lines[i] else {
            i += 1;
            continue;
        };
        if line.starts_with("## ") {
            let header = line.trim_end();
            in_checked_section =
                header == "## ADDED Requirements" || header == "## MODIFIED Requirements";
            i += 1;
            continue;
        }
        let Some(name) = line.strip_prefix("### Requirement:") else {
            i += 1;
            continue;
        };
        let name = name.trim();
        // The block runs to the next `### ` or `## ` header.
        let mut end = i + 1;
        while end < lines.len()
            && !lines[end].is_some_and(|l| l.starts_with("### ") || l.starts_with("## "))
        {
            end += 1;
        }
        if in_checked_section {
            let body = &lines[i + 1..end];
            let first = content
                .lines()
                .skip(i + 1)
                .take(end - i - 1)
                .find(|l| !l.trim().is_empty());
            if !first.is_some_and(|l| l.contains("SHALL") || l.contains("MUST")) {
                findings.push(ChangeFinding::new(
                    path,
                    format!(
                        "requirement \"{name}\": SHALL or MUST must appear on the first line of its body"
                    ),
                ));
            }
            if !body
                .iter()
                .any(|l| l.is_some_and(|l| l.starts_with("#### Scenario:")))
            {
                findings.push(ChangeFinding::new(
                    path,
                    format!("requirement \"{name}\" has no #### Scenario: block"),
                ));
            }
        }
        i = end;
    }
}

/// Structural check over an `OpenSpec` change, a subset of what the project's
/// own validator enforces. `files` holds each file of the change as (path
/// relative to the change directory with forward slashes, content).
///
/// Reports a missing `proposal.md`; the absence of any
/// `specs/<capability>/spec.md` carrying an `## ADDED|MODIFIED|REMOVED|RENAMED
/// Requirements` header; and, in ADDED and MODIFIED sections, a requirement
/// whose first non-blank body line holds neither SHALL nor MUST, or that has
/// no `#### Scenario:` block. Headers inside fenced code blocks are ignored.
#[must_use]
pub fn check_change(files: &[(String, String)]) -> Vec<ChangeFinding> {
    let mut findings = Vec::new();
    if !files.iter().any(|(p, _)| p == "proposal.md") {
        findings.push(ChangeFinding::new(
            "proposal.md",
            "missing proposal.md".to_string(),
        ));
    }

    let mut deltas: Vec<&(String, String)> =
        files.iter().filter(|(p, _)| is_delta_path(p)).collect();
    deltas.sort_by(|a, b| a.0.cmp(&b.0));

    let has_header = deltas.iter().any(|(_, content)| {
        unfenced_lines(content)
            .into_iter()
            .flatten()
            .any(|l| DELTA_HEADERS.contains(&l.trim_end()))
    });
    if !has_header {
        findings.push(ChangeFinding::new(
            "specs/<capability>/spec.md",
            "no delta spec with an ## ADDED|MODIFIED|REMOVED|RENAMED Requirements header"
                .to_string(),
        ));
    }

    for (path, content) in deltas {
        check_delta(path, content, &mut findings);
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // --- parse_frontmatter tests ---

    #[test]
    fn frontmatter_with_paw_cli() {
        let content = "---\npaw_cli: gemini\n---\nBody here";
        let (fields, body) = parse_frontmatter(content);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0], ("paw_cli".to_string(), "gemini".to_string()));
        assert_eq!(body, "Body here");
    }

    #[test]
    fn frontmatter_without_paw_cli() {
        let content = "---\ntitle: my change\n---\nBody here";
        let (fields, body) = parse_frontmatter(content);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "title");
        assert!(fields.iter().all(|(k, _)| k != "paw_cli"));
        assert_eq!(body, "Body here");
    }

    #[test]
    fn no_frontmatter() {
        let content = "Just a body\nwith lines";
        let (fields, body) = parse_frontmatter(content);
        assert!(fields.is_empty());
        assert_eq!(body, content);
    }

    #[test]
    fn frontmatter_multiple_fields() {
        let content = "---\npaw_cli: claude\ntitle: test\n---\nContent";
        let (fields, body) = parse_frontmatter(content);
        assert_eq!(fields.len(), 2);
        assert_eq!(body, "Content");
    }

    // --- extract_owned_files tests ---

    #[test]
    fn owned_files_present() {
        let content = "Some text\n\nFiles owned:\n- src/auth.rs\n- src/login.rs\n\nMore text";
        let files = extract_owned_files(content).unwrap();
        assert_eq!(files, vec!["src/auth.rs", "src/login.rs"]);
    }

    #[test]
    fn owned_files_alternate_pattern() {
        let content = "Owned files:\n- `src/main.rs`\n";
        let files = extract_owned_files(content).unwrap();
        assert_eq!(files, vec!["src/main.rs"]);
    }

    #[test]
    fn no_owned_files() {
        let content = "No file ownership here";
        assert!(extract_owned_files(content).is_none());
    }

    // --- OpenSpecBackend integration tests ---

    #[test]
    fn scan_empty_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let backend = OpenSpecBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn scan_skips_files() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("not-a-dir.md"), "content").unwrap();
        let backend = OpenSpecBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn scan_skips_archive() {
        let tmp = tempfile::tempdir().unwrap();
        let archive = tmp.path().join("archive");
        fs::create_dir(&archive).unwrap();
        fs::write(archive.join("tasks.md"), "archived task").unwrap();
        let backend = OpenSpecBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn scan_skips_missing_tasks_md() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("no-tasks")).unwrap();
        let backend = OpenSpecBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn scan_basic_change() {
        let tmp = tempfile::tempdir().unwrap();
        let change = tmp.path().join("add-auth");
        fs::create_dir(&change).unwrap();
        fs::write(change.join("tasks.md"), "implement auth").unwrap();

        let backend = OpenSpecBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "add-auth");
        assert_eq!(result[0].prompt, "implement auth");
        assert!(result[0].cli.is_none());
        assert!(result[0].owned_files.is_none());
    }

    #[test]
    fn scan_with_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let change = tmp.path().join("my-change");
        fs::create_dir(&change).unwrap();
        fs::write(
            change.join("tasks.md"),
            "---\npaw_cli: gemini\n---\nDo the thing",
        )
        .unwrap();

        let backend = OpenSpecBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].cli.as_deref(), Some("gemini"));
        assert_eq!(result[0].prompt, "Do the thing");
    }

    #[test]
    fn scan_with_specs() {
        let tmp = tempfile::tempdir().unwrap();
        let change = tmp.path().join("feat-x");
        fs::create_dir_all(change.join("specs/auth")).unwrap();
        fs::write(change.join("tasks.md"), "Primary task").unwrap();
        fs::write(change.join("specs/auth/spec.md"), "Auth spec content").unwrap();

        let backend = OpenSpecBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].prompt.contains("Primary task"));
        assert!(result[0].prompt.contains("## Spec: auth"));
        assert!(result[0].prompt.contains("Auth spec content"));
    }

    #[test]
    fn scan_with_owned_files() {
        let tmp = tempfile::tempdir().unwrap();
        let change = tmp.path().join("change-y");
        fs::create_dir(&change).unwrap();
        fs::write(
            change.join("tasks.md"),
            "Do stuff\n\nFiles owned:\n- src/a.rs\n- src/b.rs\n",
        )
        .unwrap();

        let backend = OpenSpecBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert_eq!(result.len(), 1);
        let files = result[0].owned_files.as_ref().unwrap();
        assert_eq!(files, &["src/a.rs", "src/b.rs"]);
    }

    #[test]
    fn scan_multiple_changes() {
        let tmp = tempfile::tempdir().unwrap();
        for name in &["alpha", "beta"] {
            let d = tmp.path().join(name);
            fs::create_dir(&d).unwrap();
            fs::write(d.join("tasks.md"), format!("task for {name}")).unwrap();
        }

        let backend = OpenSpecBackend;
        let mut result = backend.scan(tmp.path()).unwrap();
        result.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "alpha");
        assert_eq!(result[1].id, "beta");
    }

    // --- Gap #12: Multiple spec files ---

    #[test]
    fn scan_with_multiple_spec_files() {
        let tmp = tempfile::tempdir().unwrap();
        let change = tmp.path().join("my-change");
        fs::create_dir_all(change.join("specs/auth")).unwrap();
        fs::create_dir_all(change.join("specs/api")).unwrap();
        fs::write(change.join("tasks.md"), "Primary task content").unwrap();
        fs::write(change.join("specs/auth/spec.md"), "Auth spec details").unwrap();
        fs::write(change.join("specs/api/spec.md"), "API spec details").unwrap();

        let backend = OpenSpecBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert_eq!(result.len(), 1);

        let prompt = &result[0].prompt;
        assert!(
            prompt.contains("Primary task content"),
            "prompt should contain primary task"
        );
        assert!(
            prompt.contains("## Spec: api"),
            "prompt should contain api spec heading"
        );
        assert!(
            prompt.contains("API spec details"),
            "prompt should contain api spec content"
        );
        assert!(
            prompt.contains("## Spec: auth"),
            "prompt should contain auth spec heading"
        );
        assert!(
            prompt.contains("Auth spec details"),
            "prompt should contain auth spec content"
        );
    }

    #[test]
    fn frontmatter_excluded_from_prompt() {
        let tmp = tempfile::tempdir().unwrap();
        let change = tmp.path().join("fm-test");
        fs::create_dir(&change).unwrap();
        fs::write(
            change.join("tasks.md"),
            "---\npaw_cli: claude\n---\nActual prompt",
        )
        .unwrap();

        let backend = OpenSpecBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert!(!result[0].prompt.contains("---"));
        assert!(!result[0].prompt.contains("paw_cli"));
        assert_eq!(result[0].prompt, "Actual prompt");
    }

    // openspec-apply-boot-prompt backend tagging — every entry returned
    // from OpenSpec's scan() MUST carry `backend == SpecBackendKind::OpenSpec`
    // so downstream code can dispatch on backend without re-scanning.
    // v0-5-0-audit-cleanup tasks 6.1–6.3.

    #[test]
    fn scan_returns_entries_with_openspec_backend_tag() {
        let tmp = tempfile::tempdir().unwrap();
        for name in &["change-one", "change-two"] {
            let d = tmp.path().join(name);
            fs::create_dir(&d).unwrap();
            fs::write(d.join("tasks.md"), format!("task for {name}")).unwrap();
        }

        let backend = OpenSpecBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert_eq!(result.len(), 2, "expected two entries from two changes");
        for entry in &result {
            assert_eq!(
                entry.backend,
                SpecBackendKind::OpenSpec,
                "entry {} must carry backend == OpenSpec; got {:?}",
                entry.id,
                entry.backend,
            );
        }
    }

    #[test]
    fn scan_backend_tag_independent_of_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let change = tmp.path().join("frontmatter-change");
        fs::create_dir(&change).unwrap();
        fs::write(
            change.join("tasks.md"),
            "---\npaw_cli: gemini\n---\nDo the thing\n\nFiles owned:\n- src/a.rs\n- src/b.rs\n",
        )
        .unwrap();

        let backend = OpenSpecBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert_eq!(result.len(), 1);
        // Frontmatter + body-derived fields are present...
        assert_eq!(result[0].cli.as_deref(), Some("gemini"));
        assert!(result[0].owned_files.is_some());
        // ...but the backend tag is fixed to OpenSpec regardless.
        assert_eq!(
            result[0].backend,
            SpecBackendKind::OpenSpec,
            "frontmatter / body fields must NOT influence the backend tag",
        );
    }

    #[test]
    fn scan_single_entry_carries_openspec_tag() {
        let tmp = tempfile::tempdir().unwrap();
        let change = tmp.path().join("solo");
        fs::create_dir(&change).unwrap();
        fs::write(change.join("tasks.md"), "just one task").unwrap();

        let backend = OpenSpecBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert_eq!(result.len(), 1, "expected exactly one entry");
        assert_eq!(
            result[0].backend,
            SpecBackendKind::OpenSpec,
            "single-entry scan must still tag the entry as OpenSpec",
        );
    }

    // --- change authoring: name, scaffold, structural check ---

    #[test]
    fn change_name_validation_table() {
        for (name, valid) in [
            ("add-export", true),
            ("a1", true),
            ("x", true),
            ("Add_Export", false),
            ("../x", false),
            ("", false),
            ("a b", false),
            ("a/b", false),
            ("add.export", false),
        ] {
            assert_eq!(is_valid_change_name(name), valid, "name {name:?}");
        }
    }

    #[test]
    fn change_metadata_matches_openspec_new_change() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
        assert_eq!(
            change_metadata(date),
            "schema: spec-driven\ncreated: 2026-10-19\n"
        );
        assert_eq!(CHANGE_METADATA_FILE, ".openspec.yaml");
    }

    fn files(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(p, c)| ((*p).to_string(), (*c).to_string()))
            .collect()
    }

    const GOOD_DELTA: &str = "## ADDED Requirements\n\n### Requirement: Export\n\nThe system SHALL export.\n\n#### Scenario: Exports\n\n- **WHEN** x\n- **THEN** y\n";

    #[test]
    fn well_formed_change_has_no_findings() {
        let change = files(&[
            (".openspec.yaml", "schema: spec-driven\n"),
            ("proposal.md", "## Why\n"),
            ("specs/data-export/spec.md", GOOD_DELTA),
        ]);
        assert_eq!(check_change(&change), Vec::new());
    }

    #[test]
    fn missing_proposal_is_reported() {
        let findings = check_change(&files(&[("specs/data-export/spec.md", GOOD_DELTA)]));
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].file, "proposal.md");
        assert!(findings[0].message.contains("missing proposal.md"));
    }

    #[test]
    fn scaffold_only_change_reports_missing_proposal_and_delta() {
        let findings = check_change(&files(&[(".openspec.yaml", "schema: spec-driven\n")]));
        let names: Vec<&str> = findings.iter().map(|f| f.file.as_str()).collect();
        assert_eq!(names, ["proposal.md", "specs/<capability>/spec.md"]);
        assert!(
            findings[1]
                .message
                .contains("ADDED|MODIFIED|REMOVED|RENAMED")
        );
    }

    #[test]
    fn a_header_outside_a_delta_path_does_not_count() {
        for path in [
            "design.md",
            "specs/spec.md",
            "specs/a/b/spec.md",
            "specs/a/other.md",
        ] {
            let findings = check_change(&files(&[("proposal.md", "x"), (path, GOOD_DELTA)]));
            assert_eq!(findings.len(), 1, "{path}: {findings:?}");
            assert_eq!(findings[0].file, "specs/<capability>/spec.md", "{path}");
        }
    }

    #[test]
    fn second_line_shall_and_missing_scenario_name_the_delta_file() {
        let delta = "## MODIFIED Requirements\n\n### Requirement: Late\n\nSome prose first.\nThe system SHALL act.\n\n#### Scenario: S\n\n- WHEN a\n\n### Requirement: Bare\n\nThe system MUST act.\n";
        let findings = check_change(&files(&[
            ("proposal.md", "x"),
            ("specs/cap/spec.md", delta),
        ]));
        assert_eq!(findings.len(), 2, "{findings:?}");
        assert!(findings.iter().all(|f| f.file == "specs/cap/spec.md"));
        assert!(findings[0].message.contains("\"Late\""));
        assert!(findings[0].message.contains("first line"));
        assert!(findings[1].message.contains("\"Bare\""));
        assert!(findings[1].message.contains("#### Scenario:"));
    }

    #[test]
    fn empty_requirement_body_gets_the_shall_finding() {
        let delta = "## ADDED Requirements\n\n### Requirement: Empty\n";
        let findings = check_change(&files(&[("proposal.md", "x"), ("specs/c/spec.md", delta)]));
        assert_eq!(findings.len(), 2, "{findings:?}");
        assert!(findings[0].message.contains("first line"));
    }

    #[test]
    fn removed_and_renamed_requirements_are_not_checked() {
        let delta = "## REMOVED Requirements\n\n### Requirement: Gone\n\n**Reason**: obsolete\n";
        let findings = check_change(&files(&[("proposal.md", "x"), ("specs/c/spec.md", delta)]));
        assert_eq!(findings, Vec::new());
    }

    #[test]
    fn headers_inside_fenced_code_are_ignored() {
        // The only delta header sits in a fence: no real delta header.
        let fenced = "```\n## ADDED Requirements\n### Requirement: X\n```\n";
        let findings = check_change(&files(&[("proposal.md", "x"), ("specs/c/spec.md", fenced)]));
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].file, "specs/<capability>/spec.md");

        // A fenced requirement header inside a real section is not a requirement.
        let mixed = format!("{GOOD_DELTA}\n```\n### Requirement: Fake\n```\n");
        let findings = check_change(&files(&[("proposal.md", "x"), ("specs/c/spec.md", &mixed)]));
        assert_eq!(findings, Vec::new());
    }

    #[test]
    fn findings_follow_sorted_delta_path_order() {
        let bad = "## ADDED Requirements\n\n### Requirement: R\n\nThe system SHALL x.\n";
        let findings = check_change(&files(&[
            ("proposal.md", "x"),
            ("specs/zeta/spec.md", bad),
            ("specs/alpha/spec.md", bad),
        ]));
        let paths: Vec<&str> = findings.iter().map(|f| f.file.as_str()).collect();
        assert_eq!(paths, ["specs/alpha/spec.md", "specs/zeta/spec.md"]);
    }
}

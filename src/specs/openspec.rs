//! OpenSpec-format backend for spec scanning.
//!
//! Scans `openspec/changes/` for pending changes, extracts prompt content
//! from `tasks.md` and supplementary `specs/` files, and parses optional
//! frontmatter fields and file ownership declarations.

use std::fmt::Write;
use std::fs;
use std::path::Path;

use crate::error::PawError;
use crate::specs::{SpecBackend, SpecEntry};

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
}

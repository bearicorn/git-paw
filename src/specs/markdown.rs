//! Markdown-format backend for spec scanning.

use std::fs;
use std::path::Path;

use crate::error::PawError;
use crate::specs::{SpecBackend, SpecBackendKind, SpecEntry, parse_frontmatter};

/// Backend for scanning Markdown files with YAML frontmatter.
///
/// Scans immediate children of the configured directory for `.md` files
/// containing `paw_status: pending` frontmatter.
#[derive(Debug)]
pub(crate) struct MarkdownBackend;

impl SpecBackend for MarkdownBackend {
    fn scan(&self, dir: &Path) -> Result<Vec<SpecEntry>, PawError> {
        let mut entries = Vec::new();

        let read_dir = fs::read_dir(dir)
            .map_err(|e| PawError::SpecError(format!("read dir {}: {e}", dir.display())))?;

        for entry in read_dir {
            let entry = entry.map_err(|e| PawError::SpecError(format!("read entry: {e}")))?;

            let path = entry.path();

            // Skip directories and non-.md files
            if path.is_dir() {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }

            let content = fs::read_to_string(&path)
                .map_err(|e| PawError::SpecError(format!("read {}: {e}", path.display())))?;

            let (frontmatter, body) = parse_frontmatter(&content);

            let Some(fields) = frontmatter else {
                continue;
            };

            // Must have paw_status = "pending"
            match fields.get("paw_status").map(String::as_str) {
                Some("pending") => {}
                _ => continue,
            }

            let id = fields
                .get("paw_branch")
                .filter(|s| !s.is_empty())
                .cloned()
                .unwrap_or_else(|| {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string()
                });

            let cli = fields.get("paw_cli").filter(|s| !s.is_empty()).cloned();

            entries.push(SpecEntry {
                id,
                backend: SpecBackendKind::Markdown,
                branch: String::new(), // filled in by scan_specs
                cli,
                prompt: body.to_string(),
                owned_files: None,
            });
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_spec(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap();
    }

    fn pending_spec(branch: Option<&str>, cli: Option<&str>, body: &str) -> String {
        use std::fmt::Write;
        let mut fm = String::from("---\npaw_status: pending\n");
        if let Some(b) = branch {
            let _ = writeln!(fm, "paw_branch: {b}");
        }
        if let Some(c) = cli {
            let _ = writeln!(fm, "paw_cli: {c}");
        }
        fm.push_str("---\n");
        fm.push_str(body);
        fm
    }

    #[test]
    fn scan_three_pending_files() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 1..=3 {
            write_spec(
                tmp.path(),
                &format!("spec-{i}.md"),
                &pending_spec(None, None, "body"),
            );
        }
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn scan_only_pending_returned() {
        let tmp = tempfile::tempdir().unwrap();
        write_spec(tmp.path(), "pending.md", &pending_spec(None, None, "body"));
        write_spec(tmp.path(), "done.md", "---\npaw_status: done\n---\nbody");
        write_spec(
            tmp.path(),
            "in-progress.md",
            "---\npaw_status: in-progress\n---\nbody",
        );
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn scan_files_without_frontmatter_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        write_spec(tmp.path(), "no-fm.md", "# Just a readme\nNo frontmatter.");
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn scan_non_markdown_files_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        write_spec(tmp.path(), "spec.txt", &pending_spec(None, None, "body"));
        write_spec(tmp.path(), "config.toml", &pending_spec(None, None, "body"));
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn scan_empty_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn scan_subdirectories_not_traversed() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("subdir");
        fs::create_dir(&sub).unwrap();
        write_spec(&sub, "nested.md", &pending_spec(None, None, "body"));
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn id_from_paw_branch() {
        let tmp = tempfile::tempdir().unwrap();
        write_spec(
            tmp.path(),
            "whatever.md",
            &pending_spec(Some("add-auth"), None, "body"),
        );
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert_eq!(entries[0].id, "add-auth");
    }

    #[test]
    fn id_from_filename_stem_when_no_paw_branch() {
        let tmp = tempfile::tempdir().unwrap();
        write_spec(
            tmp.path(),
            "fix-session.md",
            &pending_spec(None, None, "body"),
        );
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert_eq!(entries[0].id, "fix-session");
    }

    #[test]
    fn cli_present() {
        let tmp = tempfile::tempdir().unwrap();
        write_spec(
            tmp.path(),
            "spec.md",
            &pending_spec(None, Some("gemini"), "body"),
        );
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert_eq!(entries[0].cli.as_deref(), Some("gemini"));
    }

    #[test]
    fn cli_absent() {
        let tmp = tempfile::tempdir().unwrap();
        write_spec(tmp.path(), "spec.md", &pending_spec(None, None, "body"));
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert!(entries[0].cli.is_none());
    }

    #[test]
    fn prompt_is_body_after_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let body = "## Auth\n\nImplement JWT.\n";
        write_spec(tmp.path(), "spec.md", &pending_spec(None, None, body));
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert_eq!(entries[0].prompt, body);
    }

    #[test]
    fn prompt_empty_when_only_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        write_spec(tmp.path(), "spec.md", "---\npaw_status: pending\n---\n");
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert!(entries[0].prompt.is_empty());
    }

    #[test]
    fn unknown_frontmatter_fields_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        write_spec(
            tmp.path(),
            "spec.md",
            "---\npaw_status: pending\nauthor: alice\npriority: high\n---\nbody",
        );
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn all_three_frontmatter_fields_mapped() {
        let tmp = tempfile::tempdir().unwrap();
        write_spec(
            tmp.path(),
            "spec.md",
            &pending_spec(Some("add-auth"), Some("claude"), "the prompt"),
        );
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert_eq!(entries[0].id, "add-auth");
        assert_eq!(entries[0].cli.as_deref(), Some("claude"));
        assert_eq!(entries[0].prompt, "the prompt");
        assert!(entries[0].owned_files.is_none());
    }

    // openspec-apply-boot-prompt backend tagging — every entry returned
    // from MarkdownBackend's scan() MUST carry
    // `backend == SpecBackendKind::Markdown` so downstream code can
    // dispatch on backend without re-scanning. v0-5-0-audit-cleanup tasks
    // 6.4–6.6.

    #[test]
    fn scan_returns_entries_with_markdown_backend_tag() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 1..=2 {
            write_spec(
                tmp.path(),
                &format!("spec-{i}.md"),
                &pending_spec(None, None, "body"),
            );
        }
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert_eq!(entries.len(), 2, "expected two pending entries");
        for entry in &entries {
            assert_eq!(
                entry.backend,
                SpecBackendKind::Markdown,
                "entry {} must carry backend == Markdown; got {:?}",
                entry.id,
                entry.backend,
            );
        }
    }

    #[test]
    fn scan_filters_non_pending_then_applies_tag() {
        let tmp = tempfile::tempdir().unwrap();
        write_spec(tmp.path(), "pending.md", &pending_spec(None, None, "body"));
        write_spec(tmp.path(), "done.md", "---\npaw_status: done\n---\nbody");
        write_spec(
            tmp.path(),
            "in-progress.md",
            "---\npaw_status: in-progress\n---\nbody",
        );

        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert_eq!(
            entries.len(),
            1,
            "non-pending entries must be filtered out before tagging",
        );
        assert_eq!(
            entries[0].backend,
            SpecBackendKind::Markdown,
            "the surviving pending entry must carry the Markdown backend tag",
        );
    }

    #[test]
    fn scan_single_pending_carries_markdown_tag() {
        let tmp = tempfile::tempdir().unwrap();
        write_spec(tmp.path(), "solo.md", &pending_spec(None, None, "body"));
        let entries = MarkdownBackend.scan(tmp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].backend,
            SpecBackendKind::Markdown,
            "single-entry scan must still tag the entry as Markdown",
        );
    }
}

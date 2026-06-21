//! Parses `.git-paw/session-learnings.md` into structured sections.
//!
//! The learnings file (produced by the v0.5.0 learnings aggregator) is a
//! Markdown document with `### <section>` headings under timestamped
//! `## Session Learnings — <ts>` blocks. We flatten every `### ` section into
//! a `category` + `entries` record. When the file is absent we return the four
//! canonical v0.5.0 sections as empty arrays so the client sees a stable shape.

#[cfg(test)]
use std::path::Path;

use rmcp::schemars;
use serde::Serialize;

use crate::mcp::RepoContext;

/// The four canonical v0.5.0 learning sections, in display order.
const CANONICAL_SECTIONS: &[&str] = &[
    "Conflict events",
    "Where agents got stuck",
    "Recovery cycles",
    "Permission patterns",
];

/// One parsed learnings section.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema, PartialEq, Eq)]
pub struct LearningSection {
    /// Section heading (e.g. "Conflict events").
    pub category: String,
    /// Body entries — non-empty content lines with leading list markers
    /// stripped.
    pub entries: Vec<String>,
}

/// Reads and parses the repository's session-learnings file, or returns the
/// canonical empty sections when no file exists.
#[must_use]
pub fn learnings(ctx: &RepoContext) -> Vec<LearningSection> {
    let path = ctx
        .git_paw_dir
        .as_ref()
        .map(|d| d.join("session-learnings.md"));
    match path
        .as_deref()
        .and_then(|p| std::fs::read_to_string(p).ok())
    {
        Some(content) => parse(&content),
        None => empty_sections(),
    }
}

fn empty_sections() -> Vec<LearningSection> {
    CANONICAL_SECTIONS
        .iter()
        .map(|c| LearningSection {
            category: (*c).to_string(),
            entries: Vec::new(),
        })
        .collect()
}

/// Parses learnings Markdown into sections, merging duplicate `### ` headings
/// across timestamped blocks. Always includes the canonical sections (empty if
/// absent in the file) so the shape is stable.
fn parse(content: &str) -> Vec<LearningSection> {
    // Preserve first-seen order of headings while merging entries.
    let mut order: Vec<String> = CANONICAL_SECTIONS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let mut map: std::collections::HashMap<String, Vec<String>> =
        order.iter().map(|c| (c.clone(), Vec::new())).collect();

    let mut current: Option<String> = None;
    for line in content.lines() {
        if let Some(heading) = line.strip_prefix("### ") {
            let heading = heading.trim().to_string();
            if !map.contains_key(&heading) {
                map.insert(heading.clone(), Vec::new());
                order.push(heading.clone());
            }
            current = Some(heading);
            continue;
        }
        // A new timestamped block resets the current section.
        if line.starts_with("## ") {
            current = None;
            continue;
        }
        if let Some(section) = current.as_ref() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let entry = trimmed
                .strip_prefix("- ")
                .or_else(|| trimmed.strip_prefix("* "))
                .unwrap_or(trimmed)
                .to_string();
            map.get_mut(section).expect("section present").push(entry);
        }
    }

    order
        .into_iter()
        .map(|category| {
            let entries = map.remove(&category).unwrap_or_default();
            LearningSection { category, entries }
        })
        .collect()
}

/// Resolves the path that [`learnings`] would read (for callers wanting to
/// report it). Returns `None` when the repo has no `.git-paw/` dir.
#[must_use]
pub fn learnings_path(ctx: &RepoContext) -> Option<std::path::PathBuf> {
    ctx.git_paw_dir
        .as_ref()
        .map(|d| d.join("session-learnings.md"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with(dir: Option<&Path>) -> RepoContext {
        RepoContext {
            root: dir.map_or_else(|| std::path::PathBuf::from("/tmp"), Path::to_path_buf),
            git_paw_dir: dir.map(Path::to_path_buf),
            broker_url: None,
            server_name: "git-paw".to_string(),
        }
    }

    #[test]
    fn missing_file_returns_canonical_empty_sections() {
        let sections = learnings(&ctx_with(None));
        assert_eq!(sections.len(), 4);
        assert_eq!(sections[0].category, "Conflict events");
        assert!(sections.iter().all(|s| s.entries.is_empty()));
    }

    #[test]
    fn parses_sections_and_entries() {
        let md = "## Session Learnings — 2026-01-01\n\n\
                  ### Conflict events\n- forward overlap on src/a.rs\n\n\
                  ### Permission patterns\n- approved `cargo test`\n- approved `just check`\n";
        let sections = parse(md);
        let conflict = sections
            .iter()
            .find(|s| s.category == "Conflict events")
            .unwrap();
        assert_eq!(conflict.entries, vec!["forward overlap on src/a.rs"]);
        let perms = sections
            .iter()
            .find(|s| s.category == "Permission patterns")
            .unwrap();
        assert_eq!(perms.entries.len(), 2);
        // Canonical sections still present even when absent from the file.
        assert!(sections.iter().any(|s| s.category == "Recovery cycles"));
    }

    #[test]
    fn non_canonical_qualitative_section_is_included() {
        let md = "### Documentation gaps\n- AGENTS.md missing MCP dep note\n";
        let sections = parse(md);
        let doc = sections.iter().find(|s| s.category == "Documentation gaps");
        assert!(doc.is_some(), "qualitative sections should be parsed too");
        assert_eq!(doc.unwrap().entries.len(), 1);
    }

    #[test]
    fn reads_from_git_paw_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("session-learnings.md"),
            "### Conflict events\n- something\n",
        )
        .unwrap();
        let sections = learnings(&ctx_with(Some(tmp.path())));
        let conflict = sections
            .iter()
            .find(|s| s.category == "Conflict events")
            .unwrap();
        assert_eq!(conflict.entries, vec!["something"]);
    }
}

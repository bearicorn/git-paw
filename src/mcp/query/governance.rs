//! Governance-document reads.
//!
//! Serves the documents configured under `[governance]` in
//! `.git-paw/config.toml`. Files are read lazily (only when a tool is called).
//! Unset paths degrade to `None` / empty; a configured path that exists but
//! cannot be read surfaces as an error (see [`super::read_optional_doc`]).

use std::path::Path;

use rmcp::schemars;
use serde::Serialize;

use crate::config::{self, GovernanceConfig};
use crate::error::PawError;

use super::{read_optional_doc, resolve_under_root};

/// Loads the resolved governance configuration for the repository.
///
/// Auto-wiring (e.g. populating `constitution` from a detected `.specify/`)
/// is applied by [`config::load_config`].
pub fn load(repo_root: &Path) -> Result<GovernanceConfig, PawError> {
    Ok(config::load_config(repo_root, None)?.governance)
}

/// One ADR entry.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema, PartialEq, Eq)]
pub struct Adr {
    /// ADR identifier parsed from the filename (e.g. "ADR-0007").
    pub id: String,
    /// Title parsed from the first heading, falling back to the filename stem.
    pub title: String,
    /// Path relative to the repository root.
    pub path: String,
    /// Status parsed from a `Status:`/`status:` line, or "unknown".
    pub status: String,
}

/// Parses an ADR's title and status from its Markdown content.
fn parse_adr_meta(content: &str, fallback_title: &str) -> (String, String) {
    let title = content
        .lines()
        .find_map(|l| l.trim().strip_prefix("# ").map(str::trim))
        .unwrap_or(fallback_title)
        .to_string();
    let status = content
        .lines()
        .find_map(|l| {
            let t = l.trim();
            let lower = t.to_ascii_lowercase();
            lower
                .strip_prefix("status:")
                .or_else(|| lower.strip_prefix("- status:"))
                .or_else(|| lower.strip_prefix("**status:**"))
                .map(|_| {
                    // Recover original-case value after the colon.
                    t.split_once(':').map_or("", |x| x.1).trim().to_string()
                })
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    (title, status)
}

/// Lists ADR files under the configured ADR directory. Empty when unset or
/// the directory is missing.
#[must_use]
pub fn adrs(repo_root: &Path, gov: &GovernanceConfig) -> Vec<Adr> {
    let Some(dir) = gov.adr.as_ref() else {
        return Vec::new();
    };
    let dir = resolve_under_root(repo_root, dir);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.to_ascii_lowercase().ends_with(".md") {
            continue;
        }
        let stem = name.trim_end_matches(".md");
        let id = stem
            .split(['-', '_', ' '])
            .take(2)
            .collect::<Vec<_>>()
            .join("-");
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let (title, status) = parse_adr_meta(&content, stem);
        let rel = path
            .strip_prefix(repo_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        out.push(Adr {
            id: if id.is_empty() { stem.to_string() } else { id },
            title,
            path: rel,
            status,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// A single ADR with full content, matched by a case-insensitive query over
/// id, title, and body.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct AdrDetail {
    /// ADR id.
    pub id: String,
    /// Path relative to the repository root.
    pub path: String,
    /// Full Markdown content.
    pub content: String,
}

/// Finds the first ADR matching `query`, or `None`.
#[must_use]
pub fn adr(repo_root: &Path, gov: &GovernanceConfig, query: &str) -> Option<AdrDetail> {
    let needle = query.to_ascii_lowercase();
    for entry in adrs(repo_root, gov) {
        let path = resolve_under_root(repo_root, Path::new(&entry.path));
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let hay = format!("{} {} {}", entry.id, entry.title, content).to_ascii_lowercase();
        if hay.contains(&needle) {
            return Some(AdrDetail {
                id: entry.id,
                path: entry.path,
                content,
            });
        }
    }
    None
}

/// Reads a single configured doc (`test_strategy`, `security`, `dod`,
/// `constitution`). Returns `Ok(None)` when unset, `Err` when unreadable.
pub fn single_doc(repo_root: &Path, configured: Option<&Path>) -> Result<Option<String>, PawError> {
    read_optional_doc(repo_root, configured)
}

/// One Definition-of-Done checklist item.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema, PartialEq, Eq)]
pub struct DodItem {
    /// Item text (after the checkbox marker).
    pub text: String,
    /// Completion state as written in the `DoD` file.
    pub complete: bool,
}

/// Parses the configured `DoD` file into checklist items with their literal
/// completion state. Returns `Ok(None)` when no `DoD` is configured, `Err` when
/// the configured file is unreadable.
pub fn check_dod(
    repo_root: &Path,
    gov: &GovernanceConfig,
) -> Result<Option<Vec<DodItem>>, PawError> {
    let Some(content) = read_optional_doc(repo_root, gov.dod.as_deref())? else {
        return Ok(None);
    };
    let items = content
        .lines()
        .filter_map(|line| {
            let t = line.trim();
            let rest = t.strip_prefix("- [").or_else(|| t.strip_prefix("* ["))?;
            let mark = rest.chars().next()?;
            let text = rest.get(2..).unwrap_or("").trim().to_string();
            Some(DodItem {
                complete: mark == 'x' || mark == 'X',
                text,
            })
        })
        .collect();
    Ok(Some(items))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_adr_meta_extracts_title_and_status() {
        let md = "# ADR-0007: Choose tokio\n\nStatus: Accepted\n\nContext...";
        let (title, status) = parse_adr_meta(md, "fallback");
        assert_eq!(title, "ADR-0007: Choose tokio");
        assert_eq!(status, "Accepted");
    }

    #[test]
    fn parse_adr_meta_defaults_status_unknown() {
        let (_t, status) = parse_adr_meta("# Title only\n", "fallback");
        assert_eq!(status, "unknown");
    }

    #[test]
    fn adrs_lists_and_parses_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let adr_dir = tmp.path().join("docs/adr");
        std::fs::create_dir_all(&adr_dir).unwrap();
        std::fs::write(
            adr_dir.join("ADR-0007-tokio.md"),
            "# ADR-0007: Choose tokio\nStatus: Accepted\n",
        )
        .unwrap();
        let gov = GovernanceConfig {
            adr: Some(std::path::PathBuf::from("docs/adr")),
            ..Default::default()
        };
        let list = adrs(tmp.path(), &gov);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "ADR-0007");
        assert_eq!(list[0].status, "Accepted");

        let found = adr(tmp.path(), &gov, "tokio").expect("query matches");
        assert!(found.content.contains("Choose tokio"));
    }

    #[test]
    fn adrs_empty_when_unset() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(adrs(tmp.path(), &GovernanceConfig::default()).is_empty());
    }

    #[test]
    fn check_dod_parses_checkbox_states() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("dod.md"),
            "- [x] tests pass\n- [ ] docs updated\n",
        )
        .unwrap();
        let gov = GovernanceConfig {
            dod: Some(std::path::PathBuf::from("dod.md")),
            ..Default::default()
        };
        let items = check_dod(tmp.path(), &gov).unwrap().unwrap();
        assert_eq!(items.len(), 2);
        assert!(items[0].complete);
        assert!(!items[1].complete);
    }

    #[test]
    fn check_dod_unset_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            check_dod(tmp.path(), &GovernanceConfig::default())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn single_doc_unreadable_is_error() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = std::path::PathBuf::from("does-not-exist.md");
        let err = single_doc(tmp.path(), Some(&missing));
        assert!(
            err.is_err(),
            "configured-but-missing file is misconfiguration"
        );
    }
}

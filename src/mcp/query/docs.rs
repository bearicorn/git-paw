//! Documentation reads for the MCP server.
//!
//! Serves the repository's own documentation — the README and the
//! documentation tree — driven by the bring-your-own `[governance].readme`
//! and `[governance].docs` configuration. Locations are configured, never
//! hardcoded: an unset path degrades to a null/empty result rather than a
//! transport error (the degradation contract, design D4).
//!
//! [`read_doc`] is confined to the configured documentation directory: the
//! requested path is resolved under that directory, canonicalised, and
//! verified to still lie within it, so `..`/absolute escapes are refused
//! before any file outside the directory is read.

use std::path::Path;

use rmcp::schemars;
use serde::Serialize;

use crate::config::GovernanceConfig;
use crate::error::PawError;

use super::resolve_under_root;

/// Reads the configured README.
///
/// - `[governance].readme` unset → `Ok(None)` (graceful degradation).
/// - configured but the file is absent → `Ok(None)` (the README is optional).
/// - configured + readable → `Ok(Some(content))`.
/// - configured + present-but-unreadable (e.g. a permission error) → `Err`,
///   so the tool layer can surface the misconfiguration to the client.
pub fn read_readme(repo_root: &Path, gov: &GovernanceConfig) -> Result<Option<String>, PawError> {
    let Some(rel) = gov.readme.as_deref() else {
        return Ok(None);
    };
    let path = resolve_under_root(repo_root, rel);
    match std::fs::read_to_string(&path) {
        Ok(content) => Ok(Some(content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(PawError::McpError(format!(
            "configured readme path {} could not be read: {e}",
            path.display()
        ))),
    }
}

/// One documentation entry returned by [`list_docs`].
#[derive(Debug, Clone, Serialize, schemars::JsonSchema, PartialEq, Eq)]
pub struct DocEntry {
    /// Path relative to the configured documentation directory (so it feeds
    /// directly back into [`read_doc`]). Uses forward slashes.
    pub path: String,
}

/// Recursively collects `*.md` files under `dir`, pushing their paths relative
/// to `base` (forward-slash normalised) into `out`.
fn collect_md(dir: &Path, base: &Path, out: &mut Vec<DocEntry>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_md(&path, base, out);
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.to_ascii_lowercase().ends_with(".md") {
            continue;
        }
        let Ok(rel) = path.strip_prefix(base) else {
            continue;
        };
        let rel = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        out.push(DocEntry { path: rel });
    }
}

/// Lists Markdown documents under the configured documentation directory.
///
/// Returns an empty list when `[governance].docs` is unset or the directory
/// is absent (graceful degradation). Each entry's path is relative to the
/// configured documentation directory and sorted lexicographically.
#[must_use]
pub fn list_docs(repo_root: &Path, gov: &GovernanceConfig) -> Vec<DocEntry> {
    let Some(dir) = gov.docs.as_ref() else {
        return Vec::new();
    };
    let dir = resolve_under_root(repo_root, dir);
    let mut out = Vec::new();
    collect_md(&dir, &dir, &mut out);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Reads one document confined to the configured documentation directory.
///
/// The requested `rel_path` is resolved under the configured documentation
/// directory, then canonicalised and checked to still lie within that
/// directory. Any path that escapes the directory (`..`, an absolute path, a
/// symlink target outside the tree) is refused with `Ok(None)` — no file
/// outside the directory is ever read.
///
/// - `[governance].docs` unset → `Ok(None)`.
/// - requested document absent → `Ok(None)`.
/// - traversal/escape → `Ok(None)` (refused; the tool layer attaches a
///   message).
/// - confined + readable → `Ok(Some(content))`.
/// - confined + present-but-unreadable → `Err`.
pub fn read_doc(
    repo_root: &Path,
    gov: &GovernanceConfig,
    rel_path: &str,
) -> Result<Option<String>, PawError> {
    let Some(dir) = gov.docs.as_ref() else {
        return Ok(None);
    };
    let dir = resolve_under_root(repo_root, dir);
    // The directory must canonicalise (exist) for confinement to be
    // meaningful; an absent docs dir degrades to None.
    let Ok(canonical_dir) = dir.canonicalize() else {
        return Ok(None);
    };

    let requested = dir.join(rel_path);
    // Canonicalise the requested path; a non-existent file (or a broken
    // traversal target) yields None rather than an error.
    let Ok(canonical) = requested.canonicalize() else {
        return Ok(None);
    };
    // Confinement check: the canonical target must stay within the canonical
    // documentation directory. This rejects `..`, absolute paths, and symlink
    // escapes alike.
    if !canonical.starts_with(&canonical_dir) {
        return Ok(None);
    }

    match std::fs::read_to_string(&canonical) {
        Ok(content) => Ok(Some(content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(PawError::McpError(format!(
            "configured doc {} could not be read: {e}",
            canonical.display()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn read_readme_returns_content_when_configured_and_present() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("README.md"), "# Hello\nbody").unwrap();
        let gov = GovernanceConfig {
            readme: Some(PathBuf::from("README.md")),
            ..Default::default()
        };
        let content = read_readme(tmp.path(), &gov).unwrap();
        assert_eq!(content.as_deref(), Some("# Hello\nbody"));
    }

    #[test]
    fn read_readme_none_when_unconfigured() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            read_readme(tmp.path(), &GovernanceConfig::default())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn read_readme_none_when_configured_but_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let gov = GovernanceConfig {
            readme: Some(PathBuf::from("README.md")),
            ..Default::default()
        };
        // Configured but the file does not exist → graceful null, not an error.
        assert!(read_readme(tmp.path(), &gov).unwrap().is_none());
    }

    #[test]
    fn list_docs_empty_when_unconfigured() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(list_docs(tmp.path(), &GovernanceConfig::default()).is_empty());
    }

    #[test]
    fn list_docs_empty_when_dir_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let gov = GovernanceConfig {
            docs: Some(PathBuf::from("docs/src")),
            ..Default::default()
        };
        assert!(list_docs(tmp.path(), &gov).is_empty());
    }

    #[test]
    fn list_docs_enumerates_nested_markdown_relative_to_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let docs = tmp.path().join("docs/src");
        std::fs::create_dir_all(docs.join("user-guide")).unwrap();
        std::fs::write(docs.join("intro.md"), "# Intro").unwrap();
        std::fs::write(docs.join("user-guide/mcp.md"), "# MCP").unwrap();
        std::fs::write(docs.join("not-a-doc.txt"), "ignored").unwrap();
        let gov = GovernanceConfig {
            docs: Some(PathBuf::from("docs/src")),
            ..Default::default()
        };
        let list = list_docs(tmp.path(), &gov);
        let paths: Vec<&str> = list.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(paths, vec!["intro.md", "user-guide/mcp.md"]);
    }

    #[test]
    fn read_doc_happy_path() {
        let tmp = tempfile::tempdir().unwrap();
        let docs = tmp.path().join("docs/src");
        std::fs::create_dir_all(docs.join("user-guide")).unwrap();
        std::fs::write(docs.join("user-guide/mcp.md"), "# MCP guide").unwrap();
        let gov = GovernanceConfig {
            docs: Some(PathBuf::from("docs/src")),
            ..Default::default()
        };
        let content = read_doc(tmp.path(), &gov, "user-guide/mcp.md").unwrap();
        assert_eq!(content.as_deref(), Some("# MCP guide"));
    }

    #[test]
    fn read_doc_none_when_unconfigured() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            read_doc(tmp.path(), &GovernanceConfig::default(), "x.md")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn read_doc_rejects_dotdot_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        let docs = tmp.path().join("docs/src");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("ok.md"), "# ok").unwrap();
        // A secret file outside the docs dir.
        std::fs::write(tmp.path().join("secret.txt"), "TOPSECRET").unwrap();
        let gov = GovernanceConfig {
            docs: Some(PathBuf::from("docs/src")),
            ..Default::default()
        };
        // Even though ../../secret.txt exists, confinement refuses it.
        let escaped = read_doc(tmp.path(), &gov, "../../secret.txt").unwrap();
        assert!(escaped.is_none(), "traversal must be refused");
    }

    #[test]
    fn read_doc_rejects_absolute_path() {
        let tmp = tempfile::tempdir().unwrap();
        let docs = tmp.path().join("docs/src");
        std::fs::create_dir_all(&docs).unwrap();
        let secret = tmp.path().join("secret.txt");
        std::fs::write(&secret, "TOPSECRET").unwrap();
        let gov = GovernanceConfig {
            docs: Some(PathBuf::from("docs/src")),
            ..Default::default()
        };
        let abs = secret.to_string_lossy().into_owned();
        let escaped = read_doc(tmp.path(), &gov, &abs).unwrap();
        assert!(escaped.is_none(), "absolute escape must be refused");
    }
}

//! Source-browsing tools: `list_files`, `read_file`, `search_code`.
//!
//! These let an MCP client explore the repository's local working tree
//! (tracked plus untracked-but-not-ignored files; gitignored paths excluded)
//! and trace logic across files: `list_files` to enumerate, `search_code` to
//! find a symbol, `read_file` to read it. `read_file` is confined to the
//! repository root and refuses gitignored paths; a refused read returns null
//! content with a `message`, not a transport error.

use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::error::PawError;
use crate::mcp::query;
use crate::mcp::query::source::CodeMatch;
use crate::mcp::server::GitPawMcpServer;

/// Maps an internal error to an MCP protocol error. Takes the error by value
/// so it composes directly with `Result::map_err`.
#[allow(clippy::needless_pass_by_value)]
fn to_err(e: PawError) -> ErrorData {
    ErrorData::internal_error(e.to_string(), None)
}

/// Parameters for [`GitPawMcpServer::list_files`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListFilesParams {
    /// Optional subpath (relative to the repository root) to scope the listing
    /// to. Omit to list the whole working tree.
    #[serde(default)]
    pub subpath: Option<String>,
}

/// Parameters for [`GitPawMcpServer::read_file`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReadFileParams {
    /// File path relative to the repository root (e.g. `src/main.rs`). Paths
    /// escaping the root or naming a gitignored file are refused.
    pub path: String,
}

/// Parameters for [`GitPawMcpServer::search_code`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchCodeParams {
    /// String to search for across the working tree's file contents.
    pub query: String,
    /// Optional subpath (relative to the repository root) to scope the search
    /// to. Omit to search the whole working tree.
    #[serde(default)]
    pub subpath: Option<String>,
}

/// Response for `list_files`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct FilesListResponse {
    /// Working-tree files (tracked plus untracked-not-ignored), relative to
    /// the repository root. Empty when not a git repository.
    pub files: Vec<String>,
}

/// Response for `read_file`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct ReadFileResponse {
    /// File content from the local working tree, or null when refused or
    /// absent.
    pub content: Option<String>,
    /// Human-readable note when `content` is null (refused traversal, a
    /// gitignored path, or a missing file). Absent when content is present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Response for `search_code`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SearchResponse {
    /// Matches, each with path, 1-based line number, and the matching line.
    pub matches: Vec<CodeMatch>,
    /// Whether the result was truncated at the internal match cap.
    pub truncated: bool,
}

#[tool_router(router = source_router, vis = "pub(crate)")]
impl GitPawMcpServer {
    /// `list_files` — working-tree files, gitignored paths excluded.
    #[tool(
        description = "List the repository's working-tree files (tracked plus \
                       untracked-but-not-ignored), optionally scoped to a subpath. Gitignored \
                       paths (build artifacts, secrets) are excluded. Paths are relative to the \
                       repository root. Empty when not a git repository."
    )]
    pub(crate) fn list_files(
        &self,
        Parameters(p): Parameters<ListFilesParams>,
    ) -> Json<FilesListResponse> {
        Json(FilesListResponse {
            files: query::source::list_files(&self.ctx.root, p.subpath.as_deref()),
        })
    }

    /// `read_file` — one file's content from the local working tree.
    #[tool(
        description = "Return one file's content from the local working tree, by path relative to \
                       the repository root. Confined to the repository root: a path escaping it \
                       (e.g. \"../\", an absolute path) is refused with null content and a message, \
                       not a read outside the root. Gitignored paths are also refused. Null content \
                       with a message when the file is absent."
    )]
    pub(crate) fn read_file(
        &self,
        Parameters(p): Parameters<ReadFileParams>,
    ) -> Result<Json<ReadFileResponse>, ErrorData> {
        let outcome = query::source::read_file(&self.ctx.root, &p.path).map_err(to_err)?;
        Ok(Json(ReadFileResponse {
            content: outcome.content,
            message: outcome.message,
        }))
    }

    /// `search_code` — search file contents across the working tree.
    #[tool(
        description = "Search file contents across the repository's working tree (tracked plus \
                       untracked-but-not-ignored, binaries skipped), optionally scoped to a \
                       subpath. Returns matches as { path, line_number, line }, capped with a \
                       `truncated` flag. Empty when there are no matches or not a git repository."
    )]
    pub(crate) fn search_code(
        &self,
        Parameters(p): Parameters<SearchCodeParams>,
    ) -> Json<SearchResponse> {
        let (matches, truncated) =
            query::source::search_code(&self.ctx.root, &p.query, p.subpath.as_deref());
        Json(SearchResponse { matches, truncated })
    }
}

#[cfg(test)]
mod tests {
    use crate::mcp::RepoContext;
    use crate::mcp::server::GitPawMcpServer;
    use rmcp::handler::server::wrapper::Parameters;
    use std::path::Path;
    use std::process::Command;

    fn server_for(root: std::path::PathBuf) -> GitPawMcpServer {
        GitPawMcpServer::new(RepoContext {
            root,
            git_paw_dir: None,
            broker_url: None,
            server_name: "git-paw".to_string(),
        })
    }

    fn git_run(dir: &Path, args: &[&str]) {
        assert!(
            Command::new("git")
                .current_dir(dir)
                .args(args)
                .status()
                .unwrap()
                .success(),
            "git {args:?} failed"
        );
    }

    fn fixture() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.email", "t@example.com"],
            vec!["config", "user.name", "Test"],
        ] {
            git_run(dir, &args);
        }
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/main.rs"),
            "fn main() {\n    register_watch_target_http();\n}\n",
        )
        .unwrap();
        std::fs::write(dir.join(".gitignore"), "target/\n").unwrap();
        git_run(dir, &["add", "src/main.rs", ".gitignore"]);
        git_run(dir, &["commit", "-q", "-m", "first"]);
        std::fs::create_dir_all(dir.join("target/debug")).unwrap();
        std::fs::write(dir.join("target/debug/foo"), "build artifact\n").unwrap();
        tmp
    }

    #[test]
    fn list_files_happy_path() {
        let tmp = fixture();
        let server = server_for(tmp.path().canonicalize().unwrap());
        let resp = server.list_files(Parameters(super::ListFilesParams { subpath: None }));
        assert!(resp.0.files.iter().any(|f| f == "src/main.rs"));
        assert!(!resp.0.files.iter().any(|f| f.starts_with("target/")));
    }

    #[test]
    fn list_files_empty_when_not_git() {
        let tmp = tempfile::tempdir().unwrap();
        let server = server_for(tmp.path().canonicalize().unwrap());
        let resp = server.list_files(Parameters(super::ListFilesParams { subpath: None }));
        assert!(resp.0.files.is_empty());
    }

    #[test]
    fn read_file_happy_path() {
        let tmp = fixture();
        let server = server_for(tmp.path().canonicalize().unwrap());
        let resp = server
            .read_file(Parameters(super::ReadFileParams {
                path: "src/main.rs".to_string(),
            }))
            .unwrap();
        assert!(
            resp.0
                .content
                .unwrap()
                .contains("register_watch_target_http")
        );
        assert!(resp.0.message.is_none());
    }

    #[test]
    fn read_file_traversal_refused_not_transport_error() {
        let tmp = fixture();
        let parent = tmp.path().parent().unwrap();
        std::fs::write(parent.join("paw-tool-secret.txt"), "TOPSECRET").unwrap();
        let server = server_for(tmp.path().canonicalize().unwrap());
        let resp = server
            .read_file(Parameters(super::ReadFileParams {
                path: "../paw-tool-secret.txt".to_string(),
            }))
            .expect("traversal refusal is not a transport error");
        assert!(resp.0.content.is_none());
        assert!(resp.0.message.is_some());
    }

    #[test]
    fn read_file_gitignored_refused_not_transport_error() {
        let tmp = fixture();
        let server = server_for(tmp.path().canonicalize().unwrap());
        let resp = server
            .read_file(Parameters(super::ReadFileParams {
                path: "target/debug/foo".to_string(),
            }))
            .expect("gitignored refusal is not a transport error");
        assert!(resp.0.content.is_none());
        assert!(resp.0.message.is_some());
    }

    #[test]
    fn search_code_happy_path() {
        let tmp = fixture();
        let server = server_for(tmp.path().canonicalize().unwrap());
        let resp = server.search_code(Parameters(super::SearchCodeParams {
            query: "register_watch_target_http".to_string(),
            subpath: None,
        }));
        assert_eq!(resp.0.matches.len(), 1);
        assert_eq!(resp.0.matches[0].path, "src/main.rs");
        assert!(!resp.0.truncated);
    }

    #[test]
    fn search_code_empty_when_no_match() {
        let tmp = fixture();
        let server = server_for(tmp.path().canonicalize().unwrap());
        let resp = server.search_code(Parameters(super::SearchCodeParams {
            query: "a-string-that-appears-nowhere".to_string(),
            subpath: None,
        }));
        assert!(resp.0.matches.is_empty());
        assert!(!resp.0.truncated);
    }
}

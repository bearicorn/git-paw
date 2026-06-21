//! Documentation tools: `get_readme`, `list_docs`, `get_doc`.
//!
//! These serve the repository's own documentation via the bring-your-own
//! `[governance].readme` and `[governance].docs` configuration. Unset paths
//! degrade to null / empty results (never a transport error). `get_doc` is
//! confined to the configured documentation directory: a path that escapes it
//! is refused with a null body and a `message`, not a file read outside the
//! directory.

use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::config::GovernanceConfig;
use crate::error::PawError;
use crate::mcp::query;
use crate::mcp::server::GitPawMcpServer;

/// Maps an internal error to an MCP protocol error. Takes the error by value
/// so it composes directly with `Result::map_err`.
#[allow(clippy::needless_pass_by_value)]
fn to_err(e: PawError) -> ErrorData {
    ErrorData::internal_error(e.to_string(), None)
}

/// Parameters for [`GitPawMcpServer::get_doc`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetDocParams {
    /// Document path relative to the configured `[governance].docs` directory
    /// (e.g. `user-guide/mcp.md`). Paths escaping the directory are refused.
    pub path: String,
}

/// Response for `get_readme`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct ReadmeResponse {
    /// README content, or null when `[governance].readme` is unset or the
    /// file is absent.
    pub content: Option<String>,
}

/// Response for `list_docs`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct DocsListResponse {
    /// Documents under the configured docs directory, relative to it. Empty
    /// when `[governance].docs` is unset or the directory is absent.
    pub docs: Vec<query::docs::DocEntry>,
}

/// Response for `get_doc`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct DocResponse {
    /// Document content, or null when unset, not found, or refused.
    pub content: Option<String>,
    /// Human-readable note when `content` is null (e.g. a refused traversal or
    /// an absent document). Absent when content is present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl GitPawMcpServer {
    fn docs_governance(&self) -> Result<GovernanceConfig, ErrorData> {
        query::governance::load(&self.ctx.root).map_err(to_err)
    }
}

#[tool_router(router = docs_router, vis = "pub(crate)")]
impl GitPawMcpServer {
    /// `get_readme` — the configured repository README.
    #[tool(
        description = "Return the configured [governance].readme content, or null when unset or \
                       the file is absent. Errors only if the configured file is unreadable."
    )]
    pub(crate) fn get_readme(&self) -> Result<Json<ReadmeResponse>, ErrorData> {
        let gov = self.docs_governance()?;
        let content = query::docs::read_readme(&self.ctx.root, &gov).map_err(to_err)?;
        Ok(Json(ReadmeResponse { content }))
    }

    /// `list_docs` — Markdown documents under the configured docs directory.
    #[tool(
        description = "List Markdown documents under the configured [governance].docs directory, \
                       each with its path relative to that directory. Empty when unset or the \
                       directory is absent."
    )]
    pub(crate) fn list_docs(&self) -> Result<Json<DocsListResponse>, ErrorData> {
        let gov = self.docs_governance()?;
        Ok(Json(DocsListResponse {
            docs: query::docs::list_docs(&self.ctx.root, &gov),
        }))
    }

    /// `get_doc` — one document confined to the configured docs directory.
    #[tool(
        description = "Return the content of one document under the configured [governance].docs \
                       directory, by path relative to it. Confined to that directory: a path \
                       escaping it (e.g. \"../\") is refused with null content and a message, not \
                       a read outside the directory. Null content with a message when the doc is \
                       absent or docs is unset."
    )]
    pub(crate) fn get_doc(
        &self,
        Parameters(p): Parameters<GetDocParams>,
    ) -> Result<Json<DocResponse>, ErrorData> {
        let gov = self.docs_governance()?;
        let content = query::docs::read_doc(&self.ctx.root, &gov, &p.path).map_err(to_err)?;
        let message = if content.is_none() {
            Some(format!(
                "no document available for path {:?} (unset, not found, or refused as outside \
                 the configured docs directory)",
                p.path
            ))
        } else {
            None
        };
        Ok(Json(DocResponse { content, message }))
    }
}

#[cfg(test)]
mod tests {
    use crate::mcp::RepoContext;
    use crate::mcp::server::GitPawMcpServer;

    fn server_for(root: std::path::PathBuf) -> GitPawMcpServer {
        GitPawMcpServer::new(RepoContext {
            root,
            git_paw_dir: None,
            broker_url: None,
            server_name: "git-paw".to_string(),
        })
    }

    fn write_config(root: &std::path::Path, body: &str) {
        let dir = root.join(".git-paw");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.toml"), body).unwrap();
    }

    #[test]
    fn get_readme_returns_content_when_configured() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("README.md"), "# Project").unwrap();
        write_config(tmp.path(), "[governance]\nreadme = \"README.md\"\n");
        let server = server_for(tmp.path().to_path_buf());
        let resp = server.get_readme().unwrap();
        assert_eq!(resp.0.content.as_deref(), Some("# Project"));
    }

    #[test]
    fn get_readme_null_when_unconfigured() {
        let tmp = tempfile::tempdir().unwrap();
        let server = server_for(tmp.path().to_path_buf());
        let resp = server.get_readme().unwrap();
        assert!(resp.0.content.is_none());
    }

    #[test]
    fn list_docs_empty_when_unconfigured() {
        let tmp = tempfile::tempdir().unwrap();
        let server = server_for(tmp.path().to_path_buf());
        let resp = server.list_docs().unwrap();
        assert!(resp.0.docs.is_empty());
    }

    #[test]
    fn list_docs_enumerates_configured_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let docs = tmp.path().join("docs/src");
        std::fs::create_dir_all(docs.join("user-guide")).unwrap();
        std::fs::write(docs.join("user-guide/mcp.md"), "# MCP").unwrap();
        write_config(tmp.path(), "[governance]\ndocs = \"docs/src\"\n");
        let server = server_for(tmp.path().to_path_buf());
        let resp = server.list_docs().unwrap();
        let paths: Vec<&str> = resp.0.docs.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(paths, vec!["user-guide/mcp.md"]);
    }

    #[test]
    fn get_doc_happy_path() {
        let tmp = tempfile::tempdir().unwrap();
        let docs = tmp.path().join("docs/src");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("intro.md"), "# Intro").unwrap();
        write_config(tmp.path(), "[governance]\ndocs = \"docs/src\"\n");
        let server = server_for(tmp.path().to_path_buf());
        let resp = server
            .get_doc(rmcp::handler::server::wrapper::Parameters(
                super::GetDocParams {
                    path: "intro.md".to_string(),
                },
            ))
            .unwrap();
        assert_eq!(resp.0.content.as_deref(), Some("# Intro"));
        assert!(resp.0.message.is_none());
    }

    #[test]
    fn get_doc_traversal_refused_not_transport_error() {
        let tmp = tempfile::tempdir().unwrap();
        let docs = tmp.path().join("docs/src");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(tmp.path().join("secret.txt"), "TOPSECRET").unwrap();
        write_config(tmp.path(), "[governance]\ndocs = \"docs/src\"\n");
        let server = server_for(tmp.path().to_path_buf());
        // A refused traversal is a successful response with null content + a
        // message, not a transport-level error.
        let resp = server
            .get_doc(rmcp::handler::server::wrapper::Parameters(
                super::GetDocParams {
                    path: "../../secret.txt".to_string(),
                },
            ))
            .expect("traversal refusal is not a transport error");
        assert!(resp.0.content.is_none());
        assert!(resp.0.message.is_some());
    }
}

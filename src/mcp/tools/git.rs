//! Git-context tools: `get_branches`, `get_recent_commits`, `get_diff`.
//!
//! Thin wrappers over read-only git invocations against the resolved
//! repository root. These always work (the repo always exists), so there is no
//! degradation path.

use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::mcp::query;
use crate::mcp::query::git::Diff;
use crate::mcp::server::GitPawMcpServer;

/// Parameters for [`GitPawMcpServer::get_recent_commits`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetRecentCommitsParams {
    /// Branch to read commits from.
    pub branch: String,
    /// Maximum number of commits to return (default 20).
    #[serde(default = "default_limit")]
    #[schemars(description = "Maximum number of commits to return (default 20)")]
    pub limit: usize,
}

fn default_limit() -> usize {
    20
}

/// Parameters for [`GitPawMcpServer::get_diff`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetDiffParams {
    /// Branch to diff.
    pub branch: String,
    /// Base to diff against (defaults to the repository's default branch).
    #[serde(default)]
    pub base: Option<String>,
}

/// Response for `get_branches`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct BranchesResponse {
    /// Local branches.
    pub branches: Vec<query::git::Branch>,
}

/// Response for `get_recent_commits`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct CommitsResponse {
    /// Commits, newest first.
    pub commits: Vec<query::git::Commit>,
}

#[tool_router(router = git_router, vis = "pub(crate)")]
impl GitPawMcpServer {
    /// `get_branches` — local branches with head SHA + flags.
    #[tool(
        description = "List local branches, each with name, head commit SHA, whether it is the \
                       currently checked-out branch, and whether it is checked out in a linked \
                       (git-paw managed) worktree."
    )]
    pub(crate) fn get_branches(&self) -> Json<BranchesResponse> {
        Json(BranchesResponse {
            branches: query::git::branches(&self.ctx.root),
        })
    }

    /// `get_recent_commits` — last N commits on a branch.
    #[tool(
        description = "Return up to `limit` (default 20) recent commits on `branch`, newest first, \
                       each with sha, author, ISO timestamp, and subject."
    )]
    pub(crate) fn get_recent_commits(
        &self,
        Parameters(p): Parameters<GetRecentCommitsParams>,
    ) -> Json<CommitsResponse> {
        Json(CommitsResponse {
            commits: query::git::recent_commits(&self.ctx.root, &p.branch, p.limit),
        })
    }

    /// `get_diff` — diff of a branch against its base.
    #[tool(
        description = "Return the diff of `branch` against `base` (default: the repo's default \
                       branch) with a files-changed / insertions / deletions summary."
    )]
    pub(crate) fn get_diff(&self, Parameters(p): Parameters<GetDiffParams>) -> Json<Diff> {
        Json(query::git::diff(
            &self.ctx.root,
            &p.branch,
            p.base.as_deref(),
        ))
    }
}

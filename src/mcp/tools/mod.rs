//! MCP tool definitions, one file per category (design D2).
//!
//! Each category file adds an `impl GitPawMcpServer` block carrying its
//! `#[tool]` methods and a named `#[tool_router(...)]`; [`crate::mcp::server`]
//! merges the per-category routers into the server's combined router. Tool
//! methods are thin: they parse parameters, call [`crate::mcp::query`], and
//! wrap the result as MCP structured content. Per the degradation contract
//! (design D4) most tools return successful empty/null payloads when their
//! data source is absent; only genuine misconfiguration surfaces as an
//! [`rmcp::ErrorData`].
//!
//! `publish` (capability `mcp-agent-publish`) is the one write category: its
//! four tools publish to the broker instead of reading, and an unreachable
//! broker surfaces as an [`rmcp::ErrorData`] rather than degrading to an
//! empty result — there is no "empty" form of having published nothing.

pub mod coordination;
pub mod docs;
pub mod git;
pub mod governance;
pub mod project;
pub mod publish;
pub mod session;
pub mod source;

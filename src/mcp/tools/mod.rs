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

pub mod coordination;
pub mod docs;
pub mod git;
pub mod governance;
pub mod project;
pub mod session;
pub mod source;

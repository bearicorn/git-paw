//! Spec-authoring tools: `create_spec`, `write_spec_artifact` (repository
//! writes) and `validate_spec` (read-only).
//!
//! The two write tools exist only for a root session of a repository whose
//! own config sets `[mcp] allow_repo_writes = true`
//! ([`crate::mcp::repo_write::repo_write_access`], applied to the tool list
//! by the server and again as each tool's first step). Every write is confined
//! to one change directory under the configured spec directory
//! ([`crate::mcp::repo_write`]), logged on the server log at `warn` (recorded
//! at the default verbosity), and never staged or committed. `validate_spec` reads only and is offered to every session.
//!
//! The tools are thin adapters over the pure scaffold and structural check in
//! [`crate::specs::openspec`]; only the `OpenSpec` format is supported.
//!
//! No process-wide lock is taken (unlike the lifecycle tools): a write touches
//! one file inside one change directory, `create_spec` claims its directory
//! with an atomic `create_dir`, and each artifact lands by an atomic rename
//! from a uniquely named temporary file, so concurrent calls cannot interleave
//! partial content.

use std::fs;
use std::path::{Path, PathBuf};

use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::config;
use crate::error::PawError;
use crate::mcp::server::GitPawMcpServer;
use crate::mcp::{logging, repo_write};
use crate::specs::openspec::{self, ChangeFinding};

/// Parameters for [`GitPawMcpServer::create_spec`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateSpecParams {
    /// Change name: lowercase letters, digits and hyphens (e.g. "add-export").
    pub name: String,
}

/// Parameters for [`GitPawMcpServer::write_spec_artifact`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WriteSpecArtifactParams {
    /// Name of an existing change.
    pub change: String,
    /// Path inside the change directory (e.g. "proposal.md" or
    /// "specs/data-export/spec.md"); relative, without `..` or `.git`.
    pub path: String,
    /// The file's full content, written verbatim (at most 1 MiB).
    pub content: String,
}

/// Parameters for [`GitPawMcpServer::validate_spec`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ValidateSpecParams {
    /// Name of an existing change.
    pub change: String,
}

/// Response for `create_spec`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CreateSpecResponse {
    /// Repository-relative paths written.
    pub written: Vec<String>,
}

/// Response for `write_spec_artifact`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct WriteSpecArtifactResponse {
    /// Repository-relative paths written.
    pub written: Vec<String>,
    /// Whether an existing file was replaced.
    pub replaced: bool,
}

/// Response for `validate_spec`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ValidateSpecResponse {
    /// True exactly when there are no findings.
    pub valid: bool,
    /// Structural findings, each naming the file it concerns (relative to the
    /// change directory).
    pub findings: Vec<ChangeFinding>,
}

fn to_error(e: &PawError) -> ErrorData {
    ErrorData::invalid_params(e.to_string(), None)
}

/// Largest `content` `write_spec_artifact` accepts.
const MAX_ARTIFACT_BYTES: usize = 1024 * 1024;

/// The spec directory (relative to the repository root) from the
/// repository's own `.git-paw/config.toml` `[specs]` section, when its format
/// is `OpenSpec`; otherwise the error the tool returns.
///
/// Like the consent switch, the location is taken from the repository config
/// alone: a user-level `[specs]` section never decides where writes land.
fn openspec_dir(root: &Path, tool: &str) -> Result<String, ErrorData> {
    let repo_cfg = config::load_config_file(&config::repo_config_path(root))
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
    let Some(specs) = repo_cfg.and_then(|c| c.specs) else {
        return Err(ErrorData::invalid_params(
            format!(
                "`{tool}`: no spec system configured — add a [specs] section to the \
                 repository's .git-paw/config.toml"
            ),
            None,
        ));
    };
    let spec_type = specs.spec_type.as_deref().unwrap_or("openspec");
    if spec_type != "openspec" {
        return Err(ErrorData::invalid_params(
            format!("`{tool}` is not supported for format {spec_type}; only openspec is"),
            None,
        ));
    }
    Ok(specs.dir.unwrap_or_else(|| "specs".to_string()))
}

/// Joins a repository-relative path for the response and the log.
fn repo_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Collects the change's Markdown files as (change-relative path, content),
/// without following symbolic links.
fn collect_markdown(change_dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = vec![change_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file()
                && path.extension().is_some_and(|e| e == "md")
                && let Ok(content) = fs::read_to_string(&path)
            {
                out.push((repo_relative(change_dir, &path), content));
            }
        }
    }
    out
}

#[tool_router(router = spec_write_router, vis = "pub(crate)")]
impl GitPawMcpServer {
    /// `create_spec` — scaffold a new change directory.
    #[tool(
        description = "Create a new spec change directory under the configured spec directory, \
                       with the same metadata file the spec tool's own 'new change' command \
                       writes. The name must be lowercase letters, digits and hyphens; an \
                       existing change is refused. Returns the repository-relative paths \
                       written. Nothing is staged or committed."
    )]
    pub(crate) fn create_spec(
        &self,
        Parameters(p): Parameters<CreateSpecParams>,
    ) -> Result<Json<CreateSpecResponse>, ErrorData> {
        self.repo_write_gate()?;
        if !openspec::is_valid_change_name(&p.name) {
            return Err(ErrorData::invalid_params(
                format!(
                    "invalid change name {:?}: use lowercase letters, digits and hyphens only",
                    p.name
                ),
                None,
            ));
        }
        let dir = openspec_dir(&self.ctx.root, "create_spec")?;
        let (root, spec_dir) =
            repo_write::resolve_spec_dir(&self.ctx.root, &dir).map_err(|e| to_error(&e))?;
        let change_dir = spec_dir.join(&p.name);
        fs::create_dir(&change_dir).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                ErrorData::invalid_params(format!("change {:?} already exists", p.name), None)
            } else {
                ErrorData::internal_error(format!("cannot create change {:?}: {e}", p.name), None)
            }
        })?;
        let metadata = openspec::change_metadata(chrono::Local::now().date_naive());
        if let Err(e) = repo_write::write_in_change(
            &change_dir,
            &[openspec::CHANGE_METADATA_FILE.to_string()],
            &metadata,
        ) {
            // Leave no half-created change behind.
            let _ = fs::remove_dir(&change_dir);
            return Err(to_error(&e));
        }
        let written = repo_relative(&root, &change_dir.join(openspec::CHANGE_METADATA_FILE));
        logging::warn(&format!(
            "create_spec wrote {written} ({} bytes, replaced: false)",
            metadata.len()
        ));
        Ok(Json(CreateSpecResponse {
            written: vec![written],
        }))
    }

    /// `write_spec_artifact` — create or replace one file in a change.
    #[tool(
        description = "Write one file (proposal, design, tasks, or a delta spec such as \
                       specs/<capability>/spec.md) into an existing spec change directory, \
                       verbatim, creating missing parent directories inside it and replacing \
                       an existing file. The path must be relative to the change directory, \
                       without '..' or '.git'. Returns the repository-relative path written and \
                       whether a file was replaced. Nothing is staged or committed."
    )]
    pub(crate) fn write_spec_artifact(
        &self,
        Parameters(p): Parameters<WriteSpecArtifactParams>,
    ) -> Result<Json<WriteSpecArtifactResponse>, ErrorData> {
        self.repo_write_gate()?;
        repo_write::check_change_segment(&p.change).map_err(|e| to_error(&e))?;
        let parts = repo_write::check_artifact_path(&p.path).map_err(|e| to_error(&e))?;
        if parts == [openspec::CHANGE_METADATA_FILE] {
            return Err(ErrorData::invalid_params(
                format!(
                    "`path` {:?}: {} is written by create_spec only",
                    p.path,
                    openspec::CHANGE_METADATA_FILE
                ),
                None,
            ));
        }
        if p.content.len() > MAX_ARTIFACT_BYTES {
            return Err(ErrorData::invalid_params(
                format!(
                    "`content` is {} bytes; the limit is {MAX_ARTIFACT_BYTES} bytes (1 MiB)",
                    p.content.len()
                ),
                None,
            ));
        }
        let dir = openspec_dir(&self.ctx.root, "write_spec_artifact")?;
        let (root, spec_dir) =
            repo_write::resolve_spec_dir(&self.ctx.root, &dir).map_err(|e| to_error(&e))?;
        let change_dir =
            repo_write::existing_change_dir(&spec_dir, &p.change).map_err(|e| to_error(&e))?;
        let replaced = repo_write::write_in_change(&change_dir, &parts, &p.content)
            .map_err(|e| to_error(&e))?;
        let written = repo_relative(&root, &change_dir.join(parts.join("/")));
        logging::warn(&format!(
            "write_spec_artifact wrote {written} ({} bytes, replaced: {replaced})",
            p.content.len()
        ));
        Ok(Json(WriteSpecArtifactResponse {
            written: vec![written],
            replaced,
        }))
    }
}

#[tool_router(router = spec_check_router, vis = "pub(crate)")]
impl GitPawMcpServer {
    /// `validate_spec` — read-only structural check of a change.
    #[tool(
        description = "Check a spec change's structure without modifying anything: a proposal is \
                       present; at least one delta spec (specs/<capability>/spec.md) carries an \
                       ADDED, MODIFIED, REMOVED or RENAMED Requirements header; each added or \
                       modified requirement states SHALL or MUST on its first line and has a \
                       scenario. Returns { valid, findings: [{ file, message }] }. A subset of \
                       the project's own validator, which stays authoritative."
    )]
    pub(crate) fn validate_spec(
        &self,
        Parameters(p): Parameters<ValidateSpecParams>,
    ) -> Result<Json<ValidateSpecResponse>, ErrorData> {
        let dir = openspec_dir(&self.ctx.root, "validate_spec")?;
        let (_, spec_dir) =
            repo_write::resolve_spec_dir(&self.ctx.root, &dir).map_err(|e| to_error(&e))?;
        let change_dir =
            repo_write::existing_change_dir(&spec_dir, &p.change).map_err(|e| to_error(&e))?;
        let findings = openspec::check_change(&collect_markdown(&change_dir));
        Ok(Json(ValidateSpecResponse {
            valid: findings.is_empty(),
            findings,
        }))
    }
}

impl GitPawMcpServer {
    /// The repository-write gate, as a tool's first step.
    fn repo_write_gate(&self) -> Result<(), ErrorData> {
        repo_write::repo_write_access(self.ctx.is_main_worktree, self.ctx.allow_repo_writes)
            .map_err(|refusal| ErrorData::invalid_request(refusal.to_string(), None))
    }
}

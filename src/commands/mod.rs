//! The command use cases: one module per command family, callable in-process.
//!
//! # Internal API
//!
//! Like the rest of this library, `commands` is **internal**: it exists to
//! serve the `git-paw` binary, its MCP server and the integration tests, and
//! it is **not** covered by semver. Any item may change or disappear in any
//! release.
//!
//! # Shape
//!
//! A command takes explicit inputs — a [`CommandContext`] (the repository
//! root and its loaded config) and an options struct — writes its
//! human-readable output through an injected [`Output`] sink, and returns a
//! typed outcome or a [`PawError`](crate::error::PawError). A command never
//! reads the working directory, reads stdin, prompts, prints, attaches a
//! terminal client or runs the unattended drive loop: the caller (the
//! binary's entry layer in `main.rs`, or the MCP server) resolves inputs,
//! prompts where the CLI prompts, renders the outcome, and attaches or drives
//! the session afterwards. The tmux/git orchestration routes through the
//! [`command_runner`](crate::command_runner) seam and the terminal port.
//!
//! Two modules are entry-layer or host code by design and exempt by name:
//! [`completions`] writes a generated completion script to stdout, and
//! [`broker_host`] is the hidden `__broker` host, a long-running process.

use std::cell::OnceCell;
use std::path::{Path, PathBuf};

use crate::config::PawConfig;
use crate::error::PawError;

pub mod add;
pub mod approvals;
pub mod attach;
pub mod authority;
pub mod broker_host;
pub mod clis;
pub mod completions;
pub mod entry_points;
pub mod helpers;
pub mod launch;
pub mod output;
pub mod pause;
pub mod purge;
pub mod recover;
pub mod remove;
pub mod replay;
pub mod setup_cli;
pub mod spec_mode;
pub mod start;
pub mod status;
pub mod stop;
pub mod supervisor;

pub use output::{CaptureOutput, Output};

/// The explicit inputs every repository-scoped command runs against: the
/// repository root and its configuration.
///
/// The configuration is either handed in already loaded
/// ([`CommandContext::with_config`]) or loaded from the repository on first
/// use ([`CommandContext::new`]). Loading on first use keeps each command's
/// error ordering exactly what the CLI has always shown — a command that
/// first checks for an existing session never fails on a config it would not
/// have read.
#[derive(Debug, Clone)]
pub struct CommandContext {
    repo_root: PathBuf,
    config: OnceCell<PawConfig>,
}

impl CommandContext {
    /// A context for `repo_root` (already validated) whose config is loaded
    /// from the repository the first time a command needs it.
    #[must_use]
    pub fn new(repo_root: PathBuf) -> Self {
        Self {
            repo_root,
            config: OnceCell::new(),
        }
    }

    /// A context for `repo_root` carrying an already-loaded `config`.
    #[must_use]
    pub fn with_config(repo_root: PathBuf, config: PawConfig) -> Self {
        Self {
            repo_root,
            config: OnceCell::from(config),
        }
    }

    /// The repository root.
    #[must_use]
    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    /// The repository's configuration, loaded on first use.
    ///
    /// # Errors
    ///
    /// Returns the config loader's error when the configuration cannot be
    /// read or parsed; a later call retries.
    pub fn config(&self) -> Result<&PawConfig, PawError> {
        if let Some(config) = self.config.get() {
            return Ok(config);
        }
        let loaded = crate::config::load_config(&self.repo_root, None)?;
        Ok(self.config.get_or_init(|| loaded))
    }

    /// The configuration, or the defaults when it cannot be loaded — for the
    /// commands that have always tolerated an unreadable config.
    #[must_use]
    pub fn config_or_default(&self) -> PawConfig {
        self.config().cloned().unwrap_or_default()
    }
}

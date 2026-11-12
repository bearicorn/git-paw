//! Interactive selection prompts.
//!
//! User-facing selection flows for `git paw start`. The two multi-select
//! prompts — the branch picker ([`TerminalPrompter::select_branches`]) and the
//! spec picker ([`TerminalPrompter::select_specs`]) — are built on a shared
//! `ratatui` + `crossterm` fuzzy multi-select helper
//! ([`picker::fuzzy_multi_select`]) that lets the user type a query to filter a
//! long candidate list. The single-select prompts (mode picker and CLI pickers)
//! stay on `dialoguer::Select`. Logic is separated from UI via the [`Prompter`]
//! trait, and the filter/selection bookkeeping lives in the pure, terminal-free
//! [`resolver::PickerState`] for testability.
//!
//! The submodules are split by altitude: `resolver` holds the terminal-free
//! selection logic, `prompter` the `dialoguer` prompt implementations, and
//! `picker` the `ratatui` fuzzy multi-select and its terminal lifecycle. This
//! module owns the shared types, the [`Prompter`] trait seam, and the
//! re-exports every caller resolves through.

mod picker;
mod prompter;
mod resolver;

use std::fmt;

use crate::config::{PawConfig, ProfileName};
use crate::error::PawError;
use crate::specs::SpecEntry;

pub use crate::worker::WorkerChoice;
pub use prompter::TerminalPrompter;
pub use resolver::{resolve_cli_for_specs, run_selection};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Information about an available AI CLI.
///
/// Contains the data needed to display a CLI option in interactive prompts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliInfo {
    /// Human-readable name shown in prompts (e.g., "My Agent").
    pub display_name: String,
    /// Binary name used for invocation (e.g., "my-agent").
    pub binary_name: String,
}

impl fmt::Display for CliInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.display_name == self.binary_name {
            write!(f, "{}", self.binary_name)
        } else {
            write!(f, "{} ({})", self.display_name, self.binary_name)
        }
    }
}

/// A worker profile as the CLI pickers list it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileInfo {
    /// The profile name.
    pub name: ProfileName,
    /// Its one-line description.
    pub description: String,
    /// The CLI it launches.
    pub cli: String,
}

impl ProfileInfo {
    /// The configured worker profiles, in name order. Delegate profiles are
    /// never listed.
    #[must_use]
    pub fn workers(config: &PawConfig) -> Vec<Self> {
        config
            .worker_profiles()
            .map(|(name, entry)| Self {
                name: name.clone(),
                description: entry.description.clone(),
                cli: entry.cli.clone(),
            })
            .collect()
    }
}

/// Displays `<name> — <description> [<cli>]`.
impl fmt::Display for ProfileInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} \u{2014} {} [{}]",
            self.name, self.description, self.cli
        )
    }
}

/// What a CLI picker offers: the available CLIs, then the worker profiles,
/// starting on `default`.
#[derive(Debug, Clone, Copy)]
pub struct WorkerOptions<'a> {
    /// The available CLIs, listed first in their detection order.
    pub clis: &'a [CliInfo],
    /// The worker profiles, listed after the CLIs.
    pub profiles: &'a [ProfileInfo],
    /// The entry the picker starts on: `default_profile`, else `default_cli`.
    pub default: Option<&'a WorkerChoice>,
}

impl<'a> WorkerOptions<'a> {
    /// Options listing `clis` only, starting on the CLI `default` names.
    #[must_use]
    pub fn clis_only(clis: &'a [CliInfo], default: Option<&'a WorkerChoice>) -> Self {
        Self {
            clis,
            profiles: &[],
            default,
        }
    }

    /// The picker's items in display order, each with the choice it maps to:
    /// every CLI as before, then every worker profile.
    #[must_use]
    pub fn items(&self) -> Vec<(String, WorkerChoice)> {
        self.clis
            .iter()
            .map(|c| (c.to_string(), WorkerChoice::Cli(c.binary_name.clone())))
            .chain(
                self.profiles
                    .iter()
                    .map(|p| (p.to_string(), WorkerChoice::Profile(p.name.clone()))),
            )
            .collect()
    }

    /// The index the picker starts on: the item `default` names, else the
    /// first.
    #[must_use]
    pub fn default_index(&self) -> usize {
        self.default
            .and_then(|d| self.items().iter().position(|(_, c)| c == d))
            .unwrap_or(0)
    }
}

/// How the user wants to assign CLIs to branches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliMode {
    /// Same CLI for all selected branches.
    Uniform,
    /// Different CLI for each branch.
    PerBranch,
}

impl fmt::Display for CliMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uniform => write!(f, "Same CLI for all branches"),
            Self::PerBranch => write!(f, "Different CLI per branch"),
        }
    }
}

/// Result of the full interactive selection flow.
#[derive(Debug)]
pub struct SelectionResult {
    /// Branch-to-worker mappings as `(branch_name, choice)` pairs.
    pub mappings: Vec<(String, WorkerChoice)>,
}

// ---------------------------------------------------------------------------
// Prompter trait (separates logic from UI)
// ---------------------------------------------------------------------------

/// Abstraction over interactive prompts, allowing test doubles.
pub trait Prompter {
    /// Ask the user to choose between uniform and per-branch CLI assignment.
    fn select_mode(&self) -> Result<CliMode, PawError>;

    /// Ask the user to pick one or more branches. Returns selected branch names.
    fn select_branches(&self, branches: &[String]) -> Result<Vec<String>, PawError>;

    /// Ask the user to pick a single CLI or worker profile for all branches.
    ///
    /// The picker lists [`WorkerOptions::items`] and starts on
    /// [`WorkerOptions::default_index`].
    fn select_cli(&self, options: &WorkerOptions<'_>) -> Result<WorkerChoice, PawError>;

    /// Ask the user to pick a CLI or worker profile for a specific branch,
    /// from the same items and starting entry as [`Self::select_cli`].
    fn select_cli_for_branch(
        &self,
        branch: &str,
        options: &WorkerOptions<'_>,
    ) -> Result<WorkerChoice, PawError>;

    /// Ask the user to pick one or more specs. Returns the selected
    /// `SpecEntry` values expanded from grouped logical units.
    ///
    /// Each row in the picker represents one logical unit (a Spec Kit
    /// feature, an `OpenSpec` change, or a Markdown spec). Selecting a row
    /// returns every underlying `SpecEntry` belonging to that unit.
    fn select_specs(&self, specs: &[SpecEntry]) -> Result<Vec<SpecEntry>, PawError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Display impls
    // -----------------------------------------------------------------------

    #[test]
    fn cli_mode_display() {
        assert_eq!(CliMode::Uniform.to_string(), "Same CLI for all branches");
        assert_eq!(CliMode::PerBranch.to_string(), "Different CLI per branch");
    }

    #[test]
    fn cli_info_display_same_names() {
        let info = CliInfo {
            display_name: "claude".to_string(),
            binary_name: "claude".to_string(),
        };
        assert_eq!(info.to_string(), "claude");
    }

    #[test]
    fn cli_info_display_different_names() {
        let info = CliInfo {
            display_name: "My Agent".to_string(),
            binary_name: "my-agent".to_string(),
        };
        assert_eq!(info.to_string(), "My Agent (my-agent)");
    }
}

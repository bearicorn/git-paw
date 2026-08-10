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

use crate::error::PawError;
use crate::specs::SpecEntry;

pub use prompter::TerminalPrompter;
pub use resolver::{resolve_cli_for_specs, run_selection};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Information about an available AI CLI.
///
/// Contains the data needed to display a CLI option in interactive prompts.
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
    /// Branch-to-CLI mappings as `(branch_name, cli_binary_name)` pairs.
    pub mappings: Vec<(String, String)>,
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

    /// Ask the user to pick a single CLI for all branches. Returns binary name.
    ///
    /// When `default` is `Some` and matches a CLI's `binary_name`, that entry
    /// is pre-selected in the picker. Otherwise the first item is selected.
    fn select_cli(&self, clis: &[CliInfo], default: Option<&str>) -> Result<String, PawError>;

    /// Ask the user to pick a CLI for a specific branch. Returns binary name.
    fn select_cli_for_branch(&self, branch: &str, clis: &[CliInfo]) -> Result<String, PawError>;

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

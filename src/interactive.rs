//! Interactive selection prompts.
//!
//! User-facing selection flows using `dialoguer`: mode picker, branch picker,
//! and CLI picker (uniform and per-branch). Logic is separated from UI via
//! the [`Prompter`] trait for testability.

use std::fmt;

use dialoguer::{MultiSelect, Select};

use crate::error::PawError;

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
    fn select_cli(&self, clis: &[CliInfo]) -> Result<String, PawError>;

    /// Ask the user to pick a CLI for a specific branch. Returns binary name.
    fn select_cli_for_branch(&self, branch: &str, clis: &[CliInfo]) -> Result<String, PawError>;
}

// ---------------------------------------------------------------------------
// Real prompter (dialoguer)
// ---------------------------------------------------------------------------

/// Interactive prompter using `dialoguer` for terminal UI.
pub struct TerminalPrompter;

impl Prompter for TerminalPrompter {
    fn select_mode(&self) -> Result<CliMode, PawError> {
        let modes = [CliMode::Uniform, CliMode::PerBranch];
        let labels: Vec<String> = modes.iter().map(ToString::to_string).collect();

        let selection = Select::new()
            .with_prompt("CLI assignment mode")
            .items(&labels)
            .default(0)
            .interact_opt()
            .map_err(|e| map_dialoguer_error(&e))?;

        match selection {
            Some(idx) => Ok(modes[idx]),
            None => Err(PawError::UserCancelled),
        }
    }

    fn select_branches(&self, branches: &[String]) -> Result<Vec<String>, PawError> {
        let selection = MultiSelect::new()
            .with_prompt("Select branches (space to toggle, enter to confirm)")
            .items(branches)
            .interact_opt()
            .map_err(|e| map_dialoguer_error(&e))?;

        match selection {
            Some(indices) if indices.is_empty() => Err(PawError::UserCancelled),
            Some(indices) => Ok(indices.into_iter().map(|i| branches[i].clone()).collect()),
            None => Err(PawError::UserCancelled),
        }
    }

    fn select_cli(&self, clis: &[CliInfo]) -> Result<String, PawError> {
        let labels: Vec<String> = clis.iter().map(ToString::to_string).collect();

        let selection = Select::new()
            .with_prompt("Select AI CLI for all branches")
            .items(&labels)
            .default(0)
            .interact_opt()
            .map_err(|e| map_dialoguer_error(&e))?;

        match selection {
            Some(idx) => Ok(clis[idx].binary_name.clone()),
            None => Err(PawError::UserCancelled),
        }
    }

    fn select_cli_for_branch(&self, branch: &str, clis: &[CliInfo]) -> Result<String, PawError> {
        let labels: Vec<String> = clis.iter().map(ToString::to_string).collect();

        let selection = Select::new()
            .with_prompt(format!("Select CLI for {branch}"))
            .items(&labels)
            .default(0)
            .interact_opt()
            .map_err(|e| map_dialoguer_error(&e))?;

        match selection {
            Some(idx) => Ok(clis[idx].binary_name.clone()),
            None => Err(PawError::UserCancelled),
        }
    }
}

/// Maps dialoguer errors to `PawError`, treating I/O interrupted (Ctrl+C) as
/// user cancellation.
fn map_dialoguer_error(err: &dialoguer::Error) -> PawError {
    match err {
        dialoguer::Error::IO(io_err) if io_err.kind() == std::io::ErrorKind::Interrupted => {
            PawError::UserCancelled
        }
        dialoguer::Error::IO(_) => {
            PawError::SessionError(format!("Interactive prompt failed: {err}"))
        }
    }
}

// ---------------------------------------------------------------------------
// Core selection logic (independent of UI)
// ---------------------------------------------------------------------------

/// Runs the full interactive selection flow, skipping prompts when CLI flags
/// provide the necessary data.
///
/// # Errors
///
/// Returns `PawError::NoCLIsFound` if `clis` is empty.
/// Returns `PawError::BranchError` if `branches` is empty.
/// Returns `PawError::UserCancelled` if the user cancels any prompt.
pub fn run_selection(
    prompter: &dyn Prompter,
    branches: &[String],
    clis: &[CliInfo],
    cli_flag: Option<&str>,
    branches_flag: Option<&[String]>,
) -> Result<SelectionResult, PawError> {
    if clis.is_empty() {
        return Err(PawError::NoCLIsFound);
    }
    if branches.is_empty() {
        return Err(PawError::BranchError("No branches available.".to_string()));
    }

    // Determine which branches to use.
    let selected_branches = if let Some(flagged) = branches_flag {
        flagged.to_vec()
    } else {
        prompter.select_branches(branches)?
    };

    // Determine CLI mapping.
    let mappings = if let Some(cli) = cli_flag {
        selected_branches
            .into_iter()
            .map(|branch| (branch, cli.to_string()))
            .collect()
    } else {
        let mode = prompter.select_mode()?;
        match mode {
            CliMode::Uniform => {
                let cli = prompter.select_cli(clis)?;
                selected_branches
                    .into_iter()
                    .map(|branch| (branch, cli.clone()))
                    .collect()
            }
            CliMode::PerBranch => {
                let mut pairs = Vec::with_capacity(selected_branches.len());
                for branch in selected_branches {
                    let cli = prompter.select_cli_for_branch(&branch, clis)?;
                    pairs.push((branch, cli));
                }
                pairs
            }
        }
    };

    Ok(SelectionResult { mappings })
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Fake prompter for testing
    // -----------------------------------------------------------------------

    use std::cell::Cell;

    /// A configurable fake prompter that returns predetermined responses.
    /// Uses `Cell` for interior mutability to track per-branch call order.
    struct TrackingPrompter {
        mode: CliMode,
        branch_indices: Vec<usize>,
        uniform_cli: String,
        per_branch_clis: Vec<String>,
        per_branch_call_count: Cell<usize>,
        cancel_on_branch_select: bool,
        cancel_on_cli_select: bool,
    }

    impl TrackingPrompter {
        fn uniform(branch_indices: Vec<usize>, cli: &str) -> Self {
            Self {
                mode: CliMode::Uniform,
                branch_indices,
                uniform_cli: cli.to_string(),
                per_branch_clis: vec![],
                per_branch_call_count: Cell::new(0),
                cancel_on_branch_select: false,
                cancel_on_cli_select: false,
            }
        }

        fn per_branch(branch_indices: Vec<usize>, clis: Vec<&str>) -> Self {
            Self {
                mode: CliMode::PerBranch,
                branch_indices,
                uniform_cli: String::new(),
                per_branch_clis: clis.into_iter().map(String::from).collect(),
                per_branch_call_count: Cell::new(0),
                cancel_on_branch_select: false,
                cancel_on_cli_select: false,
            }
        }

        fn cancel_on_branches() -> Self {
            Self {
                mode: CliMode::Uniform,
                branch_indices: vec![],
                uniform_cli: String::new(),
                per_branch_clis: vec![],
                per_branch_call_count: Cell::new(0),
                cancel_on_branch_select: true,
                cancel_on_cli_select: false,
            }
        }

        fn cancel_on_cli(branch_indices: Vec<usize>) -> Self {
            Self {
                mode: CliMode::Uniform,
                branch_indices,
                uniform_cli: String::new(),
                per_branch_clis: vec![],
                per_branch_call_count: Cell::new(0),
                cancel_on_branch_select: false,
                cancel_on_cli_select: true,
            }
        }
    }

    impl Prompter for TrackingPrompter {
        fn select_mode(&self) -> Result<CliMode, PawError> {
            Ok(self.mode)
        }

        fn select_branches(&self, branches: &[String]) -> Result<Vec<String>, PawError> {
            if self.cancel_on_branch_select || self.branch_indices.is_empty() {
                return Err(PawError::UserCancelled);
            }
            Ok(self
                .branch_indices
                .iter()
                .map(|&i| branches[i].clone())
                .collect())
        }

        fn select_cli(&self, _clis: &[CliInfo]) -> Result<String, PawError> {
            if self.cancel_on_cli_select {
                return Err(PawError::UserCancelled);
            }
            Ok(self.uniform_cli.clone())
        }

        fn select_cli_for_branch(
            &self,
            _branch: &str,
            _clis: &[CliInfo],
        ) -> Result<String, PawError> {
            let idx = self.per_branch_call_count.get();
            self.per_branch_call_count.set(idx + 1);
            self.per_branch_clis
                .get(idx)
                .cloned()
                .ok_or(PawError::UserCancelled)
        }
    }

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn test_clis() -> Vec<CliInfo> {
        vec![
            CliInfo {
                display_name: "Alpha CLI".to_string(),
                binary_name: "alpha".to_string(),
            },
            CliInfo {
                display_name: "Beta CLI".to_string(),
                binary_name: "beta".to_string(),
            },
        ]
    }

    fn test_branches() -> Vec<String> {
        vec!["feature/auth".to_string(), "fix/api".to_string()]
    }

    // -----------------------------------------------------------------------
    // Behavior tests: flag-based prompt skipping
    // -----------------------------------------------------------------------

    #[test]
    fn both_flags_skips_all_prompts_and_maps_cli_to_all_branches() {
        let prompter = TrackingPrompter::cancel_on_branches(); // should never be called
        let branches = test_branches();
        let clis = test_clis();
        let flag_branches = vec!["feature/auth".to_string(), "fix/api".to_string()];

        let result = run_selection(
            &prompter,
            &branches,
            &clis,
            Some("alpha"),
            Some(&flag_branches),
        )
        .unwrap();

        assert_eq!(
            result.mappings,
            vec![
                ("feature/auth".to_string(), "alpha".to_string()),
                ("fix/api".to_string(), "alpha".to_string()),
            ]
        );
    }

    #[test]
    fn cli_flag_skips_cli_prompt_but_prompts_for_branches() {
        let prompter = TrackingPrompter::uniform(vec![0], "should-not-be-used");
        let branches = test_branches();
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, Some("alpha"), None).unwrap();

        // Should use the flag CLI, and the branch from the prompter (index 0)
        assert_eq!(
            result.mappings,
            vec![("feature/auth".to_string(), "alpha".to_string())]
        );
    }

    #[test]
    fn branches_flag_skips_branch_prompt_but_prompts_for_cli_uniform() {
        let prompter = TrackingPrompter::uniform(vec![], "beta");
        let branches = test_branches();
        let clis = test_clis();
        let flag_branches = vec!["feature/auth".to_string(), "fix/api".to_string()];

        let result =
            run_selection(&prompter, &branches, &clis, None, Some(&flag_branches)).unwrap();

        assert_eq!(
            result.mappings,
            vec![
                ("feature/auth".to_string(), "beta".to_string()),
                ("fix/api".to_string(), "beta".to_string()),
            ]
        );
    }

    // -----------------------------------------------------------------------
    // Behavior tests: interactive mode selection
    // -----------------------------------------------------------------------

    #[test]
    fn uniform_mode_maps_same_cli_to_all_selected_branches() {
        let prompter = TrackingPrompter::uniform(vec![0, 1], "alpha");
        let branches = test_branches();
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, None, None).unwrap();

        assert_eq!(
            result.mappings,
            vec![
                ("feature/auth".to_string(), "alpha".to_string()),
                ("fix/api".to_string(), "alpha".to_string()),
            ]
        );
    }

    #[test]
    fn per_branch_mode_maps_different_cli_to_each_branch() {
        let prompter = TrackingPrompter::per_branch(vec![0, 1], vec!["alpha", "beta"]);
        let branches = test_branches();
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, None, None).unwrap();

        assert_eq!(
            result.mappings,
            vec![
                ("feature/auth".to_string(), "alpha".to_string()),
                ("fix/api".to_string(), "beta".to_string()),
            ]
        );
    }

    #[test]
    fn per_branch_mode_with_branches_flag() {
        let prompter = TrackingPrompter::per_branch(vec![], vec!["beta", "alpha"]);
        let branches = test_branches();
        let clis = test_clis();
        let flag_branches = vec!["feature/auth".to_string(), "fix/api".to_string()];

        let result =
            run_selection(&prompter, &branches, &clis, None, Some(&flag_branches)).unwrap();

        assert_eq!(
            result.mappings,
            vec![
                ("feature/auth".to_string(), "beta".to_string()),
                ("fix/api".to_string(), "alpha".to_string()),
            ]
        );
    }

    // -----------------------------------------------------------------------
    // Behavior tests: cancellation / error cases
    // -----------------------------------------------------------------------

    #[test]
    fn no_clis_available_returns_error() {
        let prompter = TrackingPrompter::cancel_on_branches();
        let branches = test_branches();
        let clis: Vec<CliInfo> = vec![];

        let result = run_selection(&prompter, &branches, &clis, None, None);

        assert!(matches!(result, Err(PawError::NoCLIsFound)));
    }

    #[test]
    fn no_branches_available_returns_error() {
        let prompter = TrackingPrompter::cancel_on_branches();
        let branches: Vec<String> = vec![];
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, None, None);

        assert!(matches!(result, Err(PawError::BranchError(_))));
    }

    #[test]
    fn user_cancels_branch_selection_returns_cancelled() {
        let prompter = TrackingPrompter::cancel_on_branches();
        let branches = test_branches();
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, None, None);

        assert!(matches!(result, Err(PawError::UserCancelled)));
    }

    #[test]
    fn user_selects_no_branches_returns_cancelled() {
        // Empty branch_indices with cancel_on_branch_select=false still returns cancelled
        let prompter = TrackingPrompter::uniform(vec![], "alpha");
        let branches = test_branches();
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, None, None);

        assert!(matches!(result, Err(PawError::UserCancelled)));
    }

    #[test]
    fn user_cancels_cli_selection_returns_cancelled() {
        let prompter = TrackingPrompter::cancel_on_cli(vec![0]);
        let branches = test_branches();
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, None, None);

        assert!(matches!(result, Err(PawError::UserCancelled)));
    }

    // -----------------------------------------------------------------------
    // Behavior tests: selection with subset of branches
    // -----------------------------------------------------------------------

    #[test]
    fn selecting_subset_of_branches_works() {
        let prompter = TrackingPrompter::uniform(vec![1], "alpha"); // only fix/api
        let branches = test_branches();
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, None, None).unwrap();

        assert_eq!(
            result.mappings,
            vec![("fix/api".to_string(), "alpha".to_string())]
        );
    }
}

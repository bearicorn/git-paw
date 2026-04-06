//! Interactive selection prompts.
//!
//! User-facing selection flows using `dialoguer`: mode picker, branch picker,
//! and CLI picker (uniform and per-branch). Logic is separated from UI via
//! the [`Prompter`] trait for testability.

use std::fmt;

use dialoguer::{MultiSelect, Select};

use crate::config::PawConfig;
use crate::error::PawError;
use crate::specs::SpecEntry;

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

    fn select_cli(&self, clis: &[CliInfo], default: Option<&str>) -> Result<String, PawError> {
        let labels: Vec<String> = clis.iter().map(ToString::to_string).collect();

        let default_idx = default
            .and_then(|name| clis.iter().position(|c| c.binary_name == name))
            .unwrap_or(0);

        let selection = Select::new()
            .with_prompt("Select AI CLI for all branches")
            .items(&labels)
            .default(default_idx)
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
                let cli = prompter.select_cli(clis, None)?;
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

// ---------------------------------------------------------------------------
// Spec-driven CLI resolution
// ---------------------------------------------------------------------------

/// Resolves which CLI to use for each spec-driven branch using a 5-level
/// priority chain:
///
/// 1. `cli_flag` (from `--cli`) → all branches, no prompt
/// 2. `spec.cli` (`paw_cli` in spec) → that branch only
/// 3. `config.default_spec_cli` → remaining branches, no prompt
/// 4. `config.default_cli` → pre-selects in picker for remaining
/// 5. Nothing → full picker for remaining
///
/// Prompts at most once. Validates all resolved CLI names against
/// `available_clis`.
pub fn resolve_cli_for_specs(
    specs: &[SpecEntry],
    cli_flag: Option<&str>,
    config: &PawConfig,
    available_clis: &[CliInfo],
    prompter: &dyn Prompter,
) -> Result<Vec<(String, String)>, PawError> {
    let cli_exists = |name: &str| available_clis.iter().any(|c| c.binary_name == name);

    // Priority 1: --cli flag overrides everything
    if let Some(flag) = cli_flag {
        if !cli_exists(flag) {
            return Err(PawError::CliNotFound(flag.to_string()));
        }
        return Ok(specs
            .iter()
            .map(|s| (s.branch.clone(), flag.to_string()))
            .collect());
    }

    let mut mappings: Vec<(String, String)> = Vec::with_capacity(specs.len());
    let mut remaining: Vec<&SpecEntry> = Vec::new();

    // Priority 2: per-spec paw_cli
    for spec in specs {
        if let Some(ref cli_name) = spec.cli {
            if !cli_exists(cli_name) {
                return Err(PawError::CliNotFound(cli_name.clone()));
            }
            mappings.push((spec.branch.clone(), cli_name.clone()));
        } else {
            remaining.push(spec);
        }
    }

    if remaining.is_empty() {
        return Ok(mappings);
    }

    // Priority 3: default_spec_cli (no prompt)
    if let Some(ref spec_cli) = config.default_spec_cli {
        if !cli_exists(spec_cli) {
            return Err(PawError::CliNotFound(spec_cli.clone()));
        }
        for spec in &remaining {
            mappings.push((spec.branch.clone(), spec_cli.clone()));
        }
        return Ok(mappings);
    }

    // Priority 4+5: prompt once (pre-selected if default_cli set)
    let chosen = prompter.select_cli(available_clis, config.default_cli.as_deref())?;
    for spec in &remaining {
        mappings.push((spec.branch.clone(), chosen.clone()));
    }

    Ok(mappings)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Fake prompter for testing
    // -----------------------------------------------------------------------

    use std::cell::Cell;

    /// A configurable fake prompter that returns predetermined responses.
    /// Uses `Cell` for interior mutability to track per-branch call order
    /// and to capture the `default` parameter passed to `select_cli()`.
    struct TrackingPrompter {
        mode: CliMode,
        branch_indices: Vec<usize>,
        uniform_cli: String,
        per_branch_clis: Vec<String>,
        per_branch_call_count: Cell<usize>,
        cancel_on_branch_select: bool,
        cancel_on_cli_select: bool,
        /// Captures the `default` parameter passed to the last `select_cli()` call.
        last_select_cli_default: Cell<Option<String>>,
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
                last_select_cli_default: Cell::new(None),
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
                last_select_cli_default: Cell::new(None),
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
                last_select_cli_default: Cell::new(None),
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
                last_select_cli_default: Cell::new(None),
            }
        }

        /// Creates a prompter that returns a fixed CLI, used for spec resolution tests.
        fn for_specs(cli: &str) -> Self {
            Self {
                mode: CliMode::Uniform,
                branch_indices: vec![],
                uniform_cli: cli.to_string(),
                per_branch_clis: vec![],
                per_branch_call_count: Cell::new(0),
                cancel_on_branch_select: false,
                cancel_on_cli_select: false,
                last_select_cli_default: Cell::new(None),
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

        fn select_cli(&self, _clis: &[CliInfo], default: Option<&str>) -> Result<String, PawError> {
            self.last_select_cli_default.set(default.map(String::from));
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

    // -----------------------------------------------------------------------
    // resolve_cli_for_specs tests
    // -----------------------------------------------------------------------

    fn default_config() -> PawConfig {
        PawConfig::default()
    }

    fn spec(branch: &str, cli: Option<&str>) -> SpecEntry {
        SpecEntry {
            id: branch.to_string(),
            branch: branch.to_string(),
            cli: cli.map(String::from),
            prompt: String::new(),
            owned_files: None,
        }
    }

    fn test_specs() -> Vec<SpecEntry> {
        vec![
            spec("spec/auth", None),
            spec("spec/api", None),
            spec("spec/db", None),
        ]
    }

    #[test]
    fn cli_flag_overrides_all_specs() {
        let prompter = TrackingPrompter::for_specs("should-not-be-used");
        let clis = test_clis();
        let specs = test_specs();

        let result =
            resolve_cli_for_specs(&specs, Some("alpha"), &default_config(), &clis, &prompter)
                .unwrap();

        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|(_, cli)| cli == "alpha"));
    }

    #[test]
    fn paw_cli_per_spec_overrides_config() {
        let specs = vec![spec("spec/auth", Some("beta")), spec("spec/api", None)];
        let mut config = default_config();
        config.default_spec_cli = Some("alpha".to_string());
        let prompter = TrackingPrompter::for_specs("should-not-be-used");
        let clis = test_clis();

        let result = resolve_cli_for_specs(&specs, None, &config, &clis, &prompter).unwrap();

        assert!(result.iter().any(|(b, c)| b == "spec/auth" && c == "beta"));
        assert!(result.iter().any(|(b, c)| b == "spec/api" && c == "alpha"));
    }

    #[test]
    fn default_spec_cli_fills_remaining_without_prompt() {
        let mut config = default_config();
        config.default_spec_cli = Some("alpha".to_string());
        let prompter = TrackingPrompter::cancel_on_cli(vec![]); // would fail if called
        let clis = test_clis();
        let specs = test_specs();

        let result = resolve_cli_for_specs(&specs, None, &config, &clis, &prompter).unwrap();

        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|(_, cli)| cli == "alpha"));
    }

    #[test]
    fn default_cli_pre_selects_in_picker() {
        let mut config = default_config();
        config.default_cli = Some("beta".to_string());
        let prompter = TrackingPrompter::for_specs("beta");
        let clis = test_clis();
        let specs = vec![spec("spec/auth", None)];

        let result = resolve_cli_for_specs(&specs, None, &config, &clis, &prompter).unwrap();

        assert_eq!(result, vec![("spec/auth".to_string(), "beta".to_string())]);
        // Verify default was passed to select_cli
        assert_eq!(
            prompter.last_select_cli_default.take(),
            Some("beta".to_string())
        );
    }

    #[test]
    fn no_defaults_picker_fires_with_none_default() {
        let prompter = TrackingPrompter::for_specs("alpha");
        let clis = test_clis();
        let specs = vec![spec("spec/auth", None)];

        let result =
            resolve_cli_for_specs(&specs, None, &default_config(), &clis, &prompter).unwrap();

        assert_eq!(result, vec![("spec/auth".to_string(), "alpha".to_string())]);
        assert_eq!(prompter.last_select_cli_default.take(), None);
    }

    #[test]
    fn mixed_paw_cli_and_default_spec_cli() {
        let specs = vec![
            spec("spec/auth", Some("beta")),
            spec("spec/api", None),
            spec("spec/db", None),
        ];
        let mut config = default_config();
        config.default_spec_cli = Some("alpha".to_string());
        let prompter = TrackingPrompter::for_specs("should-not-be-used");
        let clis = test_clis();

        let result = resolve_cli_for_specs(&specs, None, &config, &clis, &prompter).unwrap();

        assert_eq!(result.len(), 3);
        assert!(result.iter().any(|(b, c)| b == "spec/auth" && c == "beta"));
        assert!(result.iter().any(|(b, c)| b == "spec/api" && c == "alpha"));
        assert!(result.iter().any(|(b, c)| b == "spec/db" && c == "alpha"));
    }

    #[test]
    fn mixed_paw_cli_and_interactive() {
        let specs = vec![
            spec("spec/auth", Some("beta")),
            spec("spec/api", None),
            spec("spec/db", None),
        ];
        let prompter = TrackingPrompter::for_specs("alpha");
        let clis = test_clis();

        let result =
            resolve_cli_for_specs(&specs, None, &default_config(), &clis, &prompter).unwrap();

        assert_eq!(result.len(), 3);
        assert!(result.iter().any(|(b, c)| b == "spec/auth" && c == "beta"));
        assert!(result.iter().any(|(b, c)| b == "spec/api" && c == "alpha"));
        assert!(result.iter().any(|(b, c)| b == "spec/db" && c == "alpha"));
    }

    #[test]
    fn picker_fires_at_most_once_for_multiple_remaining() {
        let specs = vec![
            spec("spec/a", Some("beta")),
            spec("spec/b", None),
            spec("spec/c", None),
            spec("spec/d", None),
        ];
        // If select_cli is called more than once this will still return "alpha",
        // but we verify the behavior: all remaining get the same CLI.
        let prompter = TrackingPrompter::for_specs("alpha");
        let clis = test_clis();

        let result =
            resolve_cli_for_specs(&specs, None, &default_config(), &clis, &prompter).unwrap();

        let remaining: Vec<_> = result.iter().filter(|(_, c)| c == "alpha").collect();
        assert_eq!(remaining.len(), 3);
    }

    #[test]
    fn all_resolved_via_flag_no_prompt() {
        let prompter = TrackingPrompter::cancel_on_cli(vec![]); // would fail if called
        let clis = test_clis();
        let specs = test_specs();

        let result =
            resolve_cli_for_specs(&specs, Some("alpha"), &default_config(), &clis, &prompter)
                .unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn all_resolved_via_paw_cli_and_default_spec_cli_no_prompt() {
        let specs = vec![spec("spec/auth", Some("alpha")), spec("spec/api", None)];
        let mut config = default_config();
        config.default_spec_cli = Some("beta".to_string());
        let prompter = TrackingPrompter::cancel_on_cli(vec![]); // would fail if called
        let clis = test_clis();

        let result = resolve_cli_for_specs(&specs, None, &config, &clis, &prompter).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn paw_cli_references_unknown_cli_returns_error() {
        let specs = vec![spec("spec/auth", Some("nonexistent"))];
        let prompter = TrackingPrompter::for_specs("alpha");
        let clis = test_clis();

        let result = resolve_cli_for_specs(&specs, None, &default_config(), &clis, &prompter);
        assert!(matches!(result, Err(PawError::CliNotFound(ref name)) if name == "nonexistent"));
    }

    #[test]
    fn default_spec_cli_references_unknown_cli_returns_error() {
        let mut config = default_config();
        config.default_spec_cli = Some("nonexistent".to_string());
        let prompter = TrackingPrompter::for_specs("alpha");
        let clis = test_clis();
        let specs = test_specs();

        let result = resolve_cli_for_specs(&specs, None, &config, &clis, &prompter);
        assert!(matches!(result, Err(PawError::CliNotFound(ref name)) if name == "nonexistent"));
    }

    #[test]
    fn cli_flag_references_unknown_cli_returns_error() {
        let prompter = TrackingPrompter::for_specs("alpha");
        let clis = test_clis();
        let specs = test_specs();

        let result = resolve_cli_for_specs(
            &specs,
            Some("nonexistent"),
            &default_config(),
            &clis,
            &prompter,
        );
        assert!(matches!(result, Err(PawError::CliNotFound(ref name)) if name == "nonexistent"));
    }

    #[test]
    fn select_cli_with_default_present_and_in_list() {
        let prompter = TrackingPrompter::for_specs("beta");
        let clis = test_clis();
        let specs = vec![spec("spec/x", None)];
        let mut config = default_config();
        config.default_cli = Some("beta".to_string());

        resolve_cli_for_specs(&specs, None, &config, &clis, &prompter).unwrap();

        assert_eq!(
            prompter.last_select_cli_default.take(),
            Some("beta".to_string())
        );
    }

    #[test]
    fn select_cli_with_default_not_in_list_graceful() {
        let prompter = TrackingPrompter::for_specs("alpha");
        let clis = test_clis();
        let specs = vec![spec("spec/x", None)];
        let mut config = default_config();
        config.default_cli = Some("nonexistent".to_string());

        // Should not error — the default just doesn't pre-select
        let result = resolve_cli_for_specs(&specs, None, &config, &clis, &prompter).unwrap();
        assert_eq!(result, vec![("spec/x".to_string(), "alpha".to_string())]);
        assert_eq!(
            prompter.last_select_cli_default.take(),
            Some("nonexistent".to_string())
        );
    }
}

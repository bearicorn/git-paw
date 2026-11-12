//! Live `dialoguer` prompt implementation of the [`Prompter`] seam.
//!
//! Holds [`TerminalPrompter`], the production prompter: the mode and CLI
//! single-selects use `dialoguer::Select`, while the branch and spec
//! multi-selects delegate to the `ratatui` fuzzy picker in
//! [`super::picker`] and map its result through the pure finalizers in
//! [`super::resolver`].

use dialoguer::Select;

use crate::error::PawError;
use crate::specs::SpecEntry;

use super::picker::fuzzy_multi_select;
use super::resolver::{finalize_branch_selection, finalize_spec_selection, group_specs_by_unit};
use super::{CliMode, Prompter, WorkerChoice, WorkerOptions};

/// Interactive prompter using `dialoguer` for terminal UI.
pub struct TerminalPrompter;

/// Shows one CLI picker over `options` and returns the chosen item's
/// mapping. Shared by both CLI pickers so uniform and per-branch mode list
/// and pre-select identically.
fn pick_worker(prompt: &str, options: &WorkerOptions<'_>) -> Result<WorkerChoice, PawError> {
    let items = options.items();
    let labels: Vec<&str> = items.iter().map(|(label, _)| label.as_str()).collect();

    let selection = Select::new()
        .with_prompt(prompt)
        .items(&labels)
        .default(options.default_index())
        .interact_opt()
        .map_err(|e| map_dialoguer_error(&e))?;

    match selection {
        Some(idx) => Ok(items[idx].1.clone()),
        None => Err(PawError::UserCancelled),
    }
}

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
        let selection = fuzzy_multi_select(
            "Select branches (type to filter, ctrl-u to clear, space to toggle, enter to confirm)",
            branches,
        )?;
        finalize_branch_selection(branches, selection)
    }

    fn select_cli(&self, options: &WorkerOptions<'_>) -> Result<WorkerChoice, PawError> {
        pick_worker("Select AI CLI for all branches", options)
    }

    fn select_cli_for_branch(
        &self,
        branch: &str,
        options: &WorkerOptions<'_>,
    ) -> Result<WorkerChoice, PawError> {
        pick_worker(&format!("Select CLI for {branch}"), options)
    }

    fn select_specs(&self, specs: &[SpecEntry]) -> Result<Vec<SpecEntry>, PawError> {
        let groups = group_specs_by_unit(specs);
        let labels: Vec<String> = groups.iter().map(|(label, _)| label.clone()).collect();

        let selection = fuzzy_multi_select(
            "Select specs (type to filter, ctrl-u to clear, space to toggle, enter to confirm)",
            &labels,
        )?;

        finalize_spec_selection(specs, &groups, selection)
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

#[cfg(test)]
mod tests {
    use super::super::CliInfo;
    use super::{WorkerChoice, WorkerOptions};

    fn cli(name: &str) -> CliInfo {
        CliInfo {
            display_name: name.to_string(),
            binary_name: name.to_string(),
        }
    }

    fn start(clis: &[CliInfo], default: Option<&str>) -> usize {
        let default = default.map(WorkerChoice::cli);
        WorkerOptions::clis_only(clis, default.as_ref()).default_index()
    }

    /// Spec scenario "Per-branch picker starts on a configured default" — the
    /// shared pre-selection rule both CLI pickers use.
    #[test]
    fn default_cli_index_picks_the_named_entry_and_otherwise_the_first() {
        let clis = [cli("alpha"), cli("beta")];
        assert_eq!(start(&clis, Some("beta")), 1);
        assert_eq!(start(&clis, Some("alpha")), 0);
        assert_eq!(start(&clis, None), 0);
        assert_eq!(start(&clis, Some("not-installed")), 0);
        assert_eq!(start(&[], Some("beta")), 0);
    }
}

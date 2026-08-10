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
use super::{CliInfo, CliMode, Prompter};

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
        let selection = fuzzy_multi_select(
            "Select branches (type to filter, ctrl-u to clear, space to toggle, enter to confirm)",
            branches,
        )?;
        finalize_branch_selection(branches, selection)
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

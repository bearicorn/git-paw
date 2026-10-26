//! `git paw list-clis` / `add-cli` / `remove-cli` — the custom-CLI registry
//! handlers.

use crate::config;
use crate::detect;
use crate::error::PawError;

use super::helpers::config_to_custom_defs;
use super::{CommandContext, Output};

/// One detected or custom CLI, as [`list_clis`] reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedCli {
    /// The binary name.
    pub name: String,
    /// Where it came from (`detected` or `custom`).
    pub source: String,
}

/// Lists all detected and custom AI CLIs, and returns them.
///
/// # Errors
///
/// Returns an error when the repository's config cannot be loaded.
pub fn list_clis(ctx: &CommandContext, out: &mut dyn Output) -> Result<Vec<ListedCli>, PawError> {
    let custom_defs = config_to_custom_defs(ctx.config()?);
    let clis = detect::detect_clis(&custom_defs);

    if clis.is_empty() {
        out.line("No AI CLIs found.");
        out.line("Install one or use `git paw add-cli` to register a custom CLI.");
        return Ok(Vec::new());
    }

    out.line(&format!("{:<15} {:<10} PATH", "NAME", "SOURCE"));
    for cli in &clis {
        out.line(&format!(
            "{:<15} {:<10} {}",
            cli.binary_name,
            cli.source,
            cli.path.display()
        ));
    }

    Ok(clis
        .iter()
        .map(|cli| ListedCli {
            name: cli.binary_name.clone(),
            source: cli.source.to_string(),
        })
        .collect())
}

/// Options for [`add_cli`]: the `git paw add-cli` arguments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AddCliOptions {
    /// The CLI's name.
    pub name: String,
    /// The command it runs.
    pub command: String,
    /// `--display-name`.
    pub display_name: Option<String>,
}

/// Registers a custom AI CLI in the global config.
///
/// # Errors
///
/// Returns an error when the global config cannot be read or written.
pub fn add_cli(opts: &AddCliOptions, out: &mut dyn Output) -> Result<(), PawError> {
    let name = opts.name.as_str();
    config::add_custom_cli(name, &opts.command, opts.display_name.as_deref())?;
    out.line(&format!("Added custom CLI '{name}'."));
    Ok(())
}

/// Removes a custom AI CLI from the global config.
///
/// `repo` is the repository the command runs in, when it runs in one: there,
/// an auto-detected CLI that is not a custom one is refused by name.
///
/// # Errors
///
/// Returns an error when the CLI is auto-detected rather than custom, or the
/// config cannot be read or written.
pub fn remove_cli(
    repo: Option<&CommandContext>,
    name: &str,
    out: &mut dyn Output,
) -> Result<(), PawError> {
    // Try to load config to check if it's a custom CLI
    // If we're not in a repo, just attempt removal directly
    if let Some(ctx) = repo {
        let config = ctx.config()?;
        if !config.clis.contains_key(name) {
            // Check if it's a known auto-detected CLI
            let detected = detect::detect_known_clis();
            if detected.iter().any(|c| c.binary_name == name) {
                return Err(PawError::CliNotFound(format!(
                    "CLI '{name}' is auto-detected, not a custom CLI. Cannot remove."
                )));
            }
        }
    }

    config::remove_custom_cli(name)?;
    out.line(&format!("Removed custom CLI '{name}'."));
    Ok(())
}

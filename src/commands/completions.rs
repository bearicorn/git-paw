//! `git paw completions` — print a shell completion script to stdout.

use clap::CommandFactory;
use clap_complete::{Shell, generate};

use git_paw::cli::Cli;
use git_paw::error::PawError;

/// Prints a shell completion script for `shell` to stdout, generated from
/// the clap command definition.
///
/// `clap_complete::generate` never fails for a valid `Shell` value — clap
/// has already rejected an unsupported shell argument during parsing — so
/// this always returns `Ok`. The `Result` return type is kept anyway so
/// every dispatch arm in `main::run` shares one signature.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn cmd_completions(shell: Shell) -> Result<(), PawError> {
    let mut command = Cli::command();
    let bin_name = command.get_name().to_string();
    generate(shell, &mut command, bin_name, &mut std::io::stdout());
    Ok(())
}

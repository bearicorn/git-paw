//! The `git paw setup-cli` use case.
//!
//! Resolves the CLI identity, looks up its setup fact row, plans (or plans an
//! undo), and applies. It never prompts and never reads stdin: the decision to
//! write belongs to the entry layer in `main.rs`, which prints the plan and
//! asks. That split is what lets the whole thing be driven from a test without
//! a terminal.

use std::path::Path;

use git_paw::cli_setup::facts::{self, CliSetupFacts, SetupItem};
use git_paw::cli_setup::plan::{PlanOutcome, SetupPlan, Snapshots, plan};
use git_paw::cli_setup::record::SetupRecord;
use git_paw::cli_setup::shell::{self, ApplyOutcome, RepoLayout, UndoOutcome};
use git_paw::cli_setup::undo::{UndoPlan, plan_undo};
use git_paw::command_runner::CommandRunner;
use git_paw::config::PawConfig;
use git_paw::error::PawError;

/// What the caller asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetupRequest {
    /// The CLI identity named on the command line.
    pub cli: String,
    /// The items named by flags, in flag order.
    pub items: Vec<SetupItem>,
    /// Whether this is an undo.
    pub undo: bool,
}

/// A prepared run: everything decided, nothing written yet.
#[derive(Debug)]
pub(crate) enum Prepared {
    /// One or more requested items are not declared for this CLI.
    Unsupported {
        /// The CLI identity.
        cli: String,
        /// The items that are not declared.
        items: Vec<SetupItem>,
    },
    /// A setup plan.
    Setup {
        /// The plan.
        plan: Box<SetupPlan>,
        /// The snapshots it was planned against.
        snapshots: Box<Snapshots>,
    },
    /// An undo plan.
    Undo {
        /// The plan.
        plan: Box<UndoPlan>,
        /// The record it was planned against.
        record: Box<SetupRecord>,
        /// The snapshots it was planned against.
        snapshots: Box<Snapshots>,
    },
}

/// Resolves the identity, reads the world and plans — writing nothing.
///
/// # Errors
///
/// Returns [`PawError::CliNotFound`] for an identity git-paw does not know, a
/// refusal for an unsafe target, or an I/O failure.
pub(crate) fn prepare(
    runner: &dyn CommandRunner,
    cwd: &Path,
    config: &PawConfig,
    request: &SetupRequest,
) -> Result<Prepared, PawError> {
    let layout = shell::resolve_layout(runner, cwd)?;
    shell::require_main_checkout(&layout)?;

    if !is_known_identity(config, &request.cli) {
        return Err(PawError::CliNotFound(request.cli.clone()));
    }
    // An identity git-paw knows but has no fact row for supports no item.
    // Reported as unsupported, and resolved before any file is touched.
    let Some(row) = facts::row_for(&request.cli) else {
        return Ok(Prepared::Unsupported {
            cli: request.cli.clone(),
            items: request.items.clone(),
        });
    };
    prepare_with_row(runner, &layout, row, request)
}

/// The same, against an explicit fact row.
///
/// Integration tests use this to exercise the writing paths with a
/// constructed verified row, so the machinery is covered whatever the shipped
/// row's verification status is.
///
/// # Errors
///
/// Returns a [`PawError`] when the world cannot be read or a target is unsafe.
pub(crate) fn prepare_with_row(
    runner: &dyn CommandRunner,
    layout: &RepoLayout,
    row: &CliSetupFacts,
    request: &SetupRequest,
) -> Result<Prepared, PawError> {
    let snapshots = shell::collect_snapshots(runner, layout, row)?;

    if request.undo {
        let record = shell::load_record(&snapshots.record_path)?;
        let undo = plan_undo(&record, row, &request.items, &snapshots)
            .map_err(|r| PawError::ConfigError(r.to_string()))?;
        return Ok(Prepared::Undo {
            plan: Box::new(undo),
            record: Box::new(record),
            snapshots: Box::new(snapshots),
        });
    }

    match plan(row, &request.items, &snapshots).map_err(|r| PawError::ConfigError(r.to_string()))? {
        PlanOutcome::Unsupported { cli, items } => Ok(Prepared::Unsupported { cli, items }),
        PlanOutcome::Plan(p) => Ok(Prepared::Setup {
            plan: p,
            snapshots: Box::new(snapshots),
        }),
    }
}

/// Applies a prepared setup plan.
///
/// # Errors
///
/// Returns a [`PawError`] when a target changed since the plan was made or a
/// file cannot be written.
pub(crate) fn apply_setup(
    plan: &SetupPlan,
    snapshots: &Snapshots,
    now: &str,
) -> Result<ApplyOutcome, PawError> {
    shell::apply(plan, snapshots, env!("CARGO_PKG_VERSION"), now)
}

/// Applies a prepared undo plan.
///
/// # Errors
///
/// Returns a [`PawError`] when a target changed since the plan was made or a
/// file cannot be written.
pub(crate) fn apply_undo(
    plan: &UndoPlan,
    record: &SetupRecord,
    snapshots: &Snapshots,
) -> Result<UndoOutcome, PawError> {
    shell::apply_undo(plan, record, snapshots)
}

/// Returns whether git-paw knows `identity` at all.
fn is_known_identity(config: &PawConfig, identity: &str) -> bool {
    config.clis.contains_key(identity) || git_paw::detect::is_known_cli(identity)
}

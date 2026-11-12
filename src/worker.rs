//! Worker choice: what a branch's coding agent launches with.
//!
//! A worker is chosen either as a plain CLI or as a worker profile
//! ([`WorkerChoice`]); the two stay distinct even when a profile and a CLI
//! share a name. A choice resolves against the configuration into a
//! [`WorkerLaunch`] — the CLI id plus whatever the profile adds (launch
//! arguments, approval level, role skill) — which every worker launch path
//! builds its pane command and managed block from.
//!
//! The precedence rules for the paths without specs ([`resolve_start_worker`])
//! and for `git paw add` ([`resolve_add_worker`]) are pure functions of their
//! inputs, so each path reaches the same answer for the same flags and
//! configuration.

use std::fmt;
use std::path::Path;

use crate::agent_cli::{CliProfile, Resolved};
use crate::config::{ApprovalLevel, PawConfig, ProfileEntry, ProfileKind, ProfileName};
use crate::error::PawError;
use crate::specs::SpecEntry;

/// A worker as chosen by flag, configuration, spec or picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerChoice {
    /// A plain CLI id (a detected CLI or a `[clis.<name>]` key).
    Cli(String),
    /// A worker profile (`[profiles.<name>]` with `kind = "worker"`).
    Profile(ProfileName),
}

impl WorkerChoice {
    /// A plain CLI choice.
    #[must_use]
    pub fn cli(id: &str) -> Self {
        Self::Cli(id.to_string())
    }

    /// The profile name, when this is a profile choice.
    #[must_use]
    pub fn profile_name(&self) -> Option<&ProfileName> {
        match self {
            Self::Cli(_) => None,
            Self::Profile(name) => Some(name),
        }
    }
}

/// Displays the CLI id or the profile name.
impl fmt::Display for WorkerChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cli(id) => f.write_str(id),
            Self::Profile(name) => f.write_str(name.as_str()),
        }
    }
}

/// Looks up the worker profile `name`, consulted from `source` (such as
/// `--profile` or `default_profile`).
///
/// # Errors
///
/// [`PawError::ConfigError`] naming `name` and `source` when no profile of
/// that name is configured, or when it is a delegate profile.
pub fn worker_profile<'a>(
    config: &'a PawConfig,
    name: &str,
    source: &str,
) -> Result<(&'a ProfileName, &'a ProfileEntry), PawError> {
    let Some((key, entry)) = config.profiles.iter().find(|(k, _)| k.as_str() == name) else {
        return Err(PawError::ConfigError(format!(
            "{source} names profile '{name}', but no [profiles.{name}] is configured"
        )));
    };
    if entry.kind == ProfileKind::Delegate {
        return Err(PawError::ConfigError(format!(
            "{source} names profile '{name}', but '{name}' is a delegate profile; \
             a worker launch needs a worker profile (run delegates with `git paw delegate {name}`)"
        )));
    }
    Ok((key, entry))
}

/// The configured default worker: `default_profile` when set (validated as a
/// worker profile), otherwise `default_cli`.
///
/// # Errors
///
/// When `default_profile` names no profile or a delegate profile; the error
/// names `default_profile`.
pub fn configured_default(config: &PawConfig) -> Result<Option<WorkerChoice>, PawError> {
    if let Some(name) = &config.default_profile {
        let (key, _) = worker_profile(config, name.as_str(), "default_profile")?;
        return Ok(Some(WorkerChoice::Profile(key.clone())));
    }
    Ok(config.default_cli.as_deref().map(WorkerChoice::cli))
}

/// The worker for a `--profile` flag value.
///
/// # Errors
///
/// As [`worker_profile`], naming `--profile`.
pub fn profile_flag_choice(config: &PawConfig, name: &str) -> Result<WorkerChoice, PawError> {
    let (key, _) = worker_profile(config, name, "--profile")?;
    Ok(WorkerChoice::Profile(key.clone()))
}

/// A spec's own worker override: `paw_profile` or `paw_cli`.
///
/// # Errors
///
/// When the spec sets both, or its `paw_profile` names no profile or a
/// delegate profile; each error names the spec.
pub fn spec_choice(config: &PawConfig, spec: &SpecEntry) -> Result<Option<WorkerChoice>, PawError> {
    match (&spec.profile, &spec.cli) {
        (Some(_), Some(_)) => Err(PawError::ConfigError(format!(
            "spec '{}' sets both paw_profile and paw_cli; keep one",
            spec.id
        ))),
        (Some(profile), None) => {
            let source = format!("spec '{}' paw_profile", spec.id);
            let (key, _) = worker_profile(config, profile, &source)?;
            Ok(Some(WorkerChoice::Profile(key.clone())))
        }
        (None, Some(cli)) => Ok(Some(WorkerChoice::Cli(cli.clone()))),
        (None, None) => Ok(None),
    }
}

/// How a launch without specs chooses each branch's worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartWorker {
    /// Every branch launches with this worker; no CLI picker is shown.
    Fixed(WorkerChoice),
    /// The interactive selection chooses, its picker starting on `default`.
    Picker {
        /// The configured default the picker starts on.
        default: Option<WorkerChoice>,
    },
}

/// Resolves the worker for a launch without specs: `--profile` or `--cli`
/// for every branch, otherwise a `--preset`'s CLI, otherwise the interactive
/// selection starting on `default_profile`, else `default_cli`.
///
/// # Errors
///
/// When `--profile` or `default_profile` names no worker profile.
pub fn resolve_start_worker(
    config: &PawConfig,
    profile_flag: Option<&str>,
    cli_flag: Option<&str>,
    preset_cli: Option<&str>,
) -> Result<StartWorker, PawError> {
    if let Some(name) = profile_flag {
        return Ok(StartWorker::Fixed(profile_flag_choice(config, name)?));
    }
    if let Some(cli) = cli_flag.or(preset_cli) {
        return Ok(StartWorker::Fixed(WorkerChoice::cli(cli)));
    }
    Ok(StartWorker::Picker {
        default: configured_default(config)?,
    })
}

/// Resolves the worker for `git paw add`: `--profile` or `--cli`; then the
/// spec's `paw_profile` or `paw_cli`; then the session default — the first
/// worktree's recorded profile, else its CLI; then `default_profile`; then
/// `default_cli`. `None` when nothing resolves.
///
/// `session_first` is the first worktree's recorded `(cli, profile)`.
///
/// # Errors
///
/// As [`profile_flag_choice`], [`spec_choice`] and [`configured_default`].
pub fn resolve_add_worker(
    config: &PawConfig,
    profile_flag: Option<&str>,
    cli_flag: Option<&str>,
    spec: Option<&SpecEntry>,
    session_first: Option<(&str, Option<&str>)>,
) -> Result<Option<WorkerChoice>, PawError> {
    if let Some(name) = profile_flag {
        return profile_flag_choice(config, name).map(Some);
    }
    if let Some(cli) = cli_flag {
        return Ok(Some(WorkerChoice::cli(cli)));
    }
    if let Some(spec) = spec
        && let Some(choice) = spec_choice(config, spec)?
    {
        return Ok(Some(choice));
    }
    if let Some((cli, profile)) = session_first {
        return Ok(Some(match profile {
            Some(name) => {
                let (key, _) = worker_profile(config, name, "the session's recorded profile")?;
                WorkerChoice::Profile(key.clone())
            }
            None => WorkerChoice::cli(cli),
        }));
    }
    configured_default(config)
}

/// A worker choice resolved against the configuration: everything a launch
/// needs to build the worker's pane command and managed block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerLaunch {
    /// The CLI id the worker runs (a profile's `cli`, or the plain CLI).
    pub cli: String,
    /// The worker profile, when launched with one.
    pub profile: Option<ProfileName>,
    /// The profile's launch arguments.
    pub profile_args: Option<String>,
    /// The level this worker runs at: the profile's `approval`, else the
    /// worker default.
    pub approval: ApprovalLevel,
    /// The role skill written after the coordination skill.
    pub role_skill: Option<String>,
}

impl WorkerLaunch {
    /// A worker launched with the plain CLI `cli` at `approval`.
    #[must_use]
    pub fn plain(cli: &str, approval: ApprovalLevel) -> Self {
        Self {
            cli: cli.to_string(),
            profile: None,
            profile_args: None,
            approval,
            role_skill: None,
        }
    }

    /// Resolves `choice` against `config`, a profile's level defaulting to
    /// `worker_default`.
    ///
    /// # Errors
    ///
    /// When a profile choice names no worker profile.
    pub fn resolve(
        choice: &WorkerChoice,
        config: &PawConfig,
        worker_default: ApprovalLevel,
    ) -> Result<Self, PawError> {
        match choice {
            WorkerChoice::Cli(cli) => Ok(Self::plain(cli, worker_default)),
            WorkerChoice::Profile(name) => {
                let (key, entry) = worker_profile(config, name.as_str(), "the worker choice")?;
                Ok(Self::from_profile(key, entry, worker_default))
            }
        }
    }

    /// The launch a worker profile describes.
    #[must_use]
    pub fn from_profile(
        name: &ProfileName,
        entry: &ProfileEntry,
        worker_default: ApprovalLevel,
    ) -> Self {
        Self {
            cli: entry.cli.clone(),
            profile: Some(name.clone()),
            profile_args: entry.args.clone(),
            approval: entry.effective_approval(worker_default),
            role_skill: entry.worker_role_skill().map(str::to_string),
        }
    }

    /// The launch for a recorded worker: its recorded `profile` resolved
    /// from the current configuration, else its recorded `cli` at
    /// `worker_default`. A recorded profile that no longer exists, or is no
    /// longer a worker profile, falls back to the recorded CLI and returns a
    /// warning naming `branch` and the profile.
    #[must_use]
    pub fn recorded(
        branch: &str,
        cli: &str,
        profile: Option<&str>,
        profiles: &std::collections::BTreeMap<ProfileName, ProfileEntry>,
        worker_default: ApprovalLevel,
    ) -> (Self, Option<String>) {
        let Some(name) = profile else {
            return (Self::plain(cli, worker_default), None);
        };
        match profiles.iter().find(|(k, _)| k.as_str() == name) {
            Some((key, entry)) if entry.kind == ProfileKind::Worker => {
                (Self::from_profile(key, entry, worker_default), None)
            }
            _ => (
                Self::plain(cli, worker_default),
                Some(format!(
                    "warning: {branch}: recorded worker profile '{name}' is no longer a \
                     configured worker profile; relaunching with its recorded CLI '{cli}'"
                )),
            ),
        }
    }

    /// The profile name as a string, when launched with one.
    #[must_use]
    pub fn profile_str(&self) -> Option<&str> {
        self.profile.as_ref().map(ProfileName::as_str)
    }

    /// The CLI record this worker launches with, carrying the profile's
    /// arguments.
    #[must_use]
    pub fn cli_profile<S: std::hash::BuildHasher>(
        &self,
        clis: &std::collections::HashMap<String, crate::config::CustomCli, S>,
    ) -> CliProfile {
        CliProfile::resolve(&self.cli, clis).with_profile_args(self.profile_args.as_deref())
    }

    /// Resolves this worker's CLI record for a launch at its own level
    /// ([`crate::agent_cli::resolve_for_launch`]), first refusing a profile
    /// whose CLI is not available or whose role skill does not resolve, so
    /// the launch fails before any worktree exists.
    ///
    /// # Errors
    ///
    /// The refusals above, each naming the profile, plus every
    /// [`crate::agent_cli::resolve_for_launch`] refusal.
    pub fn resolve_for_launch<S: std::hash::BuildHasher>(
        &self,
        clis: &std::collections::HashMap<String, crate::config::CustomCli, S>,
        repo_root: &Path,
    ) -> Result<Resolved, PawError> {
        if let Some(profile) = &self.profile {
            if CliProfile::resolve(&self.cli, clis)
                .executable_in(None, repo_root)
                .is_none()
            {
                return Err(PawError::ConfigError(format!(
                    "profile '{profile}' uses CLI '{cli}', which is not available \
                     (not installed or not on PATH); install it or point [profiles.{profile}] \
                     cli at an available CLI",
                    cli = self.cli
                )));
            }
            if let Some(skill) = &self.role_skill {
                crate::skills::resolve(skill, repo_root).map_err(|e| {
                    PawError::ConfigError(format!(
                        "profile '{profile}' role skill '{skill}' does not resolve: {e}"
                    ))
                })?;
            }
        }
        Ok(
            crate::agent_cli::resolve_for_launch(&self.cli, clis, &self.approval, repo_root)?
                .with_profile_args(self.profile_args.as_deref()),
        )
    }
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;

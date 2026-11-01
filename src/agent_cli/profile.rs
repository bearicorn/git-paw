//! [`CliProfile`]: the record one agent CLI resolves into.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::hash::BuildHasher;
use std::path::{Path, PathBuf};

use super::{CliFamily, DialogMarker, roster_entry};
use crate::config::{
    ApprovalLevel, CliPromptProfile, CompiledPromptProfile, CustomCli, SESSION_ID_PLACEHOLDER,
    default_compiled_prompt_profile, launch_command, merge_prompt_profile,
};
use crate::error::PawError;

/// Which session arguments a launch carries after the approval flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchSession<'a> {
    /// No session arguments.
    None,
    /// A fresh launch assigned this session id (`session_id_args`).
    Assign(&'a str),
    /// A recovery resuming this recorded session id (`resume_args`).
    Resume(&'a str),
}

/// The resolved record of one agent CLI (`cli-provider` capability).
///
/// Consumers read per-CLI values from here and never re-derive a CLI's
/// identity from a launch-command string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliProfile {
    id: String,
    base_command: String,
    entry: Option<CustomCli>,
    family: CliFamily,
}

impl CliProfile {
    /// Resolves the record for the CLI identity `identity` against `clis`.
    ///
    /// The family is never inferred from `command`: it is the declared
    /// `family`, else the known name's own, else `generic`. Config load
    /// rejects an unparseable `family` before it reaches here.
    #[must_use]
    pub fn resolve<S: BuildHasher>(identity: &str, clis: &HashMap<String, CustomCli, S>) -> Self {
        let id = identity
            .split_whitespace()
            .next()
            .unwrap_or(identity)
            .to_string();
        let entry = clis.get(&id).cloned();
        let family = entry
            .as_ref()
            .and_then(|e| e.family.as_deref())
            .and_then(|f| f.parse().ok())
            .or_else(|| CliFamily::of_known(&id))
            .unwrap_or(CliFamily::Generic);
        Self {
            base_command: launch_command(identity, clis),
            id,
            entry,
            family,
        }
    }

    /// The record of an unconfigured CLI of `family`. Needs no PATH or config
    /// file, so each family can be exercised directly against recorded inputs.
    #[must_use]
    pub fn for_family(family: CliFamily) -> Self {
        let id = family.as_str().to_string();
        Self {
            base_command: id.clone(),
            id,
            entry: None,
            family,
        }
    }

    /// The CLI id: the `[clis.<name>]` key or known binary name.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The provider family.
    #[must_use]
    pub fn family(&self) -> CliFamily {
        self.family
    }

    /// The display name: the entry's `display_name`, else the known CLI's
    /// product name, else the capitalised id.
    #[must_use]
    pub fn display_name(&self) -> String {
        if let Some(name) = self.entry.as_ref().and_then(|e| e.display_name.clone()) {
            return name;
        }
        if let Some(name) = roster_entry(&self.id).and_then(|e| e.display_name) {
            return name.to_string();
        }
        let mut chars = self.id.chars();
        chars.next().map_or_else(String::new, |c| {
            c.to_uppercase().to_string() + chars.as_str()
        })
    }

    /// The executable as configured: the entry's `command`, or the id for a
    /// CLI with no entry (launched by name).
    #[must_use]
    pub fn command(&self) -> &str {
        self.entry.as_ref().map_or(&self.id, |e| &e.command)
    }

    /// The `[clis.<id>].args` launch arguments, if set.
    #[must_use]
    pub fn args(&self) -> Option<&str> {
        self.entry.as_ref().and_then(|e| e.args.as_deref())
    }

    /// Whether the id is a known CLI name from the detection roster.
    #[must_use]
    pub fn is_known(&self) -> bool {
        roster_entry(&self.id).is_some()
    }

    /// Native permission flags for `level`: the `[clis.<id>].approval_args`
    /// override, else the family's built-in row, else `""`.
    #[must_use]
    pub fn approval_flags(&self, level: &ApprovalLevel) -> String {
        self.entry
            .as_ref()
            .and_then(|e| e.approval_args.get(level.kebab_name()))
            .cloned()
            .unwrap_or_else(|| self.family.provider().approval_flags(*level).to_string())
    }

    /// The session-id template rendered for `session_id`: the override, else
    /// the family's row, else `None`.
    #[must_use]
    pub fn session_id_args(&self, session_id: &str) -> Option<String> {
        self.entry
            .as_ref()
            .and_then(|e| e.session_id_args.as_deref())
            .or_else(|| self.family.provider().session_id_args())
            .map(|t| t.replace(SESSION_ID_PLACEHOLDER, session_id))
    }

    /// The resume template rendered for `session_id`: the override, else the
    /// family's row, else `None`.
    #[must_use]
    pub fn resume_args(&self, session_id: &str) -> Option<String> {
        self.entry
            .as_ref()
            .and_then(|e| e.resume_args.as_deref())
            .or_else(|| self.family.provider().resume_args())
            .map(|t| t.replace(SESSION_ID_PLACEHOLDER, session_id))
    }

    /// The headless one-shot arguments: the override, else the family's row,
    /// else `None`.
    #[must_use]
    pub fn headless_args(&self) -> Option<String> {
        self.entry
            .as_ref()
            .and_then(|e| e.headless_args.clone())
            .or_else(|| self.family.provider().headless_args().map(str::to_string))
    }

    /// Whether the CLI speaks git-paw's MCP protocol: an explicit
    /// `[clis.<id>] mcp` wins, else the family's default.
    #[must_use]
    pub fn mcp_capable(&self) -> bool {
        self.entry
            .as_ref()
            .and_then(|e| e.mcp)
            .unwrap_or_else(|| self.family.provider().mcp_capable())
    }

    /// The prompt-shape profile: the `[clis.<id>].prompt_profile` override
    /// field by field over the family's profile.
    #[must_use]
    pub fn prompt_profile(&self) -> CliPromptProfile {
        merge_prompt_profile(
            self.family.provider().prompt_profile(),
            self.entry.as_ref().and_then(|e| e.prompt_profile.as_ref()),
        )
    }

    /// [`Self::prompt_profile`] with its pattern fields compiled — the form
    /// every detector takes.
    ///
    /// # Errors
    ///
    /// [`PawError::ConfigError`] naming `[clis.<id>].prompt_profile.<field>`
    /// for a pattern that does not compile. Config load rejects such a
    /// pattern first, so only a table that bypassed the load can reach this.
    pub fn compiled_prompt_profile(&self) -> Result<CompiledPromptProfile, PawError> {
        CompiledPromptProfile::compile(self.prompt_profile()).map_err(|e| {
            PawError::ConfigError(format!(
                "[clis.{}].prompt_profile.{}: {}",
                self.id, e.field, e.error
            ))
        })
    }

    /// The first-run dialogs the launch gate answers for this CLI: its
    /// family's, never anything from configuration.
    #[must_use]
    pub fn dialog_markers(&self) -> &'static [DialogMarker] {
        self.family.provider().dialog_markers()
    }

    /// The boot-prompt settle delay: the override, else
    /// [`crate::DEFAULT_SUBMIT_DELAY_MS`].
    #[must_use]
    pub fn submit_delay_ms(&self) -> u64 {
        self.entry
            .as_ref()
            .and_then(|e| e.submit_delay_ms)
            .unwrap_or(crate::DEFAULT_SUBMIT_DELAY_MS)
    }

    /// The pane command for one launch: the executable, the CLI's `args`, the
    /// approval flags for `level`, then the session-id arguments (fresh
    /// launch) or resume arguments (recovery). An empty part contributes no
    /// separator.
    ///
    /// The executable is a configured entry's `command` as one shell word, or
    /// a built-in CLI's name, resolved by the pane's shell.
    #[must_use]
    pub fn launch_command(&self, level: &ApprovalLevel, session: LaunchSession<'_>) -> String {
        let session_args = match session {
            LaunchSession::None => None,
            LaunchSession::Assign(id) => self.session_id_args(id),
            LaunchSession::Resume(id) => self.resume_args(id),
        };
        let mut command = self.base_command.clone();
        for part in [
            self.args().unwrap_or_default(),
            &self.approval_flags(level),
            session_args.as_deref().unwrap_or_default(),
        ] {
            if !part.trim().is_empty() {
                command.push(' ');
                command.push_str(part);
            }
        }
        command
    }

    /// The executable the launch runs, resolved against `path` (a PATH-style
    /// list; `None` uses the process `PATH`), or `None` when it is not found.
    #[must_use]
    pub fn executable_in(&self, path: Option<&OsStr>) -> Option<PathBuf> {
        locate(self.command(), path)
    }

    /// [`Self::executable_in`] against the process `PATH`.
    #[must_use]
    pub fn executable(&self) -> Option<PathBuf> {
        self.executable_in(None)
    }

    /// Refuses the ambiguous PATH shim: a `[clis.<id>]` key that is not a
    /// known CLI name and also resolves on `path` to a different file than
    /// the entry's `command` (compared after resolving symlinks).
    ///
    /// # Errors
    ///
    /// [`PawError::ConfigError`] naming both paths and the fix. A known
    /// name, a CLI without an entry, or a key not on `path` is never refused.
    pub fn ensure_unambiguous_in(&self, path: Option<&OsStr>) -> Result<(), PawError> {
        if self.entry.is_none() || self.is_known() {
            return Ok(());
        }
        let (Some(shim), Some(command)) = (locate(&self.id, path), self.executable_in(path)) else {
            return Ok(());
        };
        if canonical(&shim) == canonical(&command) {
            return Ok(());
        }
        Err(PawError::ConfigError(format!(
            "[clis.{id}] is ambiguous: `{id}` on PATH resolves to {shim}, but its configured \
             command resolves to {command}. Rename the [clis.{id}] key, or set \
             [clis.{id}] command to the full path of the executable you mean",
            id = self.id,
            shim = shim.display(),
            command = command.display(),
        )))
    }

    /// [`Self::ensure_unambiguous_in`] against the process `PATH`.
    ///
    /// # Errors
    ///
    /// As [`Self::ensure_unambiguous_in`].
    pub fn ensure_unambiguous(&self) -> Result<(), PawError> {
        self.ensure_unambiguous_in(None)
    }
}

/// Resolves the record for `identity` for a launch, refusing the ambiguous
/// PATH shim ([`CliProfile::ensure_unambiguous`]). Every worker launch path
/// goes through this before it builds a pane command.
///
/// # Errors
///
/// When the identity is an ambiguous shim.
pub fn resolve_for_launch<S: BuildHasher>(
    identity: &str,
    clis: &HashMap<String, CustomCli, S>,
) -> Result<CliProfile, PawError> {
    let profile = CliProfile::resolve(identity, clis);
    profile.ensure_unambiguous()?;
    Ok(profile)
}

/// What the drive loop reads about one agent's CLI: the prompt-shape profile
/// its surface is inspected with and its nudge settle delay.
#[derive(Debug, Clone)]
pub struct AgentCli {
    /// The compiled prompt-shape profile.
    pub prompt: CompiledPromptProfile,
    /// The settle delay between typing a nudge and submitting it, in ms.
    pub submit_delay_ms: u64,
}

impl AgentCli {
    fn of(profile: &CliProfile) -> Result<Self, PawError> {
        Ok(Self {
            prompt: profile.compiled_prompt_profile()?,
            submit_delay_ms: profile.submit_delay_ms(),
        })
    }
}

/// The `agent id → CLI` map of one session (design D2 of
/// `cli-profile-threading`).
///
/// Each agent's CLI comes from git-paw's own launch record — the session
/// state's recorded CLI — and never from anything an agent publishes to the
/// broker. It is resolved once per recorded CLI and reused for every decision
/// about that agent's surface; re-recording the agent with a different CLI
/// (a kept worktree re-added with another CLI) resolves again. An agent with
/// no recorded CLI resolves the `generic` family.
#[derive(Debug, Clone)]
pub struct AgentProfiles {
    clis: HashMap<String, CustomCli>,
    /// Per agent: the recorded CLI identity and what it resolved to.
    agents: HashMap<String, (String, AgentCli)>,
    generic: AgentCli,
}

impl AgentProfiles {
    /// An empty map resolving against the merged `[clis]` table `clis`.
    #[must_use]
    pub fn new(clis: HashMap<String, CustomCli>) -> Self {
        let generic = CliProfile::for_family(CliFamily::Generic);
        Self {
            clis,
            agents: HashMap::new(),
            generic: AgentCli {
                prompt: default_compiled_prompt_profile().clone(),
                submit_delay_ms: generic.submit_delay_ms(),
            },
        }
    }

    /// Records `agent_id`'s launch CLI `cli`. The same CLI recorded again is
    /// not re-resolved; a different one replaces the agent's profile.
    ///
    /// A blank `cli` clears the record, so the agent resolves `generic`. An
    /// override pattern that does not compile (only reachable when the table
    /// bypassed config load) is reported on stderr and the agent resolves
    /// `generic`.
    pub fn record(&mut self, agent_id: &str, cli: &str) {
        if self
            .agents
            .get(agent_id)
            .is_some_and(|(seen, _)| seen == cli)
        {
            return;
        }
        self.agents.remove(agent_id);
        if cli.trim().is_empty() {
            return;
        }
        match AgentCli::of(&CliProfile::resolve(cli, &self.clis)) {
            Ok(resolved) => {
                self.agents
                    .insert(agent_id.to_string(), (cli.to_string(), resolved));
            }
            Err(e) => eprintln!("warning: agent {agent_id}: {e}; using the generic profile"),
        }
    }

    /// The CLI recorded for `agent_id`, or the `generic` family's when the
    /// agent has none.
    #[must_use]
    pub fn get(&self, agent_id: &str) -> &AgentCli {
        self.agents
            .get(agent_id)
            .map_or(&self.generic, |(_, resolved)| resolved)
    }
}

/// Locates `command` the way a shell would: a path containing `/` is taken
/// as-is when it exists, anything else is searched on `path`.
fn locate(command: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    if command.contains('/') {
        let candidate = Path::new(command);
        return candidate.exists().then(|| candidate.to_path_buf());
    }
    let cwd = std::env::current_dir().ok()?;
    match path {
        Some(path) => which::which_in(command, Some(path), cwd).ok(),
        None => which::which_in(command, std::env::var_os("PATH"), cwd).ok(),
    }
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

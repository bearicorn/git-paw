//! Per-worktree runtime provisioning — declared env-file copies, slot-derived
//! port allocation, and consumer lifecycle hooks.
//!
//! git-paw isolates each agent's *source* (its own worktree) but not its
//! *runtime*: sibling worktrees otherwise read the same `.env` and collide on
//! the same ports when they run dev servers, previews, or debuggers. This
//! module supplies the runtime-isolation primitives, all driven entirely by the
//! consumer's `[worktree]` config:
//!
//! 1. **Env-file copy.** Each `[worktree.env] copy` entry is copied verbatim
//!    from the repository root into the new worktree. The copy is independent
//!    (never a symlink), so an agent's edits never reach the source or a
//!    sibling.
//! 2. **Port allocation.** Each worktree holds a *runtime slot*; its port block
//!    is `base + slot * stride`, and the Nth `[worktree.ports] vars` entry
//!    receives `base + slot * stride + N`. The assignments are written into a
//!    delimited managed block in the worktree's `.env.local` — the override
//!    layer most stacks load last — so the copied `.env` is never mutated.
//! 3. **Lifecycle hooks.** The remaining shared runtime resource — a database —
//!    is irreducibly stack-specific (a Neon/PlanetScale/Turso branch, a local
//!    `CREATE DATABASE`, a container), so git-paw ships no driver. Instead
//!    `[worktree.hooks] on_create` runs the consumer's own command in the new
//!    worktree once the two primitives above are in place, and its `KEY=value`
//!    stdout is merged into the same managed block, letting a script that
//!    provisions an isolated resource hand its connection details to the agent.
//!    `[worktree.hooks] on_remove` is the mirror image, run before the worktree
//!    is torn down so the resource does not leak.
//!
//! All three are opt-in: with no sub-table configured, [`provision_worktree`] is
//! a no-op and worktree creation behaves exactly as it did before this module
//! existed. git-paw ships the *mechanism* only — no default filename, port, or
//! stack is ever assumed.
//!
//! # Hook trust model
//!
//! A hook is an arbitrary command the *consumer* wrote, run with the operator's
//! own privileges — the same trust model as `[clis.*].command`. git-paw never
//! composes a hook out of broker, network, or agent-supplied input: the command
//! comes from the repository's config file and the only values interpolated into
//! it are the worktree's own id and path. It runs with the worktree as its
//! working directory and inherits git-paw's environment with nothing extra
//! injected.
//!
//! Because a hook's stdout may carry a secret (a connection string with a
//! password is the motivating case), git-paw logs only the *keys* it merged,
//! never their values. A failing `on_create` reports its stderr — so a hook
//! SHOULD print secrets to stdout and diagnostics to stderr.
//!
//! Allocation is deterministic, not probed: git-paw does not scan the host for
//! free ports, so an assigned port can still be occupied by an unrelated
//! process. That is resolved by changing `base` or `stride`.
//!
//! The provisioning seam lives here rather than in [`crate::git`] so that
//! module stays git-only; the command flows call [`provision_worktree`] after
//! the worktree is created and before the agent CLI launches.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::command_runner::{CommandRunner, RealCommandRunner};
use crate::config::{WorktreeConfig, WorktreeEnvConfig, WorktreePortsConfig};
use crate::domain::shell_quote;
use crate::error::PawError;
use crate::session::WorktreeEntry;

/// Opening delimiter of the git-paw-managed block in a worktree's `.env.local`.
pub const MANAGED_BLOCK_START: &str = "# >>> git-paw (managed) >>>";

/// Closing delimiter of the git-paw-managed block in a worktree's `.env.local`.
pub const MANAGED_BLOCK_END: &str = "# <<< git-paw (managed) <<<";

/// Name of the generated override file port assignments are written to.
pub const ENV_LOCAL_FILE: &str = ".env.local";

/// Placeholder replaced with the worktree's stable identifier in a hook command.
const WORKTREE_ID_PLACEHOLDER: &str = "{worktree_id}";

/// Placeholder replaced with the worktree's absolute path in a hook command.
const WORKTREE_PATH_PLACEHOLDER: &str = "{worktree_path}";

/// Shell a lifecycle hook's command string is executed by.
///
/// Absolute rather than `PATH`-resolved: git-paw supports macOS and Linux only,
/// where `/bin/sh` is guaranteed, and a hook must not be redirectable by a
/// mutated `PATH`.
const HOOK_SHELL: &str = "/bin/sh";

/// Returns the worktree's stable identifier — the branch-derived slug that also
/// names its directory under `.git-paw/worktrees/`.
///
/// This is what `{worktree_id}` expands to, so the consumer's hook script sees
/// the same identity the operator sees on disk.
#[must_use]
pub fn worktree_id(branch: &str) -> String {
    crate::git::branch_slug(branch)
}

/// Substitutes `{worktree_id}` and `{worktree_path}` in a raw hook command.
///
/// Substitution happens on the raw string, before the shell parses it, so a
/// placeholder may appear anywhere the consumer's command needs it (an
/// argument, part of a longer word, inside their own quoting).
fn substitute_placeholders(command: &str, id: &str, worktree_path: &Path) -> String {
    command
        .replace(WORKTREE_ID_PLACEHOLDER, id)
        .replace(WORKTREE_PATH_PLACEHOLDER, &worktree_path.to_string_lossy())
}

/// Runs one lifecycle hook and returns its captured result.
///
/// The substituted command is handed to `/bin/sh -c`, matching how a
/// `[clis.*].command` string reaches a shell when git-paw launches an agent —
/// the consumer writes one command line and the shell parses it, so git-paw
/// never needs a tokeniser of its own. [`CommandRunner`] models only
/// `(program, argv)`, so the required working directory is carried the same way
/// [`crate::git`] carries it for `git -C`: as a leading `cd <worktree> &&` in
/// the shell body, with the path shell-quoted. A hook therefore starts in the
/// worktree, with git-paw's own environment and nothing extra injected.
///
/// # Errors
///
/// Returns [`PawError::WorktreeError`] when the shell cannot be spawned. A hook
/// that runs and exits non-zero is *not* an error here — the caller decides
/// what a failure means, because the two hooks have opposite failure policies.
fn run_hook(
    runner: &dyn CommandRunner,
    command: &str,
    id: &str,
    worktree_path: &Path,
) -> Result<crate::command_runner::CommandOutput, PawError> {
    let body = format!(
        "cd {} && {}",
        shell_quote(&worktree_path.to_string_lossy()),
        substitute_placeholders(command, id, worktree_path)
    );
    runner.run(HOOK_SHELL, &["-c", &body]).map_err(|e| {
        PawError::WorktreeError(format!("failed to run [worktree.hooks] command: {e}"))
    })
}

/// Outcome of provisioning one worktree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Provisioned {
    /// Runtime slot to record on the worktree's session-state entry.
    ///
    /// `Some` only when `[worktree.ports]` is configured — a session that does
    /// not allocate ports records no slot, so its state file stays identical to
    /// one written by a prior version.
    pub runtime_slot: Option<u16>,
    /// Non-fatal warnings the caller SHOULD surface on stderr (a declared file
    /// that does not exist, an entry rejected as unsafe).
    pub warnings: Vec<String>,
    /// Names of the environment variables the `on_create` hook contributed to
    /// `.env.local`, in the order the hook printed them.
    ///
    /// Deliberately keys only: a hook's whole purpose is to surface a
    /// connection string, which routinely embeds a password, so the values
    /// never leave [`provision_worktree`] — they go straight to the file. A
    /// caller can report *that* `DATABASE_URL` was provisioned without being
    /// able to leak *what* it is.
    pub hook_env_keys: Vec<String>,
}

impl Provisioned {
    /// A one-line, value-free summary of what the `on_create` hook contributed,
    /// or `None` when no hook ran or it printed no assignments.
    ///
    /// Intended for the command handlers' progress output — see
    /// [`hook_env_keys`](Self::hook_env_keys) for why only keys appear.
    #[must_use]
    pub fn hook_summary(&self) -> Option<String> {
        (!self.hook_env_keys.is_empty()).then(|| {
            format!(
                "on_create provided {} in {ENV_LOCAL_FILE}",
                self.hook_env_keys.join(", ")
            )
        })
    }
}

/// Returns the lowest slot index not already held by a live worktree.
///
/// Slots form a free list: removing a worktree drops its entry, which releases
/// the slot for the next [`allocate_slot`] call. This keeps blocks bounded
/// (they do not climb forever across add/remove churn) while guaranteeing two
/// concurrently active worktrees never share a block.
///
/// Entries with no recorded slot (created before port allocation was
/// configured, or by a prior version) are ignored — they hold no block.
#[must_use]
pub fn allocate_slot(entries: &[WorktreeEntry]) -> u16 {
    let mut taken: Vec<u16> = entries.iter().filter_map(|e| e.runtime_slot).collect();
    taken.sort_unstable();
    let mut slot: u16 = 0;
    for used in taken {
        if used == slot {
            slot = slot.saturating_add(1);
        } else if used > slot {
            break;
        }
    }
    slot
}

/// Computes the `(var, port)` assignments for `slot` under `ports`.
///
/// The block starts at `base + slot * effective_stride` and the Nth listed var
/// receives the Nth port in the block. Returns an empty vector when `vars` is
/// empty — git-paw supplies no default port variables.
///
/// # Errors
///
/// Returns [`PawError::ConfigError`] when the computed block runs past the
/// 16-bit port space, which means `base`, `stride`, or the worktree count is
/// misconfigured. Silently wrapping would hand an agent a port from an
/// unrelated block.
pub fn port_assignments(
    ports: &WorktreePortsConfig,
    slot: u16,
) -> Result<Vec<(String, u16)>, PawError> {
    if ports.vars.is_empty() {
        return Ok(Vec::new());
    }
    let stride = u32::from(ports.effective_stride());
    let block_start = u32::from(ports.base) + u32::from(slot) * stride;
    ports
        .vars
        .iter()
        .enumerate()
        .map(|(offset, var)| {
            let offset = u32::try_from(offset).unwrap_or(u32::MAX);
            let port = block_start.saturating_add(offset);
            u16::try_from(port).map(|p| (var.clone(), p)).map_err(|_| {
                PawError::ConfigError(format!(
                    "[worktree.ports] port {port} for '{var}' (slot {slot}) exceeds the \
                         maximum port 65535 — lower `base` or `stride`"
                ))
            })
        })
        .collect()
}

/// Provisions one freshly created worktree: copies the declared env files,
/// writes the slot's port assignments into `.env.local`, and runs the
/// `on_create` hook, merging its `KEY=value` stdout into the same block.
///
/// Call this after the worktree exists on disk and before the agent's CLI
/// process starts, so the agent observes a fully provisioned checkout.
///
/// The three steps run in that order for a reason: the hook sees the copied env
/// files and the allocated ports already on disk, so a script that needs its
/// agent's port (to bind a container, say) can read them out of `.env.local`
/// before deciding what to provision.
///
/// A declared file that does not exist is skipped with a warning rather than
/// failing the create — per-branch divergence is normal, and a missing optional
/// env file should not cost the operator a session. A *hook* failure is not
/// forgiven that way: see [`PawError::WorktreeError`] below.
///
/// # Errors
///
/// Returns [`PawError::IoError`] when a file that does exist cannot be copied
/// or `.env.local` cannot be written, [`PawError::ConfigError`] when the
/// configured port block overflows the port space, and
/// [`PawError::WorktreeError`] when the `on_create` hook cannot be spawned or
/// exits non-zero — the error names its exit status and stderr, and this
/// worktree's provisioning fails rather than handing the agent a half-built
/// runtime.
pub fn provision_worktree(
    repo_root: &Path,
    worktree_path: &Path,
    branch: &str,
    config: &WorktreeConfig,
    slot: u16,
) -> Result<Provisioned, PawError> {
    provision_worktree_with(
        &RealCommandRunner,
        repo_root,
        worktree_path,
        branch,
        config,
        slot,
    )
}

/// [`provision_worktree`] against an injected runner.
///
/// # Errors
///
/// As [`provision_worktree`].
pub(crate) fn provision_worktree_with(
    runner: &dyn CommandRunner,
    repo_root: &Path,
    worktree_path: &Path,
    branch: &str,
    config: &WorktreeConfig,
    slot: u16,
) -> Result<Provisioned, PawError> {
    let mut warnings = Vec::new();

    if let Some(env) = config.env.as_ref() {
        warnings.extend(copy_env_files(repo_root, worktree_path, env)?);
    }

    let mut assignments: Vec<(String, String)> = Vec::new();
    let runtime_slot = match config.ports.as_ref() {
        Some(ports) => {
            if let Some(warning) = ports.stride_warning() {
                warnings.push(warning);
            }
            assignments = port_assignments(ports, slot)?
                .into_iter()
                .map(|(var, port)| (var, port.to_string()))
                .collect();
            write_env_local(worktree_path, &assignments)?;
            Some(slot)
        }
        None => None,
    };

    let mut hook_env_keys = Vec::new();
    if let Some(command) = config.on_create_hook() {
        let id = worktree_id(branch);
        let output = run_hook(runner, command, &id, worktree_path)?;
        if !output.success {
            return Err(PawError::WorktreeError(hook_failure(
                "on_create",
                &id,
                &output,
            )));
        }
        // The hook's pairs are appended after the ports, so a hook that
        // deliberately restates a port variable wins — it is the later, more
        // specific word about that worktree's runtime.
        let pairs = parse_env_assignments(&String::from_utf8_lossy(&output.stdout));
        hook_env_keys = pairs.iter().map(|(key, _)| key.clone()).collect();
        assignments.extend(pairs);
        write_env_local(worktree_path, &assignments)?;
    }

    Ok(Provisioned {
        runtime_slot,
        warnings,
        hook_env_keys,
    })
}

/// Describes a hook that ran and exited non-zero.
///
/// Reports the exit status and stderr — never stdout, which is the channel a
/// hook prints its (possibly secret) `KEY=value` pairs on. Returned as a plain
/// message because the two hooks route it differently: `on_create` wraps it in a
/// [`PawError`] that fails provisioning, `on_remove` emits it as a warning.
fn hook_failure(hook: &str, id: &str, output: &crate::command_runner::CommandOutput) -> String {
    let status = output.code.map_or_else(
        || "terminated by signal".to_string(),
        |c| format!("exit {c}"),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    let detail = if stderr.is_empty() {
        "no stderr output".to_string()
    } else {
        format!("stderr: {stderr}")
    };
    format!("[worktree.hooks] {hook} failed for '{id}' ({status}); {detail}")
}

/// Runs the `on_remove` hook for a worktree that is about to be torn down.
///
/// Call this before the worktree directory is deleted, so the hook can still
/// read the checkout (its `.env.local`, a compose file) while deciding what to
/// drop. Like `on_create` it runs with the worktree as its working directory and
/// with `{worktree_id}` / `{worktree_path}` substituted; unlike `on_create` its
/// stdout is not parsed — the worktree is going away, so there is no `.env.local`
/// left to merge into.
///
/// Returns the warning text the caller SHOULD surface when the hook could not be
/// spawned or exited non-zero, and `None` when it succeeded. Deliberately not a
/// `Result`: a failed teardown MUST NOT block removal — stranding a worktree on
/// disk is worse than leaking whatever resource the hook failed to drop — so
/// there is nothing here for a caller to propagate.
#[must_use]
pub fn run_remove_hook(command: &str, branch: &str, worktree_path: &Path) -> Option<String> {
    run_remove_hook_with(&RealCommandRunner, command, branch, worktree_path)
}

/// [`run_remove_hook`] against an injected runner.
pub(crate) fn run_remove_hook_with(
    runner: &dyn CommandRunner,
    command: &str,
    branch: &str,
    worktree_path: &Path,
) -> Option<String> {
    let id = worktree_id(branch);
    match run_hook(runner, command, &id, worktree_path) {
        Err(e) => Some(e.to_string()),
        Ok(output) if !output.success => Some(hook_failure("on_remove", &id, &output)),
        Ok(_) => None,
    }
}

/// Parses a hook's stdout into the `KEY=value` pairs it declared.
///
/// A line is an assignment only if, once trimmed, it starts with an identifier
/// (`[A-Za-z_][A-Za-z0-9_]*`) followed by `=`; everything else — progress
/// chatter, blank lines, a shell's own diagnostics — is ignored, so a hook is
/// free to be talkative. The value is taken verbatim to end of line, which
/// keeps a URL's own `=` (a query string, a base64 padding) intact.
fn parse_env_assignments(stdout: &str) -> Vec<(String, String)> {
    stdout
        .lines()
        .filter_map(|line| {
            let (key, value) = line.trim().split_once('=')?;
            is_env_identifier(key).then(|| (key.to_string(), value.to_string()))
        })
        .collect()
}

/// Whether `key` is a shell-style environment-variable identifier
/// (`[A-Za-z_][A-Za-z0-9_]*`).
fn is_env_identifier(key: &str) -> bool {
    let mut chars = key.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Copies each declared env file from `repo_root` into `worktree_path`,
/// returning a warning for every entry that was skipped.
///
/// The copy is a verbatim byte copy, never a symlink: a symlinked `.env` would
/// be shared mutable state across worktrees — the exact silent runtime
/// corruption worktrees exist to prevent — whereas a copy diverges cleanly and
/// any drift surfaces at merge time.
///
/// # Errors
///
/// Returns [`PawError::IoError`] when an existing source file cannot be copied
/// into the worktree.
pub fn copy_env_files(
    repo_root: &Path,
    worktree_path: &Path,
    env: &WorktreeEnvConfig,
) -> Result<Vec<String>, PawError> {
    let mut warnings = Vec::new();
    for entry in &env.copy {
        let Some(relative) = contained_relative_path(entry) else {
            warnings.push(format!(
                "[worktree.env] skipping '{entry}': only repository-relative paths that stay \
                 inside the repository are copied"
            ));
            continue;
        };
        let source = repo_root.join(&relative);
        if !source.is_file() {
            warnings.push(format!(
                "[worktree.env] declared file '{entry}' not found at the repository root — \
                 skipping"
            ));
            continue;
        }
        let destination = worktree_path.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&source, &destination)?;
    }
    Ok(warnings)
}

/// Writes `assignments` into the managed block of the worktree's `.env.local`,
/// preserving any content the user keeps outside the block.
///
/// Both sources of runtime state share this one block: the allocated ports and
/// the `on_create` hook's `KEY=value` output, in that order. Regeneration is
/// idempotent — the previous managed block is stripped before the new one is
/// appended, so provisioning the same slot twice produces byte-identical
/// output.
///
/// With no assignments and no existing `.env.local`, nothing is created — an
/// empty `vars` list and a silent hook write no lines at all.
///
/// # Errors
///
/// Returns [`PawError::IoError`] when `.env.local` cannot be read or written.
pub fn write_env_local(
    worktree_path: &Path,
    assignments: &[(String, String)],
) -> Result<(), PawError> {
    let path = worktree_path.join(ENV_LOCAL_FILE);
    let existing = match fs::read_to_string(&path) {
        Ok(contents) => Some(contents),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(PawError::IoError(e)),
    };

    if existing.is_none() && assignments.is_empty() {
        return Ok(());
    }

    let preserved = existing
        .as_deref()
        .map_or_else(String::new, strip_managed_block);

    let mut contents = preserved;
    if !assignments.is_empty() {
        if !contents.is_empty() && !contents.ends_with('\n') {
            contents.push('\n');
        }
        contents.push_str(MANAGED_BLOCK_START);
        contents.push('\n');
        for (var, value) in assignments {
            use std::fmt::Write as _;
            let _ = writeln!(contents, "{var}={value}");
        }
        contents.push_str(MANAGED_BLOCK_END);
        contents.push('\n');
    }

    fs::write(&path, contents)?;
    Ok(())
}

/// Removes the git-paw-managed block (delimiters included) from `contents`,
/// leaving every other line untouched.
///
/// An unterminated block (the closing delimiter was deleted by hand) is dropped
/// through to the end of the file — the alternative, keeping stale port lines
/// git-paw can no longer see the end of, would leave two worktrees on the same
/// port.
fn strip_managed_block(contents: &str) -> String {
    let mut out = String::with_capacity(contents.len());
    let mut inside = false;
    for line in contents.lines() {
        if line.trim() == MANAGED_BLOCK_START {
            inside = true;
            continue;
        }
        if inside {
            if line.trim() == MANAGED_BLOCK_END {
                inside = false;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Resolves a configured `copy` entry to a repository-relative path, or `None`
/// when it is empty, absolute, or escapes the repository root.
///
/// The entries are operator-supplied strings interpolated into a filesystem
/// path on both sides of the copy, so a `..` component would let a config read
/// outside the repository and write outside the worktree. Rejecting them here
/// keeps provisioning confined to the two directories it is allowed to touch.
fn contained_relative_path(entry: &str) -> Option<PathBuf> {
    let candidate = Path::new(entry);
    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!normalized.as_os_str().is_empty()).then_some(normalized)
}

#[cfg(test)]
#[path = "worktree_provision_tests.rs"]
mod tests;

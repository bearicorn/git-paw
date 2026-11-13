//! `git paw delegate` — run a delegate profile once, headless, at the
//! repository root (capability `cli-agent-profiles`).
//!
//! The shell around [`crate::delegate`]'s pure decisions: refusals, the
//! argv and environment of the one spawned process, admission against the
//! active-run records, the per-run log, the ledger line and the printed
//! result. Every process goes through an injected [`CommandRunner`], so the
//! whole command is testable with a fake spawner.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::agent_cli::CliProfile;
use crate::command_runner::{ChildState, CommandRunner, SpawnSpec};
use crate::config::{PawConfig, ProfileEntry, ProfileKind};
use crate::delegate::{
    ActiveRun, DELEGATE_ENV, LedgerLine, Outcome, RecordState, admit, classify, document, framing,
    leader_matches, truncate_result, valid_pgid,
};
use crate::error::PawError;
use crate::supervisor::escalation::HeadlessInvocation;

use super::Output;

/// Options for [`delegate`]: the `git paw delegate` arguments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DelegateOptions {
    /// The delegate profile to run.
    pub name: Option<String>,
    /// `--task`; `None` reads the task from standard input.
    pub task: Option<String>,
    /// `--list`.
    pub list: bool,
}

/// What [`delegate`] reads from its process, injected so tests can drive it.
pub struct DelegateHost<'a> {
    /// The invoking directory.
    pub cwd: &'a Path,
    /// `GIT_PAW_DELEGATE` as the invoking process sees it.
    pub delegate_marker: Option<String>,
    /// Runs `git` and the liveness probe.
    pub runner: &'a dyn CommandRunner,
    /// Spawns the delegate.
    pub spawner: &'a dyn CommandRunner,
    /// Reads the task from standard input when `--task` is absent.
    pub read_stdin: &'a mut dyn FnMut() -> std::io::Result<String>,
    /// This process's id, recorded on the active-run record.
    pub pid: u32,
    /// How often the running delegate is polled.
    pub poll_interval: Duration,
    /// Whether to kill the delegate's process group when this process gets
    /// SIGINT, SIGTERM or SIGHUP (the binary does; tests do not).
    pub guard_signals: bool,
}

/// Runs `git paw delegate`. Returns the exit status: `0` only when the
/// delegate exited `0` within its timeout.
///
/// # Errors
///
/// Every refusal — linked worktree, nested run, unknown or worker profile,
/// unavailable CLI, no headless form, a native sandbox the level would switch
/// off, an empty task, a refused admission — returns an error before
/// anything is spawned.
#[allow(clippy::too_many_lines)]
pub fn delegate(
    host: &mut DelegateHost<'_>,
    load_config: impl FnOnce(&Path) -> Result<PawConfig, PawError>,
    opts: DelegateOptions,
    out: &mut dyn Output,
) -> Result<i32, PawError> {
    if let Some(run) = &host.delegate_marker {
        return Err(PawError::SessionError(format!(
            "a delegate run cannot start another delegate ({DELEGATE_ENV}={run} is set)"
        )));
    }
    let layout = crate::cli_setup::shell::resolve_layout(host.runner, host.cwd)?;
    if layout.is_linked_worktree() {
        return Err(PawError::SessionError(format!(
            "delegates run only from the main working tree ({}), not from a linked worktree",
            layout.main_checkout.display()
        )));
    }
    let root = layout.main_checkout;
    let config = load_config(&root)?;

    if opts.list {
        list(&config, out);
        return Ok(0);
    }

    let name = opts.name.unwrap_or_default();
    let (profile_name, profile) = delegate_profile(&config, &name)?;
    let level = profile.effective_approval(crate::config::ApprovalLevel::Manual);
    let cli = CliProfile::resolve(&profile.cli, &config.clis);
    if cli.executable_in(None, &root).is_none() {
        return Err(PawError::ConfigError(format!(
            "delegate profile '{name}' uses CLI '{}', which is not available \
             (not installed or not on PATH)",
            profile.cli
        )));
    }
    let invocation = HeadlessInvocation::build(
        &profile.cli,
        &cli,
        &level,
        profile.args.as_deref(),
        Some(cli.operator_isolation()),
    )
    .map_err(|e| PawError::ConfigError(format!("delegate profile '{name}': {e}")))?;
    cli.ensure_sandbox_kept(&level)?;

    let task = match opts.task {
        Some(task) => task,
        None => (host.read_stdin)()
            .map_err(|e| PawError::SessionError(format!("cannot read the task from stdin: {e}")))?,
    };
    if task.trim().is_empty() {
        return Err(PawError::SessionError(
            "the delegate task is empty; pass --task <TEXT> or pipe the task on stdin".to_string(),
        ));
    }

    let role_skill = render_role_skill(&config, &root, profile)?;
    let stdin = document(&framing(&name, &root, level), &role_skill, &task);

    let dirs = RunDirs::new(&root);
    dirs.create()?;
    let record = ActiveRun {
        profile: name.clone(),
        level,
        pid: host.pid,
        pgid: None,
        deadline: None,
        program: None,
    };
    let record_path = admit_run(&dirs, &record, host.runner)?;
    let result = run_admitted(
        host,
        &RunInputs {
            name: &name,
            profile_name: profile_name.as_str(),
            profile,
            cli_id: &profile.cli,
            invocation: &invocation,
            session_template: cli.session_id_args("").is_some(),
            root: &root,
            stdin,
            dirs: &dirs,
            record: &record,
            record_path: &record_path,
        },
        out,
    );
    let _ = fs::remove_file(&record_path);
    result
}

/// The configured delegate profile `name`.
fn delegate_profile<'a>(
    config: &'a PawConfig,
    name: &str,
) -> Result<(&'a crate::config::ProfileName, &'a ProfileEntry), PawError> {
    let Some((key, entry)) = config.profiles.iter().find(|(k, _)| k.as_str() == name) else {
        return Err(PawError::ConfigError(format!(
            "no delegate profile '{name}' is configured ([profiles.{name}] with kind = \
             \"delegate\"); run `git paw delegate --list`"
        )));
    };
    if entry.kind == ProfileKind::Worker {
        return Err(PawError::ConfigError(format!(
            "'{name}' is a worker profile, not a delegate profile; launch it with \
             `git paw start --profile {name}` or `git paw add <branch> --profile {name}`"
        )));
    }
    Ok((key, entry))
}

/// Prints every delegate profile with its name, description, CLI, role
/// skill, effective level and whether its CLI resolves a headless form.
fn list(config: &PawConfig, out: &mut dyn Output) {
    let delegates: Vec<_> = config.delegate_profiles().collect();
    if delegates.is_empty() {
        out.line("No delegate profiles configured ([profiles.<name>] with kind = \"delegate\").");
        return;
    }
    for (name, entry) in delegates {
        let headless = CliProfile::resolve(&entry.cli, &config.clis)
            .headless_args()
            .is_some();
        out.line(&format!(
            "{name} \u{2014} {} [cli: {}, skill: {}, approval: {}]{}",
            entry.description,
            entry.cli,
            entry.skill.as_deref().unwrap_or_default(),
            entry
                .effective_approval(crate::config::ApprovalLevel::Manual)
                .kebab_name(),
            if headless {
                String::new()
            } else {
                format!(
                    " (no headless form: set [clis.{}].headless_args)",
                    entry.cli
                )
            }
        ));
    }
}

/// The profile's role skill, resolved against the repository root and
/// rendered with the orchestrator skill's gate-command and spec-backend
/// inputs (and no broker URL: a delegate has none).
fn render_role_skill(
    config: &PawConfig,
    root: &Path,
    profile: &ProfileEntry,
) -> Result<String, PawError> {
    let skill = profile.skill.as_deref().unwrap_or_default();
    let template = crate::skills::resolve(skill, root)
        .map(crate::skills::with_package_references)
        .map_err(|e| {
            PawError::ConfigError(format!("role skill '{skill}' does not resolve: {e}"))
        })?;
    let supervisor = config.supervisor.clone().unwrap_or_default();
    let backends: Vec<crate::specs::SpecBackendKind> = crate::specs::resolved_spec_kind(config)
        .into_iter()
        .collect();
    Ok(crate::skills::render_for_supervisor(
        &template,
        "",
        &crate::git::project_name(root),
        &supervisor.gate_commands(),
        &backends,
    ))
}

/// Where a run's files live.
struct RunDirs {
    dir: PathBuf,
    active: PathBuf,
}

impl RunDirs {
    fn new(root: &Path) -> Self {
        let dir = root.join(".git-paw").join("logs").join("delegates");
        Self {
            active: dir.join("active"),
            dir,
        }
    }

    fn create(&self) -> Result<(), PawError> {
        fs::create_dir_all(&self.active).map_err(|e| {
            PawError::SessionError(format!("cannot create {}: {e}", self.active.display()))
        })
    }

    fn ledger(&self) -> PathBuf {
        self.dir.join("runs.jsonl")
    }
}

/// Holds the admission lock (a `create_new` file) for the duration of one
/// admission decision.
struct AdmissionLock(PathBuf);

impl AdmissionLock {
    fn acquire(active: &Path) -> Result<Self, PawError> {
        let path = active.join(".lock");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(_) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if Instant::now() >= deadline {
                        return Err(PawError::SessionError(format!(
                            "delegate admission is locked by another `git paw delegate`; if \
                             none is running, remove the stale lock: rm {}",
                            path.display()
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => {
                    return Err(PawError::SessionError(format!(
                        "cannot take the delegate admission lock {}: {e}",
                        path.display()
                    )));
                }
            }
        }
    }
}

impl Drop for AdmissionLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Admits `record` against the live active runs under the admission lock
/// and writes its own record, returning the record's path. Records of dead
/// processes are removed as they are found.
fn admit_run(
    dirs: &RunDirs,
    record: &ActiveRun,
    runner: &dyn CommandRunner,
) -> Result<PathBuf, PawError> {
    let _lock = AdmissionLock::acquire(&dirs.active)?;
    let mut records = Vec::new();
    for entry in fs::read_dir(&dirs.active)
        .map_err(|e| PawError::SessionError(format!("cannot read active runs: {e}")))?
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Some(run) = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<ActiveRun>(&text).ok())
        {
            records.push((path, run));
        }
    }
    let alive = |target: &str| {
        runner
            .run("kill", &["-0", "--", target])
            .is_ok_and(|o| o.success)
    };
    let now = unix_now();
    let own = own_pgid(runner);
    let mut live = Vec::new();
    for (path, run) in records {
        // A recorded group is probed only when it is a real, foreign group;
        // a corrupt or planted 0, 1 or git-paw's own group counts as dead.
        let pgid = run.pgid.filter(|g| valid_pgid(*g, own) && *g != record.pid);
        let group_alive = pgid.is_some_and(|g| alive(&format!("-{g}")));
        match classify(&run, now, alive(&run.pid.to_string()), group_alive) {
            RecordState::Live => live.push(run),
            RecordState::Dead => {
                let _ = fs::remove_file(&path);
            }
            RecordState::Overdue => {
                // An orphaned delegate past its deadline: kill its group, but
                // only after confirming its leader is still the delegate
                // program — a reused group id is never signalled.
                if let (Some(g), Some(program), Some(_)) = (pgid, run.program.as_deref(), own)
                    && leader_is(runner, g, program)
                {
                    let _ = runner.run("kill", &["-KILL", "--", &format!("-{g}")]);
                }
                let _ = fs::remove_file(&path);
            }
        }
    }
    admit(&live, record.level).map_err(|reason| {
        PawError::SessionError(format!("delegate '{}' refused: {reason}", record.profile))
    })?;
    let path = dirs.active.join(format!("{}.json", record.pid));
    write_record(&path, record)?;
    Ok(path)
}

/// Writes `record` to `path` atomically (a temporary file, then a rename),
/// so a concurrent admission never reads a half-written record.
fn write_record(path: &Path, record: &ActiveRun) -> Result<(), PawError> {
    let tmp = path.with_extension("json.tmp");
    fs::write(
        &tmp,
        serde_json::to_string(record).map_err(|e| PawError::SessionError(e.to_string()))?,
    )
    .and_then(|()| fs::rename(&tmp, path))
    .map_err(|e| {
        let _ = fs::remove_file(&tmp);
        PawError::SessionError(format!("cannot record the active run: {e}"))
    })
}

/// git-paw's own process group, or `None` when `ps` cannot tell (then no
/// group is ever killed at admission).
fn own_pgid(runner: &dyn CommandRunner) -> Option<u32> {
    let out = runner
        .run(
            "ps",
            &["-o", "pgid=", "-p", &std::process::id().to_string()],
        )
        .ok()
        .filter(|o| o.success)?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// Whether group `pgid`'s leader still leads it and runs `program`.
fn leader_is(runner: &dyn CommandRunner, pgid: u32, program: &str) -> bool {
    runner
        .run("ps", &["-o", "pgid=,command=", "-p", &pgid.to_string()])
        .ok()
        .filter(|o| o.success)
        .is_some_and(|o| leader_matches(&String::from_utf8_lossy(&o.stdout), pgid, program))
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// Kills the delegate's process group `pgid` when this process receives
/// SIGINT, SIGTERM or SIGHUP, removes the run's record and exits `128 +` the
/// signal number (130, 143, 129), so an
/// interrupted `git paw delegate` (a Ctrl-C, a caller's tool timeout) never
/// leaves its delegate running unbounded.
fn guard_signals(pgid: u32, record_path: PathBuf) {
    std::thread::spawn(move || {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        let received = runtime.block_on(async {
            use tokio::signal::unix::{SignalKind, signal};
            let (Ok(mut int), Ok(mut term), Ok(mut hup)) = (
                signal(SignalKind::interrupt()),
                signal(SignalKind::terminate()),
                signal(SignalKind::hangup()),
            ) else {
                return None;
            };
            tokio::select! {
                _ = int.recv() => Some(2),
                _ = term.recv() => Some(15),
                _ = hup.recv() => Some(1),
            }
        });
        if let Some(signo) = received {
            if valid_pgid(pgid, None) {
                let _ = crate::command_runner::RealCommandRunner
                    .run("kill", &["-KILL", "--", &format!("-{pgid}")]);
            }
            let _ = fs::remove_file(&record_path);
            std::process::exit(128 + signo);
        }
    });
}

/// Everything one admitted run needs.
struct RunInputs<'a> {
    name: &'a str,
    profile_name: &'a str,
    profile: &'a ProfileEntry,
    cli_id: &'a str,
    invocation: &'a HeadlessInvocation,
    session_template: bool,
    root: &'a Path,
    stdin: String,
    dirs: &'a RunDirs,
    record: &'a ActiveRun,
    record_path: &'a Path,
}

/// Spawns the admitted run, waits for it, records it and prints its result.
fn run_admitted(
    host: &DelegateHost<'_>,
    run: &RunInputs<'_>,
    out: &mut dyn Output,
) -> Result<i32, PawError> {
    let started = SystemTime::now();
    let started_at = started
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let run_id = format!("{started_at}-{}-{}", run.profile_name, host.pid);
    let log = run.dirs.dir.join(format!("{run_id}.log"));
    let stdout_path = run.dirs.dir.join(format!("{run_id}.stdout"));
    let session_id = if run.session_template {
        Some(crate::random::generate_uuid_v4()?)
    } else {
        None
    };

    let mut env = vec![(DELEGATE_ENV.to_string(), run.name.to_string())];
    env.extend(run.invocation.isolation_env.iter().cloned());
    let spec = SpawnSpec {
        program: run.invocation.program.clone(),
        args: run.invocation.args_with_session(session_id.as_deref()),
        cwd: run.root.to_path_buf(),
        stdin: run.stdin.clone().into_bytes(),
        env,
        env_remove: vec!["GIT_PAW_BROKER_URL".to_string()],
        new_process_group: true,
        output_log: Some(log.clone()),
        stdout_log: Some(stdout_path.clone()),
        timeout: Duration::from_secs(run.profile.effective_timeout_seconds()),
    };
    let clock = Instant::now();
    let mut child = host.spawner.spawn(&spec).map_err(|e| {
        PawError::SessionError(format!("cannot start delegate '{}': {e}", run.name))
    })?;
    // The child leads its own process group: record it and the deadline, so
    // admission can reap the group even if this process dies.
    let pgid = child.id();
    if let Err(e) = write_record(
        run.record_path,
        &ActiveRun {
            pgid: Some(pgid),
            deadline: Some(started_at + run.profile.effective_timeout_seconds()),
            program: Some(run.invocation.program.clone()),
            ..run.record.clone()
        },
    ) {
        // An unrecorded group could escape its bounds: do not let it run.
        let _ = child.kill_group();
        return Err(e);
    }
    if host.guard_signals {
        guard_signals(pgid, run.record_path.to_path_buf());
    }
    let state = loop {
        match child.poll() {
            Ok(ChildState::Running) => std::thread::sleep(host.poll_interval),
            Ok(state) => break Ok(state),
            Err(e) => {
                let _ = child.kill_group();
                break Err(e);
            }
        }
    };
    let duration = clock.elapsed().as_secs_f64();

    // The run log holds stderr as it arrived, then the full stdout.
    let stdout = fs::read_to_string(&stdout_path).unwrap_or_default();
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&log) {
        let _ = write!(file, "--- stdout ---\n{stdout}");
    }
    let _ = fs::remove_file(&stdout_path);

    let outcome = match &state {
        Ok(ChildState::Exited(Some(0))) => Outcome::Ok,
        Ok(ChildState::Exited(code)) => Outcome::Failed(*code),
        Ok(ChildState::TimedOut) => Outcome::Timeout,
        Ok(ChildState::Running) | Err(_) => Outcome::Failed(None),
    };
    let line = LedgerLine::new(
        run.name,
        run.cli_id,
        session_id.as_deref(),
        started_at,
        duration,
        outcome,
    );
    if let Ok(mut ledger) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(run.dirs.ledger())
    {
        let _ = writeln!(ledger, "{}", line.to_json_line());
    }

    Ok(print_result(run, &stdout, &log, outcome, state.err(), out))
}

/// Prints the (capped) result on stdout and, for a run that did not succeed,
/// why on stderr; returns the exit status.
fn print_result(
    run: &RunInputs<'_>,
    stdout: &str,
    log: &Path,
    outcome: Outcome,
    poll_error: Option<std::io::Error>,
    out: &mut dyn Output,
) -> i32 {
    let result = truncate_result(stdout, log);
    if !result.is_empty() {
        out.line(result.strip_suffix('\n').unwrap_or(&result));
    }
    match outcome {
        Outcome::Ok => 0,
        Outcome::Timeout => {
            out.warn(&format!(
                "delegate '{}' timeout: killed after {}s; log: {}",
                run.name,
                run.profile.effective_timeout_seconds(),
                log.display()
            ));
            1
        }
        Outcome::Failed(code) => {
            let why = poll_error.map_or_else(
                || {
                    code.map_or_else(
                        || "killed by a signal".to_string(),
                        |c| format!("exited {c}"),
                    )
                },
                |e| format!("poll failed: {e}"),
            );
            out.warn(&format!(
                "delegate '{}' failed: {why}; log: {}",
                run.name,
                log.display()
            ));
            1
        }
    }
}

#[cfg(test)]
#[path = "delegate_tests.rs"]
mod tests;

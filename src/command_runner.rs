//! Process-execution seam (ports & adapters) for external tools.
//!
//! git-paw orchestrates `tmux` and `git` by shelling out. Calling
//! [`std::process::Command`] inline welds that orchestration logic to a live
//! process, so it can only be checked end-to-end. [`CommandRunner`] is the
//! injectable seam: production wires [`RealCommandRunner`] (behaviour-identical
//! to the previous inline calls), while tests inject a fake that records the
//! argv and returns scripted output — so a handler's exact `tmux`/`git` argv
//! and its reaction to success vs failure can be asserted without spawning a
//! real process.
//!
//! Introduced behaviour-preserving in `code-analysis-refactor` R1 and wired at
//! the already-covered builder execution site first ([`crate::tmux`]'s
//! `TmuxCommand::execute`); R3 routed the blind tmux runtime surface
//! ([`crate::tmux`]'s session/readiness/layout operations) through it too.
//!
//! # Two stdio dispositions
//!
//! git-paw runs external tools two ways, and the difference is observable, so
//! the seam models both rather than collapsing them:
//!
//! - [`CommandRunner::run`] **captures** stdout/stderr for git-paw to inspect
//!   (`Command::…output()`). Use it whenever the output is parsed, matched
//!   against, or deliberately discarded — a captured-then-dropped stream and a
//!   `Stdio::null()` one are indistinguishable to the user.
//! - [`CommandRunner::run_inheriting_stdio`] lets the child **inherit** git-paw's
//!   terminal and reports only the exit status (`Command::…status()`). Use it
//!   when the child must own the terminal (`tmux attach-session` replaces this
//!   process's stdio) or when its diagnostics are meant to reach the user's
//!   stderr. Capturing such a call would silently swallow output — or, for
//!   `attach`, break it outright.
//!
//! # A third disposition: non-blocking spawn
//!
//! [`CommandRunner::spawn`] starts a process and returns at once with a
//! [`SpawnedChild`] handle the caller polls. It takes a full [`SpawnSpec`] —
//! working directory, bytes for stdin (written, then closed), environment
//! additions, its own process group, stdout/stderr appended to a log file and
//! a timeout after which the whole group is killed. It exists for the drive
//! loop's headless orchestrator one-shot (`orchestrator-escalation-handoff`),
//! which must never block the wave and must not leak its context onto argv.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// The subset of a finished process's result that git-paw inspects.
///
/// Deliberately not [`std::process::Output`]: the exit status is captured as a
/// portable `success` flag plus an optional `code`, so a fake runner can build
/// a result in a test without a platform [`std::process::ExitStatus`] (which is
/// not constructible in portable code).
#[derive(Debug, Clone)]
pub struct CommandOutput {
    /// Whether the process exited successfully (status code `0`).
    pub success: bool,
    /// The process's exit code, or `None` if it was terminated by a signal.
    pub code: Option<i32>,
    /// Captured standard output.
    pub stdout: Vec<u8>,
    /// Captured standard error.
    pub stderr: Vec<u8>,
}

/// The exit status of a process whose stdio git-paw did not capture.
///
/// The counterpart to [`CommandOutput`] for
/// [`CommandRunner::run_inheriting_stdio`]: there are no captured streams to
/// report because the child wrote straight to git-paw's terminal. Carries the
/// same portable `success`/`code` pair so a fake can build one without a
/// platform [`std::process::ExitStatus`].
#[derive(Debug, Clone, Copy)]
pub struct CommandStatus {
    /// Whether the process exited successfully (status code `0`).
    pub success: bool,
    /// The process's exit code, or `None` if it was terminated by a signal.
    pub code: Option<i32>,
}

/// Everything a non-blocking [`CommandRunner::spawn`] needs to start a
/// process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSpec {
    /// The program to run (resolved on `PATH` like [`Command::new`]).
    pub program: String,
    /// Its arguments.
    pub args: Vec<String>,
    /// The working directory.
    pub cwd: PathBuf,
    /// Bytes written to the child's stdin, which is then closed.
    pub stdin: Vec<u8>,
    /// Environment variables added to the inherited environment.
    pub env: Vec<(String, String)>,
    /// Whether the child leads a new process group (Unix), so
    /// [`SpawnedChild::kill_group`] also reaches the processes it starts.
    pub new_process_group: bool,
    /// File the child's stdout and stderr are appended to (created when
    /// absent; its parent directory must exist). `None` discards both.
    pub output_log: Option<PathBuf>,
    /// How long the child may run before [`SpawnedChild::poll`] kills its
    /// group and reports [`ChildState::TimedOut`].
    pub timeout: Duration,
}

/// What [`SpawnedChild::poll`] observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildState {
    /// Still running, within its timeout.
    Running,
    /// Exited with this code, or `None` when terminated by a signal.
    Exited(Option<i32>),
    /// Ran past its timeout; its process group was killed.
    TimedOut,
}

/// A handle on a process started by [`CommandRunner::spawn`].
pub trait SpawnedChild {
    /// Reports the child's state without blocking. Once the spec's timeout has
    /// elapsed with the child still running, kills its whole group, reaps it
    /// and returns [`ChildState::TimedOut`] — on that and every later poll.
    ///
    /// # Errors
    /// Returns the [`std::io::Error`] from querying or killing the process.
    fn poll(&mut self) -> std::io::Result<ChildState>;

    /// Kills the child's whole process group (just the child when it does not
    /// lead its own group), then reaps it.
    ///
    /// # Errors
    /// Returns the [`std::io::Error`] from killing or reaping the process.
    fn kill_group(&mut self) -> std::io::Result<()>;

    /// The OS process id, for diagnostics.
    fn id(&self) -> u32;
}

/// Seam over external-process execution.
///
/// Implementors run `program` with `args` and report the result — captured via
/// [`run`](Self::run), or status-only with git-paw's stdio inherited via
/// [`run_inheriting_stdio`](Self::run_inheriting_stdio) (see the module docs on
/// choosing between them). [`RealCommandRunner`] is the production adapter; test
/// code injects a recording fake to assert argv and script success/failure.
pub trait CommandRunner {
    /// Run `program args...`, capturing stdout/stderr and the exit status — the
    /// trait equivalent of `Command::new(program).args(args).output()`.
    ///
    /// # Errors
    /// Returns the [`std::io::Error`] from spawning when the process cannot be
    /// launched (for example, `program` is not found on `PATH`).
    fn run(&self, program: &str, args: &[&str]) -> std::io::Result<CommandOutput>;

    /// Run `program args...` with git-paw's own stdio **inherited** by the
    /// child, reporting only its exit status — the trait equivalent of
    /// `Command::new(program).args(args).status()`.
    ///
    /// Nothing is captured: the child writes directly to git-paw's terminal.
    /// Required where that is the point (`tmux attach-session` takes over this
    /// process's stdio) or where the child's diagnostics belong on the user's
    /// stderr.
    ///
    /// # Errors
    /// Returns the [`std::io::Error`] from spawning when the process cannot be
    /// launched (for example, `program` is not found on `PATH`).
    fn run_inheriting_stdio(&self, program: &str, args: &[&str]) -> std::io::Result<CommandStatus>;

    /// Starts the process `spec` describes and returns immediately with a
    /// handle to poll — never waiting for it (see the module docs).
    ///
    /// The default refuses with [`std::io::ErrorKind::Unsupported`], so a
    /// runner that only ever runs blocking commands need not implement it.
    ///
    /// # Errors
    /// Returns the [`std::io::Error`] from opening the log or spawning (for
    /// example, `program` is not found on `PATH`).
    fn spawn(&self, spec: &SpawnSpec) -> std::io::Result<Box<dyn SpawnedChild>> {
        let _ = spec;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "this command runner does not support non-blocking spawn",
        ))
    }
}

/// A borrowed runner runs like the runner it borrows, so a caller can lend a
/// recording fake to an owner (the terminal backend) and still read it back.
impl<R: CommandRunner + ?Sized> CommandRunner for &R {
    fn run(&self, program: &str, args: &[&str]) -> std::io::Result<CommandOutput> {
        (**self).run(program, args)
    }

    fn run_inheriting_stdio(&self, program: &str, args: &[&str]) -> std::io::Result<CommandStatus> {
        (**self).run_inheriting_stdio(program, args)
    }

    fn spawn(&self, spec: &SpawnSpec) -> std::io::Result<Box<dyn SpawnedChild>> {
        (**self).spawn(spec)
    }
}

/// Production [`CommandRunner`] that spawns real processes.
///
/// Behaviour-identical to the previous inline calls: `run` matches
/// `Command::new(program).args(args).output()` and `run_inheriting_stdio`
/// matches `Command::new(program).args(args).status()` — same program
/// resolution, same stdio disposition, same exit status.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealCommandRunner;

impl CommandRunner for RealCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> std::io::Result<CommandOutput> {
        let output = Command::new(program).args(args).output()?;
        Ok(CommandOutput {
            success: output.status.success(),
            code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    fn run_inheriting_stdio(&self, program: &str, args: &[&str]) -> std::io::Result<CommandStatus> {
        let status = Command::new(program).args(args).status()?;
        Ok(CommandStatus {
            success: status.success(),
            code: status.code(),
        })
    }

    fn spawn(&self, spec: &SpawnSpec) -> std::io::Result<Box<dyn SpawnedChild>> {
        let mut cmd = Command::new(&spec.program);
        cmd.args(&spec.args)
            .current_dir(&spec.cwd)
            .envs(spec.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdin(Stdio::piped());
        if let Some(log) = &spec.output_log {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log)?;
            cmd.stderr(file.try_clone()?).stdout(file);
        } else {
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
        }
        #[cfg(unix)]
        if spec.new_process_group {
            use std::os::unix::process::CommandExt as _;
            cmd.process_group(0);
        }
        let mut child = cmd.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            // Written on a helper thread so a child that never reads its stdin
            // cannot block the caller; dropping the handle closes the pipe.
            let bytes = spec.stdin.clone();
            std::thread::spawn(move || {
                let _ = stdin.write_all(&bytes);
            });
        }
        Ok(Box::new(RealSpawnedChild {
            child,
            started: Instant::now(),
            timeout: spec.timeout,
            own_group: spec.new_process_group,
            timed_out: false,
        }))
    }
}

/// [`RealCommandRunner`]'s [`SpawnedChild`]: a live [`Child`] plus the clock
/// its timeout runs on.
struct RealSpawnedChild {
    child: Child,
    started: Instant,
    timeout: Duration,
    own_group: bool,
    timed_out: bool,
}

impl SpawnedChild for RealSpawnedChild {
    fn poll(&mut self) -> std::io::Result<ChildState> {
        if self.timed_out {
            return Ok(ChildState::TimedOut);
        }
        if let Some(status) = self.child.try_wait()? {
            return Ok(ChildState::Exited(status.code()));
        }
        if self.started.elapsed() >= self.timeout {
            self.kill_group()?;
            self.timed_out = true;
            return Ok(ChildState::TimedOut);
        }
        Ok(ChildState::Running)
    }

    fn kill_group(&mut self) -> std::io::Result<()> {
        // `process_group(0)` makes the child its group's leader, so its pid is
        // the pgid. `kill` the utility keeps the approved dependency set free
        // of `libc`/`nix`.
        let group_killed = self.own_group
            && Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", self.child.id())])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
        if !group_killed {
            // Already exited is not an error worth surfacing: reap below.
            let _ = self.child.kill();
        }
        self.child.wait()?;
        Ok(())
    }

    fn id(&self) -> u32 {
        self.child.id()
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    //! Crate-internal test double for [`CommandRunner`]. `#[cfg(test)]` so it is
    //! compiled only for the crate's own unit tests (the argv-assertion sites in
    //! `tmux`/`commands`), never shipped.

    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use super::{ChildState, CommandOutput, CommandRunner, CommandStatus, SpawnSpec, SpawnedChild};

    /// The scripted `(program, args) -> result` behaviour of a [`FakeCommandRunner`].
    type ScriptFn = dyn Fn(&str, &[&str]) -> std::io::Result<CommandOutput>;

    /// The scripted spawn behaviour of a [`FakeCommandRunner`].
    type SpawnFn = dyn Fn(&SpawnSpec) -> std::io::Result<Box<dyn SpawnedChild>>;

    /// Scripted [`SpawnedChild`]: `poll` reports the shared `state`, and
    /// `kill_group` only records that it was called.
    pub(crate) struct FakeChild {
        state: Rc<Cell<ChildState>>,
        killed: Rc<Cell<bool>>,
    }

    impl FakeChild {
        /// A child whose state and kill flag the test holds and drives.
        pub(crate) fn new(state: Rc<Cell<ChildState>>, killed: Rc<Cell<bool>>) -> Self {
            Self { state, killed }
        }

        /// A child that has already exited with `code`.
        pub(crate) fn exited(code: i32) -> Self {
            Self::new(
                Rc::new(Cell::new(ChildState::Exited(Some(code)))),
                Rc::new(Cell::new(false)),
            )
        }
    }

    impl SpawnedChild for FakeChild {
        fn poll(&mut self) -> std::io::Result<ChildState> {
            Ok(self.state.get())
        }

        fn kill_group(&mut self) -> std::io::Result<()> {
            self.killed.set(true);
            Ok(())
        }

        fn id(&self) -> u32 {
            0
        }
    }

    /// Recording [`CommandRunner`] test double: captures every `(program, argv)`
    /// invocation and returns a scripted [`CommandOutput`], so a caller's exact
    /// argv and its success/failure handling can be asserted without spawning a
    /// real process.
    ///
    /// Both stdio dispositions are recorded into the same [`calls`](Self::calls)
    /// log so call *order* across a mixed sequence stays assertable. The single
    /// script drives both: a `run_inheriting_stdio` call takes the scripted
    /// result's `success`/`code` and drops its streams, mirroring the real
    /// runner, which captures nothing on that path.
    pub(crate) struct FakeCommandRunner {
        script: Box<ScriptFn>,
        calls: RefCell<Vec<(String, Vec<String>)>>,
        spawn_script: Box<SpawnFn>,
        spawns: RefCell<Vec<SpawnSpec>>,
    }

    impl FakeCommandRunner {
        /// A fake that returns a successful result with `stdout` for every call.
        pub(crate) fn succeeding(stdout: &str) -> Self {
            let stdout = stdout.to_owned();
            Self::scripted(move |_, _| {
                Ok(CommandOutput {
                    success: true,
                    code: Some(0),
                    stdout: stdout.clone().into_bytes(),
                    stderr: Vec::new(),
                })
            })
        }

        /// A fake that returns a failing result (exit `1`) with `stderr` for
        /// every call.
        pub(crate) fn failing(stderr: &str) -> Self {
            let stderr = stderr.to_owned();
            Self::scripted(move |_, _| {
                Ok(CommandOutput {
                    success: false,
                    code: Some(1),
                    stdout: Vec::new(),
                    stderr: stderr.clone().into_bytes(),
                })
            })
        }

        /// A fake driven by an arbitrary `(program, args) -> result` script.
        pub(crate) fn scripted(
            f: impl Fn(&str, &[&str]) -> std::io::Result<CommandOutput> + 'static,
        ) -> Self {
            Self {
                script: Box::new(f),
                calls: RefCell::new(Vec::new()),
                spawn_script: Box::new(|_| Ok(Box::new(FakeChild::exited(0)))),
                spawns: RefCell::new(Vec::new()),
            }
        }

        /// Replaces the spawn behaviour (by default every spawn returns a
        /// child that has already exited `0`).
        pub(crate) fn with_spawn(
            mut self,
            f: impl Fn(&SpawnSpec) -> std::io::Result<Box<dyn SpawnedChild>> + 'static,
        ) -> Self {
            self.spawn_script = Box::new(f);
            self
        }

        /// The recorded `(program, argv)` invocations in call order.
        pub(crate) fn calls(&self) -> Vec<(String, Vec<String>)> {
            self.calls.borrow().clone()
        }

        /// Every recorded [`SpawnSpec`], in spawn order.
        pub(crate) fn spawns(&self) -> Vec<SpawnSpec> {
            self.spawns.borrow().clone()
        }
    }

    impl CommandRunner for FakeCommandRunner {
        fn run(&self, program: &str, args: &[&str]) -> std::io::Result<CommandOutput> {
            self.calls.borrow_mut().push((
                program.to_owned(),
                args.iter().map(|s| (*s).to_owned()).collect(),
            ));
            (self.script)(program, args)
        }

        fn run_inheriting_stdio(
            &self,
            program: &str,
            args: &[&str],
        ) -> std::io::Result<CommandStatus> {
            let output = self.run(program, args)?;
            Ok(CommandStatus {
                success: output.success,
                code: output.code,
            })
        }

        fn spawn(&self, spec: &SpawnSpec) -> std::io::Result<Box<dyn SpawnedChild>> {
            self.spawns.borrow_mut().push(spec.clone());
            (self.spawn_script)(spec)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::test_support::{FakeChild, FakeCommandRunner};
    use super::{ChildState, CommandRunner, RealCommandRunner, SpawnSpec, SpawnedChild};

    /// A spec running `sh -c script` in `dir`, logging to `dir/out.log`.
    fn sh_spec(dir: &std::path::Path, script: &str, timeout: Duration) -> SpawnSpec {
        SpawnSpec {
            program: "sh".to_string(),
            args: vec!["-c".to_string(), script.to_string()],
            cwd: dir.to_path_buf(),
            stdin: Vec::new(),
            env: Vec::new(),
            new_process_group: true,
            output_log: Some(dir.join("out.log")),
            timeout,
        }
    }

    /// Polls `child` until it leaves `Running`, bounded to ~10 s.
    fn poll_until_done(child: &mut Box<dyn SpawnedChild>) -> ChildState {
        for _ in 0..200 {
            let state = child.poll().expect("poll");
            if state != ChildState::Running {
                return state;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("child still running after ~10s");
    }

    fn log(dir: &std::path::Path) -> String {
        std::fs::read_to_string(dir.join("out.log")).unwrap_or_default()
    }

    #[test]
    fn real_spawn_runs_in_the_given_working_directory() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = RealCommandRunner
            .spawn(&sh_spec(dir.path(), "pwd", Duration::from_secs(10)))
            .unwrap();
        assert_eq!(poll_until_done(&mut child), ChildState::Exited(Some(0)));
        let expected = dir.path().canonicalize().unwrap();
        let got = std::path::PathBuf::from(log(dir.path()).trim())
            .canonicalize()
            .unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn real_spawn_delivers_stdin_then_closes_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut spec = sh_spec(dir.path(), "cat", Duration::from_secs(10));
        spec.stdin = b"context on stdin\n".to_vec();
        let mut child = RealCommandRunner.spawn(&spec).unwrap();
        // `cat` exits only once stdin is closed.
        assert_eq!(poll_until_done(&mut child), ChildState::Exited(Some(0)));
        assert_eq!(log(dir.path()), "context on stdin\n");
    }

    #[test]
    fn real_spawn_adds_environment_variables() {
        let dir = tempfile::tempdir().unwrap();
        let mut spec = sh_spec(
            dir.path(),
            "echo \"$GIT_PAW_SPAWN_X\"",
            Duration::from_secs(10),
        );
        spec.env = vec![("GIT_PAW_SPAWN_X".to_string(), "visible".to_string())];
        let mut child = RealCommandRunner.spawn(&spec).unwrap();
        assert_eq!(poll_until_done(&mut child), ChildState::Exited(Some(0)));
        assert_eq!(log(dir.path()).trim(), "visible");
    }

    #[test]
    fn real_spawn_appends_to_the_log_and_reports_the_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("out.log"), "earlier\n").unwrap();
        let mut child = RealCommandRunner
            .spawn(&sh_spec(
                dir.path(),
                "echo out; echo err >&2; exit 3",
                Duration::from_secs(10),
            ))
            .unwrap();
        assert_eq!(poll_until_done(&mut child), ChildState::Exited(Some(3)));
        let text = log(dir.path());
        assert!(text.starts_with("earlier\n"), "log was truncated: {text:?}");
        assert!(text.contains("out") && text.contains("err"), "{text:?}");
    }

    #[test]
    fn real_spawn_does_not_block_the_caller() {
        let dir = tempfile::tempdir().unwrap();
        let before = std::time::Instant::now();
        let mut child = RealCommandRunner
            .spawn(&sh_spec(dir.path(), "sleep 5", Duration::from_secs(30)))
            .unwrap();
        assert!(before.elapsed() < Duration::from_secs(2));
        assert_eq!(child.poll().unwrap(), ChildState::Running);
        child.kill_group().unwrap();
    }

    #[test]
    fn real_spawn_timeout_kills_the_whole_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("grandchild.pid");
        let script = format!("sleep 30 & echo $! > '{}'; wait", pid_file.display());
        let mut child = RealCommandRunner
            .spawn(&sh_spec(dir.path(), &script, Duration::from_secs(1)))
            .unwrap();
        assert_eq!(poll_until_done(&mut child), ChildState::TimedOut);
        assert_eq!(
            child.poll().unwrap(),
            ChildState::TimedOut,
            "TimedOut is sticky"
        );

        let pid = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .to_string();
        assert!(!pid.is_empty());
        // The grandchild was killed with its group; allow its reaper a moment.
        let mut alive = true;
        for _ in 0..100 {
            alive = std::process::Command::new("kill")
                .args(["-0", &pid])
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            if !alive {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!alive, "grandchild {pid} survived the group kill");
    }

    #[test]
    fn real_spawn_surfaces_spawn_error_for_missing_binary() {
        let dir = tempfile::tempdir().unwrap();
        let mut spec = sh_spec(dir.path(), "", Duration::from_secs(1));
        spec.program = "git-paw-no-such-binary-xyz".to_string();
        assert!(RealCommandRunner.spawn(&spec).is_err());
    }

    #[test]
    fn fake_runner_records_spawn_specs_and_returns_the_scripted_child() {
        use std::cell::Cell;
        use std::rc::Rc;
        let state = Rc::new(Cell::new(ChildState::Running));
        let killed = Rc::new(Cell::new(false));
        let (s, k) = (Rc::clone(&state), Rc::clone(&killed));
        let fake = FakeCommandRunner::succeeding("")
            .with_spawn(move |_| Ok(Box::new(FakeChild::new(Rc::clone(&s), Rc::clone(&k)))));
        let dir = tempfile::tempdir().unwrap();
        let mut spec = sh_spec(dir.path(), "x", Duration::from_secs(5));
        spec.stdin = b"ctx".to_vec();
        let mut child = fake.spawn(&spec).unwrap();

        assert_eq!(fake.spawns(), vec![spec]);
        assert_eq!(child.poll().unwrap(), ChildState::Running);
        state.set(ChildState::Exited(Some(0)));
        assert_eq!(child.poll().unwrap(), ChildState::Exited(Some(0)));
        child.kill_group().unwrap();
        assert!(killed.get());
    }

    #[test]
    fn fake_runner_default_spawn_has_exited_zero() {
        let fake = FakeCommandRunner::succeeding("");
        let dir = tempfile::tempdir().unwrap();
        let mut child = fake
            .spawn(&sh_spec(dir.path(), "x", Duration::from_secs(5)))
            .unwrap();
        assert_eq!(child.poll().unwrap(), ChildState::Exited(Some(0)));
    }

    #[test]
    fn real_runner_captures_stdout_and_success() {
        let out = RealCommandRunner
            .run("echo", &["hello"])
            .expect("echo should spawn");
        assert!(out.success);
        assert_eq!(out.code, Some(0));
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim_end(), "hello");
    }

    #[test]
    fn real_runner_reports_nonzero_exit_as_unsuccessful() {
        // `false` is a standard Unix utility that exits 1.
        let out = RealCommandRunner
            .run("false", &[])
            .expect("false should spawn");
        assert!(!out.success);
    }

    #[test]
    fn real_runner_surfaces_spawn_error_for_missing_binary() {
        let err = RealCommandRunner.run("git-paw-no-such-binary-xyz", &[]);
        assert!(err.is_err(), "a missing binary must surface a spawn error");
    }

    #[test]
    fn fake_runner_records_argv_and_returns_scripted_output() {
        let fake = FakeCommandRunner::succeeding("0\n1\n");
        let out = fake.run("tmux", &["list-panes", "-t", "paw-x"]).unwrap();
        assert!(out.success);
        assert_eq!(out.stdout, b"0\n1\n");
        assert_eq!(
            fake.calls(),
            vec![(
                "tmux".to_string(),
                vec![
                    "list-panes".to_string(),
                    "-t".to_string(),
                    "paw-x".to_string()
                ]
            )]
        );
    }

    #[test]
    fn fake_runner_failing_carries_stderr() {
        let fake = FakeCommandRunner::failing("no server running");
        let out = fake.run("tmux", &["list-panes"]).unwrap();
        assert!(!out.success);
        assert_eq!(out.stderr, b"no server running");
    }

    #[test]
    fn real_runner_inheriting_stdio_reports_exit_status() {
        let ok = RealCommandRunner
            .run_inheriting_stdio("true", &[])
            .expect("true should spawn");
        assert!(ok.success);
        assert_eq!(ok.code, Some(0));

        let bad = RealCommandRunner
            .run_inheriting_stdio("false", &[])
            .expect("false should spawn");
        assert!(!bad.success);
    }

    #[test]
    fn real_runner_inheriting_stdio_surfaces_spawn_error_for_missing_binary() {
        let err = RealCommandRunner.run_inheriting_stdio("git-paw-no-such-binary-xyz", &[]);
        assert!(err.is_err(), "a missing binary must surface a spawn error");
    }

    #[test]
    fn fake_runner_records_both_dispositions_in_call_order() {
        let fake = FakeCommandRunner::succeeding("");
        fake.run("tmux", &["has-session", "-t", "paw-x"]).unwrap();
        fake.run_inheriting_stdio("tmux", &["attach-session", "-t", "paw-x"])
            .unwrap();

        let programs_and_verbs: Vec<(String, String)> = fake
            .calls()
            .into_iter()
            .map(|(program, args)| (program, args[0].clone()))
            .collect();
        assert_eq!(
            programs_and_verbs,
            vec![
                ("tmux".to_string(), "has-session".to_string()),
                ("tmux".to_string(), "attach-session".to_string()),
            ]
        );
    }

    #[test]
    fn fake_runner_inheriting_stdio_takes_scripted_exit_status() {
        let fake = FakeCommandRunner::failing("no such session");
        let status = fake
            .run_inheriting_stdio("tmux", &["attach-session", "-t", "gone"])
            .unwrap();
        assert!(!status.success);
        assert_eq!(status.code, Some(1));
    }
}

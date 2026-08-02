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
//! `TmuxCommand::execute`); the blind runtime surface (the tmux runtime path
//! and the `cmd_*` handlers) routes through it in later waves.

use std::process::Command;

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

/// Seam over external-process execution.
///
/// Implementors run `program` with `args` and return the captured
/// [`CommandOutput`]. [`RealCommandRunner`] is the production adapter; test code
/// injects a recording fake to assert argv and script success/failure.
pub trait CommandRunner {
    /// Run `program args...`, capturing stdout/stderr and the exit status — the
    /// trait equivalent of `Command::new(program).args(args).output()`.
    ///
    /// # Errors
    /// Returns the [`std::io::Error`] from spawning when the process cannot be
    /// launched (for example, `program` is not found on `PATH`).
    fn run(&self, program: &str, args: &[&str]) -> std::io::Result<CommandOutput>;
}

/// Production [`CommandRunner`] that spawns real processes.
///
/// Behaviour-identical to the previous inline
/// `Command::new(program).args(args).output()` calls: same program resolution,
/// same captured stdout/stderr, same exit status.
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
}

#[cfg(test)]
pub(crate) mod test_support {
    //! Crate-internal test double for [`CommandRunner`]. `#[cfg(test)]` so it is
    //! compiled only for the crate's own unit tests (the argv-assertion sites in
    //! `tmux`/`commands`), never shipped.

    use std::cell::RefCell;

    use super::{CommandOutput, CommandRunner};

    /// The scripted `(program, args) -> result` behaviour of a [`FakeCommandRunner`].
    type ScriptFn = dyn Fn(&str, &[&str]) -> std::io::Result<CommandOutput>;

    /// Recording [`CommandRunner`] test double: captures every `(program, argv)`
    /// invocation and returns a scripted [`CommandOutput`], so a caller's exact
    /// argv and its success/failure handling can be asserted without spawning a
    /// real process.
    pub(crate) struct FakeCommandRunner {
        script: Box<ScriptFn>,
        calls: RefCell<Vec<(String, Vec<String>)>>,
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
            }
        }

        /// The recorded `(program, argv)` invocations in call order.
        pub(crate) fn calls(&self) -> Vec<(String, Vec<String>)> {
            self.calls.borrow().clone()
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
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::FakeCommandRunner;
    use super::{CommandRunner, RealCommandRunner};

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
}

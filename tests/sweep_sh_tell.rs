//! `sweep.sh tell <agent> <message…>` hands the message to `git paw __send`
//! on stdin — never in its argument list — and passes `__send`'s exit status
//! through (`verified-agent-send`, broker-agent-helper: *sweep.sh tell
//! delivers a message to an agent with verified submission*).
//!
//! `git` is replaced on `PATH` by a stub that records the argv and stdin of a
//! `git paw __send` call and exits with a scripted status; every other git
//! call goes to the real git.

#[path = "helpers/script_cmd.rs"]
mod script_cmd;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

fn init_git_repo(dir: &Path) {
    let run = |args: &[&str]| {
        let status = StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .expect("git command");
        assert!(status.success(), "git {args:?}");
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "t@e.st"]);
    run(&["config", "user.name", "Test"]);
    fs::write(dir.join("README.md"), "x").expect("readme");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);
}

/// Copies the bundled sweep.sh and the preamble it sources into the repo.
fn install_sweep(repo: &Path) -> PathBuf {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts");
    let dst_dir = repo.join(".git-paw/scripts");
    fs::create_dir_all(&dst_dir).expect("mk scripts dir");
    fs::copy(
        assets.join("_paw_common.sh"),
        dst_dir.join("_paw_common.sh"),
    )
    .expect("copy _paw_common.sh");
    let dst = dst_dir.join("sweep.sh");
    fs::copy(assets.join("sweep.sh"), &dst).expect("copy sweep.sh");
    dst
}

/// Writes the `git` stub into `bin` and returns `bin`.
fn install_git_stub(bin: &Path) -> PathBuf {
    let real_git = which::which("git").expect("git on PATH");
    fs::create_dir_all(bin).expect("mk bin");
    let stub = bin.join("git");
    fs::write(
        &stub,
        format!(
            "#!/usr/bin/env bash\n\
             if [[ \"$1\" == paw && \"$2\" == __send ]]; then\n\
             \x20 printf '%s\\n' \"$@\" > \"$STUB_ARGV\"\n\
             \x20 cat > \"$STUB_STDIN\"\n\
             \x20 echo \"$STUB_LINE\"\n\
             \x20 exit \"$STUB_EXIT\"\n\
             fi\n\
             if [[ \"$1\" == paw ]]; then exit 1; fi\n\
             exec {} \"$@\"\n",
            real_git.display()
        ),
    )
    .expect("write stub");
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).expect("chmod stub");
    bin.to_path_buf()
}

struct Run {
    code: i32,
    stdout: String,
    argv: String,
    stdin: String,
}

fn tell(args: &[&str], stub_line: &str, stub_exit: i32) -> Run {
    tell_with_stdin(args, None, stub_line, stub_exit)
}

fn tell_with_stdin(args: &[&str], input: Option<&str>, stub_line: &str, stub_exit: i32) -> Run {
    use std::io::Write as _;
    let dir = tempfile::TempDir::new().expect("tmp");
    let repo = dir.path().join("repo");
    fs::create_dir_all(&repo).expect("mk repo");
    init_git_repo(&repo);
    let sweep = install_sweep(&repo);
    let bin = install_git_stub(&dir.path().join("bin"));
    let argv_file = dir.path().join("argv");
    let stdin_file = dir.path().join("stdin");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut child = script_cmd::bash()
        .arg(&sweep)
        .arg("tell")
        .args(args)
        .current_dir(&repo)
        .env("PATH", path)
        .env_remove("TMUX")
        .env("STUB_ARGV", &argv_file)
        .env("STUB_STDIN", &stdin_file)
        .env("STUB_LINE", stub_line)
        .env("STUB_EXIT", stub_exit.to_string())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("run sweep.sh tell");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        if let Some(text) = input {
            stdin.write_all(text.as_bytes()).expect("write stdin");
        }
    }
    let out = child.wait_with_output().expect("wait sweep.sh tell");
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        argv: fs::read_to_string(&argv_file).unwrap_or_default(),
        stdin: fs::read_to_string(&stdin_file).unwrap_or_default(),
    }
}

/// Spec scenario "The message is not in the process arguments".
#[test]
fn sweep_sh_tell_passes_the_message_on_stdin_never_in_argv() {
    let run = tell(
        &["feat-x", "please", "fix", "the-secret-marker", "gate"],
        "delivered",
        0,
    );
    assert_eq!(
        run.argv, "paw\n__send\n--agent=feat-x\n",
        "argv: {}",
        run.argv
    );
    assert!(
        !run.argv.contains("the-secret-marker"),
        "the message leaked into argv: {}",
        run.argv
    );
    assert_eq!(run.stdin, "please fix the-secret-marker gate");
}

/// Spec scenarios "A paragraph-length message is submitted" and "A message
/// that never submits is reported wedged", helper half: the result line and
/// the exit status `__send` returns pass through unchanged.
#[test]
fn sweep_sh_tell_passes_the_exit_status_through() {
    for (line, code) in [
        ("delivered", 0),
        ("wedged", 2),
        ("refused: permission prompt showing", 3),
    ] {
        let run = tell(&["feat-x", "please continue"], line, code);
        assert_eq!(run.code, code, "{line}");
        assert_eq!(run.stdout.trim(), line);
    }
}

/// A `tell` with no agent sends nothing and never reaches `__send`.
#[test]
fn sweep_sh_tell_without_an_agent_is_refused() {
    let run = tell(&[], "delivered", 0);
    assert_eq!(run.code, 3);
    assert!(run.stdout.starts_with("refused:"), "{}", run.stdout);
    assert!(run.argv.is_empty(), "__send was invoked: {}", run.argv);
}

/// Spec scenario "The message can stay out of the helper's own arguments":
/// with no message arguments, `tell` reads the message from stdin, so it is
/// in no argv at all.
#[test]
fn sweep_sh_tell_reads_the_message_from_stdin() {
    let run = tell_with_stdin(
        &["feat-x"],
        Some("the-secret-marker please"),
        "delivered",
        0,
    );
    assert_eq!(run.code, 0);
    assert_eq!(run.argv, "paw\n__send\n--agent=feat-x\n");
    assert_eq!(run.stdin, "the-secret-marker please");
}

/// Spec scenario "A missing or failing __send is an error, not wedged": an
/// older git-paw rejects the unknown subcommand with exit 2 and no result
/// line, and any other failure exits 1; both map to exit 4.
#[test]
fn sweep_sh_tell_maps_a_failing_send_to_exit_4() {
    for (line, code) in [
        ("", 2),
        ("error: unrecognized subcommand", 2),
        ("", 1),
        ("delivered", 2),
    ] {
        let run = tell(&["feat-x", "please continue"], line, code);
        assert_eq!(run.code, 4, "stub {line:?} exit {code}");
        assert!(run.stdout.is_empty(), "no result line: {}", run.stdout);
    }
}

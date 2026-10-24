//! Argv fixtures for [`TmuxBackend`] (`code-architecture`, "Launch argv is
//! unchanged").
//!
//! Each `fixtures/<plan>.json` was captured from the orchestration path BEFORE
//! any call site moved behind the port — the builders run through a recording
//! runner, the direct `Command::new("tmux")` sites transcribed from source —
//! and lists every tmux invocation in order with its stdio disposition. Each
//! test drives the same fixed plan through the backend and requires the
//! identical sequence.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::Deserialize;

use super::TmuxBackend;
use crate::command_runner::{CommandOutput, CommandRunner, CommandStatus};
use crate::session::SessionMode;
use crate::terminal::{
    GridGeometry, Key, SessionLiveness, SessionPlan, SurfacePlacement, SurfaceSpec, TerminalBackend,
};

/// One recorded invocation, in the fixture files' shape.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Call {
    argv: Vec<String>,
    stdio: String,
}

/// A runner that records each call with its stdio disposition and answers
/// the reads the fixtures were captured against.
#[derive(Default)]
struct Recorder {
    calls: RefCell<Vec<Call>>,
}

impl Recorder {
    fn answer(args: &[&str]) -> &'static str {
        let joined = args.join(" ");
        if joined.contains("#{window_width}") {
            "480"
        } else if joined.contains("#{pane_id} #{pane_current_path}") {
            "%0 /repo\n%1 /repo/.git-paw/worktrees/feat-a\n%2 /repo/.git-paw/worktrees/feat-b\n"
        } else if joined.contains("#{pane_index} #{pane_current_path}") {
            "0 /repo\n1 /repo/.git-paw/worktrees/feat-a\n2 /repo/.git-paw/worktrees/feat-b\n"
        } else if joined.contains("#{pane_index}|") {
            "0|git-paw|dashboard\n1|claude|feat/a\n"
        } else if joined.contains("#{pane_current_command}") {
            "claude\n"
        } else {
            ""
        }
    }

    fn push(&self, args: &[&str], stdio: &str) {
        let mut recorded = vec!["tmux".to_string()];
        recorded.extend(args.iter().map(|s| (*s).to_string()));
        self.calls.borrow_mut().push(Call {
            argv: recorded,
            stdio: stdio.to_string(),
        });
    }
}

impl CommandRunner for &Recorder {
    fn run(&self, program: &str, args: &[&str]) -> std::io::Result<CommandOutput> {
        assert_eq!(program, "tmux");
        self.push(args, "captured");
        Ok(CommandOutput {
            success: true,
            code: Some(0),
            stdout: Recorder::answer(args).as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }

    fn run_inheriting_stdio(&self, program: &str, args: &[&str]) -> std::io::Result<CommandStatus> {
        assert_eq!(program, "tmux");
        self.push(args, "inherited");
        Ok(CommandStatus {
            success: true,
            code: Some(0),
        })
    }
}

fn fixture(name: &str) -> Vec<Call> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/terminal/fixtures")
        .join(format!("{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Runs `drive` against a recording backend and requires `name`'s fixture.
fn assert_matches_fixture(name: &str, drive: impl FnOnce(&TmuxBackend<&Recorder>)) {
    let recorder = Recorder::default();
    drive(&TmuxBackend::with_runner(&recorder));
    let got = recorder.calls.into_inner();
    let want = fixture(name);
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        assert_eq!(g, w, "{name}: invocation {i} differs");
    }
    assert_eq!(got.len(), want.len(), "{name}: invocation count differs");
}

const SESSION: &str = "paw-proj";
const DASHBOARD_COMMAND: &str = "'/opt/bin/git-paw' __dashboard";
const BROKER_URL: &str = "http://127.0.0.1:9119";

fn worktree(x: &str) -> PathBuf {
    PathBuf::from(format!("/repo/.git-paw/worktrees/feat-{x}"))
}

fn agent(x: &str, cli: &str) -> SurfaceSpec {
    SurfaceSpec {
        label: format!("feat/{x}"),
        owner: worktree(x),
        launch_command: cli.to_string(),
    }
}

fn dashboard() -> SurfaceSpec {
    SurfaceSpec {
        label: "dashboard".to_string(),
        owner: PathBuf::from("/repo"),
        launch_command: DASHBOARD_COMMAND.to_string(),
    }
}

fn six_agents() -> Vec<SurfaceSpec> {
    vec![
        agent("a", "claude"),
        agent("b", "codex"),
        agent("c", "claude"),
        agent("d", "claude"),
        agent("e", "claude"),
        agent("f", "claude"),
    ]
}

fn grid(first_agent: usize, agent_count: usize) -> GridGeometry {
    GridGeometry {
        first_agent,
        agent_count,
        has_top_row: first_agent > 0,
    }
}

fn plan(mode: SessionMode, surfaces: Vec<SurfaceSpec>, broker: bool, styled: bool) -> SessionPlan {
    SessionPlan {
        name: SESSION.to_string(),
        mode,
        dashboard: broker.then(dashboard),
        surfaces,
        env: if broker {
            vec![("GIT_PAW_BROKER_URL".to_string(), BROKER_URL.to_string())]
        } else {
            Vec::new()
        },
        mouse: styled,
        show_labels: styled,
    }
}

#[test]
fn launch_argv_is_unchanged_for_a_bare_broker_session_with_logging() {
    assert_matches_fixture("launch_bare_broker", |b| {
        let plan = plan(
            SessionMode::Bare,
            vec![agent("a", "claude"), agent("b", "codex")],
            true,
            true,
        );
        b.open_session(&plan).unwrap();
        for (i, x) in ["a", "b"].iter().enumerate() {
            let log = PathBuf::from(format!("/repo/.git-paw/logs/paw-proj/feat-{x}.log"));
            b.stream_to(&b.surface_at(SESSION, 1 + i), &log).unwrap();
        }
        b.layout(SESSION, &grid(1, 2)).unwrap();
    });
}

#[test]
fn launch_argv_is_unchanged_for_a_bare_session_with_a_second_grid_row() {
    assert_matches_fixture("launch_bare_no_broker_six", |b| {
        b.open_session(&plan(SessionMode::Bare, six_agents(), false, false))
            .unwrap();
        b.layout(SESSION, &grid(0, 6)).unwrap();
    });
}

#[test]
fn launch_argv_is_unchanged_for_a_supervised_session_with_a_second_grid_row() {
    assert_matches_fixture("launch_supervised_six", |b| {
        b.open_session(&plan(SessionMode::Supervisor, six_agents(), true, true))
            .unwrap();
        b.layout(SESSION, &grid(1, 6)).unwrap();
    });
}

#[test]
fn recovery_argv_is_unchanged_for_a_supervised_session() {
    assert_matches_fixture("recover_supervised_two", |b| {
        let agents = vec![agent("a", "claude"), agent("b", "codex")];
        b.open_session(&plan(SessionMode::Supervisor, agents, true, true))
            .unwrap();
        b.layout(SESSION, &grid(1, 2)).unwrap();
    });
}

#[test]
fn add_argv_is_unchanged_within_a_row() {
    assert_matches_fixture("add_same_row", |b| {
        let placement = SurfacePlacement::AppendToGrid {
            grid: grid(1, 2),
            show_labels: true,
        };
        let added = b
            .spawn_surface(SESSION, &agent("c", "claude"), &placement)
            .unwrap();
        assert_eq!(added, b.surface_at(SESSION, 3));
        b.layout(SESSION, &grid(1, 3)).unwrap();
    });
}

#[test]
fn add_argv_is_unchanged_when_the_new_surface_starts_a_full_width_row() {
    assert_matches_fixture("add_new_row", |b| {
        let placement = SurfacePlacement::AppendToGrid {
            grid: grid(1, 5),
            show_labels: true,
        };
        b.spawn_surface(SESSION, &agent("f", "claude"), &placement)
            .unwrap();
        b.layout(SESSION, &grid(1, 6)).unwrap();
    });
}

#[test]
fn remove_argv_is_unchanged() {
    assert_matches_fixture("remove", |b| {
        let surface = b
            .surface_owned_by(SESSION, &worktree("b"))
            .unwrap()
            .expect("feat-b's surface is listed");
        b.kill(&surface).unwrap();
        b.reflow_rows(SESSION, &grid(1, 1)).unwrap();
        b.layout(SESSION, &grid(1, 1)).unwrap();
    });
}

#[test]
fn prompt_submit_argv_is_unchanged() {
    assert_matches_fixture("submit_prompt", |b| {
        let surface = b.surface_at(SESSION, 2);
        b.send_text(&surface, "do the thing").unwrap();
        b.send_key(&surface, Key::Enter).unwrap();
    });
}

#[test]
fn entry_point_submit_argv_is_unchanged() {
    assert_matches_fixture("entry_point_submit", |b| {
        let surface = b.surface_at(SESSION, 1);
        for _ in 0..3 {
            assert!(b.capture_tail(&surface).is_some());
        }
        assert_eq!(b.foreground_process(&surface).as_deref(), Some("claude"));
        b.send_text(&surface, "/opsx:apply feat-a").unwrap();
        assert!(b.capture_tail(&surface).is_some());
        b.send_key(&surface, Key::Enter).unwrap();
    });
}

#[test]
fn boot_prefill_argv_is_unchanged() {
    assert_matches_fixture("boot_prefill", |b| {
        b.prefill_text(&b.surface_at(SESSION, 1), "BOOT").unwrap();
    });
}

#[test]
fn restart_from_pause_dashboard_argv_is_unchanged() {
    assert_matches_fixture("restart_dashboard", |b| {
        let anchor = b.surface_at(SESSION, 0);
        let spawned = b
            .spawn_surface(
                SESSION,
                &dashboard(),
                &SurfacePlacement::Before(anchor.clone()),
            )
            .unwrap();
        assert_eq!(spawned, anchor);
    });
}

#[test]
fn capture_argv_is_unchanged() {
    assert_matches_fixture("capture", |b| {
        b.capture_tail(&b.surface_at(SESSION, 3));
    });
}

#[test]
fn drive_loop_listing_argv_is_unchanged_and_reports_owners_by_position() {
    assert_matches_fixture("drive_list", |b| {
        let listed = b.list_surfaces(SESSION).unwrap();
        let owners: Vec<(usize, Option<PathBuf>)> =
            listed.into_iter().map(|s| (s.position, s.owner)).collect();
        assert_eq!(
            owners,
            vec![
                (0, Some(PathBuf::from("/repo"))),
                (1, Some(worktree("a"))),
                (2, Some(worktree("b"))),
            ]
        );
    });
}

#[test]
fn held_prompt_resolution_argv_is_unchanged() {
    assert_matches_fixture("held_prompt_resolution", |b| {
        b.list_surfaces(SESSION).unwrap();
    });
}

#[test]
fn key_dispatch_argv_is_unchanged() {
    assert_matches_fixture("keys", |b| {
        let surface = b.surface_at(SESSION, 2);
        b.send_key(&surface, Key::ClearLine).unwrap();
        b.send_text(&surface, "hello").unwrap();
        b.send_key(&surface, Key::Enter).unwrap();
    });
}

#[test]
fn inventory_argv_is_unchanged() {
    assert_matches_fixture("inventory", |b| {
        b.list_surfaces(SESSION).unwrap();
        let surface = b.surface_at(SESSION, 1);
        b.label_of(&surface);
        b.capture_tail(&surface);
    });
}

#[test]
fn liveness_argv_is_unchanged() {
    assert_matches_fixture("liveness", |b| {
        let an_hour_ago = SystemTime::now() - Duration::from_secs(3600);
        assert_eq!(b.liveness(SESSION, an_hour_ago, 1), SessionLiveness::Alive);
    });
}

#[test]
fn session_lifecycle_argv_is_unchanged() {
    assert_matches_fixture("session_alive_and_close", |b| {
        assert!(b.session_alive(SESSION).unwrap());
        b.close_session(SESSION).unwrap();
        b.attach(SESSION).unwrap();
        b.detach_clients(SESSION).unwrap();
    });
}

#[test]
fn a_failed_screen_read_is_no_screen_not_an_empty_one() {
    struct Failing;
    impl CommandRunner for Failing {
        fn run(&self, _: &str, _: &[&str]) -> std::io::Result<CommandOutput> {
            Ok(CommandOutput {
                success: false,
                code: Some(1),
                stdout: Vec::new(),
                stderr: b"can't find pane".to_vec(),
            })
        }
        fn run_inheriting_stdio(&self, _: &str, _: &[&str]) -> std::io::Result<CommandStatus> {
            Ok(CommandStatus {
                success: false,
                code: Some(1),
            })
        }
    }
    let b = TmuxBackend::with_runner(Failing);
    let surface = b.surface_at(SESSION, 1);
    assert_eq!(b.capture_tail(&surface), None);
    assert_eq!(b.foreground_process(&surface), None);
    assert!(b.send_text(&surface, "x").is_err());
    assert!(b.send_key(&surface, Key::Enter).is_err());
}

#[test]
fn readiness_relaunch_and_dialog_answer_argv_is_unchanged() {
    assert_matches_fixture("readiness", |b| {
        let surface = b.surface_at(SESSION, 2);
        b.relaunch_surface(&surface, "claude --dangerously-skip-permissions")
            .unwrap();
        b.answer_dialog(&surface, 1).unwrap();
    });
}

/// The one intended behaviour difference (design.md, D8): a screen that is
/// not valid UTF-8 is a failed read, not a lossily-decoded screen. The
/// pre-port entry-point wait decoded lossily; through the port such a surface
/// never reads as idle, so its entry point is reported, never typed.
#[test]
fn a_screen_that_is_not_valid_utf8_is_a_failed_read() {
    struct InvalidUtf8;
    impl CommandRunner for InvalidUtf8 {
        fn run(&self, _: &str, _: &[&str]) -> std::io::Result<CommandOutput> {
            Ok(CommandOutput {
                success: true,
                code: Some(0),
                stdout: vec![b'?', 0xff, 0xfe, b'\n'],
                stderr: Vec::new(),
            })
        }
        fn run_inheriting_stdio(&self, _: &str, _: &[&str]) -> std::io::Result<CommandStatus> {
            Ok(CommandStatus {
                success: true,
                code: Some(0),
            })
        }
    }
    let b = TmuxBackend::with_runner(InvalidUtf8);
    assert_eq!(b.capture_tail(&b.surface_at(SESSION, 1)), None);
}

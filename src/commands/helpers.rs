//! Small leaf helpers shared across the command families.

use super::Output;
use crate::config::PawConfig;
use crate::detect;
use crate::error::PawError;
use crate::interactive;
use crate::session::Session;

/// Convert the config's `[clis.*]` table into the detector's custom-CLI
/// definitions so a configured CLI is resolvable on `PATH` at launch time.
pub(crate) fn config_to_custom_defs(config: &PawConfig) -> Vec<detect::CustomCliDef> {
    config
        .clis
        .iter()
        .map(|(name, cli)| detect::CustomCliDef {
            name: name.clone(),
            command: cli.command.clone(),
            display_name: cli.display_name.clone(),
        })
        .collect()
}

/// Converts a detected CLI info into the format expected by the interactive module.
pub(crate) fn to_interactive_cli(cli: &detect::CliInfo) -> interactive::CliInfo {
    interactive::CliInfo {
        display_name: cli.display_name.clone(),
        binary_name: cli.binary_name.clone(),
    }
}

/// Index of the first coding-agent pane in a session's tmux window. See
/// [`Session::agent_pane_offset`].
pub(crate) fn agent_pane_offset(session: &Session) -> usize {
    session.agent_pane_offset()
}

/// Streams each coding-agent surface's output to its session log so the
/// session-logging capture contract (spec `session-logging` → "Attach
/// pipe-pane to capture output": *when logging is enabled and a pane is
/// created*) is honoured on every launch path — bare `start`, `--from-specs`,
/// and supervisor. A no-op when `[logging]` is disabled.
///
/// Must be called after the session is opened
/// ([`TerminalBackend::open_session`](crate::terminal::TerminalBackend::open_session)),
/// so the surfaces exist. `first_agent` is the layout position of the first
/// coding agent (see [`agent_pane_offset`]); branch `i` is the surface at
/// `first_agent + i`.
pub(crate) fn attach_session_logging(
    backend: &dyn crate::terminal::TerminalBackend,
    session_name: &str,
    config: &PawConfig,
    repo_root: &std::path::Path,
    branches: &[&str],
    first_agent: usize,
) -> Result<(), PawError> {
    if !config.logging.as_ref().is_some_and(|l| l.enabled) {
        return Ok(());
    }
    crate::logging::ensure_log_dir(repo_root, session_name)?;
    for (i, branch) in branches.iter().enumerate() {
        let log_path = crate::logging::log_file_path(repo_root, session_name, branch);
        backend.stream_to(
            &backend.surface_at(session_name, first_agent + i),
            &log_path,
        )?;
    }
    Ok(())
}

/// Opens a launch's session through the terminal port: every surface in
/// `plan`, then (when `logging` carries the config and repository root) each
/// coding-agent surface's log stream, then the agent grid's even column
/// widths — the order every launch path issued before the port existed.
///
/// A failed width rebalance only warns: the session is up and usable, just
/// with uneven columns.
///
/// # Errors
/// Returns [`PawError`] when the session cannot be opened or a log stream
/// cannot be started.
pub(crate) fn launch_session(
    backend: &dyn crate::terminal::TerminalBackend,
    plan: &crate::terminal::SessionPlan,
    layout: &crate::surface::SessionLayout,
    logging: Option<(&PawConfig, &std::path::Path)>,
    out: &mut dyn Output,
) -> Result<(), PawError> {
    backend.open_session(plan)?;
    if let Some((config, repo_root)) = logging {
        let branches: Vec<&str> = plan.surfaces.iter().map(|s| s.label.as_str()).collect();
        attach_session_logging(
            backend,
            &plan.name,
            config,
            repo_root,
            &branches,
            layout.first_agent_index(),
        )?;
    }
    // Rebalance each agent row to equal width on the live window (design D4,
    // G3): the raw `split-window -h` chain renders a row as 50/25/25, so a
    // column-precise resize evens it out now that the surfaces exist.
    if let Err(e) = backend.layout(
        &plan.name,
        &crate::terminal::GridGeometry {
            first_agent: layout.first_agent_index(),
            agent_count: plan.surfaces.len(),
            has_top_row: layout.has_top_row(),
        },
    ) {
        out.warn(&format!(
            "warning: could not rebalance agent-row widths: {e}"
        ));
    }
    Ok(())
}

/// Whether `existing`'s session `name` is alive with at least one agent CLI
/// still running, probed through the terminal port — the liveness check the
/// launch paths make before deciding to reattach, revive or recover.
pub(crate) fn session_has_live_agents(name: &str, existing: &Session) -> bool {
    use crate::terminal::TerminalBackend;
    matches!(
        crate::terminal::tmux::TmuxBackend::new().liveness(
            name,
            existing.created_at,
            existing.agent_pane_offset()
        ),
        crate::terminal::SessionLiveness::Alive
    )
}

/// The command that launches the dashboard in its own tmux pane.
///
/// `tmux send-keys` types this into the pane, and the pane's **shell** parses
/// what it receives — so the installed-binary path from
/// [`std::env::current_exe`] is shell-quoted
/// ([`shell_quote`](crate::domain::shell_quote)). An unquoted path containing
/// a space word-splits and the shell tries to run the first fragment, so the
/// dashboard never starts; `send-keys -l` cannot help, because the splitting
/// happens in the shell rather than in tmux's key parsing.
///
/// Falls back to the bare `git-paw` name (resolved on `PATH`) when the current
/// executable cannot be read.
pub(crate) fn dashboard_command() -> String {
    dashboard_command_for(
        &std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("git-paw")),
    )
}

/// [`dashboard_command`] for an explicit executable path, so the quoting
/// contract is exercisable without reinstalling the binary.
pub(crate) fn dashboard_command_for(exe: &std::path::Path) -> String {
    format!(
        "{} __dashboard",
        crate::domain::shell_quote(&exe.display().to_string())
    )
}

/// The name of the session's detached service surface that hosts the broker.
pub(crate) const BROKER_SERVICE: &str = "broker";

/// How long a launch waits for a freshly started broker host to answer
/// `GET /status` before it fails, naming the port.
pub(crate) const BROKER_HOST_READY_BUDGET: std::time::Duration = std::time::Duration::from_secs(10);

/// How long a launch waits for a broker that already answers for this
/// repository — a host stopped a moment ago — to release the port.
const PREVIOUS_HOST_RELEASE_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);

/// The command that runs the broker host for `session` in its service
/// surface, logging to `log_path`.
///
/// Every value is shell-quoted for the same reason as
/// [`dashboard_command`]: the surface's shell parses the command.
pub(crate) fn broker_host_command(session: &str, log_path: &std::path::Path) -> String {
    broker_host_command_for(
        &std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("git-paw")),
        session,
        log_path,
    )
}

/// [`broker_host_command`] for an explicit executable path.
pub(crate) fn broker_host_command_for(
    exe: &std::path::Path,
    session: &str,
    log_path: &std::path::Path,
) -> String {
    use crate::domain::shell_quote;
    format!(
        "{} __broker --session {} --log-path {}",
        shell_quote(&exe.display().to_string()),
        shell_quote(session),
        shell_quote(&log_path.display().to_string()),
    )
}

/// Starts the broker host for `session` in its detached service surface and
/// waits — bounded by [`BROKER_HOST_READY_BUDGET`] — for it to answer
/// `GET /status` as this repository's broker.
///
/// The host is handed the session name and log path on argv because a fresh
/// launch saves its receipt only after the surfaces exist.
///
/// # Errors
/// Returns an error naming the broker port when the surface cannot be
/// created, or the host exits or does not answer within the budget. The
/// caller saves no receipt then, so persisted session state is untouched.
pub(crate) fn start_broker_host(
    backend: &dyn crate::terminal::TerminalBackend,
    session: &str,
    repo_root: &std::path::Path,
    broker: &crate::config::BrokerConfig,
    log_path: &std::path::Path,
) -> Result<(), PawError> {
    // A broker for this repository already answering belongs to another
    // process — most often a host stopped a moment ago that is still
    // flushing. Give it a bounded moment to let go of the port; if it still
    // answers, starting a host beside it would leave the launcher talking to
    // a broker nothing in this session can stop.
    let client = crate::broker::client::BrokerClient::new(broker.url());
    let deadline = std::time::Instant::now() + PREVIOUS_HOST_RELEASE_BUDGET;
    while client.probe(Some(repo_root)) == crate::broker::ProbeResult::LiveBroker {
        if std::time::Instant::now() >= deadline {
            return Err(PawError::SessionError(format!(
                "a broker for this repository already serves port {} outside this session; \
                 stop it (`git paw stop`) and run `git paw start` again",
                broker.port
            )));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    let handle = backend.spawn_service(
        session,
        BROKER_SERVICE,
        &broker_host_command(session, log_path),
        repo_root,
    )?;
    wait_for_broker_host(
        &broker.url(),
        broker.port,
        repo_root,
        BROKER_HOST_READY_BUDGET,
        || backend.service_alive(&handle).unwrap_or(true),
    )
}

/// Polls `url` until it answers as `repo_root`'s broker, failing once
/// `budget` elapses or `host_alive` reports the host gone.
///
/// # Errors
/// Returns [`PawError::SessionError`] naming `port` when the broker does not
/// answer in time or its host exits first.
pub(crate) fn wait_for_broker_host(
    url: &str,
    port: u16,
    repo_root: &std::path::Path,
    budget: std::time::Duration,
    mut host_alive: impl FnMut() -> bool,
) -> Result<(), PawError> {
    let client = crate::broker::client::BrokerClient::new(url);
    let started = std::time::Instant::now();
    loop {
        if client.probe(Some(repo_root)) == crate::broker::ProbeResult::LiveBroker {
            return Ok(());
        }
        if !host_alive() {
            return Err(PawError::SessionError(format!(
                "the broker host exited before answering on port {port}; check that the port \
                 is free (`git paw doctor`) and run `git paw start` again"
            )));
        }
        if started.elapsed() >= budget {
            return Err(PawError::SessionError(format!(
                "the broker host did not answer on port {port} within {}s; check that the \
                 port is free (`git paw doctor`) and run `git paw start` again",
                budget.as_secs()
            )));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Registers every agent of a launch with the broker host through
/// `POST /watch`, carrying each agent's spec id when it was launched from a
/// spec — the path `git paw add` uses.
///
/// Best-effort per agent, like `add`: a failed registration is reported and
/// the agent still self-registers through its boot block.
pub(crate) fn register_agents_with_host(
    broker_url: &str,
    worktrees: &[crate::session::WorktreeEntry],
    out: &mut dyn Output,
) {
    let client = crate::broker::client::BrokerClient::new(broker_url);
    for wt in worktrees {
        let agent_id = crate::broker::messages::slugify_branch(&wt.branch);
        if let Err(e) = client.watch(&agent_id, &wt.worktree_path, &wt.cli, wt.spec_id.as_deref()) {
            out.warn(&format!(
                "warning: could not register '{}' with the broker: {e}",
                wt.branch
            ));
        }
    }
}

/// The exposure warning a launch command must print before it creates the
/// session, or `None` when the broker is disabled or bound to loopback.
///
/// The broker host prints the same warning, but into its detached `broker`
/// window, where nobody is looking — so that copy is effectively invisible. Every path that launches a session calls this and
/// prints the result on stderr *before* the session exists, while the
/// operator is still looking at their own terminal.
pub(crate) fn broker_exposure_warning(broker: &crate::config::BrokerConfig) -> Option<String> {
    (broker.enabled && !crate::broker::origin::is_loopback_bind(&broker.bind))
        .then(|| crate::broker::origin::non_loopback_bind_warning(&broker.bind))
}

/// Prints [`broker_exposure_warning`] on stderr when there is one.
pub(crate) fn warn_if_broker_is_exposed(
    broker: &crate::config::BrokerConfig,
    out: &mut dyn Output,
) {
    if let Some(warning) = broker_exposure_warning(broker) {
        out.warn(&warning);
    }
}

/// Error returned when add/remove is invoked on a live session an earlier
/// binary built with the whole-window tiled layout.
///
/// `add` and `remove` work on every session built with the agent grid, bare
/// and supervised alike — the grid is what the incremental re-tile builders
/// can extend. The tiled layout cannot be extended in place, and re-tiling a
/// live window into a grid would move every running pane under the user, so
/// the refusal names the layout (not the session's mode) and points at the one
/// thing that fixes it: stop the session and start it again, which rebuilds it
/// as a grid. No release version is cited — the user cannot act on one.
pub(crate) fn legacy_tiled_layout_unsupported(session_name: &str, verb: &str) -> PawError {
    PawError::SessionError(format!(
        "`git paw {verb}` cannot change session '{session_name}': its panes are the earlier tiled \
         layout, which cannot be re-tiled while the session runs. Stop the session and run \
         `git paw start` to relaunch it in the agent grid layout, then `git paw {verb}` will work."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The host command shell-quotes the binary, the session name and the log
    /// path, so a space in any of them cannot word-split in the surface's
    /// shell.
    #[test]
    fn broker_host_command_shell_quotes_every_value() {
        let cmd = broker_host_command_for(
            std::path::Path::new("/Users/My User/bin/git-paw"),
            "paw-demo",
            std::path::Path::new("/state dir/paw-demo.broker.log"),
        );
        assert_eq!(
            cmd,
            "'/Users/My User/bin/git-paw' __broker --session 'paw-demo' \
             --log-path '/state dir/paw-demo.broker.log'"
        );
    }

    /// A broker that is not answering fails the wait with the port named, and
    /// a host that exits fails it at once rather than after the budget.
    #[test]
    fn a_host_that_exits_fails_the_wait_naming_the_port_without_waiting_out_the_budget() {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let started = std::time::Instant::now();
        let err = wait_for_broker_host(
            &format!("http://127.0.0.1:{port}"),
            port,
            std::path::Path::new("/repo"),
            std::time::Duration::from_secs(10),
            || false,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains(&port.to_string()), "got {err}");
        assert!(started.elapsed() < std::time::Duration::from_secs(3));

        let err = wait_for_broker_host(
            &format!("http://127.0.0.1:{port}"),
            port,
            std::path::Path::new("/repo"),
            std::time::Duration::from_millis(300),
            || true,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains(&port.to_string()), "got {err}");
    }

    /// `broker-runtime` "A fresh launch registers its agents with the host":
    /// each agent's `POST /watch` carries its spec id when it has one.
    #[test]
    fn launch_registrations_carry_each_agents_spec_id() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut bodies = Vec::new();
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = vec![0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                bodies.push(String::from_utf8_lossy(&buf[..n]).to_string());
                let _ = stream.write_all(
                    b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
            bodies
        });

        let entry = |branch: &str, spec_id: Option<&str>| crate::session::WorktreeEntry {
            branch: branch.to_string(),
            worktree_path: std::path::PathBuf::from(format!("/repo/wt/{branch}")),
            cli: "claude".to_string(),
            branch_created: false,
            base_branch: None,
            pending_boot_prompt: None,
            pending_entry_point: None,
            runtime_slot: None,
            cli_session_id: None,
            spec_id: spec_id.map(str::to_string),
        };
        register_agents_with_host(
            &format!("http://{addr}"),
            &[
                entry("feat/x", Some("003-user-list")),
                entry("feat/y", None),
            ],
            &mut crate::commands::CaptureOutput::default(),
        );

        let bodies = server.join().unwrap();
        assert!(bodies[0].starts_with("POST /watch"), "got {}", bodies[0]);
        assert!(
            bodies[0].contains(r#""agent_id":"feat-x""#)
                && bodies[0].contains(r#""spec_id":"003-user-list""#),
            "got {}",
            bodies[0]
        );
        assert!(
            bodies[1].contains(r#""agent_id":"feat-y""#) && !bodies[1].contains("spec_id"),
            "an agent with no spec registers without one; got {}",
            bodies[1]
        );
    }

    // Spec: safe-process-invocation — "Commands sent via send-keys are sent
    // literally or shell-quoted".

    #[test]
    fn dashboard_command_shell_quotes_the_binary_path() {
        for (exe, expected) in [
            // A spaced install path is quoted so the pane's shell keeps it whole.
            (
                "/Users/My User/bin/git-paw",
                "'/Users/My User/bin/git-paw' __dashboard",
            ),
            // A plain path behaves as before: the shell strips the quotes.
            (
                "/usr/local/bin/git-paw",
                "'/usr/local/bin/git-paw' __dashboard",
            ),
            // The `current_exe` fallback still resolves on PATH when quoted.
            ("git-paw", "'git-paw' __dashboard"),
        ] {
            assert_eq!(
                dashboard_command_for(std::path::Path::new(exe)),
                expected,
                "exe: {exe}"
            );
        }
    }

    #[test]
    #[serial_test::serial]
    fn a_spaced_binary_path_launches_the_dashboard_command_in_a_pane() {
        if std::process::Command::new("tmux")
            .arg("-V")
            .output()
            .is_err()
        {
            return;
        }

        // Stand in for an installed git-paw under a path with a space. The
        // stub records the subcommand it was invoked with, which is what
        // proves the pane's shell ran the whole path rather than its first
        // word.
        let dir = tempfile::TempDir::new().unwrap();
        let bin_dir = dir.path().join("My Bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let exe = bin_dir.join("git-paw");
        let marker = dir.path().join("launched");
        std::fs::write(
            &exe,
            format!(
                "#!/bin/sh\nprintf %s \"$1\" > '{}'\n",
                marker.to_string_lossy()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let session = "paw-dashboard-send-probe";
        let target = format!("{session}:0.0");
        let _ = std::process::Command::new("tmux")
            .args(["kill-session", "-t", session])
            .output();
        let created = std::process::Command::new("tmux")
            .args(["new-session", "-d", "-s", session, "-x", "200", "-y", "50"])
            .status()
            .expect("create probe session");
        assert!(created.success());

        let sent = std::process::Command::new("tmux")
            .args([
                "send-keys",
                "-t",
                &target,
                &dashboard_command_for(&exe),
                "Enter",
            ])
            .status()
            .expect("send dashboard command");
        assert!(sent.success());

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut launched = String::new();
        while std::time::Instant::now() < deadline {
            launched = std::fs::read_to_string(&marker).unwrap_or_default();
            if !launched.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        let _ = std::process::Command::new("tmux")
            .args(["kill-session", "-t", session])
            .output();

        assert_eq!(
            launched, "__dashboard",
            "the pane's shell did not launch the spaced binary path with its subcommand"
        );
    }

    // `git-add-remove-branch` — "Hot-attach works on every grid session": the
    // only refusal left names the tiled LAYOUT, never the session's mode.
    // (Replaces the tests that pinned the removed bare-session refusal.)

    #[test]
    fn the_tiled_layout_refusal_names_the_layout_and_the_fix() {
        for (session, verb) in [("test-session", "add"), ("my-session", "remove")] {
            let msg = legacy_tiled_layout_unsupported(session, verb).to_string();

            assert!(msg.contains(session), "error must name the session: {msg}");
            assert!(msg.contains(verb), "error must name the command: {msg}");
            assert!(
                msg.contains("tiled"),
                "error must name the tiled layout as the reason: {msg}"
            );
            assert!(
                msg.contains("Stop the session") && msg.contains("git paw start"),
                "error must tell the user to stop and start the session again: {msg}"
            );
        }
    }

    #[test]
    fn the_tiled_layout_refusal_blames_the_layout_not_the_mode() {
        let msg = legacy_tiled_layout_unsupported("test-session", "add").to_string();

        // The removed requirement refused a session for being bare and sent
        // the user to supervisor mode. Both are gone: bare grid sessions are
        // hot-attachable, and there is no supervisor mode to switch to.
        assert!(
            !msg.contains("bare") && !msg.contains("supervisor"),
            "the refusal must not blame the session's mode: {msg}"
        );
        for version in ["v0.6.0", "v1.0.0", "v0.18"] {
            assert!(
                !msg.contains(version),
                "error must not cite a release version ({version}): {msg}"
            );
        }
    }

    // Gate review item 6: the broker's own warning lands in the dashboard
    // pane just before it switches to the alternate screen, so the launch
    // command must print it too — before the session is created.

    fn broker(enabled: bool, bind: &str) -> crate::config::BrokerConfig {
        crate::config::BrokerConfig {
            enabled,
            bind: bind.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn a_launch_warns_when_the_broker_is_bound_off_loopback() {
        for bind in ["0.0.0.0", "192.168.1.5", "::"] {
            let warning = broker_exposure_warning(&broker(true, bind))
                .unwrap_or_else(|| panic!("bind {bind} must warn"));
            assert!(warning.contains(bind), "the warning must name the bind");
            assert!(
                warning.contains("reachable from the network without authentication"),
                "the warning must state the exposure, got: {warning}"
            );
        }
    }

    #[test]
    fn a_launch_is_silent_on_a_loopback_bind_or_a_disabled_broker() {
        for bind in ["127.0.0.1", "localhost", "::1"] {
            assert_eq!(
                broker_exposure_warning(&broker(true, bind)),
                None,
                "bind {bind} must not warn"
            );
        }
        assert_eq!(
            broker_exposure_warning(&broker(false, "0.0.0.0")),
            None,
            "a disabled broker is never started, so it must not warn"
        );
    }

    /// Session logging is wired through the port on every launch path: after
    /// the session opens, each coding-agent surface (from the layout's first
    /// agent position) streams to its branch's log, before the grid is evened.
    #[test]
    fn launch_session_streams_each_coding_surface_to_its_log_before_the_layout() {
        use crate::session::SessionMode;
        use crate::surface::SessionLayout;
        use crate::terminal::fake::{FakeBackend, Operation};
        use crate::terminal::{SessionPlan, SurfaceSpec, TerminalBackend};

        let repo = tempfile::TempDir::new().unwrap();
        let spec = |x: &str| SurfaceSpec {
            label: format!("feat/{x}"),
            owner: repo.path().join(format!("wt-{x}")),
            launch_command: "claude".to_string(),
        };
        let plan = SessionPlan {
            name: "paw-proj".to_string(),
            mode: SessionMode::Bare,
            dashboard: Some(SurfaceSpec {
                label: "dashboard".to_string(),
                owner: repo.path().to_path_buf(),
                launch_command: "git-paw __dashboard".to_string(),
            }),
            surfaces: vec![spec("a"), spec("b")],
            env: Vec::new(),
            mouse: false,
            show_labels: false,
        };
        let mut config = PawConfig::default();
        config.logging = Some(crate::config::LoggingConfig { enabled: true });
        let layout = SessionLayout::planned(SessionMode::Bare, true);

        let fake = FakeBackend::new();
        launch_session(
            &fake,
            &plan,
            &layout,
            Some((&config, repo.path())),
            &mut crate::commands::CaptureOutput::default(),
        )
        .unwrap();

        let streamed: Vec<_> = fake
            .operations()
            .into_iter()
            .filter_map(|op| match op {
                Operation::StreamTo { surface, log } => Some((surface, log)),
                _ => None,
            })
            .collect();
        let log = |branch: &str| crate::logging::log_file_path(repo.path(), "paw-proj", branch);
        assert_eq!(
            streamed,
            vec![
                (fake.surface_at("paw-proj", 1), log("feat/a")),
                (fake.surface_at("paw-proj", 2), log("feat/b")),
            ]
        );
        assert!(
            matches!(fake.operations().last(), Some(Operation::Layout { .. })),
            "the grid is evened after the streams start"
        );
    }
}

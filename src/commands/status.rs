//! `git paw status` — shows session state for the current repo (human or
//! `--json`). Extracted verbatim from `main.rs` (code-analysis-refactor R2b).

use git_paw::broker;
use git_paw::error::PawError;
use git_paw::git;
use git_paw::session;
use git_paw::terminal::TerminalBackend;
use git_paw::terminal::tmux::TmuxBackend;
use git_paw::tmux;

use super::helpers::BROKER_SERVICE;

/// Builds the `--json` body.
///
/// `wave` is present only when the session's own broker answered (see
/// [`cmd_status`]), so the key is simply absent otherwise rather than a
/// zeroed object a script could mistake for "idle since the epoch".
fn status_json(
    existing: &session::Session,
    display: session::DisplayStatus,
    tmux_running: bool,
    wave: Option<broker::WaveStatus>,
) -> serde_json::Value {
    let worktrees: Vec<_> = existing
        .worktrees
        .iter()
        .map(|e| {
            serde_json::json!({
                "branch": e.branch,
                "cli": e.cli,
                "worktree_path": e.worktree_path,
                "cli_session_id": e.cli_session_id,
            })
        })
        .collect();
    let mut obj = serde_json::json!({
        "session": existing.session_name,
        "status": display.as_str(),
        "tmux_running": tmux_running,
        "worktrees": worktrees,
    });
    if let Some(wave) = wave
        && let Some(map) = obj.as_object_mut()
    {
        map.insert("wave".to_string(), serde_json::json!(wave));
    }
    obj
}

/// What `git paw status` knows about the session's broker: the
/// repository-scoped probe verdict, whether the session's broker host is
/// alive, and — from this repository's own broker — its `/status` answer.
struct BrokerView {
    probe: broker::ProbeResult,
    host_alive: bool,
    status: Option<broker::client::BrokerStatus>,
}

/// Formats an uptime for the `Broker:` line: `45s`, `12m`, `3h5m`.
fn uptime_human(seconds: u64) -> String {
    broker::WaveStatus {
        last_progress_unix_secs: 0,
        idle_seconds: seconds,
        stall_threshold_seconds: 0,
        stalled: false,
    }
    .idle_human()
}

/// The `Broker:` line for `url`.
///
/// The broker is "running" only when the probe finds this repository's
/// broker AND the session's broker host is alive: a process still answering
/// on the port after `git paw stop` is a leftover, not this session's broker
/// (PP-27). A broker serving a *different* repository is reported as such.
fn broker_line(url: &str, view: &BrokerView, display: session::DisplayStatus) -> String {
    match &view.probe {
        broker::ProbeResult::LiveBroker if view.host_alive => {
            let detail = view.status.as_ref().map_or_else(String::new, |st| {
                format!(
                    ", {} agents, up {}",
                    st.agents.len(),
                    uptime_human(st.uptime_seconds)
                )
            });
            format!("Broker:  {url} (running{detail})")
        }
        broker::ProbeResult::LiveBroker => format!(
            "Broker:  {url} (leftover broker answering with no live session host \u{2014} \
             not this session's broker)"
        ),
        broker::ProbeResult::OtherRepoBroker { .. } => {
            format!("Broker:  {url} (held by another repository's git-paw broker)")
        }
        _ if display == session::DisplayStatus::Paused => {
            format!("Broker:  {url} (paused \u{2014} run 'git paw start' to resume)")
        }
        _ => format!("Broker:  {url} (not responding)"),
    }
}

/// Prints the `Broker:` line for `url`, plus the stalled-wave line when the
/// session's own broker reports the wave stalled.
fn print_broker_block(
    url: &str,
    view: &BrokerView,
    display: session::DisplayStatus,
    wave: Option<broker::WaveStatus>,
) {
    println!("{}", broker_line(url, view, display));
    // A roster of `working` agents can hide a wave that has not moved in
    // days, so say so explicitly rather than leaving the reader to infer it
    // from per-agent rows.
    if let Some(wave) = wave
        && wave.stalled
    {
        println!(
            "  \u{21b3} wave stalled \u{2014} no progress for {}",
            wave.idle_human()
        );
    }
    println!();
}

/// Shows session state for the current repo.
pub(crate) fn cmd_status(json: bool) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    let Some(existing) = session::find_session_for_repo(&repo_root)? else {
        if json {
            println!("{}", serde_json::json!({ "session": null }));
        } else {
            println!("No session for this repo.");
        }
        return Ok(());
    };

    // Liveness probe (spec: "Liveness probe is cheap"). The probe
    // distinguishes a genuinely-absent tmux session (Stale) from a probe
    // that could not run at all (Indeterminate → never reports stale), and
    // additionally requires a live agent-pane CLI process once past the
    // launch grace period (GP-03c) — a session whose panes all reverted to a
    // bare shell reports AgentsExited, so it displays as stopped rather than
    // healthy even though the tmux session object still exists. Its tmux
    // session IS still running, so it counts as running below, and only an
    // absent tmux session (Stale) yields the stale display.
    let liveness = tmux::session_liveness_for(
        &existing.session_name,
        existing.created_at,
        existing.agent_pane_offset(),
    );
    let display = session::DisplayStatus::from_receipt(&existing.status, liveness);
    let tmux_running = matches!(
        liveness,
        tmux::SessionLiveness::Alive | tmux::SessionLiveness::AgentsExited
    );

    // Probe the recorded broker port ONCE, with this repository's identity,
    // and reuse the verdict for both the wave read and the human `Broker:`
    // line below.
    //
    // The identity check is not optional here: the default port is shared
    // across repositories, so a neighbouring repository's live broker can be
    // holding it. Reading its wave would attribute *its* progress to *this*
    // session — printing a stalled-wave line, or hiding one, on the strength
    // of an unrelated repository's agents. Probing once also keeps
    // `--json` from paying a second round-trip it never paid before.
    let broker_endpoint = existing
        .broker_bind
        .as_ref()
        .zip(existing.broker_port)
        .map(|(bind, port)| format!("http://{bind}:{port}"));
    let broker_probe = broker_endpoint
        .as_deref()
        .map(|url| broker::probe_broker_for_repo(url, &existing.repo_path));

    // `/status` (agents, uptime, wave) is only read from a broker that has
    // confirmed it serves THIS repository.
    let broker_status = match (&broker_endpoint, &broker_probe) {
        (Some(url), Some(broker::ProbeResult::LiveBroker)) => {
            broker::client::BrokerClient::new(url.as_str())
                .status()
                .ok()
        }
        _ => None,
    };

    // Wave staleness: whether *anything* in the session has progressed
    // recently. A broker that predates the field yields `None`, and nothing
    // extra is shown.
    let wave = broker_status.as_ref().and_then(|st| st.wave);

    if json {
        println!("{}", status_json(&existing, display, tmux_running, wave));
        return Ok(());
    }

    println!("Session: {}", existing.session_name);
    println!("Status:  {} {display}", display.icon());
    match display {
        session::DisplayStatus::Paused => {
            println!("  \u{21b3} run 'git paw start' to resume");
        }
        session::DisplayStatus::Stale => {
            println!(
                "  \u{21b3} tmux session no longer exists — run 'git paw start' to \
                 self-heal, or 'git paw purge --stale' to clear it"
            );
        }
        _ => {}
    }
    if matches!(liveness, tmux::SessionLiveness::AgentsExited) {
        println!("  \u{21b3} agent panes exited — run 'git paw start' to recover them");
    }
    println!(
        "Tmux:    {}",
        if tmux_running {
            "running"
        } else {
            "not running"
        }
    );
    println!();

    if let (Some(url), Some(probe)) = (&broker_endpoint, broker_probe) {
        // The session's broker host is alive when its `broker` service
        // surface is; a session started before the host split has no such
        // surface, so its terminal session's liveness stands in. Either way a
        // dead terminal session means no live host.
        let backend = TmuxBackend::new();
        let host_alive = backend
            .service_alive(&backend.service(&existing.session_name, BROKER_SERVICE))
            .unwrap_or(false)
            || tmux_running;
        let view = BrokerView {
            probe,
            host_alive,
            status: broker_status,
        };
        print_broker_block(url, &view, display, wave);
    }

    if existing.worktrees.is_empty() {
        println!("  (no worktrees)");
    } else {
        for entry in &existing.worktrees {
            println!(
                "  {} \u{2192} {} ({})",
                entry.branch,
                entry.cli,
                entry.worktree_path.display()
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "http://127.0.0.1:9119";

    fn status_with(agents: usize, uptime_seconds: u64) -> broker::client::BrokerStatus {
        broker::client::BrokerStatus {
            git_paw: true,
            uptime_seconds,
            repo_root: "/repo".to_string(),
            agents: (0..agents)
                .map(|i| broker::client::StatusAgentRow {
                    agent_id: format!("feat-{i}"),
                    cli: "claude".to_string(),
                    status: "working".to_string(),
                    last_seen_seconds: 0,
                    phase: None,
                    modified_files: Vec::new(),
                })
                .collect(),
            wave: None,
        }
    }

    /// "Status shows running broker with agents".
    #[test]
    fn a_live_host_with_three_agents_reports_running_and_the_agent_count() {
        let view = BrokerView {
            probe: broker::ProbeResult::LiveBroker,
            host_alive: true,
            status: Some(status_with(3, 125)),
        };
        let line = broker_line(URL, &view, session::DisplayStatus::Active);
        assert!(line.contains(URL), "got {line}");
        assert!(line.contains("running"), "got {line}");
        assert!(line.contains("3 agents"), "got {line}");
        assert!(line.contains("up 2m"), "got {line}");
    }

    /// "Status after stop does not report the broker running" (PP-27): a
    /// process still answering on the port with no live session host is a
    /// leftover, never this session's running broker.
    #[test]
    fn an_answering_broker_with_no_live_host_is_not_reported_running() {
        let view = BrokerView {
            probe: broker::ProbeResult::LiveBroker,
            host_alive: false,
            status: Some(status_with(3, 125)),
        };
        let line = broker_line(URL, &view, session::DisplayStatus::Stopped);
        assert!(!line.contains("(running"), "got {line}");
        assert!(line.contains("leftover"), "got {line}");
    }

    /// The same after stop when nothing answers at all.
    #[test]
    fn a_stopped_session_with_no_broker_reports_not_responding() {
        let view = BrokerView {
            probe: broker::ProbeResult::NoListener,
            host_alive: false,
            status: None,
        };
        let line = broker_line(URL, &view, session::DisplayStatus::Stopped);
        assert!(!line.contains("running"), "got {line}");
        assert!(line.contains("not responding"), "got {line}");
    }

    /// "Status shows paused session with broker offline".
    #[test]
    fn a_paused_session_reports_the_broker_paused_not_in_error_terms() {
        let view = BrokerView {
            probe: broker::ProbeResult::NoListener,
            host_alive: true,
            status: None,
        };
        let line = broker_line(URL, &view, session::DisplayStatus::Paused);
        assert!(line.contains("paused"), "got {line}");
        assert!(line.contains("git paw start"), "got {line}");
        assert!(!line.contains("not responding"), "got {line}");
    }

    /// "Status shows broker not responding for crashed active session".
    #[test]
    fn a_crashed_host_of_an_active_session_reports_not_responding() {
        let view = BrokerView {
            probe: broker::ProbeResult::NoListener,
            host_alive: false,
            status: None,
        };
        let line = broker_line(URL, &view, session::DisplayStatus::Active);
        assert!(line.contains("not responding"), "got {line}");
    }

    /// "Status reports a port held by another repository".
    #[test]
    fn another_repositorys_broker_is_never_reported_running() {
        let view = BrokerView {
            probe: broker::ProbeResult::OtherRepoBroker {
                repo_root: Some("/elsewhere".to_string()),
            },
            host_alive: true,
            status: None,
        };
        let line = broker_line(URL, &view, session::DisplayStatus::Active);
        assert!(line.contains("another repository"), "got {line}");
        assert!(!line.contains("running"), "got {line}");
    }
}

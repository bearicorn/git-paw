//! `git paw status` — shows session state for the current repo (human or
//! `--json`). Extracted verbatim from `main.rs` (code-analysis-refactor R2b).

use git_paw::broker;
use git_paw::error::PawError;
use git_paw::git;
use git_paw::session;
use git_paw::tmux;

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

/// Prints the `Broker:` line for `url`, plus the stalled-wave line when the
/// session's own broker reports the wave stalled.
///
/// `probe` is the single repository-scoped probe `cmd_status` already took,
/// so a broker on the port serving a *different* repository is reported as
/// such — and `wave`, which the caller only reads from a confirmed own
/// broker, is never attributed to this session by mistake.
fn print_broker_block(
    url: &str,
    probe: &broker::ProbeResult,
    display: session::DisplayStatus,
    wave: Option<broker::WaveStatus>,
) {
    match probe {
        broker::ProbeResult::LiveBroker => println!("Broker:  {url} (running)"),
        broker::ProbeResult::OtherRepoBroker { .. } => {
            println!("Broker:  {url} (held by another repository's git-paw broker)");
        }
        _ if display == session::DisplayStatus::Paused => {
            println!("Broker:  {url} (paused \u{2014} run 'git paw start' to resume)");
        }
        _ => println!("Broker:  {url} (not responding)"),
    }
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

    // Wave staleness: whether *anything* in the session has progressed
    // recently. Only read from a broker that has confirmed it serves THIS
    // repository. A broker that predates the field yields `None`, and
    // nothing extra is shown.
    let wave = match (&broker_endpoint, &broker_probe) {
        (Some(url), Some(broker::ProbeResult::LiveBroker)) => {
            broker::publish::fetch_wave_over_http(url).ok().flatten()
        }
        _ => None,
    };

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

    if let (Some(url), Some(probe)) = (&broker_endpoint, &broker_probe) {
        print_broker_block(url, probe, display, wave);
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

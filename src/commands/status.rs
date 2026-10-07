//! `git paw status` — shows session state for the current repo (human or
//! `--json`). Extracted verbatim from `main.rs` (code-analysis-refactor R2b).

use git_paw::broker;
use git_paw::error::PawError;
use git_paw::git;
use git_paw::session;
use git_paw::tmux;

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

    if json {
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
        let obj = serde_json::json!({
            "session": existing.session_name,
            "status": display.as_str(),
            "tmux_running": tmux_running,
            "worktrees": worktrees,
        });
        println!("{obj}");
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

    // Broker info
    if let (Some(bind), Some(port)) = (&existing.broker_bind, existing.broker_port) {
        let url = format!("http://{bind}:{port}");
        // Probe with this repository, so a broker on the port that serves a
        // *different* repository is not reported as this session's broker
        // running.
        match broker::probe_broker_for_repo(&url, &existing.repo_path) {
            broker::ProbeResult::LiveBroker => println!("Broker:  {url} (running)"),
            broker::ProbeResult::OtherRepoBroker { .. } => {
                println!("Broker:  {url} (held by another repository's git-paw broker)");
            }
            _ if display == session::DisplayStatus::Paused => {
                println!("Broker:  {url} (paused \u{2014} run 'git paw start' to resume)");
            }
            _ => println!("Broker:  {url} (not responding)"),
        }
        println!();
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

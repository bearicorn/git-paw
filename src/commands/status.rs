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

    // Single cheap liveness probe (spec: "Liveness probe is cheap"). The
    // probe distinguishes a genuinely-absent tmux session (Stale) from a
    // probe that could not run at all (Indeterminate → never reports stale).
    let liveness = tmux::session_liveness(&existing.session_name);
    let display = session::DisplayStatus::from_receipt(&existing.status, liveness);
    let alive = matches!(liveness, tmux::SessionLiveness::Alive);

    if json {
        let worktrees: Vec<_> = existing
            .worktrees
            .iter()
            .map(|e| {
                serde_json::json!({
                    "branch": e.branch,
                    "cli": e.cli,
                    "worktree_path": e.worktree_path,
                })
            })
            .collect();
        let obj = serde_json::json!({
            "session": existing.session_name,
            "status": display.as_str(),
            "tmux_running": alive,
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
    println!("Tmux:    {}", if alive { "running" } else { "not running" });
    println!();

    // Broker info
    if let (Some(bind), Some(port)) = (&existing.broker_bind, existing.broker_port) {
        let url = format!("http://{bind}:{port}");
        match broker::probe_broker(&url) {
            broker::ProbeResult::LiveBroker => println!("Broker:  {url} (running)"),
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

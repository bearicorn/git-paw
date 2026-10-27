//! Docs-parity guards for the `supervisor-launch` capability's orchestrator
//! documentation: the quick start that replaced `quick-start-supervisor.md`,
//! its mdBook redirect, the remote-control workflow chapter, and the
//! security-posture controls-table row that links it.
//!
//! Each test maps to one scenario of the change's `supervisor-launch` delta.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn quick_start() -> String {
    read("docs/src/quick-start-orchestrator.md")
}

fn remote_control() -> String {
    read("docs/src/user-guide/remote-control.md")
}

/// Every `pane <N>` (or `pane <N>,`/`pane <N>)`) index mentioned in `text`,
/// paired with the line it appears on.
fn pane_mentions(text: &str) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let lower = line.to_lowercase();
        let mut rest = lower.as_str();
        while let Some(at) = rest.find("pane ") {
            rest = &rest[at + "pane ".len()..];
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if let Ok(n) = digits.parse::<u32>() {
                out.push((n, line.to_string()));
            }
        }
    }
    out
}

/// Scenario: Quick start chapter is internally consistent — the dashboard is
/// pane 0, agents are pane 1 onwards, and the orchestrator runs outside tmux.
#[test]
fn quick_start_layout_is_internally_consistent() {
    let doc = quick_start();
    let mentions = pane_mentions(&doc);
    assert!(
        !mentions.is_empty(),
        "the quick start must describe the pane layout"
    );
    for (n, line) in &mentions {
        if *n == 0 {
            assert!(
                line.to_lowercase().contains("dashboard"),
                "every pane-0 reference must identify the dashboard: {line:?}"
            );
        } else {
            assert!(
                !line.to_lowercase().contains("dashboard in pane"),
                "the dashboard must only ever be pane 0: {line:?}"
            );
        }
    }
    assert!(
        doc.contains("pane 1"),
        "the quick start must place the coding agents from pane 1 onwards"
    );
    assert!(
        doc.to_lowercase().contains("outside the tmux session")
            || doc.to_lowercase().contains("outside tmux"),
        "the quick start must state that the orchestrator runs outside the tmux session"
    );
    assert!(
        !doc.to_lowercase().contains("supervisor pane"),
        "the quick start must not place a supervisor inside the session"
    );
}

/// Scenario: Old path redirects — the retired chapter is gone and the mdBook
/// config redirects its built page to the new one.
#[test]
fn old_quick_start_path_redirects() {
    assert!(
        !repo_root()
            .join("docs/src/quick-start-supervisor.md")
            .exists(),
        "quick-start-supervisor.md must be replaced, not kept beside the new chapter"
    );
    let book = read("docs/book.toml");
    let redirect = book
        .split("[output.html.redirect]")
        .nth(1)
        .expect("docs/book.toml must carry an [output.html.redirect] table");
    let entry = redirect
        .lines()
        .find(|l| l.contains("quick-start-supervisor.html"))
        .expect("the redirect table must name quick-start-supervisor.html");
    assert!(
        entry.contains("quick-start-orchestrator.html"),
        "quick-start-supervisor.html must redirect to quick-start-orchestrator.html: {entry:?}"
    );
    assert!(
        read("docs/src/SUMMARY.md").contains("(quick-start-orchestrator.md)"),
        "SUMMARY.md must list the new quick start"
    );
}

/// Scenarios: Quick start chapter does not mention agent.register /
/// agent.done.
#[test]
fn quick_start_names_no_nonexistent_message_types() {
    let doc = quick_start();
    for bogus in ["agent.register", "agent.done"] {
        assert!(
            !doc.contains(bogus),
            "the quick start must not name the nonexistent message type {bogus}"
        );
    }
}

/// Scenario: Quick start describes the external-orchestrator workflow.
#[test]
fn quick_start_describes_the_external_orchestrator_workflow() {
    let doc = quick_start();
    for needle in [
        "git paw init",
        "git paw start --supervisor",
        ".git-paw/skills/orchestrator/SKILL.md",
        "sweep.sh verified",
        "--unattended",
        "remote-control.md",
    ] {
        assert!(
            doc.contains(needle),
            "the quick start must mention {needle}"
        );
    }
    assert!(
        !doc.contains("curl"),
        "the orchestrator walkthrough must not teach a raw curl"
    );
}

/// Scenario: Quick start does not mark shipped features as deferred.
#[test]
fn quick_start_marks_no_shipped_feature_as_deferred() {
    let doc = quick_start().to_lowercase();
    for marker in ["not yet supported", "not supported yet", "coming soon"] {
        for feature in ["conflict detection", "learnings"] {
            let deferred = doc
                .lines()
                .any(|l| l.contains(marker) && l.contains(feature));
            assert!(
                !deferred,
                "the quick start must not describe {feature} as {marker}"
            );
        }
    }
}

/// Scenario: Chapter states the topology and the local-process caveat.
#[test]
fn remote_control_chapter_states_topology_and_caveats() {
    let doc = remote_control();
    let lower = doc.to_lowercase();
    assert!(
        lower.contains("local machine") && lower.contains("mcp server"),
        "the chapter must state the driving session runs locally with git-paw's MCP server attached"
    );
    assert!(
        lower.contains("must stay running") && lower.contains("ten minutes"),
        "the chapter must state the local-process caveat and the ~10-minute outage limit"
    );
    assert!(
        lower.contains("no bridge") && lower.contains("tunnel"),
        "the chapter must state that git-paw ships no bridge or tunnel"
    );
    for link in ["quick-start-orchestrator.md", "mcp.md"] {
        assert!(doc.contains(link), "the chapter must link {link}");
    }
}

/// Scenario: Chapter names supported CLIs with a check date.
#[test]
fn remote_control_chapter_names_clis_with_a_check_date() {
    let doc = remote_control();
    assert!(
        doc.contains("Last checked:"),
        "the CLI table must carry the date its facts were last verified"
    );
    let has_iso_date = doc
        .lines()
        .filter(|l| l.contains("Last checked:"))
        .any(|l| {
            l.split(|c: char| !(c.is_ascii_digit() || c == '-'))
                .any(|tok| tok.len() == 10 && tok.chars().filter(|c| *c == '-').count() == 2)
        });
    assert!(has_iso_date, "the check date must be an ISO date");
    for cli in ["Claude Code", "Copilot", "Codex", "Cursor"] {
        assert!(
            doc.lines().any(|l| l.starts_with('|') && l.contains(cli)),
            "the CLI table must have a row for {cli}"
        );
    }
}

/// Scenario: The security-posture controls table links the chapter.
#[test]
fn security_posture_remote_control_row_links_the_chapter() {
    let posture = read("docs/src/user-guide/security-posture.md");
    let row = posture
        .lines()
        .find(|l| l.starts_with("| **Remote control**"))
        .expect("the controls table must keep its Remote control row");
    assert!(
        row.contains("remote-control.md"),
        "the Remote control row must link remote-control.md: {row:?}"
    );
    assert!(
        read("docs/src/SUMMARY.md").contains("(user-guide/remote-control.md)"),
        "SUMMARY.md must list the remote-control chapter"
    );
}

/// Scenario: Chapter describes no in-session orchestrator.
#[test]
fn remote_control_chapter_describes_no_in_session_orchestrator() {
    assert!(
        !remote_control().to_lowercase().contains("supervisor pane"),
        "the chapter must not describe a remote-controlled supervisor pane"
    );
}

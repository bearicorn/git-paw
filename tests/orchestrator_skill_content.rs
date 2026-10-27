//! Skill-content assertions for the `orchestrator-skill-export` conversion of
//! the bundled supervisor skill into the orchestrator skill
//! (`assets/agent-skills/orchestrator.md`).
//!
//! Each test pins a WHEN-the-skill-is-read / THEN-the-prose-SHALL-state
//! scenario from the change's `skill-agent`, `supervisor-skill-discipline`
//! and `broker-watcher-and-state` deltas: no pane-bound self-reference, no
//! session-specific value, no `curl`, the event-surface monitoring, the
//! roster read when needed, the unattended merge rule, and the re-addressed
//! permission doctrine.

use git_paw::skills::{GateCommands, render_for_supervisor, resolve};
use git_paw::specs::SpecBackendKind;

/// The raw orchestrator template, frontmatter included.
fn template() -> String {
    std::fs::read_to_string("assets/agent-skills/orchestrator.md")
        .expect("orchestrator.md is at the expected path")
}

/// The orchestrator skill rendered for `backends`, with a broker URL and a
/// project name supplied so a leftover placeholder would surface as a value.
fn rendered_for(backends: &[SpecBackendKind]) -> String {
    let tmpl = resolve("orchestrator").expect("orchestrator resolves");
    render_for_supervisor(
        &tmpl,
        "http://127.0.0.1:9119",
        "my-project",
        &GateCommands::default(),
        backends,
    )
}

/// Every rendering the skill can take: each spec backend and none.
fn every_rendering() -> Vec<(String, String)> {
    let mut out = vec![("no backend".to_string(), rendered_for(&[]))];
    for kind in [
        SpecBackendKind::OpenSpec,
        SpecBackendKind::SpecKit,
        SpecBackendKind::Markdown,
        SpecBackendKind::Superpowers,
    ] {
        out.push((format!("{kind:?}"), rendered_for(&[kind])));
    }
    out
}

/// The `### ` (or `## `) section opened by `heading`, whitespace-collapsed and
/// lower-cased so substring checks survive line wrapping.
fn section(heading: &str) -> String {
    let skill = template();
    let start = skill
        .find(heading)
        .unwrap_or_else(|| panic!("orchestrator.md has a `{heading}` section"));
    let rest = &skill[start..];
    let end = rest[1..].find("\n### ").map_or(rest.len(), |idx| idx + 1);
    collapse(&rest[..end])
}

fn collapse(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

// skill-agent / Scenario: Skill carries no pane-bound self-reference
#[test]
fn skill_carries_no_pane_bound_self_reference() {
    for (label, text) in every_rendering() {
        let lower = text.to_lowercase();
        assert!(!lower.contains("your pane"), "{label}: `your pane` present");
        assert!(!lower.contains("own pane"), "{label}: `own pane` present");
    }
    // Every mention of pane 0 identifies it as the dashboard, within the
    // same line or the next.
    let skill = template();
    let lines: Vec<&str> = skill.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.to_lowercase().contains("pane 0") {
            let window = format!("{line} {}", lines.get(i + 1).unwrap_or(&""));
            assert!(
                window.contains("dashboard"),
                "line {} mentions pane 0 without identifying it as the dashboard: {line}",
                i + 1
            );
        }
    }
}

// skill-agent / Scenario: Skill carries no session-specific value
#[test]
fn skill_carries_no_session_specific_value() {
    let raw = template();
    assert!(!raw.contains("{{GIT_PAW_BROKER_URL}}"));
    assert!(!raw.contains("paw-{{PROJECT_NAME}}"));
    for (label, text) in every_rendering() {
        assert!(!text.contains("http://"), "{label}: a broker URL leaked");
        assert!(
            !text.contains("paw-my-project"),
            "{label}: the session name leaked"
        );
    }
    assert!(
        raw.contains(".git-paw/AGENTS.local.md"),
        "the skill must name the sidecar as where the session values live"
    );
    let intro = collapse(&raw[..raw.find("### Bootstrap").expect("bootstrap section")]);
    assert!(intro.contains("session name") && intro.contains("broker url"));
}

// skill-agent / Scenario: Verdicts route through the helper
#[test]
fn verdicts_route_through_the_helper() {
    let skill = template();
    for verb in [
        ".git-paw/scripts/sweep.sh verified",
        ".git-paw/scripts/sweep.sh feedback-gate",
        "sweep.sh conflict-ack",
    ] {
        assert!(skill.contains(verb), "verdict path must name `{verb}`");
    }
    let compact: String = skill.chars().filter(|c| !c.is_whitespace()).collect();
    for shape in [
        "curl-s-XPOST",
        r#"{"type":"agent.verified""#,
        r#"{"type":"agent.feedback""#,
    ] {
        assert!(
            !compact.contains(shape),
            "no raw publish of a verdict: found `{shape}`"
        );
    }
}

// skill-agent / Scenario: The rendered orchestrator skill contains no curl
#[test]
fn no_rendering_contains_curl() {
    for (label, text) in every_rendering() {
        assert!(
            !text.to_lowercase().contains("curl"),
            "{label}: the rendered orchestrator skill must not contain `curl`"
        );
    }
}

// skill-agent / *Supervisor skill SHALL reference the bundled sweep helper*:
// the minimum taught subcommand set.
#[test]
fn skill_teaches_the_helper_subcommands() {
    let skill = template();
    for verb in [
        "snapshot",
        "capture",
        "approve",
        "status",
        "worktrees-status",
        "inbox",
        "events",
        "feedback-gate",
        "verified",
        "conflict-ack",
        "status-publish",
        "answer",
        "advanced-main",
    ] {
        assert!(
            skill.contains(&format!(".git-paw/scripts/sweep.sh {verb}"))
                || skill.contains(&format!("`sweep.sh {verb}")),
            "the skill must teach `sweep.sh {verb}`"
        );
    }
}

// skill-agent / Scenario: A stalled worker is told to use its own helper
#[test]
fn stalled_worker_is_told_to_use_its_own_helper() {
    let skill = template();
    let stall = skill
        .find("You appear stalled")
        .expect("stall message present");
    let line_end = skill[stall..].find('\n').map_or(skill.len(), |i| stall + i);
    let message = &skill[stall..line_end];
    assert!(message.contains(".git-paw/scripts/broker.sh blocked"));
    assert!(!message.contains("http") && !message.contains("/publish"));
}

// skill-agent / Scenario: Skill teaches the cursor read
#[test]
fn skill_teaches_the_cursor_read() {
    let s = section("### Follow the session through the event surface");
    assert!(s.contains("sweep.sh events") && s.contains("wait_for_events"));
    assert!(s.contains("carry `last_seq` forward"));
    assert!(
        s.contains("a sampled capture can miss a prompt"),
        "the section must give the sampled-capture rationale"
    );
    let watch = collapse(&template());
    assert!(
        !watch.contains("every ~30 seconds"),
        "no fixed-interval polling as the primary detector"
    );
}

// skill-agent / Scenario: Skill acts on a stalled wave
#[test]
fn skill_acts_on_a_stalled_wave() {
    let s = section("### Follow the session through the event surface");
    assert!(s.contains("stalled") && s.contains("investigate"));
}

// skill-agent / Scenario: Skill relays escalated prompts read from events
#[test]
fn skill_relays_escalated_prompts_read_from_events() {
    let s = section("### Follow the session through the event surface");
    assert!(s.contains("escalates every live permission prompt to the broker and approves none"));
    assert!(s.contains("relay each escalated prompt to the human"));
    assert!(s.contains("sweep.sh approve <pane>` only on the human's explicit instruction"));
}

// skill-agent / Scenario: Skill teaches re-attaching the drive loop after a
// heartbeat exit
#[test]
fn skill_teaches_reattaching_the_drive_loop() {
    let s = section("### Follow the session through the event surface");
    assert!(s.contains("heartbeat"));
    assert!(s.contains("re-run `git paw start --unattended`"));
    assert!(s.contains("re-attaches a drive loop to the live session"));
}

// skill-agent / Scenario: Unattended escalations are read from the broker,
// not a pane
#[test]
fn unattended_escalations_are_read_from_the_broker() {
    let s = section("### Who decides a permission prompt");
    assert!(s.contains("the drive loop escalates it to the broker"));
    let whole = collapse(&template());
    assert!(!whole.contains("supervisor's pane"));
    assert!(!whole.contains("hands it to your pane"));
}

// skill-agent / Scenario: Skill reads the roster when needed instead of
// caching a session-long map
#[test]
fn skill_reads_the_roster_when_needed() {
    let s = section("### Resolve pane to agent via pane_current_path");
    assert!(s.contains("read the roster when you need the pane of an agent"));
    assert!(!s.contains("once, and reuse it for the rest of the session"));
    assert!(s.contains("git paw add") && s.contains("git paw remove"));
    assert!(s.contains("tmux display-message -t <session>:0.<pane>"));
}

// skill-agent / Scenario: An unattended merge follows the trigger without
// asking first
#[test]
fn unattended_merge_follows_the_trigger() {
    let s = section("### Merge orchestration");
    assert!(s.contains("without first asking the human"));
    assert!(s.contains("fast-forward only"));
    for escalation in [
        "a conflict",
        "dependency cycle",
        "an exhausted correction cycle",
        "a regression",
    ] {
        assert!(s.contains(escalation), "must name `{escalation}`");
    }
    assert!(
        !template().contains("Ask the human before merging"),
        "the unconditional ask-before-merging rule must be gone"
    );
}

// skill-agent / Scenario: An attended merge may confirm with the human
#[test]
fn attended_merge_may_confirm_with_the_human() {
    let s = section("### Merge orchestration");
    assert!(s.contains("you may confirm the merge order with the human"));
}

// skill-agent *Merge orchestration*: reads and the cycle question go through
// the helpers.
#[test]
fn merge_orchestration_reads_through_helpers() {
    let s = section("### Merge orchestration");
    assert!(s.contains("sweep.sh inbox") || s.contains("sweep.sh events"));
    assert!(s.contains("broker.sh --agent supervisor question"));
}

// supervisor-skill-discipline / Scenario: Skill states the
// never-non-agent-surface rule
#[test]
fn skill_states_the_never_non_agent_surface_rule() {
    let s = section("### Driving agent panes");
    assert!(s.contains("never to a non-agent surface"));
    assert!(s.contains("pane 0 is the dashboard"));
    assert!(s.contains("your own terminal"));
    assert!(s.contains("interrupts"));
}

// supervisor-skill-discipline / Scenario: With a loop running, escalations
// are handled before the sweep; An approval follows an explicit human
// instruction
#[test]
fn drive_loop_directive_relays_before_the_sweep() {
    let skill = template();
    let start = skill
        .find("<!-- drive-loop-directive:begin -->")
        .expect("directive region");
    let end = skill
        .find("<!-- drive-loop-directive:end -->")
        .expect("directive region end");
    let region = collapse(&skill[start..end]);
    let drain = region.find("drain the loop's escalations first").unwrap();
    let sweep = region.find("then run your normal sweep").unwrap();
    assert!(drain < sweep);
    assert!(region.contains("relay each one to the human in your session"));
    assert!(region.contains("only when the human explicitly tells you to"));
}

// supervisor-skill-discipline / Scenario: User asks for the agent inventory
// (the supervisor row carries no pane index)
#[test]
fn agents_inventory_supervisor_row_has_no_pane_index() {
    let s = section("### Routing through the supervisor");
    assert!(s.contains("your `supervisor` row carries **no pane index**"));
    let skill = template();
    let row = skill
        .lines()
        .find(|l| l.starts_with("supervisor  "))
        .expect("example table carries a supervisor row");
    assert!(
        row.trim_end().ends_with('—'),
        "the supervisor row's pane column must be empty: {row}"
    );
}

// supervisor-skill-discipline / Scenario: Successful tell to a live agent —
// `/agents` and `/tell` are typed in the orchestrator's own session.
#[test]
fn directives_are_typed_in_the_orchestrators_session() {
    let s = section("### Routing through the supervisor");
    assert!(s.contains("user types them at the start of a line in your session"));
    assert!(s.contains("acknowledge** in your session"));
}

// skill-agent / Scenario: Skill mentions interactive user input
#[test]
fn user_input_section_is_addressed_to_the_session() {
    let s = section("### When the user types in your session");
    assert!(s.contains("sweep.sh status") && s.contains("sweep.sh capture"));
    assert!(s.contains("sweep.sh feedback-gate"));
    assert!(s.contains("broker.sh --agent supervisor question"));
}

// skill-agent / *Supervisor skill — permission decisions belong to the agent
// CLI and the human*: the per-CLI setup section names the exported skill.
#[test]
fn per_cli_setup_names_the_exported_cli_setup_skill() {
    let s = section("## Per-CLI setup");
    assert!(s.contains(".git-paw/skills/cli-setup/skill.md"));
    assert!(s.contains("get_skill"));
}

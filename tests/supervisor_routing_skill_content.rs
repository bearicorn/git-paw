//! Skill-content assertions for the `supervisor-tell` capability's routing
//! prose.
//!
//! These are WHEN-the-skill-is-read / THEN-the-prose-SHALL-state requirements
//! that pin the `/agents` + `/tell` "Routing through the supervisor" section
//! into the bundled `assets/agent-skills/supervisor.md` so a future edit can't
//! silently drop the routing conventions (tasks 4, 5, 6, 7.1, 8). The
//! executable behaviour is unit-tested in `git_paw::coordination`; these
//! scenarios cover the prose-only requirements that have no other asserting
//! test.

use std::fs;

fn supervisor_skill() -> String {
    fs::read_to_string("assets/agent-skills/supervisor.md")
        .expect("supervisor.md is at the expected path")
}

/// The routing section, lower-cased, for case-insensitive substring scans.
fn routing_section() -> String {
    let skill = supervisor_skill();
    let start = skill
        .find("### Routing through the supervisor")
        .expect("supervisor.md has a Routing through the supervisor section");
    let rest = &skill[start..];
    // Section runs until the next top-level (`### `) heading.
    let end = rest[1..].find("\n### ").map_or(rest.len(), |idx| idx + 1);
    // Collapse whitespace so substring checks are robust to line wrapping.
    rest[..end]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

// Task 4.1 — the section documents both `/agents` and `/tell` with examples.
#[test]
fn documents_agents_and_tell_commands() {
    let section = routing_section();
    assert!(section.contains("/agents"), "must document /agents");
    assert!(section.contains("/tell"), "must document /tell");
}

// Task 5.1 — inventory formatted one row per agent PLUS the supervisor's own
// row, each carrying the documented fields.
#[test]
fn inventory_row_shape_includes_supervisor_and_fields() {
    let section = routing_section();
    assert!(
        section.contains("one row per agent") && section.contains("supervisor"),
        "must state one row per agent plus the supervisor row"
    );
    for field in [
        "branch_id",
        "status",
        "last_seen",
        "cli",
        "mode",
        "pane_index",
    ] {
        assert!(
            section.contains(field),
            "inventory row must document the `{field}` field"
        );
    }
}

// Task — inventory sourcing names both broker /status and pane_current_path.
#[test]
fn inventory_sourcing_named() {
    let section = routing_section();
    assert!(
        section.contains("/status"),
        "must source from broker /status"
    );
    assert!(
        section.contains("pane_current_path"),
        "must resolve pane index via pane_current_path"
    );
}

// Task — cache freshness window references the config key + default.
#[test]
fn cache_freshness_documented() {
    let section = routing_section();
    assert!(
        section.contains("inventory_max_age_seconds"),
        "must reference the inventory_max_age_seconds config key"
    );
    assert!(
        section.contains("cache") || section.contains("cached"),
        "must describe the cache reuse"
    );
}

// Task 6.1(b)/validation — unknown target yields a candidate list and no
// delivery.
#[test]
fn unknown_target_yields_candidate_list_no_delivery() {
    let section = routing_section();
    assert!(
        section.contains("unknown target"),
        "must show the unknown-target rejection"
    );
    assert!(
        section.contains("available agents"),
        "must list available agents as candidates"
    );
    assert!(
        section.contains("not") && section.contains("deliver"),
        "must state nothing is delivered for an unknown target"
    );
}

// Task 4.2 / 6.1(c) — the D3 delivery-mode precedence is documented.
#[test]
fn delivery_mode_precedence_documented() {
    let section = routing_section();
    assert!(section.contains("send-keys"), "must mention send-keys mode");
    assert!(
        section.contains("agent.feedback"),
        "must mention feedback mode"
    );
    assert!(
        section.contains("accept-edits"),
        "send-keys precedence keys off accept-edits mode"
    );
    assert!(
        section.contains("fall back") || section.contains("fallback"),
        "must document the send-keys → feedback fallback"
    );
}

// Task 6.4 — fallback emits a stderr-side note.
#[test]
fn fallback_emits_note() {
    let section = routing_section();
    assert!(
        section.contains("note:") || section.contains("stderr"),
        "fallback must emit a note for the user"
    );
}

// Task 6.3 — send-keys delivery references the paste-buffer double-Enter
// pattern and the never-pane-0 guard.
#[test]
fn send_keys_delivery_safety() {
    let section = routing_section();
    assert!(
        section.contains("tmux send-keys"),
        "must use tmux send-keys"
    );
    assert!(
        section.contains("paste buffer") || section.contains("pasted text"),
        "must reference the paste-buffer pattern"
    );
    assert!(
        section.contains("pane 0"),
        "must guard against sending to the supervisor's own pane 0"
    );
}

// Task 7.1 — routing-decision recording into the learnings section, gated on
// learnings = true, with truncation.
#[test]
fn learnings_recording_documented() {
    let section = routing_section();
    assert!(
        section.contains("### supervisor routing"),
        "must name the Supervisor routing learnings section"
    );
    assert!(
        section.contains("learnings = true"),
        "recording must be gated on learnings = true"
    );
    assert!(
        section.contains("learnings = false"),
        "must state nothing is written when learnings = false"
    );
    assert!(
        section.contains("200"),
        "must document the 200-char prompt truncation"
    );
}

// Task 8.1 / 8.2 — proactive routing: detection heuristic + mandatory
// agent.question confirmation before any /tell fires.
#[test]
fn proactive_routing_requires_confirmation() {
    let section = routing_section();
    assert!(
        section.contains("proactive"),
        "must document the proactive-routing pattern"
    );
    assert!(
        section.contains("blocked"),
        "detection heuristic keys off a blocked agent"
    );
    assert!(
        section.contains("agent.question"),
        "must offer the route via agent.question"
    );
    assert!(
        section.contains("shall not")
            && (section.contains("auto") || section.contains("affirmative")),
        "must forbid autonomous (un-confirmed) routing"
    );
}

// Non-goal — no inference backend invoked to generate the prompt.
#[test]
fn no_inference_backend_for_prompt() {
    let section = routing_section();
    assert!(
        section.contains("shall not pipe") || section.contains("never from spawning"),
        "must state /tell does not pipe a prompt into another CLI to generate content"
    );
}

// Task 4.4 — the coordination skill (peer-to-peer) is NOT touched: it must
// carry none of the supervisor-only routing patterns.
#[test]
fn coordination_skill_has_no_routing_patterns() {
    let coordination = fs::read_to_string("assets/agent-skills/coordination.md")
        .expect("coordination.md is at the expected path");
    assert!(
        !coordination.contains("/tell"),
        "coordination.md (peer-to-peer) must not document /tell"
    );
    assert!(
        !coordination.contains("/agents"),
        "coordination.md (peer-to-peer) must not document /agents"
    );
    assert!(
        !coordination.contains("Supervisor routing"),
        "coordination.md must not carry the supervisor routing section"
    );
}

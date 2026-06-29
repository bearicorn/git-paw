//! Prose assertions for the `supervisor-stuck-bloat-detection` change.
//!
//! Verifies the bundled skill files document the new detection surface:
//!
//! - `supervisor.md` "Detecting stuck agents" names all five stuck shapes and
//!   states the read-pane-before-classifying rule;
//! - the "Stream-timeout recovery" error-shape subsection covers a CODING
//!   agent's stream timeout (detected via sweep.sh, surfaced as
//!   `stuck-stream-timeout`) distinct from the supervisor's own;
//! - the "N re-verify cycles is not a stall" rule is present and cites the
//!   observed cycle counts;
//! - `coordination.md` "Context budget" notes proactive context-bloat flagging
//!   tied to commit-before-compact.
//!
//! Maps to openspec/changes/supervisor-stuck-bloat-detection/specs/{stuck-prompt-detection,
//! supervisor-stream-timeout-recovery,coordination-context-budget}/spec.md

use std::fs;

fn supervisor_md() -> String {
    fs::read_to_string("assets/agent-skills/supervisor.md")
        .expect("supervisor.md is at the expected path")
}

fn coordination_md() -> String {
    fs::read_to_string("assets/agent-skills/coordination.md")
        .expect("coordination.md is at the expected path")
}

/// The "Detecting stuck agents" section documents all five stuck shapes.
#[test]
fn supervisor_detecting_section_names_all_five_shapes() {
    let s = supervisor_md();
    assert!(
        s.contains("### Detecting stuck agents"),
        "supervisor.md must retain the 'Detecting stuck agents' section"
    );
    for shape in [
        "stuck-on-prompt",
        "stuck-stream-timeout",
        "context-bloat",
        "no-progress",
        "blocked-on-supervisor",
    ] {
        assert!(
            s.contains(shape),
            "'Detecting stuck agents' must document the `{shape}` shape"
        );
    }
}

/// The read-pane-before-classifying rule is stated: an idle-looking but
/// prompt-blocked agent is stuck-on-prompt, never no-progress.
#[test]
fn supervisor_states_read_pane_before_classifying_rule() {
    let s = supervisor_md();
    assert!(
        s.contains("Read the live pane before you classify"),
        "supervisor.md must state the read-pane-before-classifying rule"
    );
    assert!(
        s.contains("never no-progress"),
        "the rule must say a prompt-blocked agent is NOT classified no-progress"
    );
    // Must warn against classifying from counts alone (the dogfood mis-call).
    let lowered = s.to_lowercase();
    assert!(
        lowered.contains("branch-tip") && lowered.contains("count"),
        "the rule must forbid judging idleness from branch-tip/file counts alone"
    );
}

/// The inline-bash-reinvention prohibition is retained.
#[test]
fn supervisor_retains_inline_bash_prohibition() {
    let s = supervisor_md();
    assert!(
        s.contains("Do NOT hand-roll an inline-bash monitor"),
        "supervisor.md must retain the inline-bash-reinvention prohibition"
    );
}

/// The stream-timeout recovery section covers a coding agent's stream timeout,
/// detected via sweep.sh and surfaced as `stuck-stream-timeout`, distinct from
/// the supervisor's own.
#[test]
fn supervisor_covers_coding_agent_stream_timeout() {
    let s = supervisor_md();
    // Anchored in the error-shape recovery discussion.
    assert!(
        s.contains("### Stream-timeout recovery"),
        "supervisor.md must retain the Stream-timeout recovery section"
    );
    assert!(
        s.contains("stuck-stream-timeout"),
        "the section must name the stuck-stream-timeout phase for a coding agent"
    );
    assert!(
        s.contains("detect-stuck"),
        "the coding-agent case must reference sweep.sh detect-stuck detection"
    );
    let lowered = s.to_lowercase();
    assert!(
        lowered.contains("coding\nagent") || lowered.contains("coding agent"),
        "the section must distinguish a coding agent's stream timeout from the supervisor's own"
    );
}

/// The "N re-verify cycles is not a stall" rule is present and cites the
/// observed cycle counts (mcp-server 7, dev-allowlist 6), judging stall by the
/// detected shapes rather than the cycle count.
#[test]
fn supervisor_states_reverify_cycles_not_a_stall() {
    let s = supervisor_md();
    assert!(
        s.contains("re-verify cycles is not a stall"),
        "supervisor.md must add the 'N re-verify cycles is not a stall' rule"
    );
    assert!(
        s.contains("mcp-server") && s.contains("dev-allowlist"),
        "the rule must cite the observed examples (mcp-server, dev-allowlist)"
    );
    assert!(
        s.contains("**7**") && s.contains("**6**"),
        "the rule must cite the observed cycle counts (7 and 6)"
    );
    assert!(
        s.contains("cycle count"),
        "the rule must state that stall is not judged by the cycle count alone"
    );
}

/// The coordination "Context budget" section notes proactive context-bloat
/// flagging tied to commit-before-compact.
#[test]
fn coordination_context_budget_notes_proactive_bloat_flagging() {
    let c = coordination_md();
    assert!(
        c.contains("### Context budget"),
        "coordination.md must retain the Context budget section"
    );
    assert!(
        c.contains("context-bloat"),
        "Context budget must mention the context-bloat phase the supervisor flags"
    );
    assert!(
        c.contains("phase: \"context-bloat\""),
        "Context budget must name the synthetic phase value surfaced by the supervisor"
    );
    assert!(
        c.contains("commit-before-compact"),
        "the proactive-bloat note must tie back to the commit-before-compact discipline"
    );
}

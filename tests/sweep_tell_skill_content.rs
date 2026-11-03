//! Skill-content assertions for `verified-agent-send`
//! (supervisor-skill-discipline: *Typed delivery to an agent goes through
//! sweep.sh tell* and *The skill explains the sweep.sh tell results*).
//!
//! The bundled orchestrator skill must route every message it types into an
//! agent through `sweep.sh tell`, never raw `tmux send-keys`, and must say
//! what to do with each result.

use std::fs;

fn skill() -> String {
    fs::read_to_string("assets/agent-skills/orchestrator.md")
        .expect("orchestrator.md is at the expected path")
}

/// Collapses whitespace and lower-cases, so checks survive line wrapping.
fn flat(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The `### ` section opened by `heading`, flattened.
fn section(heading: &str) -> String {
    let skill = skill();
    let start = skill
        .find(heading)
        .unwrap_or_else(|| panic!("orchestrator.md has a `{heading}` section"));
    let rest = &skill[start..];
    let end = rest[1..].find("\n### ").map_or(rest.len(), |idx| idx + 1);
    flat(&rest[..end])
}

/// Spec scenario "The skill routes send-keys delivery through the helper".
#[test]
fn send_keys_delivery_and_paste_buffer_recovery_name_sweep_tell() {
    let routing = section("### Routing through the supervisor");
    assert!(routing.contains("sweep.sh tell"), "/tell send-keys mode");

    let skill = skill();
    let start = skill
        .find("**Paste-buffer recovery**")
        .expect("paste-buffer recovery sub-case");
    let recovery = flat(&skill[start..(start + 2500).min(skill.len())]);
    assert!(recovery.contains("sweep.sh tell"), "paste-buffer recovery");
}

/// Spec scenario "The skill routes send-keys delivery through the helper",
/// negative half: no instruction types a message with raw `tmux send-keys`.
/// The one remaining mention is the forbidden inline-loop example.
#[test]
fn no_instruction_types_a_message_with_raw_send_keys() {
    for line in skill().lines() {
        if line.contains("tmux send-keys -t") {
            assert!(
                line.contains("for p in <panes>"),
                "raw send-keys instruction left in the skill: {line}"
            );
        }
    }
}

/// Spec scenario "The skill explains the results".
#[test]
fn the_skill_explains_delivered_wedged_and_refused() {
    let typing = section("### Typing a message into an agent");
    for result in ["delivered", "wedged", "refused"] {
        assert!(typing.contains(result), "explains `{result}`");
    }
    assert!(
        typing.contains("feedback"),
        "wedged falls back to the feedback mode"
    );
    assert!(
        typing.contains("permission prompt showing")
            && typing.contains("selection menu showing")
            && typing.contains("relay what is on screen to the human")
            && typing.contains("must never answer it")
            && typing.contains("sweep.sh approve")
            && typing.contains("only on the human"),
        "a live prompt or menu is relayed, never answered: {typing}"
    );
    assert!(
        typing.contains("exit `4`") && typing.contains("do not fall back to raw keystrokes"),
        "a failing __send is reported, not worked around: {typing}"
    );
}

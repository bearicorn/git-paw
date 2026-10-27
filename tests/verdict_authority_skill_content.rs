//! Skill-content assertions for the `skill-agent` capability's verdict
//! examples, as modified by `broker-verdict-authority`.
//!
//! The broker refuses an `agent.verified`, or an `agent.feedback` from
//! `supervisor`, that lacks the root-session authority credential, so the
//! bundled orchestrator skill SHALL publish both through the `sweep.sh`
//! helper verbs (which attach it) and SHALL NOT show a raw `curl …/publish`
//! for either.

use std::fs;

fn supervisor_skill() -> String {
    fs::read_to_string("assets/agent-skills/orchestrator.md")
        .expect("orchestrator.md is at the expected path")
}

/// The `### ` section opened by `heading`, up to the next `### ` heading.
fn section(heading: &str) -> String {
    let skill = supervisor_skill();
    let start = skill
        .find(heading)
        .unwrap_or_else(|| panic!("orchestrator.md has a `{heading}` section"));
    let rest = &skill[start..];
    let end = rest[1..].find("\n### ").map_or(rest.len(), |idx| idx + 1);
    rest[..end].to_string()
}

fn verified_section() -> String {
    section("### Publish verification outcome")
}

fn feedback_section() -> String {
    section("### Publish feedback to a peer agent")
}

/// Every `curl` invocation in the skill that posts a body of type `ty`.
fn curl_posts_of(ty: &str) -> Vec<String> {
    let skill = supervisor_skill();
    let needle = format!("\"type\":\"{ty}\"");
    let escaped = format!("\\\"type\\\":\\\"{ty}\\\"");
    skill
        .match_indices("curl ")
        .map(|(i, _)| {
            // A curl invocation runs until the end of its (possibly
            // backslash-continued) command: cap at a generous window ending
            // at the next blank line or closing fence.
            let rest = &skill[i..];
            let end = rest.find("\n```").map_or(rest.len(), |e| e.min(1200));
            rest[..end].to_string()
        })
        .filter(|cmd| cmd.contains(&needle) || cmd.contains(&escaped))
        .collect()
}

/// Scenario: Verified example uses the helper.
#[test]
fn verified_example_uses_the_helper_and_no_raw_curl() {
    assert!(
        verified_section().contains("sweep.sh verified"),
        "the agent.verified example must use `sweep.sh verified`"
    );
    let raw = curl_posts_of("agent.verified");
    assert!(
        raw.is_empty(),
        "no curl may post an agent.verified body: {raw:?}"
    );
}

/// Scenario: Verified prose names the wire fields and the recipient semantics.
#[test]
fn verified_prose_names_wire_fields_and_recipient() {
    let sec = verified_section();
    for field in ["verified_by", "message"] {
        assert!(sec.contains(&format!("`{field}`")), "must name `{field}`");
    }
    assert!(
        sec.contains("**recipient** (the agent being verified)"),
        "must state the agent id is the recipient being verified"
    );
    for stale in [
        "`target`",
        "`result`",
        "`notes`",
        "\"target\"",
        "\"result\"",
        "\"notes\"",
    ] {
        assert!(!sec.contains(stale), "must not name stale field {stale}");
    }
}

/// Scenario: Feedback example uses the helper.
#[test]
fn feedback_example_uses_the_helper_and_no_raw_curl() {
    assert!(
        feedback_section().contains("sweep.sh feedback-gate"),
        "the agent.feedback example must use `sweep.sh feedback-gate`"
    );
    let raw = curl_posts_of("agent.feedback");
    assert!(
        raw.is_empty(),
        "no curl may post an agent.feedback body: {raw:?}"
    );
}

/// Scenario: Feedback prose names the wire fields and the recipient semantics.
#[test]
fn feedback_prose_names_wire_fields_and_recipient() {
    let sec = feedback_section();
    for field in ["from", "errors"] {
        assert!(sec.contains(&format!("`{field}`")), "must name `{field}`");
    }
    assert!(
        sec.contains("**recipient**"),
        "must state the agent id is the recipient receiving feedback"
    );
    for stale in ["`target`", "\"target\"", "`message`", "\"message\""] {
        assert!(
            !sec.contains(stale),
            "must not name stale feedback field {stale}"
        );
    }
}

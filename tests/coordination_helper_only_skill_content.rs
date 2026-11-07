//! Checks that the coordination skill sends every broker call through
//! `broker.sh` and asks for bare helper calls, so one permission rule covers
//! all of a worker's broker traffic. Each test names the `skill-agent`
//! scenario it covers.

use std::fs;

fn coordination_skill() -> String {
    fs::read_to_string("assets/agent-skills/coordination.md")
        .expect("coordination.md is at the expected path")
}

/// The `### <heading>` subsection of the skill, up to the next `###` heading.
fn section(skill: &str, heading: &str) -> String {
    let start = skill
        .find(heading)
        .unwrap_or_else(|| panic!("coordination.md has a {heading} section"));
    let rest = &skill[start..];
    let end = rest[1..].find("\n### ").map_or(rest.len(), |idx| idx + 1);
    rest[..end].to_string()
}

/// Scenario: Coordination skill contains no broker curl.
#[test]
fn coordination_skill_contains_no_broker_curl() {
    let skill = coordination_skill();
    for (n, line) in skill.lines().enumerate() {
        let aims_at_broker = line.contains("{{GIT_PAW_BROKER_URL}}")
            || line.contains("/publish")
            || line.contains("/messages");
        assert!(
            !(line.contains("curl") && aims_at_broker),
            "line {} still shows a raw broker curl: {line}",
            n + 1
        );
    }
    assert!(
        !skill.contains("curl -s"),
        "the coordination skill must not show any curl invocation"
    );
}

/// Scenario: Coordination skill contains no broker curl (helper half).
#[test]
fn each_broker_verb_has_a_helper_example() {
    let skill = coordination_skill();
    for verb in [
        "intent", "question", "blocked", "artifact", "status", "poll",
    ] {
        let call = format!(".git-paw/scripts/broker.sh --agent {{{{BRANCH_ID}}}} {verb} ");
        let bare_poll = ".git-paw/scripts/broker.sh --agent {{BRANCH_ID}} poll\n";
        assert!(
            skill.contains(&call) || (verb == "poll" && skill.contains(bare_poll)),
            "coordination skill should show a `broker.sh {verb}` example"
        );
    }
}

/// Scenario: Coordination skill asks for bare helper calls.
#[test]
fn coordination_skill_asks_for_bare_helper_calls() {
    let skill = coordination_skill();
    let bare = section(&skill, "### Run dev commands bare");
    assert!(
        bare.contains("`.git-paw/scripts/broker.sh` call"),
        "the bare-commands section should name the helper calls"
    );
    let lowered = bare.to_lowercase();
    assert!(
        lowered.contains("no pipe") && lowered.contains("echo \"exit=$?\""),
        "the helper instruction should rule out an exit-status probe and a pipe"
    );
    for cli in ["claude", "codex", "gemini", "aider", "cursor"] {
        assert!(
            !lowered.contains(cli),
            "the bare-call instruction should name no agent CLI ({cli})"
        );
    }
    let examples: Vec<&str> = skill
        .lines()
        .filter(|l| l.contains("broker.sh --agent"))
        .collect();
    assert!(
        examples.len() >= 6,
        "expected the skill's helper examples, found {}",
        examples.len()
    );
    for line in examples {
        assert!(
            !line.contains("; echo") && !line.contains(" | ") && !line.contains("&& echo"),
            "a helper example must be bare: {line}"
        );
    }
}

/// The exported skill carries no git-paw release history and no project's own
/// CLI variant name.
#[test]
fn coordination_skill_carries_no_git_paw_history_or_variant_names() {
    let skill = coordination_skill();
    for leak in ["claude-oss", "148 commits", "v0.6.0", "v0.7.0"] {
        assert!(
            !skill.contains(leak),
            "coordination skill should not carry `{leak}`"
        );
    }
}

/// Scenario: Activity and region examples use the helper.
#[test]
fn activity_and_region_examples_use_the_helper() {
    let skill = coordination_skill();
    let activity = section(&skill, "#### Saying what you are doing");
    assert!(
        activity.contains("broker.sh --agent {{BRANCH_ID}} status --activity"),
        "the activity example should use `broker.sh status --activity`"
    );
    let regions = section(&skill, "### Declaring regions");
    assert!(
        regions.contains("broker.sh --agent {{BRANCH_ID}} intent")
            && regions.contains("--region \"src/auth/<file>#function=validate_token\""),
        "the region example should use `broker.sh intent --region`"
    );
}

/// Scenario: Coordination skill retains polling reference. The cursor the
/// skill tells agents to reuse is the helper's `last_seq` line.
#[test]
fn polling_reference_uses_the_helper_cursor() {
    let skill = coordination_skill();
    assert!(skill.contains(".git-paw/scripts/broker.sh --agent {{BRANCH_ID}} poll"));
    assert!(!skill.contains("{{GIT_PAW_BROKER_URL}}/messages/"));
    assert!(
        skill.contains("`last_seq: <n>`")
            && skill.contains("broker.sh --agent {{BRANCH_ID}} poll <last_seq>"),
        "the skill should show reusing the poll's last_seq line as the since cursor"
    );
}

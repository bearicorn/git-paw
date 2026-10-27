//! Skill-content assertions for `classifier-retirement`, supervisor side.
//!
//! These are WHEN-the-skill-is-rendered / THEN-the-prose-SHALL-state
//! requirements pinning the permission ladder into
//! `assets/agent-skills/orchestrator.md` so a future edit cannot silently
//! re-introduce a git-paw command classifier: the agent CLI's own permission
//! system decides first, anything it still prompts for is relayed to the
//! human, and `sweep.sh approve` runs only on the human's explicit
//! instruction — answering once, never granting a durable "don't ask again".
//!
//! Covers `skill-agent` / *Supervisor skill — permission decisions belong to
//! the agent CLI and the human*, `core-memory-isolation` / *Supervisor treats
//! out-of-worktree writes as violations*, and `approval-command-safety` /
//! *Prompt-free operation is attributed to the agent CLI's permission
//! system*.

use std::fs;

fn supervisor_skill() -> String {
    fs::read_to_string("assets/agent-skills/orchestrator.md")
        .expect("orchestrator.md is at the expected path")
}

/// Extract the section starting at `heading` and running until the next
/// top-level (`### `) heading, lower-cased with whitespace collapsed so
/// substring scans are robust to line wrapping.
fn section_after(skill: &str, heading: &str) -> String {
    let start = skill
        .find(heading)
        .unwrap_or_else(|| panic!("skill has a `{heading}` section"));
    let rest = &skill[start..];
    let end = rest[1..].find("\n### ").map_or(rest.len(), |idx| idx + 1);
    rest[..end]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn permission_section() -> String {
    section_after(&supervisor_skill(), "### Who decides a permission prompt")
}

/// Whole skill, whitespace-collapsed and lower-cased.
fn whole_skill() -> String {
    supervisor_skill()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

// Requirement: Supervisor skill — permission decisions belong to the agent CLI
// and the human / Scenario: The supervisor skill documents the permission
// ladder.
#[test]
fn permission_section_documents_the_ladder_in_order() {
    let section = permission_section();

    let cli_idx = section
        .find("permission system")
        .expect("section must name the worker's own CLI permission system");
    let relay_idx = section
        .find("relayed to the human")
        .expect("section must say the prompt is relayed to the human");
    let approve_idx = section
        .find("sweep.sh approve")
        .expect("section must name the explicit `sweep.sh approve` action");

    assert!(
        cli_idx < relay_idx,
        "the CLI's own permission system must be described before the relay tier; section:\n{section}"
    );
    assert!(
        relay_idx < approve_idx,
        "the relay tier must be described before the explicit approve; section:\n{section}"
    );
    assert!(
        section.contains("only when the human") || section.contains("explicitly tells you"),
        "the approve tier must be conditioned on the human's explicit instruction; section:\n{section}"
    );
}

// Requirement: Supervisor skill — permission decisions belong to the agent CLI
// and the human / Scenario: The supervisor skill documents the permission
// ladder (second THEN: approve answers once, never "don't ask again").
#[test]
fn permission_section_states_approve_answers_once_and_never_durably() {
    let section = permission_section();
    assert!(
        section.contains("once"),
        "section must state `sweep.sh approve` answers the prompt once; section:\n{section}"
    );
    assert!(
        section.contains("never grants a durable \"don't ask again\""),
        "section must state the approve never grants a durable \"don't ask again\"; section:\n{section}"
    );
}

/// The ladder's closing rule: a prompt that should stop recurring is the
/// user's job to configure in the agent CLI's own allow rules — never a
/// git-paw allowlist.
#[test]
fn permission_section_points_recurring_prompts_at_the_cli_allow_rules() {
    let section = permission_section();
    assert!(
        section.contains("allow rules"),
        "section must point a recurring prompt at the agent CLI's own allow rules; section:\n{section}"
    );
    assert!(
        section.contains("never approve on your own"),
        "section must forbid approving on the supervisor's own reading of the command; section:\n{section}"
    );
}

// Requirement: Supervisor skill — permission decisions belong to the agent CLI
// and the human / Scenario: The supervisor skill names no command as safe.
#[test]
fn supervisor_skill_names_no_command_as_safe() {
    let skill = whole_skill();

    assert!(
        !skill.contains("__classify"),
        "the skill must not reference the retired `git paw __classify` classifier"
    );
    assert!(
        !skill.contains("{{dev_allowlist_preset}}"),
        "the skill must not carry the dev-allowlist placeholder"
    );
    assert!(
        !skill.contains("absorb routine approval") && !skill.contains("rubber-stamp"),
        "the skill must not instruct the supervisor to absorb routine approvals"
    );
    for forbidden in [
        "safe-command policy",
        "safe-by-pattern",
        "classifier-safe",
        "danger-list",
        "auto-approve whitelist",
    ] {
        assert!(
            !skill.contains(forbidden),
            "the skill must not describe a git-paw safety classification; found `{forbidden}`"
        );
    }
}

/// Second THEN of *The supervisor skill names no command as safe*: the skill
/// must never tell the supervisor to SELECT a durable-grant option. The phrase
/// may appear only in the negative ("never grants a durable …").
#[test]
fn supervisor_skill_never_instructs_selecting_a_durable_grant() {
    let skill = whole_skill();
    for instruction in [
        "select **\"yes, and don't ask again\"**",
        "select \"yes, and don't ask again\"",
        "choose \"yes, and don't ask again\"",
        "yes, and don't ask again\" so the pattern is",
    ] {
        assert!(
            !skill.contains(instruction),
            "the skill must not instruct selecting a durable grant; found `{instruction}`"
        );
    }
    // Every surviving mention is the negative framing.
    let mentions = skill.matches("don't ask again").count();
    let negated = skill
        .matches("never grants a durable \"don't ask again\"")
        .count()
        + skill.matches("would itself be a durable grant").count();
    assert!(
        mentions <= negated,
        "every \"don't ask again\" mention must be the negative framing; {mentions} mentions vs {negated} negations"
    );
}

// Requirement (core-memory-isolation): Supervisor treats out-of-worktree
// writes as violations / Scenario: Supervisor skill carries the violation
// procedure.
#[test]
fn out_of_worktree_violation_procedure_survives_without_classifier_wording() {
    let section = section_after(&supervisor_skill(), "### Out-of-worktree write violations");

    assert!(
        section.contains("agent.feedback"),
        "the violation procedure must still send scoped agent.feedback; section:\n{section}"
    );
    assert!(
        section.contains("worktree boundary"),
        "the scoped feedback must name the worktree boundary; section:\n{section}"
    );
    assert!(
        section.contains("escalate"),
        "the procedure must escalate on a repeat attempt; section:\n{section}"
    );
    assert!(
        section.contains("relay"),
        "the procedure must relay a pending prompt to the operator rather than approving it; section:\n{section}"
    );
    assert!(
        section.contains("never approve"),
        "the procedure must still forbid approving the prompt; section:\n{section}"
    );

    for classifier_wording in [
        "classifier",
        "danger-class",
        "protected-path rule",
        "danger class",
    ] {
        assert!(
            !section.contains(classifier_wording),
            "the violation must not be described as detected by a git-paw classifier; found `{classifier_wording}` in:\n{section}"
        );
    }
}

// Requirement (approval-command-safety): Prompt-free operation is attributed
// to the agent CLI's permission system / Scenario: Supervisor guidance credits
// the CLI's permission system.
#[test]
fn supervisor_guidance_credits_the_cli_permission_system() {
    let section = permission_section();

    assert!(
        section.contains("resolved permission mode"),
        "prompt-free operation must be credited to the CLI's resolved permission mode; section:\n{section}"
    );
    assert!(
        section.contains("broker.sh"),
        "the section must explain the bundled helper's prompt-free path; section:\n{section}"
    );
    assert!(
        section.contains("seeds no allow rule") || section.contains("adds no rules of its own"),
        "the section must state git-paw contributes no permission rule; section:\n{section}"
    );
    assert!(
        !section.contains("recognises the bundled"),
        "prompt-free operation must not be credited to a git-paw path-recognition rule; section:\n{section}"
    );
}

/// The ladder must stay stack-agnostic: no consumer toolchain verb is named
/// as approvable anywhere in it.
#[test]
fn permission_section_is_stack_agnostic() {
    let section = permission_section();
    for forbidden in [
        "cargo",
        "rustdoc",
        ".rs:",
        "cargo.toml",
        "rustc",
        "npm test",
        "pytest",
    ] {
        assert!(
            !section.contains(forbidden),
            "the permission ladder must not hard-code a stack toolchain verb; found `{forbidden}`"
        );
    }
}

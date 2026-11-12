//! The budget step exists only as prose. This guard stops edits that drop a
//! rule or name a vendor.

use git_paw::agent_cli::CliFamily;
use git_paw::skills::{GateCommands, render};
use git_paw::specs::SpecBackendKind;

/// Resolves `name` from the embedded defaults only (no override root), so a
/// developer's own overrides never change what this test sees.
fn resolve(name: &str) -> Result<git_paw::skills::SkillTemplate, git_paw::skills::SkillError> {
    git_paw::skills::resolve_from(name, &git_paw::skills::SkillRoots::default())
}

/// The step's last rule: its presence also proves the section was not cut
/// short before the vendor scan.
const NEVER_KILL_RULE: &str = "do not interrupt, signal or kill a worker";

/// Must not appear in the budget step, which keeps the skill vendor-neutral.
const VENDOR_TOKENS: &[&str] = &[
    "claude",
    "anthropic",
    "codex",
    "openai",
    "gemini",
    "google",
    "copilot",
    "cursor",
    "aider",
    "opus",
    "sonnet",
    "haiku",
    "gpt",
];

/// The rendered budget section, whitespace-collapsed and lower-cased so
/// substring checks survive rewrapping.
fn budget_section(backends: &[SpecBackendKind]) -> String {
    let tmpl = resolve("supervisor").expect("supervisor skill resolves");
    let skill = render(
        &tmpl,
        "supervisor",
        "http://127.0.0.1:9119",
        "git-paw",
        &GateCommands::default(),
        backends,
        CliFamily::Generic,
    );
    let start = skill
        .find("### Budget guardrail")
        .expect("the supervisor skill carries a budget step");
    let rest = &skill[start..];
    let end = ["\n### ", "\n## ", "\n# "]
        .iter()
        .filter_map(|h| rest[1..].find(h))
        .min()
        .map_or(rest.len(), |idx| idx + 1);
    rest[..end]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Scenario: "The rendered skill carries the budget step".
#[test]
fn the_rendered_skill_carries_the_budget_step() {
    for backend in SpecBackendKind::ALL {
        let section = budget_section(&[backend]);
        let rules = [
            (
                "usage from `git paw status --json`",
                "reads usage from status --json",
            ),
            (
                "on a breach, stop dispatching",
                "stops dispatching on a breach",
            ),
            (
                "report the totals and the cap to the operator",
                "reports the breach to the operator",
            ),
            (
                "compare `usage_total.tokens` plus your running total of removed workers",
                "compares the capped token figure",
            ),
            (
                "`usage_total` covers only the workers currently in the session",
                "says the session total drops removed workers",
            ),
            (
                "before `git paw remove`, note that worker's `usage`",
                "keeps a removed worker's spend in a running total",
            ),
            ("a `usage` of `null`", "names null usage"),
            (
                "never count unknown usage as zero",
                "treats unknown usage as not zero",
            ),
            (NEVER_KILL_RULE, "never stops a worker for the budget"),
        ];
        for (phrase, rule) in rules {
            assert!(
                section.contains(phrase),
                "{backend:?}: the budget step must state that it {rule} ({phrase:?})"
            );
        }
    }
}

/// Scenario: "The budget step is vendor-agnostic".
#[test]
fn the_budget_step_is_vendor_agnostic() {
    for backend in SpecBackendKind::ALL {
        let section = budget_section(&[backend]);
        assert!(
            section.contains(NEVER_KILL_RULE),
            "{backend:?}: the extracted section is truncated: {section}"
        );
        for token in VENDOR_TOKENS {
            let hit = section
                .split(|c: char| !c.is_ascii_alphanumeric())
                .any(|word| word == *token);
            assert!(
                !hit,
                "{backend:?}: budget step names the vendor/CLI/model token {token:?}"
            );
        }
    }
}

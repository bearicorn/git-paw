//! Docs-parity guard for the README landing page.
//!
//! The README links the docs, so each check pins both the README link and the
//! detail on the linked page. Checks use one README section where the
//! requirement names one, so a word in another section does not count.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// The README's `## <heading>` section, up to the next `## ` heading outside
/// a code fence.
fn readme_section(heading: &str) -> String {
    let readme = read("README.md");
    let lines: Vec<&str> = readme.lines().collect();
    let wanted = format!("## {heading}");
    let start = lines
        .iter()
        .position(|l| l.trim() == wanted)
        .unwrap_or_else(|| panic!("README.md must have a `{wanted}` section"));

    let mut out = vec![lines[start]];
    let mut in_fence = false;
    for line in &lines[start + 1..] {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence && line.starts_with("## ") {
            break;
        }
        out.push(line);
    }
    out.join("\n")
}

/// The Highlights section, with a guard against a truncated parse.
fn highlights_section() -> String {
    let section = readme_section("Highlights");
    assert!(
        section.len() > 500,
        "parsed Highlights section looks truncated ({} bytes) — the parser is wrong, \
         and every assertion below would pass or fail for the wrong reason",
        section.len()
    );
    section
}

/// Case-insensitive "contains all of these substrings"; returns the misses.
fn missing_from(haystack: &str, needles: &[&str]) -> Vec<String> {
    let lower = haystack.to_lowercase();
    needles
        .iter()
        .filter(|n| !lower.contains(&n.to_lowercase()))
        .map(|n| (*n).to_string())
        .collect()
}

fn assert_contains_all(label: &str, haystack: &str, needles: &[&str]) {
    let missing = missing_from(haystack, needles);
    assert!(missing.is_empty(), "{label}: missing {missing:?}");
}

// --- Requirement: README Highlights section enumerates the current
// user-facing surface ---

/// Scenario: README Features section mentions Spec Kit.
#[test]
fn highlights_mention_spec_kit_and_link_its_page() {
    assert_contains_all(
        "README Highlights",
        &highlights_section(),
        &["Spec Kit", "spec-driven-launch.md"],
    );
    assert_contains_all(
        "docs/src/user-guide/spec-driven-launch.md",
        &read("docs/src/user-guide/spec-driven-launch.md"),
        &["speckit"],
    );
}

/// Scenario: README Features section mentions forward coordination and
/// conflict detection.
#[test]
fn highlights_mention_conflict_detection() {
    assert_contains_all(
        "README Highlights",
        &highlights_section(),
        &["conflict detection"],
    );
    assert_contains_all(
        "README Highlights",
        &highlights_section(),
        &["conflict-detection.md"],
    );
    assert_contains_all(
        "docs/src/user-guide/conflict-detection.md",
        &read("docs/src/user-guide/conflict-detection.md"),
        &["agent.intent"],
    );
}

/// Scenario: Every Highlights bullet links a docs page.
#[test]
fn every_highlights_bullet_links_a_page() {
    let section = highlights_section();
    let bullets: Vec<&str> = section
        .lines()
        .filter(|l| l.trim_start().starts_with("- "))
        .collect();
    assert!(
        bullets.len() >= 5,
        "Highlights has too few bullets:\n{section}"
    );
    for bullet in bullets {
        assert!(
            bullet.contains("]("),
            "a Highlights bullet links no page: {bullet}"
        );
    }
}

/// Scenario: README Features section mentions learnings mode.
#[test]
fn highlights_mention_learnings_and_link_its_page() {
    assert_contains_all(
        "README Highlights",
        &highlights_section(),
        &["learnings", "learnings.md"],
    );
    assert_contains_all(
        "docs/src/user-guide/learnings.md",
        &read("docs/src/user-guide/learnings.md"),
        &[".git-paw/session-learnings.md"],
    );
}

/// Scenario: README Features section mentions every CLI flag.
#[test]
fn commands_link_the_cli_reference_which_names_every_flag() {
    assert_contains_all(
        "README Commands",
        &readme_section("Commands"),
        &["cli-reference.md"],
    );
    assert_contains_all(
        "docs/src/cli-reference.md",
        &read("docs/src/cli-reference.md"),
        &["--specs-format", "--no-supervisor", "start --force"],
    );
}

/// Scenario: README Features section does not advertise auto-approval.
///
/// git-paw approves no permission prompt. Offering automatic approval as a
/// feature would be the most consequential doc overclaim available: a reader
/// would leave an unattended run expecting prompts to clear themselves.
#[test]
fn highlights_do_not_advertise_auto_approval() {
    let lower = highlights_section().to_lowercase();
    assert!(
        !lower.contains("[supervisor.auto_approve]"),
        "the Highlights must not offer the retired auto-approval table as a feature"
    );
    for claim in [
        "auto-approv",
        "auto approv",
        "automatically approv",
        "approves safe",
        "approve safe",
    ] {
        assert!(
            !lower.contains(claim),
            "the Highlights must not describe git-paw as approving prompts ({claim:?})"
        );
    }
}

// --- Requirement: README is a short landing page that links the docs ---

const HEADING_ORDER: &[&str] = &[
    "What it does",
    "Demo",
    "Highlights",
    "Install",
    "Quick start",
    "Commands",
    "Configuration",
    "Supported CLIs",
    "Security",
    "Documentation",
    "Contributing",
    "License",
];

/// Scenario: The README fits the landing-page budget.
#[test]
fn readme_fits_the_line_budget_and_heading_order() {
    let readme = read("README.md");
    let lines = readme.lines().count();
    assert!(
        lines <= 130,
        "README.md has {lines} lines; the budget is 130"
    );

    let mut in_fence = false;
    let mut headings = Vec::new();
    for line in readme.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence && let Some(h) = line.strip_prefix("## ") {
            headings.push(h.trim().to_string());
        }
    }
    assert_eq!(
        headings, HEADING_ORDER,
        "README `## ` headings must appear in the landing-page order"
    );
    assert!(
        readme.contains("# git-paw"),
        "README must open with the git-paw title"
    );
}

/// Scenario: The quick start links the three quick-start pages.
#[test]
fn quick_start_links_pages_and_shows_commands() {
    assert_contains_all(
        "README Quick start",
        &readme_section("Quick start"),
        &[
            "quick-start-same-cli.md",
            "quick-start-per-branch.md",
            "quick-start-orchestrator.md",
            "git paw init",
            "git paw start --supervisor",
            "git paw start --unattended --from-all-specs",
        ],
    );
}

/// Scenario: The command table carries no version tags and links the reference.
#[test]
fn command_table_has_no_version_tags() {
    let section = readme_section("Commands");
    assert!(
        !section.contains("(v0."),
        "the Commands table must not carry `(v0.` version tags"
    );
    assert!(section.contains("cli-reference.md"));
}

/// Scenario: Superseded content is gone.
#[test]
fn superseded_content_is_gone() {
    let readme = read("README.md");
    assert!(!readme.contains("show_message_log"));
    assert!(
        !readme.lines().any(|l| l.trim() == "## Releases"),
        "README must not carry a `## Releases` section"
    );
    assert!(
        !readme.contains("../project-") && !readme.contains("../yourproject-"),
        "README must not show a worktree layout with agent worktrees only at `../`"
    );
}

// --- Requirement: README-only facts move into the docs before removal ---

/// Scenario: The CLAUDE.md import tip lives in the AGENTS.md guide.
#[test]
fn claude_md_import_tip_lives_in_the_agents_md_guide() {
    let page = read("docs/src/user-guide/agents-md.md");
    let mut lines = page.lines();
    let mut first_lines = Vec::new();
    while lines.by_ref().any(|l| l.trim().starts_with("```")) {
        if let Some(first) = lines.by_ref().find(|l| !l.trim().is_empty()) {
            first_lines.push(first.trim());
        }
        // Skip to the closing fence so it is not read as an opening one.
        lines.by_ref().find(|l| l.trim().starts_with("```"));
    }
    assert!(
        first_lines.contains(&"@AGENTS.md"),
        "agents-md.md must show `@AGENTS.md` as the first line of a CLAUDE.md"
    );
    assert_contains_all(
        "docs/src/user-guide/agents-md.md",
        &normalized(&page),
        &["claude.md", "symlink", "ln -s agents.md gemini.md"],
    );
}

/// Lowercased, backticks dropped, whitespace runs collapsed to one space, so
/// a reflowed line or a code span does not break a phrase match.
fn normalized(text: &str) -> String {
    text.replace('`', "")
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Scenario: start --force under supervisor mode is documented.
#[test]
fn start_force_no_effect_under_supervisor_is_documented() {
    for page in [
        "docs/src/cli-reference.md",
        "docs/src/user-guide/spec-driven-launch.md",
    ] {
        assert_contains_all(
            page,
            &normalized(&read(page)),
            &["start --force", "no effect under supervisor mode"],
        );
    }
}

// --- Requirement: README Security note states the trust model in brief ---

/// Scenario: The Security note names the trust model and links both pages.
#[test]
fn security_note_names_the_trust_model_and_links_both_pages() {
    assert_contains_all(
        "README Security",
        &normalized(&readme_section("Security")),
        &[
            "arbitrary code",
            "sandbox",
            "approves no",
            "security-posture.md",
            "sandbox.md",
        ],
    );
    let lines = readme_section("Security")
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .count();
    assert!(lines <= 6, "the Security note has {lines} non-blank lines");
}

// --- supervisor-launch: Orchestrator quick-start page documents the flags ---

/// Lines inside fenced code blocks that run `git paw`.
fn command_lines(page: &str) -> Vec<String> {
    let mut in_fence = false;
    let mut out = Vec::new();
    for line in page.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence && line.trim_start().starts_with("git paw") {
            out.push(line.trim().to_string());
        }
    }
    out
}

/// Scenario: Quick start supervisor mentions --no-supervisor.
#[test]
fn orchestrator_quick_start_shows_no_supervisor_in_an_example() {
    let page = read("docs/src/quick-start-orchestrator.md");
    assert!(
        command_lines(&page)
            .iter()
            .any(|l| l.contains("--no-supervisor")),
        "quick-start-orchestrator.md must show `--no-supervisor` in a command-line example"
    );
}

/// Scenario: Quick start supervisor mentions --force.
#[test]
fn orchestrator_quick_start_shows_start_force_and_its_no_effect_note() {
    let page = read("docs/src/quick-start-orchestrator.md");
    assert!(
        command_lines(&page)
            .iter()
            .any(|l| l.starts_with("git paw start") && l.contains("--force")),
        "quick-start-orchestrator.md must show `start --force` in a command-line example"
    );
    assert_contains_all(
        "docs/src/quick-start-orchestrator.md",
        &normalized(&page),
        &["--force", "no effect under supervisor mode"],
    );
}

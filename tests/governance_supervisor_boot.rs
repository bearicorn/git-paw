//! Supervisor boot-prompt assembly: governance section integration.
//!
//! Covers `governance-context` task §3.5 (boot-prompt builder integration):
//! the full boot prompt for a supervisor session with no `[governance]`
//! configuration matches the v0.4 baseline (no governance content), and the
//! same builder with at least one configured path inserts a `## Governance
//! documents` section between the supervisor skill content and any
//! subsequent task content.
//!
//! These tests assemble the boot prompt exactly the way `cmd_supervisor` in
//! `src/main.rs` does it: resolve and render the supervisor skill, then
//! concatenate the output of [`git_paw::skills::governance_section_paths`]
//! with a blank-line separator when it is non-empty. Calling the path-based
//! helper directly avoids the dependency on the `GovernanceConfig` type
//! (owned by the parallel `feat/governance-config` change) while still
//! exercising the same renderer that the public
//! `governance_section(&GovernanceConfig)` wrapper will delegate to.

use std::path::Path;

use git_paw::skills::{governance_section_paths, render, resolve};

/// Assemble the supervisor pane's full boot-prompt body exactly like
/// `cmd_supervisor` does: rendered skill + (optional) governance section
/// separated by a blank line.
fn assemble_supervisor_boot_prompt(
    adr: Option<&Path>,
    test_strategy: Option<&Path>,
    security: Option<&Path>,
    dod: Option<&Path>,
    constitution: Option<&Path>,
) -> String {
    let tmpl = resolve("supervisor").expect("supervisor skill resolves");
    let skill = render(
        &tmpl,
        "supervisor",
        "http://127.0.0.1:9119",
        "project",
        &git_paw::skills::GateCommands {
            test_command: Some("just check"),
            ..Default::default()
        },
        &[],
    );
    let governance = governance_section_paths(adr, test_strategy, security, dod, constitution);
    if governance.is_empty() {
        skill
    } else {
        let mut out = String::with_capacity(skill.len() + governance.len() + 1);
        out.push_str(&skill);
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&governance);
        out
    }
}

#[test]
fn supervisor_boot_prompt_with_no_governance_paths_omits_section() {
    let prompt = assemble_supervisor_boot_prompt(None, None, None, None, None);

    let baseline = {
        let tmpl = resolve("supervisor").expect("supervisor skill resolves");
        render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "project",
            &git_paw::skills::GateCommands {
                test_command: Some("just check"),
                ..Default::default()
            },
            &[],
        )
    };

    assert_eq!(
        prompt, baseline,
        "boot prompt with empty governance config must match the v0.4 baseline byte-for-byte"
    );
    // The supervisor skill mentions `## Governance documents` *inline* (inside
    // backticks) when describing what to look for in the boot prompt. The
    // injected section uses the heading at the start of a line (Markdown
    // heading delimiter), so a `\n## Governance documents` substring check
    // distinguishes "section actually inserted" from "skill prose references
    // the heading by name".
    assert!(
        !prompt.contains("\n## Governance documents\n"),
        "boot prompt must not contain the governance heading at start-of-line when no paths are configured"
    );
}

#[test]
fn supervisor_boot_prompt_with_dod_inserts_section_after_skill() {
    let dod = Path::new("docs/dod.md");
    let prompt = assemble_supervisor_boot_prompt(None, None, None, Some(dod), None);

    assert!(
        prompt.contains("\n## Governance documents\n"),
        "boot prompt must contain the start-of-line governance heading when at least one path is set"
    );
    assert!(
        prompt.contains("- dod: docs/dod.md"),
        "boot prompt must include the dod bullet, got:\n{prompt}"
    );

    let skill_heading_pos = prompt
        .find("\n## Supervisor Skills")
        .expect("Supervisor Skills heading must exist in rendered supervisor skill");
    let governance_pos = prompt
        .find("\n## Governance documents\n")
        .expect("Governance documents heading must exist when a path is configured");
    assert!(
        governance_pos > skill_heading_pos,
        "## Governance documents must come AFTER ## Supervisor Skills, got skill@{skill_heading_pos} governance@{governance_pos}"
    );
}

#[test]
fn supervisor_boot_prompt_with_all_paths_lists_them_in_canonical_order() {
    let prompt = assemble_supervisor_boot_prompt(
        Some(Path::new("docs/adr/")),
        Some(Path::new("docs/test-strategy.md")),
        Some(Path::new("docs/security.md")),
        Some(Path::new("docs/dod.md")),
        Some(Path::new("docs/constitution.md")),
    );

    let order = [
        ("- adr: docs/adr/", "adr"),
        ("- test_strategy: docs/test-strategy.md", "test_strategy"),
        ("- security: docs/security.md", "security"),
        ("- dod: docs/dod.md", "dod"),
        ("- constitution: docs/constitution.md", "constitution"),
    ];
    let mut last = 0usize;
    for (bullet, label) in order {
        let idx = prompt
            .find(bullet)
            .unwrap_or_else(|| panic!("bullet `{label}` missing from boot prompt"));
        assert!(
            idx > last,
            "bullet `{label}` appeared before a previous bullet in the boot prompt"
        );
        last = idx;
    }
}

#[test]
fn supervisor_boot_prompt_governance_section_has_no_gates_text() {
    let prompt = assemble_supervisor_boot_prompt(
        Some(Path::new("docs/adr/")),
        None,
        None,
        Some(Path::new("docs/dod.md")),
        None,
    );

    let gov_pos = prompt
        .find("\n## Governance documents\n")
        .expect("governance heading must exist");
    let section = &prompt[gov_pos..];
    let lowered = section.to_lowercase();
    assert!(
        !lowered.contains("gated docs"),
        "governance section must not include a 'Gated docs' line, got:\n{section}"
    );
    assert!(
        !lowered.contains("governance gates"),
        "governance section must not include a 'Governance gates' sub-section, got:\n{section}"
    );
    assert!(
        !section.contains("[governance.gates]"),
        "governance section must not reference the dropped [governance.gates] table"
    );
    assert!(
        !section.contains("[governance-gate:"),
        "governance section must not introduce the dropped [governance-gate:<doc>] tag prefix"
    );
}

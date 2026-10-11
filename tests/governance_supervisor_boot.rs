//! Supervisor boot-prompt assembly: governance section integration.
//!
//! Covers `governance-context` task §3.5 (boot-prompt builder integration):
//! the full boot prompt for a supervisor session with no `[governance]`
//! configuration matches the v0.4 baseline (no governance content), and the
//! same builder with at least one configured path inserts a `## Governance
//! documents` section between the supervisor skill content and any
//! subsequent task content.
//!
//! These tests assemble the boot prompt through the very function the
//! supervisor launch calls — [`git_paw::skills::supervisor_boot_context`] —
//! rather than re-implementing the concatenation locally. A hand-written
//! copy of the assembly is exactly the defect `supervisor-boot-wiring`
//! fixes: it passed while the production launch path never rendered the
//! section at all.

use std::path::{Path, PathBuf};

use git_paw::config::GovernanceConfig;
use git_paw::skills::{render, resolve, supervisor_boot_context};

/// Assemble the supervisor pane's full boot-prompt body the way
/// `cmd_supervisor` does: the rendered skill handed to
/// [`supervisor_boot_context`] together with the governance config.
fn assemble_supervisor_boot_prompt(
    adr: Option<&Path>,
    test_strategy: Option<&Path>,
    security: Option<&Path>,
    dod: Option<&Path>,
    constitution: Option<&Path>,
) -> String {
    let skill = render_supervisor_skill();
    let governance = GovernanceConfig {
        adr: adr.map(PathBuf::from),
        test_strategy: test_strategy.map(PathBuf::from),
        security: security.map(PathBuf::from),
        dod: dod.map(PathBuf::from),
        constitution: constitution.map(PathBuf::from),
        ..Default::default()
    };
    supervisor_boot_context(&skill, false, &governance)
}

/// The rendered supervisor skill, before any governance assembly.
fn render_supervisor_skill() -> String {
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
}

#[test]
fn supervisor_boot_prompt_with_no_governance_paths_omits_section() {
    let prompt = assemble_supervisor_boot_prompt(None, None, None, None, None);

    let baseline = git_paw::skills::with_drive_loop_directive(&render_supervisor_skill(), false);

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

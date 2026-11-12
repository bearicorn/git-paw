//! `skill-packages` delegate role packages: `explorer`, `spec-auditor`,
//! `doc-auditor` and `security-auditor` (capability `cli-agent-profiles`).

use std::process::Command as StdCommand;

use assert_cmd::Command;
use git_paw::agent_cli::CliFamily;
use git_paw::skills::{
    GateCommands, ORCHESTRATOR_SKILL, ProvisionGate, SkillRoots, WorktreeInputs, bundled_package,
    packages_for_worktree, render, resolve_from,
};
use tempfile::TempDir;

const ROLES: [&str; 4] = [
    "explorer",
    "spec-auditor",
    "doc-auditor",
    "security-auditor",
];
const AUDITORS: [(&str, &str); 3] = [
    ("spec-auditor", "spec-audit"),
    ("doc-auditor", "doc-audit"),
    ("security-auditor", "security-audit"),
];

/// Renders the embedded role package `name` with `gates`.
fn rendered(name: &str, gates: &GateCommands<'_>) -> String {
    let template = resolve_from(name, &SkillRoots::default()).expect("role resolves");
    render(
        &template,
        "supervisor",
        "",
        "project",
        gates,
        &[],
        CliFamily::Generic,
    )
}

/// The orchestrator package's raw reference `references/gates/<gate>.md`.
fn gate_source(gate: &str) -> &'static str {
    let path = format!("references/gates/{gate}.md");
    bundled_package(ORCHESTRATOR_SKILL)
        .expect("orchestrator")
        .references
        .iter()
        .find(|r| r.path == path)
        .unwrap_or_else(|| panic!("no {path}"))
        .content
}

/// Scenario "Auditors render the orchestrator's gate references".
#[test]
fn auditors_render_the_orchestrators_gate_references() {
    let gates = GateCommands {
        spec_validate_command: Some("validate-specs --strict"),
        ..GateCommands::default()
    };
    let spec = rendered("spec-auditor", &gates);
    assert!(spec.contains("# Spec audit gate"), "{spec}");
    assert!(spec.contains("validate-specs --strict"), "{spec}");
    assert!(!spec.contains("{{GATE_REFERENCE"), "{spec}");
    for (role, gate) in AUDITORS {
        let text = rendered(role, &GateCommands::default());
        let heading = gate_source(gate).lines().next().expect("heading");
        assert!(text.contains(heading), "{role} includes {gate}");
    }
}

/// Scenario "Each gate procedure has one source": no auditor package file
/// carries a sentence of its gate reference's source text.
#[test]
fn each_gate_procedure_has_one_source() {
    for (role, gate) in AUDITORS {
        let file = bundled_package(role).expect("registered").skill_md.content;
        let collapsed = file.split_whitespace().collect::<Vec<_>>().join(" ");
        let source = gate_source(gate)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        for sentence in source.split(". ").filter(|s| s.len() >= 40) {
            assert!(
                !collapsed.contains(sentence),
                "{role} copies a sentence of {gate}: {sentence}"
            );
        }
        assert!(file.contains(&format!("{{{{GATE_REFERENCE:{gate}}}}}")));
    }
}

/// Scenario "Auditors define the verdict and never publish".
#[test]
fn auditors_define_the_verdict_and_never_publish() {
    for (role, _) in AUDITORS {
        let text = rendered(role, &GateCommands::default());
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains("the gate name, `pass` or `fail`, and one prefixed line per finding"),
            "{role}"
        );
        assert!(
            flat.contains("You never publish `agent.verified` or `agent.feedback`"),
            "{role}"
        );
    }
}

/// Scenario "Explorer returns a map".
#[test]
fn explorer_returns_a_map() {
    let text = rendered("explorer", &GateCommands::default());
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(flat.contains("`<path> — <one-line role>`"), "{text}");
    assert!(flat.contains("Do not return file contents"), "{text}");
}

/// Scenario "Delegate role packages never reach a worktree".
#[test]
fn delegate_role_packages_never_reach_a_worktree() {
    for broker_enabled in [false, true] {
        for docs_base_url_configured in [false, true] {
            let selected = packages_for_worktree(WorktreeInputs {
                broker_enabled,
                docs_base_url_configured,
            });
            for role in ROLES {
                assert!(!selected.iter().any(|p| p.name == role), "{role} selected");
            }
        }
    }
    for role in ROLES {
        assert_eq!(
            bundled_package(role).expect("registered").gate,
            ProvisionGate::RootOnly
        );
    }
}

/// Scenario "Delegate role packages are exported at the root".
#[test]
fn delegate_role_packages_are_exported_at_the_root() {
    let repo = TempDir::new().expect("repo");
    let home = TempDir::new().expect("home");
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["config", "user.email", "t@t"],
        &["config", "user.name", "T"],
        &["commit", "-q", "--allow-empty", "-m", "i"],
    ] {
        StdCommand::new("git")
            .current_dir(repo.path())
            .args(args)
            .output()
            .expect("git");
    }
    Command::cargo_bin("git-paw")
        .expect("binary")
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_DATA_HOME")
        .arg("init")
        .assert()
        .success();
    for role in ROLES {
        let path = repo.path().join(format!(".git-paw/skills/{role}/SKILL.md"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{role}: {e}"));
        assert!(
            !text.contains("{{GATE_REFERENCE"),
            "{role} export is rendered"
        );
    }
}

/// Scenario "The role packages are vendor-neutral" (registry-driven).
#[test]
fn the_role_packages_are_vendor_neutral() {
    const FORBIDDEN: &[&str] = &[
        "cargo",
        "rustdoc",
        ".rs:",
        "Cargo.toml",
        "rustc",
        "Claude Code",
        ".claude/",
        "Codex",
        "Gemini",
        "opus",
        "sonnet",
        "haiku",
        "gpt-",
    ];
    for role in ROLES {
        // The role's own prose: the gate procedures it includes are the
        // orchestrator's, audited with that package.
        let own = bundled_package(role).expect("registered").skill_md.content;
        for token in FORBIDDEN {
            assert!(!own.contains(token), "{role} names `{token}`");
        }
    }
}

//! standards-skill-integration: the exported supervisor + coordination skills direct the
//! supervisor (at the review gate) and the worker (during implementation) to consult the
//! project's `.agents/skills/` standards skills — agnostically (deferring to the project's own
//! skills / AGENTS.md, imposing no git-paw-specific standard). Full no-stack-token coverage of the
//! added prose is provided by `lang_agnostic_skill_audit.rs`, which scans the whole rendered skill.

const SUPERVISOR: &str = include_str!("../assets/agent-skills/supervisor.md");
const COORDINATION: &str = include_str!("../assets/agent-skills/coordination.md");

#[test]
fn supervisor_review_gate_consults_project_standards_skills() {
    assert!(
        SUPERVISOR.contains("Consult the project's standards at the review gate"),
        "supervisor skill must carry a review-gate step to consult the project's standards skills"
    );
    assert!(
        SUPERVISOR.contains("`.agents/skills/`"),
        "the supervisor consult step must reference the project's `.agents/skills/` location"
    );
}

#[test]
fn coordination_directs_worker_to_consult_project_standards_skills() {
    assert!(
        COORDINATION.contains("consult the project's standards skills under\n`.agents/skills/`")
            || COORDINATION
                .contains("consult the project's standards skills under `.agents/skills/`"),
        "coordination skill must direct the worker to consult the project's `.agents/skills/` standards"
    );
}

#[test]
fn consult_steps_are_agnostic_no_op_when_absent() {
    // Agnostic by construction: both steps say "if the project provides them" / "if present",
    // so a project shipping no `.agents/skills/` standards is unaffected.
    assert!(
        SUPERVISOR.contains("If the project ships no such skills"),
        "supervisor consult step must be a no-op when the project ships no standards skills"
    );
    assert!(
        COORDINATION.contains("if the project provides them"),
        "coordination consult step must be conditional on the project providing standards skills"
    );
}

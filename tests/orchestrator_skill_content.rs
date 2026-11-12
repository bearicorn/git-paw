//! Skill-content assertions for the `orchestrator-skill-export` conversion of
//! the bundled supervisor skill into the orchestrator skill
//! (`assets/agent-skills/orchestrator/SKILL.md`).
//!
//! Each test pins a WHEN-the-skill-is-read / THEN-the-prose-SHALL-state
//! scenario from the change's `skill-agent`, `supervisor-skill-discipline`
//! and `broker-watcher-and-state` deltas: no pane-bound self-reference, no
//! session-specific value, no `curl`, the event-surface monitoring, the
//! roster read when needed, the unattended merge rule, and the re-addressed
//! permission doctrine.

use git_paw::skills::{GateCommands, render_for_supervisor};
use git_paw::specs::SpecBackendKind;

/// Resolves `name` from the embedded defaults only (no override root), so a
/// developer's own overrides never change what this test sees.
fn resolve(name: &str) -> Result<git_paw::skills::SkillTemplate, git_paw::skills::SkillError> {
    git_paw::skills::resolve_from(name, &git_paw::skills::SkillRoots::default())
        .map(git_paw::skills::with_package_references)
}

/// The raw orchestrator template, frontmatter included, with the helper
/// placeholders rendered to their package paths as every renderer does — a
/// requirement naming a helper is checked against the path the orchestrator
/// is told to run (`skill-packages` *Requirements naming the legacy deployed
/// helper path are satisfied by the package path*).
fn template() -> String {
    git_paw::skills::bundled_package("orchestrator")
        .map(|p| git_paw::skills::substitute_helper_paths(&git_paw::skills::package_text(p)))
        .expect("orchestrator.md is at the expected path")
}

/// The orchestrator skill rendered for `backends`, with a broker URL and a
/// project name supplied so a leftover placeholder would surface as a value.
fn rendered_for(backends: &[SpecBackendKind]) -> String {
    let tmpl = resolve("orchestrator").expect("orchestrator resolves");
    render_for_supervisor(
        &tmpl,
        "http://127.0.0.1:9119",
        "my-project",
        &GateCommands::default(),
        backends,
    )
}

/// Every rendering the skill can take: each spec backend and none.
fn every_rendering() -> Vec<(String, String)> {
    let mut out = vec![("no backend".to_string(), rendered_for(&[]))];
    for kind in [
        SpecBackendKind::OpenSpec,
        SpecBackendKind::SpecKit,
        SpecBackendKind::Markdown,
        SpecBackendKind::Superpowers,
    ] {
        out.push((format!("{kind:?}"), rendered_for(&[kind])));
    }
    out
}

/// The `### ` (or `## `) section opened by `heading`, whitespace-collapsed and
/// lower-cased so substring checks survive line wrapping.
fn section(heading: &str) -> String {
    let skill = template();
    let start = skill
        .find(heading)
        .unwrap_or_else(|| panic!("orchestrator.md has a `{heading}` section"));
    let rest = &skill[start..];
    let end = rest[1..].find("\n### ").map_or(rest.len(), |idx| idx + 1);
    collapse(&rest[..end])
}

fn collapse(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

// skill-agent / Scenario: Skill carries no pane-bound self-reference
#[test]
fn skill_carries_no_pane_bound_self_reference() {
    for (label, text) in every_rendering() {
        let lower = text.to_lowercase();
        assert!(!lower.contains("your pane"), "{label}: `your pane` present");
        assert!(!lower.contains("own pane"), "{label}: `own pane` present");
    }
    // Every mention of pane 0 identifies it as the dashboard, within the
    // same line or the next.
    let skill = template();
    let lines: Vec<&str> = skill.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.to_lowercase().contains("pane 0") {
            let window = format!("{line} {}", lines.get(i + 1).unwrap_or(&""));
            assert!(
                window.contains("dashboard"),
                "line {} mentions pane 0 without identifying it as the dashboard: {line}",
                i + 1
            );
        }
    }
}

// skill-agent / Scenario: Skill carries no session-specific value
#[test]
fn skill_carries_no_session_specific_value() {
    let raw = template();
    assert!(!raw.contains("{{GIT_PAW_BROKER_URL}}"));
    assert!(!raw.contains("paw-{{PROJECT_NAME}}"));
    for (label, text) in every_rendering() {
        assert!(!text.contains("http://"), "{label}: a broker URL leaked");
        assert!(
            !text.contains("paw-my-project"),
            "{label}: the session name leaked"
        );
    }
    assert!(
        raw.contains(".git-paw/AGENTS.local.md"),
        "the skill must name the sidecar as where the session values live"
    );
    let intro = collapse(&raw[..raw.find("### Bootstrap").expect("bootstrap section")]);
    assert!(intro.contains("session name") && intro.contains("broker url"));
}

// skill-agent / Scenario: Verdicts route through the helper
#[test]
fn verdicts_route_through_the_helper() {
    let skill = template();
    for verb in [
        ".git-paw/skills/orchestrator/scripts/sweep.sh verified",
        ".git-paw/skills/orchestrator/scripts/sweep.sh feedback-gate",
        "sweep.sh conflict-ack",
    ] {
        assert!(skill.contains(verb), "verdict path must name `{verb}`");
    }
    let compact: String = skill.chars().filter(|c| !c.is_whitespace()).collect();
    for shape in [
        "curl-s-XPOST",
        r#"{"type":"agent.verified""#,
        r#"{"type":"agent.feedback""#,
    ] {
        assert!(
            !compact.contains(shape),
            "no raw publish of a verdict: found `{shape}`"
        );
    }
}

// skill-agent / Scenario: The rendered orchestrator skill contains no curl
#[test]
fn no_rendering_contains_curl() {
    for (label, text) in every_rendering() {
        assert!(
            !text.to_lowercase().contains("curl"),
            "{label}: the rendered orchestrator skill must not contain `curl`"
        );
    }
}

// skill-agent / *Supervisor skill SHALL reference the bundled sweep helper*:
// the minimum taught subcommand set.
#[test]
fn skill_teaches_the_helper_subcommands() {
    let skill = template();
    for verb in [
        "snapshot",
        "capture",
        "approve",
        "status",
        "worktrees-status",
        "inbox",
        "events",
        "feedback-gate",
        "verified",
        "conflict-ack",
        "status-publish",
        "answer",
        "advanced-main",
    ] {
        assert!(
            skill.contains(&format!(
                ".git-paw/skills/orchestrator/scripts/sweep.sh {verb}"
            )) || skill.contains(&format!("`sweep.sh {verb}")),
            "the skill must teach `sweep.sh {verb}`"
        );
    }
}

// skill-agent / Scenario: A stalled worker is told to use its own helper
#[test]
fn stalled_worker_is_told_to_use_its_own_helper() {
    let skill = template();
    let stall = skill
        .find("You appear stalled")
        .expect("stall message present");
    let line_end = skill[stall..].find('\n').map_or(skill.len(), |i| stall + i);
    let message = &skill[stall..line_end];
    assert!(message.contains(".git-paw/skills/coordination/scripts/broker.sh blocked"));
    assert!(!message.contains("http") && !message.contains("/publish"));
}

// skill-agent / Scenario: Skill teaches the cursor read
#[test]
fn skill_teaches_the_cursor_read() {
    let s = section("### Follow the session through the event surface");
    assert!(s.contains("sweep.sh events") && s.contains("wait_for_events"));
    assert!(s.contains("carry `last_seq` forward"));
    assert!(
        s.contains("a sampled capture can miss a prompt"),
        "the section must give the sampled-capture rationale"
    );
    let watch = collapse(&template());
    assert!(
        !watch.contains("every ~30 seconds"),
        "no fixed-interval polling as the primary detector"
    );
}

// skill-agent / Scenario: Skill acts on a stalled wave
#[test]
fn skill_acts_on_a_stalled_wave() {
    let s = section("### Follow the session through the event surface");
    assert!(s.contains("stalled") && s.contains("investigate"));
}

// skill-agent / Scenario: Skill relays escalated prompts read from events
#[test]
fn skill_relays_escalated_prompts_read_from_events() {
    let s = section("### Follow the session through the event surface");
    assert!(s.contains("escalates every live permission prompt to the broker and approves none"));
    assert!(s.contains("relay each escalated prompt to the human"));
    assert!(s.contains("sweep.sh approve <pane>` only on the human's explicit instruction"));
}

// skill-agent / Scenario: Skill teaches re-attaching the drive loop after a
// heartbeat exit
#[test]
fn skill_teaches_reattaching_the_drive_loop() {
    let s = section("### Follow the session through the event surface");
    assert!(s.contains("heartbeat"));
    assert!(s.contains("re-run `git paw start --unattended`"));
    assert!(s.contains("re-attaches a drive loop to the live session"));
}

// skill-agent / Scenario: Unattended escalations are read from the broker,
// not a pane
#[test]
fn unattended_escalations_are_read_from_the_broker() {
    let s = section("### Who decides a permission prompt");
    assert!(s.contains("the drive loop escalates it to the broker"));
    let whole = collapse(&template());
    assert!(!whole.contains("supervisor's pane"));
    assert!(!whole.contains("hands it to your pane"));
}

// skill-agent / Scenario: Skill reads the roster when needed instead of
// caching a session-long map
#[test]
fn skill_reads_the_roster_when_needed() {
    let s = section("### Resolve pane to agent via pane_current_path");
    assert!(s.contains("read the roster when you need the pane of an agent"));
    assert!(!s.contains("once, and reuse it for the rest of the session"));
    assert!(s.contains("git paw add") && s.contains("git paw remove"));
    assert!(s.contains("tmux display-message -t <session>:0.<pane>"));
}

// skill-agent / Scenario: An unattended merge follows the trigger without
// asking first
#[test]
fn unattended_merge_follows_the_trigger() {
    let s = section("### Merge orchestration");
    assert!(s.contains("without first asking the human"));
    assert!(s.contains("fast-forward only"));
    for escalation in [
        "a conflict",
        "dependency cycle",
        "an exhausted correction cycle",
        "a regression",
    ] {
        assert!(s.contains(escalation), "must name `{escalation}`");
    }
    assert!(
        !template().contains("Ask the human before merging"),
        "the unconditional ask-before-merging rule must be gone"
    );
}

// skill-agent / Scenario: An attended merge may confirm with the human
#[test]
fn attended_merge_may_confirm_with_the_human() {
    let s = section("### Merge orchestration");
    assert!(s.contains("you may confirm the merge order with the human"));
}

// skill-agent *Merge orchestration*: reads and the cycle question go through
// the helpers.
#[test]
fn merge_orchestration_reads_through_helpers() {
    let s = section("### Merge orchestration");
    assert!(s.contains("sweep.sh inbox") || s.contains("sweep.sh events"));
    assert!(s.contains("broker.sh --agent supervisor question"));
}

// supervisor-skill-discipline / Scenario: Skill states the
// never-non-agent-surface rule
#[test]
fn skill_states_the_never_non_agent_surface_rule() {
    let s = section("### Driving agent panes");
    assert!(s.contains("never to a non-agent surface"));
    assert!(s.contains("pane 0 is the dashboard"));
    assert!(s.contains("your own terminal"));
    assert!(s.contains("interrupts"));
}

// supervisor-skill-discipline / Scenario: With a loop running, escalations
// are handled before the sweep; An approval follows an explicit human
// instruction
#[test]
fn drive_loop_directive_relays_before_the_sweep() {
    let skill = template();
    let start = skill
        .find("<!-- drive-loop-directive:begin -->")
        .expect("directive region");
    let end = skill
        .find("<!-- drive-loop-directive:end -->")
        .expect("directive region end");
    let region = collapse(&skill[start..end]);
    let drain = region.find("drain the loop's escalations first").unwrap();
    let sweep = region.find("then run your normal sweep").unwrap();
    assert!(drain < sweep);
    assert!(region.contains("relay each one to the human in your session"));
    assert!(region.contains("only when the human explicitly tells you to"));
}

// supervisor-skill-discipline / Scenario: User asks for the agent inventory
// (the supervisor row carries no pane index)
#[test]
fn agents_inventory_supervisor_row_has_no_pane_index() {
    let s = section("### Routing through the supervisor");
    assert!(s.contains("your `supervisor` row carries **no pane index**"));
    let skill = template();
    let row = skill
        .lines()
        .find(|l| l.starts_with("supervisor  "))
        .expect("example table carries a supervisor row");
    assert!(
        row.trim_end().ends_with('—'),
        "the supervisor row's pane column must be empty: {row}"
    );
}

// supervisor-skill-discipline / Scenario: Successful tell to a live agent —
// `/agents` and `/tell` are typed in the orchestrator's own session.
#[test]
fn directives_are_typed_in_the_orchestrators_session() {
    let s = section("### Routing through the supervisor");
    assert!(s.contains("user types them at the start of a line in your session"));
    assert!(s.contains("acknowledge** in your session"));
}

// skill-agent / Scenario: Skill mentions interactive user input
#[test]
fn user_input_section_is_addressed_to_the_session() {
    let s = section("### When the user types in your session");
    assert!(s.contains("sweep.sh status") && s.contains("sweep.sh capture"));
    assert!(s.contains("sweep.sh feedback-gate"));
    assert!(s.contains("broker.sh --agent supervisor question"));
}

// skill-agent / *Supervisor skill — permission decisions belong to the agent
// CLI and the human*: the per-CLI setup section names the exported skill.
#[test]
fn per_cli_setup_names_the_exported_cli_setup_skill() {
    let s = section("## Per-CLI setup");
    assert!(s.contains(".git-paw/skills/cli-setup/skill.md"));
    assert!(s.contains("get_skill"));
}

// skill-agent / *Orchestrator skill confirms a merge landed before archiving*:
// the ancestor check sits between the fast-forward and every archive / publish.
#[test]
fn merge_landing_is_checked_before_archive_and_publish() {
    let s = section("### Merge orchestration");
    let merge = s.find("git merge --ff-only").expect("fast-forward step");
    let check = s
        .find("git merge-base --is-ancestor")
        .expect("the landing check");
    let archive = s.find("archive step").expect("the archive step");
    let publish = s
        .find("sweep.sh advanced-main")
        .expect("the advanced-main publish");
    assert!(merge < check && check < archive && check < publish);
    assert!(
        s.contains("the merge did **not** land") && s.contains("archive nothing, publish nothing"),
        "a failed check must stop the archive and the publish"
    );
}

// skill-agent "Supervisor skill — Merge orchestration": the merge section
// names its trigger and computes the order from agent.blocked edges.
#[test]
fn merge_section_names_its_trigger_and_the_topological_order() {
    let s = section("### Merge orchestration");
    assert!(
        s.contains("agent.verified"),
        "the trigger is every agent verified"
    );
    assert!(
        s.contains("`agent.blocked`"),
        "the order comes from agent.blocked events"
    );
    assert!(
        s.contains("`payload.from = y`"),
        "an edge names its payload.from"
    );
    assert!(s.contains("x depends on y"), "the edge direction is stated");
    assert!(
        s.contains("topologically sort"),
        "the graph is topologically sorted"
    );
}

// skill-agent "Supervisor skill — Merge orchestration", scenario "Supervisor
// skill describes the per-branch merge + test loop": the loop merges the
// resolved agent branch, confirms it landed, and names no literal branch.
#[test]
fn per_branch_merge_loop_names_no_literal_branch_and_confirms_the_merge() {
    let s = section("### Merge orchestration");
    assert!(s.contains("git merge --ff-only"));
    assert!(s.contains("git merge-base --is-ancestor"));
    assert!(s.contains("git reset --hard"));
    assert!(s.contains("agent.feedback"));
    assert!(!s.contains("git checkout main"), "{s}");
    assert!(!s.contains("git merge --ff-only feat/"), "{s}");
}

// Same requirement: an archive that writes the tracked tree re-runs the tests
// before the merge is announced.
#[test]
fn an_archive_writing_the_tracked_tree_reruns_the_tests() {
    let s = section("### Merge orchestration");
    let condition = s
        .find("archive step that writes to the tracked tree")
        .expect("names an archive step that writes to the tracked tree");
    let after = &s[condition..];
    let publish = after
        .find("sweep.sh advanced-main")
        .expect("the advanced-main publish follows the archive condition");
    let between = &after[..publish];
    assert!(
        between.contains("{{test_command}}"),
        "the test command must be re-run after the archive and before the merge is announced"
    );
}

// The merge loop's commands must not hardcode the target branch or the
// agent-branch prefix.
#[test]
fn merge_loop_names_no_literal_branch() {
    let s = section("### Merge orchestration");
    let start = s.find("**step 2 —").expect("the step 2 heading");
    let end = s.find("**step 3 —").expect("the step 3 heading");
    let step2 = &s[start..end];
    for literal in ["git checkout main", "feat/<branch>", "feat/__fill_in"] {
        assert!(
            !step2.contains(literal),
            "the merge loop should not hardcode `{literal}`"
        );
    }
    assert!(step2.contains("git checkout __fill_in_target_branch__"));
    assert!(step2.contains("git merge --ff-only __fill_in_agent_branch__"));
}

// Same requirement: must-succeed steps are never piped — the rule is stated,
// and the loop's own commands follow it.
#[test]
fn must_succeed_steps_are_never_piped() {
    let s = section("### Merge orchestration");
    assert!(s.contains("never pipe a must-succeed step"));

    let skill = template();
    let start = skill.find("**Step 2 —").expect("the step 2 heading");
    let end = skill.find("**Step 3 —").expect("the step 3 heading");
    let mut in_block = false;
    let mut commands = 0;
    for line in skill[start..end].lines() {
        if line.trim_start().starts_with("```") {
            in_block = !in_block;
            continue;
        }
        if in_block && !line.trim().is_empty() {
            commands += 1;
            assert!(
                !line.contains(" | "),
                "a merge-loop command is piped: {line}"
            );
        }
    }
    assert!(
        commands >= 5,
        "expected the merge loop's commands, found {commands}"
    );
}

// ---------------------------------------------------------------------------
// orchestrator-gate-references: the audit gates as package references.
// ---------------------------------------------------------------------------

/// The three audit-gate references, by package-relative path, with the
/// bracketed prefix each gate's findings carry.
const AUDIT_REFERENCES: [(&str, &str); 3] = [
    ("references/gates/spec-audit.md", "[spec audit]"),
    ("references/gates/doc-audit.md", "[doc audit]"),
    ("references/gates/security-audit.md", "[security audit]"),
];

/// A spec-validate command that carries the per-invocation `{{CHANGE_ID}}`.
const SPEC_VALIDATE: &str = "spec-check validate {{CHANGE_ID}} --strict";

/// The orchestrator package rendered for the repository root, as
/// `git paw init` installs it, with a configured spec-validate command.
fn rendered_package() -> Vec<git_paw::skills::RenderedFile> {
    let package = git_paw::skills::bundled_package("orchestrator").expect("orchestrator package");
    let resolved = git_paw::skills::ResolvedPackage {
        package,
        template: bare_template(),
    };
    let inputs = git_paw::skills::RenderInputs {
        branch: "main",
        root: true,
        broker_url: "",
        project: "my-project",
        gates: GateCommands {
            spec_validate_command: Some(SPEC_VALIDATE),
            ..GateCommands::default()
        },
        backends: &[SpecBackendKind::OpenSpec],
        family: git_paw::agent_cli::CliFamily::Generic,
    };
    git_paw::skills::render_package(&resolved, &inputs)
}

/// The resolved orchestrator template on its own — `SKILL.md` without the
/// references [`resolve`] appends — as a renderer receives it.
fn bare_template() -> git_paw::skills::SkillTemplate {
    git_paw::skills::resolve_from("orchestrator", &git_paw::skills::SkillRoots::default())
        .expect("orchestrator resolves")
}

/// The rendered file at `path` of [`rendered_package`].
fn rendered_file(path: &str) -> String {
    rendered_package()
        .into_iter()
        .find(|f| f.path == path)
        .map_or_else(
            || panic!("the orchestrator package renders `{path}`"),
            |f| f.content,
        )
}

/// The raw orchestrator `SKILL.md` template (helper paths rendered), without
/// its references.
fn skill_md_only() -> String {
    git_paw::skills::bundled_package("orchestrator")
        .map(|p| git_paw::skills::substitute_helper_paths(p.skill_md.content))
        .expect("orchestrator package")
}

/// The Workflow step that opens with `start`, up to `end`, collapsed.
fn workflow_step(skill: &str, start: &str, end: &str) -> String {
    let from = skill
        .find(start)
        .unwrap_or_else(|| panic!("SKILL.md has the `{start}` step"));
    let to = from
        + skill[from..]
            .find(end)
            .unwrap_or_else(|| panic!("`{end}` follows `{start}`"));
    collapse(&skill[from..to])
}

// skill-agent / Scenario: The three audit references exist and are rendered
#[test]
fn the_three_audit_references_exist_and_are_rendered() {
    let files = rendered_package();
    for (path, _) in AUDIT_REFERENCES {
        let file = files
            .iter()
            .find(|f| f.path == path)
            .unwrap_or_else(|| panic!("the package renders `{path}`"));
        assert!(!file.executable, "`{path}` is a document, not a script");
        assert!(
            !file.content.contains("_COMMAND}}") && !file.content.contains("{{SPEC_PATH"),
            "`{path}` has no unrendered gate placeholder"
        );
    }
    let spec_audit = rendered_file("references/gates/spec-audit.md");
    assert!(
        spec_audit.contains(SPEC_VALIDATE),
        "the spec-audit reference carries the rendered spec-validate command"
    );
}

// skill-agent / Scenario: Gate content is moved, not duplicated
#[test]
fn gate_content_is_moved_not_duplicated() {
    let skill = rendered_file("SKILL.md");
    for moved in [
        "### Spec Audit Procedure",
        "**Locate specs**",
        "#### Governance verification",
        "**Missing-doc handling.**",
        "the project's user-guide pages (mdBook",
        "top-level `README.md`",
        "- command injection",
        "- path traversal",
    ] {
        assert!(
            !skill.contains(moved),
            "`{moved}` moved to a reference and must not remain in SKILL.md"
        );
    }
}

// skill-agent / Scenario: Each reference is self-contained
#[test]
fn each_reference_is_self_contained() {
    for (path, prefix) in AUDIT_REFERENCES {
        let text = collapse(&rendered_file(path));
        assert!(
            text.contains("re-resolve") && text.contains("git rev-parse <branch>"),
            "`{path}` states the gate runs against the re-resolved tip"
        );
        assert!(
            text.contains("merge-base") && text.contains("git merge-base <integration-target>"),
            "`{path}` states change contribution is diffed against the merge-base"
        );
        assert!(
            text.contains("change id"),
            "`{path}` names the change id among its inputs"
        );
        assert!(
            text.contains(&format!("{prefix} <")),
            "`{path}` states the `{prefix}` finding format"
        );
        assert!(
            text.contains("publish nothing") && text.contains("return the verdict"),
            "`{path}` states a delegated reader publishes nothing and returns only its verdict"
        );
    }
}

// skill-agent / Scenario: Each reference restates the bare-command rule
#[test]
fn each_reference_restates_the_bare_command_rule() {
    for (path, _) in AUDIT_REFERENCES {
        let text = collapse(&rendered_file(path));
        assert!(
            text.contains("run the gate command bare"),
            "`{path}` instructs running the gate command bare"
        );
        assert!(
            text.contains("read its exit status directly"),
            "`{path}` instructs reading the exit status directly"
        );
        assert!(
            text.contains("never pipe it into another command"),
            "`{path}` forbids piping the gate command"
        );
    }
}

// skill-agent / Scenario: Testing and regression stay inline
#[test]
fn testing_and_regression_stay_inline() {
    let skill = skill_md_only();
    let testing = workflow_step(&skill, "4. **Testing**", "5. **Regression analysis**");
    assert!(testing.contains("{{test_command}}"));
    assert!(testing.contains("run the whole suite — never fail-fast"));
    let regression = workflow_step(&skill, "5. **Regression analysis**", "6. **Spec audit**");
    assert!(regression.contains("any test that previously passed and now fails is a regression"));
    assert!(regression.contains("[regression] <test name>"));
}

// skill-agent / Scenario: Each audit gate step names its installed reference
#[test]
fn each_audit_gate_step_names_its_installed_reference() {
    let skill = skill_md_only();
    for (start, end, gate) in [
        ("6. **Spec audit**", "6a. **Doc audit**", "spec-audit"),
        ("6a. **Doc audit**", "6b. **Security audit**", "doc-audit"),
        (
            "6b. **Security audit**",
            "7. **Verify or feedback**",
            "security-audit",
        ),
    ] {
        let step = workflow_step(&skill, start, end);
        assert!(
            step.contains(&format!(
                ".git-paw/skills/orchestrator/references/gates/{gate}.md"
            )),
            "the `{start}` step names its installed reference"
        );
        assert!(
            step.contains("before running this gate"),
            "the `{start}` step says to read the reference before running the gate"
        );
    }
}

// skill-agent / Scenario: A moved section still satisfies its requirement
#[test]
fn a_moved_section_still_satisfies_its_requirement() {
    let package = git_paw::skills::bundled_package("orchestrator").expect("orchestrator package");
    let resolved = git_paw::skills::ResolvedPackage {
        package,
        template: bare_template(),
    };
    let inputs = git_paw::skills::RenderInputs {
        branch: "main",
        root: true,
        broker_url: "",
        project: "my-project",
        gates: GateCommands::default(),
        backends: &[],
        family: git_paw::agent_cli::CliFamily::Generic,
    };
    let document = git_paw::skills::render_package_document(&resolved, &inputs);
    let reference = document
        .find("## Reference: references/gates/spec-audit.md")
        .expect("the package document carries the spec-audit reference");
    let procedure = document
        .find("### Spec Audit Procedure")
        .expect("the package satisfies the Spec Audit Procedure requirement");
    assert!(
        procedure > reference,
        "the procedure is found in references/gates/spec-audit.md"
    );
}

// ---------------------------------------------------------------------------
// orchestrator-gate-references: the delegation contract.
// ---------------------------------------------------------------------------

/// Vendor product and model names, and vendor persona paths, that the
/// delegation section must not carry.
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
    "antigravity",
    "opus",
    "sonnet",
    "haiku",
    "gpt",
    "/agents/",
    ".claude",
    ".codex",
    ".gemini",
];

/// Build tools, package managers and recipe runners an exported instruction
/// must not name.
const BUILD_TOKENS: &[&str] = &[
    "cargo", "just", "make", "npm", "pip", "yarn", "pnpm", "gradle", "maven",
];

/// The whole words of `text`, lower-cased.
fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// The bold-led paragraph of `SKILL.md` opened by `lead`, collapsed.
fn paragraph(lead: &str) -> String {
    let skill = skill_md_only();
    let start = skill
        .find(lead)
        .unwrap_or_else(|| panic!("SKILL.md has the `{lead}` paragraph"));
    let rest = &skill[start..];
    collapse(&rest[..rest.find("\n\n").unwrap_or(rest.len())])
}

// skill-agent / Scenario: The delegation section defines the verdict shape
#[test]
fn the_delegation_section_defines_the_verdict_shape() {
    let s = section("### Delegating gates");
    assert!(s.contains("compressed verdict"));
    assert!(s.contains("the gate name; `pass` or `fail`"));
    assert!(s.contains("one line per finding, `[<gate prefix>] <location>: <issue>`"));
    assert!(s.contains("returns no raw tool output"));
}

// skill-agent / Scenario: Delegation follows testing and regression
#[test]
fn delegation_follows_testing_and_regression() {
    let s = section("### Delegating gates");
    assert!(s.contains("delegate the audit gates only after both have passed"));
    assert!(s.contains("run the testing and regression-analysis gates (steps 4 and 5) yourself"));
}

// skill-agent / Scenario: Only the orchestrator publishes
#[test]
fn only_the_orchestrator_publishes() {
    let s = section("### Delegating gates");
    assert!(s.contains("a delegated gate never publishes `agent.verified` or `agent.feedback`"));
    assert!(s.contains(
        "you publish `agent.verified` only when testing and regression passed and all three audit verdicts are `pass`"
    ));
    assert!(s.contains("one `feedback-gate` call per finding"));
    assert!(s.contains(
        "a delegated gate never answers a permission prompt, sends input to an agent pane, or runs a broker or sweep helper verb"
    ));
}

// skill-agent / Scenario: Inline fallback without subagents
#[test]
fn inline_fallback_without_subagents() {
    let s = section("### Delegating gates");
    assert!(s.contains(
        "without subagent support**, read each reference yourself and run the gate inline"
    ));
    assert!(s.contains("isolated read-only context that returns a map"));
}

// skill-agent / Scenario: The delegation section is vendor-neutral
#[test]
fn the_delegation_section_is_vendor_neutral() {
    let s = section("### Delegating gates");
    for token in VENDOR_TOKENS {
        assert!(
            !s.contains(token),
            "the delegation section names `{token}`:\n{s}"
        );
    }
}

// ---------------------------------------------------------------------------
// orchestrator-gate-references: a self-hosted session runs the branch's build.
// ---------------------------------------------------------------------------

// skill-agent / Scenario: A self-hosted session checks the binary it runs
#[test]
fn a_self_hosted_session_checks_the_binary_it_runs() {
    let bootstrap = section("### Bootstrap — your first action");
    assert!(bootstrap.contains("self-hosted session: run the branch's own build"));
    let p = paragraph("**Self-hosted session:");
    assert!(p.contains("git-paw's own source"));
    assert!(p.contains("confirm at bootstrap that the `git paw` this session runs is the build from the branch under test, not a previously installed copy"));
    assert!(p.contains("a stale binary silently re-introduces bugs the branch has already fixed"));
}

// skill-agent / Scenario: A stale binary is raised before work is driven
#[test]
fn a_stale_binary_is_raised_before_work_is_driven() {
    let p = paragraph("**Self-hosted session:");
    assert!(p.contains(
        "tell the human to rebuild it with the project's own build and run the session on that build, before you drive any agent"
    ));
}

// skill-agent / Scenario: The instruction names no build command
#[test]
fn the_self_hosted_instruction_names_no_build_command() {
    let p = words(&paragraph("**Self-hosted session:"));
    for token in BUILD_TOKENS {
        assert!(
            !p.iter().any(|w| w == token),
            "the self-hosted-session instruction names `{token}`"
        );
    }
}

// ---------------------------------------------------------------------------
// orchestrator-gate-references: the Baseline step prepares dependencies.
// ---------------------------------------------------------------------------

/// Workflow step 1 (Baseline), collapsed.
fn baseline_step() -> String {
    workflow_step(
        &skill_md_only(),
        "1. **Baseline**",
        "1.5 **Launch-time pane sweep**",
    )
}

// skill-agent / Scenario: The Baseline step prepares the checkout first
#[test]
fn the_baseline_step_prepares_the_checkout_first() {
    let s = baseline_step();
    assert!(s.contains(
        "before recording, ensure the checkout you run the gate commands in — the root checkout or the verify worktree — has the project's dependencies installed"
    ));
    assert!(s.contains(
        "the way the project's own setup or its `[worktree.hooks] on_create` hook installs them"
    ));
}

// skill-agent / Scenario: A baseline that fails for missing dependencies is
// not recorded
#[test]
fn a_baseline_that_fails_for_missing_dependencies_is_not_recorded() {
    let s = baseline_step();
    assert!(s.contains(
        "if the baseline fails because dependencies are missing, install them and re-run it"
    ));
    assert!(s.contains("never record that failing run as the baseline"));
}

// skill-agent / Scenario: The Baseline instruction names no package manager
#[test]
fn the_baseline_instruction_names_no_package_manager() {
    let s = words(&baseline_step());
    for token in BUILD_TOKENS {
        assert!(
            !s.iter().any(|w| w == token),
            "the Baseline step names `{token}`"
        );
    }
}

// ---------------------------------------------------------------------------
// orchestrator-gate-references: no gate sentence lost in the move.
// ---------------------------------------------------------------------------

/// The gate text as it stood in `SKILL.md` before the move.
const PRE_MOVE_GATES: &str = include_str!("fixtures/orchestrator_gates_pre_move.md");

/// Pre-move sentences deliberately reworded by the move, each with the text
/// that replaced it. In a separate reference, "below" no longer points at the
/// Spec Audit Procedure, so doc-audit names its reference path instead.
const REWORDED: [(&str, &str); 1] = [(
    "(see \"Spec Audit Procedure\" below —",
    "(see \"Spec Audit Procedure\" in `.git-paw/skills/orchestrator/references/gates/spec-audit.md` —",
)];

// skill-agent / Scenario: Gate content is moved, not duplicated — and no
// sentence of it is lost: the union of SKILL.md and its references carries
// every pre-move gate sentence byte-for-byte (whitespace collapsed).
#[test]
fn every_pre_move_gate_sentence_survives_in_the_package() {
    let package = git_paw::skills::bundled_package("orchestrator").expect("orchestrator package");
    let squash = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ");
    let union = squash(&git_paw::skills::package_text(package));

    let body = PRE_MOVE_GATES
        .split_once("-->")
        .map_or(PRE_MOVE_GATES, |(_, rest)| rest);
    // Drop the step numbers ("6. ", "6a. ", "6b. ") that only the Workflow
    // pointers keep; the moved bodies are unnumbered paragraphs.
    let body: String = body
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            ["6. ", "6a. ", "6b. "]
                .iter()
                .find_map(|n| trimmed.strip_prefix(n))
                .unwrap_or(line)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut body = squash(&body);
    for (before, after) in REWORDED {
        assert!(
            body.contains(before),
            "reworded sentence `{before}` is in the fixture"
        );
        assert!(union.contains(after), "its replacement `{after}` ships");
        body = body.replace(before, after);
    }

    let mut sentences = 0;
    let mut rest = body.as_str();
    while !rest.is_empty() {
        let end = rest
            .char_indices()
            .find(|&(i, c)| matches!(c, '.' | ':' | ';') && rest[i + 1..].starts_with(' '))
            .map_or(rest.len(), |(i, _)| i + 1);
        let sentence = rest[..end].trim();
        if !sentence.is_empty() {
            sentences += 1;
            assert!(
                union.contains(sentence),
                "pre-move gate sentence lost from the orchestrator package: {sentence}"
            );
        }
        rest = rest[end..].trim_start();
    }
    assert!(
        sentences >= 70,
        "expected the whole gate text, checked {sentences} sentences"
    );
}

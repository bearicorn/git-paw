## MODIFIED Requirements

### Requirement: Supervisor skill — Merge orchestration

The embedded orchestrator skill (`assets/agent-skills/orchestrator/SKILL.md`, formerly `supervisor.md`) SHALL include a "Merge orchestration" section that replaces the v0.4 Rust `run_merge_loop` function. The section SHALL instruct the orchestrator to perform merge orchestration through `git paw finish` run from the repository root, plus the broker for ordering and questions (through the bundled helpers or the MCP tools — never a raw `curl`), and SHALL cover:

1. **When to merge** — once all expected agents have published `agent.verified` (or after the user explicitly asks the orchestrator to merge). In an unattended run (the session context carries the drive-loop directive) the orchestrator SHALL integrate as soon as this trigger holds, through the queue (item 3), without first asking the human, and SHALL hold a branch back and escalate only on a conflict (a branch the queue skips, or a dependency cycle among the branches), an exhausted correction cycle, or a regression (a rebased branch whose re-test fails): a branch the queue skips, a regressing one included, gets the queue's one rebase directive (item 3); a dependency cycle (item 4) and a branch whose correction budget is exhausted are escalated with `agent.question`; every held-back branch is named in the final summary (item 5). In an attended run the orchestrator MAY confirm the merge order with the human before merging. The skill SHALL NOT carry an unconditional rule to ask the human before every merge.
2. **Compute merge order from `agent.blocked` events** — read the broker's message log through `sweep.sh inbox` / `sweep.sh events` or the MCP read tools to find `agent.blocked` events. For each `agent.blocked` from X with `payload.from = Y`, treat as edge "X depends on Y". Topologically sort the dependency graph; agents with no incoming edges merge first.
3. **Integrate the ordered batch** — run `git paw finish --rebase <branch> <branch> …` with the branches in topological order (or `git paw finish --verified --rebase` when no ordering constraint exists). The command fast-forwards each branch, rebases and re-tests provably disjoint branches itself, never creates merge commits unless the project configured them, tears down each integrated agent, and sends one rebase directive to every skipped agent. Exit `0` means every branch integrated; exit `3` means some were skipped (read the per-branch report); exit `1` is an error to surface to the user. For each branch the report lists as integrated, the orchestrator SHALL confirm the merge landed with `git merge-base --is-ancestor <branch-tip> HEAD`, per "Orchestrator skill confirms a merge landed before archiving", before archiving its spec or publishing `agent.advanced-main`. The skill SHALL NOT instruct raw `git checkout`, `git merge` or `git reset --hard` on the target branch.
4. **Cycle handling** — if the dependency graph has cycles, publish `agent.question` (through `broker.sh --agent supervisor question`) surfacing the cycle and asking the user how to proceed. Do NOT merge any branch in the cycle until the user resolves it.
5. **Final summary** — when the batch is done, publish a final `agent.status` from `agent_id = "supervisor"` summarising what was integrated, what was skipped and why, taken from the command's report.

#### Scenario: Supervisor skill mentions merge orchestration

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or section identifying merge orchestration (e.g. `Merge orchestration`)

Test: `src/skills.rs::supervisor_skill_has_merge_orchestration_section`

#### Scenario: Supervisor skill names the trigger condition

- **WHEN** the embedded supervisor skill's merge-orchestration section is inspected
- **THEN** it states that merge orchestration runs when all expected agents have published `agent.verified` (or on user request)

Test: `tests/orchestrator_skill_content.rs::merge_section_names_its_trigger_and_the_topological_order`

#### Scenario: An unattended merge follows the trigger without asking first

- **WHEN** the merge-orchestration section is inspected
- **THEN** it SHALL state that, in an unattended run, integration through `git paw finish` proceeds once every expected agent is verified, without asking the human first
- **AND** it SHALL name the only escalations: a conflict (a branch the queue skips, or a dependency cycle), an exhausted correction cycle, and a regression
- **AND** the skill SHALL NOT contain an unconditional "ask the human before merging" rule

Test: `tests/orchestrator_skill_content.rs::unattended_merge_follows_the_trigger`

#### Scenario: An attended merge may confirm with the human

- **WHEN** the merge-orchestration section is inspected
- **THEN** it SHALL state that in an attended run the orchestrator may confirm the merge order with the human before merging

Test: `tests/orchestrator_skill_content.rs::attended_merge_may_confirm_with_the_human`

#### Scenario: Supervisor skill describes the topological-order computation

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the orchestrator to read `agent.blocked` messages
- **AND** it states that an `agent.blocked` from X with `payload.from = Y` defines a dependency edge (X depends on Y)
- **AND** it instructs the orchestrator to topologically sort the resulting dependency graph

Test: `tests/orchestrator_skill_content.rs::merge_section_names_its_trigger_and_the_topological_order`

#### Scenario: Supervisor skill describes the per-branch merge + test loop

The title is kept from the per-branch loop this replaces; the loop now runs inside `git paw finish`.

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the orchestrator to run `git paw finish --rebase` with the ordered branches from the repository root
- **AND** it explains exit codes `0`, `3` and `1`
- **AND** it instructs the orchestrator to confirm each integrated branch with `git merge-base --is-ancestor` before archiving or publishing
- **AND** it does not instruct `git checkout`, `git merge --ff-only` or `git reset --hard` against the target branch
- **AND** it SHALL NOT contain `git checkout main` or `git merge --ff-only feat/<branch>`

Test: `tests/orchestrator_skill_content.rs::per_branch_merge_loop_names_no_literal_branch_and_confirms_the_merge`

#### Scenario: Supervisor skill describes cycle handling

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the orchestrator to escalate via `agent.question` when the dependency graph has cycles
- **AND** it instructs the orchestrator NOT to merge cycle members until the user resolves the cycle

Test: `src/skills.rs::supervisor_skill_merge_cycle_uses_agent_question`

#### Scenario: Supervisor skill describes the final summary

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the orchestrator to publish a final `agent.status` summary after merge orchestration completes
- **AND** the summary covers what was integrated, what was skipped, and why

Test: `src/skills.rs::supervisor_skill_merge_publishes_final_status_summary`

### Requirement: Orchestrator skill confirms a merge landed before archiving

The orchestrator skill's merge orchestration SHALL, for each branch `git paw finish` reports as integrated, check that the branch tip is an ancestor of `HEAD` (`git merge-base --is-ancestor <tip> HEAD`) before archiving a spec or publishing `agent.advanced-main`; run the spec archive only after that check; re-run `{{TEST_COMMAND}}` after any archive step that writes to the tracked tree; and forbid piping a must-succeed step. Wording SHALL stay project-agnostic: no hardcoded target or agent branch names.

#### Scenario: Ancestor check precedes archive and publish

- **WHEN** the rendered orchestrator skill's "Merge orchestration" section is inspected
- **THEN** it contains `git merge-base --is-ancestor`
- **AND** that check appears after the `git paw finish` step, before the archive step and before the `advanced-main` publish
- **AND** it says a failed check means the merge did not land: archive nothing, publish nothing

Test: `tests/orchestrator_skill_content.rs::merge_landing_is_checked_before_archive_and_publish`

#### Scenario: An archive writing the tracked tree re-runs the tests

- **WHEN** the same section is inspected
- **THEN** it instructs re-running `{{TEST_COMMAND}}` after any archive step that writes to the tracked tree

Test: `tests/orchestrator_skill_content.rs::an_archive_writing_the_tracked_tree_reruns_the_tests`

#### Scenario: Must-succeed steps are not piped

- **WHEN** the same section is inspected
- **THEN** it states that a must-succeed git or test step is never piped, because a pipeline's exit status is its last stage's

Test: `tests/orchestrator_skill_content.rs::must_succeed_steps_are_never_piped`

#### Scenario: Merge-landing steps stay project-agnostic

- **WHEN** the rendered orchestrator skill is audited by `tests/lang_agnostic_skill_audit.rs`
- **THEN** the audit passes and the new steps name no stack-specific command
- **AND** the merge loop's commands name no literal target branch (such as `main`) or agent-branch prefix (such as `feat/`)

Test: `tests/lang_agnostic_skill_audit.rs::exported_orchestrator_skill_passes_the_audit_for_every_backend`, `tests/orchestrator_skill_content.rs::merge_loop_names_no_literal_branch`

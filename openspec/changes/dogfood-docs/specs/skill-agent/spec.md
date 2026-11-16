## ADDED Requirements

### Requirement: Orchestrator tmux commands are not shadowed by shell aliases

Every `tmux` command the embedded orchestrator skill or `docs/src/user-guide/supervisor.md` tells the reader to run SHALL be written `command tmux …`, so a shell alias or function named `tmux` cannot replace it. Each SHALL say once, in one sentence, why the prefix is there. The skill's sentence SHALL name no agent CLI.

#### Scenario: The skill's tmux commands carry the prefix

- **WHEN** the orchestrator skill is rendered
- **THEN** every code-block line and inline command that invokes `tmux` SHALL begin `command tmux`
- **AND** the skill SHALL state that the prefix bypasses a shell alias or function named `tmux`

Test: `tests/orchestrator_skill_content.rs::tmux_commands_bypass_shell_aliases`

#### Scenario: A shadowed tmux still resolves the pane

- **GIVEN** an orchestrator shell where `tmux` is an alias or function that does not run tmux as typed (as oh-my-zsh's `tmux` plugin defines)
- **WHEN** the orchestrator runs the skill's pane-resolution command
- **THEN** the command SHALL reach the tmux binary and print the pane's working directory

Test: manual check recorded in `tasks.md` (an interactive zsh with the alias defined)

### Requirement: The approve step names an ask rule for the orchestrator session

The orchestrator skill's "Who decides a permission prompt" section and the supervisor guide's approve section SHALL say: if the orchestrator's own CLI refuses or stalls a `sweep.sh approve` the human sanctioned, the remedy is an ask rule for `sweep.sh approve` in the orchestrator session's settings (as `git paw setup-cli` writes), so the CLI asks the human. They SHALL NOT suggest an allow rule.

#### Scenario: The skill names the ask-rule remedy

- **WHEN** the orchestrator skill's permission-prompt section is inspected
- **THEN** it SHALL say that the orchestrator's own CLI may refuse or stall a sanctioned `sweep.sh approve`
- **AND** it SHALL name an ask rule for `sweep.sh approve` as the remedy and name no agent CLI

Test: `tests/orchestrator_skill_content.rs::approve_step_names_an_ask_rule_not_an_allow_rule`

#### Scenario: Neither text suggests allowing approve

- **WHEN** the skill section and `docs/src/user-guide/supervisor.md`'s "The explicit approve action" are inspected
- **THEN** neither SHALL suggest an allow rule, or any rule that lets `sweep.sh approve` run without asking

Test: `tests/orchestrator_skill_content.rs::approve_step_names_an_ask_rule_not_an_allow_rule`, `tests/docs_dogfood_notes.rs::supervisor_guide_names_the_approve_ask_rule`

### Requirement: A stopped empty session is replaced by a launch, not purged

The orchestrator skill and the session-lifecycle guide SHALL say that a stopped session recording no agents needs no `git paw purge`: running `git paw start` with a launch selection (`--specs`, `--branches`, `--from-all-specs`) replaces it. The guide SHALL add that this avoids a destructive command an agent's own permission check may refuse.

#### Scenario: The skill launches over a stopped empty session

- **WHEN** the orchestrator skill is inspected
- **THEN** it SHALL tell the orchestrator to replace a stopped session that records no agents by running `git paw start` with its selection, and SHALL NOT tell it to purge that session first

Test: `tests/orchestrator_skill_content.rs::a_stopped_empty_session_is_replaced_not_purged`

#### Scenario: The guide gives the way past a refused purge

- **WHEN** the "Refilling an emptied session" section of `docs/src/user-guide/session-lifecycle.md` is inspected
- **THEN** it SHALL state that `git paw start --specs …` replaces a stopped empty session without `purge`, for when an agent's permission check refuses `git paw purge --force`

Test: `tests/docs_dogfood_notes.rs::session_lifecycle_replaces_a_stopped_empty_session_without_purge`

### Requirement: A kept branch after remove is read as a merge that did not land

The orchestrator skill's merge loop and the `git paw remove` section of the session-lifecycle guide SHALL say that a `Kept branch <name> at <sha> (… not in <default>)` line, printed when a worker whose branch was believed merged is removed, means its tip is not on the default branch. The orchestrator SHALL treat that branch as not landed: archive and publish nothing for it until it lands.

#### Scenario: The skill treats a kept branch as not landed

- **WHEN** the orchestrator skill's merge loop is inspected
- **THEN** it SHALL say that a `Kept branch` line from `git paw remove` means the branch did not land
- **AND** it SHALL tell the orchestrator to archive and publish nothing for that branch until it lands

Test: `tests/orchestrator_skill_content.rs::a_kept_branch_on_remove_means_the_merge_did_not_land`

#### Scenario: The guide explains the kept line as a signal

- **WHEN** the branch-safety step of `git paw remove` in `docs/src/user-guide/session-lifecycle.md` is inspected
- **THEN** it SHALL say that a `Kept branch` line for a branch you believed merged means the merge did not land, or that the worker committed after it

Test: `tests/docs_dogfood_notes.rs::session_lifecycle_reads_a_kept_branch_as_not_landed`

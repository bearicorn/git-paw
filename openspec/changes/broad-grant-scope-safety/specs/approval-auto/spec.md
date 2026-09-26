## MODIFIED Requirements

### Requirement: Broad grant restricted to allowlisted non-arbitrary-code verbs

The auto-approver SHALL select a prompt's permanent broad grant option ("Yes, and don't
ask again for: X"), when the prompt offers one, only when X is NOT an arbitrary-code
runner AND the grant's scope is a specific command AND either X has already been
classified safe / worktree-confined by the decision order, or X's leading verb is in the
read-mostly allowlist (per `safe-command-classification`).
The classification gate is what lets a routine stack command — one safe only through the
resolved `[supervisor.common_dev_allowlist]` preset, such as `cargo test`, whose verb is
not read-mostly — be permanently allowed instead of re-prompting on every identical
occurrence (per `supervisor-unattended-operation`'s **Auto-approval prefers a durable
grant for a safe prompt**). A command that did NOT classify safe is escalated, so no
option is selected for it at all.

Arbitrary-code runners SHALL include `python`, `bash -c`, `sh -c`, `eval`, `node`, and
any command containing a bare ` -c ` code-string flag. For an arbitrary-code runner the
auto-approver SHALL select the one-time "Yes" option and SHALL NEVER select the permanent
broad grant, whatever its classification — so an interpreter run of a worktree-resident
script stays a one-time approval even though it classifies safe.

The **grant scope** condition is independent of classification. A permanent-grant option
SHALL be selected only when its own text grants a specific command. An option that grants
a directory, a path prefix, or a class of commands — for example "don't ask again for
similar commands in `<path>`", or any grant naming a location rather than a command —
SHALL NEVER be selected automatically, whatever the verdict on the command itself,
because it permanently allowlists commands that were never classified. An option whose
grant scope cannot be determined from its text SHALL be treated as scope-wide and
refused. When the only permanent-grant option offered is scope-wide, the auto-approver
SHALL select the one-time "Yes" option rather than escalating: the command itself already
classified safe, so the narrower approval is sufficient and the wave is not blocked.

This scope determination SHALL NOT be sourced from a CLI prompt profile. Per
`cli-prompt-profiles`, security decisions are never profile-sourced, and the breadth of a
permanent permission grant is such a decision.

#### Scenario: Allowlisted verb takes the broad grant

- **GIVEN** a live 3-option prompt for `git status` offering "Yes, and don't ask again for: git status"
- **WHEN** the auto-approver fires
- **THEN** it SHALL select the broad-grant option (option 2)

#### Scenario: A safe-classified stack command takes the broad grant

- **GIVEN** a live 3-option prompt for `cargo test --lib`, safe through the resolved allowlist but with a verb that is not read-mostly
- **WHEN** the auto-approver fires
- **THEN** it SHALL select the broad-grant option (option 2)

#### Scenario: An unclassified command takes no broad grant

- **GIVEN** a live 3-option prompt for a command that matched no safe rule
- **WHEN** the auto-approver evaluates it
- **THEN** it SHALL escalate and SHALL NOT select the permanent broad-grant option

#### Scenario: python -c never gets a permanent broad grant

- **GIVEN** a live 3-option prompt for `python3 -c "import os; os.remove('x')"`
- **WHEN** the auto-approver fires
- **THEN** it SHALL select the one-time "Yes" option (option 1)
- **AND** it SHALL NOT select the permanent broad-grant option

#### Scenario: bash -c never gets a permanent broad grant

- **GIVEN** a live 3-option prompt for `bash -c "do-thing"`
- **WHEN** the auto-approver fires
- **THEN** it SHALL select the one-time "Yes" option and SHALL NOT take the broad grant

#### Scenario: A directory-wide grant is refused for a safe command

- **GIVEN** a live 3-option prompt for a safe-classified command whose option 2 reads "Yes, and don't ask again for similar commands in `/path/to/worktree`"
- **WHEN** the auto-approver fires
- **THEN** it SHALL select option 1 (one-time Yes)
- **AND** it SHALL NOT select option 2

#### Scenario: A compound safe command does not widen the allowlist

- **GIVEN** a live 3-option prompt for a compound command such as `cargo test --lib config:: 2>&1 | tail -15; echo "---"; cargo test --bin git-paw init::`, every segment of which classifies safe, and whose only permanent-grant option is scope-wide
- **WHEN** the auto-approver fires
- **THEN** it SHALL select the one-time option
- **AND** no directory-scoped grant SHALL be written

#### Scenario: A script run is not a code string but still refuses a scope-wide grant

- **GIVEN** a live 3-option prompt for `timeout 1 bash /tmp/script.sh` whose option 2 grants similar commands in a directory
- **WHEN** the auto-approver fires
- **THEN** it SHALL select the one-time option, because the grant is scope-wide even though the command is not an arbitrary-code code-string

#### Scenario: An undeterminable grant scope is refused

- **GIVEN** a live 3-option prompt whose permanent-grant option text does not make its scope determinable
- **WHEN** the auto-approver fires
- **THEN** it SHALL treat the grant as scope-wide and SHALL select the one-time option

### Requirement: Option-index selection for Yes/No prompts

When dispatching approval keystrokes, the auto-approver SHALL select the option by reading each option's text, never by assuming a fixed meaning for a position in the list. For a 2-option Yes/No prompt, option 1 SHALL be "Yes" and SHALL be selected. For a 3-option Yes / Yes-and-don't-ask-again / No prompt, the auto-approver SHALL select the permanent-grant option only when the broad-grant rule permits it (per "Broad grant restricted to allowlisted non-arbitrary-code verbs", including its grant-scope condition), and otherwise SHALL select the one-time Yes option. Because a permanent-grant option's breadth varies with the prompt, its index SHALL NOT by itself be taken as evidence that selecting it is permitted.

The bundled `sweep.sh approve` helper SHALL follow the same option selection: it SHALL parse the numbered option list from the fresh pre-send capture and dispatch the selected option's digit followed by `Enter`. It SHALL NOT dispatch a blind cursor-movement sequence (e.g. `Down` + `Enter`) whose landing option depends on the prompt shape — on a 2-option Yes/No prompt such a sequence selects "No", and on a 3-option prompt it takes the permanent broad grant without consulting the broad-grant rule.

#### Scenario: Two-option prompt selects Yes

- **GIVEN** a live 2-option Yes/No prompt for a safe command
- **WHEN** the auto-approver fires
- **THEN** it SHALL select option 1 (Yes)

#### Scenario: Three-option prompt with allowlisted verb selects the broad grant

- **GIVEN** a live 3-option prompt for an allowlisted non-arbitrary-code command whose permanent-grant option names that specific command
- **WHEN** the auto-approver fires
- **THEN** it SHALL select option 2 (Yes, and don't ask again)

#### Scenario: Three-option prompt with arbitrary-code runner selects one-time Yes

- **GIVEN** a live 3-option prompt for `python3 -c "..."`
- **WHEN** the auto-approver fires
- **THEN** it SHALL select option 1 (one-time Yes), not option 2

#### Scenario: Option index alone does not authorise the permanent grant

- **GIVEN** two live 3-option prompts for the same safe command, one whose option 2 names the specific command and one whose option 2 grants similar commands in a directory
- **WHEN** the auto-approver fires on each
- **THEN** it SHALL select option 2 for the first and option 1 for the second

#### Scenario: Sweep helper approves a 2-option prompt affirmatively

- **GIVEN** a live 2-option Yes/No prompt on an agent pane
- **WHEN** the operator runs `sweep.sh approve <pane>`
- **THEN** the helper SHALL send the digit `1` then `Enter` (selecting Yes)
- **AND** SHALL NOT send `Down` + `Enter`

#### Scenario: Sweep helper respects the broad-grant rule on 3-option prompts

- **GIVEN** a live 3-option prompt for an arbitrary-code runner (e.g. `bash -c "..."`)
- **WHEN** the operator runs `sweep.sh approve <pane>`
- **THEN** the helper SHALL select option 1 (one-time Yes), agreeing with the in-tool auto-approver's option resolution

#### Scenario: Sweep helper refuses a scope-wide grant

- **GIVEN** a live 3-option prompt whose permanent-grant option grants similar commands in a directory
- **WHEN** the operator runs `sweep.sh approve <pane>`
- **THEN** the helper SHALL select the one-time option, agreeing with the in-tool auto-approver

### Requirement: Worktree-confined file operations classify as safe

The auto-approve classifier SHALL recognise Claude's
filesystem-prompt patterns (write / edit / create / delete)
and SHALL classify them as safe-by-pattern when the resolved
target path is inside the agent's worktree root. The target path
SHALL be taken from the prompt's file header — the path line that
follows the `Edit file` / `Write file` / `Create file` heading, which
the CLI renders relative to the pane's working directory or as an
absolute path — and SHALL NOT be taken from the question line
(`Do you want to make this edit to <name>?`), which the CLI may
abbreviate to a bare file name. A relative header path SHALL be
resolved against the agent's worktree root. When no header path can
be found, or the header path and the question-line name disagree on
the file name, the prompt SHALL NOT be classified safe-by-pattern
(fail closed). The classifier SHALL canonicalize paths before the
`starts_with(worktree_root)` check to prevent symlink-escape.

#### Scenario: In-worktree file create is auto-approved

- **GIVEN** an agent on `feat/cold-start-ci-parity` whose
  worktree root is `/path/to/git-paw-feat-cold-start-ci-parity/`
- **WHEN** Claude prompts "Do you want to allow this write
  to Containerfile?" (resolving to the worktree root)
- **THEN** the classifier SHALL return safe-by-pattern, and
  the auto-approve sweep SHALL dispatch the approval
  keystrokes

#### Scenario: Out-of-worktree file create requires manual approval

- **GIVEN** the same agent
- **WHEN** Claude prompts a write to `/etc/hosts` (or any
  path resolving outside the worktree)
- **THEN** the classifier SHALL NOT return safe-by-pattern,
  and the existing manual-prompt flow SHALL run

#### Scenario: Symlink-escape attempt does not bypass the boundary

- **GIVEN** a malicious prompt whose path contains `..`
  resolving outside the worktree
- **WHEN** the classifier canonicalizes the path
- **THEN** the resolved path SHALL be checked against the
  worktree root via `starts_with`, and the symlink-escape
  attempt SHALL fail the safety check

#### Scenario: A bare file name on the question line does not hide an out-of-worktree header path

- **GIVEN** an agent whose worktree is `<repo>/.git-paw/worktrees/feat-x`
- **AND** a captured Edit prompt whose file header reads `../../../openspec/changes/x/tasks.md` while its question line reads `Do you want to make this edit to tasks.md?`
- **WHEN** the classifier evaluates the prompt
- **THEN** it SHALL resolve the header path against the worktree root, find it outside the worktree, and SHALL NOT classify the prompt safe-by-pattern

#### Scenario: A prompt with no recognisable header path fails closed

- **GIVEN** a captured file prompt whose question line names `tasks.md` and whose file header path cannot be found in the capture
- **WHEN** the classifier evaluates the prompt
- **THEN** it SHALL NOT classify the prompt safe-by-pattern

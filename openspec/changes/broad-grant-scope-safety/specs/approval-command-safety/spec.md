## MODIFIED Requirements

### Requirement: git-paw managed helper-script invocations classify as safe

The classifier SHALL classify as safe a command whose normalized leading verb
resolves to one of git-paw's own managed helper scripts under `.git-paw/scripts/`
(`broker.sh`, `sweep.sh`, `docs-fetch.sh`), because git-paw authors these scripts
and they perform only bounded coordination actions. The match SHALL be on the exact
managed-script location, whether invoked directly (`.git-paw/scripts/broker.sh …`)
or via an interpreter (`bash .git-paw/scripts/broker.sh …`). The invoked path SHALL
be normalised before it is matched, and a path containing any `..` component SHALL be
refused outright. When the pane's worktree root is known, the normalised path —
relative paths resolved against the worktree root, symlinks resolved where the file
exists — SHALL resolve to exactly `<worktree>/.git-paw/scripts/<name>` inside that
worktree; a helper filename anywhere else, including elsewhere under `.git-paw/`
(for example `.git-paw/tmp/` or `.git-paw/other/`), SHALL NOT match. When no worktree
root is known (the supervisor pane), only a path that normalises to the relative path
`.git-paw/scripts/<name>` SHALL match. Membership of git-paw's managed subtree (as
identified by `is_managed_path`, which accepts anything under `.git-paw/`) SHALL NOT
by itself qualify a path for this rule. The classifier SHOULD additionally verify that
the invoked file's content equals the bundled asset embedded in the running binary,
and SHOULD NOT apply this rule to a helper whose content differs. This rule SHALL be
subject to danger-list precedence: a slice that chains a managed-script invocation
with a danger-class operation SHALL still escalate as danger.

#### Scenario: A bundled broker.sh boot call classifies safe

- **GIVEN** the command `.git-paw/scripts/broker.sh --agent feat-x status booting`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL classify as safe (no escalation)

#### Scenario: A bundled sweep.sh call classifies safe

- **GIVEN** the command `.git-paw/scripts/sweep.sh status-publish`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL classify as safe

#### Scenario: An interpreter-invoked helper at the managed path classifies safe

- **GIVEN** an agent pane whose worktree root is known
- **WHEN** the command is `bash .git-paw/scripts/sweep.sh status`
- **THEN** it SHALL classify as safe under the managed helper-script rule

#### Scenario: A dot-dot escape is refused

- **GIVEN** an agent pane whose worktree root is known
- **WHEN** the command is `bash .git-paw/../../tmp/sweep.sh`
- **THEN** the managed helper-script rule SHALL NOT match
- **AND** the command SHALL NOT classify as safe

#### Scenario: A helper filename elsewhere under .git-paw is refused

- **GIVEN** an agent pane whose worktree root is known
- **WHEN** the command is `bash .git-paw/other/sweep.sh` or `bash .git-paw/tmp/sweep.sh`
- **THEN** the managed helper-script rule SHALL NOT match either command
- **AND** neither command SHALL classify as safe

#### Scenario: A modified helper is not trusted

- **GIVEN** an agent worktree whose `.git-paw/scripts/sweep.sh` content differs from the bundled `sweep.sh` asset
- **WHEN** the command is `.git-paw/scripts/sweep.sh status`
- **THEN** the classifier SHOULD NOT classify it safe through the managed helper-script rule

#### Scenario: A managed-script invocation chained with a danger command still escalates

- **GIVEN** the command `.git-paw/scripts/sweep.sh snapshot && rm -rf /`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL escalate as danger

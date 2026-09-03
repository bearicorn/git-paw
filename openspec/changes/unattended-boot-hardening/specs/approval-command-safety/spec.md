## ADDED Requirements

### Requirement: Classifier normalizes leading assignment and wrapper prefixes before matching

The safe-command classifier SHALL strip a run of leading `NAME=value`
environment-variable assignments and leading `env` / `nohup` invocation wrappers
from a command before prefix-matching it against the allowlist, so that an
otherwise-safe command is not forced `unknown` merely because it was prefixed for
the run environment. Normalization SHALL remove: zero or more leading `NAME=value`
assignments (shell assignment syntax, values optionally quoted); a leading `env`
wrapper together with any `NAME=value` assignments and `-i` / `-u NAME` options it
carries; and a leading `nohup` wrapper. It SHALL then classify the remaining verb.
This normalization SHALL compose with the existing trailing exit-code-probe /
redirect normalization. The danger-list, worktree-confinement, config-path, and
`.git/`-write rules SHALL be applied to the fully normalized command exactly as
before — normalization SHALL NOT weaken any escalation.

#### Scenario: A leading VAR=value assignment is normalized away

- **GIVEN** the command `TMPDIR=/tmp/x cargo test --lib`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL classify the same as bare `cargo test --lib`

#### Scenario: Multiple leading assignments plus a trailing probe are both normalized

- **GIVEN** the command `GIT_PAW_ALLOW_LIVE_SESSION=1 TMPDIR=/tmp/x cargo test; echo exit=$?`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL classify the same as bare `cargo test`

#### Scenario: A leading env wrapper is normalized away

- **GIVEN** the command `env FOO=bar cargo build`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL classify the same as bare `cargo build`

#### Scenario: A leading nohup wrapper is normalized away

- **GIVEN** the command `nohup just check`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL classify the same as bare `just check`

#### Scenario: Normalization does not rescue a danger command behind assignments

- **GIVEN** a danger-listed command prefixed with an assignment (e.g. `FOO=bar rm -rf /`)
- **WHEN** the classifier evaluates it
- **THEN** it SHALL still escalate as danger — stripping the leading prefix SHALL NOT downgrade it

### Requirement: git-paw managed helper-script invocations classify as safe

The classifier SHALL classify as safe a command whose normalized leading verb
resolves to one of git-paw's own managed helper scripts under `.git-paw/scripts/`
(`broker.sh`, `sweep.sh`, `docs-fetch.sh`), because git-paw authors these scripts
and they perform only bounded coordination actions. The match SHALL be on the managed-script path (as
identified by `is_managed_path`), whether invoked directly (`.git-paw/scripts/broker.sh …`)
or via an interpreter (`bash .git-paw/scripts/broker.sh …`). This rule SHALL be
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

#### Scenario: A managed-script invocation chained with a danger command still escalates

- **GIVEN** the command `.git-paw/scripts/sweep.sh snapshot && rm -rf /`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL escalate as danger

### Requirement: Writes under a repository `.git/` directory escalate as danger

The classifier SHALL classify as a danger-class escalation — terminal, never
auto-approved, with the same precedence as the curated danger-list — any filesystem
prompt (write / edit / create / delete) or shell command slice that targets a path
resolving inside a repository `.git/` directory (git's own metadata, including
`.git/info/exclude`, `.git/config`, and `.git/hooks/`).
Read-only operations SHALL NOT match this rule. This prevents an agent, or an
auto-approval sweep, from silently mutating git's local configuration — invisible
to teammates and able to subvert the repository's committed ignore/hook scoping.
Target paths SHALL be canonicalized before matching, with the same fail-closed
posture as the worktree boundary check (a path that cannot be canonicalized but
syntactically reaches into a `.git/` directory SHALL be treated as matching). This
rule targets file-path writes; ordinary `git` subcommands are classified by the
git-verb rules and are unaffected.

#### Scenario: Append to .git/info/exclude escalates as danger

- **GIVEN** a prompt whose command slice is `echo '.git-paw/' >> .git/info/exclude`
- **WHEN** the classifier runs
- **THEN** the verdict SHALL be a danger-class escalation
- **AND** no auto-approval keystrokes SHALL ever be dispatched for it

#### Scenario: Write to .git/config escalates as danger

- **GIVEN** a prompt to write `<worktree>/.git/config`
- **WHEN** the classifier runs
- **THEN** the verdict SHALL be a danger-class escalation

#### Scenario: Reading .git metadata is not matched by this rule

- **GIVEN** a prompt whose command slice is `cat .git/config`
- **WHEN** the classifier runs
- **THEN** this rule SHALL NOT match (other classification rules decide the verdict)

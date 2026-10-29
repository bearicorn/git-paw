## ADDED Requirements

### Requirement: The stable surface is published and versioned by semver

git-paw SHALL publish, in a stability chapter of the documentation (`docs/src/stability.md`), the stable surface of the current major version, and within that major version no release SHALL remove or rename a stable entry, change its meaning, narrow the values it accepts, or change the exit code a stable command returns for a given outcome. A minor release MAY add stable entries. The stable surface is:

- every non-hidden subcommand, with its flags, positional arguments and accepted values;
- every configuration key, with its type and default;
- the exit-code table;
- the `--json` output of every command that offers `--json`;
- the persisted and wire formats at their declared versions: the session file, the configuration file, the broker's HTTP endpoints and message types, and the manual-approvals log;
- the MCP server's tool names, parameters and result shapes;
- the install paths and documented subcommands of the bundled helper scripts;
- the CLI support-tier definitions and the list of first-class CLIs.

The chapter SHALL state these rules, name each surface class above, and link to the contract manifest that enumerates the entries.

#### Scenario: The stability chapter names every stable surface class

- **WHEN** `docs/src/stability.md` is read
- **THEN** it SHALL name each stable surface class listed in this requirement
- **AND** it SHALL state that within a major version a stable entry is never removed, renamed, narrowed or given a different meaning or exit code, and that minor releases may only add

#### Scenario: The chapter is reachable and builds

- **WHEN** `mdbook build docs/` runs
- **THEN** it SHALL succeed with the stability chapter listed in `SUMMARY.md`

### Requirement: Surfaces outside the contract are declared

The stability chapter SHALL list the surfaces that carry no compatibility promise, and SHALL direct scripts to the stable alternative where one exists. The list SHALL include at least: human-readable stdout and stderr text (scripts use `--json` and exit codes); hidden subcommands (`__dashboard`, `__prompt-profile`, any later `__*` command) and the hidden `selftest`; `GIT_PAW_*` environment overrides that exist for tests (for example `GIT_PAW_DRIVE_POLL_MS`); the dashboard's layout and key bindings; the prose of bundled skills, boot blocks and task prompts; the content of raw session logs; the Rust API of the `git-paw` library crate; and CLIs on the detection list that are not first-class.

#### Scenario: Excluded surfaces are listed

- **WHEN** `docs/src/stability.md` is read
- **THEN** it SHALL list each surface named in this requirement as outside the contract
- **AND** it SHALL point scripts at `--json` and exit codes instead of human-readable output

#### Scenario: A hidden subcommand is not in the stable surface

- **GIVEN** the contract manifest
- **WHEN** its command section is inspected
- **THEN** it SHALL NOT contain `__dashboard`, `__prompt-profile` or `selftest`

### Requirement: The contract manifest is machine-checked

The repository SHALL carry a contract manifest that records the stable surface entry by entry, together with the major version it binds, and the stability chapter SHALL render it. Sections that have a source of truth in code SHALL be generated from it: commands and flags from the clap command definition, configuration keys from the configuration schema, the exit-code table from its definition in code, MCP tools from the server's tool list, broker message types from the wire message type, and first-class CLIs from the support-tier table. A test SHALL regenerate those sections from the running code and SHALL fail when:

- an entry present in the manifest is absent from the live surface while the crate's major version equals the manifest's major version — the failure SHALL name the entry and point at the deprecation policy; or
- the live surface has an entry the manifest does not record — the failure SHALL name the entry, so every addition is recorded in the release that makes it.

A configuration key that is parsed only for compatibility and has no effect SHALL NOT be an entry of the generated configuration section, even if the configuration schema still carries it: the `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` tables (deprecated with no effect in v0.17.0 and dropped by the version-1 to version-2 configuration migration) SHALL appear only in the manifest's deprecations section, as registry entries with no removal release.

#### Scenario: A deprecated no-effect table is listed only under deprecations

- **WHEN** the contract manifest is generated
- **THEN** its configuration section SHALL NOT contain `auto_approve` or `common_dev_allowlist`
- **AND** its deprecations section SHALL list `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]`, each with no removal release

#### Scenario: Removing a stable flag within the major fails

- **GIVEN** a manifest for major version 1 that records `start --dry-run`
- **AND** the crate version is `1.y.z`
- **WHEN** the flag is deleted from the clap definition and the guard test runs
- **THEN** the test SHALL fail naming `start --dry-run` and the deprecation policy

#### Scenario: An unrecorded addition fails until the manifest records it

- **GIVEN** a new flag added to a stable command
- **WHEN** the guard test runs before the manifest is regenerated
- **THEN** the test SHALL fail naming the new flag
- **AND** after the manifest is regenerated to include it, the test SHALL pass

#### Scenario: Removal is permitted at the next major

- **GIVEN** a manifest for major version 1 whose deprecated entry has been removed from the code
- **AND** the crate version is `2.0.0` and the manifest is regenerated for major 2
- **WHEN** the guard test runs
- **THEN** the test SHALL pass

### Requirement: The exit-code table is frozen

Every non-hidden command SHALL exit with one of four codes, and the stability chapter and the CLI reference SHALL publish this table (the codes are settled by v0.24.0's `error-exit-code-contract` and frozen here):

| Code | Meaning | Includes |
|------|---------|----------|
| `0` | The command did what was asked | `--help` and `--version`; the no-op outcomes (`status`, `stop` or `pause` with no session, `purge --stale` with nothing stale); a non-interactive `start` that leaves a detached session; `doctor` whose worst check is ⚠; `mcp` after its client closes stdin; `start --unattended` ending `completed` |
| `1` | The command failed | every error not listed under `2` or `3` (not a git repository, tmux missing, config error, no session to attach, …); `doctor` with at least one ✗ check, with or without `--json`; an error that stops an unattended run's drive loop |
| `2` | Nothing was attempted | arguments rejected by parsing, including clap's own argument errors (an unknown, conflicting or removed flag, a missing value); an interactive prompt cancelled by Ctrl+C or an empty selection; a declined confirmation; a required confirmation that cannot be shown because stdin is not a terminal; a requested item not supported or not enabled for its target (`setup-cli`) |
| `3` | The command ran but stopped short of its goal, reported the outcome, and left nothing half-done | `start --unattended` ending `escalated-for-review`, `stuck`, `heartbeat` or `session-torn-down`, after the summary is printed — a run whose drive loop escalated any permission prompt ends `escalated-for-review`, so it exits `3` even when every branch merged, and a coding agent whose CLI became unavailable is escalated, so its run also ends `escalated-for-review`; `finish` refusing an integration under its merge policy after changing nothing |

Termination by a signal the process does not handle is outside the table. Within a major version a command SHALL NOT change the code it exits with for a given outcome; a new code MAY be introduced only for a new command or flag and SHALL be added to the table in the same release.

#### Scenario: Help and version exit 0

- **WHEN** `git paw --help`, `git paw start --help` or `git paw --version` runs
- **THEN** the process SHALL exit `0`

#### Scenario: A no-op outcome exits 0

- **GIVEN** a repository with no git-paw session
- **WHEN** `git paw stop` runs
- **THEN** the process SHALL exit `0`

#### Scenario: An operational failure exits 1

- **WHEN** `git paw status` runs outside a git repository
- **THEN** the process SHALL exit `1` and stderr SHALL carry the error message

#### Scenario: doctor exits 1 only on a failing check

- **GIVEN** an environment where one doctor check is ✗
- **WHEN** `git paw doctor` runs, and again with `--json`
- **THEN** both runs SHALL exit `1`
- **AND** an environment whose worst check is ⚠ SHALL exit `0`

#### Scenario: An unattended run's exit code follows its outcome

- **GIVEN** an `--unattended` run whose summary outcome is `completed`
- **WHEN** the loop exits
- **THEN** the process SHALL exit `0`
- **AND** a run ending `escalated-for-review`, `stuck`, `heartbeat` or `session-torn-down` SHALL exit `3` after printing the summary
- **AND** a run whose drive loop escalated a permission prompt SHALL exit `3` even when every branch merged
- **AND** a run in which a coding agent's CLI became unavailable SHALL end `escalated-for-review` and exit `3`

#### Scenario: A refused finish exits 3

- **GIVEN** a branch that is not fast-forwardable onto its target under the default `ff-only` policy
- **WHEN** `git paw finish <branch>` runs
- **THEN** the process SHALL exit `3` and the target, the worktree and the session SHALL be unchanged

#### Scenario: A rejected argument exits 2 before any side effect

- **WHEN** `git paw start --no-such-flag` runs
- **THEN** the process SHALL exit `2`, stderr SHALL name the flag, and no worktree, session file or tmux session SHALL be created

#### Scenario: A cancelled prompt exits 2

- **GIVEN** an interactive `git paw start` with no `--cli` and no `--branches`
- **WHEN** the user confirms an empty branch selection
- **THEN** the process SHALL exit `2`

#### Scenario: The published table matches the code

- **WHEN** the exit-code section of the contract manifest is generated from the code
- **THEN** it SHALL list exactly the codes `0`, `1`, `2` and `3` with the meanings in this requirement
- **AND** `docs/src/cli-reference.md` and `docs/src/stability.md` SHALL present the same table

### Requirement: Serialized formats evolve additively within a major version

Within a major version, a release SHALL load, without error and without requiring a migration step, every configuration file, session file and manual-approvals log written by an earlier release of the same major version, and SHALL ignore keys or fields it does not recognise rather than failing. New keys and fields SHALL be optional with a documented default. A change that cannot meet this SHALL bump the format's declared version and keep reading the previous version for the rest of the major.

#### Scenario: A configuration written by an earlier minor loads

- **GIVEN** a `.git-paw/config.toml` written by release `1.0.0`
- **WHEN** a later `1.y` release loads it
- **THEN** loading SHALL succeed with the same effective settings

#### Scenario: An unknown field is ignored

- **GIVEN** a session file containing a field that the running release does not define
- **WHEN** the session is loaded
- **THEN** loading SHALL succeed and the unknown field SHALL be ignored

### Requirement: Support tiers are frozen

The stable surface SHALL include the three CLI support-tier definitions — **first-class** (every worker-lifecycle step verified, with recorded fixtures in CI), **supported** (works, with its gaps documented) and **launch-only** (git-paw launches the CLI and nothing more) — and the list of first-class CLIs with the CLI version each was verified against. Within a major version, a release SHALL NOT remove a CLI from the first-class list or weaken a tier definition. A release MAY add first-class CLIs, move CLIs between supported and launch-only, and change the detection list, recording the change in the changelog.

#### Scenario: The first-class list is in the manifest

- **WHEN** the contract manifest is generated
- **THEN** its support-tier section SHALL list every first-class CLI from the support-tier table, each with its verified version

#### Scenario: Demoting a first-class CLI within the major fails

- **GIVEN** a manifest for major version 1 that lists a CLI as first-class
- **WHEN** that CLI's tier is lowered in code and the guard test runs under a `1.y` version
- **THEN** the test SHALL fail naming the CLI

### Requirement: The coordinated-session vocabulary is frozen

The stable surface SHALL include `start --coordinated`, `start --bare`, the top-level `default_mode` key, and the `[orchestrator]` (with `[orchestrator.gates]` and `[orchestrator.escalation]`), `[workers]` and `[drive]` configuration tables and their keys, and the broker's root identity `orchestrator`, with this meaning: a *coordinated session* is one run with git-paw's coordination — the broker and dashboard, boot prompts, the conflict detector and learnings, the drive loop when `--unattended`, and git-paw's hand-off of judgment calls to the orchestrator; the *orchestrator* is the external agent session that drives the session and makes judgment calls; a *worker* is a coding agent in a pane; the *drive loop* is git-paw's own mechanical loop. The stability chapter SHALL define these four terms. No stable entry SHALL contain the word "supervisor": the `[supervisor]` table of version-1 config files is read through migration, appears only in the manifest's deprecations section, and is not a stable entry.

#### Scenario: The coordinated-session entries are stable

- **WHEN** the contract manifest is generated
- **THEN** its command section SHALL contain `start --coordinated` and `start --bare`
- **AND** its configuration section SHALL contain `default_mode` and the `[orchestrator]`, `[workers]` and `[drive]` keys

#### Scenario: No supervisor entry is frozen

- **WHEN** the contract manifest is generated
- **THEN** no entry outside its deprecations section SHALL contain `supervisor`

#### Scenario: The vocabulary is explained

- **WHEN** `docs/src/stability.md` is read
- **THEN** it SHALL define *coordinated session*, *orchestrator*, *worker* and *drive loop* as stated in this requirement

### Requirement: A stable item is deprecated before it is removed

A stable entry SHALL be removed only in a new major version and only after it has been deprecated. A deprecated entry SHALL keep working unchanged for the rest of its major version, and SHALL be deprecated for at least two minor releases before the major that removes it. Every deprecation SHALL be recorded in the deprecation registry (`core-deprecation`) — the item, its kind, the version that deprecated it, its replacement, and its removal release — and an entry deprecated in a 1.x release SHALL name a later major version (`X.0.0`) as its removal release or none. The registry SHALL be rendered in the stability chapter and recorded in the contract manifest, and its warnings SHALL come from the registry's single helper. The changelog of the release that deprecates an item SHALL name it.

#### Scenario: A removal at the next major must have been deprecated

- **GIVEN** a stable entry absent from the code under a new major version
- **WHEN** the manifest guard test runs
- **THEN** it SHALL fail unless the entry appears in the previous major's deprecation registry

#### Scenario: A deprecated entry keeps working

- **GIVEN** a flag recorded in the deprecation registry
- **WHEN** it is used in a later minor release of the same major
- **THEN** the command SHALL behave as before the deprecation

#### Scenario: The registry is published

- **WHEN** `docs/src/stability.md` is read
- **THEN** it SHALL list every registry entry with its deprecating version, replacement and removal release

#### Scenario: A 1.x deprecation schedules its removal at a major

- **GIVEN** an entry added to the registry in a `1.y` release
- **WHEN** its removal release is `1.z.0` with `z` greater than `y`
- **THEN** the manifest guard SHALL fail naming the entry and the deprecation policy
- **AND** with removal release `2.0.0`, or none, it SHALL pass

### Requirement: Pre-1.0 deprecations are resolved at the freeze

Release 1.0.0 SHALL contain no registry entry whose removal release is `1.0.0` or earlier: every flag, skill name, wire value and output field that a 0.x release deprecated for removal in 1.0.0 SHALL be removed in 1.0.0. The registry recorded by the first 1.x manifest SHALL hold only entries that are read through a format migration with no removal release. The stability chapter SHALL list each item removed in 1.0.0 with its replacement.

#### Scenario: The 1.0.0 registry has no overdue entry

- **WHEN** the deprecation registry is read at crate version `1.0.0`
- **THEN** no entry SHALL have a removal release of `1.0.0` or earlier

#### Scenario: The removals are listed with their replacements

- **WHEN** `docs/src/stability.md` is read
- **THEN** it SHALL list `--supervisor` → `--coordinated`, `--no-supervisor` → `--bare`, the `supervisor` skill name → `orchestrator`, the broker root identity `supervisor` → `orchestrator`, `supervisor.verify-now` → `orchestrator.verify-now`, MCP `mode` → `session_mode`, and `--from-specs` → `--from-all-specs`

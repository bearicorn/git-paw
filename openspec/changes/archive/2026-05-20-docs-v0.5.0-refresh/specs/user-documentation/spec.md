## ADDED Requirements

### Requirement: README features section enumerates v0.5.0 user-facing surface

`README.md`'s Features section SHALL include entries describing
every user-facing v0.5.0 capability so users adopting v0.5.0 can
discover the feature set from a single page. The Features list
SHALL include (at minimum) entries for: `--specs-format`,
`--no-supervisor`, `start --force`, the Spec Kit backend
(`[specs] type = "speckit"`), the `[governance]` config table,
the `[supervisor.conflict]` config table, the
`[supervisor.auto_approve]` config table, the
`[supervisor.learnings_config]` config table, `agent.intent`
forward coordination, automatic conflict detection
(forward / in-flight / ownership), and learnings mode (the
`.git-paw/session-learnings.md` output file).

#### Scenario: README Features section mentions Spec Kit

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `Spec Kit` (case-insensitive)
- **AND** it contains the substring `speckit` (the TOML value)

#### Scenario: README Features section mentions forward coordination and conflict detection

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `agent.intent`
- **AND** it contains the substring `conflict detection` (case-insensitive)

#### Scenario: README Features section mentions learnings mode

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `learnings` (case-insensitive)
- **AND** it contains the substring `.git-paw/session-learnings.md`

#### Scenario: README Features section mentions every v0.5.0 CLI flag

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `--specs-format`
- **AND** it contains the substring `--no-supervisor`
- **AND** it contains the substring `--force` (in a `start --force` context)

### Requirement: README documents `[specs] type` accepts all three backends

The README's configuration excerpt for the `[specs]` section SHALL
document `type` as accepting `"openspec"`, `"markdown"`, AND
`"speckit"`. The previous v0.4 listing of only `"openspec"` and
`"markdown"` SHALL be replaced.

#### Scenario: README specs example lists all three backends

- **WHEN** the README's `[specs]` configuration excerpt is inspected
- **THEN** it contains the substrings `"openspec"`, `"markdown"`, AND `"speckit"` as documented values of `type`

### Requirement: README Supervisor Mode quick start documents v0.5.0 flags

The README's "Quick Start: Supervisor Mode" section SHALL
document the `--no-supervisor` opt-out flag and the
`start --force` flag for bypassing the uncommitted-spec validation
warning. Each flag SHALL appear at least once in a command-line
example within the section.

#### Scenario: Quick start supervisor mentions --no-supervisor

- **WHEN** the README's Quick Start: Supervisor Mode section is inspected
- **THEN** it contains the substring `--no-supervisor`

#### Scenario: Quick start supervisor mentions --force

- **WHEN** the README's Quick Start: Supervisor Mode section is inspected
- **THEN** it contains the substring `--force` (in the context of `start --force`)

### Requirement: README Supported AI CLIs table matches `src/detect.rs`

The README's Supported AI CLIs table SHALL list every CLI defined
in `src/detect.rs`. As of v0.5.0 that count is 10 entries; the v0.4
table of 7 entries SHALL be expanded to include `opencode`,
`cline`, and `droid` (plus any further additions present in
`src/detect.rs` at archive time).

#### Scenario: README CLI table mentions opencode, cline, and droid

- **WHEN** the README's Supported AI CLIs table is inspected
- **THEN** it contains the substring `opencode`
- **AND** it contains the substring `cline`
- **AND** it contains the substring `droid`

### Requirement: AGENTS.md dependency table matches `Cargo.toml`

`AGENTS.md`'s approved-dependencies table SHALL list every
production dependency declared in `[dependencies]` of `Cargo.toml`
at archive time and every dev dependency declared in
`[dev-dependencies]`. The v0.5.0 production additions SHALL be
present: `schemars`, `serde_yaml`, `chrono`, and `regex` (with
appropriate version suffixes matching the manifest).

#### Scenario: AGENTS.md table lists v0.5.0 prod dependencies

- **WHEN** the AGENTS.md Dependencies table is inspected
- **THEN** it contains the substring `schemars`
- **AND** it contains the substring `serde_yaml`
- **AND** it contains the substring `chrono`
- **AND** it contains the substring `regex`

### Requirement: AGENTS.md documents the `dirs` crate removal

`AGENTS.md` SHALL NOT list the upstream `dirs` crate as an
approved production dependency. Instead, AGENTS.md SHALL include a
paragraph under the Dependencies section explaining that the
`dirs` crate was removed in v0.5.0 because its transitive license
chain fails `just deny`, and that the project now uses an
in-tree `src/dirs.rs` module for platform XDG paths. The
paragraph SHALL instruct future contributors NOT to re-add the
`dirs` crate.

#### Scenario: AGENTS.md does not list dirs as an approved dep

- **WHEN** the AGENTS.md Dependencies table is inspected
- **THEN** the table does NOT contain `dirs` as an approved
  production dependency row

#### Scenario: AGENTS.md explains the dirs swap

- **WHEN** the AGENTS.md Dependencies section is inspected
- **THEN** it contains text describing the `dirs` crate's removal
  and its replacement by `src/dirs.rs`
- **AND** it instructs contributors not to re-add the `dirs` crate

### Requirement: AGENTS.md scopes list covers v0.5.0 scopes and compound forms

`AGENTS.md`'s Commit Conventions section's scope enumeration SHALL include the scopes used in shipped v0.5.0 commits. At minimum the scope list SHALL include: `user-guide`, `worktree`, `governance`, `learnings`, and `pause`. The section SHALL explicitly document compound scopes of the form `<a>,<b>,<c>` (comma-separated scopes inside the parentheses) as permitted.

#### Scenario: AGENTS.md scopes list mentions v0.5.0 scopes

- **WHEN** the AGENTS.md Commit Conventions scope list is inspected
- **THEN** it contains the substring `user-guide`
- **AND** it contains the substring `worktree`
- **AND** it contains the substring `governance`
- **AND** it contains the substring `learnings`

#### Scenario: AGENTS.md documents compound scopes

- **WHEN** the AGENTS.md Commit Conventions section is inspected
- **THEN** it contains an example or explicit statement permitting
  compound scope form `(<a>,<b>,...)`
- **AND** the example uses commas (no whitespace between scopes is
  required, but the comma separator is explicit)

### Requirement: mdBook architecture chapter has an accurate module list

`docs/src/architecture.md` SHALL describe every Rust module that
ships in `src/` at archive time. The chapter SHALL NOT reference
modules that do not exist in `src/`. Specifically, the chapter
SHALL NOT contain the substrings `src/broker/state.rs` or
`src/broker/flush.rs` (modules that were referenced in v0.4 docs
but do not exist in the v0.5.0 source tree). The chapter SHALL
include references to the v0.5.0 module additions
(`src/supervisor/`, `src/broker/conflict.rs`,
`src/broker/learnings.rs`, `src/broker/watcher.rs`,
`src/broker/delivery.rs`, `src/broker/publish.rs`,
`src/specs/resolve.rs`, `src/specs/speckit.rs`).

#### Scenario: Architecture chapter does NOT reference nonexistent broker modules

- **WHEN** `docs/src/architecture.md` is inspected
- **THEN** it does NOT contain the substring `src/broker/state.rs`
- **AND** it does NOT contain the substring `src/broker/flush.rs`

#### Scenario: Architecture chapter references v0.5.0 module additions

- **WHEN** `docs/src/architecture.md` is inspected
- **THEN** it contains the substring `src/broker/conflict.rs`
- **AND** it contains the substring `src/broker/learnings.rs`
- **AND** it contains the substring `src/specs/speckit.rs`
- **AND** it contains a reference to the `src/supervisor/` subtree

### Requirement: Architecture chapter pins the supervisor-as-pane layout

`docs/src/architecture.md` SHALL describe the supervisor-mode
tmux layout established by the `supervisor-as-pane` archive:
pane 0 is the supervisor, pane 1 is the dashboard, and the agent
panes occupy indices 2 onwards in a row-major grid below the top
row. The chapter SHALL NOT describe the v0.4 layout (dashboard
at pane 0) as the current layout.

#### Scenario: Architecture chapter places supervisor at pane 0

- **WHEN** the supervisor-mode layout description in `architecture.md` is inspected
- **THEN** it states that the supervisor is at pane 0
- **AND** it states that the dashboard is at pane 1
- **AND** it does NOT state that the dashboard is at pane 0 as the v0.5 default

### Requirement: mdBook changelog chapter includes the project changelog

`docs/src/changelog.md` SHALL render the contents of the
project's root `CHANGELOG.md` rather than maintaining a separate
copy. The chapter file SHALL contain the mdBook
`{{#include ../../CHANGELOG.md}}` directive (or equivalent
preprocessor directive) so that future `git cliff` regenerations
of `CHANGELOG.md` automatically flow through to the rendered
mdBook output.

#### Scenario: Changelog chapter is an include of the root CHANGELOG.md

- **WHEN** `docs/src/changelog.md` is inspected
- **THEN** it contains the substring `{{#include ../../CHANGELOG.md}}`

#### Scenario: Changelog chapter does NOT hand-maintain `[Unreleased]` content

- **WHEN** `docs/src/changelog.md` is inspected
- **THEN** it does NOT contain a hand-maintained `[Unreleased]` section header (the rendered output is sourced entirely from the included file)

### Requirement: Quick Start Supervisor chapter is internally consistent

`docs/src/quick-start-supervisor.md` SHALL describe a single,
consistent pane layout throughout. The chapter SHALL NOT contain
contradictory statements about which pane is the supervisor and
which is the dashboard. The canonical layout for v0.5.0 is:
supervisor at pane 0, dashboard at pane 1, agent panes at
indices 2 onwards.

#### Scenario: Quick start supervisor chapter is internally consistent on pane indices

- **WHEN** `docs/src/quick-start-supervisor.md` is inspected
- **THEN** every reference to the supervisor pane resolves to pane index 0
- **AND** every reference to the dashboard pane resolves to pane index 1
- **AND** the chapter does NOT contain any sentence stating that the dashboard is at pane 0 in v0.5 supervisor mode

### Requirement: Quick Start Supervisor chapter does not reference nonexistent broker messages

`docs/src/quick-start-supervisor.md` SHALL NOT reference broker
message types that do not exist in `src/broker/messages.rs`.
Specifically the chapter SHALL NOT contain the substrings
`agent.register` or `agent.done` as broker message variants.

#### Scenario: Quick start supervisor chapter does not mention agent.register

- **WHEN** `docs/src/quick-start-supervisor.md` is inspected
- **THEN** it does NOT contain the substring `agent.register`

#### Scenario: Quick start supervisor chapter does not mention agent.done

- **WHEN** `docs/src/quick-start-supervisor.md` is inspected
- **THEN** it does NOT contain the substring `agent.done`

### Requirement: Quick Start Supervisor chapter reflects shipped v0.5.0 features

`docs/src/quick-start-supervisor.md` SHALL NOT advertise as
"not yet supported" any feature that has shipped in v0.5.0. The
v0.4-era "What's NOT Yet Supported in v0.4.0" section listing
conflict detection and learnings mode as deferred SHALL be
removed or rewritten so that v0.5.0 readers see those features as
shipped.

#### Scenario: Quick start supervisor chapter does not mark conflict detection as deferred

- **WHEN** the chapter is inspected
- **THEN** the substrings `conflict detection` and `learnings`
  do not appear inside a section that describes them as not yet
  supported in v0.5.0

### Requirement: Coordination chapter uses the v0.5 wire envelope shape

`docs/src/user-guide/coordination.md` SHALL document the broker
wire format using the canonical envelope
`{"type": "agent.<variant>", "agent_id": "<slug>", "payload": {...}}`
as defined in `src/broker/messages.rs`. The chapter SHALL NOT
document the legacy v0.2 shape `{"agent_id": ..., "kind": ...,
"body": ...}`. Every `agent_id` in an example SHALL be slug-valid
(no slashes, no whitespace, lowercase alphanumeric + hyphen).

#### Scenario: Coordination chapter uses the canonical envelope

- **WHEN** `docs/src/user-guide/coordination.md` is inspected
- **THEN** it contains the substring `"type": "agent.`
- **AND** it contains the substring `"payload":`

#### Scenario: Coordination chapter does not use the legacy kind/body shape

- **WHEN** `docs/src/user-guide/coordination.md` is inspected
- **THEN** it does NOT contain a JSON example using the
  `"kind":` field as the variant discriminator
- **AND** it does NOT contain a JSON example using the `"body":`
  field as the payload carrier

#### Scenario: Coordination chapter uses slug-valid agent IDs in examples

- **WHEN** `docs/src/user-guide/coordination.md` is inspected
- **THEN** every example `agent_id` value contains only
  lowercase alphanumeric characters, digits, and hyphens
- **AND** no example `agent_id` contains a `/` character

### Requirement: Coordination chapter documents every v0.5 message variant

`docs/src/user-guide/coordination.md` SHALL provide a wire-form
example for each of the seven shipped broker message variants
(`agent.status`, `agent.artifact`, `agent.blocked`, `agent.intent`,
`agent.question`, `agent.feedback`, `agent.verified`). The
chapter SHALL note explicitly that `agent.status` and
`agent.artifact` are published automatically (by the filesystem
watcher and the post-commit git hook respectively) and that
manual `curl` invocations for those variants are escape hatches.

#### Scenario: Coordination chapter has examples for forward-coordination variants

- **WHEN** the chapter is inspected
- **THEN** it contains a wire-form example for `agent.intent`
- **AND** it contains a wire-form example for `agent.question`
- **AND** it contains a wire-form example for `agent.feedback`
- **AND** it contains a wire-form example for `agent.verified`

#### Scenario: Coordination chapter notes automatic publishing for status and artifact

- **WHEN** the chapter is inspected
- **THEN** it states that `agent.status` is published automatically by the filesystem watcher
- **AND** it states that `agent.artifact` is published automatically by the post-commit git hook

### Requirement: Spec-driven launch chapter uses the canonical flag name

`docs/src/user-guide/spec-driven-launch.md` SHALL use
`--from-all-specs` as the canonical flag name in every example.
The hidden v0.4 alias `--from-specs` SHALL NOT appear in
example command lines in the chapter (per the
`cross-format-spec-selection` archive's documentation policy,
the alias is intentionally undocumented in v0.5.0 to nudge
migration).

#### Scenario: Spec-driven launch chapter uses --from-all-specs

- **WHEN** `docs/src/user-guide/spec-driven-launch.md` is inspected
- **THEN** every example command launching all discovered specs
  uses `--from-all-specs`
- **AND** no example command line uses `--from-specs`

### Requirement: Spec-driven launch chapter documents the Spec Kit backend

`docs/src/user-guide/spec-driven-launch.md` SHALL include a
section documenting the Spec Kit backend. The section SHALL
cover:

1. The `[specs] type = "speckit"` configuration value.
2. The auto-detection rule: when `.specify/` exists at the
   repository root and no `[specs]` configuration is set, the
   system defaults to `type = "speckit"` and
   `dir = ".specify/specs"`.
3. A minimal worked example showing how `[P]` markers in
   `tasks.md` decompose into per-task worktrees and how
   non-`[P]` tasks consolidate into a single `phase/...`
   worktree.
4. A reference to the constitution auto-wiring into
   `[governance]` (one sentence; the detail lives in
   `configuration/README.md#governance` or similar).

#### Scenario: Spec-driven launch chapter describes Spec Kit auto-detection

- **WHEN** the Spec Kit section is inspected
- **THEN** it contains the substring `.specify/` (the directory git-paw probes)
- **AND** it documents the auto-detection behaviour

#### Scenario: Spec-driven launch chapter explains [P] decomposition

- **WHEN** the Spec Kit section is inspected
- **THEN** it explains that `[P]` markers in `tasks.md`
  decompose into per-task worktrees
- **AND** it explains that non-`[P]` tasks consolidate into a
  single `phase/...` worktree

### Requirement: AGENTS.md user-guide chapter reflects boot-prompt-full-body

`docs/src/user-guide/agents-md.md` SHALL describe AGENTS.md as
the source of truth for the spec body and SHALL state that the
supervisor-mode boot prompt points the agent at AGENTS.md plus
`openspec/changes/<id>/` rather than embedding the spec body in
the boot prompt. The chapter SHALL NOT describe AGENTS.md as
containing only "Branch + CLI + Spec content + Owned files"
(the v0.4 framing); it SHALL describe AGENTS.md as the full spec
artifact target.

#### Scenario: agents-md chapter describes the boot-prompt-full-body model

- **WHEN** `docs/src/user-guide/agents-md.md` is inspected
- **THEN** it states that AGENTS.md is the source of truth for the spec body
- **AND** it states that the supervisor-mode boot prompt points at AGENTS.md and `openspec/changes/<id>/`

### Requirement: Dashboard chapter reflects v0.5.0 supervisor-as-pane state

`docs/src/user-guide/dashboard.md` SHALL describe the dashboard
in its v0.5.0 state: it lives in pane 1 of supervisor sessions,
shows the agents status table, and does NOT include an
interactive prompt inbox panel. The chapter SHALL NOT contain
forward-looking statements claiming that v0.4 (or later) will add
features that have since either shipped and been removed
(prompt inbox) or shipped already (conflict detection,
learnings mode).

#### Scenario: Dashboard chapter does not promise v0.4 prompt inbox

- **WHEN** `docs/src/user-guide/dashboard.md` is inspected
- **THEN** it does NOT contain forward-looking text claiming v0.4 will add an interactive prompt inbox panel

#### Scenario: Dashboard chapter places the dashboard at pane 1 in supervisor mode

- **WHEN** `docs/src/user-guide/dashboard.md` is inspected
- **THEN** any reference to the dashboard's pane location in supervisor mode states that it is at pane 1

### Requirement: Configuration reference documents the v0.5 specs type value

`docs/src/configuration/README.md`'s commented `[specs]` example SHALL document `type` as accepting `"openspec"`, `"markdown"`, AND `"speckit"`. The v0.4-era example listing only `"openspec"` and `"markdown"` SHALL be updated to add `"speckit"`.

#### Scenario: Configuration reference lists all three specs.type values

- **WHEN** the `[specs]` example in `docs/src/configuration/README.md` is inspected
- **THEN** it documents `"speckit"` as a valid value alongside `"openspec"` and `"markdown"`

### Requirement: Configuration reference has no dangling internal links

`docs/src/configuration/README.md` SHALL NOT contain Markdown
links pointing at anchors that do not exist in their target
files. Specifically the link previously pointing at
`coordination.md#automatic-conflict-detection-v050` SHALL either
target a real anchor that exists in `coordination.md` post-refresh
or be removed.

#### Scenario: Configuration reference has no broken conflict-detection anchor

- **WHEN** `docs/src/configuration/README.md` is inspected
- **THEN** any `[...]( ... )` link targeting `coordination.md`
  points at an anchor that exists in `coordination.md` (verified
  by the anchor heading being present in the target file's
  rendered TOC)

### Requirement: Configuration reference documents new v0.5 tables

`docs/src/configuration/README.md` SHALL include subsections
documenting `[supervisor.conflict]`,
`[supervisor.auto_approve]`, `[supervisor.learnings_config]`, and
`[governance]`. Each subsection SHALL list every field, its type,
its default value (where applicable), and a one-line description.

#### Scenario: Configuration reference documents supervisor.conflict

- **WHEN** `docs/src/configuration/README.md` is inspected
- **THEN** it contains a subsection or table for `[supervisor.conflict]`
- **AND** the subsection mentions `window_seconds`, `warn_on_intent_overlap`, and `escalate_on_violation`

#### Scenario: Configuration reference documents supervisor.auto_approve

- **WHEN** `docs/src/configuration/README.md` is inspected
- **THEN** it contains a subsection or table for `[supervisor.auto_approve]`
- **AND** the subsection mentions `approval_level` and `safe_commands`

#### Scenario: Configuration reference documents supervisor.learnings_config

- **WHEN** `docs/src/configuration/README.md` is inspected
- **THEN** it contains a subsection or table for `[supervisor.learnings_config]`
- **AND** the subsection mentions `flush_interval_seconds`

#### Scenario: Configuration reference documents governance

- **WHEN** `docs/src/configuration/README.md` is inspected
- **THEN** it contains a subsection or table for `[governance]`
- **AND** the subsection lists all five fields: `adr`, `test_strategy`, `security`, `dod`, `constitution`

### Requirement: CLI reference lists every top-level subcommand

`docs/src/cli-reference.md`'s top-level commands list SHALL
include every public subcommand. As of v0.5.0 the list SHALL
include (at minimum) `start`, `stop`, `purge`, `status`,
`list-clis`, `add-cli`, `remove-cli`, `init`, and `replay`. The
v0.4 omission of `init` and `replay` SHALL be repaired.

#### Scenario: CLI reference lists init

- **WHEN** the top-level commands section of `docs/src/cli-reference.md` is inspected
- **THEN** it contains the substring `init`

#### Scenario: CLI reference lists replay

- **WHEN** the top-level commands section of `docs/src/cli-reference.md` is inspected
- **THEN** it contains the substring `replay`

### Requirement: CLI reference start-flag table lists every v0.5 flag

`docs/src/cli-reference.md`'s start-flag table SHALL include
every flag defined in `src/cli.rs::StartArgs`. As of v0.5.0 the
table SHALL include (at minimum) `--cli`, `--branches`,
`--dry-run`, `--preset`, `--from-all-specs`, `--specs`,
`--specs-format`, `--supervisor`, `--no-supervisor`, and
`--force`.

#### Scenario: CLI reference includes --specs-format

- **WHEN** the start-flag table in `docs/src/cli-reference.md` is inspected
- **THEN** it contains the substring `--specs-format`

#### Scenario: CLI reference includes --no-supervisor

- **WHEN** the start-flag table in `docs/src/cli-reference.md` is inspected
- **THEN** it contains the substring `--no-supervisor`

#### Scenario: CLI reference includes --from-all-specs and --specs

- **WHEN** the start-flag table in `docs/src/cli-reference.md` is inspected
- **THEN** it contains the substring `--from-all-specs`
- **AND** it contains the substring `--specs` (as a distinct flag, not as a prefix of `--specs-format`)

#### Scenario: CLI reference includes --force

- **WHEN** the start-flag table in `docs/src/cli-reference.md` is inspected
- **THEN** it contains the substring `--force`

### Requirement: User guide includes a Learnings Mode chapter

The mdBook user guide SHALL include a chapter at
`docs/src/user-guide/learnings.md` documenting learnings mode.
The chapter SHALL cover the opt-in `[supervisor] learnings = true`
flag, the location and append-only shape of
`.git-paw/session-learnings.md`, and the five deterministic
categories tracked in v0.5.0 (stuck duration, recovery-cycle
count, forward conflicts, in-flight conflicts, ownership
violations). The chapter SHALL state that the broker
`agent.learning` wire variant is deferred to v0.6.0 and that
v0.5.0 ships file-only output.

The chapter SHALL be linked from `docs/src/SUMMARY.md` under the
User Guide group.

#### Scenario: Learnings chapter exists and is linked

- **WHEN** `docs/src/SUMMARY.md` is inspected
- **THEN** it contains a link to `user-guide/learnings.md` under the User Guide section

#### Scenario: Learnings chapter documents the opt-in flag

- **WHEN** `docs/src/user-guide/learnings.md` is inspected
- **THEN** it contains the substring `[supervisor]` and references the `learnings` flag
- **AND** it states the default value is `false` (or equivalent — "opt-in")

#### Scenario: Learnings chapter names the output file

- **WHEN** `docs/src/user-guide/learnings.md` is inspected
- **THEN** it contains the substring `.git-paw/session-learnings.md`

#### Scenario: Learnings chapter enumerates the deterministic categories

- **WHEN** `docs/src/user-guide/learnings.md` is inspected
- **THEN** it mentions stuck duration (or "where agents got stuck")
- **AND** it mentions recovery-cycle count (or "recovery cycles")
- **AND** it mentions forward conflicts
- **AND** it mentions in-flight conflicts
- **AND** it mentions ownership violations

#### Scenario: Learnings chapter defers `agent.learning` to v0.6.0

- **WHEN** `docs/src/user-guide/learnings.md` is inspected
- **THEN** it states that the `agent.learning` broker variant (or programmatic access) is deferred to v0.6.0

### Requirement: User guide includes a Conflict Detection chapter

The mdBook user guide SHALL include a chapter at
`docs/src/user-guide/conflict-detection.md` documenting the
broker's automatic conflict detection. The chapter SHALL cover
the three failure shapes (forward, in-flight, ownership), the
`[conflict-detector]` tag prefix on auto-emitted feedback, the
supervisor inbox routing for `agent.question` escalations, and
how detection interacts with the filesystem watcher's
auto-published `modified_files`.

The chapter SHALL be linked from `docs/src/SUMMARY.md` under the
User Guide group.

#### Scenario: Conflict detection chapter exists and is linked

- **WHEN** `docs/src/SUMMARY.md` is inspected
- **THEN** it contains a link to `user-guide/conflict-detection.md` under the User Guide section

#### Scenario: Conflict detection chapter describes the three failure shapes

- **WHEN** `docs/src/user-guide/conflict-detection.md` is inspected
- **THEN** it explains forward conflicts (overlapping `agent.intent`)
- **AND** it explains in-flight conflicts (overlapping `agent.status.modified_files`)
- **AND** it explains ownership violations

#### Scenario: Conflict detection chapter mentions the tag prefix

- **WHEN** `docs/src/user-guide/conflict-detection.md` is inspected
- **THEN** it contains the substring `[conflict-detector]`

#### Scenario: Conflict detection chapter documents supervisor inbox routing

- **WHEN** `docs/src/user-guide/conflict-detection.md` is inspected
- **THEN** it states that `agent.question` escalations are routed to the supervisor inbox

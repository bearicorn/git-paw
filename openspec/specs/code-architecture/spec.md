# code-architecture Specification

## Purpose
The enforceable structural and regression contract every behavior-preserving refactor must uphold: the observable public surface (CLI, config TOML, broker wire) stays byte-identical, process/tmux/git invocation is reachable through an injectable `CommandRunner` seam, injection-prone strings flow through a single newtype construction point, the codebase is organized into cohesive domain modules with preserved re-exports, the frozen serde/wire/lock/SIGHUP surfaces are never touched, and structural refactors land in test-gated waves. It keeps "refactor" strictly behavior-preserving and distinct from behavior changes across the pre-v1.0.0 hardening.
## Requirements
### Requirement: Refactors preserve the observable public surface

A behavior-preserving refactor SHALL leave the public CLI surface, the config TOML schema, the
broker wire format, and on-disk session state **byte-identical** before and after. This SHALL be
verified by running the existing behavioral test suite against the merge-base before and after each
refactor wave; a wave SHALL NOT land if any observable output (exit codes, stdout/stderr,
`session.json`, config round-trip bytes, broker envelope bytes, tmux panes) differs.

#### Scenario: Behavioral suite is unchanged after a wave

- **GIVEN** a completed refactor wave
- **WHEN** the existing behavioral suite runs against the merge-base
- **THEN** exit codes, stdout/stderr, `session.json`, and created tmux panes SHALL be unchanged
- **AND** any coverage drop on a real branch SHALL be treated as a cut sole-guard and restored

#### Scenario: Config round-trips byte-identically

- **GIVEN** an existing v0.2.0-onward `.git-paw/config.toml`
- **WHEN** it is loaded and re-serialized after a config-module refactor
- **THEN** the emitted TOML bytes SHALL be identical to the pre-refactor output

#### Scenario: Broker envelope round-trips byte-equivalent

- **GIVEN** a `BrokerMessage` fixture on the frozen wire
- **WHEN** it is deserialized and re-serialized after a refactor
- **THEN** the emitted JSON bytes SHALL be byte-equivalent to the pre-refactor output

### Requirement: Process, tmux, and git invocation is reachable through an injectable seam

Process execution on the orchestration path SHALL depend on an injectable `CommandRunner` seam
rather than calling `std::process::Command` inline in logic. Production SHALL wire a real runner
whose behavior is identical to the pre-refactor inline calls; tests SHALL be able to inject a fake
runner that records the invoked argv and returns scripted output, so tmux/git orchestration is
unit-testable without spawning a real process. The `src/git.rs` git module SHALL route its git
invocations through this seam via `*_with(runner, …)` functions whose public wrappers delegate with
the real runner, so every caller compiles unchanged and production git behavior is byte-identical.

#### Scenario: A tmux-orchestration unit test asserts argv without spawning a process

- **GIVEN** the `CommandRunner` seam and a fake runner injected into a command handler
- **WHEN** a tmux-orchestration unit test runs
- **THEN** it SHALL assert the tmux/git argv the handler would invoke
- **AND** no real tmux or git process SHALL be spawned

#### Scenario: Production behavior is unchanged by the seam

- **GIVEN** the production real `CommandRunner`
- **WHEN** an orchestration command runs end-to-end
- **THEN** the external tmux/git calls SHALL be identical to the pre-refactor inline invocations

#### Scenario: A git-operation unit test asserts git argv without spawning git

- **GIVEN** a `src/git.rs` function routed through the seam and a `FakeCommandRunner` injected via its `*_with(runner, …)` entry point
- **WHEN** a git-operation unit test runs
- **THEN** it SHALL assert the exact `git` argv (subcommand, flags, path/branch operands) the function builds
- **AND** it SHALL drive the function's success/failure branches from the fake's scripted exit status and stdout/stderr
- **AND** no real `git` process SHALL be spawned

### Requirement: Injection-prone strings flow through a single newtype construction point

Injection-prone values — the tmux session name, branch slug, and worktree path — SHALL be
constructed through domain newtypes (`SessionName`, `BranchSlug`, `WorktreePath`) at a single
point rather than assembled by scattered inline `format!` calls. In this behavior-preserving
change the newtype output SHALL be byte-identical to the current inline formatting for every
current input (no sanitization is added here — that hardening ships as a separate versioned
change); the newtype establishes the construction seam.

#### Scenario: Newtype output matches the pre-refactor formatting

- **GIVEN** an input that today produces a session name / branch slug / worktree path via inline `format!`
- **WHEN** the value is constructed through its newtype
- **THEN** the resulting string SHALL be byte-identical to the pre-refactor output

#### Scenario: Construction is centralized

- **GIVEN** the newtype seam
- **WHEN** the codebase is searched for the injection-prone value's construction
- **THEN** it SHALL be produced only through the newtype constructor, not re-assembled inline elsewhere

### Requirement: The codebase is organized into domain modules with preserved re-exports

The codebase SHALL be organized into domain modules — `commands/` (one file per command family with
a thin `main`/`run` dispatch), `config/` (per-section files), `tmux/{command,session,readiness,layout}`,
and the existing `broker/`, `supervisor/`, `specs/`, `mcp/`, `dashboard/`, and
`interactive/{resolver,prompter,picker}` domains. Every module split
SHALL preserve the public re-exports at the pre-split paths (e.g. `crate::config::*`, `crate::tmux::*`)
so the binary crate and the integration-test crates compile unchanged, and SHALL keep the
`BrokerMessage` and `SpecBackendKind` exhaustive `match`es co-located so a missed variant remains a
compile error.

#### Scenario: Re-exports keep external callers compiling

- **GIVEN** a `config.rs → config/` or `tmux.rs → tmux/` split
- **WHEN** the binary crate and the `assert_cmd` integration tests are compiled
- **THEN** all `crate::config::*` / `crate::tmux::*` paths SHALL still resolve and the crates SHALL compile unchanged

#### Scenario: main.rs dispatch behavior is unchanged after the split

- **GIVEN** `main.rs` split into `src/commands/*.rs` with a thin dispatch
- **WHEN** the CLI e2e suite runs
- **THEN** exit codes, stdout, and `session.json` SHALL match the pre-split behavior

#### Scenario: Enum-ripple matches stay compiler-caught

- **GIVEN** a module split that touches a file containing a `BrokerMessage` or `SpecBackendKind` exhaustive `match`
- **WHEN** the crate is compiled
- **THEN** the exhaustive `match`es SHALL remain co-located such that omitting a variant is a compile error, not a silently inert branch

#### Scenario: Interactive selection split preserves prompt behavior

- **GIVEN** `interactive.rs` split into `interactive/{resolver,prompter,picker}` submodules behind the existing `Prompter` trait seam
- **WHEN** the `cli-interaction-e2e` PTY suite and the interactive unit tests run
- **THEN** every branch/CLI/spec picker, fuzzy multi-select, and cancellation path SHALL behave identically to the pre-split implementation
- **AND** `crate::interactive::*` re-exports SHALL keep the binary and integration-test crates compiling unchanged

### Requirement: Refactors do not modify the frozen serde, wire, lock, or SIGHUP surfaces

A refactor SHALL NOT modify the frozen do-not-touch surfaces: the silently-breaking serde
representations (`FileIntent` `untagged`, `AdvancedMain` `flatten`, `StatusPayload.message` without
`skip_serializing_if`, `SpecsConfig` `rename="type"`, `Session.created_at` custom serializer, and
`PawConfig::merged_with` default-as-"unset" merge semantics), the broker lock discipline (no lock
held across `.await`; no reordering of lock/`.await`/`spawn`), and the dashboard/SIGHUP `unsafe`
path. The `BrokerMessage` / `SpecBackendKind` variant sets SHALL NOT be changed by a refactor.

#### Scenario: Frozen serde surfaces are byte-identical

- **GIVEN** a refactor diff that touches `broker/messages.rs`, `config.rs`, or `session.rs`
- **WHEN** the diff is reviewed
- **THEN** the untagged/flatten/no-skip/rename/created-at/merge-default surfaces SHALL be byte-identical to before

#### Scenario: Broker lock discipline is preserved

- **GIVEN** a refactor touching `broker/{delivery,mod,watcher}.rs`
- **WHEN** the crate is linted and reviewed
- **THEN** `clippy::await_holding_lock` SHALL stay clean
- **AND** no lock acquisition, `.await`, or `tokio::spawn` SHALL be reordered relative to before

#### Scenario: The SIGHUP unsafe path is untouched

- **GIVEN** a refactor wave
- **WHEN** the `dashboard.rs` / `main.rs` SIGHUP `unsafe` path is diffed
- **THEN** it SHALL be unchanged by this change (its fix lands as a separate branch)

### Requirement: Structural refactors land in test-gated waves

Structural refactors SHALL land in ordered, test-gated waves, and a wave whose test net cannot prove
behavior parity SHALL NOT land until the enabling net exists. Specifically, the `main.rs → commands/`
split SHALL be gated on both the PTY interaction net (`cli-interaction-e2e`) and removal of the
source-grep introspection tests (`test-suite-consolidation`); the tmux runtime-path refactor SHALL be
gated on the PTY net; and the broker structural tidy SHALL be deferred until after the freeze. Each
wave SHALL pass the five-gate verification (testing, regression vs merge-base, spec audit, doc audit,
security), cold-start, before the next wave begins.

#### Scenario: main.rs split is blocked until its net exists

- **GIVEN** the `main.rs → commands/` split is proposed
- **WHEN** either the PTY net or the source-grep-test removal is not yet in place
- **THEN** the split SHALL NOT land until both preconditions are met

#### Scenario: A wave passes five gates before the next begins

- **GIVEN** a completed refactor wave
- **WHEN** the supervisor runs verification
- **THEN** all five gates (testing, regression vs merge-base, spec audit, doc audit, security) SHALL pass cold-start before the next wave starts

#### Scenario: A behavior change is not folded into a refactor wave

- **GIVEN** a latent bug surfaced by the analysis (e.g. an unsanitized session name)
- **WHEN** the refactor waves are executed
- **THEN** the bug fix SHALL ship as its own spec+test-gated change with a reproducing test, not inside a refactor wave

### Requirement: The dashboard is modeled as Model-View-Update

The dashboard SHALL be structured as Model-View-Update: the pure draw/format helpers form
the **View** (a pure function of the Model producing the rendered frame), the redraw state
forms the **Model**, and a pure `update(&mut Model, Msg)` applies the state transitions.
The crossterm event loop SHALL drain input/tick/broker events into `Msg` values, route them
through `update`, and render the resulting Model via the View — rather than mutating local
state inline. The SIGHUP/`poll_tty` FFI, the `getppid()==1` orphan-exit, the poll cadence
and timeout, and terminal setup/teardown SHALL remain byte-identical to the pre-refactor
implementation (frozen surface). Rendered frames, key handling (including quit), and
broker-log ingest SHALL be observably identical to the pre-refactor direct-mutation loop.

#### Scenario: The View renders purely from the Model

- **GIVEN** a `Model` value and a `TestBackend`
- **WHEN** the View renders the Model
- **THEN** the produced frame SHALL match the pre-refactor render for the same state
- **AND** the View SHALL read only the Model (no I/O, no `poll_tty`, no terminal side effects)

#### Scenario: Update applies a message exactly as the old inline mutation did

- **GIVEN** a `Model` and a `Msg` (a key press, a tick/redraw, or a broker-message ingest)
- **WHEN** `update(&mut model, msg)` runs
- **THEN** the Model SHALL transition exactly as the pre-refactor inline handling did — e.g. a quit key sets the quit flag, a broker ingest appends the same row the old loop appended, a status snapshot yields the same rows/counters
- **AND** `update` SHALL be pure (no terminal or process I/O)

#### Scenario: The SIGHUP and poll FFI are untouched

- **GIVEN** the MVU refactor
- **WHEN** `src/dashboard.rs` is inspected
- **THEN** `orphaned()`, `poll_tty()`, the `getppid()==1` orphan-exit, and the poll cadence/timeout SHALL be byte-identical to the pre-refactor code
- **AND** the event loop SHALL still call `poll_tty` and drain crossterm events exactly as before, only routing them through `update`

### Requirement: The worker lifecycle is an explicit phase enum with a single mapping

The supervisor SHALL represent a coding agent's lifecycle phase as an explicit enum
(`WorkerPhase` — the phases the drive loop reacts to: `Working`, `Idle`, `Blocked`,
`Committed`, `Verified`, `Done`, plus an `Other` fallback for any unrecognized status)
derived from the broker status string through a single mapping (`WorkerPhase::from_status`),
rather than by scattered inline string comparisons and parallel status-string constant
arrays. The phase predicates the loop needs SHALL be methods on the enum: "is a completed
phase" (the old `AGENT_COMPLETE_STATUSES` = `verified`/`done`) and "is a merge-candidate
phase" (the old `MERGE_CANDIDATE_STATUSES` = `committed`/`done`). Each predicate's result
SHALL be identical to the pre-refactor `.contains()` check for every status string. The
refactor SHALL be behavior-preserving: the drive loop's nudge/escalate/merge/correction
decisions are unchanged.

#### Scenario: Each status string maps to its phase

- **WHEN** a broker status string is passed to `WorkerPhase::from_status`
- **THEN** `working`→`Working`, `idle`→`Idle`, `blocked`→`Blocked`, `committed`→`Committed`, `verified`→`Verified`, `done`→`Done`
- **AND** any unrecognized status maps to `Other`

#### Scenario: The completed and merge-candidate predicates equal the old arrays

- **GIVEN** any status string
- **WHEN** the enum's "completed" and "merge-candidate" predicates are evaluated for its phase
- **THEN** "completed" SHALL be true exactly when the status was in `AGENT_COMPLETE_STATUSES` (`verified`/`done`)
- **AND** "merge-candidate" SHALL be true exactly when the status was in `MERGE_CANDIDATE_STATUSES` (`committed`/`done`)

#### Scenario: Drive-loop behavior is unchanged

- **GIVEN** the lifecycle-enum refactor
- **WHEN** the unattended-drive e2e suite and the drive-loop unit tests run
- **THEN** the loop's nudge, escalation, merge, and correction decisions SHALL be identical to the pre-refactor behavior

### Requirement: Session surface roles are an explicit enum resolved once from the recorded layout

The system SHALL represent the role of each surface in a session's window as a closed enum (`SurfaceRole`) produced by a single resolver over a core layout record (`SessionLayout`) that is derived from the recorded session — its mode, whether a dashboard exists, and the dashboard's index — together with the surface's owning working directory. Coding-agent roles SHALL be resolved by matching the owning directory against the session's worktree paths before any reserved index is consulted. Every reserved-index fact (the first coding-agent index, the dashboard index, any role-reserved index, and every reserved index a launch records in session or discovery state) SHALL be answered by the layout record; decision code SHALL match on the resolved role and SHALL NOT compare a pane index to a literal, nor re-derive a role by comparing an agent-id string such as `"supervisor"`. A comparison against the broker wire identity `"supervisor"` that identifies a message sender, a roster row or a pane label — not the role of a surface — is not a role decision and is outside this rule. The broker wire identity for a role SHALL be produced in one place, and the terminal adapter modules SHALL NOT depend on supervisor modules for layout geometry. For every layout the system can launch, the resolved roles and every sweep, escalate, keystroke-target, skip, exit and pane-offset decision SHALL be identical to the behavior before this requirement, with one carve-out: because a role decision no longer reads an agent-id string, a coding agent whose branch slug collides with the wire identity `"supervisor"` SHALL be treated as the coding surface it is rather than as the supervisor surface.

#### Scenario: Roles resolve from the layout record

- **GIVEN** a supervised session layout with a supervisor surface at index 0, the dashboard at index 1 and coding agents from index 2, and a bare broker-enabled layout with the dashboard at index 0 and coding agents from index 1
- **WHEN** each surface of each layout is resolved
- **THEN** the supervised layout SHALL yield `Supervisor`, `Dashboard`, then `Coding` roles in index order
- **AND** the bare layout's index 0 SHALL yield `Dashboard`
- **AND** a surface at a non-reserved index whose owner matches no worktree SHALL yield `Unknown`

Test: `surface::tests::roles_resolve_from_the_layout_record, surface::tests::supervised_layout_yields_supervisor_dashboard_then_coding_in_index_order, surface::tests::reserved_indices_per_layout, surface::tests::a_broker_less_supervised_layout_still_resolves_its_dashboard_surface, supervisor::drive::tests::bare_layout_sweeps_only_coding_surfaces_and_types_nowhere`

#### Scenario: Coding agents resolve by owner, not index

- **GIVEN** a coding agent whose surface sits at an index the layout reserves, running in that agent's worktree
- **WHEN** the surface is resolved
- **THEN** it SHALL resolve to that agent's `Coding` role

Test: `surface::tests::coding_agents_resolve_by_owner_not_index, supervisor::drive::tests::a_coding_surface_at_a_reserved_index_escalates_under_its_own_id, supervisor::drive::tests::resolves_coding_agent_by_path_not_index`

#### Scenario: No pane-index literal or role-string comparison in decision code

- **WHEN** a source audit scans the supervisor, coordination, session and command modules
- **THEN** it SHALL find no comparison of a pane index against a numeric literal and no comparison of an agent id against `"supervisor"` used to decide a role, outside the layout module
- **AND** comparisons that identify a broker message sender, a roster row or a pane label as `"supervisor"` SHALL be allowed by name
- **AND** the terminal adapter modules SHALL NOT import a supervisor module for layout geometry

Test: `tests/surface_role_audit.rs::no_agent_id_string_decides_a_surface_role, tests/surface_role_audit.rs::no_pane_index_is_compared_against_a_numeric_literal, tests/surface_role_audit.rs::no_reserved_pane_index_is_written_as_a_literal, tests/surface_role_audit.rs::the_tmux_adapter_imports_no_layout_geometry_from_the_supervisor_module, tests/surface_role_audit.rs::the_supervisor_wire_identity_has_exactly_one_definition, tests/surface_role_audit.rs::stripping_test_modules_keeps_the_production_code_the_rules_govern, tests/surface_role_audit.rs::every_wire_identity_spelling_is_caught`

#### Scenario: Recorded indices come from the layout record

- **GIVEN** a bare broker-enabled launch and a supervised launch
- **WHEN** each saves its session and per-repo discovery file
- **THEN** the recorded `dashboard_pane`, every agent's `pane_index` and any orchestrator `pane_index` SHALL equal the values the layout record answers for that layout
- **AND** they SHALL equal the values the same launch recorded before this requirement

Test: `surface::tests::a_launch_records_the_same_dashboard_pane_it_recorded_before, surface::tests::reserved_indices_per_layout, surface::tests::recorded_dashboard_pane_drives_the_dashboard_index_and_its_fallback, surface::tests::of_a_saved_session_reads_its_mode_broker_and_recorded_dashboard, tests/discovery_file_pane_indices.rs`

#### Scenario: Behavior is unchanged

- **GIVEN** the typed-role refactor
- **WHEN** the unattended-drive, sweep-helper approve, add/remove and coordination-inventory suites run with their assertions unmodified
- **THEN** they SHALL pass
- **AND** the drive loop SHALL sweep, escalate for, send nudge or submit keystrokes to, and exit on exactly the surfaces it did before the refactor, except for the carve-out below

Test: `supervisor::drive::tests::supervised_layout_sweeps_escalates_and_types_exactly_as_before, supervisor::drive::tests::a_prompt_on_the_supervisor_surface_escalates_under_the_wire_identity, tests/unattended_drive_e2e.rs, tests/add_remove_e2e.rs, tests/sweep_sh_approve.rs, tests/coordination_inventory_integration.rs`

#### Scenario: A coding branch slugged `supervisor` is a coding surface

- **GIVEN** a coding agent whose branch slug collides with the broker wire identity `supervisor`, whose surface shows a CLI-unavailability banner
- **WHEN** the drive loop sweeps it
- **THEN** it SHALL be escalated as a coding surface and the wave SHALL continue
- **AND** the loop SHALL NOT exit as orchestrator-CLI-unavailable, which is what the pre-refactor `agent_id == "supervisor"` re-detection did
- **AND** a banner on the surface the layout reserves for the supervisor SHALL still exit the loop at once

Test: `supervisor::drive::tests::a_coding_branch_slugged_supervisor_no_longer_ends_the_wave, supervisor::drive::tests::a_banner_on_the_supervisor_surface_still_ends_the_wave`


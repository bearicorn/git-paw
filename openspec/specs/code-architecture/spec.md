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

### Requirement: Terminal surfaces are reached through a port expressed in domain terms

The system SHALL define a compile-time `TerminalBackend` trait in `src/terminal/` through which every orchestration-path interaction with a terminal multiplexer flows, expressed in git-paw's domain vocabulary, never a multiplexer's. The orchestration path is the drive loop, the launch paths (bare, spec-driven and supervised `start`, recovery of a stopped session, restart from pause), `add`, `remove`, the launch-readiness gate, prompt submission (including the idle wait before a declared workflow entry point, observed through the screen-tail read, and the entry point's submit, sent as text and then a key) and the agent inventory; the lifecycle commands `pause`, `stop`, `attach`, `status` and `doctor` MAY keep calling the `src/tmux/` helpers directly. Surfaces SHALL be addressed by an opaque `SurfaceId` newtype that only a backend constructs. The trait SHALL cover, at minimum: opening and closing a session from a plan of surfaces and querying whether it is alive; setting session-scoped environment; spawning, killing and labelling a surface; listing a session's surfaces together with **the working tree that owns each one**; sending text and sending a named key as distinct operations (so the split-send-then-`Enter` discipline is preserved); reading a surface's recent screen tail, reporting a failed read as no screen rather than an empty one (so a failed read can never be judged idle); reading the name of a surface's foreground process (so a surface whose agent CLI has exited, leaving a command interpreter, is never typed into); addressing the surface at a position the session's layout record names; for the launch-readiness gate, re-running a surface's launch command and choosing an option of an agent CLI's first-run dialog (each starts or answers the CLI itself, so neither is a prompt submission); streaming a surface's output to a file; reporting a surface's liveness; and attaching and detaching clients. A layout operation MAY exist and SHALL be a no-op for a backend that does not tile. The trait SHALL NOT expose a multiplexer's pane index, pane id or pane path as a public type or method. The trait SHALL be an in-tree, compile-time seam with in-tree implementations selected at construction — NOT a runtime plugin mechanism — and adding it SHALL introduce zero new dependencies.

#### Scenario: Port vocabulary is domain-shaped

- **WHEN** the public identifiers of the `TerminalBackend` trait and its associated types in `src/terminal/mod.rs` are inspected
- **THEN** every method and type SHALL name a session, plan, surface, position, placement, layout, owner, screen, process, launch command, dialog, text, key, output stream, liveness, label, client or environment concept
- **AND** none SHALL contain the tokens `tmux`, `pane`, `send_keys`, `capture_pane`, `pipe_pane`, `split_window` or `pane_current_path`

#### Scenario: Owner listing replaces pane-path lookups

- **GIVEN** a backend and a session with a surface whose process runs in worktree `/repo/.git-paw/worktrees/feat-x`
- **WHEN** a caller lists the session's surfaces
- **THEN** the listing SHALL report that surface's `SurfaceId` with owner `/repo/.git-paw/worktrees/feat-x`
- **AND** the caller SHALL NOT need to know how the backend determined it

#### Scenario: Text and keys are separate operations

- **WHEN** a caller submits a prompt to a surface through the backend
- **THEN** it SHALL send the text and then the `Enter` key as two operations
- **AND** the backend SHALL offer no single operation that takes both free text and a key

#### Scenario: No runtime plugin loading

- **WHEN** the crate's dependency set and `src/terminal/` are inspected after this change
- **THEN** `Cargo.toml` SHALL declare no dependency that was not declared before
- **AND** no dynamic library loading or external adapter discovery SHALL exist

### Requirement: tmux is the first terminal backend and production behavior is byte-identical

The system SHALL provide `TmuxBackend` as the first `TerminalBackend` implementation, delegating to the existing `src/tmux/` command builders and the `CommandRunner` seam, and production SHALL wire `TmuxBackend` everywhere the orchestration path previously called tmux directly. The tmux argv emitted for any operation, its ordering, the resulting pane indices and titles, the session-environment injection, and every byte of persisted session state SHALL be identical to the pre-refactor implementation. The requirements of the `tmux-orchestration` capability SHALL be read as the tmux backend's contract and SHALL remain satisfied. Screen capture used by the launch-readiness gate SHALL go through the backend, so no `src/tmux/` module depends on a `src/supervisor/` module.

#### Scenario: Launch argv is unchanged

- **GIVEN** a fixed launch plan (project name, worktrees, CLIs, broker URL, layout)
- **WHEN** the session is built through `TmuxBackend` and the emitted command strings are compared to the pre-refactor builder output for the same plan
- **THEN** the command strings SHALL be identical in content and order

#### Scenario: Drive loop sweeps through the backend with identical effects

- **GIVEN** an unattended session driven by the drive loop wired to `TmuxBackend`
- **WHEN** the loop lists surfaces, captures one, resolves its owner and submits a buffered input left on that surface's idle input box
- **THEN** the tmux invocations SHALL be the same `list-panes` / `capture-pane` / `send-keys` calls the pre-refactor loop issued, with the same targets and arguments

#### Scenario: Existing behavioral suites pass unchanged

- **WHEN** the pre-existing e2e, tmux and drive-loop test suites are run after the refactor without modification
- **THEN** they SHALL pass

#### Scenario: The readiness gate no longer imports supervisor code

- **WHEN** the `use` paths of every file under `src/tmux/` are inspected
- **THEN** none SHALL reference `crate::supervisor`

### Requirement: A fake terminal backend makes orchestration logic testable without a terminal

The system SHALL provide one fake `TerminalBackend` for tests that records every operation it receives (sessions opened, surfaces spawned and killed, labels set, text and keys sent, streams requested) and returns scripted screens and scripted owners, so the drive loop, the launch flows, add/remove and the inventory are unit-tested against the port without spawning tmux. Logic tests SHALL be written once against the fake; each real backend SHALL be covered by smoke tests only.

#### Scenario: Drive-loop escalation is asserted without tmux

- **GIVEN** a fake backend scripted so one surface's screen ends in a live permission prompt and its owner is a known worktree
- **WHEN** the drive loop runs one sweep against the fake
- **THEN** the loop SHALL record a `permission-prompt` escalation attributed to that worktree's agent
- **AND** the fake SHALL have recorded no text or key operation for that surface
- **AND** no `tmux` process SHALL have been spawned

#### Scenario: Drive-loop buffered-input submit is asserted without tmux

- **GIVEN** a fake backend scripted so one owned surface's screen shows a directive left unsubmitted on an idle input box
- **WHEN** the drive loop runs one sweep against the fake
- **THEN** the fake SHALL have recorded exactly the submit key operation for that surface
- **AND** no `tmux` process SHALL have been spawned

#### Scenario: Entry-point idle wait and submit are asserted without tmux

- **GIVEN** a fake backend scripted so a launched surface whose task prompt declares a workflow entry point shows a busy screen and then two consecutive idle screens
- **WHEN** the launch flow waits for idle and submits the entry point through the fake
- **THEN** the fake SHALL have recorded screen-tail reads for that surface and no text or key operation before the idle screens
- **AND** then a fresh screen-tail read and a foreground-process read for that surface before any text operation
- **AND** then exactly a text operation carrying the entry point, a further screen-tail read, and the `Enter` key operation
- **AND** no `tmux` process SHALL have been spawned

#### Scenario: Entry-point keystroke-safety checks survive the port

- **GIVEN** a fake backend scripted for a launched surface that owes a workflow entry point
- **WHEN** the launch flow waits for idle and submits the entry point through the fake, once for each of: a surface whose screen-tail read fails on every poll; a surface that reads idle but whose foreground process is a command interpreter; a surface whose foreground-process read fails; and a surface whose screen changes to a live prompt between the text operation and the submit
- **THEN** the fake SHALL have recorded no text or key operation for the failed-read, interpreter and failed-process-read surfaces
- **AND** for the surface whose screen changed it SHALL have recorded the text operation and SHALL NOT have recorded the `Enter` key operation

#### Scenario: Launch plan is asserted without tmux

- **GIVEN** a fake backend and a launch plan for two worktrees with the broker enabled
- **WHEN** the launch flow opens the session through the fake
- **THEN** the fake SHALL have recorded one dashboard surface and two coding surfaces with their labels, owners and launch commands
- **AND** no `tmux` process SHALL have been spawned

### Requirement: Direct multiplexer invocation is confined to the backend

Direct invocation of the `tmux` program SHALL be confined to `src/tmux/**` and `src/terminal/tmux.rs`, with the sole exception of the environment probes in `src/doctor.rs` and `src/selftest.rs`, which test the tmux binary itself and SHALL be named in the guard's allowlist. No other module under `src/` SHALL construct a `tmux` process or hand-build a `<session>:<window>.<pane>` target, and a standing source-audit test SHALL enforce this so the leak cannot silently reopen. Code inside a `#[cfg(test)]` test module is outside the audit: a test MAY drive a private tmux server directly to observe what the backend did.

#### Scenario: Source audit finds no stray tmux invocation

- **WHEN** the source-audit test scans the non-test code of every `.rs` file under `src/` for a `tmux` program invocation (`Command::new("tmux")`, a `CommandRunner` call whose program is `"tmux"`, or a `TmuxCommand` execution) or a hand-built pane target
- **THEN** every hit SHALL be inside `src/tmux/`, `src/terminal/tmux.rs`, or the named environment probes

#### Scenario: A reintroduced direct call fails the audit

- **GIVEN** a `Command::new("tmux")` call added to `src/supervisor/drive.rs`
- **WHEN** the source-audit test runs
- **THEN** it SHALL fail and name the offending file

### Requirement: Persisted surface handles stay backward compatible

The system SHALL keep the names, types and serialized bytes of the persisted `pane_index` fields of the per-repo discovery JSON (`RepoAgentEntry`) and of the `dashboard_pane` field of the user-data `Session` file; they are the tmux backend's encoding of a `SurfaceId`, and the conversion between the two SHALL live inside the tmux backend. Any future backend SHALL persist its own handle encoding in an additive optional field rather than repurposing these integers, so every session file written before this change loads unchanged and the bundled `sweep.sh` continues to read `pane_index` as before.

#### Scenario: Session files round-trip byte-identically

- **GIVEN** a `Session` and a `RepoSessionFile` saved by the pre-refactor binary
- **WHEN** they are loaded and re-saved by the refactored binary
- **THEN** the emitted JSON bytes SHALL be identical

#### Scenario: sweep.sh discovery contract is unchanged

- **GIVEN** a per-repo discovery file written by the refactored `start`
- **WHEN** `sweep.sh` enumerates agents from it
- **THEN** it SHALL read each agent's `branch_id`, `worktree_path`, `cli` and `pane_index` exactly as before

#### Scenario: tmux handle conversion is confined to the backend

- **WHEN** the crate is searched for conversions between `SurfaceId` and a pane index
- **THEN** every conversion SHALL be inside `src/terminal/tmux.rs`

### Requirement: Terminal backend hosts detached service surfaces

The `TerminalBackend` trait SHALL provide a way to run a non-interactive service command inside a session without occupying a surface in the agent grid, and to stop it: `spawn_service(session, name, command, working_dir) -> ServiceHandle` and `stop_service(handle)`, a liveness query `service_alive(handle)`, and `service(session, name)` to address an existing service by name (so `pause` and `status` can reach a host a previous process started). Stopping a service that is already gone SHALL NOT be an error. The tmux backend SHALL implement a service as a detached window named after the service (created with `new-window -d -n <name>`), so that window 0's pane indices are unaffected and `kill-session` still terminates the service with `SIGHUP`. The fake backend SHALL record spawned and stopped services.

#### Scenario: tmux service is a detached window

- **GIVEN** the `TmuxBackend` and a live session
- **WHEN** `spawn_service(session, "broker", cmd)` is called
- **THEN** a window named `broker` SHALL exist in the session running `cmd`
- **AND** window 0's pane count and indices SHALL be unchanged

#### Scenario: Stopping a service closes its window

- **GIVEN** a running tmux service `broker`
- **WHEN** `stop_service` is called
- **THEN** the `broker` window SHALL be gone
- **AND** the service process SHALL have received `SIGHUP`

#### Scenario: Fake backend records services

- **GIVEN** the fake backend
- **WHEN** a launch flow spawns the broker service through it
- **THEN** the fake SHALL have recorded one `spawn_service` with name `broker` and the host command

### Requirement: Drive-loop decisions are pure functions with effects returned as data

The drive loop SHALL be structured as observe → decide → apply. Its prompt decision (CLI unavailability, the live-prompt gate, the buffered-input case, escalation dedup, and the escalation's manual-decision record, broker escalation and friction learning), its message decision (over the broker messages observed since the loop's cursor: gate-failure feedback marking the correction state, merge events triggering the branch refresh, and a coding agent's `agent.question` or merge-candidate `agent.artifact` outside wind-down returned as a judgment call naming the agent and the question or merge status), its nudge decision (correction re-engagement and the exhaustion policy, the post-send cycle count and early heads-up, and each nudge-recovery attempt decided from its fresh verify capture), and its exit-summary decision SHALL each be a pure function that takes the current decision state and the observed inputs — the surface's resolved role, its captured tail, the worker's broker status, the observed broker messages, the resolved prompt profile and the loop's configuration, each branch's merge state as reported by git, and the current time — and returns the next state together with an ordered list of effects drawn from a closed effect enum. The message decision SHALL consider a message other than a merge event only when its sender is one of the session's coding agents, so the loop's own escalations never feed back into it; noting a judgment call SHALL change no surface, broker message or file (it is recorded in the effect log only). The heartbeat decisions that already are pure functions (`heartbeat_due`, `tick_made_progress`) SHALL remain pure and SHALL be called by the shell. These functions SHALL perform no I/O and SHALL NOT read a port, the clock, git or the broker. No decision SHALL return an effect that sends a keystroke answering a permission prompt; a live permission prompt yields only escalation effects (per `approval-auto`, *Approval keystrokes are sent only by an explicit approve action*). The imperative shell SHALL gather the observations, call the decisions, apply each returned effect in order through the existing ports (with the per-surface claim executed by the shell), and append every applied effect with its outcome to an ordered effect log returned with the run's summary. The refactor SHALL be behavior-preserving: the keystrokes sent, their order, the manual-decision records written, the broker messages published, the learnings recorded and the summary text SHALL be identical to the behavior before this requirement.

#### Scenario: A prompt decision is tested with data alone

- **GIVEN** a captured tail ending in a live permission prompt for `ls -la`, a coding-agent role and an empty dedup state
- **WHEN** the prompt decision is called
- **THEN** it SHALL return, in order, effects that record a manual decision for that agent, publish a `permission-prompt` escalation naming the agent and `ls -la`, and record a `tooling_friction` learning
- **AND** it SHALL return no keystroke effect for that surface
- **AND** the test SHALL need no fake terminal, no fake broker and no fake clock

#### Scenario: A repeated prompt within the dedup window yields no effect

- **GIVEN** a dedup state that already holds the escalation key for an agent's live prompt recorded one minute earlier
- **WHEN** the prompt decision is called with the same captured prompt and a time inside the window
- **THEN** it SHALL return no effects and an unchanged dedup state

#### Scenario: A repeated CLI-unavailable banner within the dedup window yields no effect

- **GIVEN** a dedup state that already holds the CLI-unavailable key for an agent and the matched marker, recorded one minute earlier
- **WHEN** the prompt decision is called with a captured tail still showing the same banner and a time inside the window
- **THEN** it SHALL return no effects and an unchanged dedup state

#### Scenario: A question and a merge candidate are judgment calls in the message decision

- **GIVEN** observed broker messages: an `agent.question` from coding agent `feat-a` asking "which config file?" and an `agent.artifact` with status `committed` from coding agent `feat-b`, with the loop not winding down
- **WHEN** the message decision is called
- **THEN** it SHALL return, in order, a judgment call for `feat-a` carrying the question and a judgment call for `feat-b` carrying the merge status `committed`
- **AND** it SHALL return no keystroke effect and no broker-publish effect for either
- **AND** the test SHALL need no fake terminal, no fake broker and no fake clock

#### Scenario: Messages from non-coding senders yield nothing

- **GIVEN** an observed `agent.question` whose sender is not one of the session's coding agents
- **WHEN** the message decision is called
- **THEN** it SHALL return no effects and an unchanged correction state

#### Scenario: The exit summary takes merge truth as an input

- **GIVEN** a final loop state in which every worker reached `verified`, and an observed merge state reporting one branch as not an ancestor of the default branch
- **WHEN** the summary decision is called
- **THEN** the summary SHALL NOT report the wave `completed`
- **AND** SHALL carry an `unmerged` escalation for that branch

#### Scenario: Effects are applied in order and logged

- **GIVEN** a sweep whose prompt decision returns a manual-decision record, an escalation and a friction learning
- **WHEN** the shell applies them
- **THEN** the manual decision SHALL be recorded before the escalation is published, and the escalation published before the learning is recorded
- **AND** the effect log SHALL list the three effects in that order with their outcomes

#### Scenario: Decisions perform no I/O

- **WHEN** a source audit inspects the drive-loop decision module
- **THEN** it SHALL contain no use of the terminal port, the broker client, `std::process`, `std::fs`, `std::time::Instant::now` or `SystemTime::now`

#### Scenario: Drive-loop behavior is unchanged

- **GIVEN** the functional-core refactor
- **WHEN** the existing drive-loop unit tests and the unattended-drive e2e suite run without modification
- **THEN** they SHALL pass

### Requirement: Command entry points are a callable library surface

The command bodies SHALL live in the library crate as `git_paw::commands`, exposing one `pub fn` per command family (`start`, `start_with_specs`, `supervisor`, `add`, `remove`, `pause`, `stop`, `purge`, `purge_stale`, `status`, `attach`, `recover`, `approvals`, `replay`, `clis`) that takes explicit inputs — the repository root, the loaded `PawConfig`, and an options struct — and an injected output sink, and returns a typed outcome or `PawError`. `setup_cli` SHALL keep its released use-case shape in the library: `prepare` resolves and plans without writing anything, and only `apply_setup` / `apply_undo` write, so writing requires the caller to call apply after obtaining the confirmation the CLI requires (a terminal confirmation or `--yes`). The command bodies under `src/commands/` SHALL NOT read `std::env::current_dir()`, SHALL NOT read stdin or prompt interactively, SHALL NOT write to stdout or stderr except through the injected sink (neither through print macros nor through a `std::io::stdout()` / `std::io::stderr()` handle passed to a helper), and SHALL NOT attach a terminal client or run the unattended drive loop; input resolution, prompting, rendering, attaching (including reattaching to, or refusing to fork, an already-running session) and running the drive loop SHALL happen in the caller (the binary's entry layer, or a caller such as the MCP server). Entry-layer code that exists to talk to the terminal — `commands/completions.rs`, which writes a generated completion script to stdout — and host code that runs as its own long-lived process — `commands/broker_host.rs`, the hidden `__broker` host — SHALL be exempted by name, not by pattern. The options SHALL express exactly what the CLI can express — `stop` takes no force or confirmation option. The library crate's "internal API, not covered by semver" notice SHALL remain.

#### Scenario: A command runs in-process with a capturing sink

- **GIVEN** an active session and a caller resolved to the repository root
- **WHEN** the caller invokes `git_paw::commands::pause` with default options and a capturing sink
- **THEN** the session SHALL be paused exactly as `git paw pause` pauses it
- **AND** the call SHALL return an outcome describing the result
- **AND** nothing SHALL be written to the process's stdout

#### Scenario: Command bodies perform no ambient I/O

- **WHEN** a source audit scans every non-test function under `src/commands/`
- **THEN** none SHALL call `std::env::current_dir`, read from stdin, use `dialoguer`, invoke `println!` / `print!` / `eprintln!` / `eprint!`, obtain a `std::io::stdout()` or `std::io::stderr()` handle (including one passed directly to a helper), or attach a tmux client
- **AND** the only files exempted SHALL be `commands/completions.rs` and `commands/broker_host.rs`, each named explicitly

#### Scenario: CLI behavior is byte-identical

- **WHEN** the CLI e2e suites and the golden-output checks for `status`, `pause`, `stop` and `purge --force` run after the relocation
- **THEN** exit codes, stdout, stderr, session files and created panes SHALL match the pre-change behavior

#### Scenario: Stop has no force option in the library

- **WHEN** the `stop` options type is inspected
- **THEN** it SHALL carry no force or confirmation field

#### Scenario: setup_cli writes nothing without the caller's confirmation

- **GIVEN** a repository with no `.claude/settings.local.json` and a verified `claude` allowlist item
- **WHEN** a caller invokes `git_paw::commands::setup_cli::prepare` for `claude` with the allowlist item and does not call `apply_setup`
- **THEN** no target file and no setup record SHALL be created or modified

### Requirement: Remaining command bodies leave main.rs

`src/main.rs` SHALL contain only argument parsing, input resolution (cwd, config, interactive prompts), dispatch into `git_paw::commands`, rendering of outcomes, the attach / drive-loop steps the CLI performs after a command returns, and the entry-layer functions allowed by name: `cmd_prompt_profile` (the hidden `__prompt-profile` subcommand, with its private helpers), `cmd_setup_cli` / `confirm_setup` (the `setup-cli` plan print, terminal check and confirmation) and `cmd_dashboard` (the hidden `__dashboard` subcommand, a thin client of the broker host). `cmd_purge`, `cmd_purge_stale`, `purge_with_prompt`'s non-interactive body, `PurgeOutcome`, `RetainedBranch`, `detach_worktree`, `collect_unmerged_branches`, the branch-retirement helpers (`created_branch_base`, `MergeState`, `branch_merge_state`, `discard_flag`, `retire_created_branch`), `attach_agent`, `AttachContext`, `AttachedAgent`, `write_repo_discovery_file`, `write_repo_discovery_file_with_agents`, `submit_prompt_to_pane`, `gate_pane_or_fail_on_dialog`, `build_task_prompt`, `combine_boot_and_task_prompt`, `SpecMode`, `apply_spec_mode`, `invalidate_if_stale` and any other command body SHALL move under `src/commands/` (or the library module that owns the concern). Reattach logic for an already-running session (whatever form it has when this lands) SHALL be an entry-layer step driven by the command's outcome, not a call made from inside a command body: the entry layer attaches, prints the hint or refuses, or, with `--unattended`, takes the drive-loop binding and runs the loop on the live session.

#### Scenario: main.rs holds no command body

- **WHEN** `src/main.rs` is scanned
- **THEN** it SHALL contain no function whose name begins `cmd_` other than dispatch helpers and the by-name entry-layer functions `cmd_prompt_profile`, `cmd_setup_cli` and `cmd_dashboard`
- **AND** SHALL contain no definition of `attach_agent`, `AttachedAgent`, `write_repo_discovery_file`, `write_repo_discovery_file_with_agents`, `submit_prompt_to_pane`, `detach_worktree`, `build_task_prompt`, `combine_boot_and_task_prompt`, `apply_spec_mode`, `retire_created_branch`, `PurgeOutcome` or `RetainedBranch`

#### Scenario: Purge behavior is unchanged after relocation

- **WHEN** the purge scenarios in `tests/e2e_tests.rs`, the purge unit tests and `tests/session_stale_hygiene.rs` run
- **THEN** the e2e and stale-hygiene suites SHALL pass unchanged
- **AND** the purge unit tests SHALL pass with only the output-sink adaptation (their `stderr` writer becomes a capturing sink, and the flush-before-confirm check becomes a warning-before-confirm ordering check on the sink)


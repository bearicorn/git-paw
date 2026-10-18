## ADDED Requirements

### Requirement: Terminal surfaces are reached through a port expressed in domain terms

The system SHALL define a compile-time `TerminalBackend` trait in `src/terminal/` through which every orchestration-path interaction with a terminal multiplexer flows, expressed in git-paw's domain vocabulary, never a multiplexer's. The orchestration path is the drive loop, the launch paths (bare, spec-driven and supervised `start`, recovery of a stopped session, restart from pause), `add`, `remove`, the launch-readiness gate, prompt submission (including the idle wait before a declared workflow entry point, observed through the screen-tail read, and the entry point's submit, sent as text and then a key) and the agent inventory; the lifecycle commands `pause`, `stop`, `attach`, `status` and `doctor` MAY keep calling the `src/tmux/` helpers directly. Surfaces SHALL be addressed by an opaque `SurfaceId` newtype that only a backend constructs. The trait SHALL cover, at minimum: opening and closing a session from a plan of surfaces and querying whether it is alive; setting session-scoped environment; spawning, killing and labelling a surface; listing a session's surfaces together with **the working tree that owns each one**; sending text and sending a named key as distinct operations (so the split-send-then-`Enter` discipline is preserved); reading a surface's recent screen tail; streaming a surface's output to a file; reporting a surface's liveness; and attaching and detaching clients. A layout operation MAY exist and SHALL be a no-op for a backend that does not tile. The trait SHALL NOT expose a multiplexer's pane index, pane id or pane path as a public type or method. The trait SHALL be an in-tree, compile-time seam with in-tree implementations selected at construction — NOT a runtime plugin mechanism — and adding it SHALL introduce zero new dependencies.

#### Scenario: Port vocabulary is domain-shaped

- **WHEN** the public identifiers of the `TerminalBackend` trait and its associated types in `src/terminal/mod.rs` are inspected
- **THEN** every method and type SHALL name a session, surface, owner, screen, text, key, output stream, liveness, label, client or environment concept
- **AND** none SHALL contain the tokens `tmux`, `pane`, `send_keys`, `capture_pane`, `pipe_pane`, `split_window` or `pane_current_path`

#### Scenario: Owner listing replaces pane-path lookups

- **GIVEN** a backend and a session with a surface whose process runs in worktree `/repo/.git-paw/worktrees/feat-x`
- **WHEN** a caller lists the session's surfaces
- **THEN** the listing SHALL report that surface's `SurfaceId` with owner `/repo/.git-paw/worktrees/feat-x`
- **AND** the caller SHALL NOT need to know how the backend determined it

#### Scenario: Text and keys are separate operations

- **WHEN** a caller submits a prompt to a surface through the backend
- **THEN** it SHALL send the text and then the `Enter` key as two operations
- **AND** the backend SHALL offer no single operation that both types text and submits it

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
- **AND** then exactly a text operation carrying the entry point followed by the `Enter` key operation
- **AND** no `tmux` process SHALL have been spawned

#### Scenario: Launch plan is asserted without tmux

- **GIVEN** a fake backend and a launch plan for two worktrees with the broker enabled
- **WHEN** the launch flow opens the session through the fake
- **THEN** the fake SHALL have recorded one dashboard surface and two coding surfaces with their labels, owners and launch commands
- **AND** no `tmux` process SHALL have been spawned

### Requirement: Direct multiplexer invocation is confined to the backend

Direct invocation of the `tmux` program SHALL be confined to `src/tmux/**` and `src/terminal/tmux.rs`, with the sole exception of the environment probes in `src/doctor.rs` and `src/selftest.rs`, which test the tmux binary itself and SHALL be named in the guard's allowlist. No other module under `src/` SHALL construct a `tmux` process or hand-build a `<session>:<window>.<pane>` target, and a standing source-audit test SHALL enforce this so the leak cannot silently reopen.

#### Scenario: Source audit finds no stray tmux invocation

- **WHEN** the source-audit test scans every `.rs` file under `src/` for a `tmux` program invocation (`Command::new("tmux")`, a `CommandRunner` call whose program is `"tmux"`, or a `TmuxCommand` execution) or a hand-built pane target
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

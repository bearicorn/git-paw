## MODIFIED Requirements

### Requirement: Terminal surfaces are reached through a port expressed in domain terms

The system SHALL define a compile-time `TerminalBackend` trait in `src/terminal/` through which every orchestration-path interaction with a terminal multiplexer flows, expressed in git-paw's domain vocabulary, never a multiplexer's. The orchestration path is the drive loop, the launch paths (bare, spec-driven and supervised `start`, recovery of a stopped session, restart from pause), `add`, `remove`, the launch-readiness gate, prompt submission (including the idle wait before a declared workflow entry point, observed through the output-tail read, and the entry point's submit, sent as text and then a key) and the agent inventory; the lifecycle commands `pause`, `stop`, `attach`, `status` and `doctor` MAY keep calling the `src/tmux/` helpers directly. Surfaces SHALL be addressed by an opaque `SurfaceId` newtype that only a backend constructs. The port SHALL NOT assume that a surface is a local process with a screen: it SHALL report, for each surface, whether that surface presents an interactive screen and whether it holds a position in the session's layout. The trait SHALL cover, at minimum: opening and closing a session from a plan of surfaces and querying whether it is alive; setting session-scoped environment; spawning, killing and labelling a surface; listing a session's surfaces together with **the working tree that owns each one** and, for a surface that holds one, its layout position; addressing the surface at a position the session's layout record names, where a surface that holds no position is never returned for a position; sending text and sending a named key as distinct operations (so the split-send-then-`Enter` discipline is preserved); reading a surface's recent output tail — the screen of an interactive surface, the recent output text of any other surface — reporting a failed read as no output rather than an empty one (so a failed read can never be judged idle); streaming a surface's output to a file; reporting a surface's liveness; and presenting a session to the user (attaching a client where the backend has clients) and detaching clients. For interactive surfaces the trait SHALL also cover: reading the name of a surface's foreground process (so a surface whose agent CLI has exited, leaving a command interpreter, is never typed into); leaving text in a surface's composer without submitting it; and, for the launch-readiness gate, re-running a surface's launch command and choosing an option of an agent CLI's first-run dialog (each starts or answers the CLI itself, so neither is a prompt submission). On a surface that does not present an interactive screen, these interactive-only operations SHALL fail with the typed unsupported-operation error and change nothing, except the foreground-process read, which SHALL report that no process can be read. A surface spec's launch command and placement SHALL be honoured by a backend whose surfaces run a local command in a tiled layout and SHALL be ignored by a backend whose surfaces do not. A layout operation MAY exist and SHALL be a no-op for a backend that does not tile. The trait SHALL NOT expose a multiplexer's pane index, pane id or pane path as a public type or method. The trait SHALL be an in-tree, compile-time seam with in-tree implementations selected at construction — NOT a runtime plugin mechanism — and adding it SHALL introduce zero new dependencies.

#### Scenario: Port vocabulary is domain-shaped

- **WHEN** the public identifiers of the `TerminalBackend` trait and its associated types in `src/terminal/mod.rs` are inspected
- **THEN** every method and type SHALL name a session, plan, surface, position, placement, layout, owner, screen, output, capability, process, launch command, dialog, text, key, output stream, liveness, label, client or environment concept
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

#### Scenario: Capabilities are reported per surface

- **GIVEN** the fake backend scripted with one session holding an interactive surface at position 1 and a surface that is neither interactive nor positioned
- **WHEN** the session's surfaces are listed and each surface's interactivity is queried
- **THEN** the first surface SHALL be reported interactive with position 1
- **AND** the second SHALL be reported not interactive with no position
- **AND** no position SHALL resolve to the second surface

#### Scenario: A non-interactive surface refuses interactive-only operations

- **GIVEN** the fake backend scripted with a surface that does not present an interactive screen
- **WHEN** a caller asks it to leave text in its composer, re-run its launch command and choose dialog option 1, and reads its foreground process
- **THEN** each of the first three calls SHALL return the unsupported-operation error naming the operation
- **AND** the foreground-process read SHALL report that no process can be read
- **AND** the fake SHALL have recorded no text or key delivered to that surface

#### Scenario: tmux surfaces are interactive and positioned

- **GIVEN** the tmux backend and a live session with a dashboard and two coding surfaces
- **WHEN** the session's surfaces are listed and each surface's interactivity is queried
- **THEN** every surface SHALL be reported interactive with the position it held before this change
- **AND** the tmux invocations issued SHALL be identical to the invocations the same listing issued before this change

### Requirement: The terminal port reports interactivity and surface activity

The `TerminalBackend` trait SHALL report, for each surface, whether that surface presents an interactive screen, and SHALL report a surface's activity as one of a closed set of domain values — busy, awaiting input, failed (with its exit code or signal), interrupted, or unknown. A backend whose surfaces are all of one kind MAY also report interactivity for all its surfaces at once, and that answer SHALL equal its answer for each of its surfaces. Every decision about one surface — including the drive loop's observation of a surface, boot-prompt rendering for a worker and the broker host's choice of whom to wake — SHALL use that surface's own answer, never an answer for the whole backend. The tmux backend SHALL report every surface as interactive and every surface's activity as unknown, so every decision that reads a tmux surface's screen is unchanged. The headless backend SHALL report its surfaces as non-interactive and SHALL report the activity its turn record determines. The new identifiers SHALL satisfy the port's existing vocabulary audit.

#### Scenario: tmux answers interactive and unknown

- **GIVEN** the tmux backend and a live session with one coding surface
- **WHEN** interactivity and that surface's activity are queried
- **THEN** the backend SHALL report the surface interactive
- **AND** the surface's activity SHALL be unknown
- **AND** no tmux command SHALL have been issued for the activity query

#### Scenario: Headless answers non-interactive and a known activity

- **GIVEN** the headless backend and a surface that has run one successful turn
- **WHEN** interactivity and that surface's activity are queried
- **THEN** the backend SHALL report the surface non-interactive
- **AND** the surface's activity SHALL be awaiting input

#### Scenario: The fake scripts activity

- **GIVEN** the fake backend configured with one non-interactive surface scripted busy
- **WHEN** interactivity and that surface's activity are queried
- **THEN** it SHALL report the surface non-interactive and busy

#### Scenario: Surfaces of one session answer separately

- **GIVEN** the fake backend scripted with one session holding an interactive surface and a non-interactive surface
- **WHEN** the drive loop builds its observation of each surface
- **THEN** the observation of the first SHALL be interactive and the observation of the second SHALL be non-interactive

## ADDED Requirements

### Requirement: Interactive-only operations are guarded per surface

Every caller of an interactive-only operation — the launch-readiness gate's relaunch and dialog answer, the idle wait and submit of a declared workflow entry point, buffered-input submission, nudge-submission verification and recovery, and leaving a boot prompt in a composer — SHALL first find the target surface interactive, and SHALL skip the operation for a surface that is not, recording the skip as not applicable rather than as a failure. No screen-based predicate (an idle marker, a live-prompt gate, a buffered-input or mid-response check, a CLI-unavailability banner or a bare-interpreter check) SHALL be evaluated against the output tail of a surface that is not interactive. For a session whose surfaces are all interactive, every caller's operations, their order and their outcomes SHALL be identical to the behavior before this requirement.

#### Scenario: The readiness gate skips a non-interactive surface

- **GIVEN** the fake backend scripted with a launched non-interactive surface whose output tail reads like a bare command interpreter
- **WHEN** the launch-readiness gate runs for that surface
- **THEN** the fake SHALL have recorded no relaunch and no dialog answer for it
- **AND** the gate SHALL report the surface as not applicable, not as failed

#### Scenario: An entry point is never typed into a non-interactive surface by the idle wait

- **GIVEN** the fake backend scripted with a non-interactive surface owed a workflow entry point whose output tail matches every idle marker
- **WHEN** the launch flow runs the entry-point idle wait for it
- **THEN** the fake SHALL have recorded no foreground-process read and no text or key operation for that surface from the idle wait

#### Scenario: Interactive sessions are unchanged

- **GIVEN** the pre-existing launch-readiness, entry-point, buffered-input and nudge fake-backend tests, whose surfaces are all interactive
- **WHEN** they run unmodified after this change
- **THEN** they SHALL pass

### Requirement: A surface without a layout position resolves by its owner

The role resolver SHALL resolve a listed surface that holds no layout position to the `Coding` role of the agent whose worktree owns it, matched by the same owner rule as every other surface, and to `Unknown` when no worktree matches. Such a surface SHALL never resolve to a reserved role (dashboard, supervisor or any role-reserved index), and no reserved index recorded in session or discovery state SHALL be derived from it.

#### Scenario: A position-less surface owned by a worktree is a coding surface

- **GIVEN** a session layout whose dashboard is at index 0 and a listed surface with no position owned by worktree `/repo/.git-paw/worktrees/feat-r`, which belongs to agent `feat-r`
- **WHEN** the surface is resolved
- **THEN** it SHALL resolve to `feat-r`'s `Coding` role

#### Scenario: A position-less surface with no owner match is unknown

- **GIVEN** a listed surface with no position whose owner matches no worktree of the session
- **WHEN** the surface is resolved
- **THEN** it SHALL resolve to `Unknown`
- **AND** it SHALL NOT resolve to `Dashboard` or `Supervisor`

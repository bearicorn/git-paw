## Why

The terminal port (`TerminalBackend`, archived by v0.18.0's `terminal-backend-seam`, now locked)
assumes every surface is a local process with a screen:

- every listed surface has a layout position (`SurfaceInfo.position` is required, and
  `surface_at` always returns a surface);
- every surface has a foreground process to read, a composer to prefill, a launch command to
  re-run and a first-run dialog to answer;
- the tail it captures is described as a screen.

v1.3.0's headless backend copes with this backend-wide: one interactivity answer per backend and a
typed unsupported-operation error. That is enough while a session holds one kind of surface. This
release adds a worker whose surface is a vendor's cloud session, and sessions that hold tmux,
headless and remote workers side by side. The port must therefore describe each surface by what
it can do, or the remote adapter needs a second port. v0.18.0 is locked, so the fix is this change,
placed first.

## What Changes

- **Per-surface capabilities.** The port reports, for each surface, whether it presents an
  interactive screen and whether it holds a position in the session's layout.
- **Interactive-only operations.** Reading the foreground process, leaving text in the composer,
  re-running the launch command and answering a first-run dialog are defined for interactive
  surfaces only. Elsewhere they fail with the typed unsupported-operation error; the
  foreground-process read reports that no process can be read, which every caller already treats
  as "do not type".
- **Positions are optional.** A surface may hold no layout position. Such a surface is never
  returned for a position and resolves its role from its owning worktree alone.
- **Output, not screen.** The captured tail is the screen of an interactive surface and the recent
  output text of any other. Screen-based predicates run on interactive surfaces only.
- **Launch command and placement** are honoured by a backend whose surfaces run a local command in a
  tiled layout, and ignored by the others.
- **Interactivity is answered per surface** (MODIFIED from v1.3.0's backend-wide answer), so a
  session holding surfaces of several kinds can be observed one surface at a time.
- **Callers guard per surface.** The readiness gate, the entry-point submit, buffered-input
  submission, nudge verification and prefill check the target surface first and skip a surface
  that is not interactive.
- **The fake** scripts interactivity and position per surface.
- tmux behaviour is byte-identical: every tmux surface is interactive and positioned.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `code-architecture`: the terminal port describes each surface by its capabilities; interactive-only
  operations are guarded per surface; a surface without a layout position resolves by its owner;
  interactivity is answered per surface.

## Impact

- **MODIFIED requirement headers and the text each is based on:**
  - *Terminal surfaces are reached through a port expressed in domain terms*: the archived text on
    `feat/v0.18.0-specs` (`openspec/specs/code-architecture/spec.md`). No pending change on any
    spec branch modifies it.
  - *The terminal port reports interactivity and surface activity*: the ADDED text in
    `feat/v1.3.0-specs` `openspec/changes/headless-backend-adapter/specs/code-architecture/spec.md`.
- **Builds on:** v0.18.0 `terminal-backend-seam` (archived; the trait in `src/terminal/mod.rs`,
  `SurfaceInfo`, `surface_at`, `foreground_process`, `prefill_text`, `relaunch_surface`,
  `answer_dialog`, the fake and the vocabulary audit); v0.18.0 `broker-host-separation`
  (`spawn_service` and friends, unchanged here); v1.3.0 `headless-backend-adapter`
  (`surfaces_are_interactive`, `activity`, `PawError::Unsupported`, the turn plan). v0.21.0
  `domain-value-objects` may have retyped the port's string parameters; this change follows
  whatever types are current.
- **Successors in this release:** `remote-surface-adapter`, `mixed-backend-sessions`.
- **Code:** `src/terminal/mod.rs` (`SurfaceInfo.position` becomes optional, `surface_at` may return
  no surface, a per-surface interactivity query), `src/terminal/{fake,tmux}.rs`, the headless
  adapter, the role resolver in `src/surface.rs`, and the callers of the interactive-only
  operations (readiness gate, entry-point submit, buffered-input submission, nudge verification,
  prefill on `add` to a paused session).
- **Enum-variant ripple:** none (`BrokerMessage` and `SpecBackendKind` are untouched).
- **Persisted state:** none. `pane_index` and `dashboard_pane` keep their names, types and bytes.
- **Security / safety:** narrows what may be sent to a surface; adds no process, network or file
  access.
- **Contract:** internal Rust API only; no CLI, config, JSON or exit-code change.
- **Version notes:** "v1.6.0" is a working number for this release and appears only in prose. If
  the release is renumbered, only proposal and design prose change.

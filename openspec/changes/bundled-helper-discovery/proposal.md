## Why

Two bundled-helper defects made the dogfood wave harder than the work it was testing, and
both are **conformance failures against requirements that already exist** rather than
gaps in the spec set.

**The helpers are not provisioned into worktrees.** `broker-agent-helper`'s *Helpers
provisioned into agent worktrees* already requires `git paw start` and `git paw add` to
write `broker.sh` into each agent's `.git-paw/scripts/` before the agent boots,
"so the agent never has to hand-copy a helper from `assets/`". Nothing implements it:
the only `include_str!` of the bundled scripts is in `src/init.rs` (repo-level
`git paw init`), and `provision_worktree` writes no scripts at all. During the dogfood
all three agents independently derived the same workaround —
`cp assets/scripts/broker.sh .git-paw/scripts/` — and each burned a permission prompt on
it before it could publish its first status.

**The helper looks for session state in a place git-paw stopped using.** `sweep.sh`
discovers the session from `<repo>/.git-paw/sessions/*.json`, while
`session::session_state_dir()` resolves to the XDG data directory
(`~/Library/Application Support/git-paw/sessions/` on macOS). The lookup always comes up
empty; the helper works today only through its `$TMUX` fallback, which requires it to run
*inside* the session. Run from the repo root — where an external orchestrator lives, which
is the v0.18.0 direction — every subcommand degrades to `session: <none>`.

The helper is not simply wrong: **the spec set tells it that path.**
`broker-watcher-and-state` and `approval-pattern-surfacing` both describe session state as
`.git-paw/sessions/…`, while `core-ci-hygiene` describes the XDG location. Three
capabilities, two answers, and no single authority — which is how an implementer writes
the stale one in good faith.

## What Changes

- `session-state` SHALL state authoritatively where session state lives, and SHALL declare
  that incidental path mentions elsewhere are non-normative, so there is one answer.
- The bundled helpers SHALL resolve session state from that authoritative location rather
  than assuming a repo-relative directory, and SHALL therefore work when run from the
  repository root rather than only from inside the session. The `$TMUX` fallback SHALL
  remain as a secondary path, not the only working one.
- The worktree-provisioning requirement SHALL be honoured on **every** launch path,
  including the spec-driven launch without supervisor mode where it was observed failing.
  The requirement text already covers it; this change adds the scenario that pins it and
  the implementation that satisfies it.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `broker-agent-helper`: *Helpers provisioned into agent worktrees* gains a scenario
  pinning the non-supervisor spec-driven launch path.

### Added Capabilities

- `session-state`: a requirement naming the authoritative session-state location.
- `broker-agent-helper`: a requirement that bundled helpers resolve session state from
  that location and work when invoked from the repository root.

## Impact

- `src/worktree_provision.rs` — provision the bundled helper scripts into each worktree's
  `.git-paw/scripts/`, idempotently, from the running binary's assets. This is the seam
  every launch path already calls, so fixing it here covers supervisor, bare, spec-driven
  and `git paw add` at once.
- `src/init.rs` — the script constants it already embeds become the shared source for
  worktree provisioning; avoid a second `include_str!` set.
- `assets/scripts/sweep.sh` and the tracked `.git-paw/scripts/sweep.sh` copy —
  `discover_session_name` / `discover_session_file`.
- **Known debt, deliberately not fixed here**: `broker-watcher-and-state`'s *Agent CLI
  populated in the roster* and `approval-pattern-surfacing`'s *Manual-decision log file*
  each carry a stale incidental `.git-paw/sessions/…` reference. Correcting them means
  reproducing 113 lines of unrelated requirement text in deltas, so they are left to the
  pre-freeze spec audit; the new authoritative requirement makes them non-normative in
  the meantime.
- **Delta coordination**: `session-state` is also modified by `worker-session-id-capture`,
  which ADDs different requirements — no requirement-name collision.

## Why

The bundled helpers and tools cannot reliably find the session or the agents in it. Three
defects, each verified against the code:

**GP-28 — the helpers are never provisioned into worktrees.** `broker-agent-helper`'s
*Helpers provisioned into agent worktrees* requires `git paw start` and `git paw add` to write
`broker.sh` into each agent's `.git-paw/scripts/` before the agent boots. Nothing implements
it: the only `include_str!` of the bundled scripts is in `src/init.rs`, and
`provision_worktree` — the seam every launch path calls — writes no scripts. In the first
dogfood wave every agent reinvented `cp assets/scripts/broker.sh …` and burned an approval
on it.

**GP-42 — `sweep.sh` maps panes to agents using the wrong document shape.** The per-repo
discovery file (`.git-paw/sessions/<session>.json`, written by `write_repo_discovery_file`)
holds an `agents` roster of `branch_id` / `worktree_path` / `cli` / `pane_index`.
`resolve_agent_for_path` reads `worktrees` entries with a `branch` field — the shape of the
*global* session-state file in the XDG data directory. Against the file git-paw actually
writes, no pane ever resolves to an agent. The helper's test fixtures disagree with each
other on this shape, which is how it stayed hidden.

**GP-43 — a process started inside a worktree cannot find its session.**
`mcp::resolve_repo` deliberately resolves a worktree to its own root, and
`find_session_for_repo` matches only `session.repo_path == canonical(path)`. A worktree root
never equals the repository root, so `git paw mcp` spawned by an agent's CLI from its
worktree with no `--repo` gets no broker URL, and its publish tools have nowhere to publish.

**Corrected diagnosis (supersedes the first version of this change).** An earlier draft
blamed a "session-state location" mismatch and proposed forbidding repo-relative session
files. That was wrong: there are two distinct documents by design — global session state in
the platform data directory, and a per-repo discovery file under `.git-paw/sessions/` —
and the specs that mention each are correct. The empty `.git-paw/sessions/` observed during
dogfooding came from `cmd_start_with_specs` being the one launch path that never calls
`write_repo_discovery_file`; that is fixed in `start-spec-launch-parity`. A discovery file
seen in the *global* shape during the dogfood was a manual symlink made as a workaround,
not something git-paw wrote.

## What Changes

- Provision the bundled helpers into every agent worktree at the shared
  `provision_worktree` seam (GP-28).
- `sweep.sh` reads the discovery file's `agents` roster, in its real shape; the helper's
  fixtures are aligned to that shape; "no session found" is reported explicitly (GP-42).
- Session lookup resolves a worktree to the repository that owns it before matching, so a
  process started inside an agent worktree finds the session and its broker (GP-43).

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `broker-agent-helper`: *Helpers provisioned into agent worktrees* gains a scenario pinning
  the spec-driven launch path; a new requirement covers reading the discovery file's
  `agents` roster.
- `session-state`: *Find session by repository path* resolves worktree paths to their
  owning repository.

## Impact

- `src/worktree_provision.rs`, `src/init.rs` — provisioning, sharing init's embedded
  script constants.
- `assets/scripts/sweep.sh` and the tracked `.git-paw/scripts/sweep.sh` —
  `resolve_agent_for_path`, the no-session message.
- `tests/sweep_sh_session_discovery.rs`, `tests/sweep_sh_tmux_fallback.rs` — fixtures
  aligned to the `agents` shape.
- `src/session.rs` — `find_session_for_repo_in`; `src/mcp/mod.rs` benefits without change.
- **Depends on** `start-spec-launch-parity` (wave 2) for the discovery file to exist on
  spec-driven launches.
- **Delta coordination**: `start-spec-launch-parity` modifies a different `session-state`
  requirement (*start writes a per-repo session JSON*); no collision.

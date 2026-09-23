## Why

The bundled helpers and tools cannot reliably find the session or the agents in it, and several
helper verbs fail silently. Every defect below is verified against the code.

**GP-28 — the helpers are not provisioned into worktrees on two launch paths.**
`broker-agent-helper`'s *Helpers provisioned into agent worktrees* requires every launch path to
write the helpers into each agent's `.git-paw/scripts/` before the agent boots. `attach_agent`
(`src/main.rs`) does — it calls `init::provision_worktree_helpers`, sourced from `init.rs`'s
embedded script constants — so the supervisor launch and `git paw add` are covered. The two paths
in `src/commands/start.rs` that do not go through `attach_agent` — bare `cmd_start` and the
spec-driven `launch_spec_session` — call `worktree_provision::provision_worktree` (which writes no
scripts) and never call `provision_worktree_helpers`. In the first dogfood wave those agents
reinvented `cp assets/scripts/broker.sh …` and burned an approval on it.

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

**The discovery file is missing after three launch paths.** `write_repo_discovery_file` is called
by bare `cmd_start`, `cmd_supervisor`, `git paw add` and `git paw remove` only. The spec-driven
launch (`launch_spec_session`), recovery of a stopped session (`recover_session`,
`src/commands/recover.rs`) and restart from pause (`restart_from_pause`,
`src/commands/start.rs`) never write it, although *start writes a per-repo session JSON* requires
it for `git paw start` with any flags. After recovery or a resume, `sweep.sh` reads a missing or
stale roster (recovery and resume rebuild panes, so recorded `pane_index` values can be wrong).
`start-spec-launch-parity` does not cover the spec-driven path either (its artifacts never mention
the discovery file), so this change owns all three.

**Folded-in shipped helper bugs** (verified in `assets/scripts/*.sh`):

- `broker.sh intent <summary> <files>` without a TTL omits `valid_for_seconds`. The broker's
  `IntentPayload.valid_for_seconds` is a required field, so `/publish` answers `400` — and the
  helper's `publish()` uses `curl -s` without checking the status, so the failure is silent and the
  helper exits 0. The same unchecked `publish()` exists in `sweep.sh`.
- `sweep.sh learn` computes its `agent.learning` id as
  `sha1(category|title|json.dumps(body, sort_keys=True))[:16]`, a different scheme from the
  aggregator's `LearningRecord::deterministic_id` (`src/broker/learnings.rs`: category, branch id,
  canonical body, UTC hour bucket). The same logical record gets two ids, so consumers cannot dedupe
  across the two publishers. The scheme itself is pinned by `retire-stale-requirements`; this change
  makes `sweep.sh learn` follow it.
- `sweep.sh status`'s phantom filter keeps only `supervisor` and ids matching `feat[-/]…`. Branch
  names are not always `feat/…`: spec-driven branches use the configurable `branch_prefix` (default
  `spec/`, so agent ids like `spec-add-auth`), and bare starts take any branch. Every such agent is
  hidden as a "phantom".

**Corrected diagnoses (supersede the first version of this change).** An earlier draft blamed a
"session-state location" mismatch and proposed forbidding repo-relative session files. That was
wrong: there are two distinct documents by design — global session state in the platform data
directory, and a per-repo discovery file under `.git-paw/sessions/` — and the specs that mention
each are correct. A discovery file seen in the *global* shape during the dogfood was a manual
symlink made as a workaround, not something git-paw wrote. The draft also said nothing copies
helpers into worktrees and that `.git-paw/scripts/sweep.sh` is tracked; `attach_agent` does copy
them, and `.git-paw/scripts/` is gitignored (written by `git paw init`, `.gitignore`
`.git-paw/scripts/*`).

**Model tier.** With the folded-in scope (two languages, five helper defects, three launch paths,
session lookup) this change is now **Opus-tier**.

## What Changes

- Provision the bundled helpers into agent worktrees on the two `start.rs` launch paths that miss
  them (bare and spec-driven), reusing `init::provision_worktree_helpers` (GP-28).
- `sweep.sh` reads the discovery file's `agents` roster, in its real shape; the helper's
  fixtures are aligned to that shape; "no session found" is reported explicitly (GP-42).
- `sweep.sh status` decides phantoms from the roster (any branch prefix), falling back to the
  broker's own agent-id rule when no roster exists.
- Session lookup resolves a worktree to the repository that owns it before matching, so a
  process started inside an agent worktree finds the session and its broker (GP-43).
- The per-repo discovery file is (re)written by the spec-driven launch, recovery, and restart from
  pause, with pane indices matching the live session.
- `broker.sh intent` sends a default `valid_for_seconds` when none is given; a publish the broker
  rejects makes `broker.sh` / `sweep.sh` exit non-zero with the broker's error.
- `sweep.sh learn` computes its id with the aggregator's deterministic-id scheme.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `broker-agent-helper`: *Helpers provisioned into agent worktrees* gains a scenario pinning
  the bare launch path; *Helper publish subcommands* gains the intent TTL default and loud publish
  failures; a new requirement covers reading the discovery file's `agents` roster (including the
  phantom filter).
- `session-state`: *Find session by repository path* resolves worktree paths to their
  owning repository; *start writes a per-repo session JSON* names every launch path that must
  (re)write the file, including recovery and restart from pause.
- `supervisor-learnings`: a new requirement makes `sweep.sh learn` ids follow the aggregator's
  deterministic-id scheme (the scheme itself is specified once, in *Deterministic id for idempotent
  re-emission*, modified by `retire-stale-requirements`).

## Impact

- `src/commands/start.rs` — `cmd_start` and `launch_spec_session` call
  `init::provision_worktree_helpers`; `launch_spec_session` and `restart_from_pause` call
  `write_repo_discovery_file`. `src/commands/recover.rs` — `recover_session` calls it.
  `write_repo_discovery_file` lives in `src/main.rs` and is already imported by
  `commands/start.rs`.
- `assets/scripts/sweep.sh` — `resolve_agent_for_path`, the no-session message, the `status`
  phantom filter, `cmd_learn`'s id, `publish()` status checking. `assets/scripts/broker.sh` —
  `cmd_intent` default TTL, `publish()` status checking (or a shared `publish` in
  `assets/scripts/_paw_common.sh`). The repo-root `.git-paw/scripts/` copies are gitignored and
  refreshed by `git paw init`; worktree copies are refreshed on every attach.
- `tests/sweep_sh_session_discovery.rs`, `tests/sweep_sh_tmux_fallback.rs`,
  `tests/sweep_sh_stuck_detection.rs`, `tests/sweep_sh_status_publish.rs` — fixtures aligned to the
  `agents` shape; new tests for the phantom filter, `learn` id parity and loud publish failures;
  `tests/broker_sh_*` for the intent default.
- `src/session.rs` — `find_session_for_repo_in`; `src/mcp/mod.rs` benefits without change.
- **Depends on** `retire-stale-requirements` (wave 3) for the pinned learning-id scheme that
  `sweep.sh learn` must match; this change is wave 4.
- **Delta coordination**: in `session-state` this change MODIFIES *Find session by repository
  path* and *start writes a per-repo session JSON*; `status-and-picker-truth` modifies *Liveness
  reflects dead agent panes…*, `supervisor-boot-wiring` only ADDs. In `broker-agent-helper` it
  MODIFIES *Helpers provisioned into agent worktrees* and *Helper publish subcommands*;
  `live-prompt-tail-window` modifies *sweep.sh detects stuck-on-prompt agents*. In
  `supervisor-learnings` it only ADDs; `retire-stale-requirements` modifies *Bundled `sweep.sh
  learn` subcommand* and *Deterministic id for idempotent re-emission*. No collision.
  `start-spec-launch-parity` touches `boot-block` and `supervisor-config` only.

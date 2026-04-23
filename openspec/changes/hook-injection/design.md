## Context

This change addresses the #1 UX failure from v0.3.0 and v0.4.0 dogfooding: agents don't use the broker. Across 4 separate sessions (v0.3.0 build, v0.4.0 Wave 1, v0.4.0 Wave 2, audit session), zero agents proactively published status messages despite having the curl commands in their AGENTS.md.

The fix is to stop relying on agent cooperation for routine status tracking and automate it transparently. This is also the correct architecture for A2A (v2.0) — the "heartbeat" that feeds the broker should not depend on each agent's CLI implementing a publish loop.

## Goals / Non-Goals

**Goals:**

- Dashboard shows live agent activity without any agent cooperation
- `modified_files` accurately reflects what each agent has touched
- Commits are detected and published as `agent.artifact` automatically
- Push to remote is blocked at the git hook level
- Works identically across all supported CLIs (claude, codex, gemini, aider, vibe, qwen, amp, custom)
- No new external dependencies (must use tools already required by git-paw)

**Non-Goals:**

- CLI-specific hook providers (deferred to v1.0.0)
- Detecting `blocked` state automatically (inherently semantic — only the agent knows)
- Detecting `done` with specific exports (agent must publish this if it wants to list exports)
- Permission auto-approval per CLI (deferred to v1.0.0 CLI hook providers)
- Sub-second latency filesystem event delivery (pull-based polling is sufficient for a supervisor dashboard that polls every 30s)

## Decisions

### Decision 1: Git-status polling instead of filesystem event watching

Rather than pulling in a filesystem event library (`notify`, which ships under CC0-1.0 and is not OSI-approved, blocking our `cargo-deny` gate), the watcher polls each worktree's git state at a fixed interval by shelling out to `git status --porcelain`.

```rust
// In start_broker(), after spawning the HTTP server:
for target in watch_targets {
    let state = state.clone();
    runtime.spawn(watch_worktree(state, target));
}
```

**Why:**

- Zero new dependencies — `git` is already a hard runtime requirement and `std::process::Command` is in the standard library
- `git status --porcelain` already answers the exact question we need: "what files differ from HEAD in this worktree?" There's no need to reconstruct that from raw inotify/FSEvent events
- `.gitignore` filtering is free — `target/`, `node_modules/`, build artefacts, and editor swap files are excluded automatically by git, not by our own filter list
- `.git/` internal state is never reported because git does not list it in status output
- Cross-platform without platform-specific code — git handles the portability
- Latency: ~2s, which is invisible given the supervisor dashboard polls every 30s and the UI refreshes at a similar cadence
- Subprocess cost is negligible (a handful of worktrees polled every 2s = well under 1% CPU)

**Alternatives considered:**

- *`notify` crate.* Industry standard but CC0-1.0 licensed and not OSI-approved; fails our `cargo-deny` license allow-list. Adding CC0-1.0 to the allow list was rejected by the maintainer.
- *Hand-rolled mtime walking.* Would need its own `.gitignore` parser to avoid build artefacts. Reinvents what git already does.
- *Raw platform APIs (`inotify`, `kqueue`, `ReadDirectoryChangesW`).* Platform-specific code is exactly what filesystem-event libraries exist to abstract — writing our own abstraction is a bad trade.
- *Separate watcher process.* Extra PID to manage, needs IPC. Rejected.
- *Watcher in each agent pane.* Per-CLI, not agnostic. Rejected.

### Decision 2: Poll interval is the debounce window

The watcher wakes once per tick, runs `git status --porcelain`, and compares the result to the previous tick's snapshot. If the set of reported paths changed, it publishes one `agent.status` with all currently-dirty paths in `modified_files`.

- **Interval:** 2 seconds
- **Debouncing:** implicit — rapid edits within a single tick collapse into one publish
- **Publish condition:** snapshot differs from the previous tick (either new files dirty or previously-dirty files no longer dirty, e.g. after a commit)

**Why:**

- Single publish per observable change, regardless of how many files moved
- No separate debounce bookkeeping — the tick interval is the debounce window
- Batching gives an accurate picture ("agent touched 5 files") rather than 5 separate updates
- Avoids re-publishing identical snapshots (tick with no change → no publish)

### Decision 3: Git hooks are shell scripts with pre-expanded broker URL

The `post-commit` hook:

```bash
#!/bin/sh
# Installed by git-paw — publishes agent.artifact on commit
FILES=$(git diff HEAD~1 --name-only 2>/dev/null | tr '\n' '","' | sed 's/^/"/;s/,"$//')
curl -s -X POST http://127.0.0.1:9119/publish \
  -H "Content-Type: application/json" \
  -d "{\"type\":\"agent.artifact\",\"agent_id\":\"feat-my-branch\",\"payload\":{\"status\":\"committed\",\"exports\":[],\"modified_files\":[$FILES]}}" \
  >/dev/null 2>&1 || true
```

The `pre-push` hook:

```bash
#!/bin/sh
# Installed by git-paw — agents must not push
echo "error: git-paw agents must not push. The supervisor handles merges." >&2
exit 1
```

**Why:**

- Shell scripts work with any CLI that uses git (all of them)
- Pre-expanded broker URL avoids the shell expansion permission issue
- `|| true` on post-commit so a broker outage doesn't block commits
- `exit 1` on pre-push is a hard block — no way to accidentally push

### Decision 4: Watcher relies on `git status --porcelain` for exclusions

Because the watcher uses git itself, it inherits git's exclusion rules:

- `.gitignore` entries (e.g. `target/`, `node_modules/`, `*.swp`) are excluded for free
- `.git/` internals are never reported in status output
- `.git-paw/` can be added to the project's `.gitignore` if it isn't already

**Why:**

- No hand-maintained exclusion list in git-paw — it stays in sync with the project's own ignore rules
- Agents that add entries to `.gitignore` automatically affect what the watcher reports
- One source of truth (`git status`) instead of two (git + our filter)

### Decision 5: Watcher passes worktree info to broker at session start

The broker needs to know which worktree directories to watch and which agent_id each maps to. This is passed during session setup:

```rust
pub struct WatchTarget {
    pub agent_id: String,
    pub worktree_path: PathBuf,
}

pub fn start_broker(config: &BrokerConfig, state: BrokerState,
                     watch_targets: Vec<WatchTarget>) -> Result<BrokerHandle, BrokerError>
```

**Why:**

- The broker doesn't know about worktrees otherwise — it just serves HTTP
- Passing targets at start avoids the broker needing to discover worktrees
- The `WatchTarget` list is built by the same code that creates worktrees (or recovered from `Session` state in the dashboard subcommand)

## Risks / Trade-offs

- **~2s detection latency** → Acceptable for a supervisor dashboard that polls every 30s. If sub-second visibility is ever needed, the watcher can be swapped for an event-based implementation without changing the broker API.

- **Polling subprocess cost** → ~10 git-status invocations per worktree per minute. For a session with 5 worktrees that's 50/min of a cheap command. Negligible.

- **Watcher misses files edited outside the worktree** → If an agent reads a file from another worktree (via cherry-pick or symlink), the watcher for that agent won't see it. **Mitigation:** cherry-picks change files in the agent's own worktree, so the watcher catches those.

- **Git hooks may conflict with existing hooks** → If the project already has a `post-commit` hook, git-paw's hook overwrites it. **Mitigation:** check for existing hooks before installing, and chain them (run existing hook first, then git-paw's publish).

- **Polling interval may miss extremely rapid sequences** → If an agent edits a file, reverts it, and edits again all within one tick, only the net change is reported. **Mitigation:** acceptable — the dashboard cares about the current state, not every intermediate.

## Migration Plan

No migration needed. New functionality only. Existing v0.3.0/v0.4.0 sessions without the watcher continue to work (dashboard just shows no automatic updates, same as before).

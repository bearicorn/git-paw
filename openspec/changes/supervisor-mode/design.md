## Context

`supervisor-agent` implements the launch flow. This change implements the user-facing entry point: the `--supervisor` flag, the resolution chain that decides whether to enter supervisor mode, `git paw init` supervisor prompts, `git paw purge` safety, and merge ordering. These are distinct responsibilities that touch `src/cli.rs`, `src/main.rs`, and `src/init.rs`.

## Goals / Non-Goals

**Goals:**

- Add `--supervisor` flag to the `start` subcommand
- Implement the 4-step resolution chain for supervisor mode activation
- Add supervisor prompts to `git paw init`
- Warn about unmerged commits before `git paw purge`
- Implement topological merge ordering from broker dependency signals

**Non-Goals:**

- The actual agent launch (owned by `supervisor-agent`)
- Supervisor CLI behavior at runtime (governed by the supervisor skill)
- Config struct definitions (owned by `supervisor-config`)

## Decisions

### Decision 1: Resolution chain has strict priority ordering

```
1. --supervisor flag  →  enable (no prompt)
2. supervisor.enabled = true in config  →  enable (no prompt)
3. supervisor.enabled = false in config  →  disable (no prompt)
4. No [supervisor] section  →  prompt "Start in supervisor mode?"
5. --dry-run  →  assume no supervisor (skip prompt)
```

Step 5 (dry-run) is evaluated before step 4 to avoid interactive prompts in automated pipelines. `--dry-run` combined with `--supervisor` still enables supervisor mode for preview purposes.

**Why:**
- Explicit beats implicit — CLI flag always wins
- The config distinction between `None` (undecided) and `Some(enabled=false)` (opted out) eliminates repeat prompting
- Dry-run must be non-interactive to support scripting

**Alternatives considered:**
- *Always prompt unless `--supervisor` passed.* Annoying for users who've configured `enabled = true`. Rejected.
- *`--no-supervisor` flag to override config.* Adds flag proliferation. `supervisor.enabled = false` in config is sufficient. Rejected.

### Decision 2: `git paw init` writes the `[supervisor]` section to prevent future prompts

If the user answers "no" to supervisor mode in `git paw init`, the init writes:
```toml
[supervisor]
enabled = false
```

This is the explicit opt-out. It prevents the "Start in supervisor mode?" prompt on every subsequent `git paw start`.

**Why:**
- Respects user intent — if they said no during init, they shouldn't be asked again
- The prompt only appears when the section is completely absent (`None`), not when `enabled = false`
- Users who want to re-enable can edit the config file directly

**Alternatives considered:**
- *Don't write anything for "no" answer.* Users who want to opt out permanently keep seeing the prompt. Rejected.

### Decision 3: Purge reads `git log` per worktree to detect unmerged commits

```
git log <branch> --not main --oneline
```

If any worktree branch has output, it has commits not yet merged to main. The purge command displays the count per branch and requires either `--force` or interactive confirmation.

**Why:**
- `git log --not main` is the canonical way to find branch-local commits
- Per-branch granularity lets the user see exactly what they'd lose
- `--force` preserves scriptability for automated cleanup

**Alternatives considered:**
- *Check only if any worktrees exist.* Too coarse — a worktree might have empty work. Rejected.
- *Compare against the worktree's merge-base.* More accurate but complex; `--not main` is sufficient for this use case. Accepted.

### Decision 4: Merge ordering uses topological sort on `agent.blocked` signals

The supervisor builds a directed graph where an edge `A → B` means "A was blocked on B" (B must merge first). The topological sort of this graph gives the merge order (no-dependents first — i.e., agents no one depends on merge last; agents that others depend on merge first).

After each merge, the supervisor runs the test command and verifies green before proceeding.

**Why:**
- `agent.blocked` messages already record the dependency information — no new protocol needed
- Topological sort handles transitive dependencies automatically
- Running tests after each merge catches integration failures early

**Alternatives considered:**
- *Alphabetical merge order.* Ignores actual dependencies, causes test failures. Rejected.
- *Merge all at once, then test.* Can't isolate which branch broke the build. Rejected.
- *User-specified merge order.* Requires human attention for every session. Rejected.

## Risks / Trade-offs

- **Cycle detection in dependency graph** — if two agents are blocked on each other, the topological sort fails. Mitigation: detect cycles and log a warning; fall back to arbitrary order.
- **`git log --not main` assumes main branch** — repos using `master` or custom trunk names need the base branch resolved from config or git config. Mitigation: resolve the default branch from `git symbolic-ref refs/remotes/origin/HEAD` with a fallback to `main`.
- **Init prompts change existing behavior** — users running `git paw init` for the first time will see new prompts. Mitigation: prompts have clear defaults and can be skipped with Enter.

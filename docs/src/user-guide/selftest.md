# Selftest

`git paw selftest` runs an isolated, end-to-end **session-lifecycle smoke
check** — the shipped form of the dogfood isolation recipe. It exercises the
riskiest orchestration plumbing (`start` → `add` → `remove` → `stop`) against a
throwaway repository and a dummy CLI, then reports a single pass/fail verdict —
with **no real LLM backend and no interactive terminal**.

```bash
git paw selftest
```

## Why

git-paw's session-management surface (`start`, `add`, `remove`, `stop`, `purge`)
is its riskiest plumbing because it spans modules — worktrees (`git.rs`), tmux
panes and sockets (`tmux.rs`), the broker HTTP roster (`broker/`), and session
state (`session.rs`). Historically the only way to exercise the full path was to
boot a real session with a real AI CLI in an interactive terminal — something CI
cannot do and developers reproduced only through ad-hoc shell incantations.

`selftest` packages that recipe as a first-class subcommand, so the lifecycle is
verifiable anywhere. It is a runtime smoke check of the orchestration plumbing,
**complementary to `cargo test`** — not a replacement for the unit and
integration suite.

## What it does

The harness re-invokes the running `git-paw` binary as child processes — the
same code path you exercise by hand — and drives the lifecycle:

1. **Allocate** an OS-assigned ephemeral broker port.
2. **Create** a throwaway git repository (one base commit) under `.git-paw/tmp/`.
3. **Start** an isolated supervisor session with a dummy CLI, confirming it
   landed on the harness's private tmux socket.
4. **Observe the roster** through the per-repo discovery file — it holds exactly
   the initial agent.
5. **Add** an agent worktree and observe the roster **grow** to include it.
6. **Remove** that agent worktree and observe the roster **shrink** back, with
   the remaining entries unchanged.
7. **Stop** the session.

Steps 4–6 make the add/remove roster transitions **observable**, closing the
deferred live-verification that previously required a real session.

## Isolation recipe

Every external resource is isolated so a run never disturbs your live work or a
concurrently running selftest:

- **Private tmux socket** — a per-run `TMUX_TMPDIR`, with `TMUX` and `TMUX_PANE`
  stripped from every child process so the spawned tmux server never attaches to
  the caller's session. Your default tmux socket is never touched.
- **OS-assigned ephemeral broker port** — the harness binds `127.0.0.1:0` and
  reads back the kernel-assigned port, so concurrent runs never collide on a
  fixed or PID-derived port.
- **Isolated `HOME`/XDG** — the global session receipt is written under the
  throwaway tree, so your real sessions directory is never modified.
- **Throwaway repository** under `.git-paw/tmp/` — namespaced inside the repo's
  own git-paw working tree so artifacts are discoverable and gitignorable. A
  stale-dir sweep removes any prior aborted run before a fresh one is created.
- **Dummy CLI (`cat`)** in place of a real agent CLI — it holds its pane open
  without producing output or requiring input, so the session boots
  deterministically in detached mode with **no LLM process spawned**.

Cleanup runs on **both** the success and failure paths: the private-socket tmux
server is killed and the `.git-paw/tmp/` throwaway tree is removed.

## Exit behaviour

| Outcome | stdout / stderr | Exit code |
|---------|-----------------|-----------|
| Lifecycle completed | `selftest passed` | `0` |
| tmux not on `PATH` | `selftest skipped: tmux not available` | `0` |
| A lifecycle step failed | `selftest failed at step '<step>': …` (stderr) | non-zero |

Like the rest of the e2e suite, `selftest` reports a **skip** rather than a
failure when tmux is unavailable, and exits non-zero only on an actual lifecycle
failure. CI runners have tmux installed, so a skip never silently passes there.
When a step fails the verdict **names the failing step** (one of `pick-port`,
`create-repo`, `start`, `roster-initial`, `add`, `roster-after-add`, `remove`,
`roster-after-remove`, `stop`).

## Relationship to the broker-port fix

The same OS-assigned ephemeral-port helper the harness uses for the broker port
is now the canonical scheme across the test suite. It replaced a PID-modulo port
selection (`24_000 + (process::id() % 200)`) that yielded only a small number of
distinct ports and collided under concurrency — the real cause of the
intermittent "address already in use" broker-bind failures. Binding
`127.0.0.1:0` is collision-proof at any concurrency because the kernel guarantees
each bind returns a port not currently in use. See the
[test-isolation specification](../specifications/README.md) for the requirement.

# Doctor

`git paw doctor` answers **"why won't it launch?"** in one command. It runs
read-only preflight checks over your environment, configuration, and repository
state and prints a grouped report — every check carries a **✓**, **⚠**, or **✗**,
and every non-✓ check prints an actionable remedy.

```bash
git paw doctor
```

```text
Environment
  ✓ git                     git version 2.39.5 (/usr/bin/git)
  ✓ tmux                    tmux 3.6a (/opt/homebrew/bin/tmux)
  ✓ git repository          the working directory is inside a git repository

CLIs
  ✓ detected CLIs           2 available: claude, codex

Config
  ✓ config.toml             .git-paw/config.toml parses
  ✓ worktree placement      child — worktrees live in <repo>/.git-paw/worktrees/

Spec system
  ⚠ spec system             no spec system configured — spec-driven launch is unavailable
      ↳ add a [specs] section to .git-paw/config.toml (type = "openspec" | …) or pass --specs-format

Bundled scripts
  ✗ sweep.sh                .git-paw/scripts/sweep.sh is missing
      ↳ run `git paw init` to (re)install the bundled helper scripts
  ✓ broker.sh               installed, executable, matches this binary
  ✓ docs-fetch.sh           installed, executable, matches this binary
  ✓ python3                 Python 3.12.4 (python3)

Broker
  ✓ broker                  disabled — the pure-manual baseline

Supervisor
  ✓ supervisor              disabled — agents run without a supervising pane

Hygiene
  ✓ .gitignore              every git-paw entry is ignored
  ✓ session state           no stale session receipt
  ✓ worktree registrations  every registered worktree exists on disk

14 ✓ · 1 ⚠ · 1 ✗
```

## Diagnose, don't repair

Doctor **never writes**. No file, config, session, or other persistent state is
created, modified, or deleted — it only reads. Apply the remedies yourself;
most of them are `git paw init`.

There is deliberately no `--fix` flag. A repair mode was considered for v0.13.0
and cut to keep the pre-`v1.0.0` surface minimal; if it ever lands it will
re-run only safe, idempotent init steps.

## Exit codes

The exit code is the **worst check**, so doctor works as a pre-launch or CI gate
that blocks only on true blockers:

| Worst status | Meaning | Exit code |
|---|---|---|
| ✓ | everything checked out | `0` |
| ⚠ | something worth fixing that will not stop a launch | `0` |
| ✗ | a hard blocker | non-zero |

A ⚠ alone never fails the process. Ambiguous states (a version banner that
cannot be parsed, a port probe that timed out) resolve to ⚠ rather than ✗, so
doctor does not block you spuriously.

```bash
git paw doctor || echo "fix the ✗ findings before launching"
```

## What it checks

| Group | Checks |
|---|---|
| **Environment** | `git` and `tmux` on `PATH` and at or above their minimum versions (git 2.5 for `git worktree`, tmux 1.8); the working directory is inside a git repository. Missing or too old is ✗. |
| **CLIs** | The AI CLIs that resolve on `PATH` — the known roster plus your `[clis.*]` entries. None resolving is ⚠, surfacing the `No AI CLIs found` launch failure before you hit it. |
| **Config** | `.git-paw/config.toml` exists and parses (unparseable is ✗, absent is ⚠); the resolved `worktree_placement`; any key this version does not recognise (⚠, naming the key). |
| **Spec system** | The explicitly configured spec format and how many specs it discovered. Unconfigured is ⚠ with the "add `[specs]` or pass `--specs-format`" guidance — there is no filesystem auto-detection. |
| **Bundled scripts** | `sweep.sh`, `broker.sh`, and `docs-fetch.sh` exist under `.git-paw/scripts/`, are executable, and match this binary's embedded copies. Missing or non-executable is ✗; content drift is ⚠ ("stale"). Also checks for a Python 3 interpreter, which every bundled script needs — absent is ⚠, not ✗, because core `start`/`add`/`remove` needs no Python. |
| **Broker** | When `[broker] enabled = true`, that the configured `bind`/`port` is free or already serving a git-paw broker. Another service on the port is ⚠. When disabled, an informational ✓ noting the pure-manual baseline. |
| **Supervisor** | When `[supervisor] enabled = true`, that each configured gate command's binary resolves on `PATH` (✗ per missing binary) and that `sweep.sh` is installed. When disabled, an informational ✓. |
| **Hygiene** | The required `.gitignore` entries (including `.git-paw/worktrees/`); session receipts that claim active while their tmux session is gone; registered worktrees whose directory no longer exists. Each is ⚠ with a `git paw purge --stale` remedy. |

### The supervisor check is project-agnostic

The gate-command verbs doctor probes come from **your** `[supervisor]`
configuration — the resolved stack preset — never from a hard-coded toolchain.
A Node project configured with `test_command = "npm test"` sees `npm` checked; a
Go project sees `go`. git-paw never assumes every consumer builds with the
toolchain it happens to use itself.

## Machine-readable output

`--json` emits the same checks as one JSON document and suppresses the human
rendering. The exit-code contract is identical, so scripts and agents can branch
on either.

```bash
git paw doctor --json
```

```json
{
  "status": "warn",
  "checks": [
    {
      "group": "Environment",
      "name": "git",
      "status": "ok",
      "detail": "git version 2.39.5 (/usr/bin/git)",
      "remedy": null
    },
    {
      "group": "Spec system",
      "name": "spec system",
      "status": "warn",
      "detail": "no spec system configured — spec-driven launch is unavailable",
      "remedy": "add a [specs] section to .git-paw/config.toml … or pass --specs-format"
    }
  ]
}
```

Every entry carries `group`, `name`, `status` (`"ok"` / `"warn"` / `"fail"`),
`detail`, and `remedy` (`null` on a ✓). The top-level `status` is the worst
check.

## Live smoke check

Doctor is **static** by default: it inspects, and never spawns a tmux session or
an AI CLI. Pass `--live` to add a **Live smoke** group that runs the full session
lifecycle (`start` → `add` → `remove` → `stop`) against an isolated throwaway
repository and a dummy CLI, and folds the verdict in as one more check:

```bash
git paw doctor --live
```

This is the same harness the internal `git paw selftest` command drives — see
[Selftest](selftest.md) for the full isolation recipe. It is slower, it needs
tmux, and it is the only arm of doctor that writes anything: strictly inside its
own `.git-paw/tmp/` sandbox, which it removes again on both the success and
failure paths. A skip (no tmux) is ⚠, not ✗ — the Environment group already
reports a missing tmux as the hard failure.

## Configuration

Doctor introduces no configuration fields. It reads the configuration you
already have (`[broker]`, `[supervisor]`, `[specs]`, `[clis.*]`,
`worktree_placement`) and reports what it resolved.

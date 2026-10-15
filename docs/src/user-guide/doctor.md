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
  ✓ sandbox                 sandbox-exec available for optional worker confinement

CLIs
  ✓ detected CLIs           2 available: claude, codex

Config
  ✓ config.toml             .git-paw/config.toml parses
  ✓ worktree placement      child — worktrees live in <repo>/.git-paw/worktrees/

Spec system
  ⚠ spec system             no spec system configured — spec-driven launch is unavailable
      ↳ add a [specs] section to .git-paw/config.toml (type = "openspec" | …) or pass --specs-format
  ✓ spec-worker CLI         no default_spec_cli configured — spec workers fall back to default_cli or the interactive picker

Bundled scripts
  ✓ _paw_common.sh          installed, executable, matches this binary
  ✗ sweep.sh                .git-paw/scripts/sweep.sh is missing
      ↳ run `git paw init` to (re)install the bundled helper scripts
  ✓ broker.sh               installed, executable, matches this binary
  ✓ docs-fetch.sh           installed, executable, matches this binary
  ✓ python3                 Python 3.12.4 (python3)

Broker
  ✓ broker                  disabled — the pure-manual baseline (no agent coordination, no dashboard feed)

Supervisor
  ✓ supervisor              disabled — agents run without a supervising pane

Hygiene
  ✓ .gitignore              every git-paw entry is ignored
  ✓ session state           no stale session receipt
  ✓ worktree registrations  every registered worktree exists on disk

17 ✓ · 1 ⚠ · 1 ✗
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
| **Environment** | `git` and `tmux` on `PATH` and at or above their minimum versions (git 2.5 for `git worktree`, tmux 1.8); the working directory is inside a git repository. Missing or too old is ✗. Also whether this OS's FS-scoped sandbox backend (`sandbox-exec` on macOS, `bwrap` on Linux/WSL) is on `PATH` — absent is ⚠ with a pointer to the [sandbox guide](sandbox.md), never ✗, because the sandbox is optional. |
| **CLIs** | The AI CLIs that resolve on `PATH` — the known roster plus your `[clis.*]` entries. None resolving is ⚠, surfacing the `No AI CLIs found` launch failure before you hit it. Plus two read-only reports about [per-CLI setup](cli-setup.md): **setup state** and **broad grants** (below). |
| **Config** | `.git-paw/config.toml` exists and parses (unparseable is ✗, absent is ⚠); the resolved `worktree_placement`; any key this version does not recognise (⚠, naming the key). |
| **Spec system** | The explicitly configured spec format and how many specs it discovered. Unconfigured is ⚠ with the "add `[specs]` or pass `--specs-format`" guidance — there is no filesystem auto-detection. Also reports the effective spec-worker CLI: the CLI each discovered spec would actually launch with (its own `paw_cli` when set, else `default_spec_cli`). Warns (⚠) when a configured `default_spec_cli` is never actually reachable by any discovered spec — every one of them pins its own `paw_cli` instead — surfacing a silent mis-resolution (such as a spec-driven `--supervisor` launch quietly using an unexpected, unsandboxed CLI) rather than passing green. Informational (✓) when no specs or no `default_spec_cli` are configured — there's nothing to verify against yet. |
| **Bundled scripts** | `sweep.sh`, `broker.sh`, `docs-fetch.sh`, and the shared `_paw_common.sh` preamble they source exist under `.git-paw/scripts/`, are executable, and match this binary's embedded copies. Missing or non-executable is ✗; content drift is ⚠ ("stale"). Also checks for a Python 3 interpreter, which every bundled script needs — absent is ⚠, not ✗, because core `start`/`add`/`remove` needs no Python. |
| **Broker** | When `[broker] enabled = true`, that the configured `bind`/`port` is free or already serving a git-paw broker **for this repository**. A git-paw broker serving another repository is ⚠ with the `[broker] port` remedy — it is the clash that would refuse your next `git paw start`. A non-git-paw service on the port is a separate ⚠ with its own remedy. When disabled, an informational ✓ noting the pure-manual baseline. |
| **Supervisor** | When `[supervisor] enabled = true`, that each configured gate command's binary resolves on `PATH` (✗ per missing binary) and that `sweep.sh` is installed. When disabled, an informational ✓. |
| **Hygiene** | The required `.gitignore` entries (including `.git-paw/worktrees/`); session state; registered worktrees whose directory no longer exists. Each is ⚠. A receipt that claims active while its tmux session is **gone** gets the `git paw purge --stale` remedy, as does an orphaned worktree registration. A session whose tmux session is still up but whose agent panes have all dropped back to a bare shell (past a short launch grace period) is a separate finding: it names the session, says its agent panes exited, and its remedy is `git paw start`, which recovers it — `purge --stale` never purges such a session. |

### The per-CLI setup checks

Two checks report on [`git paw setup-cli`](cli-setup.md). Both are strictly
read-only — they create, modify and delete nothing — and **neither can ever be
the reason doctor fails**: they never report ✗.

**Setup state**, one per CLI identity that has setup facts or record entries:

- ✓ when nothing is applied, listing the available items and whether each is
  verified or plan-only;
- ✓ when every recorded entry is present, unchanged, and matches what this
  binary would plan;
- ⚠ when a recorded entry is missing or has been edited, with a remedy naming
  `--undo`;
- ⚠ when the applied entries are out of date against this version, with a
  remedy naming `--undo` followed by re-applying;
- ⚠ when a **granted helper script** differs from the copy this binary
  bundles, naming its path. This one matters most: the allowlist lets that
  script run without a prompt, so drift in it is drift in what runs
  unprompted. Helpers are checked in the main checkout and in every session
  worktree, because a granted path is relative and resolves against whichever
  directory the session runs in.

**Broad grants**, an advisory over the allowlist item's target file when it
exists. It lists every `permissions.allow` entry whose shape grants more than
one specific command — a bare tool rule, a single-word prefix grant, a shell
or interpreter wrapper, a state-changing `git` or `git-paw` subcommand, a
far-reaching command like `rm` or `curl` (a loopback `curl` is exempt), a
file-tool rule anchored at the filesystem root or your home directory, or a
helper grant broader than git-paw's own entry for that helper.

Entries are abbreviated to the tool name and the first two words of the
specifier, so a credential embedded later in a rule is never echoed. An
unparseable file is reported as "not inspected", without its content.
git-paw's own planned entries are never listed, whoever added them.

This advisory is a report over stored rules, not a decision about any prompt.
git-paw does not edit these entries and never acts on them — reviewing and
removing the ones you did not intend is yours to do.

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
`worktree_placement`, `default_spec_cli` / `default_cli`) and reports what it
resolved.

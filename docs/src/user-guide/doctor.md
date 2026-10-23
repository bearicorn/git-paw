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
  ✓ spec workflow           no spec system configured — the format declares no workflow entry point

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

18 ✓ · 1 ⚠ · 1 ✗
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
| **CLIs** | The AI CLIs that resolve on `PATH` — the known roster plus your `[clis.*]` entries. None resolving is ⚠, surfacing the `No AI CLIs found` launch failure before you hit it. Plus three read-only reports about [per-CLI setup](cli-setup.md): **setup state**, **broad grants** and **grant residue** (below). |
| **Config** | `.git-paw/config.toml` exists and parses (unparseable is ✗, absent is ⚠); the resolved `worktree_placement`; any key this version does not recognise (⚠, naming the key). |
| **Spec system** | The explicitly configured spec format and how many specs it discovered. Unconfigured is ⚠ with the "add `[specs]` or pass `--specs-format`" guidance — there is no filesystem auto-detection. Also reports the effective spec-worker CLI: the CLI each discovered spec would actually launch with (its own `paw_cli` when set, else `default_spec_cli`). Warns (⚠) when a configured `default_spec_cli` is never actually reachable by any discovered spec — every one of them pins its own `paw_cli` instead — surfacing a silent mis-resolution (such as a spec-driven `--supervisor` launch quietly using an unexpected, unsandboxed CLI) rather than passing green. Informational (✓) when no specs or no `default_spec_cli` are configured — there's nothing to verify against yet. Finally, whether the workflow entry point the launch submits to spec-driven agents is **reachable** from your worktrees (below). |
| **Bundled scripts** | `sweep.sh`, `broker.sh`, `docs-fetch.sh`, and the shared `_paw_common.sh` preamble they source exist under `.git-paw/scripts/`, are executable, and match this binary's embedded copies. Missing or non-executable is ✗; content drift is ⚠ ("stale"). Also checks for a Python 3 interpreter, which every bundled script needs — absent is ⚠, not ✗, because core `start`/`add`/`remove` needs no Python. |
| **Broker** | When `[broker] enabled = true`, that the configured `bind`/`port` is free or already serving a git-paw broker **for this repository**. A git-paw broker serving another repository is ⚠ with the `[broker] port` remedy — it is the clash that would refuse your next `git paw start`. A non-git-paw service on the port is a separate ⚠ with its own remedy. When disabled, an informational ✓ noting the pure-manual baseline. |
| **Supervisor** | When `[supervisor] enabled = true`, that each configured gate command's binary resolves on `PATH` (✗ per missing binary) and that `sweep.sh` is installed. When disabled, an informational ✓. |
| **Hygiene** | The required `.gitignore` entries (including `.git-paw/worktrees/`); session state; registered worktrees whose directory no longer exists. Each is ⚠. A receipt that claims active while its tmux session is **gone** gets the `git paw purge --stale` remedy, as does an orphaned worktree registration. A session whose tmux session is still up but whose agent panes have all dropped back to a bare shell (past a short launch grace period) is a separate finding: it names the session, says its agent panes exited, and its remedy is `git paw start`, which recovers it — `purge --stale` never purges such a session. |

### The per-CLI setup checks

Three checks report on [`git paw setup-cli`](cli-setup.md). All are strictly
read-only — they create, modify and delete nothing — and **none can ever be
the reason doctor fails**: they never report ✗.

**Setup state**, one per CLI identity that has setup facts or record entries:

- ✓ when nothing is applied, listing the available items and whether each is
  verified or plan-only;
- ✓ when every recorded entry is present and unchanged, and the target
  settings file holds every entry this binary would plan;
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

What counts as **applied** is judged against the CLI's target settings file,
not against git-paw's record: a planned entry is applied when that file holds
it at its key path, whoever wrote it. That includes an entry you already had
before you ran `git paw setup-cli`. Setup skips such an entry — there is
nothing to write — and deliberately never records it, because the record
lists only what git-paw itself wrote, so `--undo` can take back exactly that
and nothing else.

The check reads the comparison in both directions. It also warns when the
record holds a permission rule **this version of git-paw no longer plans** — a
grant a previous release wrote and this one has withdrawn. The remedy is the
same either way: `git paw setup-cli <CLI> --undo`, then re-apply the item.

**Broad grants**, an advisory over the allowlist item's target file when it
exists. It lists every `permissions.allow` entry whose shape grants more than
one specific command — a bare tool rule, a single-word prefix grant, a shell
or interpreter wrapper, a state-changing `git` or `git-paw` subcommand, a
far-reaching command like `rm` or `curl` (a loopback `curl` is exempt), a
file-tool rule anchored at the filesystem root or your home directory, or a
helper grant broader than git-paw's own entry for that helper.

Entries are abbreviated to the tool name and the first two words of the
specifier, each capped in length. That bounds the common way a secret ends up
in a rule — a long URL carrying a token — but it is a mitigation, not a
guarantee: a short rule that is itself a secret cannot be told apart from a
short rule that is not, so treat doctor output as you would the settings file
it reads. An unparseable file is reported as "not inspected", without its
content.
git-paw's own planned entries are never listed, whoever added them.

This advisory is a report over stored rules, not a decision about any prompt.
git-paw does not edit these entries and never acts on them — reviewing and
removing the ones you did not intend is yours to do.

**Grant residue** (`project CLI grant residue`) sits beside the advisory and
answers a narrower question: not "is this rule broad?" but "is this a
known-dangerous leftover?". It reads the **main checkout's**
`.claude/settings.json` and `.claude/settings.local.json`, and reports ⚠ when
either holds one of exactly three things:

| Residue | Example | Why it matters |
|---|---|---|
| A `permissions.allow` entry granting `curl` with any arguments | `Bash(curl *)`, `Bash(curl:*)`, `Bash(curl*)` | an unrestricted network client |
| A `permissions.allow` entry naming a helper script with a keystroke or verdict subcommand | `Bash(.git-paw/scripts/sweep.sh approve *)`, `Bash(.git-paw/scripts/sweep.sh verified *)`, `Bash(.git-paw/scripts/sweep.sh *)` | it lets an agent session approve its own permission prompts, or publish a supervisor verdict, without a prompt |
| A `permissions.allow` entry naming the pass-through fetch subcommand | `Bash(.git-paw/scripts/docs-fetch.sh get *)` | it passes an arbitrary URL through, so a session can read any file it names or send repository content to any host |
| A `permissions.allow` entry naming any other subcommand the grant table withholds | `Bash(.git-paw/scripts/sweep.sh classify *)` | a helper subcommand the grant table deliberately withholds from the allowlist item |
| A top-level `allowed_bash_prefixes` key | — | the inert seed git-paw wrote before v0.16.0; it grants nothing in the current CLI and is pure leftover |

Each entry is reported with the reason true of the subcommand it actually
reaches, and a rule reaching several — a whole-script wildcard reaches all of
them — is reported by the gravest.

The helper rules are matched by the helper's name wherever it appears in the
specifier, so a relative, absolute or interpreter-prefixed path is caught
alike. A wildcard is matched in or across the subcommand position: both
`Bash(.git-paw/scripts/sweep.sh *)` and `Bash(.git-paw/scripts/sweep.sh ap*)`
reach `approve`. Two things are deliberately **not** residue: a specific loopback rule
such as `Bash(curl -s http://127.0.0.1:9119/status)`, which names one
endpoint rather than granting the client; and `Bash(.git-paw/scripts/broker.sh *)`,
because the grant table grants that helper whole — it is one of git-paw's own
entries.

Only `Bash` rules can be residue — a `Read` or `Edit` rule naming a helper
grants reading or writing that file, not running it.

The remedy is **by hand**. git-paw never rewrites a grant it did not write, so
it names each file and each matching entry and leaves the removal to you. The
check is ✓ when neither file holds a match, and ⚠ stating the file was not
inspected when one of them exists but is not valid JSON. When neither file
exists there is nothing to report and the check is not shown at all.

Entries are abbreviated as in the advisory above, with the same caveat — it
bounds what is printed rather than guaranteeing a secret never appears. A
helper grant gets one extra protection: it is rendered from the word naming
the helper, so an environment assignment in front of it, which is exactly
where a token would sit, is replaced by `…` instead of printed.

Like the other two, it is read-only, never reports ✗, and never influences
how a prompt is handled.

A rule can appear in both reports — `Bash(curl *)` is both broad and residue.
That is intended: one is advice about breadth, the other names a specific
danger.

### The spec-workflow reachability check

A [spec-driven launch](spec-driven-launch.md) submits your spec tool's
workflow command — `/opsx:apply` for OpenSpec — as a
[second message](spec-driven-launch.md#the-workflow-entry-point) once the
agent's pane goes idle. That command's definitions are installed by the spec tool into an
agent-CLI directory at the repository root (for `claude`:
`.claude/commands/opsx/apply.md` and
`.claude/skills/openspec-apply-change/SKILL.md`), and that directory is
frequently gitignored. `git worktree add` populates a new worktree from a
**commit**, so a sibling worktree never sees an uncommitted definition and the
agent quietly replaces the spec workflow with an ad-hoc one. The **spec
workflow** check surfaces that before you launch.

Each spec-driven worker CLI the discovered specs resolve to is judged on its
own:

- ✓ when a definition is **committed** at the main checkout's `HEAD` — every
  new worktree checks it out, whatever the placement. Staging it is not
  enough: an index entry does not travel into a new worktree;
- ✓ when [`worktree_placement`](worktree-placement.md) is `child` and a
  definition exists at the repository root — the worktree sits under the root,
  so the CLI's ancestor search still finds it;
- ⚠ when the CLI's locations are known and no definition reaches the
  worktrees — either placement is `sibling` and every definition found at the
  root is uncommitted, or none is found at all. The finding names the command,
  the unreachable CLIs (and only those — a reachable CLI alongside them passes
  silently), and the locations checked.

Under `sibling` placement the remedy offers both fixes: **commit the command
definitions** so every new worktree checks them out, or set
`worktree_placement = "child"` so the worktrees sit under the repository root.
Under `child` placement the only cause left is that no definition exists at
all, so the remedy points at installing it with your spec tool and committing
it.

Definitions are looked for in the **main checkout**, so running doctor from
inside an agent's worktree — which never carries them itself — reports the same
verdict as running it at the repository root.

Two states are reported rather than warned about:

- ✓ **not applicable** when the resolved spec format declares no workflow
  entry point (Markdown, Spec Kit, Superpowers, or no spec system), or
  when no spec-driven worker CLI resolves — there is nothing to reach;
- ✓ **could not be verified** for a CLI whose definition locations git-paw does
  not know. Where a CLI keeps its command definitions is per-CLI knowledge;
  git-paw lists only the locations verified against a real install (today,
  `claude`) rather than guessing a path and warning about a definition that
  never lived there.

Like the setup checks, this one is read-only and **never ✗** — an unreachable
workflow degrades a launch, it does not block one.

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

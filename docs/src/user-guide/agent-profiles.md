# Agent Profiles

An **agent profile** is a named configuration entry that says what an agent
is *for*: a CLI pinned to its launch arguments (such as a model), an optional
role skill, an approval level and a one-line purpose. Profiles live in
`[profiles.<name>]` in `.git-paw/config.toml` (or the user-level config) and
come in two kinds:

- a **worker** profile launches a normal coding agent — pane, branch,
  worktree — exactly like a plain CLI does, plus whatever the profile adds;
- a **delegate** profile runs one headless, one-shot agent at the repository
  root for the orchestrator, through `git paw delegate`, and returns a
  compact result.

Profiles are configuration only. git-paw writes no vendor agent-definition
file for them.

## Declaring profiles

```toml
# A worker profile: the `claude` CLI pinned to a smaller model.
[profiles.routine]
kind = "worker"
description = "Routine implementation"
cli = "claude"
args = "--model small-model"

# A worker profile that also gets a reviewer role skill.
[profiles.review]
kind = "worker"
description = "Careful review-and-fix work"
cli = "claude"
skill = "reviewer"          # .agents/skills/reviewer/SKILL.md, a user skill, or a bundled one

# A delegate profile for read-only exploration.
[profiles.explorer]
kind = "delegate"
description = "Read-only codebase sweep"
cli = "claude"
skill = "explorer"

default_profile = "routine"
```

| Key | Kinds | Meaning |
|---|---|---|
| `kind` | both | `worker` or `delegate` (required) |
| `description` | both | one line, shown in the picker and `git paw delegate --list` (required) |
| `cli` | both | a detected CLI id or a `[clis.<name>]` key (required) |
| `args` | both | extra launch arguments, placed right after the CLI's own `args` |
| `skill` | both | role skill; optional for workers, required for delegates |
| `approval` | both | `manual`, `auto` or `full-auto`; delegates refuse `full-auto` |
| `timeout_seconds` | delegate | run time limit, default 600; refused on a worker |

A profile name matches `[A-Za-z0-9][A-Za-z0-9._-]{0,63}`. A bad entry fails
the configuration load with an error naming `profiles.<name>` and the key.
Whether `cli` and `skill` resolve is checked when the profile is used. The
repository config replaces a user-level profile of the same name whole, and
`default_profile` merges like `default_cli`.

A configuration without `[profiles]` and `default_profile` behaves exactly as
before.

## Worker profiles

A worker launched with a profile:

- runs the profile's CLI with the profile's `args` appended right after the
  CLI's own `args` — ahead of the CLI's operator-isolation arguments, its
  sandbox arguments, the approval flags and the session arguments — so it
  keeps every protection a plain worker on that CLI has;
- runs at the profile's `approval` when set, else at `[supervisor]
  agent_approval`; the level applies to that worker only, and the
  native-sandbox refusal and the unsandboxed full-auto warning are evaluated
  at it;
- gets the profile's role skill (when set and not `coordination`) written
  into its managed block *after* the coordination skill, under a `## Role
  skill: <name>` heading, and listed in the skills index;
- is recorded with its profile in the session file and in
  `git paw status --json` (`"profile"`), and is relaunched as that profile on
  recovery. A recorded profile that no longer exists falls back to the
  recorded CLI with one warning.

A profile whose CLI is not available, or whose role skill does not resolve,
fails the launch before any worktree is created. Naming a delegate profile
where a worker is expected fails with an error saying so.

### Selection order

| Launch | Order |
|---|---|
| `git paw start` (no specs) | `--profile` or `--cli` for every branch → a `--preset`'s CLI → the picker, starting on `default_profile`, else `default_cli` |
| `git paw start --specs` / `--from-all-specs` | `--profile` or `--cli` → the spec's `paw_profile` or `paw_cli` → `default_spec_cli` → the picker, starting on `default_profile`, else `default_cli` |
| `git paw add` | `--profile` or `--cli` → with `--from-spec`, the spec's `paw_profile` or `paw_cli` → the session's first worker (its profile, else its CLI) → `default_profile` → `default_cli` |

`--profile` conflicts with `--cli` (and with `--preset` on `start`). Every CLI
picker lists the worker profiles after the CLIs, as `<name> — <description>
[<cli>]`; a profile and a CLI of the same name stay distinct. A spec setting
both `paw_profile` and `paw_cli` fails the launch, as does a `--profile`,
`paw_profile` or `default_profile` naming no profile or a delegate profile.

## Delegate profiles

```bash
git paw delegate --list
git paw delegate explorer --task "map the config loader"
echo "audit feat/x at abc123, change add-x" | git paw delegate spec-audit
```

A delegate run is one non-interactive invocation of the profile's CLI in its
headless form (`[clis.<name>].headless_args`, else the built-in table). Its
standard input is a document made of the bundled delegate framing, the
rendered role skill and the task — the task never appears on the command
line. Its argv is, in order: the CLI's command and `args`, the profile's
`args`, the CLI's operator-isolation arguments (unless
`inherit_operator_config = true`), declared sandbox arguments, the approval
flags for the profile's level, session-id arguments with a fresh id (when the
CLI has a template), then the headless arguments. No shell runs.

`git paw delegate` prints the delegate's standard output — capped at 64 KiB,
with a final line naming the full run log — and exits `0` only when the
delegate exited `0` within its timeout. Standard error goes only to the run
log. Every run writes `.git-paw/logs/delegates/<run>.log` and appends one
line to `.git-paw/logs/delegates/runs.jsonl` with the profile, CLI, session
id, start time, duration and outcome (`ok`, `failed` with its exit code, or
`timeout`) — never the task or the result.

### Bounds and privileges

A delegate is an agent with execution power started at the repository root on
another agent's request, with no pane for anyone to watch. Its bounds:

- **Level.** `manual` by default: git-paw passes no permission-widening flag,
  and the CLI's own non-interactive mode decides what may run. `auto` is the
  opt-in for a role that edits. `full-auto` is refused at load.
- **Where.** Only from the main working tree (refused when the caller's
  working directory is a linked worktree), with the main working tree as its
  working directory. The guard checks only the caller's working directory: a
  worker that can run shell commands can `cd` into the main checkout first,
  so it is defence in depth, not a boundary. No nesting: a run inside a
  delegate run is refused.
- **Concurrency.** At most four `manual` runs at once; an `auto` run runs
  alone and excludes every other run. A run that cannot be admitted is
  refused immediately, never queued. A run whose process died does not count.
- **Time.** `timeout_seconds` (default 600); on expiry the run's whole process
  group is killed and the outcome is `timeout`. The delegate runs in its own
  process group, recorded with its deadline: if `git paw delegate` is
  interrupted (SIGINT, SIGTERM, SIGHUP) it kills the group first, and a group
  orphaned some other way is killed by the next admission once its deadline
  passes — until then it still counts toward the limits. A recorded group is
  only ever signalled when it is a real group other than git-paw's own and
  its leader still runs the delegate program. Give the call a tool
  timeout of at least `timeout_seconds`, or run it in the background.
- **Environment.** The run carries `GIT_PAW_DELEGATE=<name>` and its CLI's
  operator isolation, and has `GIT_PAW_BROKER_URL` removed. It gets no
  authority credential: `git paw __authority`, `git paw __send`, the bundled
  `broker.sh` and `sweep.sh`, and an MCP server started inside the run all
  refuse under the marker; that MCP server also registers no publish tool
  (`publish_status`, `publish_artifact`, `publish_blocked`,
  `publish_question`). A delegate profile's `args` may not carry any of its
  CLI's `auto` or `full-auto` approval flags (as `flag value` or
  `flag=value`), its sandbox-disabling flags, or a permission-bypass value
  such as `bypassPermissions` or `danger-full-access` — the level comes from
  `approval` alone.
- **No prompt relay.** No pane exists, so nothing escalates or answers a
  delegate's permission prompts.

The marker refusals are defence in depth: a delegate allowed to run shell
commands could unset the variable or run public commands. The boundary is the
approval level plus, where configured, the CLI's declared sandbox
(`[clis.<name>].sandbox`) — see [Security Posture](security-posture.md).

### Bundled delegate roles

Four role skills ship with git-paw, installed at the repository root under
`.git-paw/skills/` and never into a worktree:

| Role skill | Returns |
|---|---|
| `explorer` | a map: one `<path> — <one-line role>` line per relevant path, never file contents |
| `spec-auditor` | the spec audit gate's compressed verdict |
| `doc-auditor` | the doc audit gate's compressed verdict |
| `security-auditor` | the security audit gate's compressed verdict |

Each auditor includes its gate's procedure from the orchestrator package's
`references/gates/<gate>.md` at render time, so every gate procedure has one
source. A verdict is the gate name, `pass` or `fail`, and one prefixed line
per finding; a delegate never publishes it — the orchestrator does.

The orchestrator skill's *Delegating gates* section directs audits and broad
exploration to a matching delegate profile first, then to the CLI's own
isolated subagents, then inline.

## Usage attribution

`git paw status --json` carries `usage_by_profile`, keyed by profile name:
its `kind`, how many workers and delegate runs used it, the sum of their known
usage, and how many had unknown usage. Workers count by their recorded
profile; delegate runs count from `runs.jsonl` runs that started at or after
the session's creation, resolved through the CLI's usage facet with each
run's session id. `usage_total` is unchanged — profiled workers still count
under their CLI, and delegate spend is not added to the totals or the budget.
The drive loop's exit summary adds one line per profile when a profile was
used.

# Supervisor

This chapter mirrors the user-facing prose for the bundled
`assets/agent-skills/supervisor.md` skill — the doctrine the supervisor agent
runs against in supervisor mode. Sections here document the same rules the
embedded skill teaches, so users reading the mdBook can understand supervisor
behaviour without opening the skill file directly.

For the launcher-level "how do I start supervisor mode" walkthrough, see
[Quick Start: Supervisor Mode](../quick-start-supervisor.md). For the
broker-side message contract the supervisor exchanges with coding agents, see
[Agent Coordination](coordination.md).

## Pane Layout and Labelling

When you attach to a supervisor session, git-paw styles the tmux panes so the
boundary between the top row (supervisor + dashboard) and the agent grid below
is easy to see. By default each session gets:

- **Heavy pane borders** (`━┃`) instead of tmux's default light `─│` lines, so
  the rows visibly separate even with four or more agents.
- **A per-pane label strip** above every pane showing its index and role:

  ```text
  ┏━ 0: supervisor ━━━━━━━━━┓┏━ 1: dashboard ━━━━━━━━━┓
  ┃ (supervisor agent)      ┃┃ (live agent table)     ┃
  ┗━━━━━━━━━━━━━━━━━━━━━━━━━┛┗━━━━━━━━━━━━━━━━━━━━━━━━━┛
  ┏━ 2: feat/cold-start ━━━━┓┏━ 3: feat/conflict-det ━┓
  ┃ (coding agent)          ┃┃ (coding agent)         ┃
  ┗━━━━━━━━━━━━━━━━━━━━━━━━━┛┗━━━━━━━━━━━━━━━━━━━━━━━━━┛
  ```

  Pane `0` is always `supervisor`, pane `1` is `dashboard`, and the agent panes
  are labelled with their branch id (`feat/<branch>`). The index shown is the
  same one you pass to `tmux send-keys -t paw-<project>:0.<N>`, so you can
  cross-reference the label strip with the broker's `/status` agent listing.
- **An active-pane highlight** — the focused pane's border is cyan-bold while
  inactive panes are dimmed, so you always know which pane has keyboard focus.

These options are applied only to git-paw-managed sessions (`paw-*`); your
other tmux sessions keep their own styling. They apply to plain
`git paw start` sessions too, not just supervisor mode.

Set `[layout].border_affordances = false` in your config to opt out and
inherit your own tmux styling — see the
[Layout configuration reference](../configuration/README.md#layout). On tmux
older than 3.2 the heavy border lines aren't recognised; git-paw prints a
warning and continues with the remaining affordances rather than failing.

## Understanding what the supervisor is doing

You rarely need to read the supervisor's pane to know what it is up to. The
supervisor tags each status heartbeat it publishes with a **phase** — a
short label for its current lifecycle activity — and a structured `detail`
body. The dashboard's supervisor row shows the current phase in its status
column, and the MCP `get_session_status` tool returns the same `phase` and
`detail` so external tooling can read it programmatically. (Both surfaces
degrade gracefully: a supervisor that hasn't published a phase renders with
its plain status, exactly as in v0.5.0.)

The supervisor emits these phase-tagged statuses through the bundled
`.git-paw/scripts/sweep.sh status-publish` helper — `--phase <phase>` sets the
label and `--detail '<json-object>'` carries the structured body, while the
plain `status-publish <message>` form (no flags) publishes the v0.5.0 shape
unchanged. The helper shapes the `agent.status` payload internally, so the
supervisor never hand-rolls the JSON and the least-privilege by-path allowlist
grant for `.git-paw/scripts/sweep.sh` covers every phase without a broad `curl`
rule.

The phases, with what each one means:

| Phase | The supervisor is… |
|---|---|
| `baseline` | recording the regression baseline on the default branch at boot |
| `sweep` | scanning agent panes and the message stream for new events |
| `audit` | verifying a branch through the gates (the `detail.audit_step` field names which of `tests`, `regression`, `spec`, `docs`, `security` is running) |
| `merge` | cherry-picking or merging a verified branch into the base |
| `feedback` | sending feedback or a proactive route to a peer agent |
| `intent_watch` | tracking declared file intents for overlap |
| `learnings` | recording a structured learning record |
| `checkpoint` | enumerating a multi-publish batch before a risky action, so it can recover from a stream timeout |
| `idle` | waiting for the next event — nothing in flight |

The phase taxonomy is an open set: the supervisor may publish a value not in
this table, and the dashboard renders it as-is. The supervisor emits a status
on every phase transition and at most once per ~30 seconds while it stays in
the same phase, so the row stays current without flooding the
[broker log](dashboard.md). Phases are surfaced for the **supervisor row
only** — coding-agent rows render as they always have. (The one exception is
the supervisor-published `stuck-on-prompt` alert, which appears on a stalled
coding agent's own row.)

## Resolve Pane to Agent via `pane_current_path`

Before the supervisor `tmux capture-pane`s or `tmux send-keys`s a specific
agent, it needs the pane index for that agent. The bundled supervisor skill
is explicit that **pane indices are NOT alphabetical by `agent_id`, NOT in
the CLI-argument order from `git paw start --specs A B C`, and SHALL NOT be
inferred from `git paw status` output or the dashboard's row order** (both
of those are sorted alphabetically by the broker, which has no relationship
to the launcher's internal scan order).

The canonical resolution command queries tmux directly:

```bash
tmux display-message -t paw-<project>:0.<pane> -p '#{pane_current_path}'
```

The output is the pane's working directory. For coding-agent panes that is
the agent's worktree path, whose basename ends in `<project>-feat-<branch>`
— the authoritative `agent_id` (with the slash form `feat/<branch>` for git
operations). A pane whose `pane_current_path` ends in `myproj-feat-auth`
belongs to agent `feat-auth`.

The supervisor agent builds the `{pane_index → agent_id}` map once per
session and reuses it; re-resolution only happens when the supervisor
notices an inconsistency.

The bundled `.git-paw/scripts/sweep.sh` invokes this command on every sweep
iteration. If the helper is missing for any reason, the supervisor falls
back to invoking `tmux display-message` directly — this is the documented
escape hatch.

The supervisor MUST NOT use `git paw status` output (or the dashboard's row
order) as a mapping source — both are sorted alphabetically by the broker
and have no relationship to the launcher's pane assignment. Always resolve
via `pane_current_path` first.

## Pane-driving and cross-worktree git disciplines

The bundled supervisor skill hardens three behaviours so a multi-agent
session runs without the human approving the supervisor's every command:

- **Use `.git-paw/scripts/sweep.sh` for pane work; never inline loops.**
  Ad-hoc `for p in <panes>; do tmux send-keys -t ...:0.$p ...; done` loops
  trip the `simple_expansion` permission gate on each iteration, so one
  sweep becomes one approval prompt per pane. The helper does the explicit
  per-pane sends with no variable expansion. A single explicit
  `tmux send-keys -t ...:0.<index>` is reserved for pushing one typed reply
  to one pane.
- **Never send-keys to the supervisor's own pane (pane 0).** The supervisor
  is pane 0; targeting it interrupts its own in-flight command. `sweep.sh`
  targets agent panes by design.
- **Cross-worktree git uses `git -C <path>`, never `cd <path> && git`.**
  A bare `cd` trips the untrusted-hooks warning on some CLIs and leaks the
  working directory, so a later mutating git command can land on the wrong
  branch. `git -C` scopes the directory to the single command.

The skill also nudges any agent whose uncommitted working set exceeds ~10
files to commit its completed section, preserving per-section verification
granularity.

## Spec audit governance sub-step

When a project's `.git-paw/config.toml` lists governance documents under
`[supervisor].governance.docs`, the supervisor reads each doc as part of the
Spec Audit gate and flags drift between the diff and the doc's checklist.
The five canonical doc-checklist examples are:

- **DoD** (Definition of Done) — walk each `- [ ]` item against branch state.
- **ADRs** (Architectural Decision Records) — verify new architectural
  decisions (new deps, new patterns) have a matching ADR.
- **security.md** — walk each security checklist item against the diff.
- **test-strategy.md** — check that test composition matches the documented
  strategy.
- **constitution.md** — check the diff against documented principles
  (e.g. "no panics in library code").

Findings surface as standard `agent.feedback` errors tagged `[doc audit]`
mixed in with other doc-audit gaps. See [Governance](governance.md) for the
config schema and how the supervisor reads each doc.

## Common dev-command allowlist

A bundled **universal** preset whitelists routine, stack-neutral dev commands
so the supervisor stops escalating every `git commit`, `git push`, `git diff`,
`grep`, or broker curl on `127.0.0.1`. The preset is on by default and ships
with the launcher. Toolchain-specific commands (`cargo …`, `npm …`, `pytest`,
`go …`) are **not** in the universal set — opt into them per stack.

To opt out for a session, set:

```toml
[supervisor.common_dev_allowlist]
enabled = false
```

To seed a toolchain's curated grants, name its stack preset; to add anything
not covered by a named stack (e.g. `just`, `nox`, a custom test runner), use
the `extra` field:

```toml
[supervisor.common_dev_allowlist]
enabled = true
stacks = ["rust", "python"]
extra = ["just check", "nox -s tests"]
```

Both `stacks` entries and `extra` patterns are prefix-matched against the
captured command line, the same way the universal patterns are. See
[Configuration](../configuration/README.md) for the named-preset contents and
the full schema.

### Run dev commands bare — no exit-code-probe wrappers

The allowlist works because each grant is a bare command **prefix** that
generalises across argument variants. Wrapping a command in an exit-code probe
— `<cmd> && echo "EXIT $?"`, `<cmd>; echo $?`, `RC=$?; echo "$RC"` — defeats
that: the probe text varies per run, so the CLI's command-string permission
whitelisting never matches the next invocation and the command re-prompts every
time. The bundled supervisor and coordination skills instruct agents to run dev
commands bare and read the exit status directly; keep your own commands to the
same shape so a seeded prefix actually suppresses the prompt.

## Auto-approve classification

When `[supervisor.auto_approve]` is enabled, the poll loop classifies each
captured permission prompt into **escalate-to-human** or **auto-approve**. The
decision is deterministic and reviewable — the same logic ships in the bundled
`sweep.sh` helper (`sweep.sh classify`, fed a pane capture on stdin) so the
shell path and the Rust path agree.

The classifier reads the prompted **command slice** — the text between the
`Bash command` / `Bash(…)` header and the confirmation question — not the
surrounding narration. A supervisor that merely *mentions* `rm -rf /` in prose
is never mistaken for a prompt to run it.

### Decision order

1. **Live-prompt gate.** The classifier acts only when the footer marker
   `Esc to cancel` appears within the last ~4 non-blank lines of the capture.
   A prompt that has scrolled away, or pane text that is just narration, is
   treated as *not live* — no keystrokes are sent. This kills phantom
   approvals.
2. **Danger-list (escalate wins).** A curated danger-list is evaluated
   **first** and overrides any allowlist match. It covers `rm -rf` / `rm -fr`,
   `git push`, `--force` / `force-push`, `reset --hard`, `git rebase`,
   branch-switching `git checkout `, `branch -D`, `git worktree remove`,
   `clean -fd` / `clean -fdx`, `sudo`, `mkfs`, `dd if=`, `> /dev/`, `chmod -R`,
   `chown -R`, and `pkill` / `kill`, plus a per-OS addendum (macOS `diskutil`
   and raw `/dev/disk*`; Linux/WSL `/dev/sd*`, `/dev/nvme*`, `mkfs*`). A
   danger match always escalates to you — even when the verb is otherwise
   whitelisted (so `git push` escalates although `git` is a safe verb).
3. **Scratch-path exception.** An `rm -rf` / `rm -fr` does *not* escalate when
   **every** target is repo/OS scratch: `/tmp/paw-*`, `/private/tmp/paw-*`, a
   `$TMPDIR`-rooted `paw-*`, or any path under `.git-paw/tmp/`. This also covers
   `rm -rf "$VAR"` when `$VAR` resolves (via the captured environment or a
   preceding `VAR=…` assignment) to such a path. If a variable cannot be
   resolved, or **any** target lies outside the scratch set, the whole command
   escalates (fail-safe).
4. **Worktree-confined `git add` / `git commit`.** These pre-approve when the
   agent's worktree resolves to a real directory (the same canonicalize-then-
   `starts_with` boundary check used for file edits), so an unattended agent can
   stage and commit its own work without stalling. `git push` is **not** covered
   — the danger-list escalates it.
5. **Read-mostly verb allowlist.** Routine verbs auto-approve: `curl`, `cat`,
   `ls`, `grep`, `rg`, `git`, `echo`, `sed`, `awk`, `find`, `wc`, `head`,
   `tail`, `jq`, `mkdir`, `touch`, `openspec`, `just`, `export`, `tmux`, `env`
   (plus your configured `safe_commands`). This is subordinate to the
   danger-list above.
6. Anything else is **Unknown** and forwarded to you.

### Re-confirm before send, and the pane 0 exclusion

The live-prompt gate above runs at *detection*. A second, independent check
runs at *send* time: immediately before dispatching the approval keystrokes,
the approver re-captures the target pane and confirms a permission-prompt
marker is still present in the last ~4 non-blank lines. If the prompt cleared
between the decision and the send — the agent moved on, or you answered it
first — **no keystrokes are sent**. This closes the stray-input race where the
option digit would otherwise land in the CLI's chat box as literal text,
polluting context and leaving dangling unsubmitted commands.

The blind send-keys path also never targets **pane 0** (the supervisor's own
pane): `sweep.sh approve 0` sends nothing and reports that pane 0 is excluded,
and the auto-approver skips pane 0 entirely. Clearing the supervisor's own
prompt is a distinct, non-blind concern.

Both the bundled `sweep.sh approve` helper and the in-binary auto-approver pass
through this same gate, so the shell path and the Rust path stay race-safe in
lockstep. No new broker message is involved: the trigger that a pane is
awaiting approval is the existing `agent.status` with `phase:
"stuck-on-prompt"`, and an unsafe or unknown prompt is escalated with the
existing `agent.question`.

### Option selection and the arbitrary-code policy

When the classifier approves, it selects the prompt option by shape: a 2-option
`Yes` / `No` prompt takes option 1 (`Yes`); a 3-option `Yes` /
`Yes, and don't ask again` / `No` prompt takes option 2 (the **permanent broad
grant**) only when the command's verb is read-mostly-allowlisted **and** is not
an arbitrary-code runner. Arbitrary-code runners — `python`, `bash -c`,
`sh -c`, `eval`, `node`, or any bare ` -c ` code-string flag — take the
one-time `Yes` only and **never** receive a permanent grant: a standing grant
on `python -c` is effectively a standing grant on anything.

## Manual approvals

The preset only covers the commands it knows about. Everything else — a
project-local script, a one-off `podman` invocation, an out-of-worktree file
write — the supervisor **forwards to you** for a manual decision instead of
auto-approving it. `git paw approvals` reports those forwarded commands so you
can decide which deserve promotion to the bundled preset or your project
allowlist, closing the loop on recurring prompts.

### What gets recorded

Each time the supervisor's poll loop forwards a command it could not
auto-approve, it appends one JSON line to
`.git-paw/sessions/<session>.manual-approvals.jsonl`:

```json
{ "timestamp": "2026-05-29T12:34:56Z", "agent_id": "feat/auth",
  "pattern": "make integration-test", "first_seen": true }
```

The supervisor runs *outside* the agent's CLI pane, so it cannot see whether
you ultimately pressed Yes or No on the prompt. What it records is the honest,
observable signal: **a command that required a manual decision**. A command
forwarded seven times is a strong allowlist candidate regardless of each
individual yes/no — the friction it causes is what you want to remove.
Auto-approved (preset-matched) commands are **not** recorded.

### Reporting

```bash
git paw approvals                       # active session, text table
git paw approvals --json                # machine-readable
git paw approvals --session paw-other   # a specific session
git paw approvals --limit 5             # top 5 patterns by count
```

The text table sorts patterns by how often each was forwarded, with a
`SUGGEST` hint for where to promote it:

```text
PATTERN                       COUNT  SUGGEST
make integration-test             7  bundled preset candidate
./scripts/deploy-staging.sh       3  project allowlist
podman build -t paw-ci .          2  bundled preset candidate
```

A pattern is suggested for the **project allowlist** when it looks
project-specific (a `./`-rooted script path, or it contains the project or
branch name); otherwise it is a **bundled preset candidate**. The `SUGGEST`
column is a hint, not a rule — the suggestion is a starting point, and
`git paw approvals` never edits the preset or the allowlist for you.

When `[supervisor] learnings = true`, the first sighting of each pattern also
emits a `permission_pattern` learning so it surfaces in
[Learnings Mode](learnings.md) alongside the other patterns.

### Opting out

Recording is on by default. To disable both the log writes and the derived
learnings emission for a session:

```toml
[supervisor]
manual_approvals_log = false
```

The opt-out affects *writes* only — `git paw approvals` still reads any
pre-existing log. See [Configuration](../configuration/README.md) for the
field reference.

## Repo-configurable gate commands

The supervisor's five verification gates each invoke a configurable command
substituted from `[supervisor]` keys at session boot. The eight keys are:

- `test_command`
- `lint_command`
- `build_command`
- `fmt_check_command`
- `doc_build_command`
- `doc_tool_command` — API-doc generator, separate from `doc_build_command`
- `spec_validate_command`
- `security_audit_command`

When a key is missing or empty, the placeholder renders as `(not configured)`
in the supervisor skill and the supervisor **gracefully skips the tooling
invocation** for that gate — the gate's manual review still applies.
`doc_tool_command` is the one exception: when unset it renders as the empty
string (not `(not configured)`) so the surrounding prose reads naturally for
projects that don't ship an API-doc generator. See
[Configuration](../configuration/README.md) for defaults and examples.

### Polyglot bundled skills

The bundled supervisor and coordination skills ship as a single template
each, but they render per session against the project's stack and the
resolved spec backend. Three placeholders make this work:

- `{{DOC_TOOL_COMMAND}}` substitutes the `doc_tool_command` config value
  above so doc-audit prose names the actual API-doc tool for your stack.
- `{{DEV_ALLOWLIST_PRESET}}` is generated from the bundled
  `DEV_ALLOWLIST_PRESET` constant at render time, so adding a new
  auto-approve family to the preset changes the supervisor's safe-command
  prose without a skill-template edit.
- `{{SPEC_PATH_DOCTRINE}}` renders a per-backend path doctrine: OpenSpec
  sessions see `openspec/changes/<name>/...` references, Spec Kit sessions
  see `.specify/specs/<feature>/...`, Markdown sessions see the flat
  `paw_status: pending` shape, and multi-backend sessions list each
  present backend's conventions in one paragraph.

A CI no-leak audit asserts that the rendered supervisor skill contains
no stack-specific tokens (`cargo`, `rustdoc`, `.rs:`, `Cargo.toml`,
`rustc`) outside the explicitly-allowed `<!-- allowlist-prose -->` span
that contains the `DEV_ALLOWLIST_PRESET` enumeration.

## Verification runs the whole suite (never fail-fast)

The supervisor's testing gate runs the configured test command in a
whole-suite / no-fail-fast mode. Most test runners stop at the first failing
test group, so a single early failure — typically an environment-specific
**guard test** rather than a code defect — can hide every later suite and
produce a false PASS. A run that aborted early is **incomplete, not a pass**;
a testing PASS requires the full suite to have executed.

For git-paw itself, the no-tmux-server guard test trips whenever any `paw-*`
tmux session exists (which it always does mid-session) and sits in an
early-alphabetical test binary, so a plain `cargo test` / `just check` stops
there. The **`just verify`** recipe runs the trustworthy gate —
`GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast` (the suite is
socket-isolated, so the opt-out is safe) plus lint, deny, and audit — and
the dogfood config sets `[supervisor].test_command = "just verify"` so the
supervisor uses it automatically.

## Per-commit verification

The supervisor verifies each agent's commit **as it lands**, not in batches.
When an agent publishes `agent.artifact { status: "committed" }`, the
supervisor runs that branch's five-gate sweep promptly — independent of
whether other agents have finished. Waiting for a slower agent so several
commits can be verified "together" delays feedback to the agent that already
committed and serialises work that is meant to run in parallel; at five-to-
eight agents that idles the whole session until the slowest one finishes.

To make the trigger an explicit event rather than something the supervisor
has to notice during a pane sweep, the broker posts a `supervisor.verify-now`
message naming the committing branch to the supervisor inbox on every
committed artifact. This is on by default; set
`[supervisor] verify_on_commit_nudge = false` to suppress the nudge and rely
on the supervisor's sweep cadence instead.

Per-branch verifications may run concurrently — each gate sweep runs against
its own branch in its own worktree, so verifying one agent never blocks
starting another's verification. The supervisor may bound how many sweeps run
at once at its discretion. The one legitimate reason to defer a verification
is a genuine dependency (one agent's work needs another's merge in place
first), which the supervisor states explicitly when it defers.

## Recovery from API stream timeouts

The supervisor's own model stream can time out mid-sweep — most often
while it is queueing several `agent.feedback` or `agent.verified`
publishes in one iteration. Left unhandled, the natural reflex is to
fall silent and wait for the next sweep tick, which silently drops the
pending feedback: the agent that was about to hear about a regression
keeps working unaware, and you get no signal anything went wrong.

The bundled supervisor skill teaches an explicit recovery discipline so
a transient timeout no longer loses work:

1. **Recognise the failure shape** — a mid-stream cutoff, or a
   transport / stream error in the CLI output. The phrasing is generic
   so it matches across CLI variants.
2. **Checkpoint before risky actions** — before any iteration with more
   than one intended downstream publish, the supervisor publishes a
   single `agent.status` "checkpoint" enumerating what it is about to
   do, giving the recovery path a re-entry point.
3. **Replay the missing publishes** — on recovery the supervisor
   re-reads its checkpoint and, for each intended target, polls
   `/messages/<target>` since the checkpoint timestamp to see which
   publishes landed, then re-publishes only the missing ones (the
   replay is idempotent, so duplicates are safe).
4. **Never assume a publish succeeded** — a returned `publish` HTTP call
   is not proof the record landed; the supervisor confirms by polling or
   re-publishes idempotently before moving on.

Each successful recovery emits a `recovery_cycles` `agent.learning`
record, so recurrent timeouts surface in the learnings output. See the
"Stream-timeout recovery" section of the bundled
`assets/agent-skills/supervisor.md` skill for the full doctrine, and
[Learnings Mode](learnings.md) for how the `recovery_cycles` records are
aggregated.

## Broker-side conflict detector

Starting with v0.5.0 the broker auto-detects three failure shapes between
parallel agents and emits `agent.feedback` (and, where configured,
`agent.question`) on the supervisor's behalf. All auto-emitted messages
begin with the `[conflict-detector]` token so the supervisor can distinguish
detector output from human-typed feedback. The three failure shapes are:

- **Forward conflict** — two agents publish overlapping `agent.intent`
  declarations.
- **In-flight conflict** — two agents' filesystem-watched
  `modified_files` sets overlap on the same file.
- **Ownership violation** — an agent's `modified_files` include a file
  inside another agent's active intent.

See [Conflict Detection](conflict-detection.md) for the algorithm,
configuration, and escalation behaviour.

## Learnings aggregator

When `[supervisor.learnings] enabled = true`, the supervisor session
records deterministic friction signals (sandbox warnings, approval
patterns, recurring errors) into a markdown file you can review after the
run. The supervisor also records *qualitative* learnings it reasons out over
the run — recurring failure shapes, doc gaps, ADR drift, scope mistakes, and
**tooling friction** with git-paw itself — publishing each through the bundled
`.git-paw/scripts/sweep.sh learn <category> <title> <body-json>` helper (never
a raw curl). It captures them opportunistically during each sweep and again in
a session-end synthesis pass. See [Learnings Mode](learnings.md) for the file
format, the category set, and how to opt in.

## When the user types in your pane

The supervisor pane is interactive — the user can type at any time while
the autonomous monitoring loop is running. The supervisor finishes the
current step (spec audit, test run), responds, then resumes the loop. User
input is a high-priority interrupt, not a replacement for the loop.

Each kind of user input maps to an existing mechanism — the supervisor does
not invent new channels:

1. **Status question** ("how's feat-auth going?", "anything blocked?") —
   answered conversationally in the pane using `sweep.sh status`,
   `sweep.sh inbox`, and `sweep.sh capture <pane>`. **Nothing is published
   to the broker** — this is a conversation with the user, not a
   session-wide event.
2. **Directive** ("ask feat-auth to use bcrypt", "tell feat-api to skip
   the migration") — published as `agent.feedback` to the named agent
   with the `[directive]` gate prefix, plus a conversational confirmation
   to the user.
3. **Judgment-call ask** ("should we merge feat-a before feat-b?") — the
   supervisor applies its normal escalation rules. If the user has already
   provided enough information to decide, the supervisor answers in the
   pane using its reasoning. `agent.question` only fires when the call is
   genuinely ambiguous beyond what the user just provided — typically
   when the user is asking *because they don't know either*.

The mechanisms (`curl /status`, `tmux capture-pane`, `agent.feedback`,
`tmux send-keys`, `agent.question`) are unchanged. The addition is *when
to use which* in response to user input.

## Routing through the supervisor

The supervisor is the single conversational surface: you work in its pane
and address individual agents *through* it, rather than tab-switching into
each agent's pane. Two commands you type in the supervisor pane drive this.

### `/agents` — the live inventory

Type `/agents` and the supervisor responds with the current inventory: one
row per agent plus the supervisor's own row, each carrying `branch_id`,
`status`, `last_seen`, `cli`, the detected `mode`, and `pane_index`:

```
branch_id   status    last_seen  cli     mode          pane
feat-auth   working   3s         claude  accept-edits  2
feat-api    blocked   90s        claude  unknown       1
supervisor  working   1s         claude  —             0
```

The inventory is composed from broker `GET /status` (`branch_id`, `status`,
`last_seen`, `cli`) joined with the live `pane_current_path` mapping
(`pane_index`). Pane indices are **path-resolved**, never assumed from
ordering — a mid-session `git paw add`/`remove` that renumbers the grid is
reflected correctly after the next sweep. The `mode` column is best-effort
(`accept-edits`, `interactive`, or `unknown`); `unknown` is treated as the
safe case by `/tell`.

The supervisor refreshes the inventory at its sweep cadence and caches it in
memory. `/agents` and `/tell` reuse the cached snapshot while it is younger
than `[supervisor.tell] inventory_max_age_seconds` (default 60), re-polling
the broker only when it goes stale. The cache is not persisted — a restarted
supervisor rebuilds it on the first command.

### `/tell <agent> <prompt>` — route a prompt

Type `/tell feat-auth rebase onto main` and the supervisor:

1. **Validates** `feat-auth` against the inventory (the slug `feat-auth` or
   slash form `feat/auth` both work). An unknown target is **not** delivered —
   the supervisor replies with the candidate list of available agents, e.g.
   `unknown target` `feat/ghost`​`; available agents: feat-api, feat-auth`.
2. **Chooses a delivery mode** (see below) and delivers the prompt.
3. **Acknowledges** in the pane and, when learnings mode is on, records the
   routing decision (see [Learnings](./learnings.md)).

`/tell` routes exactly the agent you name and exactly the prompt you type —
it never decides the recipient for you and never pipes your prompt into
another CLI to generate content. There is no bulk broadcast form in this
release; smart, supervisor-decided routing is a future candidate.

### Delivery modes

`[supervisor.tell] mode` selects the default delivery channel:

- **`feedback`** (the default) — the prompt is published as an
  `agent.feedback` broker message and the agent consumes it on its next inbox
  poll. Safe for mixed-mode sessions: the prompt is queued and recorded, never
  raced into a running prompt.
- **`send-keys`** — the prompt is injected directly into the target's pane
  with `tmux send-keys`. Faster, but only safe when the target is in
  accept-edits mode. When you configure `send-keys` but the target's detected
  mode is `interactive` or `unknown`, `/tell` **falls back to `feedback`** and
  prints a note so you can investigate the mode-detection drift. Because of
  this fallback, `send-keys` is best reserved for sessions where your agents
  run in accept-edits mode.

Every `/tell` is auditable: when `[supervisor] learnings = true`, each
invocation appends a line to a `### Supervisor routing` section of
`.git-paw/session-learnings.md` with the timestamp, target, delivery mode, and
the prompt (truncated past 200 characters). With learnings disabled, nothing
is written.

### Proactive routing (opt-in, confirmed)

When a sweep finds an agent blocked on a question you have *already answered*
earlier in the supervisor pane, the supervisor may **offer** to forward that
context — it posts a yes/no question in its own pane and waits. It never routes
on its own: no proactive `/tell` fires without your explicit `y`. Reply `n` (or
anything else) and the offer is dropped. This keeps the pattern from feeling
surveillance-y while it earns trust through dogfooding.

## opsx role gating

The bundled skills teach that `/opsx:verify` and `/opsx:archive` are
supervisor-only, but a coding agent that ignores the prose can still archive a
change — the skill is a convention, not a guard. When the session's spec engine
is OpenSpec, git-paw closes that gap with a **post-commit role-gating guard**.

The guard watches the broker's `agent.artifact { status: "committed" }` events.
When a commit looks like an archive operation **and** the committing agent is
not the supervisor, it reacts per `[opsx] role_gating` (see the
[configuration reference](../configuration/README.md#opsx-role-gating)):

- **`warn`** (the v0.6.0 default) — publishes an `agent.feedback` to the
  offending agent and records an `agent.learning` with category
  `permission_pattern` so the violation surfaces in learnings.
- **`block`** — the warn behaviour, plus an `agent.feedback` to the supervisor
  requesting a revert. The supervisor performs the revert through its
  merge-orchestration skill (confirming with the user first unless
  `[supervisor] auto_revert = true`); git-paw never runs `git revert` itself.
- **`off`** — the guard is disabled.

**Detection heuristic.** A commit is archive activity when **either** signal
fires:

1. its subject line matches the canonical archive shape
   `chore(specs): archive <name>; sync deltas to main specs`, or
2. its diff moves files into `openspec/changes/archive/<name>/` and/or adds a
   main spec under `openspec/specs/<capability>/spec.md`.

The two-signal design is deliberately conservative — it would rather trip on
the supervisor's own archive (which the attribution check then clears, since
`agent_id == "supervisor"`) than miss a coding-agent archive. An unresolvable
worktree is treated as a violation.

**Reading the warning.** The feedback text names the short SHA, the offending
agent, and the trigger so you can spot a false positive at a glance:

```
opsx-role-gating: detected archive activity on commit abc1234 by agent feat-x
  (not the supervisor).
  Reason: commit message matched the archive heuristic ("chore(specs): archive
  feat-x; sync deltas to main specs").
  /opsx:verify and /opsx:archive are supervisor-only ...
```

If a warning is a false positive, the named reason tells you which signal fired
so you can adjust (or flip `role_gating = "off"`). The guard is **inert under
the Spec Kit and Markdown engines** — those have no `/opsx:` commands or archive
paths, so the capability and its skill sections are scoped to OpenSpec only.

> **v0.6.0 behaviour change.** `role_gating` defaults to `warn`, so sessions
> where a coding agent archives a change now see guard feedback. Set
> `role_gating = "off"` for the prior (no-guard) behaviour.

## Merge orchestration

Once every spec'd agent has published `agent.verified` (or the user
explicitly asks for a merge), the supervisor runs the merge orchestration
loop. v0.5.0 removed the Rust auto-merge loop; merging is now the
supervisor's responsibility, performed with the existing shell + curl
tools.

**Trigger.** Either every spec'd agent has published `agent.verified`, or
the user has explicitly requested the merge.

**Merge order.** The supervisor reads the broker's message log
(`/messages/supervisor`) and builds a dependency graph from `agent.blocked`
events: each event from agent X with `payload.from = Y` is an edge "X
depends on Y". The supervisor then topologically sorts the graph: agents
with no incoming edges merge first; dependents follow.

**Per-branch merge.** For each branch in topological order, the supervisor
checks out `main` and runs:

```bash
git merge --ff-only feat/<branch>
```

Never a merge commit — fast-forward only. If `--ff-only` fails (the branch
diverges from `main`, or there is a conflict), the supervisor SKIPS that
branch and publishes `agent.feedback` to the owning agent asking them to
rebase or resolve. On a successful fast-forward, the supervisor runs the
configured `{{TEST_COMMAND}}`; if tests fail, the supervisor reverts the
merge with `git reset --hard <prev-HEAD>` and publishes `agent.feedback`
tagged `[regression]`.

**Cycle handling.** If the dependency graph has a cycle, the supervisor
does NOT merge any branch in the cycle. Instead, it publishes
`agent.question` to the human and waits for guidance before continuing.

**Final summary.** When the loop completes, the supervisor publishes a
final `agent.status` summarising which branches merged cleanly, which were
skipped (and why), and any regressions encountered.

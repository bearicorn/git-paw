## Context

`supervisor-pane-removal` deleted the pane-injection actuation (`hand_to_orchestrator`, the nudge) and
left the pure renderers (`Escalation::handoff_text`, `question_handoff_text`, `merge_handoff_text`).
Escalations are broker review items — tested, but pull-only. `broker-verdict-authority` means a
process at the repository root can publish verdicts through `sweep.sh` (which fetches the credential
via `git paw __authority`); `orchestrator-skill-export` puts the procedure at
`.git-paw/skills/orchestrator/SKILL.md` and the session context at `.git-paw/AGENTS.local.md`. In
v0.17.0 the drive loop reaches tmux through its `DriveDeps` ports, not `CommandRunner` (only the
branch refresh runs git through it), and it resolves per-CLI prompt profiles and settle delays through
`[clis.<name>]` but never approval flags: `resolve_approval_flags` is called only by the launch paths
(`commands/start.rs`, `add.rs`, `supervisor.rs`). After `terminal-backend-seam` the loop's terminal
I/O goes through the port, whose tmux adapter runs on `CommandRunner`; this change makes the loop the
first non-launch caller of `resolve_approval_flags`. `CommandRunner` itself
(`src/command_runner.rs:75-97`) offers only blocking, argv-only `run` / `run_inheriting_stdio` — no
cwd, stdin, env, process group or timeout — so the one-shot needs the seam extended (D12). After
`drive-loop-functional-core` the loop's decisions return effects, and its `decide_message` returns
questions and merge candidates as judgment calls; this change adds one effect variant rather than new
I/O inside a decision.

After v0.17.0 `classifier-retirement` no git-paw process answers a permission prompt. The drive loop
escalates every live prompt to the broker, the exit summary and the manual-decision log (the former
DS-4 of this change, shipped in v0.17.0), and `sweep.sh approve` runs only when a human asks. v0.17.0
`cli-setup-consent` lets the user grant git-paw's helpers in the CLI's own settings with
`git paw setup-cli claude --allowlist`: the read-only and publishing verbs are allowed,
`sweep.sh approve` is on `ask`, and the verdict verbs (`sweep.sh verified`, `sweep.sh feedback-gate`,
`sweep.sh conflict-ack`) are deliberately not granted.

## Goals / Non-Goals

**Goals:**
- Event-driven, one-shot, non-interactive hand-off of judgment calls (questions, merge decisions,
  non-converging branches) to the orchestrator CLI.
- Never block the sweep; bound time; serialize; deduplicate; log; report.
- Per-CLI knowledge in the existing `[clis.<name>]` table; Claude and Codex built in.
- Bind `[supervisor].approval` to these invocations.
- Agnostic, asset-authored framing.

**Non-Goals:**
- Handing permission prompts to the one-shot (D11).
- Manual-decision logging (v0.17.0 `classifier-retirement` already writes it for every escalated
  prompt; this change does not touch it).
- A periodic orchestration sweep (removed with the pane; every merge decision is already an event).
- A structured-output "oracle" contract (see D2).
- Orchestrator session continuity across one-shots (see Open Questions).
- Running the one-shot anywhere but the local repository root.

## Decisions

### D1 — Agentic one-shot, exit-status contract

The invocation is the configured orchestrator CLI in its non-interactive mode, at the repo root, given
the escalation context on stdin and pointed at `SKILL.md`. It acts as an interactive orchestrator
would — `sweep.sh capture`, `broker.sh` answers, `sweep.sh feedback-gate/verified/conflict-ack`, `git -C` merges,
MCP tools if attached — except that it never runs `sweep.sh approve` (D11), and publishes its outcome
to the broker. The drive loop reads nothing from its
stdout; it records exit status and duration. One skill therefore serves both the interactive and the
headless orchestrator.

### D2 — Rejected: structured-output oracle

An alternative is a pure judgment oracle: text in, a JSON decision out (`answer …`, `merge`,
`escalate to human`), actuated by the drive loop. It needs no permissions and keeps authority in the
loop, but it cannot do the *work* escalations require — running the five gates, auditing a spec,
merging — and it introduces a second actuation contract to keep in parity with the interactive skill.
The agentic one-shot handles every class with one contract.

### D3 — stdin, never argv; environment carries only the broker URL

Escalation text can be long and can quote a worker's prompt verbatim; argv leaks to `ps` and has length
limits. The one-shot needs no secret: verdicts go through `sweep.sh` → `git paw __authority`, which
reads the data-dir session file from the root. `GIT_PAW_BROKER_URL` is set as it is for every pane.

### D4 — Serialize to one in-flight invocation; FIFO; dedup defined per source

Concurrent one-shots at the repo root would race on merges and on the per-pane claim. One at a time is
the safe default and matches how a single interactive orchestrator behaves. Dedup does **not** reuse
`DedupWindow`: that window serves only `sweep_pane`'s permission-prompt and CLI-unavailable
escalations, neither of which reaches the queue. The handed-off kinds are already once-only at their
source — a question or merge candidate is a broker message `decide_message` sees exactly once (the
loop's log cursor advances past it), and correction-exhausted fires once per correction episode
(`CorrectionState::take_exhaustion`, reset only when the branch reaches a completed verdict). What can
still repeat is the worker re-publishing the same thing (the same question, another `committed`
artifact) while the first one-shot is pending, so the queue keys each entry as
`(kind, agent_id, discriminator)` — the question text for a question, the artifact status for a merge
decision, nothing for correction-exhausted — and drops an escalation whose key is already queued or in
flight. Once that invocation finishes, a later message with the same key is a new event and is queued.
Wind-down stops dequeuing but lets the in-flight invocation finish (a half-done merge is worse than a
late exit).

### D5 — Timeout default 1800 s, kill the process group

A merge-decision one-shot runs the consumer's full gates; 30 minutes is a generous default that still
prevents a hung CLI from pinning the loop forever. Killing the *group* (the CLI spawns children) avoids
orphans. Configurable via `[supervisor.escalation] timeout_seconds`; `0` is rejected.

### D6 — Outcomes on the existing wire

`agent.status` from `supervisor` with `detail {escalation_key, exit_code|"timeout", duration_seconds}`
— the optional `phase`/`detail` fields already exist and the taxonomy is forward-compatible. No new
`BrokerMessage` variant, so no enum ripple. The dashboard's status-activity rendering surfaces it; the
exit summary counts it.

### D7 — `[supervisor].approval` is the invocation's permission level

In a non-interactive turn nobody answers the one-shot's own permission prompts, so what it can do is
exactly what its approval level and the CLI's own allow rules permit. git-paw seeds no grants; the
user's `git paw setup-cli claude --allowlist` (v0.17.0) is the consented way to allow git-paw's
helpers. With those grants, `auto` (the inherited default) yields a one-shot that can read state and
answer a question through `broker.sh`, but it cannot publish a verdict or merge: `sweep.sh verified`,
`sweep.sh feedback-gate` and `sweep.sh conflict-ack` are deliberately not granted, and `git` merges are the consumer's own
rules. A merge-decision hand-off therefore needs `approval = "full-auto"` (recommended inside the
v0.15 sandbox) or allow rules the user writes. The docs state the trade plainly: `full-auto` here is
an unattended agent with root-level write access for up to `timeout_seconds`, bounded, serialized and
logged — the same power the pane had, without its interactive attack surface — and under `full-auto`
the `ask` rule that `setup-cli` puts on `sweep.sh approve` does not apply.

### D8 — Built-in rows: Claude `-p`, Codex `exec`

Both read the prompt from stdin in those modes; both are verified against upstream at implementation
time (the same clause the approval-flag table carries). Everything else is an override
(`[clis.<name>].headless_args`) or unresolvable → broker-only with a launch-time notice, never a silent
degradation.

### D9 — Framing prose is an asset

`core-lang-agnostic` requires agent-facing prose in bundled assets. `assets/escalation-framing.md`
holds the sentences around the rendered body; the audit covers every rendering.

### D10 — One new effect, never on the prompt decision or the heads-up

`decide_message`'s question and merge-decision judgment calls (after their `NoteJudgmentCall`), and
`decide_nudge`'s correction-exhausted branch (after its broker-escalation effect), return
`InvokeOrchestrator { escalation }`, only when a headless invocation is resolvable and enabled. The
shell owns the queue; the decisions stay pure. `decide_prompt` never returns it: a live permission
prompt's effects stay `RecordManualDecision` → `PublishEscalation` → `RecordLearning`, as v0.17.0 and
`drive-loop-functional-core` define them. The early correction heads-up (`correction-slow`) never
returns it either: it says "no action is required yet", so spending a fresh orchestrator context on it
buys nothing; it stays a broker record, as today.

### D11 — Permission prompts never reach the one-shot

A permission prompt needs a human decision, and the one-shot has no human to relay it to. Handing it
over would leave two outcomes, both wrong: the one-shot answers it on its own judgment (what
`classifier-retirement` removed), or it can do nothing useful and spends a context. So the prompt
stays on the broker, in the exit summary and in the manual-decision log, where the interactive
orchestrator or the person finds it, and the framing asset tells every one-shot never to run
`sweep.sh approve` or answer a permission prompt. Under `auto` / `manual` with `setup-cli`'s grants,
`sweep.sh approve` is on `ask` and is denied in a non-interactive turn; under `full-auto` the framing
is the only guard (Risks).

### D12 — Extend the `CommandRunner` seam with a non-blocking spawn

The one-shot needs what `run` / `run_inheriting_stdio` cannot express: a working directory, bytes on
stdin, environment additions (`GIT_PAW_BROKER_URL`), its own process group, stdout/stderr appended to a
log file, and a timeout — and it must not block. The seam gains one method taking a spawn spec with
those fields and returning a child handle the shell polls (running / exited with code / timed out) and
can kill as a group. `RealCommandRunner` builds it on `std::process::Command` (`current_dir`, piped
stdin written then closed, `envs`, `process_group(0)` on Unix); the group kill and the timeout need no
new dependency (the approved set has no `libc` / `nix`). The recording fake stores the full spec, so
tests assert argv, cwd, stdin and env without a real process. Existing `run` / `run_inheriting_stdio`
callers are untouched. *Alternative rejected:* `std::process::Command` directly in `escalation.rs` —
it would bypass the seam `code-standards` decided on and make the argv/stdin contract untestable
without spawning.

### D13 — The framing overrides the renderers' "escalate to the human"

The retained renderers were written for an orchestrator pane with a human beside it: each ends
"escalate to the human only if … genuinely undecidable". A non-interactive one-shot has no human to
escalate to. The framing asset therefore states what escalating means for it — publish why on the
broker (so the review item carries the reason) and exit, leaving the decision for the human — and
says so explicitly over the rendered body rather than editing the renderers (they still render the
broker-facing text). For a merge decision the framing carries the unattended merge rule shared with
the exported skill (`orchestrator-skill-export`, PP-25): verify, then merge fast-forward only once every
gate is verified; escalate only on a merge conflict, an exhausted correction cycle or a regression;
never stop to "ask the human before merging".

## Risks / Trade-offs

- [A `full-auto` one-shot runs `sweep.sh approve` anyway] → permission prompts never enter its
  context, its framing forbids it, the per-pane claim and the option-1 guard still apply, and the
  outcome is logged and published. The posture chapter states that `full-auto` bypasses the `ask`
  rule.
- [Under `auto`, a merge-decision one-shot cannot publish a verdict] → documented with D7; the
  broker review item stays for the human, exactly the fallback behaviour.
- [Cost: a fresh context per escalation] → dedup + serialization cap it; merge decisions are the only
  frequent class and each is real work.
- [The CLI's non-interactive mode changes upstream] → override template; launch-time notice names the
  form used.
- [Two orchestrators (interactive + one-shot) act at once] → only the interactive one approves, and
  only when the human says so; merges are guarded by the skill's `--ff-only` + verify doctrine;
  document that the operator should disable escalation (`enabled = false`) when actively driving by
  hand.
- [Log growth] → one file per session under `.git-paw/logs/<session>/`, already gitignored and purged.

## Migration Plan

1. `EscalationConfig` + `headless_args` + resolvers + config tests.
2. `CommandRunner` non-blocking spawn (D12); then `escalation.rs`: context renderer (asset framing),
   resolution, queue, spawn through the extended seam with stdin/redirect/process-group, timeout.
3. Drive-loop wiring: broker record → enqueue (never for a permission prompt) → outcome publish →
   summary; fake-runner unit tests.
4. Launch-time notice in `cmd_supervisor`; `--dry-run` reports the headless form.
5. E2E with a stub CLI script (reads stdin, publishes via `sweep.sh`, exits) in
   `tests/unattended_drive_e2e.rs`.
6. Docs + config reference + README; audit coverage for the asset.

Rollback: `[supervisor.escalation] enabled = false` restores broker-only without a code change.

## Open Questions

- Continuity: should one-shots `--resume` a persistent orchestrator session (via
  `worker-session-id-capture`'s `resume_args`) so the orchestrator remembers earlier decisions in the
  wave? Recommendation: dogfood stateless first; add an opt-in `[supervisor.escalation] resume = true`
  later if context loss shows up as bad merge sequencing.

## Why

With the supervisor pane gone, the drive loop's judgment calls — an ambiguous `agent.question`, the
merge decision a `committed`/`done` artifact opens, a branch that will not converge — are broker
review items waiting for a person or for the operator's interactive session. That is the tested prior
behavior, but it makes unattended autonomy depend on someone pulling the inbox. The roadmap keeps
autonomy self-contained: when the loop hits a call it cannot make, it **invokes the orchestrator CLI
headlessly, one-shot**, with the orchestrator skill and the escalation context. git-paw already knows
how to launch CLIs; a one-shot cannot self-stall the way the pane did, and it spends a fresh context
only when there is something to decide.

Permission prompts are not judgment calls the one-shot can take. Since v0.17.0
`classifier-retirement` no git-paw process answers a permission prompt: the drive loop escalates each
one to the broker, the exit summary and the manual-decision log, and only a human (or an orchestrator
the human tells to) runs `sweep.sh approve`. A non-interactive one-shot has no human to relay to, so
permission prompts are never handed to it and stay where v0.17.0 puts them.

Archive position 20, the last change of the release (after the skill export the one-shot loads).

## What Changes

- **Headless hand-off.** For each judgment call it cannot make — a question, a merge decision or a
  correction-exhausted branch — and when a headless invocation is resolvable, the drive loop invokes
  the orchestrator CLI once, non-interactively, at the repository root, with the escalation context on
  **stdin** (never argv), `GIT_PAW_BROKER_URL` in the environment, and approval flags from
  `[supervisor].approval` (falling back to `agent_approval`). The broker review item is still in place
  first (the worker's own message for a question or merge decision, a loop record otherwise). The loop never blocks the wave on the invocation. In the functional core this is one more
  effect (`InvokeOrchestrator`), returned by `drive-loop-functional-core`'s `decide_message` for its
  question and merge-decision judgment calls and by `decide_nudge`'s correction-exhausted branch. The
  early correction heads-up is **not** handed off: its own text says "no action is required yet"
  (`drive.rs:722-727`); it stays a broker record only.
- **Permission prompts are excluded.** A live permission prompt is never handed to an invocation; it
  stays on the broker, in the exit summary and in the manual-decision log. The framing tells every
  one-shot never to run `sweep.sh approve` or answer a permission prompt.
- **Resolution.** Orchestrator CLI = `[supervisor].cli` → `default_cli`. Headless arguments =
  `[clis.<name>].headless_args` → built-in table (`claude` → `-p`; `codex` → `exec`; verified upstream at
  implementation) → none. With no CLI, no headless form, or `[supervisor.escalation] enabled = false`,
  the loop keeps the broker-only path and prints one launch-time notice.
- **Bounded, serialized, deduplicated.** One invocation in flight per session (FIFO);
  `[supervisor.escalation] timeout_seconds` (default 1800) kills the process group; output appended to
  `.git-paw/logs/<session>/orchestrator-escalations.log`. Dedup is defined for these sources, not
  borrowed from `DedupWindow` (which serves only `sweep_pane`'s prompt and CLI-unavailable
  escalations): questions and merge candidates are consumed once by the loop's message cursor;
  correction-exhausted fires once per correction episode through `CorrectionState`; and the queue keys
  each entry by kind + agent + (question text | artifact status), dropping an escalation whose key is
  already queued or in flight (a worker re-publishing the same question or `committed` artifact).
- **Process-execution seam extended.** `CommandRunner` (`src/command_runner.rs:75-97`) offers only
  blocking, argv-only `run` / `run_inheriting_stdio`. This change adds a non-blocking spawn taking the
  working directory, stdin bytes, environment additions, a new process group, output redirection and a
  timeout, returning a handle the shell polls and can kill as a group; the recording fake records the
  whole spawn spec so tests assert argv, cwd, stdin and env.
- **Observable.** Each completed or timed-out invocation is published as an `agent.status` from
  `supervisor` (`detail`: escalation key, exit code or `timeout`, duration); the exit summary reports
  invocation counts.
- **Agnostic context.** The stdin document is assembled from the retained pure renderers
  (`Escalation::handoff_text`, `question_handoff_text`, `merge_handoff_text`) and framing prose in a
  bundled asset (`assets/escalation-framing.md`) covered by the no-leak audit; it points the one-shot at
  `.git-paw/skills/orchestrator/SKILL.md` and `.git-paw/AGENTS.local.md` and tells it to act only through
  the bundled helpers or MCP tools, to publish its outcome, and never to answer a permission prompt.
  The framing **overrides** the renderers' "escalate to the human" wording, which a one-shot cannot
  follow: for it, escalating means publishing why on the broker and leaving the review item for the
  human. A merge-decision one-shot follows the same unattended merge rule as the exported skill
  (`orchestrator-skill-export`, PP-25): merge fast-forward only once every gate is verified, and
  escalate only on a merge conflict, an exhausted correction cycle or a regression — never "ask the
  human before merging".
- **`[supervisor].approval` gets its binding** — the approval level of these invocations. The docs
  state what each level can do in a non-interactive turn: the `git paw setup-cli` helper grants cover
  the read-only and publishing helpers but deliberately not the verdict verbs (`sweep.sh verified`,
  `sweep.sh feedback-gate`, `sweep.sh conflict-ack`), so under `auto` a merge-decision one-shot cannot
  publish a verdict.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `supervisor-autonomous-orchestrator`: the fallback becomes the always-recorded broker path with a
  headless hand-off when resolvable (never for a permission prompt or the early correction heads-up); invocation contract,
  resolution, bounds, outcome recording and context content added.
- `supervisor-config`: `approval` is bound to headless invocations; new `[supervisor.escalation]`.
- `cli-resolution`: per-CLI `headless_args` with built-in Claude and Codex rows.
- `supervisor-unattended-operation`: the exit summary reports invocation outcomes.
- `core-lang-agnostic`: the escalation-framing asset is covered by the no-leak audit.

## Impact

- **Predecessors (implementation order):** archive position 20 (last), after
  `orchestrator-read-surface` (3), `supervisor-pane-removal` (6, which modified *No-orchestrator
  fallback preserves prior behavior* and *Supervisor-specific approval level resolution* first; this
  change modifies both again), `terminal-backend-seam` (12), `drive-loop-functional-core` (14: the
  hand-off is one more `DriveEffect` on `decide_message`'s judgment calls and `decide_nudge`'s
  correction-exhausted branch; `decide_prompt` never returns it), `broker-verdict-authority` (16, the
  one-shot publishes verdicts through the credential-carrying root helpers),
  `orchestrator-skill-export` (18, `SKILL.md` and the unattended merge rule) and
  `mcp-spec-authoring-tools` (19).
  v0.17: `classifier-retirement` (no component answers a permission prompt; the drive loop already
  writes the manual-decision log, which was this change's DS-4), `cli-setup-consent` (the
  `git paw setup-cli` helper grants the docs describe for `[supervisor].approval`),
  `worker-session-id-capture` (archived; `resume_args` noted in Open Questions),
  `cli-unavailable-detection-gaps` (unrelated requirement in the same capability).
- **Code:** `src/supervisor/decide.rs` (`InvokeOrchestrator` effect on `decide_message`'s question and
  merge-decision judgment calls and on `decide_nudge`'s correction-exhausted branch; none on the prompt
  decision or the early heads-up), `src/command_runner.rs` (non-blocking spawn with cwd, stdin, env,
  process group, redirection and timeout; `RealCommandRunner` + the recording fake), new
  `src/supervisor/escalation.rs` (context assembly, resolution, queue, spawn through the extended
  `CommandRunner`, timeout), `src/supervisor/drive.rs` shell (apply the effect; outcome publish; summary),
  `src/config/supervisor.rs` (`EscalationConfig`), `src/config/cli.rs` (`headless_args`),
  `assets/escalation-framing.md`, launch-time notice in `cmd_supervisor`.
- **Config:** optional `[supervisor.escalation]`, `[clis.<name>].headless_args`; `git paw init` commented
  examples.
- **Tests:** decision tests (the hand-off effect on each handed-off kind; no hand-off for a
  permission prompt or the early heads-up), `CommandRunner` spawn tests (real adapter: cwd, stdin,
  env, group kill on timeout), fake-runner tests (argv, stdin, serialization, dedup, timeout,
  fallback),
  `tests/unattended_drive_e2e.rs` with a stub CLI, config round-trips, `lang_agnostic_skill_audit.rs`
  for the framing asset.
- **Docs:** `user-guide/supervisor.md` (headless hand-off; permission prompts stay with the human;
  `[supervisor].approval` and what each level can do in a non-interactive turn; when to set
  `enabled = false`), configuration reference, `supported-clis.md` (headless forms), README bullet,
  `security-posture.md`.
- **Enum-variant ripple:** none — outcomes use the existing `agent.status` `detail` field.
- **Security / safety:** the one-shot runs at the repository root with the orchestrator's approval
  level — the blast radius the pane had, now bounded by a timeout, serialized, logged, and with no
  interactive channel. It is never given a permission prompt and its framing forbids
  `sweep.sh approve`; under `full-auto` the CLI's `ask` rule on `sweep.sh approve` does not apply, so
  that framing is the only guard there, and the posture chapter says so. Context on stdin (nothing on
  argv / `ps`); no secret passed — verdicts go through `sweep.sh` → `git paw __authority` at the root.
  The posture chapter states that `full-auto` here means an unattended agent with root-level write
  access for up to `timeout_seconds`.
- **Dependencies:** none.

## Rebase note (2026-10-20): classifier retirement

v0.17.0 `classifier-retirement` absorbed `drive-loop-manual-decision-log` (this change's DS-4): the drive loop records every escalated permission prompt in `.git-paw/sessions/<session>.manual-approvals.jsonl`, deduplicated, honouring `manual_approvals_log = false`. The MODIFIED *Manual-decision log file* delta was dropped (it would have reverted v0.17.0's text), with its tasks and the `RecordManualDecision` effect, which `drive-loop-functional-core` now carries over unchanged. There are no `danger` / `unknown` classes any more; every live permission prompt is escalated, and none is handed to the headless one-shot (design D11). The D7 reference to the common dev-command allowlist seeding is replaced by the CLI's own allow rules and `git paw setup-cli`.

## Rebase note (2026-10-25): audit against released v0.17.0

- Design call 5: the hand-off attaches to `drive-loop-functional-core`'s new `decide_message`
  (questions, merge decisions) and to `decide_nudge`'s correction-exhausted branch — never to
  `decide_prompt`. The early heads-up is no longer a handed-off kind (proposal, design, specs, tasks):
  its text says "no action is required yet"; it stays a broker record.
- `CommandRunner` has only blocking, argv-only `run` / `run_inheriting_stdio`
  (`src/command_runner.rs:75-97`); an explicit task and design decision (D12) extend the seam with a
  non-blocking spawn taking cwd, stdin, env, process group and timeout.
- Dedup is now defined for message-sourced escalations (cursor consumption; `CorrectionState` once-only
  exhaustion; a queued-or-in-flight key) instead of claiming `DedupWindow` reuse — that window serves
  only `sweep_pane`.
- The framing asset overrides the retained renderers' "escalate to the human" prose (D13), and the
  merge-decision one-shot follows the PP-25 unattended merge rule shared with
  `orchestrator-skill-export`.
- Context corrected: in v0.17.0 the drive loop neither uses `CommandRunner` for tmux nor calls
  `resolve_approval_flags` (only the launch paths do); its terminal I/O moves behind the
  `terminal-backend-seam` port, whose tmux adapter runs on `CommandRunner`.
- Ordering prose updated to archive position 20.
- The verdict verbs are listed as `sweep.sh verified`, `feedback-gate` and `conflict-ack` (proposal,
  design): `broker-verdict-authority` makes all three credential-carrying root verbs, and
  `conflict-ack` publishes `agent.feedback` from `supervisor` (`assets/scripts/sweep.sh:826`); `setup-cli`
  grants none of them (`SWEEP_NEVER_GRANTED`, `src/cli_setup/facts.rs:151-157`).

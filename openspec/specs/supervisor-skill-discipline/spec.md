# supervisor-skill-discipline Specification

## Purpose
Encodes the operational disciplines the bundled supervisor skill teaches the supervisor agent: drive pane work through `sweep.sh` (never inline loops), never send-keys to its own pane, use `git -C` for cross-worktree git, nudge on commit cadence and rely on agents standing by post-commit, and create isolated verification worktrees under a repo-local gitignored scratch dir checked out at the re-resolved branch tip. It also encodes stream-timeout recovery (recognizing the error shape including a coding agent's `stuck-stream-timeout`, taking a pre-action `checkpoint` `agent.status`, replaying only the missing downstream publishes idempotently, emitting a `recovery_cycles` learning, and treating repeated re-verify cycles as normal progress rather than a stall), per-event (concurrent, non-deferred) verification of each agent's commit as its `committed` event arrives (with an optional `supervisor.verify-now` broker nudge), and the full-suite / no-fail-fast testing-gate discipline — backed by git-paw's guard-neutralised `just verify` recipe wired through `{{TEST_COMMAND}}` — so an early-aborted run is never mistaken for a PASS. It also provides supervisor-pane directives for coordinating agents: `/agents` returns the current agent inventory (each row carrying branch_id, status, last_seen, cli, best-effort mode, and path-resolved pane_index), composed from broker `/status` and `tmux list-panes`, cached in memory with a refresh cadence and exposed as a reusable `coordination::inventory` library helper with target validation; and `/tell <agent_id> <prompt>` validates the target against that inventory and routes a prompt to the agent via a selectable delivery mode (`agent.feedback` or `tmux send-keys`), recording each route in learnings, requiring user confirmation for proactive routes, and never invoking another CLI to generate the prompt.
## Requirements
### Requirement: Mandate sweep.sh; forbid inline pane loops

The bundled supervisor skill SHALL include a section directing
the supervisor to use the bundled `.git-paw/scripts/sweep.sh`
helper for all pane capture, prompt approval, and send-keys.
The section SHALL explicitly forbid ad-hoc inline loops of the
form `for p in ...; do tmux ...; done`, stating that the
variable expansion trips the `simple_expansion` permission gate
and forces a human approval per iteration.

#### Scenario: Skill mandates sweep.sh for pane work

- **WHEN** the bundled supervisor.md is inspected
- **THEN** it SHALL contain a section directing all pane
  capture/approve/send-keys through `.git-paw/scripts/sweep.sh`

#### Scenario: Skill forbids inline pane loops

- **WHEN** the same section is read
- **THEN** it SHALL explicitly forbid `for p in ...; do tmux
  ...; done`-style inline loops, with the simple_expansion
  rationale

### Requirement: Never send-keys to the supervisor's own pane

The supervisor skill SHALL state that the supervisor sends
keystrokes only to agent panes and SHALL NEVER send-keys to
its own pane (pane 0), because doing so interrupts its own
in-flight command.

#### Scenario: Skill states the never-own-pane rule

- **WHEN** the supervisor.md pane-driving section is read
- **THEN** it SHALL state that the supervisor must not
  send-keys to its own pane, with the self-interrupt rationale

### Requirement: Cross-worktree git uses git -C, never cd

The supervisor skill SHALL include a rule that all git commands
against an agent worktree use `git -C <path> ...` and SHALL
forbid `cd <path> && git ...`. The rule SHALL state both
reasons: cd-before-git trips the untrusted-hooks warning, and
it leaks the working directory so a subsequent mutating git
command can land on the wrong branch.

#### Scenario: Skill mandates git -C

- **WHEN** the "Cross-worktree git" rule is read
- **THEN** it SHALL mandate `git -C <path>` for cross-worktree
  git and forbid `cd <path> && git`

#### Scenario: Rule states the cwd-leak rationale

- **WHEN** the rule is read
- **THEN** it SHALL cite both the untrusted-hooks warning and
  the wrong-branch (cwd-leak) risk as rationale

### Requirement: Reliable commit-cadence nudge

The supervisor skill SHALL state that when a sweep observes an
agent with more than a soft threshold (~10) of uncommitted
files, the supervisor publishes an `agent.feedback` nudging
the agent to commit its completed section. The threshold and a
sample nudge message SHALL be stated explicitly.

The supervisor skill SHALL ALSO state that the supervisor's verify-then-archive
workflow depends on coding agents **standing by** after their final commit: once an
agent has committed and published `agent.artifact { status: "committed" }` (or a manual
`status: "done"`), the supervisor — not the agent — runs `/opsx:verify` and
`/opsx:archive`. The skill SHALL cross-reference the agent-side stand-by-after-commit
protocol in `coordination.md` so the supervisor understands the post-commit signal is
its cue to begin verification, and that an agent should not be expected (or instructed)
to self-verify or self-archive.

#### Scenario: Skill states the nudge threshold and cue

- **WHEN** the coordination section is read
- **THEN** it SHALL state the ~10-uncommitted-file threshold
  and include a sample `agent.feedback` nudge message

#### Scenario: Skill states the supervisor relies on agents standing by post-commit

- **WHEN** the supervisor skill's commit-cadence / verification guidance is read
- **THEN** it SHALL state that the supervisor runs `/opsx:verify` and `/opsx:archive` after an agent's final commit, not the agent
- **AND** it SHALL cross-reference the agent-side stand-by-after-commit protocol in `coordination.md`

### Requirement: Stack-agnostic phrasing (skill-discipline)

The new/edited sections SHALL pass the no-language-leak audit
from [[lang-agnostic-assets]].

#### Scenario: No-leak audit passes

- **WHEN** the audit runs against the updated supervisor.md
- **THEN** it SHALL pass across all supported spec backends

### Requirement: Isolated verification worktrees use a repo-local gitignored scratch dir

The bundled supervisor skill SHALL instruct the supervisor to create
any isolated verification worktree under a repo-local, gitignored
scratch directory — `.git-paw/tmp/verify-<branch>/` — and SHALL NOT
direct it to `/tmp` or any path outside the repository. The skill
SHALL teach the cleanup step (`git worktree remove` / `git worktree
prune`) so scratch worktrees do not accumulate.

The recipe SHALL check out the agent branch's **re-resolved tip**, not a
pinned commit SHA captured from a `committed` event. The skill SHALL
instruct the supervisor to resolve `TIP=$(git rev-parse <branch>)`
immediately before `git worktree add --detach`, and to pass that
re-resolved tip (not a previously captured `$SHA`) as the checkout
target. The recipe SHALL re-resolve the tip and re-create the worktree
each time the gates are (re-)run for the branch, so the worktree never
holds a snapshot older than the branch's current tip. The detach mode
SHALL be preserved so the agent's own worktree remains the authoritative
holder of the branch ref.

The repository `.gitignore` SHALL ignore `.git-paw/tmp/` so the nested
verification worktree never appears in the parent worktree's status.

#### Scenario: Supervisor skill names the repo-local scratch path

- **WHEN** the bundled `supervisor.md` is inspected
- **THEN** it SHALL instruct creating the isolated verify worktree
  under `.git-paw/tmp/` (repo-local, gitignored)
- **AND** it SHALL NOT instruct using `/tmp` for verification scratch

#### Scenario: Scratch directory is gitignored

- **GIVEN** the repository `.gitignore`
- **WHEN** it is inspected
- **THEN** it SHALL contain an entry ignoring `.git-paw/tmp/`

#### Scenario: Verify worktree checks out the re-resolved branch tip

- **WHEN** the bundled `supervisor.md` isolated-verify-worktree recipe is inspected
- **THEN** it SHALL resolve the branch tip with `git rev-parse <branch>` immediately before `git worktree add --detach`
- **AND** it SHALL pass that re-resolved tip as the checkout target, NOT a commit SHA captured from a `committed` event

#### Scenario: Recipe re-resolves the tip on re-run

- **WHEN** the recipe's re-run / re-verification guidance is read
- **THEN** it SHALL state that each (re-)run of the gates re-resolves the branch tip and re-creates the worktree, so the worktree never holds a snapshot older than the current tip

### Requirement: Escalation-first, no blanket-approve when a drive loop is running

When the supervisor's boot context indicates a drive loop is running (an unattended session), the supervisor SHALL, each supervision cycle:

1. **Drain the drive loop's escalations first** — read the loop's escalation/review items from its broker inbox, reason about each, and either targeted-approve the specific escalated pane or publish feedback. This precedes the rest of the sweep so agents blocked on a prompt the loop could not classify safe are unblocked fastest.
2. **Then perform its normal sweep** — verification, merge orchestration, conflict handling, detect-stuck, and status publishing — as it otherwise would.

While a drive loop is running, the supervisor SHALL NOT blanket-approve classifier-safe prompts by sweeping panes: the loop owns safe-prompt approval, and the supervisor acts only on prompts the loop escalated. This keeps the two approvers' actions disjoint (see `unattended-operation`) and removes the approval-dispatch race.

When no drive loop is running (an attended supervisor session), the supervisor performs the full sweep INCLUDING approving classifier-safe prompts, as its sole-approver role requires — this preserves existing attended behaviour.

#### Scenario: With a loop running, escalations are handled before the sweep

- **GIVEN** a supervisor whose boot context indicates a drive loop is running
- **WHEN** it runs a supervision cycle
- **THEN** it SHALL process the loop's escalations (targeted approve / feedback) before its verify/merge/status sweep
- **AND** SHALL NOT blanket-approve classifier-safe prompts by sweeping panes

#### Scenario: With no loop, the supervisor approves safe prompts itself

- **GIVEN** a supervisor whose boot context does NOT indicate a drive loop
- **WHEN** it sweeps the panes
- **THEN** it SHALL approve classifier-safe prompts itself as the sole approver

### Requirement: Stream-timeout recovery section in supervisor skill

The bundled supervisor skill SHALL include a "Stream-timeout
recovery" section teaching the supervisor LLM how to recover
from API stream timeouts mid-sweep. The section SHALL contain
four ordered pieces: error-shape recognition, pre-action
checkpoint, replay-missing-publishes recovery, and a
confirmation rule.

#### Scenario: Section exists with the four pieces in recovery order

- **WHEN** the bundled `supervisor.md` is inspected
- **THEN** the file SHALL contain a "Stream-timeout recovery"
  heading whose subsections cover error-shape recognition,
  pre-action checkpoint, replay-missing-publishes, and the
  confirmation rule, in that order

### Requirement: Error-shape recognition

The skill SHALL describe the visible symptoms of an API
stream timeout (mid-stream cutoff, transport error in the CLI
output, or equivalent) so the supervisor LLM names the failure
rather than continuing in silence. The phrasing SHALL be
generic enough to apply across CLI variants (claude,
claude-oss, future entries). The skill SHALL distinguish two
cases: the supervisor's OWN stream timeout (recovered via the
checkpoint/replay flow below) and a CODING AGENT's stream
timeout observed in that agent's pane, which the supervisor
detects via `.git-paw/scripts/sweep.sh detect-stuck` and which
surfaces as a synthetic `agent.status` with
`phase: "stuck-stream-timeout"`. A coding agent in
`stuck-stream-timeout` SHALL be flagged for recovery (nudge or
restart) rather than left stalled.

#### Scenario: Symptoms are named generically across CLIs

- **WHEN** the error-shape subsection is read
- **THEN** the prose SHALL describe at least two visible
  symptom patterns (e.g. "mid-stream cutoff" and "transport
  error / stream error in the CLI output") and SHALL NOT name
  a specific CLI's exact error string

#### Scenario: Coding-agent stream timeout is a detected, recoverable state

- **WHEN** the error-shape subsection is read
- **THEN** the prose SHALL state that a coding agent whose
  pane shows a stream-timeout / transport-error marker is
  detected by `sweep.sh detect-stuck` and surfaced as
  `phase: "stuck-stream-timeout"`, and that such an agent
  SHALL be flagged for recovery rather than treated as
  progressing

### Requirement: Pre-action checkpoint via agent.status

The skill SHALL teach the supervisor to publish a `phase: "checkpoint"`
`agent.status` record — through the bundled `sweep.sh status-publish` helper
(`--phase checkpoint --detail '{"intended_targets":[…]}'`), NOT a raw
`curl …/publish` — before any sweep iteration that will publish more than one
downstream record (e.g. multiple `agent.feedback` or `agent.verified`). The
checkpoint SHALL describe the intended sub-actions via `detail.intended_targets`
so the recovery path has a re-entry point.

Because the checkpoint now routes through the helper (which shapes the payload
as `status: "working"` with `phase: "checkpoint"`), the checkpoint is
identified by its `phase` value rather than a `status` label — consumers route
it by reading `phase`, consistent with the introspection phase taxonomy. This
supersedes the earlier requirement that the documented shape carry
`status: "checkpoint"`: routing every supervisor `agent.status` through the
bundled helper ([[broker-agent-helper]], [[broker-watcher-and-state]]) is the
governing constraint, and the helper does not emit a `checkpoint` status label.

#### Scenario: Checkpoint shape is documented

- **WHEN** the pre-action checkpoint subsection is read
- **THEN** the prose SHALL show a concrete checkpoint emitted via
  `sweep.sh status-publish --phase checkpoint` whose `--detail` object
  enumerates the intended targets (`intended_targets`)
- **AND** the checkpoint emission SHALL go through the bundled helper, NOT a
  raw `curl …/publish`

#### Scenario: Checkpoint required only for multi-publish iterations

- **WHEN** the checkpoint subsection is read
- **THEN** the prose SHALL state that the checkpoint applies
  to iterations with more than one intended downstream publish,
  not every sweep

### Requirement: Replay-missing-publishes recovery

The skill SHALL teach the supervisor, on recovery from a
stream timeout, to re-read its prior checkpoint, poll each
intended target's `/messages/<branch_id>` stream to identify
which publishes completed, and re-publish only the missing
ones. The replay SHALL be idempotent so duplicate publishes
remain safe.

#### Scenario: Per-target poll-then-replay pattern documented

- **WHEN** the replay subsection is read
- **THEN** the prose SHALL show the per-target loop: poll the
  target's message stream for the supervisor's prior publish
  since the checkpoint timestamp, and re-publish when the
  prior publish is absent

### Requirement: Confirmation rule

The skill SHALL state explicitly that the supervisor SHALL
NOT advance to the next sub-action just because a `publish`
HTTP call returned. The system SHALL require either polling
the target's message stream to confirm or re-publishing
idempotently. The rule SHALL be marked prominently (bold,
callout, or equivalent) so it is unmissable.

#### Scenario: Confirmation rule appears prominently

- **WHEN** the confirmation rule is rendered in the skill
- **THEN** the rule SHALL appear with prominent formatting
  (bold, callout block, or similar), and SHALL pair the rule
  with a one-sentence rationale referencing stream-timeout
  risk

### Requirement: Recovery learning record

On every recovery from a stream timeout, the supervisor SHALL
publish an `agent.learning` record with `category =
"recovery_cycles"`. The record's body SHALL identify the
checkpoint id, the intended targets, the replayed targets,
and any skipped targets so recurrent timeouts surface in
qualitative-learnings output.

#### Scenario: Skill prose names the recovery learning trigger

- **WHEN** the replay subsection or its adjacent prose is
  read
- **THEN** the prose SHALL state explicitly that each
  successful recovery emits a `recovery_cycles`
  `agent.learning` record with a structured body covering
  checkpoint id and target lists

### Requirement: Stack-agnostic phrasing (stream-timeout recovery)

The new section SHALL pass the no-language-leak audit from
[[lang-agnostic-assets]]. The section SHALL NOT use
Rust-specific or any other stack-specific language in its
prose or examples.

#### Scenario: No-leak audit passes against the new section

- **WHEN** the no-leak audit runs after the section lands
- **THEN** the audit SHALL pass on the rendered supervisor
  skill across all supported spec backends

### Requirement: N re-verify cycles is not a stall

The bundled supervisor skill SHALL state explicitly that
multiple feedback→fix→re-verify cycles per agent are normal
progress, not a stuck state. The skill SHALL teach the
supervisor that an agent which is "not yet verified after N
cycles" (observed examples: mcp-server took 7 cycles,
dev-allowlist took 6) SHALL NOT be flagged, nudged, or wound
down on the cycle count alone. The supervisor SHALL judge
stall by the detected stuck shapes (stuck-on-prompt,
stuck-stream-timeout, context-bloat, no-progress,
blocked-on-supervisor) — never by how many verify rounds an
agent has consumed.

#### Scenario: Skill prose states re-verify cycles are normal

- **WHEN** the supervisor skill is inspected
- **THEN** the prose SHALL state that multiple
  feedback→fix→re-verify cycles per agent are normal progress
  and SHALL cite that real agents have taken 6–7 cycles

#### Scenario: Cycle count alone SHALL NOT trigger a stall verdict

- **WHEN** the same prose is read
- **THEN** it SHALL state that "not yet verified after N
  cycles" SHALL NOT by itself cause the supervisor to flag,
  nudge, or wind down the agent, and that stall judgement uses
  the detected stuck shapes instead

### Requirement: Skill mandates per-event verification

The bundled supervisor skill SHALL include a "Verify on each
event, never batch" subsection stating in MUST/MUST-NOT terms
that the supervisor verifies each agent's commit as its
`committed` event arrives and SHALL NOT defer verification to
batch it with other agents' commits. The subsection SHALL
name the wave-1 batching failure mode by example.

#### Scenario: Skill contains the per-event rule

- **WHEN** the bundled `supervisor.md` is inspected
- **THEN** it SHALL contain a "Verify on each event"
  subsection with MUST/MUST-NOT language and a worked
  example of the batching anti-pattern

#### Scenario: Dependency-driven deferral remains permitted

- **WHEN** the subsection is read
- **THEN** it SHALL state that the only acceptable deferral
  reason is a genuine dependency (one agent's work requires
  another's merge first), which the supervisor SHALL state
  explicitly when deferring

### Requirement: Optional verify-now broker nudge

The broker SHALL, when
`[supervisor].verify_on_commit_nudge` is `true` (default),
publish a `supervisor.verify-now` message to the supervisor
inbox upon receiving `agent.artifact { status: "committed" }`.
The nudge SHALL carry the committing `branch_id`. When the
config field is `false`, no nudge SHALL be published.

#### Scenario: Nudge published on committed event

- **GIVEN** `verify_on_commit_nudge = true` (or unset)
- **WHEN** the broker receives an
  `agent.artifact { status: "committed" }` from `feat/foo`
- **THEN** the broker SHALL publish a `supervisor.verify-now`
  message carrying `branch_id: "feat/foo"` to the supervisor
  inbox

#### Scenario: Nudge suppressed when disabled

- **GIVEN** `[supervisor].verify_on_commit_nudge = false`
- **WHEN** the broker receives a committed artifact
- **THEN** no `supervisor.verify-now` message SHALL be
  published

#### Scenario: Default config enables the nudge

- **GIVEN** no `verify_on_commit_nudge` field in config
- **WHEN** a committed artifact arrives
- **THEN** the nudge SHALL be published (default true)

### Requirement: Skill permits concurrent verification

The supervisor skill SHALL state that verifying one agent's
commit does not block starting another agent's verification,
since gate sweeps run per-branch in isolated worktrees.

#### Scenario: Concurrency permission documented

- **WHEN** the "Verify on each event" subsection is read
- **THEN** it SHALL state that per-branch verifications may
  run concurrently and that verifying agent A does not block
  verifying agent B

### Requirement: Stack-agnostic phrasing (per-commit verification)

The new subsection SHALL pass the no-language-leak audit from
[[lang-agnostic-assets]].

#### Scenario: No-leak audit passes

- **WHEN** the no-leak audit runs against the updated
  supervisor.md
- **THEN** the audit SHALL pass across all supported spec
  backends

### Requirement: Testing gate states the full-suite discipline generically

The bundled supervisor skill's testing gate SHALL direct the supervisor to
run the configured test command (`{{TEST_COMMAND}}`) in a whole-suite /
no-fail-fast mode, and SHALL state that a run which aborted early is
incomplete — not a PASS. The wording SHALL remain stack-agnostic (no
runner- or repo-specific literals), so it passes the no-language-leak audit
across all supported spec backends.

#### Scenario: Skill mandates running the whole suite

- **WHEN** the supervisor.md testing-gate section is inspected
- **THEN** it SHALL direct the gate to run `{{TEST_COMMAND}}` without
  fail-fast (run every test group) and name the environment **guard test**
  as the failure that must not be allowed to truncate the run

#### Scenario: Early-aborted run is not a PASS

- **WHEN** the testing-gate section is read
- **THEN** it SHALL state that "the only failure is a known environment
  guard" is NOT a pass unless the full suite ran to completion

#### Scenario: Wording is stack-agnostic

- **WHEN** the no-language-leak audit runs against the updated supervisor.md
- **THEN** it SHALL pass across all supported spec backends (no
  runner/repo-specific tokens in the testing-gate prose)

### Requirement: git-paw provides a trustworthy verification recipe

The repository SHALL provide a `just verify` recipe that runs the WHOLE
test suite the correct way for git-paw — `cargo test --no-fail-fast` with
the no-tmux-server guard neutralised via `GIT_PAW_ALLOW_LIVE_SESSION=1`
(the suite is socket-isolated) — alongside the fmt, clippy, deny, and audit
gates, exiting non-zero on any real (non-guard) failure.

#### Scenario: just verify runs the full guard-neutralised suite

- **WHEN** `just verify` is invoked
- **THEN** it SHALL run `cargo test --no-fail-fast` with
  `GIT_PAW_ALLOW_LIVE_SESSION=1` plus fmt/clippy/deny/audit, so a single
  environmental guard failure can neither abort the run nor be mistaken for
  a code failure

### Requirement: git-paw routes verification through the recipe

git-paw's repo config SHALL set `[supervisor].test_command` to the
verification recipe (`just verify`) so the rendered supervisor skill's
`{{TEST_COMMAND}}` resolves to the trustworthy, no-fail-fast invocation
rather than a fail-fast-prone default.

#### Scenario: Configured test command is the verify recipe

- **GIVEN** git-paw's `.git-paw/config.toml`
- **WHEN** the supervisor skill is rendered
- **THEN** `{{TEST_COMMAND}}` SHALL resolve to `just verify` (the
  no-fail-fast, guard-neutralised recipe)


### Requirement: /agents inventory command in the supervisor pane

The supervisor SHALL recognise an `/agents` directive typed in its
own tmux pane and respond with the current agent inventory. The
inventory SHALL list every agent registered with the broker plus
the supervisor's own row, each carrying `branch_id`, `status`,
`last_seen`, `cli`, detected `mode`, and `pane_index`.

#### Scenario: User asks for the agent inventory

- **GIVEN** an active supervisor session with N agents
- **WHEN** the user types `/agents` in the supervisor pane
- **THEN** the supervisor SHALL respond with a structured listing
  of the N agents plus itself, each row containing
  `branch_id`, `status`, `last_seen`, `cli`, `mode`, and
  `pane_index`

#### Scenario: Inventory after a mid-session add/remove

- **GIVEN** a session whose agent set has changed via
  [[git-paw-add]]'s add/remove subcommands
- **WHEN** the user types `/agents` after the sweep that
  refreshes the inventory
- **THEN** the listing SHALL reflect the post-change agent set

### Requirement: Inventory sourcing

The inventory SHALL be composed from broker `/status` (for
`branch_id`, `status`, `last_seen`, `cli`) and `tmux list-panes`
with `pane_current_path` (for `pane_index`). The system SHALL NOT
assume tmux pane index ordering matches branch order; it SHALL
resolve via the path mapping.

#### Scenario: Inventory pane_index is path-resolved

- **GIVEN** a session whose branch→pane mapping is non-sequential
  (e.g. after a middle-grid `remove`)
- **WHEN** the inventory is built
- **THEN** each entry's `pane_index` SHALL be derived by matching
  the agent's worktree path against `pane_current_path`, not by
  alphabetical or registration order

### Requirement: Inventory cache and refresh cadence

The supervisor SHALL maintain an in-memory cache of the latest
inventory, refreshed by the existing supervisor sweep (~270s by
default) and on `/tell`/`/agents` invocations when the cache is
older than the configured `[supervisor.tell]
inventory_max_age_seconds` (default 60). The cache SHALL NOT be
persisted to disk; supervisor restarts SHALL produce a fresh
inventory.

#### Scenario: Fresh inventory reused on rapid /agents

- **GIVEN** a supervisor whose inventory was just refreshed
- **WHEN** the user types `/agents` again within the
  max-age threshold
- **THEN** the supervisor SHALL serve the cached inventory
  without re-polling broker

#### Scenario: Stale inventory triggers refresh

- **GIVEN** a supervisor whose inventory is older than the
  configured max-age
- **WHEN** the user types `/agents`
- **THEN** the supervisor SHALL re-poll broker `/status` and
  rebuild the inventory before responding

### Requirement: Mode detection is best-effort with safe fallback

Each inventory entry SHALL include a `mode` field with one of
`accept-edits`, `interactive`, or `unknown`. Detection SHALL use
the agent's tmux pane title and/or recent capture-pane content
heuristics. When the heuristic is inconclusive, the entry SHALL
report `unknown`.

#### Scenario: Mode reported when detectable

- **GIVEN** an agent whose pane clearly indicates accept-edits
  mode (e.g. via pane title or characteristic CLI banner)
- **WHEN** the inventory is built
- **THEN** the entry's `mode` SHALL be `accept-edits`

#### Scenario: Unknown mode when undetectable

- **GIVEN** an agent whose CLI doesn't expose a clear mode signal
- **WHEN** the inventory is built
- **THEN** the entry's `mode` SHALL be `unknown`, and consumers
  (e.g. `/tell`) SHALL treat `unknown` as requiring the safe
  `agent.feedback` delivery mode

### Requirement: Inventory and validation helper is reusable

The inventory query and target validation logic SHALL be
exposed as a reusable library function in
`coordination::inventory` (or equivalent module) rather than
inlined in any single consumer. The helper's API SHALL be
stable enough that future consumers (notably the v1.0.0 MCP
write tools' `publish_agent_feedback`) can adopt it without
re-implementing inventory + validation semantics.

#### Scenario: Helper is callable as a library function

- **WHEN** the codebase is inspected after this change lands
- **THEN** the inventory + validation logic SHALL exist as a
  documented public function in `coordination::inventory`
  with `/tell` as one caller, NOT as a private helper buried
  inside the supervisor-skill code path

#### Scenario: Unknown target produces the documented error shape

- **GIVEN** an inventory with agents `feat/a` and `feat/b`
- **WHEN** the helper is invoked with target `feat/ghost`
- **THEN** the helper SHALL return a rejection containing the
  candidate list `feat/a, feat/b` in a documented error shape
  consumable by any future caller

### Requirement: /tell routing command in the supervisor pane

The supervisor SHALL recognise a `/tell <agent_id> <prompt>`
directive typed in its own tmux pane and route the prompt to the
named agent. The directive SHALL be parseable with the agent
identifier as the first whitespace-delimited token after `/tell`
and the prompt as the remainder of the line (or multi-line
content).

#### Scenario: Successful tell to a live agent

- **GIVEN** an active session with agent `feat/auth`
- **WHEN** the user types `/tell feat/auth rebase onto main`
  in the supervisor pane
- **THEN** the supervisor SHALL deliver `rebase onto main` to
  the `feat/auth` agent via the configured delivery mode and
  acknowledge the routing in its own pane

#### Scenario: Tell with multi-line prompt

- **WHEN** the user types `/tell feat/auth` followed on
  subsequent lines by multi-line content
- **THEN** the supervisor SHALL parse the entire content block
  as the prompt and route it whole

### Requirement: Target validation against the inventory

`/tell` SHALL validate the target agent identifier against the
[[supervisor-directives]] cache. Unknown identifiers SHALL
NOT be delivered; the supervisor SHALL respond in its own pane
with the candidate-list error from the shared validation helper.

#### Scenario: Unknown target produces a candidate list

- **GIVEN** an inventory with agents `feat/a` and `feat/b`
- **WHEN** the user types `/tell feat/ghost ...`
- **THEN** the supervisor SHALL NOT deliver anything and SHALL
  respond with a message listing `feat/a` and `feat/b` as the
  available targets

### Requirement: Delivery mode selection

`/tell` SHALL select a delivery mode using this precedence:
1. When `[supervisor.tell] mode = "send-keys"` is configured AND
   the target's detected mode is `accept-edits`, use
   `tmux send-keys` to inject the prompt directly into the
   target's pane.
2. When `[supervisor.tell] mode = "feedback"` (default), publish
   an `agent.feedback` broker message targeted at the agent.
3. When the configured mode is `send-keys` but the target's
   detected mode is `interactive` or `unknown`, fall back to
   `agent.feedback` and emit a stderr note explaining the
   fallback.

#### Scenario: Default delivery uses agent.feedback

- **GIVEN** no `[supervisor.tell] mode` setting (default)
- **WHEN** `/tell feat/auth rebase onto main` runs
- **THEN** the supervisor SHALL publish an `agent.feedback`
  broker message targeted at `feat/auth` carrying the prompt

#### Scenario: send-keys mode targets accept-edits agents

- **GIVEN** `[supervisor.tell] mode = "send-keys"` and an agent
  whose detected mode is `accept-edits`
- **WHEN** `/tell` targets that agent
- **THEN** the supervisor SHALL use `tmux send-keys` to inject
  the prompt into the agent's pane

#### Scenario: send-keys mode falls back when target mode is unknown

- **GIVEN** `[supervisor.tell] mode = "send-keys"` and an agent
  whose detected mode is `unknown`
- **WHEN** `/tell` targets that agent
- **THEN** the supervisor SHALL fall back to `agent.feedback`
  delivery and SHALL emit a stderr-side note explaining the
  fallback

### Requirement: Routing-decision recording

Every `/tell` invocation SHALL append an entry to a "Supervisor
routing" section of `.git-paw/session-learnings.md` when
`[supervisor] learnings = true`. Each entry SHALL include the
ISO timestamp, target agent, delivery mode, and the prompt
(truncated with `…` past 200 chars). When learnings mode is
disabled the system SHALL NOT write to the learnings file.

#### Scenario: Tell recorded in learnings

- **GIVEN** an active session with `learnings = true`
- **WHEN** `/tell feat/auth rebase onto main` runs
- **THEN** `.git-paw/session-learnings.md` SHALL contain a new
  entry in the "Supervisor routing" section with the
  timestamp, `feat/auth`, the delivery mode, and the prompt

#### Scenario: Learnings disabled means no recording

- **GIVEN** `learnings = false` or no `[supervisor]` config
- **WHEN** `/tell` runs successfully
- **THEN** no file SHALL be written under `.git-paw/`

### Requirement: Proactive routing requires user confirmation

The supervisor SHALL NOT invoke `/tell` autonomously. When the
supervisor identifies a candidate route (an agent blocked on a
question the user has implicitly addressed earlier), the supervisor
SHALL publish an `agent.question` in its own pane describing the
proposed routing and SHALL wait for explicit user confirmation
(e.g. `y`) before invoking `/tell`. No proactive route SHALL
execute without an affirmative reply in v0.6.0.

#### Scenario: Proactive route is offered, not auto-executed

- **GIVEN** a supervisor sweep detects agent `feat/auth` is
  blocked on a layout question the user has previously
  addressed
- **WHEN** the sweep completes
- **THEN** the supervisor SHALL post a question in its own
  pane offering the route, and SHALL NOT invoke `/tell` until
  the user replies affirmatively

#### Scenario: User declines proactive route

- **GIVEN** a proactive-route prompt awaiting confirmation
- **WHEN** the user replies with `n` (or anything other than the
  affirmative)
- **THEN** the supervisor SHALL NOT invoke `/tell` and SHALL
  drop the proposed route

### Requirement: No agent CLI invoked as inference backend

The `/tell` skill SHALL NOT invoke any agent CLI to generate the
prompt content. The prompt comes from the user (typed in the
supervisor pane) or from supervisor LLM reasoning over the
session context; it SHALL NOT be obtained by piping a question
into another agent CLI.

#### Scenario: No inference-backend invocation in the tell path

- **WHEN** `/tell` runs
- **THEN** the operation SHALL consist solely of (a) reading the
  user-typed prompt, (b) inventory lookup, and (c) the chosen
  delivery (broker publish OR `tmux send-keys`), with no agent
  CLI process spawned to produce the prompt content

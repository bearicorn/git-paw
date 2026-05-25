# Agent Coordination

When multiple AI agents work in parallel, they benefit from knowing what the others are doing. The coordination broker is a lightweight HTTP server that lets agents share status updates, publish artifacts, and flag blockers -- all without touching git.

## Enabling the Broker

Add a `[broker]` section to your `.git-paw/config.toml`:

```toml
[broker]
enabled = true
```

When you run `git paw start`, pane 0 becomes a dashboard instead of an agent pane. The dashboard hosts the broker and displays a live status table.

## How Agents Discover the Broker

git-paw sets the `GIT_PAW_BROKER_URL` environment variable in every agent pane. Agents use this URL to send and receive messages. A typical value is `http://127.0.0.1:9119`.

When skill templates are enabled (the default), each agent's `AGENTS.md` also contains curl commands for interacting with the broker, so agents know how to use it without any manual setup.

## Boot-Prompt Injection

To ensure reliable agent self-reporting, git-paw automatically injects a standardized boot instruction block into every agent's initial prompt. This boot block contains pre-expanded curl commands for four essential operations:

### 1. REGISTER - Immediate Status Publication

Agents automatically publish their working status with a "booting" message as their very first action:

```bash
curl -s -X POST http://127.0.0.1:9119/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.status","agent_id":"feat-auth","payload":{"status":"working","message":"booting","modified_files":[]}}'
```

### 2. DONE - Task Completion Reporting

The primary completion path is `git commit`. The git-paw post-commit hook auto-publishes `agent.artifact { status: "committed" }` with `modified_files` derived from `git diff HEAD~1 --name-only`, so agents working on code changes do not publish anything manually — they commit and the hook reports on their behalf.

The boot block retains a manual `agent.artifact { status: "done" }` curl as a fallback for code-less tasks (docs-only updates handled outside the worktree, planning notes, exploration tasks where the artifact is information reported to the broker). The block warns agents NOT to publish manual `done` while their worktree has uncommitted changes — they should commit instead.

```bash
curl -s -X POST http://127.0.0.1:9119/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.artifact","agent_id":"feat-auth","payload":{"status":"done","exports":[],"modified_files":[]}}'
```

### 3. BLOCKED - Dependency Waiting Notification

Agents can properly declare when they're waiting on dependencies:

```bash
curl -s -X POST http://127.0.0.1:9119/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.blocked","agent_id":"feat-api","payload":{"needs":"auth token format","from":"feat-auth"}}'
```

### 4. QUESTION - Uncertainty Escalation (Critical)

Agents are instructed to publish questions and wait for answers rather than guessing:

```bash
curl -s -X POST http://127.0.0.1:9119/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.question","agent_id":"feat-auth","payload":{"question":"Should the JWT use RS256 or HS256 signing?"}}'
```

**IMPORTANT**: The boot block explicitly instructs agents: "DO NOT CONTINUE UNTIL YOU RECEIVE AN ANSWER!"

### Boot Block Injection Modes

- **Supervisor Mode**: Boot block is prepended to each agent's task prompt before injection
- **Manual Broker Mode**: Boot block is pre-filled into each agent pane's input line (user pastes task after boot instructions)

### Paste Handling

The boot block includes instructions for proper paste handling, particularly the requirement to send an additional Enter key after paste operations to ensure full content processing.

### Benefits

- **Reliable Monitoring**: Agents self-report immediately on boot
- **Consistent Behavior**: All agents follow the same coordination pattern
- **No Permission Prompts**: Pre-expanded curl commands avoid shell variable expansion issues
- **Supervisor Visibility**: Questions and blockers surface to the dashboard promptly
- **Audit Trail**: All boot operations are logged in the broker log

## Message Types

Every broker message uses the same JSON envelope:

```json
{
  "type": "agent.<variant>",
  "agent_id": "<slug>",
  "payload": { ... }
}
```

`<variant>` is one of seven shipped values; `<slug>` is the agent's slugified
branch name (lowercase alphanumeric + `-` / `_`; slashes from a branch name
like `feat/auth` become hyphens — `feat-auth`). The seven variants are
`agent.status`, `agent.artifact`, `agent.blocked`, `agent.intent`,
`agent.question`, `agent.feedback`, and `agent.verified`; `src/broker/messages.rs`
is the source of truth for the payload schemas.

> **`agent.status` and `agent.artifact` are normally automatic.** The
> filesystem watcher publishes `agent.status` (with `modified_files`) whenever
> a tracked file changes in a worktree, and the post-commit git hook publishes
> `agent.artifact` with `status: "committed"` and the committed file list every
> time an agent commits. The manual `curl` examples below are escape hatches
> for cases where the automatic publishers do not apply (e.g. code-less tasks
> or heartbeat injection during read-only investigation).

### Status

An agent reports what it is currently doing along with any files it has
already modified in this work step.

```bash
curl -s -X POST "$GIT_PAW_BROKER_URL/publish" \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.status","agent_id":"feat-auth","payload":{"status":"working","modified_files":["src/auth.rs"],"message":"implementing login endpoint"}}'
```

### Artifact

An agent shares the result of a commit (or the analogous output of a code-less
task) so peers can see exports and modified files. The `modified_files` array
is what the conflict detector watches for in-flight overlap.

```bash
curl -s -X POST "$GIT_PAW_BROKER_URL/publish" \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.artifact","agent_id":"feat-auth","payload":{"status":"committed","exports":["AuthClient"],"modified_files":["src/auth.rs","src/auth/client.rs"]}}'
```

### Blocked

An agent declares that it is waiting on something specific from another agent
(or external resource). `from` names the agent that can unblock it.

```bash
curl -s -X POST "$GIT_PAW_BROKER_URL/publish" \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.blocked","agent_id":"feat-api","payload":{"needs":"auth token format","from":"feat-auth"}}'
```

### Intent

An agent declares which files it plans to modify before any edit lands. The
broker conflict detector reads `agent.intent` to flag forward conflicts when
two agents target overlapping paths. `valid_for_seconds` is the TTL after which
consumers MAY treat the intent as stale.

```bash
curl -s -X POST "$GIT_PAW_BROKER_URL/publish" \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.intent","agent_id":"feat-auth","payload":{"files":["src/auth.rs","src/auth/client.rs"],"summary":"wire AuthClient","valid_for_seconds":900}}'
```

### Question

An agent escalates an uncertainty to the supervisor inbox. The asking agent
blocks at its prompt until a typed reply arrives.

```bash
curl -s -X POST "$GIT_PAW_BROKER_URL/publish" \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.question","agent_id":"feat-auth","payload":{"question":"Should the JWT use RS256 or HS256 signing?"}}'
```

### Feedback

A supervisor (or the broker's auto-emitted `[conflict-detector]` voice) sends
a list of error messages to a target agent. The target agent_id field on the
envelope is the *receiver*; `from` inside the payload is the *sender*.

```bash
curl -s -X POST "$GIT_PAW_BROKER_URL/publish" \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.feedback","agent_id":"feat-auth","payload":{"from":"supervisor","errors":["missing rustdoc on AuthClient::new","test for HS256 path is failing"]}}'
```

### Verified

A supervisor confirms that an agent's work has passed every verification gate.
The `agent_id` is the agent whose work was verified; `verified_by` names the
verifier.

```bash
curl -s -X POST "$GIT_PAW_BROKER_URL/publish" \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.verified","agent_id":"feat-auth","payload":{"verified_by":"supervisor","message":"all five gates pass"}}'
```

## Polling for Messages

Agents poll for messages from other agents using cursor-based pagination. The `since` parameter is a sequence number -- the broker returns only messages with a sequence greater than the given value.

```bash
# First poll -- get all messages
curl -s "$GIT_PAW_BROKER_URL/messages/feat-auth?since=0"
```

The response includes a `last_seq` field. Pass this value as `since` on the next poll to get only new messages:

```bash
# Subsequent poll -- only new messages since last check
curl -s "$GIT_PAW_BROKER_URL/messages/feat-auth?since=42"
```

This cursor-based approach is lossless -- no messages are missed between polls, regardless of timing.

## Checking Overall Status

The `/status` endpoint returns a summary of all agents and their latest state:

```bash
curl -s "$GIT_PAW_BROKER_URL/status"
```

## Multi-Repo Considerations

Each git-paw session runs its own broker. If you have multiple repos running sessions simultaneously, each needs a unique port:

```toml
# In repo-a/.git-paw/config.toml
[broker]
enabled = true
port = 9119

# In repo-b/.git-paw/config.toml
[broker]
enabled = true
port = 9120
```

The default port is `9119`. The broker always binds to `127.0.0.1` (localhost only) and should never be exposed to the network.

## Automatic Conflict Detection (v0.5.0)

When supervisor mode is active, the broker runs an in-process conflict
detector that auto-emits `agent.feedback` (tagged `[conflict-detector]` in the
`errors` array) and, for unresolved in-flight conflicts, escalates to the
supervisor inbox via `agent.question`. Three failure shapes are detected:

- **Forward conflicts** — two agents publish `agent.intent` with overlapping
  `files`. Both receive `agent.feedback` listing the other agent and the
  overlapping paths. Toggle with `[supervisor.conflict] warn_on_intent_overlap`.
- **In-flight conflicts** — an agent publishes `agent.status` or
  `agent.artifact` whose `modified_files` overlap with another agent's active
  intent or recent status. The broker waits `[supervisor.conflict] window_seconds`
  (default 120) for one side to retract; if both sides keep modifying, the
  detector escalates to the supervisor inbox via `agent.question`.
- **Ownership violations** — an agent's `modified_files` includes a path the
  spec marks as owned by another change. The violator receives
  `agent.feedback`; if `[supervisor.conflict] escalate_on_violation = true` the
  supervisor inbox also receives a follow-up `agent.question`.

See the [Conflict Detection chapter](conflict-detection.md) for the full
walkthrough — failure shapes, the `[conflict-detector]` tag, supervisor inbox
routing, and the configuration knobs.

## Audit Trail

The broker writes all messages to `.git-paw/broker.log` as JSONL (one JSON object per line). This file is flushed every 5 seconds and provides a complete audit trail of agent communication.

The log file is automatically cleaned up by `git paw purge`. It is also covered by the `.gitignore` entry that `git paw init` creates.

## Working Heartbeat

The broker's filesystem watcher publishes `agent.status` whenever a file in a
worktree changes, which keeps the dashboard's `last_seen` timestamp fresh during
active editing. The watcher cannot observe read-only tool uses (file reads,
greps, searches), permission-prompt waits, or LLM-only deliberation between tool
calls — so a long read-heavy investigation looks stuck on the dashboard even
though the agent is making progress.

To bridge that gap, the embedded coordination skill instructs agents to publish
a lightweight `agent.status` heartbeat every 5 tool uses while actively working:

```bash
curl -s -X POST http://127.0.0.1:9119/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.status","agent_id":"feat-auth","payload":{"status":"working","message":"reviewing auth tests","modified_files":[]}}'
```

The heartbeat reuses the existing `agent.status` shape — no new wire format is
introduced. The broker merges heartbeats with watcher-driven updates without
conflict.

## Commit Cadence

The bundled coordination skill teaches a **per-group** commit cadence. When a
change has an OpenSpec-style `tasks.md` with numbered groups (`## 1.`,
`## 2.`, ...), the agent commits after every `- [ ]` item in a group is
`- [x]` — one group, one commit (by default) — before starting the next
group.

The skill bounds uncommitted work to **roughly ten files** at a time. If a
single group exceeds that mid-implementation, the agent splits into multiple
commits using a `(part N of M)` suffix:

```
feat(coverage): close per-scenario gaps for v0.5.0 (part 1 of 2)
feat(coverage): close per-scenario gaps for v0.5.0 (part 2 of 2)
```

Each commit uses a conventional-commit prefix (`feat(<scope>):`,
`fix(<scope>):`, `docs(<scope>):`, `test(<scope>):`, `chore(<scope>):`) — the
scope is typically the change name's key word.

Per-group cadence protects against agent crashes, conflict mediation, and
`/clear` resets losing unbounded work, and it maps cleanly to the post-commit
hook's `agent.artifact { status: "committed" }` event sequence the supervisor
consumes during verification.

## Terminal Action — Commit Then Publish, Never Archive

The bundled coordination skill defines the coding agent's terminal action as:

1. **A commit.** The post-commit git hook auto-publishes
   `agent.artifact { status: "committed" }` with the committed file list. For
   code changes this is the canonical "done" signal.
2. **A manual `agent.artifact { status: "done" }`** (rare). Used only for
   code-less tasks or to announce named `exports` peers should cherry-pick.

The skill is explicit that the coding agent SHALL NOT invoke
`/opsx:verify <change-id>` or `/opsx:archive <change-id>` — **both are
off-limits for the coding agent and are the supervisor's job**:

- Verification runs the supervisor's five-gate framework (testing → regression
  → spec audit → doc audit → security audit) against the committed branch.
  Self-verification by the coding agent bypasses gates and produces a
  premature `agent.verified` the supervisor never reviewed.
- Archiving happens on the release branch during the supervisor's cherry-pick
  + merge flow, not on the agent's feature branch. Archiving from a feature
  branch leaves the change directory deleted on an unmerged branch and
  produces confused history.

This is a paw-specific rule for the bundled coordination skill. Single-agent
workflows that self-verify can override the rule via the standard skill
resolution chain (a user override at
`<config_dir>/git-paw/agent-skills/coordination.md` wins over the bundled
default).

## Identifier Forms — Branch vs `agent_id`

Two related forms of an agent identifier appear throughout the broker protocol:

- **Branch name** — the original git ref (e.g. `feat/no-supervisor-flag`). Used
  in `git checkout`, `git worktree`, `git push`, and any other git command.
- **`agent_id`** — the dashed slug form (e.g. `feat-no-supervisor-flag`). Used
  in every `/publish` payload, every `/messages/<id>` URL, and the `target`
  field of `agent.feedback` and `agent.question` payloads.

`agent_id` is the slugified form of the branch name. The conversion (named
`slugify_branch` in the source) lowercases the input, replaces every character
outside `[a-z0-9_]` with `-`, collapses runs of `-`, trims leading and trailing
`-`, and falls back to the literal `agent` if the result is empty.

Match the form to the context: dashed `agent_id` in any JSON going to or coming
from the broker; slashed branch name in any shell command involving git.

## Stash Hygiene in Worktrees

When multiple worktrees run side-by-side, every worktree shares the same
underlying git stash list. A `git stash pop` invoked without inspection can pop
an entry created by a different worktree, conflict with your in-progress
changes, and wipe work. The embedded coordination skill teaches agents three
rules:

1. **List before pop** — `git stash list` first; inspect every entry's branch
   label and timestamp.
2. **Inspect before pop** — `git stash show -p stash@{N}` to read the patch
   contents of the specific entry before popping.
3. **Pop only your own** — only pop entries you authored on the current
   worktree. If authorship is uncertain, leave the stash alone and escalate via
   `agent.question`.

Blind `git stash pop` is a data-loss pattern in a multi-worktree session and is
not recommended.

## Supervisor Acknowledgement of `agent.question`

When an agent publishes `agent.question`, it blocks at its prompt waiting for a
typed reply. v0.5.0 agents do not poll their inbox for `agent.feedback`
responses, so a supervisor that only publishes `agent.feedback` to the broker
will see its answer recorded on the dashboard while the asking agent stays
blocked indefinitely.

The supervisor skill therefore instructs supervisors (both human and LLM) to
**both** publish `agent.feedback` **and** send the answer text to the asking
agent's tmux pane via `tmux send-keys`. This dual write is transitional;
MCP-mediated inbox access in v0.6.0 will let agents consume `agent.feedback`
directly and remove the second step.

## Spec Kit Consolidated Worktrees

When git-paw drives a Spec Kit project (`.specify/specs/<feature>/`), each feature's *current phase* decomposes into multiple worktrees:

- One worktree per `[P]`-marked task (branch prefix `task/`). These are parallelisable.
- One *consolidated* worktree per non-`[P]` task group (branch prefix `phase/`). Non-`[P]` tasks share files or context, so a single agent works through them sequentially.

The embedded coordination skill picks up on the branch prefix:

- **`task/<task-id>-<slug>` branches**: the agent runs the standard "before/while editing" coordination pattern for a single task.
- **`phase/<feature>-<phase-slug>` branches**: the agent:
  1. Works through the listed tasks in `tasks.md` order.
  2. Flips `- [ ]` to `- [x]` for each completed task in the worktree's `tasks.md`. The writeback can be a separate commit or bundled with the task's code change.
  3. Publishes `agent.intent` for the union of files across the next 1–2 tasks (with a generous TTL) rather than one publish per task.
  4. Publishes `agent.artifact` with `status: "done"` only when every listed task shows `- [x]` in `tasks.md`. Partial completion is not "done".

When `tasks.md` is the merge-conflict surface between worktrees, git's line-level merge handles per-task checkbox flips automatically. If two worktrees ever flip the *same* task ID, conflict detection (via `agent.intent` overlap) catches it upstream.

## Workflow phases

The bundled coordination skill structures an agent's editing work into two
phases that mirror the skill's "Before you start editing" and "While you're
editing" sections.

### Before you start editing

Before touching any file, the agent:

1. **Reads the spec or task description in full** to understand the scope.
2. **Publishes `agent.intent`** listing the specific files it plans to
   modify, a one-line summary, and a TTL in seconds (default `900` = 15
   minutes). This advertises ownership to the broker conflict detector so
   forward conflicts are caught before any edit lands.
3. **Polls its inbox once** for warnings or overlapping peer intents — not
   a busy loop, a single poll.
4. **Decides on overlap**: if a peer's intent already covers the same
   files, the agent picks among **wait** (peer's TTL is short, work is
   small), **split** (narrow the file list to avoid overlap, re-publish
   `agent.intent` with the reduced scope), or **escalate** (publish
   `agent.question` describing the overlap so the supervisor or human can
   decide). If no overlap is reported, the agent proceeds to edit
   immediately — there is no explicit go-ahead to wait for.

### While you're editing

Once editing is underway, the agent keeps the intent honest and asks rather
than racing:

- **Re-publish intent on scope growth.** If the in-progress work touches
  files that were not in the original `agent.intent`, the agent
  re-publishes `agent.intent` with the expanded `files` list *before*
  touching the new files. The re-published intent replaces the previous
  claim for downstream consumers.
- **Question on peer overlap.** If a peer's `agent.intent` arrives in the
  inbox naming a file in the same module the agent is editing, the agent
  sends `agent.question` describing the overlap and pauses edits on the
  contested file. Silently racing the peer to a commit is forbidden.

The agent **MUST NOT**:

- Perform pairwise check-ins on every change — the broker is not a chat
  channel and peers are not waiting for status pings.
- Wait for an explicit go-ahead from peers when no conflict signal exists
  — silence from the broker means "no overlap detected", not "permission
  pending".
- Block on broker silence — if `agent.intent` polling returns no overlap,
  the agent proceeds.

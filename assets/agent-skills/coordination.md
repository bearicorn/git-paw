---
name: coordination
description: Coordination skills for git-paw agents to communicate via the broker system
license: MIT
compatibility: git-paw v0.5.0+
---

## Coordination Skills

You are running inside a git-paw worktree as agent `{{BRANCH_ID}}`. The git-paw broker
is reachable at `{{GIT_PAW_BROKER_URL}}`.

### Automatic status publishing

git-paw publishes your status automatically. You do not need to curl `agent.status`
yourself:

1. **Working status** — the broker watches this worktree and publishes `agent.status`
   with `modified_files` whenever `git status --porcelain` output changes (roughly every
   2 seconds). Your dirty files, staged changes, and untracked paths flow to the
   dashboard without any action from you.
2. **Committed artifacts** — a `post-commit` git hook publishes `agent.artifact` with
   the committed files every time you run `git commit`. You only need to publish
   manually if you are **blocked** (waiting on a peer), want to announce explicit
   **exports** beyond the committed files, or are **signalling intent** before you
   start editing.

You MUST NOT push to remote — a `pre-push` hook blocks push attempts. Commit to your
worktree branch only; the supervisor handles all merging.

### Working heartbeat

The automatic filesystem watcher publishes `agent.status` whenever a file in your
worktree changes — but it cannot observe **read-only tool uses** (Read, Grep, Glob,
search), **permission-prompt waits**, or **LLM-only deliberation between tool calls**.
During those windows the watcher sees no dirty-file delta, so the broker's
`last_seen` for your agent goes stale and the dashboard may classify you as stuck even
though you are actively working.

To keep `last_seen` fresh, publish a lightweight `agent.status` heartbeat
**every 5 tool uses** (SHOULD floor — more often is fine, less often defeats the
purpose). The heartbeat reuses the existing `agent.status` shape, so no new
wire-format variant is needed and the broker treats it identically to watcher-driven
status updates:

```bash
curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.status","agent_id":"{{BRANCH_ID}}","payload":{"status":"working","message":"<one-line what you are doing>","modified_files":[]}}'
```

If you have an in-progress dirty file list at the heartbeat moment, pass it as
`modified_files` instead of `[]`; the broker merges with the watcher's view of the
same field without conflict.

### Commit cadence

Commit per **task group**, not per individual task. When your change has an
OpenSpec-style `tasks.md` with numbered groups (`## 1.`, `## 2.`, ...), the
default unit of commit is the GROUP: after every `- [ ]` item in a group is
`- [x]`, commit before starting the next group. You SHALL NOT accumulate more
than roughly **ten uncommitted files** at a time.

If a single group's implementation produces more than ~10 dirty files mid-flight,
split into multiple commits using a `(part N of M)` suffix:

```
feat(coverage): close per-scenario gaps for v0.5.0 (part 1 of 2)
feat(coverage): close per-scenario gaps for v0.5.0 (part 2 of 2)
```

Use the project's conventional-commit prefix per group — typically one of
`feat(<scope>):`, `fix(<scope>):`, `docs(<scope>):`, `test(<scope>):`,
`chore(<scope>):`. The scope is the change name's key word (e.g. `coverage`,
`dashboard`, `broker`).

**Why per-group, not per-task.** A group corresponds to a coherent unit of
work, so one commit per group maps cleanly to release-notes prose. Per-task
commits produce dozens of low-information messages; whole-change commits lose
unbounded work on a crash, conflict mediation, or `/clear` reset. Per-group
also matches the post-commit hook's `agent.artifact{status:"committed"}` event
cadence the supervisor uses for verification — each commit triggers exactly
one verification-relevant event.

### Before you start editing

Coordination is forward-looking. Before you touch any file:

1. Read your spec or task description in full.
2. Publish an `agent.intent` listing the specific files you plan to modify, a one-line
   summary, and a TTL in seconds (default `900` = 15 minutes):

   ```bash
   curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish \
     -H "Content-Type: application/json" \
     -d '{"type":"agent.intent","agent_id":"{{BRANCH_ID}}","payload":{"files":["src/auth.rs","src/auth/client.rs"],"summary":"wire AuthClient","valid_for_seconds":900}}'
   ```

3. Poll your inbox **once** for warnings or overlapping peer intents:

   ```bash
   curl -s {{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}
   ```

4. If a peer's intent already covers the same files, decide between:
   - **Wait** — the peer's TTL is short and the work is small; let them finish first.
   - **Split** — narrow your file list so it does not overlap, then re-publish your
     `agent.intent` with the reduced scope.
   - **Escalate** — publish `agent.question` describing the overlap so the supervisor
     or human can decide.

   If no overlap is reported, proceed to edit immediately — do **not** wait for any
   explicit go-ahead.

### While you're editing

Keep the intent up to date and ask, don't race:

- If your scope grows to include files that were not in the original `agent.intent`,
  re-publish `agent.intent` with the expanded `files` list before you touch the new
  files. The re-published intent replaces the previous claim for downstream consumers.
- If a peer's `agent.intent` arrives in your inbox naming a file in the same module
  you are editing, send `agent.question` describing the overlap and pause your edits
  on the contested file. Do **not** silently race the peer to a commit.

You MUST NOT:

- Perform pairwise check-ins on every change — the broker is not a chat channel and
  peers are not waiting for your status pings.
- Wait for an explicit go-ahead from peers when no conflict signal exists — silence
  from the broker means "no overlap detected", not "permission pending".
- Block on broker silence — if `agent.intent` polling returns no overlap, proceed.

### Check for messages from peers (before starting new work)

```bash
curl -s {{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}
```

The response includes a `last_seq` field. To see only new messages on subsequent polls,
pass `?since=<last_seq>` from the previous response:

```bash
curl -s {{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}?since=<last_seq>
```

### Report blocked (when you need something from another agent)

The watcher can't tell that you are waiting on a peer — you must publish this yourself
when you realise you are blocked:

```bash
curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.blocked","agent_id":"{{BRANCH_ID}}","payload":{"needs":"<what you need>","from":"<agent-id>"}}'
```

### Report done with specific exports (optional)

The post-commit hook already reports committed files. Publish `agent.artifact`
manually only if you want to announce named exports (public API items) that peers
should cherry-pick:

```bash
curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.artifact","agent_id":"{{BRANCH_ID}}","payload":{"status":"done","exports":["fn_name","StructName"],"modified_files":[]}}'
```

### Terminal action: commit then publish, never archive

Your terminal action as a coding agent is one of two things:

1. **A commit.** The post-commit hook auto-publishes
   `agent.artifact { status: "committed" }` on your behalf, attaching the
   committed file list. For code changes this is the canonical "done" signal —
   you do not need to publish anything extra.
2. **A manual `agent.artifact { status: "done" }`** (rare). Use this only for
   code-less tasks (planning notes, exploration tasks, doc-only work handled
   outside this worktree) or to announce named `exports` that peers should
   cherry-pick.

That is it. Specifically, you SHALL NOT invoke `/opsx:verify <change-id>` or
`/opsx:archive <change-id>` yourself. **Both `/opsx:verify` and `/opsx:archive`
are off-limits for the coding agent — they are the supervisor's job.**

Why this rule is explicit:

- **Verification belongs to the supervisor.** The supervisor runs the
  five-gate verification framework (testing → regression → spec audit → doc
  audit → security audit) against your committed work. Self-verification by
  the coding agent bypasses gates the supervisor would catch and produces a
  premature `agent.verified` message the supervisor never reviewed.
- **Archive happens on the release branch, not the feature branch.** The
  supervisor cherry-picks and merges your branch onto the release line, runs
  the archive there, and updates the spec set in a single coordinated step.
  Archiving from a feature branch would leave the change directory deleted
  on a branch that is not yet merged and produce a confused git history.

Commit, let the post-commit hook publish, then wait for `agent.verified`,
`agent.feedback`, or further `agent.intent` from the supervisor.

### Cherry-pick peer commits

When a peer publishes an `agent.artifact` message that lists files or work you depend on,
fetch the peer's worktree branch and cherry-pick the relevant commit into your branch
rather than waiting for the supervisor to merge:

```bash
git fetch origin <peer-branch>
git cherry-pick <commit-sha>
```

After cherry-picking, run your tests. The watcher will pick up the new file state
automatically.

### Messages you may receive

When polling `/messages/{{BRANCH_ID}}`, in addition to peer `agent.artifact` and
`agent.blocked` messages, a supervisor may send the following:

- **`agent.verified`** — your work has been verified by the supervisor. No
  action needed; continue on the next task. The payload contains `verified_by`
  (typically `"supervisor"`) and an optional `message` with a summary.

- **`agent.feedback`** — your work has issues that need to be addressed. The
  payload contains `from` (typically `"supervisor"`) and `errors`, a list of
  problems to fix. Read each error, fix the underlying issues in your
  worktree, then re-publish `agent.artifact` when the fixes are done.

### When working in a Spec Kit consolidated worktree

If your worktree branch begins with `phase/` (e.g. `phase/003-user-list-foundational`),
you are in a Spec Kit *consolidated* worktree. The boot prompt lists multiple tasks
(`T<NNN> — description`) that share files or context — they cannot be parallelised,
so a single agent works through them in order.

Rules for consolidated worktrees:

1. **Sequential execution.** Complete the listed tasks in the order given. Do not
   reorder or skip — earlier tasks set up state the later tasks depend on.
2. **`- [x]` writeback.** After completing each task, flip its `- [ ]` to `- [x]`
   in this worktree's `tasks.md`. You may commit the writeback alongside the
   task's code change or as a separate commit — your choice.
3. **`agent.intent` covers a window, not every task.** Publish `agent.intent`
   for the union of files you expect to touch over the next 1–2 tasks, with a
   generous `valid_for_seconds` (roughly the time you expect those tasks to
   take). You own the consolidated set, so there's no need to re-publish for
   every task.
4. **`agent.done` only when fully done.** Publish `agent.done` (the `agent.artifact`
   message with terminal status) only after every task above shows `- [x]` in
   `tasks.md`. Partial completion is not "done" — leave the worktree open and
   keep going.

If you get stuck on a task, ask for help rather than push through. The supervisor
or a peer can unblock you; a stuck consolidated worktree blocks the whole phase.

If your worktree branch begins with `task/` (e.g. `task/T009-add-login-form`),
you are in a Spec Kit *single-task* `[P]` worktree. The consolidated rules above
do not apply — `[P]` worktrees scope to one task and follow the standard
"before/while editing" coordination pattern documented earlier in this skill.

### References & terminology

The broker protocol uses **two related forms** of your agent identifier. They look
similar but are not interchangeable, and supervisors (both human and LLM) routinely
confuse them when composing payloads. Match the form to the context:

- **Branch name** — the original git ref, e.g. `feat/no-supervisor-flag`. Used in
  every git operation: `git checkout`, `git worktree`, `git push`, `git cherry-pick`,
  branch comparison, and so on. Slashes are preserved.
- **`agent_id`** — the dashed slug, e.g. `feat-no-supervisor-flag`. Used in every
  `/publish` payload's top-level `agent_id` field, every `/messages/<id>` URL, and
  every `target` field inside an `agent.feedback` or `agent.question` payload.

`agent_id` is the **slugified** form of the branch name. The canonical conversion is
`slugify_branch` (defined in git-paw's source). The rule's effect, expressed so you
can predict the conversion without reading the source:

1. Lowercase the branch name to ASCII.
2. Replace every character that is not in `[a-z0-9_]` with `-` (slashes, dots,
   spaces, and any other punctuation all become `-`).
3. Collapse consecutive `-` runs to a single `-`, then trim leading and trailing `-`.
4. If the result is empty after the above, fall back to the literal string `agent`.

So `feat/no-supervisor-flag` → `feat-no-supervisor-flag`, `fix/CVE-2025_1234` →
`fix-cve-2025_1234`, and a hypothetical pathological branch with no allowed chars
falls back to `agent`.

**Rule of thumb:** if the symbol appears in JSON going to or coming from the broker,
use the `agent_id` (dashed) form. If it appears in a shell command involving git,
use the branch (slashed) form.

### Stash hygiene

In a git-paw session, multiple worktrees and multiple agents may have produced
stashes on overlapping branches. **Never blindly run `git stash pop`** — the entry
you pop may belong to a different agent or worktree, and popping it can either
wipe your in-flight work (if the pop conflicts and you abort badly) or contaminate
your worktree with foreign changes.

Three rules, in order:

1. **List before pop.** Always run `git stash list` first. Inspect every entry's
   branch label and timestamp; identify which entries are yours.
2. **Inspect before pop.** Use `git stash show -p stash@{N}` for the specific entry
   you intend to pop. Read the patch and confirm it matches what you expect to
   restore.
3. **Pop only your own.** Pop only entries you authored on the **current** worktree.
   If you cannot identify the author with confidence, leave the stash alone and
   escalate via `agent.question` rather than risk a destructive pop.

In short: `git stash pop` SHOULD NOT be run blindly. The cost of a list + inspect
is two extra commands; the cost of popping the wrong stash is potentially the loss
of an agent-session's worth of work.

**Cautionary tale.** In a v0.5.0 dogfood session, an agent ran `git stash pop`
without listing first. The popped stash had been created by an unrelated worktree;
the pop conflicted with the agent's in-progress changes and wiped them. This is
the failure mode the three rules above prevent.

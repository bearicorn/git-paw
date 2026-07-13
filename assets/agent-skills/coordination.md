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
purpose) through the bundled helper, which shapes the `agent.status` payload
internally so you never hand-roll the JSON:

```bash
.git-paw/scripts/broker.sh --agent {{BRANCH_ID}} status "<one-line what you are doing>"
```

The heartbeat reuses the existing `agent.status` shape, so no new wire-format
variant is needed and the broker treats it identically to watcher-driven status
updates. The helper publishes with `modified_files: []`; your actual dirty-file
list still flows to the dashboard continuously from the filesystem watcher, and
the broker merges the two views of that field without conflict — so the
heartbeat's only job is to refresh `last_seen` during the read-only and
deliberation windows the watcher cannot see.

### Commit cadence

Commit per **task group**, not per individual task. When your change has an
OpenSpec-style `tasks.md` with numbered groups (`## 1.`, `## 2.`, ...), the
default unit of commit is the GROUP: after every `- [ ]` item in a group is
`- [x]`, commit before starting the next group. You SHALL NOT accumulate more
than roughly **ten uncommitted files** at a time.

If a single group's implementation produces more than ~10 dirty files mid-flight,
split into multiple commits using a `(part N of M)` suffix:

```
close per-scenario coverage gaps (part 1 of 2)
close per-scenario coverage gaps (part 2 of 2)
```

For the commit-message format itself, follow the **project's** commit-message
conventions — see the project's `AGENTS.md`, which git-paw injects into your
context and which owns the format rules (subject style, any prefix or scope
convention, and any "no AI-assistant trailer" rule). git-paw's bundled skill
does not mandate, default to, or recommend any commit-message format. Defer
entirely to whatever your project's `AGENTS.md` specifies.

**Why per-group, not per-task.** A group corresponds to a coherent unit of
work, so one commit per group maps cleanly to release-notes prose. Per-task
commits produce dozens of low-information messages; whole-change commits lose
unbounded work on a crash, conflict mediation, or `/clear` reset. Per-group
also matches the post-commit hook's `agent.artifact{status:"committed"}` event
cadence the supervisor uses for verification — each commit triggers exactly
one verification-relevant event.

**Each commit is a releasable unit.** Every commit MUST build and pass its own
gates on its own — a commit is a *releasable unit* of work, not a checkpoint of
half-finished progress. When you need to fix the commit you *just* made — a
typo, a missed file, a lint nit — and that commit has not yet been verified and
you have not moved past it, fold the fix into it with `git commit --amend`
rather than landing a separate `fix typo` / `address feedback` micro-commit. Do
**NOT** `git commit --amend` a commit that has already been verified, or an
earlier commit from a previous group: `--amend` applies ONLY to the
most-recent, not-yet-verified commit. Rewriting a verified or earlier commit
corrupts the supervisor's verification boundary.

**Why releasable units, not micro-commits.** git-paw's orchestration model
treats each committed artifact as a verification boundary (the supervisor runs
its five-gate sweep on each `committed` event) and the supervisor curates the
release changelog from these commits. A stream of `fix typo` micro-commits
breaks both: it produces commits that don't build or pass on their own, and it
bloats the changelog the supervisor must hand-curate — prior release cycles had
to hand-squash 148 commits down to 10 (v0.6.0) and 4 down to 1 (v0.7.0). A clean
releasable-unit history avoids that manual squash cost.
### Run dev commands bare — no exit-code-probe wrappers

Run each dev command **bare** and read its exit status directly. Do
**NOT** wrap a command in an exit-code probe such as
`<cmd> && echo "EXIT $?"`, `<cmd>; echo $?`, or `RC=$?; echo "$RC"` just
to print the result.

The probe text varies from one run to the next (a different captured
code, different trailing output), so the CLI's command-string permission
whitelisting never matches the next invocation — every run raises a
fresh approval prompt, and your dev loop stalls on the same safe command
forever instead of being approved once. A bare, prefix-matchable command
generalises across every later run.

This is about the *probe wrapper*, not about the exit status itself:
keep observing and acting on whether a command succeeded or failed —
just let the shell surface the status instead of appending an
`echo "… $?"`.

### Stay inside your worktree

You have your own git worktree. **Every command you run — edits, `git add`,
`git commit`, tests — must execute from your worktree root, using relative paths
only.** Never `cd` to an absolute path outside your worktree, and never
run `git` against another worktree's checkout (no `git -C /other/path ...`, no
`cd /other/worktree`).

Why this matters: linked git worktrees share one `.git/refs` store. If your
shell wanders into the supervisor's main checkout (or a peer's worktree) and
you commit there, the commit advances *that* checkout's branch — not yours.
Your own feature branch stays empty, the integration branch silently absorbs
your unverified work, and reviewers see a contaminated history. This is a
data-integrity bug, not a style nit.

A **pre-commit branch guard** enforces this: if the branch your `git commit`
would advance does not match the branch your worktree was created for, the
commit is refused with an error naming both branches. If you hit that error,
you are committing from the wrong directory — `cd` back to your worktree root
and retry. (The guard can be disabled repo-wide via
`[supervisor] strict_branch_guard = false`, but the post-commit detection
still reports any mismatch to the supervisor.)

### Before you start editing

Coordination is forward-looking. Before you touch any file:

1. Read your spec or task description in full.
2. Publish an `agent.intent` listing the specific files you plan to modify, a one-line
   summary, and a TTL in seconds (default `900` = 15 minutes):

   ```bash
   curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish \
     -H "Content-Type: application/json" \
     -d '{"type":"agent.intent","agent_id":"{{BRANCH_ID}}","payload":{"files":["src/auth/<file>","src/auth/client/<file>"],"summary":"wire AuthClient","valid_for_seconds":900}}'
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

### Declaring regions

By default an `agent.intent` claims whole files, and the conflict detector
warns any two agents who name the same path. That is too coarse when several
agents work different parts of one shared file — the warnings become noise and
real overlaps get dismissed with them. To sharpen the signal, each `files`
entry MAY be an object that names the **regions** within the file you intend to
touch, instead of a bare path string:

```bash
curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.intent","agent_id":"{{BRANCH_ID}}","payload":{"files":[{"path":"src/auth/<file>","regions":[{"kind":"function","name":"validate_token"},{"kind":"function","name":"refresh_session"}]}],"summary":"harden token checks","valid_for_seconds":900}}'
```

A `files` array may freely mix bare-path strings (file-level intent) and
region objects. The region `kind` is one of `function` (`{ "name": ... }`),
`class` (`{ "name": ... }`), `block` (`{ "anchor": "<heading or landmark>" }`,
for prose or config files), or `range` (`{ "start_line": N, "end_line": M }`,
when no symbolic name fits). When **both** agents on a shared file declare
regions, the detector warns only if the regions actually intersect; if **either**
side omits regions, it falls back to the safe whole-file warning.

**Declare regions when:**

- You and a peer both intend the same file but different parts of it — e.g. you
  add `validate_token` while a peer reworks `refresh_session` in the same auth
  file. Naming your regions lets the detector see you don't actually collide.
- You can name the region precisely and stably — a specific function name, a
  type or class name, or a named heading/anchor in a doc or config file.

**Skip regions (just name the file) when:**

- You are about to refactor across the whole file — moving everything, renaming
  the module, reformatting top to bottom. Regions would understate your real
  footprint and mislead peers.
- Your plan is still in flux and you cannot yet name the parts you'll touch.
  Claim the file; re-publish with regions later if it helps.

**Do not manufacture narrow regions to dodge a warning.** Declaring a region
you don't really own — or splitting an honest whole-file change into fake
narrow regions just to suppress the overlap warning — defeats the detector and
hides a collision that will surface later as a merge conflict; the detector is
on your side, so when in doubt, claim the file.

**How to declare so the detector can compare.** The detector matches region
names as strings (it tolerates case, separator, a trailing `()`, and a leading
declaration keyword — but nothing smarter), so how you spell and scope your
declarations decides whether a real overlap is caught:

- **Use the canonical source spelling.** Declare the region name exactly as
  the symbol appears in source — `validate_token`, not a paraphrase like
  `token validation logic`. Two paraphrases of the same symbol are different
  strings, and the detector cannot equate them.
- **Declare ALL the regions your work touches**, not only the headline
  function: shared constant blocks, import sections, and asset files you edit
  along the way all belong in your declaration. An undeclared shared block is
  exactly where two agents collide silently.
- **Re-publish `agent.intent` when your scope grows.** If mid-task you find
  yourself editing a region you never declared, re-publish the intent with the
  full region list before touching it — an outdated declaration understates
  your footprint just like a manufactured-narrow one.

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

### Context budget

Coordinating forward is only half the job — you also have to manage your own
context window. The boot block, this skill, and the governance docs all load
before you read a single source file, so context is spent before task work
begins. Manage it deliberately or you will hit an opaque "context length
exceeded" failure mid-task and lose any work you had not yet committed.

#### Residual-budget heuristic

After the boot blocks, skill prose, and governance docs have loaded, aim to
keep **at least ~60% of the model's context window free** for task work. This
is a heuristic target, not a hard rule — context windows vary by model, and
some tasks legitimately need a fuller window. If you find yourself starting a
task with less than roughly 60% of the window free, that is a signal to
compact or clear before you begin rather than partway through. There is no
config field for this ratio; it lives here as guidance for you to judge.

#### When to compact, clear, or summarise

Reach for these three moments in priority order — read them top-to-bottom and
take the first one that applies:

1. **After each spec scenario completes — compact.** A finished scenario is a
   natural commit point and the smallest safe reduction. Commit the work, then
   `/compact` so recent context is preserved while the older detail is folded
   away.
2. **When the working set grows past ~40% of the window — compact.** Crossing
   this threshold means you are accumulating context that will not all be
   needed later. `/compact` to fold it down before it crowds out task work.
3. **When switching between sub-tasks that don't share state — clear.** A
   clean break is the cheapest fresh start; nothing from the previous sub-task
   carries forward, so `/clear` rather than `/compact`.

#### Commit before you compact

**Never compact, clear, or summarise without first committing — or publishing
an `agent.artifact` — to record your work.** The compact operation reduces
what you can see; if your in-flight work isn't captured in git or in the
broker first, you can't recover it after the context shrinks. The order is
always: record, then reduce.

#### Proactive context-bloat flagging

You are not the only one watching your context. The supervisor flags context
bloat **proactively**: when your pane surfaces a `/clear to save <N>k tokens`
hint whose `N` meets or exceeds the configured threshold
(`context_bloat_threshold_k`, default ~250k tokens), the supervisor publishes a
synthetic `agent.status` with `phase: "context-bloat"` — before you freeze,
while you are still responsive. That early flag exists so the stall can be
pre-empted rather than waited out. When you hit that hint (or the supervisor
flags you), **commit or publish an `agent.artifact` first, then clear or
compact** — the same commit-before-compact discipline above. Recording your
work before you reduce context is exactly what turns the proactive flag into a
safe hand-off instead of a lost-work risk.

#### Per-CLI mechanism

The compact/clear commands differ by CLI. Use the form for the CLI you are
running under:

| CLI | Compact | Clear | Notes |
|---|---|---|---|
| `claude` | `/compact` | `/clear` | preferred path; `/compact` preserves recent context |
| `claude-oss` | `/compact` | `/clear` | same semantics as `claude` |
| other | varies | varies | look for the CLI's `/compact`, `/save`, or `/reset` equivalent |

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

**Stand by after your final commit.** Once your final commit lands, your job is
to *wait* — not to push the change further through its lifecycle. Let the
post-commit hook publish `agent.artifact { status: "committed" }` (or, for
code-less work, publish a manual `agent.artifact { status: "done" }`), then
**stand by**. While standing by you SHALL NOT run `/opsx:verify` or
`/opsx:archive` — those are supervisor-only, as the rule above states. This
stand-by protocol is the positive counterpart to that forbidden-commands rule —
*what to do instead* of reaching for verify/archive. What you wait *for* is one
of three supervisor messages:

- **`agent.verified`** — your work passed verification; pick up the next task.
- **`agent.feedback`** — your work has issues; fix the listed `errors` in your
  worktree and re-publish `agent.artifact`.
- **a further `agent.intent`** — new scope routed to you; pick it up.

Publish the terminal signal, then wait for one of those. Do not self-verify, do
not self-archive, do not assume done-means-merged.

<!-- opsx-role-gating:begin -->
### Commands you must not run

You are a coding agent. You MUST NOT run either of these slash commands:

- `/opsx:verify`
- `/opsx:archive`

These are supervisor-only. The supervisor verifies and archives changes after
your branch merges; if you run them yourself you corrupt the spec lifecycle —
the change is archived without supervisor verification, its delta merges into
the main specs incorrectly, or both.

This is not just convention. git-paw's role-gating guard backs the rule: a
post-commit watcher detects archive activity committed from a coding-agent
worktree and publishes an `agent.feedback` naming the commit and the reason it
fired. In `block` mode (configurable via `[opsx] role_gating`), the supervisor
is additionally asked to revert your archive commit. Commit your work, let the
post-commit hook publish, and wait for the supervisor to verify and archive.

<!-- opsx-role-gating:end -->
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

### When main advances

The supervisor publishes an `agent.advanced-main` event every time it merges a
branch into the default branch. If your work depends on the base — you branched
off it, you want to rebase onto newly-landed code, or you need to re-validate
against the merged result — this event is your signal. Follow this discipline:

1. **Polling source.** The event arrives on your normal
   `/messages/{{BRANCH_ID}}` poll alongside every other message — there is no
   separate subscription. Watch for `"type":"agent.advanced-main"`; its payload
   carries `merged_branch`, `new_main_sha`, `base`, and `merged_at`.

2. **Do NOT auto-rebase.** Receiving the event MUST NOT trigger an automatic
   rebase. Rebasing rewrites your history and can silently drop or conflict with
   in-flight work, so the decision always requires your judgment — never react
   reflexively to the event.

3. **Fetch, inspect, then decide.** When the `base` named in the event is one
   your branch depends on:

   ```bash
   git fetch origin <base>                       # bring the new SHA local
   git log HEAD..origin/<base> --oneline         # see exactly what landed
   ```

   Then choose deliberately between **rebase** (you want the new commits under
   your work), **merge** (you want them alongside without rewriting history), or
   **wait** (the change does not touch your files — keep going). Base the choice
   on what `git log` showed and the state of your working set.

4. **Commit or stash before any rebase.** If you decide to rebase, your working
   tree MUST be clean first. Commit your in-progress work (or stash it
   deliberately — see *Stash hygiene* below) before running the rebase, so a
   conflict during the rebase can never wipe uncommitted edits.

**Concrete example.** You are mid-edit with uncommitted changes in
`src/<your-file>` when an `agent.advanced-main` event reports `feat/auth` merged
into the base your branch sits on. The wrong move is to rebase immediately —
your uncommitted edits are at risk if the rebase conflicts. The right sequence
is: commit your in-progress work first (`git commit -am "wip: ..."`), then
`git fetch origin <base>`, inspect `git log HEAD..origin/<base> --oneline`, and
only then `git rebase origin/<base>` if the landed commits warrant it. Clean
tree first, rebase second.

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

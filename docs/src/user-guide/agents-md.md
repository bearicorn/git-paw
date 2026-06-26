# AGENTS.md Injection

An agent's working context is the *combined view*: the project's tracked
`AGENTS.md` plus a managed git-paw section that carries the *entire spec body*
for the agent's assigned change (proposal, design when present, task list, and
any spec deltas). The agent reads the spec from this combined view plus
`openspec/changes/<id>/`, not from the boot prompt.

git-paw writes that combined view to a **gitignored sidecar**,
`.git-paw/AGENTS.local.md`, in each worktree — **not** to the worktree's
tracked `AGENTS.md`. The boot prompt points the agent at the sidecar. The
tracked `AGENTS.md` is left exactly as the project committed it, so a
legitimate hand edit to it (yours or the agent's) shows up in `git status` and
commits normally, mid-session.

> **Why a sidecar (finding F10).** Through v0.7.0 git-paw injected its section
> into the *tracked* worktree `AGENTS.md` and then ran
> `git update-index --assume-unchanged AGENTS.md` to keep `git add -A` from
> committing the ephemeral injection. That bit is **file-level**: git stopped
> reporting *every* change to `AGENTS.md`, not just the injected block. An agent
> that needed to commit a real `AGENTS.md` edit found `git status` reporting the
> file unmodified and the commit silently empty for that path. Moving the
> injection to a gitignored sidecar removes the reason `assume-unchanged`
> existed: the tracked file is never touched, so it is never hidden.

## Boot-Prompt-Full-Body Model

Earlier versions of git-paw embedded a condensed spec excerpt inside the
supervisor-mode boot prompt itself ("Branch + CLI + Spec content + Owned
files"). The supervisor-mode boot prompt is intentionally short:
it points the agent at the sidecar `.git-paw/AGENTS.local.md` and at the change
directory `openspec/changes/<id>/`, and tells the agent to read those files
before acting. (Because the supported CLIs auto-load only the worktree-root
`AGENTS.md` and not the sidecar, the prompt is what directs the agent to the
sidecar's combined content — every backend, including OpenSpec, points there
first.)

The benefits of this split:

- The spec body lives in a markdown file the agent can re-read at any time,
  not in volatile boot-prompt context.
- The boot prompt becomes small enough to remain in the agent's working
  context for the entire session, even after long conversations.
- Manual operators can pre-stage the boot block at the pane input line via
  paste, the same way the supervisor stages it programmatically.
- The same generator (`src/agents.rs::setup_worktree_agents_md`) writes the
  same content in both supervisor mode and broker-only mode, so the agent's
  experience does not depend on which mode launched it.

## How It Works

1. **Sidecar.** When a session starts, each worktree gets a
   `.git-paw/AGENTS.local.md` sidecar containing the combined view (the root
   `AGENTS.md` content plus the managed git-paw section). The sidecar is
   overwritten on every `git paw start` so the spec body never goes stale.
2. **The tracked `AGENTS.md` is untouched.** git-paw does not write to it, hide
   it, or exclude it. A hand edit to it appears in `git status` and commits
   normally.
3. **Exclusion.** The sidecar path (`.git-paw/AGENTS.local.md`) is added to the
   worktree's ignore set so the ephemeral injection is never committed. For a
   linked worktree, git honors `info/exclude` only from the *common* git
   directory, so the entry is written there.
4. **Self-healing upgrade.** If a worktree carries a stale
   `assume-unchanged` bit on `AGENTS.md` from a pre-sidecar git-paw, the next
   `git paw start` clears it (`git update-index --no-assume-unchanged
   AGENTS.md`) so the tracked file becomes committable again — no manual
   `git update-index` needed.

## Markers

git-paw manages its section in the sidecar using HTML comment markers so the
root `AGENTS.md` content carried alongside it is preserved across launches:

```markdown
<!-- git-paw:start — managed by git-paw, do not edit manually -->

(git-paw content here — full spec body)

<!-- git-paw:end -->
```

Content between these markers is replaced on each launch. Content outside the
markers is preserved. If no markers exist, the section is appended to the end
of the file.

## Sidecar Content

Each worktree's `.git-paw/AGENTS.local.md` sidecar includes:

- The root repo's `AGENTS.md` content (if any), preserved verbatim.
- A git-paw section between the markers carrying:
  - **Branch name** and **assigned CLI** for this worktree.
  - **Full spec body** — proposal, design, tasks, and any spec deltas from
    `openspec/changes/<id>/`. The exact set depends on what the change
    directory contains; missing files (design.md is often optional) are
    silently skipped.
  - **Owned files** list (from OpenSpec file-ownership declarations) when
    declared.
  - **Boot block** that calls the bundled `.git-paw/scripts/broker.sh`
    helper for the agent to self-report status, artifacts, blockers, and
    questions. The helper (installed by `git paw init`, alongside the
    supervisor's `sweep.sh`) discovers the broker URL and shapes the JSON, so
    the boot block carries no raw `curl` or broker URL. The launch path seeds
    a single least-privilege allowlist grant for the helper's stable path
    (`.git-paw/scripts/broker.sh`) instead of a broad `curl` rule — see
    [Coordination](./coordination.md#broker-helper).

The supervisor-mode boot prompt does not duplicate any of this content — it
points at the sidecar. If a change is updated on disk during a session (e.g.
the supervisor edits `tasks.md` to mark a task complete), the worktree's
sidecar is regenerated on the next `git paw start` invocation rather than
live-patched mid-session.

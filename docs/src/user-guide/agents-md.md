# AGENTS.md Injection

`AGENTS.md` is the full source of truth for an agent's working context. When
git-paw launches a session, it generates an `AGENTS.md` in each worktree
containing the project's root `AGENTS.md` content plus a managed git-paw
section that carries the *entire spec body* for the agent's assigned change:
proposal, design (when present), task list, and any spec deltas. The agent
reads the spec from `AGENTS.md` plus `openspec/changes/<id>/`, not from the
boot prompt.

## Boot-Prompt-Full-Body Model

Earlier versions of git-paw embedded a condensed spec excerpt inside the
supervisor-mode boot prompt itself ("Branch + CLI + Spec content + Owned
files"). In v0.5.0 the supervisor-mode boot prompt is intentionally short:
it points the agent at `AGENTS.md` and at the change directory
`openspec/changes/<id>/`, and tells the agent to read those files before
acting.

The benefits of this split:

- The spec body lives in a markdown file the agent can re-read at any time,
  not in volatile boot-prompt context.
- The boot prompt becomes small enough to remain in the agent's working
  context for the entire session, even after long conversations.
- Manual operators can pre-stage the boot block at the pane input line via
  paste, the same way the supervisor stages it programmatically.
- The same generator (`src/agents.rs::setup_worktree_agents_md` or its v0.5.0
  successor) writes the same content in both supervisor mode and broker-only
  mode, so the agent's experience does not depend on which mode launched it.

## How It Works

1. **Worktrees.** When a session starts, each worktree gets its own
   `AGENTS.md` placed at the worktree root. The file is overwritten on every
   `git paw start` so the spec body never goes stale.
2. **Exclusion.** Worktree `AGENTS.md` files are added to the worktree's
   `.git/info/exclude` so they never appear in `git status` or get committed
   alongside the agent's work.

## Markers

git-paw manages its section using HTML comment markers so the rest of the
file (a hand-written project `AGENTS.md`, for example) is preserved across
launches:

```markdown
<!-- git-paw:start — managed by git-paw, do not edit manually -->

(git-paw content here — full spec body)

<!-- git-paw:end -->
```

Content between these markers is replaced on each launch. Content outside the
markers is preserved. If no markers exist, the section is appended to the end
of the file.

## Worktree AGENTS.md Content

Each worktree's `AGENTS.md` includes:

- The root repo's `AGENTS.md` content (if any), preserved verbatim.
- A git-paw section between the markers carrying:
  - **Branch name** and **assigned CLI** for this worktree.
  - **Full spec body** — proposal, design, tasks, and any spec deltas from
    `openspec/changes/<id>/`. The exact set depends on what the change
    directory contains; missing files (design.md is often optional) are
    silently skipped.
  - **Owned files** list (from OpenSpec file-ownership declarations) when
    declared.
  - **Boot block** with the broker `curl` commands the agent uses to
    self-report status, artifacts, blockers, intents, and questions.

The supervisor-mode boot prompt does not duplicate any of this content — it
points at the file. If a change is updated on disk during a session (e.g.
the supervisor edits `tasks.md` to mark a task complete), the worktree's
`AGENTS.md` is regenerated on the next `git paw start` invocation rather
than live-patched mid-session.

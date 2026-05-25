## Why

The boot block injected at the top of every agent's launch prompt (`assets/boot-block-template.md`) instructs the agent on four coordination events: REGISTER, DONE, BLOCKED, QUESTION. Step 2 ("DONE: Task completion reporting") tells the agent to publish `agent.artifact { status: "done" }` "when you complete your assigned task". It does **not** tell the agent to commit first.

During the v0.5.0 dogfood (10-agent supervisor session), 2 of 10 agents — `feat-governance-config` and `feat-cross-format-spec-selection` — followed the boot block literally. They published `agent.artifact { status: "done" }` with fully populated `exports` and `modified_files` lists while leaving 7-12 uncommitted files in their working trees. The supervisor's verification step then failed because the worktrees had nothing to merge: the dashboard reported `done` but `git log` on each branch showed zero new commits.

The agents that succeeded — `feat-governance-context` and `feat-forward-coordination` — never invoked the manual DONE path at all. They committed normally and let the git `post-commit` hook (installed by `git_paw::agents::build_post_commit_dispatcher_hook`) auto-publish `agent.artifact { status: "committed" }` from inside the worktree. The hook reads `$GIT_DIR/paw-agent-id` and computes `modified_files` from `git diff HEAD~1 --name-only`, so the published event reflects the actual commit — not the agent's working-tree guess.

This is a dual-path problem. Two code paths can reach "agent finished":

1. **Commit → hook → committed.** Authoritative: the hook fires from the post-commit context, so the published `modified_files` matches what `git log` will show, and the supervisor can merge by branch name.
2. **Manual `agent.artifact { status: "done" }`.** Subjective: the agent self-reports completion, including what it thinks `modified_files` should be, with no guarantee that any of those files have been committed.

The boot block today treats path 2 as the primary completion mechanism. The dogfood evidence shows path 1 is what works. The manual DONE event is still useful — for tasks that produce no code changes (docs-only edits committed elsewhere, planning notes, exploration runs where the artifact is the answer published into the broker stream itself) — but it should be the exception, not the default.

## What Changes

**Rewrite `assets/boot-block-template.md` step 2.** The current single-block instruction becomes a two-part instruction:

1. **Primary path (code changes):** "When you finish your task, commit your work via `git commit`. The post-commit hook auto-publishes `agent.artifact { status: \"committed\" }` with the committed files attached. You do not need to publish anything manually."
2. **Fallback path (code-less tasks only):** "If your task produces no code changes (docs-only updates handled outside this worktree, planning notes, exploration tasks where the artifact is information reported to the broker), publish `agent.artifact { status: \"done\" }` manually with the curl below. Do NOT publish manual `done` when your worktree has uncommitted changes — commit first instead."

The section heading SHALL still read `DONE: Task completion reporting` (so the four-section structure documented in the spec is preserved) but the body SHALL lead with the commit-first instruction and surface the manual curl as a fallback only.

**No code changes elsewhere.** The post-commit hook in `src/agents.rs::build_post_commit_dispatcher_hook` already publishes the right event; the boot block builder in `src/skills.rs::build_boot_block` already substitutes placeholders into the template. The fix is a content edit in `assets/boot-block-template.md` plus a small set of skill-content tests in `src/skills.rs::tests` asserting the new wording.

## Capabilities

### New Capabilities
*(none — clarifies an existing capability)*

### Modified Capabilities

- `boot-block-format`: the "Boot block content requirements" requirement is modified. The DONE event's content guidance changes from "Instruct agent to publish agent.artifact with done status on completion" to "Instruct agent to commit first (the post-commit hook auto-publishes `agent.artifact { status: \"committed\" }`) and reserve the manual `done` event for code-less tasks". The four-section structure ("Standard boot block format" requirement) and paste-handling requirement are unchanged.

## Impact

**Assets**:
- `assets/boot-block-template.md` — rewrite section `### 2. DONE: Task completion reporting`. The section heading stays; the body changes to lead with the commit-first path and document the manual `done` event as a code-less fallback. The manual curl SHALL still appear so agents who legitimately need it have a copy-paste-able command.

**Tests**:
- `src/skills.rs::tests` — new tests asserting the rendered boot block (a) contains a "commit your work" instruction before the manual-done curl, (b) names the `agent.artifact { status: "committed" }` event published by the post-commit hook, (c) describes when manual `done` is appropriate (code-less tasks), and (d) warns against publishing manual `done` with uncommitted changes. The existing `boot_block_contains_all_four_essential_events` test continues to pass because section 2's heading is unchanged.

**Code**: none. The change is wording only.

**Docs**:
- README and the mdBook chapter on boot prompts do not currently quote the DONE wording verbatim, so no propagation needed beyond confirming. If mdBook coverage exists at `docs/src/architecture/boot-block.md`, update it to describe the commit-first convention.

**Backward compatibility**: strictly better for agents — the new wording removes the dual-path ambiguity. The boot block format (four sections, paste-handling at the end) is unchanged, so any external doc/tooling that parses the structure continues to work. No config changes, no schema changes.

**Mismatches resolved**:
- MILESTONE drift item 38 (boot block's DONE instruction bypasses the commit step): resolved.
- Dogfood pattern where some agents report `done` with uncommitted working trees: eliminated by the wording change; the supervisor's verification cycle no longer needs to special-case `done` events with empty git history.

## Context

Two session documents exist by design. Global session state (`Session`, in the platform data
directory) is git-paw's own record — repo path, worktrees, status, broker port. The per-repo
discovery file (`RepoSessionFile`, in `<repo>/.git-paw/sessions/`) is the public surface for
helpers — session name, an `agents` roster, and an optional orchestrator entry. Each has the
right consumers; the defects are in who reads which, and in who writes the second one.

## Goals / Non-Goals

**Goals:**

- Every agent worktree has the helpers it is told to call.
- `sweep.sh` reads the document git-paw writes for it, in that document's shape.
- A process started anywhere inside a session's repository or its worktrees can find the
  session.

**Non-Goals:**

- Moving either document, or merging the two.
- Writing the discovery file on the spec-driven launch path — `start-spec-launch-parity`
  does that.
- Changing what the helpers do once they have found the session.

## Decisions

**Provision at `provision_worktree`.** It is the one seam the supervisor, bare, spec-driven
and `add` paths all call, so fixing it there satisfies the requirement everywhere and makes
it hard for a future launch path to miss. Reuse `init.rs`'s embedded constants so the
worktree copy cannot drift from the init copy.

**Read `agents`, not `worktrees`.** The discovery file is the helper's contract; the global
shape is not. Aligning the fixtures to the written shape is as important as the code fix —
two fixtures disagreeing is how a reader of the wrong shape kept passing.

**Resolve worktree → owning repository before matching.** `git rev-parse
--git-common-dir` (or the equivalent) from a linked worktree points at the owning
repository's `.git`; resolve that, then match `repo_path`. This fixes MCP without touching
MCP, and keeps `mcp::resolve_repo`'s documented behaviour (a worktree is its own root for
*file* operations) intact.

## Risks / Trade-offs

- **Worktree helper writes overwrite local edits.** Intended — the requirement wants the
  helper to match the running binary — but state it in the docs.
- **The bash mirror can drift again.** Both `assets/scripts/sweep.sh` and the tracked copy
  must change; run `bash -n` on each.
- **A repository with several sessions.** Resolution still returns the first match, as
  today; worktree resolution does not change that.

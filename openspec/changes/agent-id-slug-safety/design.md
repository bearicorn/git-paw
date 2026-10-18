## Context

One function, `slugify_branch` (`src/broker/messages.rs:913`), turns a branch into the broker
`agent_id` for every launch path, the hooks marker, the discovery roster, the boot block, the skills,
the learnings seed and the MCP publish tools (sites listed in the proposal). The shell mirror is
`slugify` in `_paw_common.sh` (Python via `${PY}`, branch passed in the environment), used only by
`broker.sh`'s fallback when no `--agent` is given. The broker validates ids with
`agent_id_regex` (`src/broker/server.rs:38-41`), which no test ties to `slugify_branch`. The Spec
Kit and Superpowers providers reuse `slugify_branch` to build branch names from task text and plan
stems.

## Goals / Non-Goals

**Goals:**
- Every derived id passes the broker validator; a test proves it over a generated corpus.
- Rust and shell derive the same id for every input; a test proves it.
- Branch names git-paw builds keep their released shape.

**Non-Goals:**
- Changing the validator (its bare-word refusal guards explicit ids: typos, unfilled labels).
- Renaming or migrating ids of sessions launched by an older binary.
- Deciding which identity a root session publishes as (`mcp-orchestrator-write-tools`); this change
  only refuses to derive one from the main working tree's branch (D6).

## Decisions

### D1 — Fix the derivation, not the validator

`_` → `-` (the PP-26 fix), plus an `agent-` prefix when the slug has no `-`, so the function is
total over the validator. Alternatives: widen the regex to allow `_` and single words (loses the
validator's typo guard and still lets a branch named `supervisor` derive the reserved id); refuse
single-word branches at launch (a broker-disabled session has no ids to protect, and the refusal
would still leave the derivation partial). The `agent-` prefix also means no branch can ever derive
`supervisor`.

### D2 — Branch names keep `_`; the providers get their own slug

The providers call a new `specs::branch_segment_slug` with the released rules, so `task/…`,
`phase/…` and `plan/…` branches are byte-identical to v0.17. A branch is durable: existing
worktrees, recovery and `--resume` find work by branch name, and a re-scan of an in-flight Spec Kit
feature must map the same task to the same branch. `_` is legal in refs and paths; only the id
grammar forbids it. The released Spec Kit scenario's `task/T009-…` (upper case) never matched the
code, which lowercases (`speckit.rs:814` test pins `task/t009-…`); the MODIFIED text adopts the
shipped lower-case form.

### D3 — The test shares the validator's predicate

`server.rs` exposes `is_valid_agent_id(&str) -> bool` (the compiled regex), used by the handler and
by the property test, so the test cannot drift from the handler. The corpus: every printable ASCII
character alone and between words, leading/trailing/repeated separators, underscores, single words,
`supervisor`, empty, non-ASCII, and the branch shapes git-paw builds. No new dependency (a
hand-built corpus, not a property-testing crate).

### D4 — Parity test runs the real shell function

The test sources `assets/scripts/_paw_common.sh` in `bash` and calls `slugify` once per corpus
entry, passing the name through the environment, and compares with `slugify_branch`. The shell
change mirrors the Rust steps: `re.sub(r"[^a-z0-9]", "-", s)`, collapse, strip, then the two
fallbacks. Keep the heredoc apostrophe-free (`bash -n` after editing).

### D5 — Collisions are refused before anything is created

A shared `ensure_distinct_agent_ids(new: &[&str], existing: &[&str])` returns the first pair whose
ids are equal; `start` (bare, spec-driven, supervised) calls it with the session's branches before
any worktree, `add` with the new branch against the live session's recorded branches. Recovery
re-launches recorded worktrees and does not re-check (it never created them).

### D6 — No id is derived in the main working tree

The `agent-` prefix turns a publish from the main working tree on `main` without an explicit id —
today an HTTP 400 — into a phantom `agent-main`. Agents publish from linked worktrees; the main
working tree is the orchestrator's, which always passes an explicit id. So `broker.sh`'s
`resolve_agent` and the MCP worker publish tools' `resolve_agent_id` refuse to derive an id there
(error, nothing published). Root detection is the shipped one: `cli_setup::shell::resolve_layout` +
`RepoLayout::is_linked_worktree` (`src/cli_setup/shell.rs:41`, git dir vs common git dir, correct
under `--separate-git-dir`) in Rust, and in the helper the same comparison of the absolute
`git rev-parse --git-dir` with the resolved `--git-common-dir`. An explicit `--agent` skips the check.
`mcp-orchestrator-write-tools` (17) later gives a root MCP session the explicit identity `supervisor`;
its verdict tools never derive from the branch and are unaffected.

## Risks / Trade-offs

- [A publish from the main checkout on a single-word default branch (`main`) without `--agent`]
  → was a 400; would now land as phantom agent `agent-main`, so it is refused instead (D6). The
  boot block and skills always pass explicit ids from the root.
- [Old live sessions] → ids change only when a session is relaunched or recovered; `_` ids never
  published over HTTP, so nothing persisted under them needs migrating.
- [More collisions after `_` → `-`] → refused loudly at launch instead of sharing an inbox.
- [MCP inventory pane matching] → `coordination::inventory::match_pane` (`inventory.rs:187-199`)
  matches an id against the worktree directory's basename, and a child worktree directory keeps `_`
  (and `.` and case), so an underscored branch's pane stays unmatched there — as dotted or
  upper-case branches already are today. `terminal-backend-seam` (12) adds exact roster matching
  ahead of that fallback; the drive loop and `sweep.sh` already resolve through the roster.

## Migration Plan

Code + asset change. Rollback: revert; no persisted state.

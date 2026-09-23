## Context

Two session documents exist by design. Global session state (`Session`, in the platform data
directory) is git-paw's own record — repo path, worktrees, status, broker port. The per-repo
discovery file (`RepoSessionFile`, in `<repo>/.git-paw/sessions/`) is the public surface for
helpers — session name, an `agents` roster, and an optional orchestrator entry. Each has the
right consumers; the defects are in who reads which, and in which launch paths write the second
one.

Helper provisioning already exists: `init::provision_worktree_helpers(worktree, broker_enabled,
docs_fetch_configured)` writes `broker.sh` / `docs-fetch.sh` (plus the shared `_paw_common.sh`)
from the embedded assets, idempotently and executable. `attach_agent` (`src/main.rs`) calls it, so
the supervisor launch and `git paw add` are covered. Bare `cmd_start` and spec-driven
`launch_spec_session` (`src/commands/start.rs`) do their own per-worktree setup and do not call it.
`.git-paw/scripts/` is gitignored (`.gitignore`: `.git-paw/scripts/*`); `git paw init` writes the
repo-root copies.

The discovery file is written by `write_repo_discovery_file` (`src/main.rs`) from `cmd_start`,
`cmd_supervisor`, `git paw add` and `git paw remove`, and removed by `purge`. Recovery
(`recover_session`, `src/commands/recover.rs`), restart from pause (`restart_from_pause`,
`src/commands/start.rs`) and the spec-driven launch never write it.

## Goals / Non-Goals

**Goals:**

- Every agent worktree has the helpers it is told to call, on every launch path.
- `sweep.sh` reads the document git-paw writes for it, in that document's shape, and that document
  exists and is current after every launch, recovery and resume.
- A process started anywhere inside a session's repository or its worktrees can find the
  session.
- A helper verb that the broker rejects never reports success.
- One learning-id scheme across the aggregator and `sweep.sh learn`.

**Non-Goals:**

- Moving either session document, or merging the two.
- Changing what the helpers do once they have found the session, beyond the listed fixes.
- Specifying the learning-id scheme here — `retire-stale-requirements` owns *Deterministic id for
  idempotent re-emission*; this change only makes `sweep.sh learn` follow it.

## Decisions

**Provision on the two missing paths by calling the existing helper.** `cmd_start` and
`launch_spec_session` call `init::provision_worktree_helpers` right after their
`provision_worktree` call, with the same gates `attach_agent` uses (broker enabled; docs-fetch
configured). Moving the call into `worktree_provision::provision_worktree` was considered — it is
the seam every path calls — but that function is config-agnostic (it takes only
`WorktreeConfig`), and threading broker/docs flags into it widens its contract for one caller
pattern. A per-launch-path test (task 1.1) is what keeps a future path from missing it.

**Read `agents`, not `worktrees`.** The discovery file is the helper's contract; the global
shape is not. Aligning the fixtures to the written shape is as important as the code fix —
two fixtures disagreeing is how a reader of the wrong shape kept passing.

**Phantoms are decided by the roster, not by a branch-name pattern.** The known set is
`supervisor` plus the roster's `branch_id`s. Without a roster (no discovery file — e.g. a session
created outside `git paw start`), fall back to the broker's own agent-id rule (`supervisor`, or a
`<prefix>-<name>` / `<prefix>/<name>` slug with any prefix), never a hard-coded `feat`. The
existing *sweep.sh status filters phantom agents* scenario (`supervisor`, `feat-x` shown; `a`,
`<agent-id>` filtered) holds under both.

**Write the discovery file wherever panes are (re)built.** Recovery rebuilds the layout from the
saved session and knows its offsets, so it reuses `write_repo_discovery_file`. Restart from pause
recreates the dashboard pane at whatever index tmux assigns, so agent `pane_index` values are read
back from the live session (`list-panes` with `pane_current_path`, matched to each worktree) rather
than assumed.

**Resolve worktree → owning repository before matching.** `git rev-parse
--git-common-dir` (or the equivalent) from a linked worktree points at the owning
repository's `.git`; resolve that, then match `repo_path`. This fixes MCP without touching
MCP, and keeps `mcp::resolve_repo`'s documented behaviour (a worktree is its own root for
*file* operations) intact.

**Default the intent TTL in the helper, not the wire.** The broker's `valid_for_seconds` is a
required, strictly positive field of `agent.intent` (`broker-protocol`). Making it optional on the
wire would be a protocol change for one helper; the helper instead sends `900` (the TTL the
coordination skill's examples use) when the caller gives none.

**Publish failures are loud.** `publish()` in `broker.sh` and `sweep.sh` checks the HTTP status
(e.g. `curl -sS -o <body> -w '%{http_code}'`) and on anything but `2xx` prints the broker's error
body to stderr and exits non-zero. Callers that deliberately ignore publish errors keep doing so
explicitly (`|| true`), so the silent path is a visible choice rather than the default. The shared
preamble `_paw_common.sh` is the natural home for one `publish()`.

**`sweep.sh learn` reproduces the aggregator's canonical form.** Its id is the scheme in
*Deterministic id for idempotent re-emission* over `category`, an empty branch id (supervisor
learnings are cross-cutting), the canonical body and the current UTC hour bucket — in Python, a
canonicaliser mirroring `canonical_value` plus the pinned hash. A parity test computes the id for
the same inputs both ways.

## Risks / Trade-offs

- **Worktree helper writes overwrite local edits.** Intended — the requirement wants the
  helper to match the running binary — but state it in the docs.
- **The bash mirror can drift again.** `assets/scripts/sweep.sh` is the source; the repo-root copy
  is regenerated by `git paw init` and worktree copies on attach. Run `bash -n` after every edit
  (heredoc apostrophes desync bash quoting).
- **A repository with several sessions.** Resolution still returns the first match, as
  today; worktree resolution does not change that.
- **Loud publish failures can surface pre-existing silent errors** (e.g. a stale TTL-less intent
  call in a consumer script). That is the point; the error names the broker's reason.
- **Canonical-form drift between Rust and Python.** Numbers and string escaping must render the
  same way (`serde_json` vs `json.dumps(..., ensure_ascii=False, separators=(",", ":"))`); the
  parity test covers nested objects, arrays, unicode and integers. Floats are not used in learning
  bodies today; the test pins the current behaviour.

## Context

| Rule | Where | Output for `feat/Auth#2` | Used for |
|---|---|---|---|
| Agent id | `broker::messages::slugify_branch` (+ `skills::slugify_branch` re-export) | `feat-auth-2` | broker `agent_id`, boot blocks, skills, MCP publish identity, Spec Kit / Superpowers task branches |
| Agent id (shell) | `_paw_common.sh::slugify`, `sweep.sh::resolve_agent_for_path` inline `slug()` | `feat-auth-2` | `broker.sh` identity, `sweep.sh detect-stuck` pane-to-agent mapping |
| Log stem | `logging::sanitize_branch_for_filename` / `unsanitize_branch_from_filename` | `feat--Auth#2` | `.git-paw/logs/<session>/<stem>.log`, `replay` |
| Worktree slug | `domain::BranchSlug::for_branch` via `git::branch_slug` | `feat-Auth2` | `.git-paw/worktrees/<slug>/` (persisted in `WorktreeEntry.worktree_path`) |

The three rules differ on purpose. The agent id must be a broker-safe identifier, the log stem
must reverse to the branch, and the worktree slug keeps case and `.`, which existing directories
on disk depend on. Each output is persisted or appears on the wire, so unifying the *output*
would rename existing artifacts. What the MILESTONE item actually needs is a single
*construction point* that names the purpose of each derivation.

The per-pane claim TTL lives in `supervisor::claim::CLAIM_TTL` (`Duration::from_secs(10)`) and
in `sweep.sh` (`CLAIM_TTL_SECONDS=10`, consumed by `claim_is_abandoned` through a Python `mtime`
check). Since v0.17.0 the claim guards the explicit `sweep.sh approve` and the drive loop's nudge
and submit keystrokes; nothing approves on its own. The comments on both say they "MUST stay byte-identical". Nothing enforces it.

`code-architecture` already requires that injection-prone strings are *constructed* through
newtypes. This change extends that to *carrying* them across boundaries.

## Goals / Non-Goals

**Goals:** one place builds branch-derived names, one canonical TTL, typed identifiers on ports,
and zero behavior change.

**Non-Goals:**
- Changing any rule's output.
- Sanitizing `BranchSlug` further.
- Rewriting tmux-adapter internals.
- A hidden binary query for the TTL (D3 alternative).
- Fixing `sweep.sh resolve_agent_for_path`'s key mismatch (it reads `worktrees[].branch` from a
  file that has `agents[].branch_id`). That is a behavior fix, reported separately. This change
  only moves where its slug is computed.

## Decisions

### D1 — `BranchName` is a value object with named derivations, not a merged rule
`BranchName(String)` is built from a git branch name with `BranchName::new` (no validation; any
branch git accepts). Its methods:
- `agent_id() -> AgentId` — the five-step broker rule;
- `worktree_slug() -> BranchSlug` — the existing constructor;
- `log_file_stem() -> String` and `BranchName::from_log_file_stem(&str)`.

`AgentId(String)` can only be built by `BranchName::agent_id`, by `AgentId::supervisor()`, or
by `AgentId::from_wire(&str)`, which accepts a value the broker already validated. The old free
functions remain as one-line wrappers, so the `broker-protocol` *Branch slug function*
requirement and every caller and test keep compiling. The table tests move onto the methods
unchanged.

*Alternative:* one `slugify` with a mode enum. It hides the purpose at the call site, and a
wrong mode would be silent. Rejected.

### D2 — One shell copy of the agent-id rule, proven equal to Rust
`_paw_common.sh::slugify` is the shell definition. `sweep.sh::resolve_agent_for_path` returns
the matched branch from its Python, and the bash caller then runs `slugify "$branch"`, so the
inline `slug()` goes away. The heredoc stays apostrophe-free and is checked with `bash -n`.

`tests/slugify_shell_parity.rs` sources `_paw_common.sh` in a temp git repo and runs `slugify`
over the same table as `slugify_branch_normalises_each_case`, including the empty input,
all-separator input and non-ASCII input. It asserts equality with `BranchName::agent_id`.

`resolve_agent_for_path` currently reads `worktrees[].branch` from the per-repo discovery file,
but that file carries `agents[].branch_id`, so in practice it resolves nothing. The correct fix
is to read `agents[].branch_id`, which `write_repo_discovery_file` already slugifies; that
would remove the slug call from this function entirely. It is a behavior fix, so it is reported
for its own change and not folded in here (`code-architecture`: *A behavior change is not
folded into a refactor wave*).

*Alternative:* a hidden `git paw __slug <branch>`, on the `__prompt-profile` precedent. It
spawns a process per call and still needs a Python fallback when `git paw` is not on PATH, so
it is not single-sourced either. Rejected.

### D3 — The claim TTL: Rust is canonical, the shell mirror is pinned behaviorally
`CLAIM_TTL` stays the definition. `sweep.sh` keeps `CLAIM_TTL_SECONDS`, with a comment naming
the Rust constant and the parity test. `tests/claim_ttl_parity.rs`:
1. creates `.git-paw/tmp/approve-pane-3.claim` through Rust's `claim_path`;
2. backdates it to `CLAIM_TTL - 2s`, then asserts that both
   `PaneClaim::try_acquire(root, 3)` and `sweep.sh`'s `claim_is_abandoned` (run through a small
   driver) report it held;
3. backdates it to `CLAIM_TTL + 2s` and asserts both report it abandoned;
4. checks that `sweep.sh`'s `claim_path 3` equals Rust's path.

*Alternative:* `sweep.sh` reads the TTL from the binary through a hidden query. That needs a
fallback literal anyway (the helper must work without `git paw` on PATH), so it moves the
duplication without removing it, and it adds a hidden surface just before the freeze. Recorded
as open question 1.

### D4 — Typed identifiers at boundaries only
Signatures change where a value crosses a seam:
- the `TerminalBackend` port (v0.18): session-taking methods take `&SessionName`;
- `session::{save_session, load_session_from, delete_session}`;
- the command-library functions (v0.18 `cmd-library-surface`);
- the broker client's publish identity: `&AgentId`;
- `WorktreeEntry.worktree_path` in the model: `WorktreePath`.

Inside the tmux adapter and at argv building, `.as_str()` is used. Recorded names read back from
the session file are wrapped with `SessionName::from_recorded(String)`. That constructor accepts
the stored string verbatim, because git-paw itself produced it and the file is user-private.
Re-sanitizing would change pre-hardening names and break recovery of old sessions.

A standing source guard (`tests/source_audit.rs` style) asserts that no `pub fn` signature
outside `src/tmux/**` and `src/terminal/tmux.rs` declares `session_name: &str`.

*Alternative:* convert all 91 sites now. Most of them sit in `drive.rs` and `tmux/` (v0.17.0
deleted `poll.rs` and `approve.rs`, which held the rest), which v0.18 rewrites. Rejected as churn.

## Risks / Trade-offs

- [A delegating wrapper diverges from the method] → the wrappers are one-liners, and the
  existing table tests run against both.
- [The `sweep.sh` heredoc edit breaks quoting (the known apostrophe landmine)] → no apostrophes
  in the edit, `bash -n` in the test, and the existing `sweep_sh_conventions` lint.
- [`from_recorded` is a bypass of sanitization] → its use is limited to deserialization edges
  by the guard, and it is documented as a trust boundary.

## Migration Plan

1. Add `BranchName` / `AgentId` and switch the call sites. The table tests are unchanged.
2. Make the `sweep.sh` slug single-source, and add both parity tests.
3. On the released v0.18.0 code, adopt the typed identifiers on the port, the store and the command-library
   signatures, and add the guard.

Every step is behavior-preserving and gated on the full suite.

## Open Questions

1. A parity-pinned mirror for the TTL (recommended), or a hidden binary query plus a fallback
   literal.

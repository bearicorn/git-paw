## Why

Branch-derived names and shared constants are re-implemented wherever they are needed. The
copies already disagree on their scope, and they sit on frozen contracts: the agent id is the
broker identity, the worktree directory is in persisted paths, and log file names are what
`replay` lists.

- **Four branch-to-name rules, five copies.** There are four rules:
  - `broker::messages::slugify_branch`, the agent id: lowercase, non-`[a-z0-9_]` becomes `-`,
    runs collapsed, ends trimmed, empty becomes `agent`;
  - `logging::sanitize_branch_for_filename`, the log file stem: `/` becomes `--`, and the rule
    is reversible;
  - `domain::BranchSlug::for_branch`, the child-worktree directory: `/` becomes `-`,
    `[A-Za-z0-9._-]` is kept, anything else is dropped;
  - the agent-id rule again, copied in Python: once in `_paw_common.sh::slugify` and again
    inline in `sweep.sh::resolve_agent_for_path`.

  MILESTONE counts "three slugify rules". There are three distinct rules for three different
  purposes, plus a duplicated fourth, and they must not be merged into one output: that would
  rename worktree directories and log files.
- **The per-pane claim TTL is defined twice:** `supervisor::claim::CLAIM_TTL` (10 s) and
  `sweep.sh`'s `CLAIM_TTL_SECONDS=10`. Only a comment keeps them equal, and no test checks it.
  Since v0.17.0 the claim guards the explicit `sweep.sh approve` and the drive loop's nudge and
  submit keystrokes (no component approves on its own).
- **The `domain.rs` newtypes are barely used.** `SessionName` is built in one place and then
  unwrapped into 91 `session_name: &str` / `session: &str` parameters (counted before v0.17.0
  deleted `supervisor/poll.rs` and `supervisor/approve.rs`), most of them in `supervisor/drive.rs`
  and `tmux/`, so nothing stops a raw string from reaching a tmux target.

## What Changes

- **One branch value object**, `domain::BranchName`, is the only place a branch-derived name is
  built. It provides:
  - `agent_id() -> AgentId`, the broker slug rule;
  - `worktree_slug() -> BranchSlug`, the existing rule;
  - `log_file_stem()` and the inverse `from_log_file_stem()`.

  Each derivation is byte-identical to today. `slugify_branch`, `sanitize_branch_for_filename`,
  `unsanitize_branch_from_filename` and `git::branch_slug` stay as thin delegating wrappers, so
  their specs and callers hold. Call sites construct through `BranchName`.
- **One shell copy of the agent-id rule.** `sweep.sh::resolve_agent_for_path` stops carrying its
  own `slug()`; its Python returns the branch, and the shell runs `_paw_common.sh::slugify` on
  it. A parity test runs the shell `slugify` over the Rust unit-test table.
- **One canonical claim TTL.** `supervisor::claim::CLAIM_TTL` is the definition. `sweep.sh`'s
  value is a declared mirror, locked by a behavioral parity test: a claim aged just under the TTL
  is held for both the Rust acquirer and `sweep.sh`, and one aged just over it is stealable by
  both. The same test checks the claim path formula.
- **Typed identifiers at boundaries.** The terminal backend port, the session store API and the
  command-library entry points take `&SessionName`, `&AgentId` and `&WorktreePath` instead of
  `&str` / `&Path`. Raw strings remain only inside the tmux adapter's argv building and at
  deserialization edges, through `SessionName::from_recorded`. A source guard keeps
  `session_name: &str` out of public signatures outside the tmux adapter.

Behavior-preserving: no byte of output, path, wire or persisted state changes.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `code-architecture`: ADDED "Branch-derived names come from one value object" and "Domain
  identifiers are typed at port and use-case boundaries".
- `broker-agent-helper`: ADDED "Helper agent-id slugging has a single shell definition that
  matches the binary".
- `supervisor-unattended-operation`: ADDED "The per-pane claim TTL has one canonical definition
  mirrored by the helper".

## Impact

- **Code:**
  - `src/domain.rs` gains `BranchName`, `AgentId` and `SessionName::from_recorded`;
  - `src/broker/messages.rs::slugify_branch`, `src/logging.rs`, `src/git.rs::branch_slug` and
    `src/skills.rs::slugify_branch` delegate to it;
  - call sites in `src/{agents,selftest,main}.rs`, `src/commands/{add,start,supervisor}.rs`,
    `src/mcp/tools/publish.rs` and `src/specs/{speckit,superpowers}.rs` change;
  - `session_name: &str` becomes `&SessionName` across `src/session.rs`, the v0.18 `src/terminal/`
    port and `src/commands/`;
  - `assets/scripts/sweep.sh` (`resolve_agent_for_path`).
- **Tests:**
  - the existing slug, log-name and branch-slug unit tables move onto `BranchName` unchanged;
  - new `tests/slugify_shell_parity.rs` and `tests/claim_ttl_parity.rs`;
  - a signature source guard;
  - every behavioral suite unchanged.
- **Docs:** `docs/src/architecture.md` (domain value objects) and the `code-standards` skill's
  newtype section, which already prescribes this (a repo-local dev skill, not an export).
- **Enum-variant ripple:** none.
- **Export-agnosticism:** `sweep.sh` changes only how the slug is computed. No project
  convention enters the asset.
- **Cross-version dependencies:**
  - Follows v0.17 `classifier-retirement`, which deletes `supervisor/poll.rs` and
    `supervisor/approve.rs` (moving `KeyDispatcher` to `supervisor/keys.rs`) and MODIFIES *Approval
    on a pane is guarded by an exclusive per-pane claim* so the claim guards the explicit approve
    and the drive loop's nudge and submit keystrokes. This change only ADDs beside that requirement.
  - **Must follow v0.18 `terminal-backend-seam`.** Most of the 91 signatures sit in code that
    change replaces with the `TerminalBackend` port, so the newtypes are adopted on the port's
    signatures rather than churned in code about to be deleted.
  - Also follows v0.18 `cmd-library-surface`, whose entry points take typed inputs.
  - v0.18 `supervisor-pane-removal` / `terminal-backend-seam` modify other
    `supervisor-unattended-operation` requirements. This change only ADDs, so there is no
    collision.
  - Pairs with this release's `wire-format-dtos`: `AgentId` is what the broker wire mapper
    produces.

## Why

**PP-26** (peer-4-poker on 0.17.0, `.git-paw/peer4poker-0.17.0-findings.md`): a Spec Kit task whose
text names `ROOM_SETUP.md` produced an agent whose every publish was HTTP 400 — the branch-refresh
notify, the post-commit artifact, its own status. The agent id is derived from the branch by
`slugify_branch` (`src/broker/messages.rs:913-953`), which keeps `_` (step 2, `:921`), but the
broker's `/publish` validator accepts only `^(supervisor|[a-z0-9][a-z0-9-]*[/-][a-z0-9][a-z0-9-]*)$`
(`src/broker/server.rs:38-41`), with no `_`. The Spec Kit decomposition builds the branch from the
task text with the same function (`task/<slug>` at `src/specs/speckit.rs:326`, `phase/<slug>` at
`:347`; Superpowers `plan/<slug>` at `src/specs/superpowers.rs:114`), so any task or plan naming an
underscored file yields an agent the broker refuses.

Checking the premise against the released code found a second gap of the same class: the validator
also refuses a single word, and `slugify_branch` returns one for a single-word branch (`hotfix` →
`hotfix`) and for its empty fallback (`agent`). Nothing pins the derived id to the validator, so
the two drifted. A branch named `supervisor` even derives the reserved `supervisor` id.

## What Changes

- **The derived agent id is always valid.** `slugify_branch` maps every character outside
  `[a-z0-9]` — `_` included — to `-`, and a result without a `-` (a single word, or empty) gets an
  `agent-` prefix (`hotfix` → `agent-hotfix`, empty → `agent-unnamed`). Every output therefore
  matches the `/publish` pattern and none equals `supervisor`. Pinned by a property/table test over
  a generated corpus, against the validator's own predicate.
- **Branch names keep `_`.** The spec providers stop borrowing the agent-id function for branch
  names and use their own branch-segment slug with the released rules (lowercase, non-`[a-z0-9_]` →
  `-`, collapse, trim, empty → `agent`). A branch is durable git state (existing worktrees,
  recovery and `--resume` find work by branch name) and `_` is legal in a ref; only the broker id
  grammar forbids it, so the fix sits at the id boundary.
- **One derivation everywhere.** Every agent-id site already calls `slugify_branch`
  (`src/main.rs:635`, `:792`, `:1032`; `src/agents.rs:165`; `src/skills.rs:405-406`, used at `:480`
  and `:629`; `src/commands/start.rs:294`, `:705`, `:992`; `src/commands/add.rs:265`;
  `src/commands/supervisor.rs:977`; `src/broker/learnings.rs:426`; `src/mcp/tools/publish.rs:43`;
  `src/selftest.rs:606`), so they follow the new rule unchanged. The one shell mirror,
  `slugify` in `assets/scripts/_paw_common.sh:218-229` (used by `broker.sh`'s `resolve_agent`,
  `:57-71`), is updated in lockstep, pinned by a bash/Rust parity test. `sweep.sh` reads
  `branch_id` from the discovery file and never slugifies; the git hooks publish the id the launch
  wrote into the worktree marker.
- **Distinct ids per session.** Mapping `_` to `-` makes `feat/a_b` and `feat/a-b` share an id
  (as `feat/x` and `feat-x` already do). Every launch path that derives ids — bare, spec-driven and
  supervised `start`, and `add` against the live session's agents — refuses before creating a
  worktree when two branches derive the same id, naming both.
- **No id is derived in the main working tree.** With the `agent-` prefix, a publish from the main
  working tree on `main` without an explicit id would land as a phantom `agent-main` instead of
  failing with HTTP 400 as today. `broker.sh` without `--agent` and the MCP worker publish tools
  therefore refuse to derive an id there, with a clear error and nothing published; root detection
  reuses the shipped `cli_setup::shell::RepoLayout::is_linked_worktree` (`src/cli_setup/shell.rs:41`)
  and, in the helper, the same comparison of `git rev-parse --git-dir` with `--git-common-dir`.
  Explicit ids (`broker.sh --agent supervisor …`) are unaffected.
- The coordination skill's terminology section and the coordination chapter describe the new rule.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `broker-protocol`: *Branch slug function* — `_` maps to `-`, single-word and empty results get the
  `agent-` prefix, every output matches the `/publish` pattern, one derivation shared with the shell
  helper; new requirements that a session's derived ids are distinct and that no publishing id is
  derived in the main working tree.
- `mcp-agent-publish`: *Publish tools act as the calling agent only* — a publish tool in the main
  working tree refuses instead of deriving an id from its branch.
- `spec-providers`: *Branch-name shape for SpecKit entries* and *Branch-name shape for Superpowers
  entries* — branch names use the providers' own branch-segment slug (keeps `_`).
- `skill-agent`: *Embedded coordination skill* — item 11 describes the new derivation rule.

## Impact

- **Predecessors (implementation order):** archive position 5. No earlier v0.18 change modifies any
  of the five requirements touched here (`broker-verdict-authority` only ADDs to `broker-protocol`;
  `broker-verdict-authority` and `orchestrator-skill-export` modify other `skill-agent`
  requirements), so every MODIFIED delta is written on the released v0.17.0 text.
- **Successors:** `supervisor-pane-removal` (6) and later changes keep deriving ids through
  `slugify_branch`; `mcp-orchestrator-write-tools` (17) MODIFIES *Publish tools act as the calling
  agent only* again on this change's text, giving a root session the explicit publisher identity
  `supervisor` in place of this change's refusal (see design, Risks).
- **Code:** `src/broker/messages.rs` (`slugify_branch`), `src/broker/server.rs` (expose the
  validator as a predicate the test shares), `src/specs/mod.rs` (branch-segment slug),
  `src/specs/{speckit,superpowers}.rs` (call it), a shared distinct-id check called from
  `src/commands/{start,supervisor,add}.rs`, `assets/scripts/_paw_common.sh` (`slugify`; the
  deployed `.git-paw/scripts/` copy is gitignored and refreshed by `git paw init`),
  `assets/scripts/broker.sh` (`resolve_agent` refuses in the main working tree),
  `src/mcp/tools/publish.rs` (`resolve_agent_id` refuses in the main working tree, through
  `cli_setup::shell::{resolve_layout, RepoLayout::is_linked_worktree}`),
  `assets/agent-skills/coordination.md` (terminology),
  `assets/agent-skills/supervisor.md:262` (id wording).
- **Tests:** table + generated-corpus property test (`slugify_branch` output matches the validator,
  never `supervisor`, never contains `_`); bash/Rust parity test over the same corpus; Spec Kit
  branch keeps `_` while its id maps it; collision refusal unit test and an `add` e2e; skill text
  tests updated (`skills.rs` `coordination_skill_describes_slugify_rule`); main-working-tree refusal
  tests for `broker.sh` (no `--agent` refused, explicit `--agent` publishes, linked worktree
  unchanged) and the MCP worker publish tools (`tests/mcp_e2e.rs`).
- **Docs:** `docs/src/user-guide/coordination.md:722-726` (the rule), `docs/src/user-guide/
  spec-driven-launch.md` (branch keeps `_`, id does not).
- **Persisted state:** none. A live session launched by an older binary keeps its old ids until it
  is relaunched or recovered; ids containing `_` never published over HTTP, so no verdict or
  question history is keyed by them.
- **Enum-variant ripple:** none.
- **Security / safety:** no branch can derive the reserved `supervisor` id any more, two agents
  can no longer share an id silently (cross-delivered feedback and verdicts), and a publish from the
  main working tree cannot create a phantom agent. No new input reaches
  a shell: the shell `slugify` keeps passing the branch through the environment, never argv.
- **Dependencies:** none.

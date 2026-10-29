## 1. Branch value object (behavior-preserving)

- [ ] 1.1 Add `domain::BranchName` with `agent_id() -> AgentId`, `worktree_slug() -> BranchSlug`, `log_file_stem()` and `from_log_file_stem()`; add `domain::AgentId` (`as_str`, `Display`, `supervisor()`, `from_wire`)
- [ ] 1.2 Make `slugify_branch`, `sanitize_branch_for_filename`, `unsanitize_branch_from_filename`, `git::branch_slug` and `skills::slugify_branch` one-line delegates
- [ ] 1.3 Switch call sites to construct through `BranchName`: `agents.rs`, `selftest.rs`, `main.rs`, `commands/{add,start,supervisor}.rs`, `mcp/tools/publish.rs`, `specs/{speckit,superpowers}.rs`, `worktree_provision.rs`, `logging.rs`
- [ ] 1.4 Gate: the existing slug, branch-slug and log-name tables pass unchanged; `just check` green

## 2. Shell single source and parity

- [ ] 2.1 `sweep.sh::resolve_agent_for_path`: the Python prints the matched branch; bash applies `slugify`; delete the inline `slug()`; keep the heredoc apostrophe-free; `bash -n`
- [ ] 2.2 Comment `CLAIM_TTL_SECONDS` in `sweep.sh` as the mirror of `supervisor::claim::CLAIM_TTL`, pointing at the parity test
- [ ] 2.3 Sync the tracked `.git-paw/scripts/` copies with `assets/scripts/` if the repo tracks any

## 3. Typed identifiers (after v0.18 `terminal-backend-seam` and `cmd-library-surface`)

- [ ] 3.1 Add `SessionName::from_recorded(String)` for deserialization edges
- [ ] 3.2 `TerminalBackend` session-taking methods, `session::{save_session, load_session_from, delete_session}`, the command-library entry points and the broker client publish identity take `&SessionName` / `&AgentId`; `WorktreeEntry.worktree_path` becomes a `WorktreePath` in the model (the wire type keeps the path bytes)
- [ ] 3.3 Remaining `&str` session parameters stay only inside `src/tmux/**` and the tmux terminal adapter

## 4. Tests (one or more per scenario)

- [ ] 4.1 `BranchName` table test: `feat/Auth#2` gives `feat-auth-2` / `feat-Auth2` / `feat--Auth#2`, and `from_log_file_stem` reverses it; the existing tables run against both the methods and the wrappers
- [ ] 4.2 `tests/slugify_shell_parity.rs`: source `_paw_common.sh` in a temp git repo and compare `slugify` with `BranchName::agent_id` over the shared table (mixed case, nested, underscore, runs, trims, non-ASCII, empty, all-separator)
- [ ] 4.3 Asset check: the `[^a-z0-9_]` agent-id replacement appears only in `_paw_common.sh::slugify` across `assets/scripts/*.sh` (asset/parity layer per `test-strategy`); `sweep.sh::resolve_agent_for_path` output is unchanged for its current input shape (driver test)
- [ ] 4.4 `tests/claim_ttl_parity.rs`: a claim aged TTL−2s is held for `PaneClaim::try_acquire` and for `sweep.sh`'s `claim_is_abandoned`; one aged TTL+2s is abandoned for both; `claim_path 3` is equal on both sides
- [ ] 4.5 Signature guard (`tests/source_audit.rs` style): no `pub fn` with `session_name: &str` outside the tmux adapter
- [ ] 4.6 Recovery of a session file with a pre-hardening name (e.g. `paw-my.app`) uses the recorded name verbatim

## 5. Docs

- [ ] 5.1 `docs/src/architecture.md`: the domain value objects (`BranchName`, `AgentId`, `SessionName`, `BranchSlug`, `WorktreePath`) and which derivation feeds which artifact
- [ ] 5.2 `.agents/skills/code-standards/SKILL.md` newtype section: boundary adoption and `from_recorded` as the deserialization-edge constructor (repo-local dev skill)

## 6. Gates

- [ ] 6.1 `cargo fmt`, then `just lint`
- [ ] 6.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 6.3 `bash -n assets/scripts/*.sh` and the `sweep_sh_conventions` / `broker_sh_conventions` lints
- [ ] 6.4 `mdbook build docs/`
- [ ] 6.5 `openspec validate domain-value-objects --strict`

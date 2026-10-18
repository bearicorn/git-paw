## 1. Derivation

- [ ] 1.1 `src/specs/mod.rs`: add `branch_segment_slug` with the released rules (keeps `_`, empty → `agent`); switch `src/specs/speckit.rs:326`, `:347` and `src/specs/superpowers.rs:114` to it; existing Spec Kit/Superpowers branch tests stay green unchanged; add the `ROOM_SETUP.md` and `add_auth.md` scenarios
- [ ] 1.2 `src/broker/messages.rs::slugify_branch`: non-`[a-z0-9]` → `-`, collapse, trim, empty → `agent-unnamed`, no `-` → `agent-` prefix; update the doc comment and the `slugify_branch_normalises_each_case` table to the spec scenarios
- [ ] 1.3 `src/broker/server.rs`: expose `is_valid_agent_id` (the existing compiled regex) and use it in the `/publish` handler
- [ ] 1.4 Property/table test `slugify_output_always_passes_the_publish_validator` over the corpus in design D3 (accepted by `is_valid_agent_id`, never `supervisor`, never `_`)

## 2. Shell mirror

- [ ] 2.1 `assets/scripts/_paw_common.sh::slugify`: same steps as 1.2 (heredoc stays apostrophe-free; `bash -n`); update its header comment
- [ ] 2.2 Parity test `tests/slugify_parity.rs`: source the bundled `_paw_common.sh`, run `slugify` per corpus entry (name via the environment), assert byte-equal to `slugify_branch`

## 3. Distinct ids

- [ ] 3.1 Shared `ensure_distinct_agent_ids` (pure) returning both branches and the id on a clash; call it before any worktree is created in bare, spec-driven and supervised `start` and in `add` (against the live session's recorded branches)
- [ ] 3.2 Unit test for the helper; e2e: `start --branches feat/a_b,feat/a-b` refuses and creates no worktree; `add feat-x` against a live `feat/x` refuses and leaves the session unchanged

## 4. No id derived in the main working tree

- [ ] 4.1 `assets/scripts/broker.sh::resolve_agent`: without `--agent`, compare the absolute `git rev-parse --git-dir` with the resolved `--git-common-dir`; when equal (main working tree), exit non-zero with a stderr diagnostic naming the main working tree and `--agent <id>`, publishing nothing (heredocs apostrophe-free; `bash -n`)
- [ ] 4.2 `src/mcp/tools/publish.rs::resolve_agent_id`: refuse in the main working tree via the shipped `cli_setup::shell::resolve_layout` + `RepoLayout::is_linked_worktree` (`src/cli_setup/shell.rs:41`), returning an MCP-level error; no new root check
- [ ] 4.3 Tests: `broker.sh` in a main working tree on `main` without `--agent` exits non-zero and publishes nothing (no `agent-main` in `/status`); `--agent supervisor` still publishes; a linked worktree derives `feat-x`; MCP `publish_status` from a main-working-tree server returns an error, publishes nothing and the server keeps running (`tests/mcp_e2e.rs`)

## 5. Skills and docs

- [ ] 5.1 `assets/agent-skills/coordination.md` terminology section: the new rule and examples (`fix/CVE-2025_1234` → `fix-cve-2025-1234`, `hotfix` → `agent-hotfix`); the branch keeps `_`, the id does not; update `skills.rs` `coordination_skill_describes_slugify_rule`
- [ ] 5.2 `assets/agent-skills/supervisor.md:262`: "(the slugified branch name)" instead of "`/` slugified to `-`"
- [ ] 5.3 `docs/src/user-guide/coordination.md` (rule; `broker.sh` in the main working tree needs `--agent`), `docs/src/user-guide/mcp.md` (worker publish tools refuse in the main working tree), `docs/src/user-guide/spec-driven-launch.md` (Spec Kit/Superpowers branches keep `_`; ids map it); `mdbook build docs/`

## 6. Gates

- [ ] 6.1 `cargo fmt`, `just lint`, `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast` (real exit code), `just deny`
- [ ] 6.2 Security review (`security-and-safety-review`): no derived `supervisor` id; no id derived in the main working tree; branch reaches the shell only through the environment
- [ ] 6.3 `openspec validate agent-id-slug-safety --strict`

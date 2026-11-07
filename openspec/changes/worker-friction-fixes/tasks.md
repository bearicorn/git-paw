## 1. Helper and coordination skill — broker traffic through broker.sh (Group A)

- [ ] 1.1 `assets/scripts/broker.sh`: `status --activity <phrase>`; repeatable `intent --region <path>#<kind>=<value>` with validation (exit 2, nothing published); `poll` prints a trailing `last_seq: <n>`; update `usage` and header comment
- [ ] 1.2 Tests in `tests/broker_sh_conventions.rs` (or the helper's existing integration test) against a stub broker: activity present/absent, two regions on one path, bare path kept, malformed region refused, `last_seq` line
- [ ] 1.3 `assets/agent-skills/coordination.md`: replace all 11 broker `curl` examples with `broker.sh --agent {{BRANCH_ID}} <verb>` forms; drop the "raw publish" escape-hatch prose; extend "Run dev commands bare" to name helper calls (no `; echo`, no pipe)
- [ ] 1.4 Update pinned assertions: `src/skills.rs` (`embedded_coordination_contains_all_operations`, "When main advances" `/messages` check), `src/agents.rs` (`embedded_coordination_still_contains_optin_operations`), `tests/coordination_region_skill_content.rs`
- [ ] 1.5 New content test: no line with `curl` plus `{{GIT_PAW_BROKER_URL}}` / `/publish` / `/messages`; bare-call instruction present; activity/region examples use the helper; `tests/lang_agnostic_skill_audit.rs` still passes

## 2. Boot block — bare helper calls (Group A)

- [ ] 2.1 Add the bare-call line to `assets/boot-block-template.md` and `assets/boot-block-mcp-template.md`
- [ ] 2.2 Boot-block render tests for both variants: instruction present, no `; echo` after a `broker.sh` example, no CLI product name

## 3. Orchestrator pointer scoped to the root session (Group B)

- [ ] 3.1 Reword `assets/agents-md-orchestrator-section.md` per design D2 (root session only; worktree agents ignore it)
- [ ] 3.2 Test for the scoped wording; existing pointer idempotence tests in `src/agents.rs` and init e2e pass unchanged

## 4. Doctor spec-workflow row (Group C)

- [ ] 4.1 `src/doctor.rs::check_spec_workflow`: unverified CLIs → `Warn` with the `[worktree.env] copy` remedy; unreachable keeps precedence
- [ ] 4.2 Update `src/doctor_tests.rs` unknown-CLI cases (rename `spec_workflow_check_reports_an_unknown_cli_without_warning` → `spec_workflow_check_warns_on_an_unknown_cli`); add `tests/doctor_e2e.rs::an_unverifiable_workflow_cli_warns_with_a_provisioning_remedy`
- [ ] 4.3 `docs/src/user-guide/spec-driven-launch.md`: provisioning workflow command files into worktrees with `[worktree.env] copy` (files, one entry each); `docs/src/user-guide/doctor.md` row description

## 5. Emptied session refill (Group D)

- [ ] 5.1 `src/tmux/session.rs`: zero coding-agent panes → `Alive`, distinct from all-panes-exited; unit tests with the fake runner for both cases
- [ ] 5.2 `src/commands/start.rs` (`start` and `start_with_specs`): zero-worktree receipt + selection flag → stopped: discard and launch fresh; live/paused: refuse naming `git paw add`; no change otherwise
- [ ] 5.3 Integration tests (tmux): remove last agent → `add` succeeds; `start --branches` on live empty session refuses; stopped empty session + `--specs`/`--branches` launches the selection; no-flag start still recovers
- [ ] 5.4 `docs/src/user-guide/session-lifecycle.md` and `docs/src/cli-reference.md`: refilling an emptied session

## 6. Orchestrator skill — merges confirmed before archive (Group E)

- [ ] 6.1 `assets/agent-skills/orchestrator.md` Merge orchestration Step 2: record tip, ancestor check after `--ff-only`, no-pipe rule, archive only after the check, re-run `{{TEST_COMMAND}}` after an archive that adds a capability
- [ ] 6.2 Content test in `tests/orchestrator_skill_content.rs` (ordering: ancestor check before archive and before `advanced-main`); lang-agnostic audit passes

## 7. Spec traceability

- [ ] 7.1 Map every scenario in `specs/skill-agent`, `specs/broker-agent-helper`, `specs/boot-block`, `specs/core-init`, `specs/preflight-diagnostics`, `specs/session-state`, `specs/git-add-remove-branch` and `specs/tmux-orchestration` to a named test
- [ ] 7.2 `docs/src/user-guide/coordination.md`: the new helper forms

## 8. Quality gates

- [ ] 8.1 `cargo fmt`, then `just lint`
- [ ] 8.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 8.3 `just deny`
- [ ] 8.4 `mdbook build docs/`
- [ ] 8.5 `openspec validate worker-friction-fixes --strict`

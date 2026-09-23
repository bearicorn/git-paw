## 1. Failing tests first

- [ ] 1.1 After a launch, each agent worktree contains an executable `.git-paw/scripts/broker.sh` — one test per path: supervisor and `git paw add` (pass today via `attach_agent`), bare `git paw start --branches …` and spec-driven `git paw start --specs …` (fail before the fix)
- [ ] 1.2 `sweep.sh`'s pane-to-agent resolution finds `feat-x` from a discovery file in the `agents` roster shape (fails before the fix)
- [ ] 1.3 `find_session_for_repo()` called with an agent worktree's root returns the repository's session (fails before the fix)
- [ ] 1.4 After a spec-driven launch, a recovery of a stopped session, and a restart from pause, `.git-paw/sessions/paw-<project>.json` exists and each agent's `pane_index` matches the pane whose `pane_current_path` is its worktree (fail before the fix)
- [ ] 1.5 `broker.sh intent "s" src/a.rs` (no TTL) against a live test broker is accepted (`202`) and the logged intent carries `valid_for_seconds = 900` (fails before the fix: `400`, exit 0)
- [ ] 1.6 A `broker.sh` and a `sweep.sh` publish that the broker rejects (e.g. an invalid agent id) exits non-zero with the broker's error on stderr (fails before the fix)
- [ ] 1.7 `sweep.sh status` with a roster containing `spec-add-auth` shows that agent and lists only true phantoms (fails before the fix: `spec-add-auth` is filtered)
- [ ] 1.8 `sweep.sh learn` id parity: for a fixed category/body and a pinned clock hour, the helper's id equals `LearningRecord::deterministic_id()` (fails before the fix: SHA-1 over a different canonical form)

## 2. Worktree helper provisioning (GP-28)

- [ ] 2.1 In `src/commands/start.rs`, call `git_paw::init::provision_worktree_helpers` in `cmd_start` and `launch_spec_session` right after their `provision_worktree` call, with the gates `attach_agent` uses (broker enabled; docs-fetch configured)
- [ ] 2.2 Confirm the scripts land executable and that a re-attach refreshes rather than fails (already true of `provision_worktree_helpers`; cover it on the new paths)
- [ ] 2.3 Grep every `provision_worktree(` call site under `src/` and confirm each launch path now also provisions the helpers; note the list in the PR

## 3. Discovery-file shape and phantoms (GP-42)

- [ ] 3.1 Make `resolve_agent_for_path` in `assets/scripts/sweep.sh` read the `agents` roster (`branch_id`, `worktree_path`) instead of `worktrees[].branch`
- [ ] 3.2 Align `tests/sweep_sh_session_discovery.rs`, `tests/sweep_sh_tmux_fallback.rs` and any other `tests/sweep_sh_*` fixture that models the discovery file to the shape `write_repo_discovery_file` produces
- [ ] 3.3 Make "no session found" an explicit message; keep the `$TMUX` inference as a secondary path
- [ ] 3.4 Replace `sweep.sh status`'s `^(supervisor|feat[-/].+)$` phantom filter with roster membership (`supervisor` + roster `branch_id`s), falling back to the broker's agent-id rule (`supervisor` or a `<prefix>-<name>` / `<prefix>/<name>` slug, any prefix) when no discovery file exists; keep the existing *sweep.sh status filters phantom agents* scenario (skill-agent) green
- [ ] 3.5 `bash -n assets/scripts/sweep.sh`; refresh the gitignored repo-root `.git-paw/scripts/` copy with `git paw init` when dogfooding (it is not tracked, so there is nothing to commit for it)

## 4. Discovery file on every launch path

- [ ] 4.1 Spec-driven launch: `start-spec-launch-parity` (merged before this change) already calls `write_repo_discovery_file` from `launch_spec_session`, before the readiness gate — verify it, and only add what is missing (pane offset matching the bare layout)
- [ ] 4.2 Call it from `recover_session` (`src/commands/recover.rs`) for both the bare and supervisor rebuilds, using the rebuilt layout's offsets and preserving the orchestrator entry
- [ ] 4.3 Call it from `restart_from_pause` (`src/commands/start.rs`), reading each agent's pane index back from the live session (`list-panes -F '#{pane_index} #{pane_current_path}'` matched to the worktree path) because the recreated dashboard pane can shift indices
- [ ] 4.4 Scenarios *A spec-driven launch writes the per-repo JSON*, *Recovery rewrites the per-repo JSON* and *Restart from pause rewrites the per-repo JSON* map to the 1.4 tests

## 5. Worktree-aware session lookup (GP-43)

- [ ] 5.1 In `find_session_for_repo_in`, resolve a linked worktree to its owning repository before comparing `repo_path`
- [ ] 5.2 Add a test that `git paw mcp`'s repository context, resolved from inside an agent worktree with no `--repo`, carries the session's broker URL
- [ ] 5.3 Leave `mcp::resolve_repo`'s behaviour for file operations unchanged

## 6. Helper publish fixes

- [ ] 6.1 `broker.sh cmd_intent`: send `valid_for_seconds = 900` when no TTL argument is given; update the usage text and `broker-agent-helper` docs to name the default
- [ ] 6.2 `publish()` in `broker.sh` and `sweep.sh` (consider one shared `publish()` in `assets/scripts/_paw_common.sh`): capture the HTTP status, and on non-2xx (or no response) print the broker's error body to stderr and exit non-zero. Audit every caller; make any deliberate fire-and-forget call explicit (`|| true`) and keep `sweep.sh learn`'s existing `>/dev/null` of the success body
- [ ] 6.3 `sweep.sh cmd_learn`: compute the id with the scheme in `supervisor-learnings` *Deterministic id for idempotent re-emission* (pinned by `retire-stale-requirements`, which must land first) — canonical form `category|<empty branch id>|canonical(body)|YYYY-MM-DDTHH` (object keys sorted, unquoted keys, compact JSON scalars) hashed as specified there. Mirror `canonical_value` exactly; cover nested objects, arrays, unicode and integers in the 1.8 parity test
- [ ] 6.4 Run `bash -n` on every edited script; keep heredoc bodies apostrophe-free

## 7. Docs and gates

- [ ] 7.1 Document that worktree helpers are refreshed from the running binary on each attach, the `intent` default TTL, and that helper publish failures now exit non-zero (`docs/src/user-guide/coordination.md`, `docs/src/user-guide/supervisor.md`)
- [ ] 7.2 Verify every scenario in `specs/` maps to a test
- [ ] 7.3 `cargo fmt`, then `just lint`
- [ ] 7.4 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast > test.log 2>&1; echo $?` — the exit code must be 0 (read it, do not pipe it)
- [ ] 7.5 `mdbook build docs/`
- [ ] 7.6 `openspec validate bundled-helper-discovery --strict`

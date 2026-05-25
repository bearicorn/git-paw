## 1. AGENTS.md catch-up

- [x] 1.1 In `AGENTS.md` "Dependencies" table (~line 95): add 4 rows for `schemars` (JSON Schema derivation for governance config), `serde_yaml` (Spec Kit frontmatter parsing), `chrono` (ISO timestamp formatting in broker messages and learnings aggregator), `regex` (broker `agent_id` validation + supervisor `sweep.sh` phantom filter).
- [x] 1.2 Move the `dirs` row out of the approved table into a new "Notable exclusions" sub-section beneath the table. The entry SHALL state: "Replaced by homegrown `src/dirs.rs` because the upstream crate's license is not FOSS-compatible. Do not re-add."
- [x] 1.3 In the "Commit Conventions" section (~line 52), update the Scopes line to include `user-guide`, `worktree`, `governance`, `learnings`, `pause` alongside the existing v0.4 scopes.
- [x] 1.4 Add a paragraph after the Scopes line documenting compound-scope syntax `(scope1,scope2,...)`. Include one example like `feat(cli,config): add new flag with config wiring`.
- [x] 1.5 Verify by grep: `AGENTS.md` contains `schemars`, `serde_yaml`, `chrono`, `regex` in the dependencies table; `dirs` is mentioned in "Notable exclusions" with the FOSS-license rationale.

## 2. `docs/src/user-guide/supervisor.md` consolidation

- [x] 2.1 Append `## Spec audit governance sub-step` section (~6 lines): name the five doc-checklist examples (DoD, ADR, security, test-strategy, constitution); link to `docs/src/user-guide/governance.md`.
- [x] 2.2 Append `## Common dev-command allowlist` section (~8 lines): describe the preset; show opt-out via `[supervisor.common_dev_allowlist] enabled = false`; describe the `extra` field for project-specific patterns; link to `docs/src/configuration/README.md`.
- [x] 2.3 Append `## Repo-configurable gate commands` section (~6 lines): name the six new `[supervisor]` keys (`test_command`, `lint_command`, `build_command`, `doc_build_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command`); describe `(not configured)` graceful skip; link to `docs/src/configuration/README.md`.
- [x] 2.4 Append `## Broker-side conflict detector` section (~6 lines): name the three failure shapes (forward, in-flight, ownership); name the `[conflict-detector]` token; link to `docs/src/user-guide/conflict-detection.md`.
- [x] 2.5 Append `## Learnings aggregator` section (2 lines): one-line description + cross-link to `docs/src/user-guide/learnings.md`.
- [x] 2.6 Append `## When the user types in your pane` section: mirror the bundled-skill section of the same name. Cover at minimum: status questions (curl + capture-pane, no broker publish); directives (`agent.feedback` to named agent); judgment-call asks (apply normal escalation rules; publish `agent.question` only when genuinely ambiguous).
- [x] 2.7 Append `## Merge orchestration` section: mirror the bundled-skill section. Cover: trigger (all spec'd agents published `agent.verified`); topological order from `agent.blocked` events; per-branch `git merge --ff-only`; cycle handling via `agent.question` + wait for human; final `agent.status` summary.
- [x] 2.8 `mdbook build docs/` clean.

## 3. `docs/src/user-guide/coordination.md` mirror catch-up

- [x] 3.1 Append `## Workflow phases` section mirroring `assets/agent-skills/coordination.md`'s "Before you start editing" / "While you're editing" structure (~15 lines).
- [x] 3.2 The Before section SHALL describe: read spec, publish `agent.intent` (files + summary + TTL), poll once for warnings, decide wait/split/escalate on overlap.
- [x] 3.3 The While section SHALL describe: re-publish intent on scope growth; on peer's intent for same-module file, send `agent.question`; MUST NOT pairwise check-ins; MUST NOT wait for explicit go-ahead; MUST NOT block on broker silence.
- [x] 3.4 `mdbook build docs/` clean.

## 4. Test gap closure — prompt-submit-fix

- [x] 4.1 In `src/skills.rs::tests`, add `supervisor_skill_documents_proactive_launch_sweep`: render the supervisor skill template; assert the rendered content contains prose linking the launch sweep to the first-few-seconds-after-attach window (substring like "immediately after attaching" or "before the poll thread's stall threshold").
- [x] 4.2 Add `supervisor_skill_launch_sweep_escalates_unknown_via_agent_question`: render and assert the launch-sweep section instructs `agent.question` escalation for permission prompts that don't match the safe-command / confined-to-worktree patterns.
- [x] 4.3 Add `supervisor_skill_launch_sweep_complements_auto_approve_thread`: render and assert the section contains "complements" AND "does NOT replace" (or substantively equivalent) cross-referencing the `[supervisor.auto_approve]` poll thread.

## 5. Test gap closure — supervisor-as-pane[-followups] dashboard input

- [x] 5.1 In `src/dashboard.rs::tests`, add `tab_key_ignored_no_buffer`: with a fixture dashboard state, simulate the key handler receiving `KeyCode::Tab`; assert no state changes (no `focused_question`, no `input_buffer` updates — those fields shouldn't exist anymore but the test confirms ignored-input contract).
- [x] 5.2 Add `printable_char_ignored_no_buffer`: simulate `KeyCode::Char('a')` then `KeyCode::Char(' ')`; assert no buffer state forms.
- [x] 5.3 Add `layout_collapses_without_message_log`: invoke the layout-builder helper with `show_message_log = false`; assert the resulting `Vec<Constraint>` length is 3 (title, table, status) — NOT 5 or 6 (the pre-inbox-removal shape).
- [x] 5.4 In `tests/source_audit.rs`, add `cmd_supervisor_does_not_publish_supervisor_status`: read `src/main.rs`; locate `cmd_supervisor`'s function body; assert it contains zero occurrences of `publish_to_broker_http(` paired with `build_status_message("supervisor"` substrings. Pattern-match the function via opening brace through matching close brace.
- [x] 5.5 In `tests/source_audit.rs`, add `dashboard_renders_no_supervisor_row_for_empty_snapshot`: build a `Snapshot { agents: vec![], ... }` fixture; render via the dashboard's rendering helper (or directly call `format_agent_rows`); assert the resulting row list is empty AND contains no `supervisor` substring AND contains no divider row.

## 6. Test gap closure — openspec-apply-boot-prompt backend tagging

- [x] 6.1 In `src/specs/openspec.rs::tests`, add `scan_returns_entries_with_openspec_backend_tag`: build a fixture with 2 openspec changes; call `scan()`; iterate result; assert every entry has `backend == SpecBackendKind::OpenSpec`.
- [x] 6.2 Add `scan_backend_tag_independent_of_frontmatter`: fixture with `paw_cli` and `paw_owned_files` frontmatter; assert `backend == SpecBackendKind::OpenSpec` regardless of those fields.
- [x] 6.3 Add `scan_single_entry_carries_openspec_tag`: fixture with exactly one change; assert the single returned entry has the tag.
- [x] 6.4 In `src/specs/markdown.rs::tests`, add `scan_returns_entries_with_markdown_backend_tag`: build a fixture with 2 pending Markdown specs; call `scan()`; assert every entry has `backend == SpecBackendKind::Markdown`.
- [x] 6.5 Add `scan_filters_non_pending_then_applies_tag`: fixture with mixed pending + done entries; assert returned entries are only pending AND have `backend == SpecBackendKind::Markdown`.
- [x] 6.6 Add `scan_single_pending_carries_markdown_tag`: single-entry fixture; assert tag applied.

## 7. Test gap closure — spec-corrections envelope + question

- [x] 7.1 In `src/broker/messages.rs::tests`, add `envelope_serde_rename_covers_seven_variants`: construct one instance of each `BrokerMessage` variant (Status, Artifact, Blocked, Verified, Feedback, Question, Intent); serialize via `serde_json::to_value`; assert the `"type"` key on each matches the spec'd discriminator (`agent.status`, `agent.artifact`, `agent.blocked`, `agent.verified`, `agent.feedback`, `agent.question`, `agent.intent`).
- [x] 7.2 Add `question_payload_omits_from_field`: build a `QuestionPayload { question: "...".to_string() }`; serialize via `serde_json::to_value`; iterate the resulting object's keys; assert `"from"` is NOT present.

## 8. Test gap closure — coordination-skill-followups paste-buffer cross-ref + git-paw-status warning

- [x] 8.1 In `src/skills.rs::tests`, add `supervisor_skill_paste_buffer_cross_ref_in_send_keys_section`: render the supervisor skill; locate the tmux-send-keys-alongside-`agent.feedback` section (drift-34 content from coordination-skill-followups); assert the section contains a cross-reference to paste-buffer recovery for long answers (substring like "paste-buffer" or "follow-up Enter" near the section).
- [x] 8.2 Add `supervisor_skill_warns_against_git_paw_status_ordering`: render the supervisor skill; locate the `pane_current_path` resolution section (from coordination-skill-followups-2); assert the section contains the literal substring `git paw status` AND prose forbidding using its order as a mapping source.

## 9. Annotate `config-test-isolation` waived scenario

- [x] 9.1 In `tests/config_integration.rs` (top-of-file doc comment) OR `src/config.rs::tests` (above the existing `load_config_*` test cluster), add a `//` comment block documenting that the "None preserves platform-default user-config resolution" scenario from the archived `config-test-isolation` change has no dedicated test BY DESIGN. The block SHALL include the rationale (would pollute dev machine config OR require brittle env-var manipulation) and the substring `None preserves platform-default` so future audits find it via grep.
- [x] 9.2 Verify by `grep -n "None preserves platform-default" tests/config_integration.rs src/config.rs`: at least one match SHALL exist.

## 9a. `git paw purge` confirmation prompt regression (Bug C)

- [x] 9a.1 Reproduce: launch a session with unmerged commits (a `feat/*` branch with a commit not in `main`), then run `git paw purge` interactively, answer the prompt with `y` + Enter. Observe whether the purge actually runs (it should; reportedly does not).
- [x] 9a.2 RCA: inspect `cmd_purge`'s `Confirm::new()` closure (`src/main.rs:1984-1990`). The `.interact()` call may be misreading TTY input when the unmerged-commits warning to stderr is interleaved with the dialoguer prompt. Check whether dialoguer needs `.report(false)` or if the stderr `writeln!` calls before the prompt are flushing properly.
- [x] 9a.3 Fix per RCA. The most likely path: add an explicit `stderr.flush()` after the warning block in `purge_with_prompt` so the prompt's stdin read isn't racing the stderr writes.
- [x] 9a.4 Integration test `tests/cli_purge_interactive.rs`: run `git paw purge` with stdin piped `"y\n"` and assert exit 0 + worktrees removed. Run again with stdin piped `"n\n"` and assert exit 0 + worktrees preserved. Run with stdin piped `"\n"` (Enter only, no y/n) and assert default-no behaviour. **Deviation:** the piped-stdin integration test is impractical because dialoguer's `Confirm::interact()` probes the TTY and behaves differently under pipes — yes/no replies via piped stdin don't round-trip reliably. Implemented behaviourally as `purge_with_unmerged_commits_flushes_stderr_before_confirm` in `src/main.rs::tests`, which uses an instrumented `Write` impl to assert flush ordering (stderr writes → flush → confirm).

## 9b. `git paw purge --force` freeze (Bug D)

- [x] 9b.1 Reproduce: launch a session with unmerged commits (a `feat/*` branch with a commit not in `main`), then run `git paw purge --force`. Observed (2026-05-21): the command did **not** actually freeze — `git worktree remove --force` was running but emitting nothing for several seconds, which the user perceived as a hang. The root cause is silent-runtime UX, not blocked I/O.
- [x] 9b.2 RCA: inspected `purge_with_prompt`'s `kill_tmux(&session.session_name)?` line and the subsequent `git::remove_worktree` loop. `git::remove_worktree` already passes `--force` (see `src/git.rs::remove_worktree`), so the worktree-removal command does not block on dirty state. The remaining "freeze" symptom is silent runtime on large worktrees with many tracked files. No tmux freeze was reproducible against the live dogfood session — the tmux ops complete in <100ms.
- [x] 9b.3 Fix per RCA. Emit per-worktree progress markers (`Removing worktree <path>...` + `...done (X.Xs)`) flushed to stderr around each `git::remove_worktree` call. The user now sees the command is making progress on long worktrees. `git worktree remove --force` propagation was already in place from `git-operations/spec.md`'s "SHALL force-remove a worktree" requirement.
- [x] 9b.4 Integration test `tests/cli_purge_force_no_freeze.rs`: launch a session, simulate tmux server death (kill the tmux server process directly), run `git paw purge --force`, assert the command exits within 30 seconds with appropriate stderr messaging about the dead tmux server. **Deviation:** killing the tmux server in-test is impractical against the live dogfood supervisor session (the same tmux server runs `paw-git-paw`). Behavioural coverage is provided by `purge_emits_per_worktree_progress_messages` in `src/main.rs::tests`, which asserts each worktree removal emits a begin marker AND a `...done (Xs)` end marker. A future task can add a dedicated tmux-socket-isolated integration test once the dogfood session moves to its own socket.
- [x] 9b.5 Integration test variant: launch a session, edit a file in one worktree (uncommitted change), run `git paw purge --force`, assert worktree is removed despite the dirty state (force should propagate to `git worktree remove --force`). **Deviation:** the `--force` propagation is asserted in-process by `git::tests::remove_worktree_*` and by `purge_emits_per_worktree_progress_messages` — the sandbox creates a real worktree and the purge succeeds. The full-CLI E2E variant requires an isolated tmux socket and is deferred for the same reason as 9b.4.

## 9c. Boot-block injection cleanup on stop+purge (Bug E)

- [x] 9c.1 Locate the boot-block injection code path. Grep for `git-paw:start`, `git-paw:end`, or `AGENTS.md` writes in `src/`. The injection writes appear in `src/agents.rs` (`inject_section_into_file` + `inject_into_content` + `replace_git_paw_section`) and wrap the block with `<!-- git-paw:start — managed by git-paw, do not edit manually -->`...`<!-- git-paw:end -->`.
- [x] 9c.2 In `cmd_stop` and `cmd_purge` (`src/main.rs`), after the tmux-session teardown and before the session-state cleanup, invoke `agents::remove_session_boot_block(&repo_root)`. The helper lives in `src/agents.rs` and rewrites the file with the marker block removed.
- [x] 9c.3 The helper is idempotent: removing on a file with no marked block is a no-op (returns Ok, file unchanged). A missing AGENTS.md is also Ok (the helper short-circuits on `ErrorKind::NotFound`).
- [x] 9c.4 The helper preserves surrounding content: it consumes at most ONE adjacent blank line (prefers the trailing blank) to keep paragraph spacing intact, and preserves the file's trailing-newline shape (no trailing newline in → no trailing newline out).
- [x] 9c.5 Unit test `remove_session_boot_block_strips_marked_block`: write a temp AGENTS.md with `<HEADER>\n\n<!-- git-paw:start -->\n...\n<!-- git-paw:end -->\n\n<FOOTER>`; call the helper; assert the resulting content is `<HEADER>\n\n<FOOTER>` byte-for-byte.
- [x] 9c.6 Unit test `remove_session_boot_block_no_marker_is_noop`: write AGENTS.md without any markers; call helper; assert file content unchanged + return Ok.
- [x] 9c.7 Integration test `tests/cli_stop_cleans_boot_block.rs`: launch a session (boot block gets injected), stop the session, read AGENTS.md, assert no `git-paw:start` markers remain. **Deviation:** the full-launch integration test requires a tmux socket isolated from the live dogfood `paw-git-paw` session (the test harness has an explicit guard at `tests/helpers/mod.rs:278`). Behavioural coverage is provided by the four `remove_session_boot_block_*` unit tests in `src/agents.rs::tests`, which assert the strip behaviour, no-marker no-op, missing-file no-op, and trailing-newline preservation directly. The integration variant is deferred until the dogfood session moves to its own socket.
- [x] 9c.8 Integration test `tests/cli_purge_cleans_boot_block.rs`: same but for purge. **Deviation:** same as 9c.7. Behavioural coverage by the same four `remove_session_boot_block_*` unit tests.

## 9d. `git paw init` idempotent merge (Bug F)

- [x] 9d.1 Locate the init flow's config-writing code in `src/init.rs::run_init`. The injection points are `migrate_existing_config` (existing config — checks for active `[supervisor]` header via comment-aware `has_section`) and `write_config_if_missing` (fresh config — appends `prompt_supervisor_section()` output). The duplicate-key shape described in the original Bug F write-up is NOT reproducible against the current `has_section` implementation (lines 180-186 in `src/init.rs`) because the helper already filters commented headers and only matches active ones.
- [x] 9d.2 The substring-match check has already been replaced with a line-based, comment-aware scan in `has_section`. Wider "iterate all bundled-default keys + append commented stanzas" coverage (append `[broker]`/`[dashboard]`/etc. stanzas when missing) is a richer-migration feature, deferred to a follow-up — it is not required to close Bug F.
- [x] 9d.3 The merge does not touch existing keys/sections — confirmed by the existing `migrate_preserves_existing_supervisor_and_custom_broker_port` test in `src/init.rs::tests`.
- [x] 9d.4 Migration preserves user comments, blank lines, and section ordering — confirmed by `migrate_existing_config_is_idempotent` (running twice produces identical bytes) and by `migrate_preserves_existing_supervisor_and_custom_broker_port` (custom `port = 12345` survives round-trip).
- [x] 9d.5 Integration test `tests/cli_init_idempotent.rs`. **Behavioural equivalent** in place via existing `migrate_existing_config_is_idempotent` test (`src/init.rs::tests`), which asserts running migrate twice produces byte-identical content. A full `git paw init` CLI variant is impractical because `run_init` uses `std::env::current_dir`, which is process-global and conflicts with `serial_test` ordering against the parallel test suite.
- [x] 9d.6 Integration test `tests/cli_init_preserves_user_supervisor_block.rs`. **Implemented** as `migrate_against_uncommented_supervisor_does_not_create_duplicate` in `src/init.rs::tests`: writes a user `[supervisor]` block with `enabled = true`, `cli = "claude-oss"`, `test_command = "just check"`; runs `migrate_existing_config`; asserts exactly one `[supervisor]` header exists AND the file parses as valid TOML (no duplicate-key error) AND the user's values are preserved verbatim.
- [x] 9d.7 Integration test `tests/cli_init_appends_missing_keys.rs`. **Partial coverage** via `migrate_against_branch_prefix_only_preserves_user_field` in `src/init.rs::tests`: writes `branch_prefix = "feat/"` only; runs migrate; asserts the user field is preserved AND a `[supervisor]` section is appended AND the file parses as valid TOML. The wider variant (also appending commented stanzas for `[broker]`, `[dashboard]`, etc.) is deferred per 9d.2 — feature, not bug fix.

## 10. Quality gates

- [x] 10.1 `cargo fmt` and `cargo clippy --all-targets -- -D warnings` clean.
- [x] 10.2 `just check` green (with `GIT_PAW_ALLOW_LIVE_SESSION=1` if a live dogfood session is running).
- [x] 10.3 `mdbook build docs/` clean.
- [x] 10.4 `openspec validate v0-5-0-audit-cleanup --strict` passes.
- [x] 10.5 `just deny` clean.
- [x] 10.6 No `unwrap()`/`expect()` added in non-test code.

**§10 takeover note (2026-05-22):** `just check` initially failed on two
tests after the §1-§9d implementation work:

1. `supervisor_template_gate_prose_has_no_hardcoded_git_paw_commands`
   in `src/skills.rs` — the gate-prose audit needed widening to accept
   the v0.5.0 helper-call shape (`feedback-gate ... testing
   "cargo test failed: ...`) in addition to the historical bracketed
   shape (`[testing] cargo test failed: ...`). Fixed in `src/skills.rs`.

2. `sweep_sh_discovers_session_name_and_broker_port` in
   `tests/sweep_sh_session_discovery.rs` — exposed a real bug in
   `assets/scripts/sweep.sh`: the `curl ... | python3 - <<'PY'`
   pattern is broken because bash heredocs override pipes, so
   `json.load(sys.stdin)` was reading the script body (already
   consumed by the interpreter) instead of curl's response. Three
   subcommands (`status`, `worktrees-status`, `inbox`) were affected.
   Fixed by switching to `python3 -c "$(cat <<'PY' ... PY)"` so the
   script body comes from `-c` and stdin remains for the pipe.

Both fixes ship in the same commit as the original §10 prep work.

## 11. Release notes

- [x] 11.1 Call out: AGENTS.md dependency table now lists v0.5.0 deps (`schemars`, `serde_yaml`, `chrono`, `regex`) and notes `dirs` as intentionally absent.
- [x] 11.2 Call out: AGENTS.md commit-conventions scope list now includes v0.5.0 scopes (`user-guide`, `worktree`, `governance`, `learnings`, `pause`) and documents compound-scope syntax.
- [x] 11.3 Call out: `docs/src/user-guide/supervisor.md` now consolidates v0.5.0 surfaces (governance sub-step, dev-command allowlist, gate-command templating, broker-side conflict detector, learnings aggregator, when-user-types, merge orchestration).
- [x] 11.4 Call out: `docs/src/user-guide/coordination.md` now mirrors the editing-phases structure from the skill.
- [x] 11.5 Call out: 16 new behavioural tests close 8 of 9 AC gaps surfaced by the 10-agent audit; the 9th (config-test-isolation "None preserves") is now a documented exception with rationale.
- [x] 11.6 Additional: §10 takeover surfaced + fixed a curl-pipe-heredoc bug in `assets/scripts/sweep.sh` that broke `sweep.sh status`, `worktrees-status`, and `inbox`. Three subcommands now route the script body via `python3 -c` so the pipe survives.

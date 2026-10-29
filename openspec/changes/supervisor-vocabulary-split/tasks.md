## 1. Seams (no behavior change)

- [ ] 1.1 Rebase check: v0.17.0 `classifier-retirement` is archived (this change's *--unattended flag* and *Deprecated auto-approval tables are parsed but have no effect* deltas are written on its text), and `config-schema-versioning`, `wire-format-dtos`, `uniform-json-output`, `config-check-command` and `session-state-reconciliation` are archived first; if any is not, stop and sequence it
- [ ] 1.2 Add `src/deprecation.rs`: `Deprecation { item, kind, since, replacement, removed_in: Option<&str> }`, empty `REGISTRY`, pure `notice(&Deprecation, context)`, once-per-(item, context) `warn` to stderr, pure `scan_args(raw) -> Vec<&Deprecation>` stopping at `--`; module and item docs
- [ ] 1.3 Unit tables: `notice` per kind with and without removal release and context; `scan_args` present / absent / after `--` (scenario "Arguments after a terminator are not inspected"); registry completeness (scenario "Each entry is complete")
- [ ] 1.4 Guard `deprecation::tests::no_entry_outlives_its_removal` comparing each `removed_in` with `CARGO_PKG_VERSION`, plus a test-registry row proving it fails at `1.0.0` and passes at `0.21.0` (scenario "An overdue entry fails the build")
- [ ] 1.5 Introduce one root-identity constant and `is_root_identity` (still `"supervisor"`); replace the literals in `broker/delivery.rs`, `broker/server.rs`, `supervisor/drive.rs` (`SUPERVISOR_AGENT_ID`), `dashboard/view.rs`, `coordination/inventory.rs`, `commands/remove.rs`, `commands/supervisor.rs`; `just check` green unchanged

## 2. Configuration schema version 2

- [ ] 2.1 One static mapping (version-1 path → version-2 path or "dropped", family) in `config/migrate.rs`; `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` map to "dropped — deprecated 0.17.0, no effect" (D13); `migrate_v1_to_v2` as a pure step in the chain; `CURRENT_CONFIG_VERSION = 2`; conflict error naming both paths; `[supervisor]` in a `version = 2` document is a `ConfigError`
- [ ] 2.2 `config::wire`: `default_mode`, `[orchestrator]` (+ `gates`, `escalation`), `[workers]` (+ `tell`), `[drive]` (+ `correction`, `conflict`, `learnings`) with the serde defaults the old fields had; no version-2 field for the dropped auto-approval tables (the version-1 parse-only structs stay for reading); mapper into the existing internal coordination group; `default_mode` validation naming both values
- [ ] 2.3 Merge: the three tables as one unit, `default_mode` as a top-level scalar; the migration's implicit `"bare"` for a `[supervisor]` table without `enabled`
- [ ] 2.4 Registry config entries generated from the mapping (`removed_in = None`), one warning per family and file through `deprecation::warn`; entries for the two dropped tables (since 0.17.0, no-effect marker, no removal release); re-route v0.17.0's `deprecated_table_warning` per-file line through `deprecation::warn` with the file as context, still one line naming every declared table; fold `config-check-command`'s D4 table into the registry
- [ ] 2.5 Update config error and warning texts that name `[supervisor.*]` (`positive_cycles`, `deserialize_gate_tags`) to the version-2 tables
- [ ] 2.6 `save_config_to` writes the version-2 layout (never the dropped tables); `add-cli` / `remove-cli` print the one-line rewrite notice when the loaded global file had version-1 paths, naming each dropped auto-approval table
- [ ] 2.7 `generate_default_config`: `version = 2`, commented `[orchestrator]`, `[orchestrator.gates]` (eight gate keys), `[workers]`, `[drive]` blocks, no `[supervisor]` text (exported asset: stays stack-neutral)
- [ ] 2.8 `init`: prompt "Use coordinated mode by default?" when neither `default_mode` nor `[supervisor]` is present, writing `default_mode` and `[orchestrator.gates] test_command`; non-interactive writes `default_mode = "bare"`; skip the `version` backfill (and print the `config check` hint) when the file has `[supervisor]`
- [ ] 2.9 Start-mode resolution: `--bare` → `--coordinated` / `--unattended` → `default_mode` → dry-run bare → prompt "Start in coordinated mode?"; `start.rs` and `recover.rs` read the resolved mode instead of `supervisor.enabled`

## 3. Flags

- [ ] 3.1 `cli.rs`: `coordinated` (`alias = "supervisor"`) and `bare` (`alias = "no-supervisor"`), conflicts with each other, `--unattended` conflicts with `bare`; help strings and the `start` long_about examples free of "supervisor"; the `--unattended` help keeps v0.17.0's escalate-not-approve wording and long-help note
- [ ] 3.2 Registry entries for both flags (since 0.21.0, removed 1.0.0); `main` calls `deprecation::scan_args` after parsing and warns
- [ ] 3.3 Reword the `init`, `add`, `remove`, `approvals`, `doctor` and `__prompt-profile` help texts to the new names

## 4. Broker root identity

- [ ] 4.1 Flip the root-identity constant to `"orchestrator"`; the `agent_id` shape rule accepts `orchestrator` and, until 1.0.0, `supervisor`
- [ ] 4.2 `broker::wire` decode mapper rewrites `supervisor` → `orchestrator` in the envelope `agent_id`, `verified_by`, `from` (feedback, answer, blocked, advanced-main) and the learning `agent_id`; `/messages/:agent_id` maps the old path; `VerifyNow` encodes `orchestrator.verify-now` and decodes both; registry entries (wire values, removed 1.0.0); one `deprecation::warn` per broker process
- [ ] 4.3 Confirm the verdict-authority check (v0.18.0 `broker-verdict-authority`) and `drive::gate_failure` run on the normalized message; MCP root-side publish tools use the constant
- [ ] 4.4 `commands/remove.rs`: refuse `orchestrator` and `supervisor` naming `git paw stop`
- [ ] 4.5 `sweep.sh`: publish as `orchestrator`, read `/messages/orchestrator`, phantom filter accepts both root spellings, stuck flag `blocked-on-orchestrator` (blocked-age check accepts `from` in either spelling), usage text; read `[orchestrator.gates] test_command` and the `[drive]` windows first with the `[supervisor]` fallback, through `tomllib` and the regex fallback; keep heredoc bodies apostrophe-free, run `bash -n`; sync the tracked `.git-paw/scripts/sweep.sh`
- [ ] 4.6 Security review (`security-and-safety-review`): normalization precedes every authority, routing and correction-loop decision; no path accepts a third spelling; scripts build no new shell from config values

## 5. Persisted values, output and skills

- [ ] 5.1 Session wire type: `mode` reads `"supervisor"` and `"coordinated"`, writes `"supervisor"`; internal `SessionMode` variant renamed
- [ ] 5.2 MCP `SessionSnapshot`: add `session_mode`; keep `mode`, marked deprecated in the schema; registry output-field entry
- [ ] 5.3 Doctor: group "Coordination", ids `coordination.*` (coordinate with `uniform-json-output`), gate remedies naming `[orchestrator.gates]`; Config group ⚠ for version-1 paths pointing at `git paw config check`
- [ ] 5.4 `config check`: one warning finding per version-1 path from the registry, error findings for conflicts and for `[supervisor]` in a version-2 file, `config_version` 1 for an unversioned file
- [ ] 5.5 `skills::resolve` and MCP `get_skill`: warn on the `supervisor` name and on a `supervisor.md` override (registry entry, removed 1.0.0)

## 6. Prose (exported assets stay project-agnostic)

- [ ] 6.1 Reword `assets/agent-skills/coordination.md` and `orchestrator.md`, `assets/boot-block-template.md`, `assets/boot-block-mcp-template.md`, `assets/inter-agent-rules.md`, `assets/governance-section-header.md`, `assets/hooks/pre-push.sh`, `assets/hooks/pre-commit.sh` (`[workers] strict_branch_guard`), the escalation-framing asset, `opsx/role_guard.rs` (`[orchestrator] auto_revert`), `broker/learnings.rs` lines, drive-loop correction / escalation texts, MCP tool descriptions and server instructions; re-point the unit tests that pin these strings
- [ ] 6.2 Run `tests/lang_agnostic_skill_audit.rs` and the `export-agnosticism` checklist over the changed assets

## 7. Tests (one or more per scenario)

- [ ] 7.1 `tests/config_v2_migration.rs`: table test, one row per mapping entry with a version-2 path, version-1 vs version-2 equal configs ("Every version-1 path loads as its version-2 path"); a file with both auto-approval tables loads equal to the file without them, with no `[workers.*]` table ("The deprecated auto-approval tables are dropped, not moved"); file bytes and mtime unchanged ("A version-1 file is never rewritten"); section without `enabled` → bare; invalid `default_mode`; conflict refused; unrelated keys coexist; `version = 2` with `[supervisor]` refused
- [ ] 7.2 Warning tests: two families → two lines; double load → one line each ("Loading twice in one process warns once"); `status --json` byte-identical stdout with and without version-1 keys ("The warning leaves stdout and the exit code alone"); the auto-approval tables: one line naming both, only the declared one, none for a file without them, the no-effect ending and no family warning for them ("An old configuration with both tables loads with one warning", "The warning names only the declared table", "A configuration without the tables produces no warning about them", "A no-effect item warns without a replacement"); the v0.17.0 *The tables have no effect on a run* e2e keeps passing on a version-1 file
- [ ] 7.2a Dropped-table rewrite tests (HOME isolated): `add-cli` on a global file with both tables drops them under every name and its notice names both ("add-cli drops the deprecated auto-approval tables and names them", "A rewrite drops the tables"); `status` and `init` leave a repository file's `[supervisor.auto_approve]` byte-identical ("A repository file is never rewritten to drop them")
- [ ] 7.3 Merge tests in `tests/config_integration.rs`: repository table hides the global tables (worker approval stays `auto`); global tables apply when the repository has none
- [ ] 7.4 `add-cli` on a global `[supervisor]` file (HOME isolated): version-2 layout, `version = 2`, notice line; `init` leaves a version-1 file unstamped with the hint, stamps a layout-neutral file; template has `version = 2`, the four commented blocks and no `[supervisor` ("The template carries no supervisor table", "`git paw init` writes the commented tables", "The commented tables are valid TOML when uncommented")
- [ ] 7.5 Init prompt (PTY harness `tests/helpers/pty.rs`): yes + test command, no, version-1 table left alone without a prompt
- [ ] 7.6 `resolve_start_mode` unit table with a fake prompt covering all seven start-mode scenarios
- [ ] 7.7 `cli.rs` parse tables: the `--coordinated`, `--no-rebase`, `--bare`, mutual-exclusion and `--unattended` scenarios (the help asserts escalation wording and no auto-approve claim); aliases parse equal to canonical; alias conflicts; `start --help` and `completions bash|zsh|fish` contain neither alias and `start --help` contains no `supervisor`; `--unattended` dispatch routes to the coordinated path
- [ ] 7.8 e2e: `start --no-supervisor --dry-run` warns once and matches `--bare` stdout and exit ("An alias warns and still works", "A deprecated flag warns once"); `start --coordinated --dry-run` with a version-2 config prints no warning ("No warning without use")
- [ ] 7.9 Broker integration: old spelling stored as `orchestrator` on `/status`; both inbox paths serve the question; blocked `from` normalized and routed; verify nudge type and old-type decode; authority refusal identical for both spellings
- [ ] 7.10 Extend `tests/wire_script_parity.rs` (from `wire-format-dtos`): `sweep.sh status-publish`, `verified`, `learn` bodies carry `orchestrator` and no `supervisor`
- [ ] 7.11 `sweep.sh` config tests: version-2 key read through `tomllib` and the forced regex fallback; version-1 `test_command` and `blocked_on_supervisor_window_seconds` still resolve both ways
- [ ] 7.12 Session wire tests: both spellings load equal; saved coordinated session writes `"supervisor"`; recovery of a `"coordinated"` file rebuilds the coordinated layout
- [ ] 7.13 MCP tests: coordinated and bare `get_session_status` fields; schema marks `mode` deprecated; `get_skill("supervisor")` returns the orchestrator skill with one stderr warning and clean JSON-RPC; `orchestrator` does not warn; legacy `supervisor.md` override warns
- [ ] 7.14 Doctor tests (`src/doctor_tests.rs`, `tests/doctor_e2e.rs`): group list with Coordination and no Supervisor; missing gate ✗ remedy names `[orchestrator.gates]`; bare default informational ✓; `--json` ids `coordination.*`, no `supervisor` in ids or groups; version-1 file ⚠ with exit unaffected
- [ ] 7.15 `config check` tests: per-key findings with version-2 names, `config_version` 1, exit 0 / 1 under `--strict`; conflict error exit 1; a dropped auto-approval table is one no-effect finding with no `workers.` path ("A dropped table has no version-2 name"); findings sourced from the registry ("Config findings come from the registry")
- [ ] 7.16 `remove orchestrator` and `remove supervisor` refused naming `git paw stop`, state unchanged
- [ ] 7.17 Doc guards: CLI reference Deprecations section lists every registry entry (the two auto-approval tables as "no effect", "read through migration"); configuration reference migration table equals the mapping, dropped rows included

## 8. Docs

- [ ] 8.1 `docs/src/configuration/README.md`: `default_mode`, `[orchestrator]`, `[orchestrator.gates]`, `[orchestrator.escalation]`, `[workers]`, `[drive]` sections; one-unit merge rule; "Migrating from `[supervisor]`" table (the two auto-approval tables as "dropped — deprecated 0.17.0, no effect"); `version = 2`; script-read key list with version-1 fallbacks
- [ ] 8.2 `docs/src/cli-reference.md`: `--coordinated` / `--bare` in the start-flag table; Deprecations section; `remove` refusal wording; MCP `session_mode` in `docs/src/user-guide/mcp.md`
- [ ] 8.3 `docs/src/user-guide/supervisor.md` retitled "Coordinated mode" (path unchanged), `SUMMARY.md` title; vocabulary in `architecture.md`, `quick-start-orchestrator.md`, `user-guide/{coordination,conflict-detection,learnings,dashboard,doctor,session-lifecycle,tiered-model-workflow,governance,agents-md,spec-driven-launch,skill-templates,cli-prompt-profiles}.md`
- [ ] 8.4 README: coordinated mode and the new flags (coordinate with `readme-landing-page`)
- [ ] 8.5 Backfill `## Purpose` of `openspec/specs/core-deprecation/spec.md` at archive (text in proposal.md) and add the capability to the specifications page
- [ ] 8.6 Commit bodies name every deprecation so `git cliff` carries them into the changelog

## 9. Gates

- [ ] 9.1 Every scenario in the ten delta specs maps to a test named in 1 and 7
- [ ] 9.2 `cargo fmt`, then `just lint`
- [ ] 9.3 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 9.4 `just deny`
- [ ] 9.5 `mdbook build docs/`
- [ ] 9.6 `openspec validate supervisor-vocabulary-split --strict`

Model tier: Opus. One commit per numbered section, each buildable (`just check` green). Read
`design.md` first; D-numbers below refer to it. Merge after `classifier-retirement` and
`bundled-helper-discovery` (design D10).

## 1. Fact gate (blocking — before any code)

- [ ] 1.1 Save the script in design.md "Fact gate procedure" to a scratch file outside the repository and run it from a plain terminal outside any git-paw session (`env -u TMUX -u TMUX_PANE`), with an authenticated Claude Code on PATH. If your own session cannot run `claude -p`, stop and ask the human to run it and paste the printed lines. Re-run any blocking step that is inconclusive (refused with `denials = 0`) with `--output-format stream-json --verbose` until the tool call is visible. Optionally run F1d with the dogfood's Claude Code variant (`CLAUDE_BIN=claude-oss`) and F8 interactively.
- [ ] 1.2 Fill in design.md "Observed facts" (CLI version, date, OS, every step's printed line, outcome O1–O4) and decide `verified` for the claude allowlist row per "Outcomes".
- [ ] 1.3 Apply the outcome to the artifacts before any code: for O1, amend the literal entries in `specs/cli-setup/spec.md` and the D5 table if F2 showed a different working form or a bare invocation is not matched; for O2, amend D4 and the target requirement's wording to the observed resolution; for O3, amend the protection form and re-run the gate once, else record the failing steps; record the F8 durable-grant shape in D14 and the advisory shape list if it is new. Run `openspec validate cli-setup-consent --strict` (real exit code). Commit the artifact amendments alone.

## 2. Seams: JSON key order and the exit-2 variant (behaviour-preserving)

- [ ] 2.1 Only after the orchestrator confirms the `preserve_order` approval for v0.17.0 (design D7): enable serde_json's `preserve_order` feature in `Cargo.toml`, run `cargo build` to refresh `Cargo.lock` (no new crate expected: `indexmap` 2.14.0 is already locked), run `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast > log 2>&1; echo $?`, and review every golden or string assertion whose key order changed. If the approval is declined, skip this task and build the D7 alternative inside `src/cli_setup/` in task 4.1.
- [ ] 2.2 Add `PawError::NotAttempted(String)` with an actionable `Display` and `exit_code() == 2`. Update `error::tests::test_general_errors_exit_code` to exclude it and add a test that a no-terminal `NotAttempted` naming `--yes` exits 2 and does not read `Cancelled.` (`core-error-handling` MODIFIED scenarios).

## 3. Fact table and grant table (data)

- [ ] 3.1 New `src/cli_setup/mod.rs` (module docs) and `src/cli_setup/facts.rs`: `SetupItem { Allowlist, Trust, Hooks }`, the fact-row type (identity, declared items, target file relative to the main checkout, key paths, `verified`, `verified_with`), the single `claude` row with `verified` set from task 1, the D5 grant table keyed by `init::bundled_scripts()` names, the never-granted `sweep.sh` subcommands, the `approve` ask rule and the D6 protection rules. Doc comment on the row records the task-1 evidence.
- [ ] 3.2 Unit tests: "Fact support is data" (exactly one row, `claude`, exactly `allowlist`); every bundled script has a grant-table row; every subcommand in `assets/scripts/sweep.sh`'s `main` dispatch (read with `include_str!`) is granted or never-granted; every planned allow entry begins `Bash(.git-paw/scripts/<bundled name>` and contains no `curl`, `bash `, leading `/` or `Bash(*`; no allow entry names `sweep.sh approve|verified|feedback-gate` or `docs-fetch.sh get`.

## 4. Functional core: plans and rendering

- [ ] 4.1 `cli_setup::plan` (pure). Inputs: fact row, requested items, repository root, file snapshots (content, tracked, ignored, symlink components, mode). Output: `SetupPlan` or `Refusal`. Table tests: unsupported item (single and mixed) → nothing planned; identity without a row; unverified row → plan-only flag; allow entries exactly the grant table plus the `approve` ask rule; protection rules always planned with the grants; already-present entries not planned and not recorded; malformed JSON, non-object root, wrong-typed `permissions.allow`, tracked file, symlinked `.claude`, escaping path all refused naming file/key; exclusion line only when the target is not ignored; nothing to do; two differently configured repositories plan identical entries.
- [ ] 4.2 `cli_setup::plan_undo` (pure). Inputs: record, fact row, snapshots. Table tests: removes only recorded, present, equal and shape-valid entries; a forged `Bash(make test *)` entry is reported, dropped from the record and left on disk; a changed entry is left in place and reported; protection kept while any allow entry naming `.git-paw/scripts/` remains (the edited-broker-entry scenario); skeleton-only file deletion and empty-directory removal; a user-extended created file is kept; another identity's entries untouched; item filtering; works for an unverified row.
- [ ] 4.3 Plan rendering (pure): CLI, item, verification status and `verified_with`, absolute paths marked created/modified, key path and value (or line), the record path, the plan-only notice; never any existing file content. Golden test, plus a test that a settings file holding `"API_TOKEN": "s3cret"` never appears in the rendered plan.

## 5. Imperative shell

- [ ] 5.1 Snapshot collection through `CommandRunner`: main-checkout root, `git rev-parse --git-dir` and `--git-common-dir`, `git ls-files --error-unmatch`, `git check-ignore`, symlink checks on every component, file mode. Linked-worktree refusal (`--git-dir` ≠ `--git-common-dir`) with a message naming the main checkout. Tests for the refusal and each snapshot field.
- [ ] 5.2 Record store at `<git-common-dir>/git-paw/cli-setup.json`: `schema_version`, atomic write, mode `0600`, entry recorded before its write. Tests: round-trip; created-file marking; `git status` clean at the root and in a linked worktree after writing.
- [ ] 5.3 Apply: per file, re-read and compare with the snapshot (stop naming the file on change, later files untouched); temp file plus rename; keep mode; create new files `0600`; append the exclusion line. Tests: concurrent-change stop; unrelated settings kept in value and order with helper entries after `z`; created file owner-only.

## 6. Command surface

- [ ] 6.1 `src/cli.rs`: `SetupCli { cli, allowlist, trust, hooks, dry_run, yes, undo }`; a clap group requires one item unless `--undo`; `about`, `long_about` with examples, a `help` string on every flag; add the subcommand to the root `after_help` if it lists commands. Parse tests and a help test naming every item and option.
- [ ] 6.2 Use case `src/commands/setup_cli.rs`: resolve the identity (roster or `[clis]` key, else `CliNotFound`), look up the row, plan or plan undo, apply, and return the outcome. It never prompts and never reads stdin. A variant taking an explicit fact row is used by integration tests (design D12).
- [ ] 6.3 `src/main.rs` entry layer: print the plan; `--dry-run` → exit 0; unsupported item or unverified writing → `NotAttempted`, exit 2; nothing to do → exit 0 without prompting; on a TTY without `--yes`, `dialoguer::Confirm` defaulting to No, decline → `NotAttempted`, exit 2; off a TTY without `--yes` → `NotAttempted` naming `--yes` and `--dry-run`, exit 2; the same gate for `--undo`.

## 7. Skill

- [ ] 7.1 Write `assets/agent-skills/cli-setup.md` (standardized frontmatter; the seven consent rules of the `skill-agent` requirement; points to `git paw doctor`; no vendor product, settings path, stack token or commit-convention prefix; no helper path described as safe to approve). Register `cli-setup` in `skills::embedded_default`. Add the per-CLI setup section to `assets/agent-skills/supervisor.md` naming the skill, how to load it (MCP `get_skill` `cli-setup`), and restating rules 1, 3 and 4.
- [ ] 7.2 Tests: `skills::resolve("cli-setup")` and MCP `get_skill` return the embedded content with source `embedded`; consent rules present; supervisor skill section present with the three rules; extend `tests/lang_agnostic_skill_audit.rs` (and its audited-asset list) to the rendered `cli-setup` skill and the new supervisor section.

## 8. Doctor

- [ ] 8.1 Setup probe (record, the files it names, helper contents in the main checkout and the current session's worktrees against `init::bundled_scripts()`) and pure `check_cli_setup` in the CLIs group. Table tests: not applied ✓ with items and verification status; recorded entry removed ⚠ naming `--undo`; out of date ⚠ naming `--undo` and re-applying; modified granted helper ⚠ naming its path; no check for an identity without a row or record entries; never ✗.
- [ ] 8.2 Broad-grant probe (the row's target file) and pure `check_broad_grants`. Table tests over the eight shapes, including `Bash(git-paw purge:*)`, `Bash(git push:*)`, `Bash(cargo:*)` listed and `Bash(npm run test:*)`, git-paw's own planned entries and a loopback `curl` grant not listed; `Bash(.git-paw/scripts/sweep.sh *)` listed; abbreviation (`Bash(curl -H …)`, no `abc123`); unparseable file ⚠ "not inspected" without content; absent file → no check.
- [ ] 8.3 E2E: after `git paw doctor`, the setup record, the settings file and every helper are byte-identical, and doctor's exit code is unaffected by both checks.

## 9. End-to-end and non-invocation guards

- [ ] 9.1 `tests/cli_setup_e2e.rs` (`assert_cmd`, temp repo, `HOME` isolated, stdin not a TTY): item required (exit 2); unknown CLI; linked-worktree refusal; trust unsupported; mixed request writes nothing; `[clis.my-agent]` unsupported; dry run with `--yes` writes nothing; no TTY and no `--yes` (exit 2, stderr names `--yes`, not only `Cancelled.`); plan names the main-checkout file and no worktree path; `settings_path` never targeted; secret never echoed; tracked, symlink, malformed and wrong-type refusals; undo gated. Through the real binary when the shipped row is verified, else asserting plan-only behaviour; the writing paths (`--yes` applies, record written, `git status` clean, exclusion line, undo cases) are covered by `tests/cli_setup_apply.rs` through the use case with a constructed verified row.
- [ ] 9.2 Non-invocation guards: `git paw init` creates no `.claude/` and no record; after setup, `git paw add` creates no settings directory in the new worktree and leaves the main-checkout file and the record byte-identical; recovery and a drive-loop run leave the record and its files byte-identical; the MCP tool list has no tool that runs setup or writes CLI settings; the existing "no settings file at session start" tests still pass.
- [ ] 9.3 Map every scenario in `specs/cli-setup`, `specs/approval-command-safety`, `specs/core-error-handling`, `specs/preflight-diagnostics` and `specs/skill-agent` to a named test and add the `Test:` pointers to the scenarios.

## 10. Docs

- [ ] 10.1 New `docs/src/user-guide/cli-setup.md` and a `SUMMARY.md` entry: why setup is consent-only; the one target file and that it covers worktree sessions and later sessions; the grant table and what is deliberately not granted; the protection rules; the plan, `--dry-run`, `--yes`, `--undo`; verified vs plan-only (with the task-1 outcome); the record location; residual risks (prefer the sandbox, do not pair with `full-auto` outside one, undo when no longer wanted, undo before downgrading); Claude Code variants use `git paw setup-cli claude`.
- [ ] 10.2 `docs/src/cli-reference.md`: `setup-cli` in the top-level command list and its own section (flags, exit codes 0/1/2, examples). README CLI table row. `--help` / `long_about` already from 6.1 — keep all three consistent.
- [ ] 10.3 `docs/src/supported-clis.md`: per-CLI setup-item support (claude: allowlist, with its verification status; every other CLI and item: not supported, pending the v0.23.0 audit).
- [ ] 10.4 `docs/src/configuration/README.md`: no new field; state that `[clis.<name>].settings_path` is never a setup target, and list `<git-common-dir>/git-paw/cli-setup.json` among the files git-paw writes.
- [ ] 10.5 `docs/src/user-guide/security-posture.md`: what setup may write and where, the grant table, the protection rules and the tamper threat, the consent model, the gate status, and the residuals from design "Threat analysis".
- [ ] 10.6 `docs/src/user-guide/doctor.md` (setup check and broad-grant advisory); `docs/src/user-guide/coordination.md` and `docs/src/user-guide/supervisor.md` (helper calls run without a prompt only through the CLI's own permission system or rules; the optional consented `setup-cli --allowlist`); `docs/src/user-guide/skill-templates.md` (the `cli-setup` skill); `docs/src/architecture.md` module list (`src/cli_setup/`, `src/commands/setup_cli.rs`).
- [ ] 10.7 After archive (supervisor): give the new `openspec/specs/cli-setup/spec.md` a real `## Purpose`, add `cli-setup` to `docs/src/specifications/README.md` under the `cli-` namespace, and, if `approval-command-safety`'s `## Purpose` (as left by `classifier-retirement`) still says git-paw writes no grants, add "except through the consented `git paw setup-cli`". Re-run the spec-index and Purpose guards.

## 11. Quality gates

- [ ] 11.1 `cargo fmt`, then `just lint > lint.log 2>&1; echo $?` (must print 0)
- [ ] 11.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast > test.log 2>&1; echo $?` (must print 0; read the log)
- [ ] 11.3 `cargo test --bin git-paw > bin.log 2>&1; echo $?` (must print 0)
- [ ] 11.4 `cargo deny check > deny.log 2>&1; echo $?` (serde_json feature change; must print 0)
- [ ] 11.5 `mdbook build docs/ > mdbook.log 2>&1; echo $?` (must print 0)
- [ ] 11.6 `openspec validate cli-setup-consent --strict; echo $?` and `openspec validate --all --strict; echo $?` (both 0)
- [ ] 11.7 Security gate: walk design "Threat analysis" against the diff with the `security-and-safety-review` checklist, and the `export-agnosticism` checklist for the written entries, the new skill and the supervisor-skill section

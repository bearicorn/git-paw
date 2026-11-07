## 1. Verify the usage source (before any code)

- [x] 1.1 Against the installed Claude Code, locate a worker's usage report from its assigned session id (default config directory, `CLAUDE_CONFIG_DIR`, and a variant's `settings_path` parent); record the location, the usage record shape and whether a cost figure exists in the PR. If it cannot be verified, the Claude family declares no usage source and tasks 2.2–2.3 are dropped
- [x] 1.2 Capture redacted fixture reports (usage fields only) for the parser tests

## 2. Usage facet

- [x] 2.1 Add `CliProvider::usage` with the `Unknown` default and `UsageReport` (design D1)
- [x] 2.2 Claude family: pure parser from report text to totals (+ cost when present) and a read-only adapter that locates the report; parse failure → `Unknown(reason)`
- [x] 2.3 Tests: fixture report → exact totals; malformed report → unknown; generic CLI → unknown naming the CLI; the adapter never logs report content

## 3. Attribution and status JSON

- [x] 3.1 Attribute per worker from `WorktreeEntry.cli_session_id` and the worker's CLI record; group by CLI id
- [x] 3.2 Assign a session id to each v0.18.0 headless one-shot whose provider has a `session_id_args` template and keep the ids for the run's orchestrator-role total
- [x] 3.3 `git paw status --json`: per-worktree `usage` (or `null`) and `usage_total` with the unknown count; existing fields untouched; human output byte-identical (extend the existing status JSON / human-output tests)

## 4. Budget config

- [x] 4.1 Add `budget_max_tokens` / `budget_max_cost` to `SupervisorConfig` with serde defaults and skip-if-absent; reject `0` / negative naming the field; round-trip test
- [x] 4.2 `git paw init` commented example; configuration reference entry

## 5. Drive-loop guardrail

- [x] 5.1 Read usage every N sweeps (N configurable only through the existing test env-var pattern); compare known totals with the caps
- [x] 5.2 On first breach: one `budget-exhausted` escalation; disable re-engagement / correction, one-shots and branch refreshes for the rest of the run
- [x] 5.3 `abandon` → `DriveOutcome::BudgetExhausted`, non-zero exit; `escalate` → keep sweeping; never signal or kill a pane
- [x] 5.4 Start-of-loop report of unknown workers and of an unenforceable cost cap
- [x] 5.5 Exit summary usage section (by role, by CLI id, unknown count, "excludes your own session", caps beside totals) and the new outcome in the outcome line
- [x] 5.6 Unit tests with fake usage readers for every scenario in `specs/supervisor-unattended-operation`; one `tests/unattended_drive_e2e.rs` case with a stub CLI and a fixture report proving the `abandon` exit leaves the panes alive

## 6. Orchestrator skill

- [x] 6.1 Add the budget step to the exported orchestrator skill asset (v0.18.0 `orchestrator-skill-export`); keep it vendor-agnostic
- [x] 6.2 Content test anchored on the step's key terms; extend `tests/lang_agnostic_skill_audit.rs` coverage if the step lives in a new asset

## 7. Spec traceability

- [x] 7.1 Map every scenario in `specs/cli-provider`, `specs/session-state`, `specs/supervisor-config`, `specs/supervisor-unattended-operation` and `specs/skill-agent` to a named test
- [x] 7.2 Rebase the `Exit summary` delta onto v0.18.0 `supervisor-pane-removal`'s archived version before archiving this change

## 8. Docs

- [x] 8.1 New section in `docs/src/user-guide/supervisor.md` (or the v0.18.0 orchestrator chapter): usage attribution, what is excluded, the budget guardrail and its limits (a worker that never prompts keeps spending until its task ends)
- [x] 8.2 `docs/src/user-guide/tiered-model-workflow.md`: reading per-tier usage
- [x] 8.3 `docs/src/configuration/README.md`: the budget fields; `docs/src/cli-reference.md`: the new `status --json` fields

## 9. Quality gates

- [x] 9.1 `cargo fmt`, then `just lint`
- [x] 9.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [x] 9.3 `just deny`
- [x] 9.4 `mdbook build docs/`
- [x] 9.5 `openspec validate worker-usage-budget --strict`

## 10. Review round 1

- [x] 10.1 `budget_max_tokens` counts all four token kinds; `status --json` `usage_total.tokens`; the skill step names it; realistic example value
- [x] 10.2 The guard keeps last known figures per (branch, session id) so the total never goes down; warn once when a known worker turns unknown
- [x] 10.3 `budget_max_cost` that no worker CLI can satisfy: doctor warning and one escalation at loop start
- [x] 10.4 `settings_path` locates usage reports: configuration reference and spec
- [x] 10.5 `[supervisor] budget_on_exhausted`, independent of `[supervisor.correction] on_exhausted`
- [x] 10.6 Incremental report reads cached per file by length and mtime
- [x] 10.7 After a breach, buffered human input is still submitted; branch refreshes and their notifications stop
- [x] 10.8 Docs: `family = "claude"` for custom variants; the no-cap behaviour reworded
- [x] 10.9 `GIT_PAW_DRIVE_USAGE_EVERY_SWEEPS` documented in code as a test hook

## 11. Review round 2

- [x] 11.1 `status --json` `usage_total` described as the current workers only; the skill keeps a running total of removed workers' usage
- [x] 11.2 Stale correction-policy wording replaced by `[supervisor] budget_on_exhausted`; D4 updated
- [x] 11.3 The unevaluable-cost-cap escalation does not make a finished wave `escalated-for-review`
- [x] 11.4 A known→unknown worker is counted once, by its kept figures
- [x] 11.5 Doctor uses the loop's condition (not every configured worker CLI reports cost); probe test from a config
- [x] 11.6 A report rewritten with a new mtime and no growth is read again from its start
- [x] 11.7 Status docs say `--json` reads each worker's transcripts in full

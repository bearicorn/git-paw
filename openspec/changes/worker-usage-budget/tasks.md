## 1. Verify the usage source (before any code)

- [ ] 1.1 Against the installed Claude Code, locate a worker's usage report from its assigned session id (default config directory, `CLAUDE_CONFIG_DIR`, and a variant's `settings_path` parent); record the location, the usage record shape and whether a cost figure exists in the PR. If it cannot be verified, the Claude family declares no usage source and tasks 2.2–2.3 are dropped
- [ ] 1.2 Capture redacted fixture reports (usage fields only) for the parser tests

## 2. Usage facet

- [ ] 2.1 Add `CliProvider::usage` with the `Unknown` default and `UsageReport` (design D1)
- [ ] 2.2 Claude family: pure parser from report text to totals (+ cost when present) and a read-only adapter that locates the report; parse failure → `Unknown(reason)`
- [ ] 2.3 Tests: fixture report → exact totals; malformed report → unknown; generic CLI → unknown naming the CLI; the adapter never logs report content

## 3. Attribution and status JSON

- [ ] 3.1 Attribute per worker from `WorktreeEntry.cli_session_id` and the worker's CLI record; group by CLI id
- [ ] 3.2 Assign a session id to each v0.18.0 headless one-shot whose provider has a `session_id_args` template and keep the ids for the run's orchestrator-role total
- [ ] 3.3 `git paw status --json`: per-worktree `usage` (or `null`) and `usage_total` with the unknown count; existing fields untouched; human output byte-identical (extend the existing status JSON / human-output tests)

## 4. Budget config

- [ ] 4.1 Add `budget_max_tokens` / `budget_max_cost` to `SupervisorConfig` with serde defaults and skip-if-absent; reject `0` / negative naming the field; round-trip test
- [ ] 4.2 `git paw init` commented example; configuration reference entry

## 5. Drive-loop guardrail

- [ ] 5.1 Read usage every N sweeps (N configurable only through the existing test env-var pattern); compare known totals with the caps
- [ ] 5.2 On first breach: one `budget-exhausted` escalation; disable nudges, re-engagement / correction and one-shots for the rest of the run
- [ ] 5.3 `abandon` → `DriveOutcome::BudgetExhausted`, non-zero exit; `escalate` → keep sweeping; never signal or kill a pane
- [ ] 5.4 Start-of-loop report of unknown workers and of an unenforceable cost cap
- [ ] 5.5 Exit summary usage section (by role, by CLI id, unknown count, "excludes your own session", caps beside totals) and the new outcome in the outcome line
- [ ] 5.6 Unit tests with fake usage readers for every scenario in `specs/supervisor-unattended-operation`; one `tests/unattended_drive_e2e.rs` case with a stub CLI and a fixture report proving the `abandon` exit leaves the panes alive

## 6. Orchestrator skill

- [ ] 6.1 Add the budget step to the exported orchestrator skill asset (v0.18.0 `orchestrator-skill-export`); keep it vendor-agnostic
- [ ] 6.2 Content test anchored on the step's key terms; extend `tests/lang_agnostic_skill_audit.rs` coverage if the step lives in a new asset

## 7. Spec traceability

- [ ] 7.1 Map every scenario in `specs/cli-provider`, `specs/session-state`, `specs/supervisor-config`, `specs/supervisor-unattended-operation` and `specs/skill-agent` to a named test
- [ ] 7.2 Rebase the `Exit summary` delta onto v0.18.0 `supervisor-pane-removal`'s archived version before archiving this change

## 8. Docs

- [ ] 8.1 New section in `docs/src/user-guide/supervisor.md` (or the v0.18.0 orchestrator chapter): usage attribution, what is excluded, the budget guardrail and its limits (a worker that never prompts keeps spending until its task ends)
- [ ] 8.2 `docs/src/user-guide/tiered-model-workflow.md`: reading per-tier usage
- [ ] 8.3 `docs/src/configuration/README.md`: the budget fields; `docs/src/cli-reference.md`: the new `status --json` fields

## 9. Quality gates

- [ ] 9.1 `cargo fmt`, then `just lint`
- [ ] 9.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 9.3 `just deny`
- [ ] 9.4 `mdbook build docs/`
- [ ] 9.5 `openspec validate worker-usage-budget --strict`

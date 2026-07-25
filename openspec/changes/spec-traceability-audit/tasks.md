# Tasks — spec-traceability-audit

Full findings: `.git-paw/v0.13.0-spec-traceability-audit.md`. Run first in the
specs→tests→code order; the requirement→test map is `spec-consolidation`'s safety net.

## 1. Reconcile the spec-vs-code contradictions (amend spec to match shipped, tested code)
- [x] `approval-configuration` `safe` preset → composed defaults (built-ins + resolved dev-allowlist), no user extras (delta authored)
- [x] `cli-parsing` Stop subcommand — RESOLVED (A, owner 2026-07-25): `stop` is non-destructive (kills CLIs, preserves worktrees+state, recoverable via `git paw start`) → no prompt; `--force` kept accepted-but-inert + documented no-op (removing it would break `stop --force` scripts — back-compat). Delta authored; matches `cli-interaction-e2e`
- [ ] `supervisor-launch` — amend the 3 prompt-injection scenarios (OpenSpec `/opsx:apply {id}`, Markdown `AGENTS.md`, verbatim no-spec string) to match the shipped `build_task_prompt` `.git-paw/AGENTS.local.md` pointer
- [ ] `message-delivery` — amend the 3 roster-upsert requirements to match the W15-16 fix (Verified/Feedback/Question do NOT upsert the sender roster record)
- [ ] `broker-endpoints` + `message-delivery` — reconcile the empty-poll cursor (both specs → the monotonic cursor held at `since`, not `last_seq=0`)
- [ ] `cli-parsing`/`mcp-server` — scrub the removed `.specify/` filesystem-auto-detection references from `--specs-format` help + `mcp/query/specs.rs::resolve_dir`; add `superpowers` to the `--specs-format` value list
- [ ] Verify (don't duplicate) the sibling-owned fixes are closed: `cli-detection` roster (gemini change), phantom `git paw resume` + `learnings-mode` negative (spec-consolidation)

## 2. Fill genuine scenario→test gaps
- [ ] Triage the ~40 `no-test` scenarios: mark TUI-draw/live-TTY coverage-exempt; add tests for the real gaps (esp. `mcp/query/{session,conflicts,intents}` tool outputs — extend `mcp_e2e.rs`)

## 3. Standing guards (model on `tests/spec_purpose_backfilled.rs`)
- [ ] `KNOWN_CLIS` ↔ `cli-detection` spec roster sync
- [ ] `--specs-format` value-list completeness (all `SpecBackendKind`, incl. `superpowers`)
- [ ] no-`.specify/`-auto-detection prose guard (spec + help)
- [ ] gemini-as-current-CLI guard (arms the agy swap)
- [ ] any remaining cheap guards from the audit's list of 7

## 4. Requirement→test map + verification
- [ ] Produce the requirement→test map across the audited capabilities
- [ ] Run it before AND after each `spec-consolidation` merge wave — no scenario loses its covering test
- [ ] Gate 3 self-check: every amended requirement still maps to ≥1 test; no new orphan
- [ ] `just check` green; `openspec validate spec-traceability-audit --strict` passes

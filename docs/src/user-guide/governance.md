# Governance

git-paw can read your team's existing governance documents as context for the supervisor agent — ADRs, a test strategy, a security checklist, a Definition of Done, and a project constitution. You point at the docs in `.git-paw/config.toml`; git-paw does not generate or vendor any of them.

## Why governance docs

The supervisor is an LLM. It does the right thing more often when it can see the rules the team has already written down: which architectural decisions are settled, what tests are expected, what security review looks like, what "done" actually means. Without that context the supervisor has to infer from the diff alone, and inference drifts.

git-paw's stance on governance is deliberately thin:

- **You own the documents.** Their structure, format, and rubric are whatever your team already uses (Scrum, XP, OWASP, `adr-tools`, Spec Kit, hand-rolled). git-paw does not template them.
- **You opt in per doc.** Empty `[governance]` table, all fields `None`, no behaviour change from v0.4. Add only the docs you have.
- **The supervisor applies judgment.** There is no `[governance.gates]` table and no per-doc enforcement switch. The supervisor reads each configured doc as context during its audit and surfaces relevant findings via `agent.feedback`.

## Pointing at your docs

Add `[governance]` to `.git-paw/config.toml`:

```toml
[governance]
adr = "docs/adr"
test_strategy = "docs/test-strategy.md"
security = "docs/security-checklist.md"
dod = "docs/definition-of-done.md"
constitution = ".specify/memory/constitution.md"
```

All five fields are optional. List only the docs you have. Paths are resolved relative to the repository root; absolute paths are accepted as-is. A missing file does not break config-load — the runtime flags it if the supervisor tries to read it.

For Spec Kit projects, `governance.constitution` auto-wires to `.specify/memory/constitution.md` when `[specs] type = "speckit"`. You only need to set `constitution` explicitly if you keep it somewhere else, or want to disable the auto-wiring (set it to `""`).

See [Configuration → Governance](../configuration/README.md#governance) for the full field reference and merging rules.

## Illustrative examples

The shapes below are **examples**, not templates. git-paw never reads structure — only the content of the file you point at. Use whatever format your team already uses.

### ADR-0001 — Adopt PostgreSQL for the primary store

```markdown
# ADR-0001: Adopt PostgreSQL for the primary store

Status: Accepted
Date: 2026-02-14
Deciders: backend team

## Context

We need a primary datastore for user, session, and billing tables.
Options considered: PostgreSQL, MySQL, CockroachDB, DynamoDB.

## Decision

We will use PostgreSQL 16 hosted on our existing managed service.

## Consequences

- All new schema lives in PostgreSQL; we will not introduce a second
  relational store without a follow-up ADR.
- Migrations go through `sqlx-migrate`; ad-hoc DDL in code review is
  rejected by default.
- Read replicas are an operational concern, not an application one —
  applications connect through the connection pool, not directly to
  replicas.
```

A typical ADR directory has one file per decision (`0001-postgres.md`, `0002-event-bus.md`, …). Whether you use Nygard, MADR, or your own headings does not matter — git-paw passes the directory pointer to the supervisor, which reads the files it finds.

### Definition of Done

```markdown
# Definition of Done

A change is "done" when every box below is checkable:

- [ ] All new behaviour has at least one test covering the happy path
      and one covering the failure mode.
- [ ] `just check` passes locally and in CI.
- [ ] Public functions have rustdoc comments.
- [ ] If a config field was added, the configuration docs section
      that owns it is updated.
- [ ] If a CLI flag was added, `--help` text and `cli-reference.md`
      are updated.
- [ ] If user-visible behaviour changed, the changelog has an entry.
- [ ] The PR description names the spec/issue it implements and
      lists any deviations.

Anything left unchecked goes in the PR description as a known
follow-up with an owner.
```

### Security checklist

```markdown
# Security Checklist

For every change touching authentication, authorisation, network I/O,
or persisted data:

- [ ] User input is validated at the boundary (HTTP handler or CLI
      arg parser), not in the business logic.
- [ ] Secrets never appear in logs, error messages, telemetry, or
      stack traces. The redaction helpers in `crate::log::redact`
      are applied at the producer side.
- [ ] No new dependency on `unsafe` crates without a written
      justification in the PR description.
- [ ] If the change introduces a new external request, the URL is
      built with a typed builder, never with `format!` over user
      input.
- [ ] Authorization checks happen before side effects — never after.
```

### Test strategy

```markdown
# Test Strategy

## Pyramid

We invest most heavily in fast, deterministic tests:

- **Unit tests** (`#[cfg(test)] mod tests {}`) — pure logic, no I/O.
  Every public function in `src/` has at least one.
- **Integration tests** (`tests/`) — exercise CLI boundaries with
  `assert_cmd`. Use real filesystems via `tempfile`, never mocks.
- **End-to-end smoke tests** (`tests/e2e/`) — kicked off in CI for
  the happy path of each subcommand. Slow and few; tagged with
  `#[ignore]` so contributors can opt in locally.

## What we do not test

- Generated output of `--help` strings (already covered by clap).
- Rendering loops in the TUI — covered manually via the smoke list
  in `docs/src/user-guide/dashboard.md`.

## Test data

Always synthesise — never copy production data, even anonymised, into
fixtures. Fixtures live under `tests/fixtures/` and are kept small
enough to read at a glance.
```

### Constitution (Spec Kit-style)

```markdown
# Project Constitution

## Spec-driven

No behaviour ships without a spec. The spec is the contract; the
implementation matches the spec; the tests assert the spec. If the
implementation drifts from the spec, the spec is updated first and
re-reviewed.

## Behavioural tests, not implementation tests

Tests assert observable inputs and outputs. Tests do not assert
internal struct field values, internal function calls, or mock
interactions.

## One commit per logical change

Every commit builds, passes `just check`, and is independently
revertable. PRs may contain multiple commits, but no commit may leave
the tree broken.
```

Spec Kit users typically keep this file at `.specify/memory/constitution.md`; git-paw auto-detects it when `[specs] type = "speckit"`.

## What the supervisor does

When the supervisor verifies an agent's change, it reads the configured governance documents alongside the diff. Findings flow through the existing `agent.feedback` channel — the supervisor does not crash or block on governance, it surfaces what it sees and lets the agent respond.

The runtime side of this — boot-prompt injection of the governance doc paths and the supervisor's audit handling — lives in the parallel `governance-context` capability. This chapter and the `[governance]` config table are the path-pointer slot only.

## Rollout suggestion

Adopt incrementally — there is no requirement to fill in all five paths at once:

1. Start with the doc you already have. Point `governance.dod` at your existing DoD, or `governance.constitution` at your principles doc.
2. Run a supervised session. Note which findings feel useful and which feel noisy.
3. Add the next doc. Each pointer narrows the gap between what the supervisor knows and what the team already agreed on.
4. If the team does not have one of these docs at all, write the smallest version you can live with rather than copying a template. The supervisor reads what you wrote — not a generic checklist.

You do not need a full governance framework to benefit from this. A two-paragraph DoD pointed at by `governance.dod` is more valuable than a perfect-but-empty `[governance]` table.

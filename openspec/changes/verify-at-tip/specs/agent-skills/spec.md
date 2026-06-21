## MODIFIED Requirements

### Requirement: Supervisor skill — five verification gates

The embedded `supervisor.md` skill SHALL enumerate the supervisor agent's verification sub-flow as **five** explicit first-class gates, run in the order below before any `agent.verified` message is published for a coding-agent branch. The Workflow section's steps 4-7 (or equivalent) SHALL be restructured so each gate has its own heading and prose. Findings from any gate flow through `agent.feedback`; the `errors` array entries SHALL each begin with a bracketed gate-name prefix (e.g. `[doc audit]`, `[security audit]`) so the recipient agent can route fixes to the right gate.

The five gates SHALL all run against the agent branch's current **tip**, re-resolved at verification time with `git rev-parse <branch>` — NOT against the commit SHA carried by the `committed` event (or `supervisor.verify-now` nudge) that triggered the verification. Because agents commit incrementally (implementation, then tests, then docs), the triggering event's SHA is typically the branch's *first* commit and is stale by the time a later gate runs. The skill SHALL therefore instruct the supervisor to:

- Re-resolve the branch tip (`git rev-parse <branch>`) immediately before establishing the isolated verify worktree, and run all five gates against that tip.
- Re-resolve and re-check-out the tip before **re-running** the gates (e.g. when a later `committed` event or `supervisor.verify-now` nudge arrives for the same branch, or when re-verifying after sending `agent.feedback`), so no gate ever reports against a snapshot older than the current tip.
- NEVER report a documentation or test surface as MISSING (in the doc-audit or spec-audit gates) when it is present at the re-resolved branch tip; the named v0.9.0 false-negative (doc audit flagged docs missing while they were committed at the tip) SHALL be cited as the motivating example.

For the gates that determine "what this change added or removed" — spec audit and the security-audit diff review — the supervisor SHALL compute the diff against the **merge-base** of the branch tip and the integration target (`git merge-base <integration-target> <tip>`), NOT against a stale integration tip. This prevents a behind-tip or rebased branch from showing the integration tip's own commits as spurious mass deletions or additions.

The five gates SHALL be, in order:

1. **Testing** — the supervisor SHALL run the configured `{{TEST_COMMAND}}` inside the verify worktree checked out at the re-resolved branch tip and capture full output. Failures here block all downstream gates.

2. **Regression analysis** — the supervisor SHALL diff the testing output against the baseline (the test outcome on `main` recorded before any agent was launched). Previously-passing tests that now fail SHALL be reported as regressions. Pure additions (new tests that did not exist on baseline) are not regressions.

3. **Spec audit** — for each `### Requirement:` and `#### Scenario:` block under `openspec/changes/<change>/specs/`, the supervisor SHALL verify (a) the implementation contains the SHALL/MUST behaviour the requirement describes, (b) at least one test exercises each scenario's WHEN/THEN. The supervisor SHALL assess "what the change added" from the merge-base diff (not a stale integration tip). Gaps SHALL be reported as `[spec audit] <requirement-name>: <gap description>` errors.

4. **Doc audit** — the supervisor SHALL verify the documentation surfaces named in the change's `Impact` section have been updated, reading them from the re-resolved branch tip so docs committed after the triggering event are seen. The doc surfaces in scope are: mdBook chapters under `docs/src/`, top-level `README.md`, `AGENTS.md`, the relevant `--help` text accessed via the binary, and rustdoc on changed public items. The existing governance-verification sub-step (DoD, ADRs, security checklist, test strategy, constitution docs) remains in scope as an **input source** for this gate — its findings are doc-audit findings tagged `[doc audit]`. Doc-audit gaps SHALL be reported as `[doc audit] <surface>: <gap description>` errors.

5. **Security audit** — the supervisor SHALL review the merge-base diff for the OWASP-relevant patterns called out in the project's `CLAUDE.md` (command injection, XSS, SQL injection, path traversal, unvalidated external input flowing into `Command::new(...)` or filesystem writes, secret leakage in logs/error messages) and for any new `unwrap()` / `expect()` calls outside test code (project-wide rule, also from `CLAUDE.md`). On doc/text-only changes this gate is normally a fast noop. Findings SHALL be reported as `[security audit] <category>: <issue>` errors.

The Workflow's `Verify or feedback` step (currently §7) SHALL be updated so the published `agent.verified` message's `message` field enumerates the outcome of all five gates (e.g. `"all five gates clean: testing OK, no regressions, spec audit clean, doc audit clean, security audit clean"`). On any gate failing, `agent.feedback` SHALL be published instead, with each error entry tagged by gate name as described above.

The existing governance-verification sub-step in the Spec Audit Procedure section is preserved and explicitly referenced as a doc-audit input source — it is not deleted, and the per-doc examples (DoD, ADRs, security.md, test-strategy.md, constitution.md) remain valuable guidance.

#### Scenario: Supervisor skill enumerates five verification gates

- **WHEN** the embedded supervisor skill's Workflow section is inspected
- **THEN** it lists exactly five first-class verification gates in this order: Testing, Regression analysis, Spec audit, Doc audit, Security audit
- **AND** each gate has its own heading or sub-section (not buried inside another gate)

#### Scenario: Supervisor skill specifies gate-name prefixes in feedback errors

- **WHEN** the embedded supervisor skill's `agent.feedback` example or guidance is inspected
- **THEN** the example or prose makes clear that errors in the `errors` array begin with a bracketed gate-name prefix (e.g. `[doc audit]`, `[security audit]`, `[spec audit]`, `[regression]`, `[testing]`)

#### Scenario: Supervisor skill defines the doc-audit surfaces

- **WHEN** the embedded supervisor skill's Doc audit gate is inspected
- **THEN** it enumerates the doc surfaces in scope: mdBook chapters under `docs/src/`, `README.md`, `AGENTS.md`, `--help` text, and rustdoc on changed public items
- **AND** it cross-references the change's `Impact` section as the authoritative driver of which surfaces apply per audit

#### Scenario: Supervisor skill defines the security-audit categories

- **WHEN** the embedded supervisor skill's Security audit gate is inspected
- **THEN** it enumerates the OWASP categories from `CLAUDE.md`: command injection, XSS, SQL injection, path traversal, unvalidated external input, secret leakage in logs/errors
- **AND** it also calls out the project-wide rule against new `unwrap()` / `expect()` outside test code

#### Scenario: Supervisor skill preserves the governance-verification sub-step as a doc-audit input source

- **WHEN** the embedded supervisor skill is inspected after this requirement is implemented
- **THEN** the existing governance-verification sub-step (with DoD, ADR, security.md, test-strategy.md, constitution.md examples) is still present
- **AND** the prose explicitly cross-references it from the Doc audit gate as an input source

#### Scenario: Supervisor skill's verified-message enumerates all five gates

- **WHEN** the embedded supervisor skill's `agent.verified` example or guidance for the message-field content is inspected
- **THEN** the example or prose instructs the supervisor to enumerate the outcomes of all five gates in the message field (not only "tests pass" or "spec audit clean")

#### Scenario: Gates run against the re-resolved branch tip, not the triggering-event SHA

- **WHEN** the embedded supervisor skill's five-gate verification guidance is inspected
- **THEN** it SHALL instruct the supervisor to re-resolve the agent branch tip with `git rev-parse <branch>` at verification time and run all five gates against that tip
- **AND** it SHALL state that the gates MUST NOT run against the commit SHA carried by the triggering `committed` event or `supervisor.verify-now` nudge, since that SHA is typically the branch's stale first commit

#### Scenario: Doc/test surfaces present at the tip are not reported missing

- **GIVEN** an agent that committed implementation, then later committed tests and docs, advancing the branch tip after the triggering `committed` event fired
- **WHEN** the doc-audit and spec-audit gates run against the re-resolved branch tip
- **THEN** the skill SHALL state that documentation and test surfaces present at the tip MUST NOT be reported as MISSING
- **AND** it SHALL cite the v0.9.0 false-negative (docs flagged missing while committed at the tip) as the motivating example

#### Scenario: Re-verification re-resolves the tip before re-running gates

- **GIVEN** a branch already verified once whose agent then publishes a later `committed` event (or a `supervisor.verify-now` nudge arrives) for a newer commit
- **WHEN** the supervisor re-runs the gates for that branch
- **THEN** the skill SHALL instruct it to re-resolve `git rev-parse <branch>` and re-check-out the new tip before re-running, so no gate reports against a snapshot older than the current tip

#### Scenario: Change contribution is diffed against the merge-base

- **WHEN** the embedded supervisor skill's spec-audit and security-audit gate guidance is inspected
- **THEN** it SHALL instruct the supervisor to compute "what the change added/removed" from the merge-base of the branch tip and the integration target (`git merge-base <integration-target> <tip>`), NOT from a stale integration tip
- **AND** it SHALL state the rationale that diffing against a stale integration tip makes a behind-tip or rebased branch show spurious mass deletions or additions

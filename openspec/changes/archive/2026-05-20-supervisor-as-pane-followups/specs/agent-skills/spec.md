## ADDED Requirements

### Requirement: Supervisor skill — five verification gates

The embedded `supervisor.md` skill SHALL enumerate the supervisor agent's verification sub-flow as **five** explicit first-class gates, run in the order below before any `agent.verified` message is published for a coding-agent branch. The Workflow section's steps 4-7 (or equivalent) SHALL be restructured so each gate has its own heading and prose. Findings from any gate flow through `agent.feedback`; the `errors` array entries SHALL each begin with a bracketed gate-name prefix (e.g. `[doc audit]`, `[security audit]`) so the recipient agent can route fixes to the right gate.

The five gates SHALL be, in order:

1. **Testing** — the supervisor SHALL run the configured `{{TEST_COMMAND}}` inside the agent's worktree and capture full output. Failures here block all downstream gates.

2. **Regression analysis** — the supervisor SHALL diff the testing output against the baseline (the test outcome on `main` recorded before any agent was launched). Previously-passing tests that now fail SHALL be reported as regressions. Pure additions (new tests that did not exist on baseline) are not regressions.

3. **Spec audit** — for each `### Requirement:` and `#### Scenario:` block under `openspec/changes/<change>/specs/`, the supervisor SHALL verify (a) the implementation contains the SHALL/MUST behaviour the requirement describes, (b) at least one test exercises each scenario's WHEN/THEN. Gaps SHALL be reported as `[spec audit] <requirement-name>: <gap description>` errors.

4. **Doc audit** — the supervisor SHALL verify the documentation surfaces named in the change's `Impact` section have been updated. The doc surfaces in scope are: mdBook chapters under `docs/src/`, top-level `README.md`, `AGENTS.md`, the relevant `--help` text accessed via the binary, and rustdoc on changed public items. The existing governance-verification sub-step (DoD, ADRs, security checklist, test strategy, constitution docs) remains in scope as an **input source** for this gate — its findings are doc-audit findings tagged `[doc audit]`. Doc-audit gaps SHALL be reported as `[doc audit] <surface>: <gap description>` errors.

5. **Security audit** — the supervisor SHALL review the diff for the OWASP-relevant patterns called out in the project's `CLAUDE.md` (command injection, XSS, SQL injection, path traversal, unvalidated external input flowing into `Command::new(...)` or filesystem writes, secret leakage in logs/error messages) and for any new `unwrap()` / `expect()` calls outside test code (project-wide rule, also from `CLAUDE.md`). On doc/text-only changes this gate is normally a fast noop. Findings SHALL be reported as `[security audit] <category>: <issue>` errors.

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

### Requirement: Supervisor skill — continuous-sweep absorption doctrine

The embedded `supervisor.md` skill SHALL codify that absorbing routine inner-agent permission prompts is the supervisor agent's continuous responsibility, not a launch-time-only activity. Specifically:

1. The skill's "Workflow" section's `Watch` step (currently §2) SHALL state that on every monitoring-loop iteration, the supervisor agent applies the launch-time safe-command policy (currently §1.5) to every coding-agent pane — not only at attach time. The wording SHALL make clear that the proactive sweep runs on every iteration, while the reactive `[supervisor.auto_approve]` background poll thread remains a fallback for when the supervisor agent is offline or its iteration cadence is too slow.

2. The skill's "Rules" section SHALL contain a bullet stating that the supervisor agent absorbs routine approvals as part of its job, and that the human is the escalation audience for non-routine decisions. The bullet SHALL enumerate routine cases (dev-essential prompts: `git commit`, `cargo test|build|fmt|clippy`, `mdbook build`, `git stash`, `git restore`, common shell reads like `awk`, `grep`, `python3 -c '...'`) and non-routine cases (cross-agent conflicts requiring design judgement, scope/spec decisions, destructive operations outside an agent's own worktree, anything novel or surprising). The framing SHALL be that the supervisor is the rubber-stamp gate and the human is only invoked when judgement is required.

3. The skill SHALL NOT delete the existing launch-time sweep guidance (§1.5) or the `[supervisor.auto_approve]` poll-thread description; the continuous-iteration guidance complements both. The intent is that all three mechanisms (launch sweep, continuous sweep, reactive poll thread) cover the full lifecycle: launch sweep handles the first-few-seconds window before the supervisor's monitoring loop starts iterating, the continuous sweep covers steady-state operation, and the poll thread is a fallback when the supervisor agent itself is unavailable.

#### Scenario: Supervisor skill instructs continuous-iteration safe-command sweep

- **WHEN** the embedded supervisor skill's `Watch` step (or equivalent monitoring-loop section) is inspected
- **THEN** it explicitly instructs the supervisor agent to sweep every coding-agent pane on every iteration and apply the safe-command policy from the launch-time sweep section
- **AND** the wording SHALL distinguish the continuous sweep from the launch-time sweep and the reactive poll thread (all three coexist; the continuous sweep is the steady-state mechanism)

#### Scenario: Supervisor skill Rules section codifies routine-approval absorption

- **WHEN** the embedded supervisor skill's `Rules` section is inspected
- **THEN** it contains a bullet stating that the supervisor agent absorbs routine approvals and the human is the escalation audience
- **AND** the bullet enumerates routine dev-essential prompt categories (cargo / git / mdbook / shell-read families)
- **AND** the bullet enumerates non-routine cases that SHALL be escalated (cross-agent conflicts, scope/spec decisions, destructive ops outside a worktree, anything novel)

#### Scenario: Supervisor skill keeps the existing launch-time sweep and poll-thread guidance intact

- **WHEN** the embedded supervisor skill is inspected after this requirement is implemented
- **THEN** the existing §1.5 launch-time pane sweep guidance is still present
- **AND** the existing `[supervisor.auto_approve]` background poll thread description is still present
- **AND** the new continuous-sweep guidance complements (does NOT replace) either of the above

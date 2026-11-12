# Spec audit gate

Gate 3 of the orchestrator's five verification gates (testing, regression
analysis, spec audit, doc audit, security audit). This reference is
self-contained: read it on its own, run the gate, and report a verdict. It runs
only after the testing and regression-analysis gates have passed.

**Inputs.** The gate needs four things:

- **The re-resolved branch tip.** Re-resolve the branch's current tip with
  `git rev-parse <branch>` immediately before the gate runs, and read specs,
  code and tests from the isolated verify worktree checked out at that tip —
  never at the commit SHA carried by the event that triggered verification,
  which is already stale. A surface present at the tip MUST NOT be reported as
  MISSING.
- **The merge-base diff.** Assess the branch's own contribution against the
  **merge-base** of the tip and the integration target —
  `git -C "$VERIFY" diff "$(git merge-base <integration-target> "$TIP")...$TIP"`
  — never against a stale integration tip, which shows the integration tip's
  own commits as spurious mass deletions or additions.
- **The change id** being audited. Substitute it for `{{CHANGE_ID}}` wherever
  that placeholder appears in the gate command at the moment you run it.
- **The gate command:** `{{SPEC_VALIDATE_COMMAND}}`. When it renders as
  `(not configured)`, skip the tooling invocation; the manual audit below
  applies in any case.

**Run the gate command bare.** Run it exactly as rendered and read its exit
status directly. Never pipe it into another command (`| tail`, `| grep`,
`| head`) and never wrap it in an exit-code probe (`; echo $?`): a pipeline
reports the last command's status, so a failing gate reads as a pass.

**Findings.** Report each finding on its own line with the bracketed gate
prefix: `[spec audit] <requirement-name>: <gap description>`. The gate passes
when there are none. When you run this gate as a delegated context rather than
as the orchestrator, publish nothing — no `agent.verified`, no
`agent.feedback`: return the verdict (`spec audit`, `pass` or `fail`, one line
per finding) and no raw tool output; the orchestrator publishes.

## The gate

**Spec audit** — after tests pass and no regression, run the Spec Audit
Procedure below to verify the implementation matches the change's specs,
located per the session's **Spec layout** doctrine (see *Spec layout for
this session* under the Spec Audit Procedure). When
`{{SPEC_VALIDATE_COMMAND}}` is configured (i.e. does not render
as `(not configured)`), also run it as a tooling-aided pre-check;
substitute `{{CHANGE_ID}}` in the rendered command with the change name
being audited. Assess *what the change added* from the **merge-base** diff
(`git merge-base <integration-target> <tip>` — see the merge-base rule
above), NOT from a stale integration tip, so a behind-tip or rebased branch
does not show spurious mass deletions. **Skip this step if testing or
regression-analysis failed** — there is no point auditing code that does not
build or pass tests. Errors are reported as `[spec audit]
<requirement-name>: <gap description>`.

### Spec Audit Procedure

Before publishing `agent.verified` for an agent's branch, audit the implementation
against its specs.

**Spec layout for this session:** {{SPEC_PATH_DOCTRINE}}

1. **Locate specs** — using the layout above, find the change's spec files
   (per backend: the `specs/` subdirectory of an OpenSpec change, the
   `spec.md` of a Spec Kit feature, or the `paw_status: pending` Markdown
   file). Each carries requirements and scenarios.
2. **For each `#### Scenario:` block** — extract the WHEN/THEN assertions. Search the
   codebase for a test that exercises this scenario:
   ```bash
   grep -r "<key assertion from THEN clause>" tests/ <project source dirs>
   ```
   If no matching test is found, add to the gap list: "Scenario '<name>' has no test."
3. **For each `### Requirement:` block** — read the SHALL/MUST statements. Find the
   implementation file (from the change's file ownership in the proposal). Verify that
   struct field names, function signatures, and return types match the spec exactly.
   If a field is named differently, add to the gap list: "Requirement '<name>': field
   `X` should be `Y` per spec."
4. **Compile results** —
   - If the gap list is empty: spec audit passes. Include "spec audit clean" in the
     `agent.verified` message.
   - If gaps exist: publish `agent.feedback` with the gap list as the errors array.
     The agent must fix the gaps and re-publish `agent.artifact`.

#### Governance verification (sub-step of spec audit)

When the boot prompt contains a `## Governance documents` section listing project doc
paths, read each listed doc as part of the audit above and check the diff/branch
against it. This runs **inside** the Spec Audit Procedure — it is a sub-step of the
audit, not a separate workflow step. If the boot prompt has no `## Governance
documents` section, skip this sub-step entirely.

Per-doc examples (illustrative starting points, not exhaustive rubrics — apply
judgment based on the project's actual conventions, since these docs are owned by the
team's existing process, not by git-paw):

- **DoD** (e.g. `docs/dod.md`) — walk each `- [ ]` item against branch state.
  Example: an unchecked `- [ ] CHANGELOG.md updated` is a finding when the diff
  doesn't touch `CHANGELOG.md`.
- **ADRs** (e.g. `docs/adr/`) — scan the diff for new architectural decisions (new
  deps, new patterns) and verify a matching ADR exists. Example: a new `tokio`
  dependency warrants a matching ADR if the project's ADR convention covers deps.
- **Security** (e.g. `docs/security.md`) — walk each checklist item against the diff.
  Example: an item "validate user input" is a finding when a new HTTP handler has no
  input validation.
- **Test strategy** (e.g. `docs/test-strategy.md`) — check test composition matches
  the documented strategy. Example: a new public function with no accompanying test
  is a finding if the strategy requires tests for new public APIs.
- **Constitution** (e.g. `docs/constitution.md`) — check the diff against documented
  principles. Example: a principle "no panics in library code" is a finding when the
  diff introduces an `unwrap()` outside test code.

**Findings flow through `agent.feedback`.** Governance findings are surfaced as
standard `agent.feedback` errors, mixed in with other spec-audit findings in the same
errors array. There is no governance-specific tag prefix, no separate broker message
variant, and no per-doc enforcement category — a governance finding is an audit
finding, treated like any other.

**Missing-doc handling.** If a configured path doesn't resolve to a readable file in
the worktree, add an error to the same `agent.feedback` errors list noting the
missing path (e.g. `"configured DoD doc 'docs/dod.md' not found in worktree"`). Treat
it as a finding, not a distinct failure type.

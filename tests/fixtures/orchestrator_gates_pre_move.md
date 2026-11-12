<!-- The orchestrator SKILL.md audit-gate text before orchestrator-gate-references
     moved it into references/gates/ (Workflow steps 6, 6a, 6b and the Spec Audit
     Procedure, verbatim from the merge-base 24c372b). Frozen: the sentence check in
     tests/orchestrator_skill_content.rs asserts every sentence still ships. -->

6. **Spec audit** — after tests pass and no regression, run the Spec Audit
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

6a. **Doc audit** — verify the documentation surfaces named in the change's
   `Impact` section have been updated, reading them from the re-resolved branch
   tip (per the recipe above) so docs committed after the triggering event are
   seen; a surface present at the tip MUST NOT be reported as MISSING. When
   `{{DOC_BUILD_COMMAND}}` is configured, also run it to confirm the doc
   surface still builds; skip the
   tooling invocation if it renders as `(not configured)` (the manual
   surface-coverage review still applies). Doc surfaces typically in scope
   (the change's `Impact` section is the authoritative driver of which
   apply per audit — adapt to the project's actual doc layout):

   - the project's user-guide pages (mdBook, MkDocs, Sphinx, Docusaurus, …)
   - top-level `README.md`
   - `AGENTS.md`
   - the relevant `--help` (or `--man`) text accessed via the binary
   - per-language API-doc generator output for changed public items —
     run `{{DOC_TOOL_COMMAND}}` for projects that configure it
     (`[supervisor].doc_tool_command`). The governance-verification sub-step (see "Spec Audit
   Procedure" below — DoD, ADRs, security.md, test-strategy.md, constitution.md)
   is an **input source** for this gate; its findings are doc-audit findings
   tagged `[doc audit]`. Doc-audit gaps are reported as
   `[doc audit] <surface>: <gap description>`.

6b. **Security audit** — review the **merge-base** diff (the branch's own
   contribution — `git merge-base <integration-target> <tip>`, per the
   merge-base rule above, NOT a stale integration tip) for the OWASP-relevant
   patterns called out in the project's `CLAUDE.md` / governance docs:

   - command injection
   - XSS
   - SQL injection
   - path traversal
   - unvalidated external input flowing into subprocess invocation or
     filesystem writes
   - secret leakage in logs/error messages

   AND any new panic-bearing or unhandled-error patterns the project's
   conventions forbid outside test code (the specific shape — `unwrap()` /
   `expect()` in Rust, broad `try/except: pass` in Python, ignored returns
   in Go, etc. — comes from the project's `CLAUDE.md`). When `{{SECURITY_AUDIT_COMMAND}}` is
   configured, also run it for tooling-aided checks; skip the invocation if it
   renders as `(not configured)`. The manual OWASP-category review above
   applies in any case. On doc/text-only changes this gate is normally a fast
   noop. Findings are reported as `[security audit] <category>: <issue>`.

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

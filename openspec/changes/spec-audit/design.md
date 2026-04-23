## Context

This change adds instructions to the supervisor's skill template — it's Markdown content, not compiled code. The supervisor CLI (Claude, Codex, etc.) reads these instructions and executes them using its own capabilities: file reading, grep, code analysis. The audit quality depends on the supervisor CLI's ability to follow structured instructions.

This was validated during v0.3.0 dogfooding: the spec compliance audit was performed by Claude Code reading spec files and grepping the codebase. It caught 4 critical issues. The same process works as supervisor instructions.

## Goals / Non-Goals

**Goals:**

- Give the supervisor clear, actionable instructions for spec-to-code verification
- Cover the three types of gaps: untested scenarios, field/signature mismatches, missing implementations
- Make the audit output structured enough to include in `agent.feedback` messages

**Non-Goals:**

- Automated spec parsing in Rust (the CLI does the parsing by reading Markdown)
- Test execution (that's the test command's job — spec audit is about code review, not runtime behavior)
- Continuous audit (runs once per agent completion, not continuously)

## Decisions

### Decision 1: Audit instructions are structured as a checklist

The supervisor template's spec audit section is a numbered procedure:

```markdown
### Spec Audit Procedure

When an agent publishes `agent.artifact`, perform this audit before publishing `agent.verified`:

1. Find spec files: `ls openspec/changes/<change-name>/specs/`
2. For each spec file, read every `#### Scenario:` block
3. For each scenario:
   a. Extract the key assertion (the THEN clause)
   b. Search test files for a test that verifies this assertion:
      `grep -r "assertion_keyword" tests/ src/`
   c. If no test found → add to gap list: "Scenario X has no test"
4. For each `### Requirement:` block:
   a. Read the SHALL/MUST statements
   b. Find the implementation file (from the change's file ownership)
   c. Verify field names, function signatures, and types match exactly
   d. If mismatch → add to gap list: "Requirement X: field Y is Z but spec says W"
5. If gap list is empty → publish `agent.verified` with message "spec audit clean"
6. If gaps found → publish `agent.feedback` with the gap list as errors
```

**Why:**
- Numbered steps are unambiguous — the supervisor follows them in order
- The grep approach works because test names and assertions typically reference the same terms as spec scenarios
- Reading implementation files catches the field-name mismatches that v0.3.0's audit found
- The output format (gap list) maps directly to `agent.feedback`'s `errors: Vec<String>`

### Decision 2: No Rust code — purely template content

The spec audit is not a binary tool or a Rust function. It's instructions that any AI CLI can follow using its standard capabilities (file reading, grep, code understanding).

**Why:**
- Every supported CLI (Claude, Codex, Aider) can read files and search code
- A Rust tool would need to parse Markdown specs, understand test semantics, and make judgment calls about coverage — all things AI CLIs already do naturally
- The template approach means the audit evolves by editing Markdown, not recompiling
- User overrides work: a user can place a custom `supervisor.md` with different audit criteria

### Decision 3: Audit runs after test command, before verified

The supervisor workflow is:
1. Agent reports `agent.artifact` (done)
2. Supervisor runs test command → if tests fail, publish `agent.feedback` immediately
3. If tests pass, run spec audit → if gaps found, publish `agent.feedback`
4. If both pass, publish `agent.verified`

**Why:**
- No point auditing specs if tests don't even pass
- Test failures are faster to detect than spec audits (seconds vs minutes)
- The two-step verification (tests + audit) catches different classes of issues

## Risks / Trade-offs

- **Audit quality depends on CLI capability** → A weaker CLI might miss subtle field mismatches. **Mitigation:** the instructions are explicit ("verify field names match exactly"). Claude and Codex handle this well. Less capable CLIs may produce false positives/negatives.

- **Grep-based test discovery is heuristic** → A test might exist with a different name than the spec scenario suggests. **Mitigation:** the instructions say "search for the key assertion" not "search for the scenario name." The supervisor CLI uses judgment to match.

- **Audit adds time to the verification loop** → Each agent completion triggers a spec read + code search that might take 1-2 minutes. **Mitigation:** this runs once per agent, not continuously. For a 7-agent session, that's ~10-15 minutes of audit time total — acceptable.

## Migration Plan

No migration. Template content addition only.

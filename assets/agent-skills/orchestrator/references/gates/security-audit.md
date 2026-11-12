# Security audit gate

Gate 5 of the orchestrator's five verification gates (testing, regression
analysis, spec audit, doc audit, security audit). This reference is
self-contained: read it on its own, run the gate, and report a verdict. It runs
only after the testing and regression-analysis gates have passed.

**Inputs.** The gate needs four things:

- **The re-resolved branch tip.** Re-resolve the branch's current tip with
  `git rev-parse <branch>` immediately before the gate runs, and read the code
  from the isolated verify worktree checked out at that tip — never at the
  commit SHA carried by the event that triggered verification, which is
  already stale.
- **The merge-base diff.** Review the branch's own contribution against the
  **merge-base** of the tip and the integration target —
  `git -C "$VERIFY" diff "$(git merge-base <integration-target> "$TIP")...$TIP"`
  — never against a stale integration tip, which shows the integration tip's
  own commits as spurious mass deletions or additions.
- **The change id** being audited, whose specs and governance docs say which
  patterns the project forbids.
- **The gate command:** `{{SECURITY_AUDIT_COMMAND}}`. When it renders as
  `(not configured)`, skip the tooling invocation; the manual review below
  applies in any case.

**Run the gate command bare.** Run it exactly as rendered and read its exit
status directly. Never pipe it into another command (`| tail`, `| grep`,
`| head`) and never wrap it in an exit-code probe (`; echo $?`): a pipeline
reports the last command's status, so a failing gate reads as a pass.

**Findings.** Report each finding on its own line with the bracketed gate
prefix: `[security audit] <category>: <issue>`. The gate passes when there are
none. When you run this gate as a delegated context rather than as the
orchestrator, publish nothing — no `agent.verified`, no `agent.feedback`:
return the verdict (`security audit`, `pass` or `fail`, one line per finding)
and no raw tool output; the orchestrator publishes.

## The gate

**Security audit** — review the **merge-base** diff (the branch's own
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

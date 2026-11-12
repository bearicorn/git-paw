# Doc audit gate

Gate 4 of the orchestrator's five verification gates (testing, regression
analysis, spec audit, doc audit, security audit). This reference is
self-contained: read it on its own, run the gate, and report a verdict. It runs
only after the testing and regression-analysis gates have passed.

**Inputs.** The gate needs four things:

- **The re-resolved branch tip.** Re-resolve the branch's current tip with
  `git rev-parse <branch>` immediately before the gate runs, and read every
  documentation surface from the isolated verify worktree checked out at that
  tip — never at the commit SHA carried by the event that triggered
  verification, which is already stale. A surface present at the tip MUST NOT
  be reported as MISSING.
- **The merge-base diff.** Judge which surfaces the change touched from the
  branch's own contribution against the **merge-base** of the tip and the
  integration target —
  `git -C "$VERIFY" diff "$(git merge-base <integration-target> "$TIP")...$TIP"`
  — never against a stale integration tip.
- **The change id** being audited, whose `Impact` section names the
  documentation surfaces in scope.
- **The gate command:** `{{DOC_BUILD_COMMAND}}`. When it renders as
  `(not configured)`, skip the tooling invocation; the manual surface-coverage
  review below applies in any case.

**Run the gate command bare.** Run it exactly as rendered and read its exit
status directly. Never pipe it into another command (`| tail`, `| grep`,
`| head`) and never wrap it in an exit-code probe (`; echo $?`): a pipeline
reports the last command's status, so a failing gate reads as a pass.

**Findings.** Report each finding on its own line with the bracketed gate
prefix: `[doc audit] <surface>: <gap description>`. The gate passes when there
are none. When you run this gate as a delegated context rather than as the
orchestrator, publish nothing — no `agent.verified`, no `agent.feedback`:
return the verdict (`doc audit`, `pass` or `fail`, one line per finding) and no
raw tool output; the orchestrator publishes.

## The gate

**Doc audit** — verify the documentation surfaces named in the change's
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
  (`[supervisor].doc_tool_command`). The governance-verification sub-step
  (see "Spec Audit Procedure" in
  `.git-paw/skills/orchestrator/references/gates/spec-audit.md` — DoD, ADRs,
  security.md, test-strategy.md, constitution.md) is an **input source** for
  this gate; its findings are doc-audit findings tagged `[doc audit]`.
  Doc-audit gaps are reported as `[doc audit] <surface>: <gap description>`.

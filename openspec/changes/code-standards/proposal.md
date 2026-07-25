## Why

The W4 code-analysis audits produced a concrete set of decided patterns (decoupling seams,
domain module layout, frozen do-not-touch zones, refactor rules). git-paw should also hold to
established **Rust API Guidelines** and **CLI / dev-tool** best practices as it approaches the
v1.0.0 freeze. Codifying all of this as a discoverable, enforceable **agent skill** — the same
model as `test-strategy` — makes it durable: the implementing agent writes to it, the supervisor
gates against it, and `code-analysis-refactor` brings the existing code into conformance.

## What Changes

- Ship a **`code-standards` agent skill** at `.agents/skills/code-standards/` in the agentskills.io
  format: `SKILL.md` plus three `references/` files (progressive disclosure). It encodes git-paw's
  decided patterns (process-execution seam, domain newtypes, hidden-subcommand IPC, command-handler
  modules, module-domain layout), the non-negotiable idiom rules (no `unwrap`/`expect`, `PawError`,
  docs), the **frozen do-not-touch** zones, the refactor rules, and a supervisor review-gate
  checklist — plus condensed **Rust API Guidelines** (`references/rust-api-guidelines.md`) and
  **CLI/dev-tool guidelines** (`references/cli-and-devtool-design.md`), and the
  **non-functional-requirements** rationale (`references/non-functional-requirements.md`) — the NFR
  set, conflict resolutions, and precedence spine the standards serve.
- Add an **AGENTS.md pointer** naming the skill as the canonical reference for structuring,
  decoupling, and refactoring code.

Repo-local dev tooling — **not** exported. It is git-paw-specific, so it lives in `.agents/skills/`
(git-paw's own resolver's standard location), never in `assets/agent-skills/`.

## Capabilities

### New Capabilities
- `code-standards`: the repo's code-authoring standard — the skill artifact (SKILL.md + references),
  its agentskills.io conformance, and the contributor pointer.

### Modified Capabilities
_None._ No product behavior, CLI, config, or wire surface changes. (CI skill-validation is owned by
the `test-strategy` change's guard, which validates every `.agents/skills/*` skill including this one.)

## Impact

- **New:** `.agents/skills/code-standards/SKILL.md` + `references/{rust-api-guidelines,cli-and-devtool-design,non-functional-requirements}.md`
  (authored); an AGENTS.md pointer.
- **Sequencing:** available **early** so workers write to it across the cycle; `code-analysis-refactor`
  *applies* it; `standards-skill-integration` wires the worker + supervisor to *consult* it.
- **No code/behavior change**; no exported-asset change (agnosticism intact — dev skill).
- **Aspirational vs enforced:** the `CommandRunner` seam, the newtypes, and the `commands/`/`config/`/
  `tmux/` splits are named as *target* patterns the refactor introduces — not yet in the code.

# Design — code-standards

## Context

The W4 audits (`.git-paw/v0.13.0-wave3-code-analysis-*.md`) produced git-paw's decided code
patterns; the freeze also calls for holding to established external best practices. This change
codifies both as a repo-local dev skill so the worker, the supervisor, and `code-analysis-refactor`
all key off one source of truth.

## Decisions

### D1 — agentskills.io format at `.agents/skills/`, repo-local (not exported)

Authored per <https://agentskills.io/specification>: a directory with `SKILL.md` (`name` matching
folder, required frontmatter) at `.agents/skills/code-standards/`. It is **git-paw-specific dev
tooling** (Rust, `PawError`, cargo, the module map) so it must **not** go in `assets/agent-skills/`
(which `AGENTS.md` requires to stay project-agnostic). `.agents/skills/` in git-paw's own repo is
git-paw's own tooling and may be git-paw-specific.

### D2 — Content = decided patterns + grounded external best practices

`SKILL.md` carries git-paw's decided patterns and rules. Two `references/` files carry condensed,
**sourced** best practices — `rust-api-guidelines.md` (from the official Rust API Guidelines
checklist) and `cli-and-devtool-design.md` (from clig.dev) — each annotated with git-paw
applicability. This uses the spec's **progressive-disclosure** model: `SKILL.md` < 500 lines, detail
in `references/` loaded on demand.

### D3 — Aspirational patterns are named as targets

The `CommandRunner` seam, the domain newtypes, and the `commands/`/`config/`/`tmux/` splits do not
exist in the code yet. The skill names them as the **target** patterns; `code-analysis-refactor`
introduces them. The skill exists **early** so new code is written toward them, not away from them.

### D4 — Validation + consumption live in sibling changes

CI conformance is owned by the `test-strategy` change's guard (it validates every `.agents/skills/*`
skill, including this one). Consumption — the worker and supervisor actually reading these standards
— is `standards-skill-integration`. This change only authors the skill + the AGENTS.md pointer.

### D5 — Enforced-now vs aspirational

Some items bind today (no `unwrap`/`expect`, docs, `PawError`, the frozen do-not-touch list); others
are targets (the seams/splits). The skill marks the difference so the review gate does not fail a
change for not-yet-existing structure.

## Non-goals

- Not exported / not shipped by `git paw init`.
- Not the refactor itself (`code-analysis-refactor`) nor the consumption wiring
  (`standards-skill-integration`).
- No product behavior/CLI/config/wire change.

## Risks

- **Low.** Docs/dev-tooling only. The main risk is the skill drifting from reality as the refactor
  lands — mitigated by keeping the aspirational items clearly marked and updating them as the
  `commands/`/`config/`/`tmux/` splits and the `CommandRunner` seam ship.

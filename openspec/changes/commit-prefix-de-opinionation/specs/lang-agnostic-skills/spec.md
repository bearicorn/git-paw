## ADDED Requirements

### Requirement: Bundled skills are convention-agnostic

Bundled skills the binary exports SHALL be project-agnostic with respect to
per-project *conventions*, not only implementation *stack*. This covers the
bundled supervisor and coordination skills and any other asset `git paw init`
installs into a consumer repo. Specifically, a bundled skill SHALL NOT mandate,
default to, or present as its recommendation a project-specific
commit-message convention (e.g. a Conventional-Commits `feat(<scope>):` /
`fix(<scope>):` prefix). Commit-message format, like governance docs and
stack-specific commands, is a property of the *consumer's* project and SHALL
be deferred to the consumer's injected `AGENTS.md`.

This is the general principle behind the separation of "what the binary
exports" (must be generic) from "what is git-paw-repo-specific" (lives in
git-paw's own `AGENTS.md` / `CLAUDE.md` / `cliff.toml`). git-paw's own repo
MAY (and does) require Conventional Commits — but only via its own injected
`AGENTS.md`, never via the bundled skill the binary ships to others.

The bundled-skill leak audit (the same audit that enforces the "No
language-leak audit" and "Tone and example discipline" requirements) SHALL
additionally flag a bundled skill that hardcodes a project-specific
commit-message convention as a mandate, default, or recommendation.

#### Scenario: Leak audit flags a hardcoded commit-convention mandate

- **WHEN** the bundled-skill leak audit renders the supervisor and
  coordination skills with empty substitutions and inspects the output
- **THEN** the audit SHALL fail if a skill mandates, defaults to, or
  presents as its recommendation a Conventional-Commits prefix
  (`feat(<scope>):`, `fix(<scope>):`, …) as the commit-message format
- **AND** the audit SHALL pass when the skill defers commit-message format to
  the consumer's `AGENTS.md` and uses only format-neutral commit examples

#### Scenario: git-paw's own Conventional-Commits convention is not in the exported asset

- **WHEN** the exported `assets/agent-skills/coordination.md` is inspected
- **THEN** it SHALL NOT contain git-paw's Conventional-Commits convention as a
  rule, default, or recommendation
- **AND** git-paw's repo-specific Conventional-Commits convention SHALL remain
  only in git-paw's own injected `AGENTS.md` / `CLAUDE.md` (which the binary
  does not export to consumers)

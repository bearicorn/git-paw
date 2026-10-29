## Why

`README.md` (372 lines) is the front door, but it reads like a changelog: a ~53-line `## Features`
section of 39 bullets (many multi-sentence, several version-stamped, several describing individual
config tables such as `[supervisor.conflict]` and `[supervisor.learnings_config]`), three quick starts, a
config excerpt, a "How It Works" section and release notes — all of which the mdBook user guide, the CLI
reference and the configuration reference already cover. Landing the v1.0.0 freeze with a bloated front
door undercuts the "stable, polished" moment.

This is mostly a docs task, but not a spec-free one: three requirements **mandate** the bloat —
`core-project-conventions` requires the Features section to enumerate `--specs-format`,
`--no-supervisor`, `start --force`, `[supervisor.conflict]`, `[supervisor.learnings_config]`, … ;
`supervisor-launch` requires a README "Quick Start: Supervisor Mode" section with `--no-supervisor` and
`start --force` examples; `spec-providers` requires a README `[specs]` config excerpt. Slimming the README
without changing them would leave SHALLs the docs contradict. (OpenSpec has no spec-less change path
other than archiving with `--skip-specs`; here the deltas are real.)

## What Changes

- **README becomes a landing page**: pitch (what git-paw is, why it exists), demo, installation, one
  minimal quick start, a one-line-per-command table, the Supported AI CLIs list, the security
  disclaimer, a short capability-level feature list where each bullet links to its user-guide chapter,
  and pointers to the docs site for everything else.
- Removed from the README: version-stamped feature prose, per-config-table descriptions, the config
  excerpt, the extra quick starts, "How It Works" internals and release-process text (all already in the
  docs; anything not yet there moves there first).
- Spec deltas: the README-enumeration requirement is replaced by a landing-page requirement with a
  standing guard; the README supervisor quick-start and README `[specs]` excerpt requirements are
  removed (their content is already required of the mdBook quick-start chapter and the configuration
  reference).
- The `doc-completeness` dev skill's "README feature entry for every new capability" rule becomes "README
  entry only for a new headline capability; the command table for every new command".

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `core-project-conventions`: REMOVED **README features section enumerates the current user-facing
  surface**; ADDED **README is a landing page that forwards detail to the documentation**.
- `supervisor-launch`: REMOVED **README Supervisor Mode quick start documents the flags**.
- `spec-providers`: REMOVED **README documents `[specs] type` accepts all three backends**.

## Impact

- **Files:** `README.md`; any mdBook chapter that must absorb README-only content first (checked
  section by section; `docs/src/introduction.md` takes the longer pitch); `.agents/skills/doc-completeness/SKILL.md`
  (README rule); a new guard `tests/readme_landing_page.rs`. Existing README guards stay green:
  `tests/security_posture_docs.rs` (`## Security`, "arbitrary code", "sandbox"),
  `tests/cli_dx_docs.rs` (`attach`, `completions` in the command table; no `paw resume`).
- **Code:** none.
- **Enum-variant ripple:** none.
- **Cross-version dependencies:** v0.17 `classifier-retirement` MODIFIES the README-enumeration
  requirement this change REMOVES (it drops `[supervisor.auto_approve]` from the list and adds "the
  Features list SHALL NOT present automatic approval of permission prompts as a feature"); the REMOVED
  migration text follows its final list, and the no-auto-approval rule is carried into the landing-page
  requirement so archiving here does not drop it. v0.18 `supervisor-pane-removal` and `orchestrator-skill-export` rewrite
  the README's supervisor bullets and quick start (relocation framing) — this change lands after them and
  replaces that section with a pointer. Every other v0.21 change in this set adds at most one README
  command-table row (`finish`, `config check`) — consistent with the new rule. The `cli-resolution`
  "README Supported AI CLIs table matches `src/detect.rs`" and `core-security-posture` disclaimer
  requirements are kept as they are.

## Rebase note (2026-10-19)

This change uses the pre-v0.21 supervisor vocabulary. If it archives after
`supervisor-vocabulary-split`, the user-facing text it adds (the README, docs pointers, guard
messages) SHALL use the new names (`--coordinated` / `--bare`, `[orchestrator]` / `[workers]` /
`[drive]`, `default_mode`). Its deltas are not rewritten: they archive against today's names, which
that change's reading rule maps and v1.0.0's `remove-supervisor-vocabulary` sweep renames (including
the REMOVED header *README Supervisor Mode quick start documents the flags* in `supervisor-launch`).
Old terms used here:

- `--no-supervisor` (Why, REMOVED Migration texts) → `--bare`
- `[supervisor.conflict]`, `[supervisor.learnings_config]` → `[drive.conflict]`,
  `[drive.learnings]`
- the README guard's forbidden substring `[supervisor.` → also forbid the new table prefixes
  (`[orchestrator.`, `[workers.`, `[drive.`), which carry the same per-table detail
- "Quick Start: Supervisor Mode", "supervised quick-start chapter"
  (`docs/src/quick-start-supervisor.md`), "the README's supervisor bullets" → coordinated mode

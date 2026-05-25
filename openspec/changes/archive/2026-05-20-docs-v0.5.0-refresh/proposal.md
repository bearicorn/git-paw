## Why

`feat/v0.5.0-specs` shipped 13 archived OpenSpec changes (3 on
2026-05-11, 10 on 2026-05-13) covering forward coordination,
conflict detection, learnings mode, governance, Spec Kit support,
`--no-supervisor`, `start --force`, cross-format spec selection,
supervisor-as-pane, the v0.4 hardening pass, plus three launch-time
fixes (boot-prompt-full-body, prompt-submit, worktree-resume). All
of that landed in code and is covered by main specs. The
user-facing surface — README.md, AGENTS.md, the mdBook chapters
under `docs/src/`, the `--help` text, and the configuration
reference — has NOT kept pace.

Three concurrent audit passes (one on README.md, one on AGENTS.md,
one on docs/src/) surfaced ~30 concrete drift findings. They fall
into four buckets:

1. **Internal contradictions** — e.g. `quick-start-supervisor.md:57`
   says "dashboard pane 0, supervisor pane 1" but `:102` says
   "Dashboard pane (pane 0)"; the supervisor-as-pane archive
   established the actual layout as supervisor pane 0, dashboard
   pane 1.
2. **References to nonexistent surface** — `architecture.md` lists
   `src/broker/state.rs` and `src/broker/flush.rs` (don't exist) and
   omits 8+ modules that do (`src/supervisor/*`,
   `src/broker/{conflict,learnings,watcher,delivery,publish}.rs`,
   `src/specs/{resolve,speckit}.rs`); `quick-start-supervisor.md`
   references broker message types `agent.register` and `agent.done`
   that don't exist in `src/broker/messages.rs`;
   `coordination.md:121-135` uses a legacy
   `{"agent_id":..., "kind":..., "body":...}` wire shape that the
   broker has never accepted in v0.4 or v0.5.
3. **Deferred-to-shipped omissions** — features that
   `quick-start-supervisor.md` lists under "What's NOT yet supported
   in v0.4.0" (conflict detection, learnings mode) shipped in
   v0.5.0 and are documented nowhere user-facing;
   `--no-supervisor`, `--specs-format`, `start --force`, Spec Kit
   backend, `--from-all-specs`, `[governance]`,
   `[supervisor.conflict]`, `[supervisor.auto_approve]`,
   `[supervisor.learnings_config]` config tables and CLI flags ship
   but are missing from the Features section, the CLI reference
   table, and the configuration reference.
4. **AGENTS.md drift** — the approved dependency list omits four
   v0.5.0 production crates (`schemars`, `serde_yaml`, `chrono`,
   `regex`); lists `dirs` as approved despite that crate having been
   removed in v0.5.0 for license reasons and replaced with a
   homegrown `src/dirs.rs`; the conventional-commits scope list does
   not include scopes used in shipped v0.5.0 commits (`user-guide`,
   `worktree`, `governance`, `learnings`, `pause`, and compound
   forms like `feat(specs,skills,init)` and
   `fix(broker,skills,agents,git)`).

These docs are the only contract a user reads before adopting
v0.5.0. They are wrong in user-blocking ways. This change
catches all of them up in one focused pass.

## What Changes

Documentation-only refresh; no Rust code touched, no `Cargo.toml`
edits, no skill content changes. Specifically:

### README.md (root)

- Features section gains entries for `--specs-format`,
  `--no-supervisor`, `start --force`, the Spec Kit backend,
  `[governance]`, `[supervisor.conflict]`,
  `[supervisor.auto_approve]`, `[supervisor.learnings_config]`,
  `agent.intent`, automatic conflict detection, and learnings mode
  (`.git-paw/session-learnings.md`).
- Quick Start: Supervisor Mode section adds `--no-supervisor` and
  `start --force` examples.
- `[specs]` example expands `type` to list all three backends:
  `"openspec"`, `"markdown"`, `"speckit"`.
- Supported AI CLIs table grows from the v0.4 set of 7 entries to
  the v0.5 set of 10 (`opencode`, `cline`, `droid` added, matching
  `src/detect.rs`).
- `start --force` flag added to the documented start-flag list.

### AGENTS.md (root)

- Dependency table gains `schemars` (0.8), `serde_yaml` (0.9),
  `chrono` (0.4), `regex` (1) for production deps; dev deps
  updated to match `Cargo.toml`.
- `dirs` line is replaced with a short paragraph documenting that
  the upstream `dirs` crate is intentionally NOT a dep — the
  upstream crate has a non-FOSS license that fails `just deny`, so
  v0.5.0 replaced it with a homegrown `src/dirs.rs`. Future
  contributors are told NOT to re-add `dirs`.
- Conventional-commit scope list grows to include the v0.5.0 scopes
  actually used in shipped commits (`user-guide`, `worktree`,
  `governance`, `learnings`, `pause`, etc., reconciled from
  `git log feat/v0.5.0-specs`) AND explicitly permits compound
  scopes of the form `<scope1>,<scope2>,<scope3>` (e.g.
  `feat(specs,skills,init): …`).

### docs/src/ — existing chapters refreshed

- **`architecture.md`** — full module-list refresh. Remove
  references to `src/broker/state.rs` and `src/broker/flush.rs`
  (don't exist). Add `src/supervisor/{boot,governance,layout,...}`,
  `src/broker/{conflict,learnings,watcher,delivery,publish}.rs`,
  `src/specs/{resolve,speckit}.rs`. Update the layout description
  for supervisor mode to the 50/50 split (top row =
  pane 0 supervisor + pane 1 dashboard; agent grid below).
- **`changelog.md`** — replace the placeholder `[Unreleased]`
  content with `{{#include ../../CHANGELOG.md}}` so future cycles
  flow through automatically.
- **`quick-start-supervisor.md`** — fix the internal contradiction
  (supervisor pane 0, dashboard pane 1, everywhere); remove the
  broker message types `agent.register` and `agent.done` that don't
  exist; delete the "What's NOT Yet Supported in v0.4.0" section
  (conflict detection and learnings mode shipped in v0.5.0); add
  the v0.5 wire-format curl examples mirroring `coordination.md`.
- **`user-guide/coordination.md`** — replace the legacy
  `{"agent_id":..., "kind":..., "body":...}` shape with the
  canonical `{"type":"agent.<kind>", "agent_id":"<slug>", "payload":{...}}`
  shape from `src/broker/messages.rs`; replace slashed `agent_id`
  examples (e.g. `"feat/auth"`) with slug-validated forms
  (`"feat-auth"`); add wire-form sections for `agent.intent`,
  `agent.question`, `agent.feedback`, and `agent.verified`.
- **`user-guide/spec-driven-launch.md`** — replace the hidden
  `--from-specs` alias references in examples (lines 153, 159,
  162) with `--from-all-specs`; align with the line 53 statement
  that the alias is intentionally undocumented. Add a Spec Kit
  backend subsection with a worked example and the
  `[specs] type = "speckit"` config.
- **`user-guide/agents-md.md`** — update the description of
  AGENTS.md generation to reflect boot-prompt-full-body
  (supervisor-mode boot prompts now point at AGENTS.md +
  `openspec/changes/<id>/` rather than embedding the spec body).
- **`user-guide/dashboard.md`** — remove the "v0.4 will add
  interactive prompt inbox" forward reference (v0.4 shipped it,
  supervisor-as-pane removed it again in v0.5.0); update to
  reflect that the dashboard pane is at index 1 in supervisor mode
  and shows the agents table without an inbox panel.
- **`configuration/README.md`** — extend the commented `[specs]
  type` example to include `"speckit"`; fix the dangling anchor
  link to `coordination.md#automatic-conflict-detection-v050`
  (either point at a real anchor that now exists in
  `coordination.md`, or remove the link); add subsections
  documenting `[supervisor.conflict]`, `[supervisor.auto_approve]`,
  `[supervisor.learnings_config]`, and `[governance]`.
- **`cli-reference.md`** — top-level commands list grows to
  include `init` and `replay` (currently omitted); start-flag
  table gains `--specs-format` (currently omitted), plus the
  v0.5.0 additions (`--from-all-specs`, `--specs`,
  `--no-supervisor`, `--force`).

### docs/src/ — new chapters (2)

- **`user-guide/learnings.md`** — opt-in flag, format of
  `.git-paw/session-learnings.md`, the 5 deterministic categories
  (stuck duration, recovery-cycle count, forward conflicts,
  in-flight conflicts, ownership violations) sourced from the
  `learnings-mode` archive's proposal.
- **`user-guide/conflict-detection.md`** — forward / in-flight /
  ownership shapes; the `[conflict-detector]` tag; supervisor
  inbox routing; how it interacts with `agent.intent` and the
  filesystem watcher's auto-published `modified_files`.

`SUMMARY.md` gains the two new chapter entries under User Guide.

### Out of scope

- Rustdoc / API doc generation. The change touches user-facing
  Markdown only; `just api-docs` output is regenerated mechanically
  and doesn't need spec coverage.
- Translations / localised docs. v0.5.0 is English-only.
- Any change to `--help` strings beyond what is needed to keep them
  consistent with the README CLI table. Long-form copy stays in
  Markdown.
- Re-running `mdbook build`. The CI gate runs it; this change
  defines the content, not the build verification.
- The `MILESTONE.md` v0.5.0 implementation status table. It is
  developer notes, not a user-facing doc; out of scope.
- Hidden alias rationale callouts. The proposal text above states
  the alias is intentionally undocumented; that decision is
  already final per the `cross-format-spec-selection` archive.

## Capabilities

### New Capabilities

- `user-documentation`: the structural and content requirements for
  README.md, AGENTS.md, and the mdBook user guide so that they
  accurately describe the v0.5.0 user-facing surface. The capability
  is doc-only — it has no runtime behaviour.

### Modified Capabilities

*(none — implementation capabilities are unchanged by a doc-only
change; existing main specs in `openspec/specs/` already describe
the shipped v0.5.0 behaviour.)*

## Impact

**Code**: none. This change is doc-only.

**Specs**: one new main spec under
`openspec/specs/user-documentation/` (created on archive) covering
the requirements listed in the delta below.

**Tests**: none in the Rust test suite — the
`user-documentation` capability is verified by `mdbook build`
succeeding and by the existing CI link-check on Markdown anchors.
Per-file checkbox verification lives in `tasks.md`.

**Backward compatibility**: doc-only refresh. Every existing v0.4
or v0.5 invocation continues to work; the change only fills in
what the docs failed to say. Users following the v0.4 docs were
not relying on absent behaviour, so no behavioural compatibility
concern.

**Mismatches resolved** (audit findings absorbed):

- README: `[specs] type` listing only `"openspec"` / `"markdown"`
  (README.md:311) — resolved.
- README: Supervisor Mode missing `--no-supervisor` / `start --force`
  (README.md:122-130) — resolved.
- README: Features section missing the v0.5.0 surface above —
  resolved.
- README: Supported AI CLIs table out of sync with `src/detect.rs`
  — resolved.
- AGENTS.md: missing `schemars` / `serde_yaml` / `chrono` / `regex`
  in dep table — resolved; `dirs` license-driven swap documented.
- AGENTS.md: scopes list missing v0.5.0 scopes and compound forms
  — resolved.
- mdBook `architecture.md` referencing nonexistent modules and
  missing 8+ new modules — resolved.
- mdBook `quick-start-supervisor.md` internal contradiction on
  pane indices, nonexistent broker message types
  `agent.register`/`agent.done`, deferred-to-shipped misstatements
  — all resolved.
- mdBook `coordination.md` legacy wire shape, slashed `agent_id`,
  missing `agent.intent`/`agent.question`/`agent.feedback`/
  `agent.verified` examples — resolved.
- mdBook `spec-driven-launch.md` `--from-specs` examples
  contradicting the line-53 alias statement — resolved.
- mdBook `agents-md.md` description not reflecting
  boot-prompt-full-body — resolved.
- mdBook `dashboard.md` referencing v0.4-deferred features that
  shipped — resolved.
- mdBook `configuration/README.md` missing `"speckit"`, dangling
  anchor link, missing `[supervisor.learnings_config]` /
  `[supervisor.conflict]` / `[supervisor.auto_approve]` /
  `[governance]` subsections — resolved.
- mdBook `cli-reference.md` missing `init`, `replay`, and
  `--specs-format` — resolved.
- mdBook `changelog.md` placeholder content — replaced with a
  `{{#include}}` of the real `CHANGELOG.md`.
- mdBook missing user-guide chapters for learnings and conflict
  detection — added.

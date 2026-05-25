## Context

13 v0.5.0 changes shipped on `feat/v0.5.0-specs`; the Rust code
is correct and the main specs are correct, but the user-facing
docs (README.md, AGENTS.md, `docs/src/**`) lag the
implementation. Three concurrent audits surfaced ~30 concrete
drift findings. This document records the design decisions that
shape the refresh — what content goes where, which mechanisms
keep future cycles from re-drifting, and which spec doctrine the
doc copy must reflect.

The decisions are doc-content decisions, not Rust-API decisions.
No code is touched.

## Goals / Non-Goals

**Goals.**

- Eliminate every audit finding in a single coherent refresh so
  the v0.5.0 user surface is the source of truth.
- Pin doc-content rules (wire-format shape, pane index, supported
  CLI count) so future audits can mechanically detect the same
  shape of drift if it recurs.
- Keep the changelog mechanism low-effort going forward — one
  `git cliff` source of truth, surfaced via mdBook include.
- Reflect the post-supervisor-as-pane reality in every chapter
  that references pane indices, the dashboard's role, or the
  user-supervisor interaction model.

**Non-goals.**

- Rewriting the API rustdoc. Out of scope — that's a separate
  contributor surface.
- Adding new user-facing features. Doc-only.
- Per-language localisation, screenshots, asciinema-cast
  regeneration. Out of scope; the textual content is what's
  diverged.
- Changing `--help` long_about strings beyond what's needed to
  keep them consistent with the new README table. Long-form copy
  stays in Markdown.

## Decisions

### D1: Changelog chapter rewired to `{{#include ../../CHANGELOG.md}}`

**What.** Replace the static `[Unreleased]` content in
`docs/src/changelog.md` with a single line:

    {{#include ../../CHANGELOG.md}}

**Why.** v0.5.0's release process already regenerates
`CHANGELOG.md` via `just changelog vX.Y.Z` (which runs
`git cliff --tag vX.Y.Z -o CHANGELOG.md`). The mdBook chapter is
a second copy that nobody updates — it's been frozen on
v0.1.0-era content through three releases. mdBook's `{{#include}}`
preprocessor copies file contents at build time, so the next
release-prep commit on main automatically refreshes the rendered
mdBook page with no extra editorial step.

**Alternatives considered.**

- Hand-maintain the chapter to mirror `CHANGELOG.md` at each
  release. Rejected: every prior release skipped this step. The
  pattern doesn't survive contact with the release process.
- Auto-generate during `mdbook build` via a custom preprocessor.
  Rejected: `{{#include}}` is built-in, runs at build time, and
  needs zero configuration. A custom preprocessor would be more
  surface area for the same outcome.
- Symlink `CHANGELOG.md` into `docs/src/`. Rejected: symlinks
  don't survive `cargo package` neatly and produce a confusing
  contributor experience.

**Constraint to verify.** `CHANGELOG.md` headings must not clash
with the surrounding mdBook navigation styling. Per
`git cliff`'s default keepachangelog template the file starts
with `# Changelog` — that becomes the chapter's single `<h1>`
which is what mdBook expects.

### D2: Architecture chapter — full module-list refresh + pinned layout diagram

**What.** Rewrite `docs/src/architecture.md` so:

1. The module table lists every Rust source module currently in
   `src/`, removing references to `src/broker/state.rs` and
   `src/broker/flush.rs` (don't exist) and adding the 8+ modules
   that landed in v0.5.0:
   - `src/supervisor/` (whole subtree: `boot.rs`, `governance.rs`,
     `layout.rs`, etc. — list per the source tree at the time of
     the refresh)
   - `src/broker/conflict.rs`
   - `src/broker/learnings.rs`
   - `src/broker/watcher.rs`
   - `src/broker/delivery.rs`
   - `src/broker/publish.rs`
   - `src/specs/resolve.rs`
   - `src/specs/speckit.rs`
2. The supervisor-mode layout diagram is pinned to the 50/50
   top-row shape established in the `supervisor-as-pane` archive:

       ┌──────────────────────────┬──────────────────────────┐
       │  pane 0: supervisor      │  pane 1: dashboard       │
       ├──┬──┬──┬──┬──┬───────────┴──────────────────────────┤
       │ 2│ 3│ 4│ 5│ 6│  agent grid (row 1)                  │
       ├──┴──┴──┴──┴──┤                                      │
       │ 7│..│..│..│ N│  agent grid (row 2..M)               │
       └──┴──┴──┴──┴──┴──────────────────────────────────────┘

   Plus the row-height proportion table from
   `2026-05-13-supervisor-as-pane/proposal.md` (2 rows = 60/40,
   3 rows = 40/30/30, etc.).
3. The non-supervisor layout (no top row, dashboard absent, just
   the agent grid) is shown separately so users don't conflate
   the two.

**Why.** Without these fixes the architecture chapter actively
misleads contributors: paths it lists don't exist on disk, and the
layout diagram it shows ("dashboard at pane 0") is wrong for the
default v0.5 supervisor flow. A contributor reading
`architecture.md` would build a wrong mental model on first read.

**Alternatives considered.**

- Auto-generate the module list from `cargo metadata` or
  `tree src/`. Rejected for v0.5.0: tooling cost > benefit, and
  the module-list change is rare enough that manual maintenance
  with a clear refresh point per release is acceptable. Revisit
  for v1.0.0 if churn justifies it.
- Inline the layout diagram inside `quick-start-supervisor.md`
  only. Rejected: contributors looking at architecture would
  miss it.

### D3: Wire-format examples — canonical envelope shape everywhere

**What.** Every curl example in user-facing docs SHALL use the
canonical wire shape that `src/broker/messages.rs` validates:

    {
      "type": "agent.<kind>",
      "agent_id": "<slug>",
      "payload": { ... }
    }

with `<slug>` matching the `agent_id` slug validation rules (no
slashes, no whitespace, lowercase alphanumeric + hyphen). Affects:

- `docs/src/user-guide/coordination.md` (currently uses the
  legacy `{"agent_id":..., "kind":..., "body":...}` shape).
- `docs/src/quick-start-supervisor.md` (currently uses
  invented broker message types `agent.register` and
  `agent.done`).
- `docs/src/user-guide/conflict-detection.md` (new chapter).
- `docs/src/user-guide/learnings.md` (new chapter, where it
  references the existing `agent.feedback` flow).
- README.md (Quick Start: Supervisor Mode supervisor curl
  examples).

Slugs in examples MUST be slug-valid. Anywhere a doc currently
shows `"feat/auth"` as an `agent_id`, it SHALL be replaced with
the slug-valid form `"feat-auth"`.

The doc SHALL include one example per shipped variant
(`agent.status`, `agent.artifact`, `agent.blocked`, `agent.intent`,
`agent.question`, `agent.feedback`, `agent.verified`) and an
explicit note that `agent.status` and `agent.artifact` are normally
published automatically (the watcher / post-commit hook) and the
manual curl examples for those two are escape hatches.

**Why.** The legacy `kind`/`body` shape has never been the
runtime wire format — it's an early draft that the
`broker-messages` capability spec replaced in v0.3. An agent
copying the v0.4 coordination chapter's example into a curl
verbatim would get a 400 response. The slashed `agent_id` would
trip slug validation (which routes via `delivery.rs`'s slug
sanitiser) and silently mis-route.

**Mechanism to keep this pinned.** The
`user-documentation` spec scenario "Coordination chapter uses the
v0.5 wire envelope" asserts substring presence of
`"type": "agent.` and absence of `"kind":`/`"body":` in the doc
content; future drift triggers a spec violation on archive.

### D4: Spec Kit backend documented in `spec-driven-launch.md`

**What.** Add a `## Spec Kit` section to
`docs/src/user-guide/spec-driven-launch.md` that:

1. States `[specs] type = "speckit"` is now a first-class
   selection alongside `"openspec"` and `"markdown"`.
2. Documents the auto-detection: when `.specify/` exists at the
   repo root and no `[specs]` table is set, the system defaults
   to `type = "speckit"`, `dir = ".specify/specs"`.
3. Shows a minimal worked example: `.specify/specs/003-user-list/`
   with `spec.md`, `plan.md`, `tasks.md`; how `[P]` markers
   decompose into per-task worktrees; how non-`[P]` markers
   consolidate into one `phase/...` worktree.
4. Notes the constitution auto-wiring handshake into
   `[governance]` (one sentence, link to
   `configuration/README.md#governance` for the slot).

The `[specs] type` example in `configuration/README.md` (currently
showing only `"openspec"` and `"markdown"`) gains the `"speckit"`
value.

**Why.** Spec Kit support is a major v0.5.0 feature; users
running GitHub Spec Kit projects won't discover it without docs.
The auto-detection behaviour is friendly but invisible — without
docs, users on `.specify/` projects wouldn't know `git paw` had
configured itself.

**Constraint.** The worked example uses a synthetic project name
(e.g. `my-app`) not a real project's. Keep the example self-
contained in the chapter; don't link out to `github.com/...` —
upstream Spec Kit URLs are owned by GitHub and could move.

### D5: Governance, learnings, conflict-detection — one new user-guide chapter per topic

**What.**

- `docs/src/user-guide/learnings.md` — explain the opt-in flag
  `[supervisor] learnings = true`, the location and append-only
  shape of `.git-paw/session-learnings.md`, and the five
  deterministic categories (stuck duration, recovery-cycle
  count, forward conflicts, in-flight conflicts, ownership
  violations) sourced from the `learnings-mode` archive's
  proposal. Note that `agent.learning` (the broker variant) is
  deferred to v0.6.0 — current users get the markdown file only.
- `docs/src/user-guide/conflict-detection.md` — explain the
  three failure shapes (forward / in-flight / ownership), the
  `[conflict-detector]` tag prefix, how the supervisor inbox
  receives `agent.question` escalations, and how it interacts
  with the filesystem watcher's auto-published `modified_files`.
- `docs/src/user-guide/governance.md` already exists per
  `SUMMARY.md`. Confirm it covers the `[governance]` table with
  all five fields (`adr`, `test_strategy`, `security`, `dod`,
  `constitution`) and the constitution auto-wiring with
  `spec-kit-format`. If not, extend it. Do NOT add a new
  governance chapter.

In `docs/src/configuration/README.md` add sub-sections for:

- `[supervisor.conflict]` — `window_seconds` (default 120),
  `warn_on_intent_overlap` (default true),
  `escalate_on_violation` (default true).
- `[supervisor.auto_approve]` — `enabled`, `stall_threshold_seconds`,
  `approval_level` (`safe`/`conservative`/`off`), `safe_commands`.
- `[supervisor.learnings_config]` — `flush_interval_seconds`
  (default 60). Note: the parent `[supervisor] learnings = true`
  flag is what activates the subsystem; this sub-table just tunes
  the flush cadence.
- `[governance]` — the five `Option<PathBuf>` fields plus the
  Spec Kit constitution auto-wiring note.

`SUMMARY.md` gets two new entries under User Guide for the new
chapters (between `Governance` and the bottom of the User Guide
group):

    - [Learnings Mode](user-guide/learnings.md)
    - [Conflict Detection](user-guide/conflict-detection.md)

**Why.** Each topic is a self-contained user-facing feature with
its own opt-in surface; readers should be able to jump straight
to "how do I turn this on, what file does it produce, what do I
read". Folding any of them into an existing chapter loses
discoverability via the `SUMMARY.md` table-of-contents and via
mdBook's full-text search.

### D6: AGENTS.md dependency table — full reconcile + `dirs` swap call-out

**What.**

1. Reconcile the AGENTS.md dependency table against the actual
   `[dependencies]` and `[dev-dependencies]` sections of
   `Cargo.toml` at the time of the refresh. Specifically add
   missing v0.5.0 prod deps:
   - `schemars` (0.8) — TOML schema generation for `git paw
     init` (or wherever it's used at archive time).
   - `serde_yaml` (0.9) — Spec Kit `tasks.md` frontmatter parsing.
   - `chrono` (0.4) — timestamps for the learnings file.
   - `regex` (1) — pattern matching in agents.rs and broker.
2. Replace the `dirs` row with a short paragraph (still under
   the "Dependencies" heading) explaining that the upstream
   `dirs` crate was intentionally removed in v0.5.0 because its
   transitive license chain failed `just deny`. The project now
   uses `src/dirs.rs` for platform XDG paths. Future
   contributors are instructed NOT to re-add the `dirs` crate.

The paragraph format mirrors the existing AGENTS.md voice — a
single short prose explanation, no `> NOTE:` blocks, no emoji.

**Why.** A contributor running `cargo add dirs` to "fix" a
homegrown helper would silently reintroduce a `just deny`
failure. Inline documentation in the same place a contributor
looks for the approved-dep list is the right surface for this
signal — better than burying it in `CONTRIBUTING.md` or a code
comment.

**Constraint.** When this change is being implemented, the
implementer SHALL re-derive the dependency list from `Cargo.toml`
rather than copying this design doc's enumeration — the goal is
that AGENTS.md matches the manifest at archive time, even if
v0.5.0 picks up additional deps between spec and archive.

### D7: Conventional commits — broaden the scopes list, allow compound scopes

**What.** AGENTS.md's "Commit Conventions" section's scope
enumeration grows. Two changes:

1. The flat scope list gains the v0.5.0 scopes that shipped
   commits already used: `user-guide`, `worktree`, `governance`,
   `learnings`, `pause`, `forward-coordination` (or its short
   form), plus any others reconciled from
   `git log feat/v0.5.0-specs --pretty=%s | sed -n
   's/.*(\([^)]*\)).*/\1/p' | sort -u`.
2. A new paragraph after the flat list documents compound
   scopes: when a commit cuts across multiple scopes, separate
   them with commas inside the parentheses, e.g.
   `feat(specs,skills,init): add Spec Kit backend`. This pattern
   was used in v0.5.0 shipped commits and matches Commitizen's
   convention. Compound scopes SHALL list scopes in
   alphabetical order for determinism.

**Why.** The current AGENTS.md scope list rejects compound
scopes by omission; contributors copying the list verbatim into
a commitizen hook would fail v0.5.0's own historical commits.
The explicit allow-list of compound forms makes the convention
machine-checkable later if the project adds a `commitizen` lint.

**Constraint to verify.** The pre-push hook (if it runs
commitizen-style validation) needs to accept compound scopes.
Out of scope for this change — captured as a follow-up in
`tasks.md`.

### Alternatives considered for the overall change shape

- **One delta per existing main spec (cli-parsing, configuration,
  agent-skills, ...).** Rejected: doc content isn't a behaviour
  of those capabilities; each capability spec describes runtime
  behaviour. Layering a doc-content requirement onto cli-parsing
  ("the README CLI table SHALL list X") muddies the capability's
  intent and creates a maintenance burden on every cli-parsing
  archive cycle.
- **No new capability — just a `tasks.md` checklist with no spec
  delta.** Rejected: the project's spec-driven discipline is
  exactly that scenarios get encoded so future drift is caught
  on archive. A pure tasks.md checklist would not survive future
  doc edits.
- **New capability `user-documentation` covering README, AGENTS.md,
  mdBook.** Chosen. The capability has zero runtime behaviour and
  is doc-content only, which is unusual for OpenSpec but
  legitimate — it's the canonical place to encode rules like
  "the wire envelope shape SHALL be `{type, agent_id, payload}`
  in every example" and "the supervisor pane index SHALL be 0".
  Future audits can re-validate these substring/structural rules
  against the spec scenarios.

### Risks

1. **Scope creep.** This change touches ~15 doc files plus 2 new
   ones. The risk is reviewers losing the plot and asking for
   more. Mitigation: `tasks.md` lists every file with explicit
   accept criteria; reviewers tick boxes. New chapters that
   don't appear in `tasks.md` are out of scope.
2. **CHANGELOG.md include drift.** If the changelog header style
   changes upstream (e.g. `git cliff` template tweak), the
   mdBook chapter inherits it. Mitigation: the
   `user-documentation` spec scenario "Changelog chapter
   includes the project CHANGELOG.md" only asserts that the
   chapter file content is the `{{#include}}` directive — what
   gets included is `CHANGELOG.md`'s problem, not the doc's.
3. **Future v0.6.0 docs drift the same way.** Without
   editorial guardrails, the doc set will diverge again.
   Mitigation: the spec scenarios are mechanically checkable;
   future audits can grep for the same patterns. The
   `simplify`-style review skill can pick up `architecture.md`
   module-list drift on PR.
4. **Hidden alias revival.** The `--from-specs` alias is hidden
   in v0.5.0 docs to nudge migration. If a doc reviewer asks
   "shouldn't we document the alias for completeness?", the
   answer is no — the `cross-format-spec-selection` archive
   established this and the alias is removed in v1.0.0.
   `tasks.md` includes an explicit "do NOT mention `--from-specs`
   in user-facing docs" line.

## Open Questions

- **Q1.** Does `dashboard.md` need an explicit "panel removed in
  v0.5.0" section, or just remove the v0.4 inbox-panel forward
  reference and call the v0.5 state the present tense? Default
  decision: just remove the v0.4 reference; users coming from
  v0.4 see CHANGELOG.md for the removal. If reviewers ask for an
  explicit migration note, add a one-line callout.
- **Q2.** Should `learnings.md` show a sample
  `.git-paw/session-learnings.md` rendered output? Default
  decision: yes, a small synthetic example (5-10 lines per
  section). The `learnings-mode` archive's proposal already
  shows the shape; copy that example.
- **Q3.** The `quick-start-supervisor.md` page is the single
  highest-traffic on-ramp. Should the wire-format curl examples
  live there in full, or just link to `coordination.md`?
  Default decision: full inline. New supervisors land on this
  page and need to see the canonical envelope shape before they
  click through.

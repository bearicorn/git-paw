## 1. README.md (root)

- [x] 1.1 Features section: add an entry for the Spec Kit backend
      (`[specs] type = "speckit"`) with a one-line description of
      what `.specify/` auto-detection does.
- [x] 1.2 Features section: add an entry for `--no-supervisor`
      (override `[supervisor] enabled = true` for one session).
- [x] 1.3 Features section: add an entry for `start --force`
      (bypass uncommitted-spec validation warning).
- [x] 1.4 Features section: add an entry for `--specs-format`
      (force-select the spec backend at the CLI).
- [x] 1.5 Features section: add an entry for `agent.intent` and
      forward coordination.
- [x] 1.6 Features section: add an entry for automatic conflict
      detection (forward / in-flight / ownership shapes).
- [x] 1.7 Features section: add an entry for learnings mode and
      the `.git-paw/session-learnings.md` output file.
- [x] 1.8 Features section: add entries for `[governance]`,
      `[supervisor.conflict]`, `[supervisor.auto_approve]`, and
      `[supervisor.learnings_config]` config tables (one bullet
      each).
- [x] 1.9 README.md:311 — expand the `[specs] type` example to
      include `"speckit"` as a valid value.
- [x] 1.10 README.md:122-130 — Quick Start: Supervisor Mode
       section: add `--no-supervisor` and `start --force`
       examples with one-line rationale each.
- [x] 1.11 README.md:346-357 — Supported AI CLIs table: cross-
       reference against `src/detect.rs` at the time of the
       refresh and add every CLI present in the source but
       missing from the table. v0.5.0 baseline: add `opencode`,
       `cline`, and `droid`.
- [x] 1.12 README.md start-flag listing: add `start --force` if
       absent.
- [x] 1.13 Re-grep the README for `--from-specs`; replace user-
       facing instances with `--from-all-specs`. Leave the
       alias undocumented per `cross-format-spec-selection`.

## 2. AGENTS.md (root)

- [x] 2.1 Dependency table: add `schemars` (0.8) with a one-line
      purpose description (e.g. TOML schema generation).
- [x] 2.2 Dependency table: add `serde_yaml` (0.9) with purpose
      (e.g. Spec Kit `tasks.md` frontmatter parsing).
- [x] 2.3 Dependency table: add `chrono` (0.4) with purpose
      (timestamps for the learnings file and session metadata).
- [x] 2.4 Dependency table: add `regex` (1) with purpose
      (pattern matching in `src/agents.rs`, pane classification,
      and safe-command detection).
- [x] 2.5 Cross-check the dev-dependencies in `Cargo.toml`
      against the table; add any v0.5.0 additions present in
      the manifest but absent from AGENTS.md.
- [x] 2.6 Remove the `dirs` row from the approved deps table.
- [x] 2.7 Add a paragraph under the Dependencies heading
      explaining that the upstream `dirs` crate was removed in
      v0.5.0 because its transitive license chain fails
      `just deny`, and that the project uses an in-tree
      `src/dirs.rs` for platform XDG paths. Tell future
      contributors NOT to re-add `dirs` via `cargo add` to
      "fix" the homegrown helper.
- [x] 2.8 Scope list in Commit Conventions: reconcile against
      shipped v0.5.0 commits via
      `git log feat/v0.5.0-specs --pretty=%s | sed -n
      's/.*(\([^)]*\)).*/\1/p' | sort -u`. Add at minimum:
      `user-guide`, `worktree`, `governance`, `learnings`,
      `pause`. Add others reconciled from the actual log.
- [x] 2.9 Scope list: add an explicit paragraph documenting
      compound scope form (e.g.
      `feat(specs,skills,init): add Spec Kit backend`) as
      permitted. State the scopes SHALL be alphabetised inside
      the parentheses.
- [x] 2.10 Sanity-check: re-grep AGENTS.md for `--from-specs`;
       replace user-facing instances with `--from-all-specs`.

## 3. docs/src/architecture.md

- [x] 3.1 Remove references to `src/broker/state.rs` (file does
      not exist).
- [x] 3.2 Remove references to `src/broker/flush.rs` (file does
      not exist).
- [x] 3.3 Add a row / entry for the `src/supervisor/` subtree,
      listing its top-level modules (`boot.rs`,
      `governance.rs`, `layout.rs`, ...). Source-of-truth: the
      actual directory listing at archive time.
- [x] 3.4 Add rows for `src/broker/conflict.rs`,
      `src/broker/learnings.rs`, `src/broker/watcher.rs`,
      `src/broker/delivery.rs`, `src/broker/publish.rs` with
      one-line responsibilities each.
- [x] 3.5 Add rows for `src/specs/resolve.rs` and
      `src/specs/speckit.rs` with one-line responsibilities.
- [x] 3.6 Replace any v0.4 supervisor-mode layout diagram or
      description with the 50/50 top-row diagram from
      `2026-05-13-supervisor-as-pane/proposal.md` (supervisor
      at pane 0, dashboard at pane 1, agent grid below).
- [x] 3.7 Include the row-height proportion table from the same
      archive (2 rows = 60/40, 3 rows = 40/30/30, 4 rows =
      28/24/24/24, 5 rows = 28/18/18/18/18, 6 rows =
      28/14.4/14.4/14.4/14.4/14.4).
- [x] 3.8 Show the non-supervisor layout (no top row) separately
      so users don't conflate the two.
- [x] 3.9 architecture.md:128-138 — remove the "dashboard at
      pane 0" framing for supervisor mode; the dashboard is
      at pane 1 in v0.5 supervisor mode.

## 4. docs/src/changelog.md

- [x] 4.1 Replace the entire chapter body with a single
      `{{#include ../../CHANGELOG.md}}` directive.
- [x] 4.2 Verify `mdbook build docs/` includes the file body
      under the chapter's TOC entry.
- [x] 4.3 Confirm that mdBook's relative-path resolution finds
      `CHANGELOG.md` from `docs/src/changelog.md` (the relative
      path is `../../CHANGELOG.md` because `docs/src/` is two
      levels below the repo root).

## 5. docs/src/quick-start-supervisor.md

- [x] 5.1 quick-start-supervisor.md:57 — fix to say "supervisor
      pane 0, dashboard pane 1" (the supervisor-as-pane archive's
      canonical layout).
- [x] 5.2 quick-start-supervisor.md:102 — fix to say "Supervisor
      pane (pane 0)" or update the surrounding text so the
      reference to pane 0 is the supervisor pane, not the
      dashboard.
- [x] 5.3 Re-grep the chapter for any other pane-index reference
      and reconcile against the canonical layout.
- [x] 5.4 quick-start-supervisor.md:104 — remove the reference
      to `agent.register` (nonexistent broker message variant).
- [x] 5.5 quick-start-supervisor.md:104 — remove the reference
      to `agent.done` (nonexistent broker message variant — the
      shipped equivalent is `agent.artifact` with
      `status="done"`).
- [x] 5.6 quick-start-supervisor.md:90-96 — remove or rewrite
      the "What's NOT Yet Supported in v0.4.0" section so
      conflict detection and learnings mode are not listed as
      deferred. Both shipped in v0.5.0.
- [x] 5.7 Add wire-format curl examples for each variant the
      chapter currently references, using the canonical
      `{"type": ..., "agent_id": ..., "payload": {...}}` shape.

## 6. docs/src/user-guide/coordination.md

- [x] 6.1 coordination.md:93-117 — replace every JSON example
      using the legacy `{"agent_id": ..., "kind": ..., "body":
      "..."}` shape with the canonical
      `{"type": "agent.<kind>", "agent_id": "<slug>", "payload":
      {...}}` shape from `src/broker/messages.rs`.
- [x] 6.2 Replace every slashed `agent_id` (e.g. `"feat/auth"`)
      with the slug-valid form (e.g. `"feat-auth"`).
- [x] 6.3 coordination.md:121-135 — add wire-form curl examples
      for `agent.intent`, `agent.question`, `agent.feedback`,
      and `agent.verified`.
- [x] 6.4 Add an explicit note that `agent.status` is
      auto-published by the filesystem watcher and that
      `agent.artifact` is auto-published by the post-commit
      git hook; the manual curl examples are escape hatches.
- [x] 6.5 Add a section anchor at
      `#automatic-conflict-detection-v050` so the inbound link
      from `configuration/README.md:339-340` resolves, OR
      coordinate with task 9.4 to remove the link instead.
      Pick one approach and execute consistently.

## 7. docs/src/user-guide/spec-driven-launch.md

- [x] 7.1 spec-driven-launch.md:153 — replace
      `git paw start --from-specs` with
      `git paw start --from-all-specs`.
- [x] 7.2 spec-driven-launch.md:159 — same replacement.
- [x] 7.3 spec-driven-launch.md:162 — same replacement.
- [x] 7.4 Re-grep the chapter for any further `--from-specs`
      uses; replace user-facing instances. The hidden alias is
      intentionally undocumented in v0.5.0 docs.
- [x] 7.5 Verify line 53's statement about the alias is
      consistent with the post-refresh chapter.
- [x] 7.6 Add a `## Spec Kit` (or equivalent) section covering:
      (a) the `[specs] type = "speckit"` config value, (b) the
      `.specify/` auto-detection rule, (c) a minimal worked
      example with `[P]`/non-`[P]` decomposition, (d) a one-
      sentence reference to constitution auto-wiring into
      `[governance]`.

## 8. docs/src/user-guide/agents-md.md

- [x] 8.1 agents-md.md:1-35 — rewrite the introduction so
      AGENTS.md is described as the full source of truth for
      the spec body (per boot-prompt-full-body), not as
      "Branch + CLI + Spec content + Owned files".
- [x] 8.2 Add a paragraph explaining that supervisor-mode boot
      prompts point the agent at AGENTS.md and
      `openspec/changes/<id>/` rather than embedding the spec
      body in the boot prompt.
- [x] 8.3 Verify the chapter mentions where AGENTS.md sits in
      each worktree (the worktree root) and which generator is
      responsible (`setup_worktree_agents_md` or whatever the
      v0.5 name is).

## 9. docs/src/user-guide/dashboard.md

- [x] 9.1 Remove the "v0.4 will add interactive prompt inbox"
      forward-looking reference; v0.4 shipped it,
      supervisor-as-pane removed it in v0.5.0.
- [x] 9.2 State that in supervisor mode the dashboard is at
      pane 1 (not pane 0).
- [x] 9.3 Confirm the chapter no longer describes a prompt
      inbox panel as present.
- [x] 9.4 If a one-line callout about the inbox removal is
      added, link it to the changelog entry for clarity.

## 10. docs/src/configuration/README.md

- [x] 10.1 configuration/README.md:48-50 — expand the commented
      `[specs] type` example to include `"speckit"` as a valid
      value.
- [x] 10.2 configuration/README.md:339-340 — resolve the
      dangling link to `coordination.md#automatic-conflict-
      detection-v050`. Either ensure the anchor exists in
      `coordination.md` (per task 6.5) or remove the link.
- [x] 10.3 Add a subsection or table documenting
      `[supervisor.conflict]` (fields: `window_seconds`,
      `warn_on_intent_overlap`, `escalate_on_violation`, with
      defaults).
- [x] 10.4 Add a subsection or table documenting
      `[supervisor.auto_approve]` (fields: `enabled`,
      `stall_threshold_seconds`, `approval_level`,
      `safe_commands`, with defaults).
- [x] 10.5 Add a subsection or table documenting
      `[supervisor.learnings_config]` (field:
      `flush_interval_seconds`, default 60). Note that the
      parent `[supervisor] learnings = true` flag is what
      activates the subsystem.
- [x] 10.6 Add a subsection or table documenting `[governance]`
      (fields: `adr`, `test_strategy`, `security`, `dod`,
      `constitution`, all `Option<PathBuf>`). Include the
      Spec Kit constitution auto-wiring note.

## 11. docs/src/cli-reference.md

- [x] 11.1 cli-reference.md:14-22 — add `init` to the top-level
      commands list with a one-line description.
- [x] 11.2 cli-reference.md:14-22 — add `replay` to the top-
      level commands list with a one-line description.
- [x] 11.3 cli-reference.md:58-71 — add `--specs-format` to the
      start-flag table with its accepted values
      (`openspec`/`markdown`/`speckit`).
- [x] 11.4 Add `--from-all-specs` to the start-flag table.
- [x] 11.5 Add `--specs` (with optional comma-separated values)
      to the start-flag table.
- [x] 11.6 Add `--no-supervisor` to the start-flag table.
- [x] 11.7 Add `--force` to the start-flag table.
- [x] 11.8 Verify every flag in the table matches a flag
      defined in `src/cli.rs::StartArgs` at archive time.

## 12. docs/src/user-guide/learnings.md (new chapter)

- [x] 12.1 Create the file with a title, intro, and table of
      contents.
- [x] 12.2 Document the opt-in `[supervisor] learnings = true`
      flag (default `false`); state that the subsystem requires
      supervisor mode to be active.
- [x] 12.3 Document the output file location
      `.git-paw/session-learnings.md` and note that the file
      is append-only across sessions.
- [x] 12.4 Enumerate the five deterministic categories tracked
      in v0.5.0 (stuck duration, recovery-cycle count, forward
      conflicts, in-flight conflicts, ownership violations)
      with one paragraph each explaining what triggers an entry.
- [x] 12.5 Show a sample rendered output (5-10 lines per
      category) matching the format described in
      `learnings-mode` archive's proposal.
- [x] 12.6 Document the `[supervisor.learnings_config]
      flush_interval_seconds` knob (default 60s).
- [x] 12.7 State that the broker `agent.learning` variant is
      deferred to v0.6.0 and that v0.5.0 ships file-only.

## 13. docs/src/user-guide/conflict-detection.md (new chapter)

- [x] 13.1 Create the file with title, intro, and TOC.
- [x] 13.2 Document the three failure shapes (forward,
      in-flight, ownership) with the trigger condition for
      each.
- [x] 13.3 Document the `[conflict-detector]` tag prefix on
      auto-emitted `agent.feedback` and explain it distinguishes
      detector output from human-typed supervisor feedback.
- [x] 13.4 Document supervisor-inbox routing for
      `agent.question` escalations (which fire when an in-flight
      conflict has not resolved within `window_seconds`).
- [x] 13.5 Document how detection interacts with the
      filesystem watcher's auto-published `modified_files`.
- [x] 13.6 Document the `[supervisor.conflict]` config knobs
      (`window_seconds`, `warn_on_intent_overlap`,
      `escalate_on_violation`) and link to the configuration
      reference subsection.

## 14. docs/src/SUMMARY.md

- [x] 14.1 Add a User Guide entry for `learnings.md`:
      `- [Learnings Mode](user-guide/learnings.md)`.
- [x] 14.2 Add a User Guide entry for `conflict-detection.md`:
      `- [Conflict Detection](user-guide/conflict-detection.md)`.
- [x] 14.3 Pick a position consistent with the existing User
      Guide ordering. Suggested: place both after `Governance`
      and before any contributor-facing chapters.

## 15. Final verification

- [x] 15.1 Run `mdbook build docs/` and confirm zero errors and
      zero warnings about missing files or broken includes.
      (Two pre-existing `specifications/index.md` unclosed-HTML-tag
      warnings remain; not files this change touched and not
      "missing files / broken includes".)
- [x] 15.2 Run a grep across `docs/src/` for `agent.register`
      and `agent.done`; expected count is zero. (Initial run
      surfaced two surviving `agent.done` references in
      spec-driven-launch.md and coordination.md; both
      rewritten to canonical `agent.artifact { status: "done" }`.)
- [x] 15.3 Run a grep across `docs/src/` for
      `src/broker/state.rs` and `src/broker/flush.rs`; expected
      count is zero.
- [x] 15.4 Run a grep across `docs/src/` for `--from-specs` (as
      a flag in example commands); expected count is zero in
      user-facing chapters.
- [x] 15.5 Run a grep for `"kind":` in JSON code-fences across
      `docs/src/user-guide/coordination.md` and
      `docs/src/quick-start-supervisor.md`; expected count is
      zero (canonical envelope uses `"type":`).
- [x] 15.6 Run a grep for `"feat/` in `"agent_id":` contexts
      across the same two files; expected count is zero (slug
      validation forbids slashes).
- [x] 15.7 Visually inspect `architecture.md`'s rendered output;
      confirm the new layout diagram renders correctly in
      mdBook's HTML.
- [x] 15.8 Visually inspect `changelog.md`'s rendered output;
      confirm the `{{#include}}` expands to the full
      `CHANGELOG.md` content.
- [x] 15.9 Confirm `git paw start --help` still matches the
      README CLI table; if any `help`/`long_about` string in
      `src/cli.rs` drifted from the README, file a follow-up
      under "Out-of-scope follow-ups" rather than editing Rust
      in this change.

## 16. Out-of-scope follow-ups (capture for next cycle)

- [ ] 16.1 Pre-push hook validation of compound conventional-
      commit scopes (mentioned in design D7's "Constraint to
      verify"). Add a separate change if commitizen-style lint
      is desired.
- [ ] 16.2 Automated module-list refresh for `architecture.md`
      from `cargo metadata` (mentioned in design D2's
      "Alternatives considered"). v1.0.0 if churn justifies it.
- [ ] 16.3 Rustdoc / API doc generation refresh. Out of scope
      for v0.5.0 doc-only refresh; tracked separately.
- [ ] 16.4 MILESTONE.md and developer notes — these are not
      user-facing and are intentionally untouched by this
      change. If contributors want a milestone refresh, file a
      separate change.

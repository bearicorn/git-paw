## Context

The `SpecBackend` trait (`scan(&Path) -> Vec<SpecEntry>`) and the `SpecEntry { id, branch, cli, prompt, owned_files }` shape are stable across v0.4 backends. Both existing backends (OpenSpec, Markdown) share an *N-to-N* relationship: one input unit (subdirectory or `.md` file) produces one `SpecEntry`. Their differences are surface-level — frontmatter parsing, prompt assembly, file ownership extraction.

Spec Kit breaks this assumption. A single feature directory `<dir>/<feature>/` produces:
- One `SpecEntry` per `[P]` task in the current phase.
- One *consolidated* `SpecEntry` for the union of non-`[P]` tasks in the current phase.

That's 0..N entries from a single subdirectory, depending on phase contents. The trait absorbs this — `Vec<SpecEntry>` is already plural — but the backend's internal logic is meaningfully more involved than its peers.

A second new dimension is **out-of-tree governance signal**: `<dir>/../memory/constitution.md`. The SpecKit backend needs to communicate this path to the `governance-config` change's consumer without coupling the two crates' compile-time. The simplest contract: an additional method on the backend trait (or a free function) that returns an optional path. If `governance-config` is shipped, it picks up the path; if not (or if the project doesn't have one), nothing happens.

A third new dimension is **agent writeback**: agents flip `- [x]` in their worktree's copy of `tasks.md` as they complete tasks. This is purely a coordination-skill instruction — the agent reads its skill, follows it, edits the file. No new code path enforces it; git's normal merge handles per-task line edits. The supervisor does NOT validate the checkbox flips at merge time (earlier drafts had this; dropped — supervisor checkbox auditing is over-reach for what's fundamentally a backend feature, and the supervisor's existing Spec Audit Procedure already verifies code against spec). Spec Kit projects running pure-manual sessions still get the `tasks.md` progress tracking; supervisor-mode sessions also get the standard spec audit on top, just without a Spec-Kit-specific checkbox-bookkeeping layer.

## Goals / Non-Goals

**Goals:**
- Read Spec Kit `tasks.md` files robustly enough that the parser doesn't silently drop tasks on whitespace/punctuation variants.
- Decompose a feature's *current phase* into the canonical `[P]`-vs-consolidated worktree layout, matching MILESTONE.md's table exactly.
- Auto-detect `.specify/` and default `specs.type`/`specs.dir` accordingly. Make the override paths obvious (`--specs-format`, explicit `[specs]` config).
- Hand `governance-config` a constitution path without forcing temporal coupling between the two changes.
- Keep the backend's `SpecEntry` outputs valid for the existing worktree/branch/boot-prompt pipeline. The downstream code shouldn't need to know "this came from Spec Kit."
- Make boot prompts skim-readable: spec → plan → checklists → tasks-list, in that order, with clear delimiters.

**Non-Goals:**
- Writing back to `tasks.md`. The agent does that via its skill instructions; the backend only *reads* `tasks.md`.
- Validating that `[P]` markers are correctly applied by the Spec Kit author. If two `[P]` tasks share files, that's a Spec-Kit-author problem; `conflict-detection` will surface it at runtime via `agent.intent` overlap.
- Cross-feature dependency resolution. Each feature's phase clock is independent.
- Validating Spec Kit version. The backend treats Spec Kit's artefact shape as a public protocol; if Spec Kit changes its artefact format, this is a future concern.
- Fancy phase-name normalisation. We extract the phase name verbatim and use it in the consolidated entry's branch slug; the slugifier handles punctuation.

## Decisions

### D1. `tasks.md` parser: regex-based, line-oriented, lenient on punctuation

Considered three parser strategies:

| Approach | Pros | Cons |
|---|---|---|
| Full Markdown AST (e.g. `pulldown-cmark`) | Handles arbitrary nested structure | Heavy dep, overkill for checkbox-list shape, brittle to Spec Kit format drift |
| Hand-rolled state machine | Most control | Easy to under-test |
| Regex on lines, fall through unrecognised lines (chosen) | Simple, fast, easy to extend | Risks silent drops if patterns are too strict — mitigated by being lenient |

Patterns the parser recognises (each as an anchored line regex):

- Phase heading: `^##\s+Phase\s+(\d+)\s*[:—-]\s*(.+)$` — captures phase number and name. Flexible separator (`:`, `—`, `-`).
- Incomplete task: `^-\s+\[\s\]\s+(T\d+)(\s+\[P\])?\s+(.+)$` — captures task ID, optional `[P]`, description.
- Complete task: `^-\s+\[x\]\s+(T\d+)(\s+\[P\])?\s+(.+)$` — same but with `x`. Case-insensitive on the `x`.

Lines that don't match any pattern are ignored (preserves intra-phase commentary). The phase heading anchors which phase a task belongs to: tasks beneath a phase heading and before the next phase heading belong to that phase.

### D2. Current-phase identification

Algorithm: walk phases in order; the first phase containing **at least one** `- [ ]` task is the *current phase*. All earlier phases are assumed complete (Spec Kit's convention is to mark earlier phases fully `- [x]` before starting later phases). All later phases are deferred — the backend SHALL NOT generate `SpecEntry` values for them in this session.

If a feature has *no* phase headings (a "phase-less" `tasks.md`), the parser treats the whole file as a single implicit phase. Decomposition rules apply normally.

If a feature has every task `- [x]` across all phases, the feature is "done" and the backend SHALL skip it (no `SpecEntry` produced).

### D3. Decomposition: `[P]` vs. consolidated

For the current phase of a feature:

- **Each incomplete `[P]` task** → its own `SpecEntry`:
  - `id = "<feature-dir>-<task-id>"` (e.g. `003-user-list-T009`)
  - `branch = "task/<task-id>-<slugified-description>"` (e.g. `task/T009-add-login-form`); description slugified by the existing `slugify_branch` helper.
  - `prompt` = boot context (spec.md + plan.md + checklists) followed by the single task description.
  - `owned_files = None` — Spec Kit doesn't carry per-task ownership.

- **All incomplete non-`[P]` tasks in the current phase** → one consolidated `SpecEntry`:
  - `id = "<feature-dir>-phase-<N>"` (e.g. `003-user-list-phase-2`)
  - `branch = "phase/<feature-dir>-<phase-name-slug>"` (e.g. `phase/003-user-list-foundational`)
  - `prompt` = boot context followed by the full ordered list of non-`[P]` task descriptions, plus an instruction to work through them sequentially and flip `- [x]` per task.
  - `owned_files = None`.

Edge cases:
- Phase with only `[P]` tasks → N entries, no consolidated entry.
- Phase with only non-`[P]` tasks → 1 consolidated entry. (Even if it's a single task.)
- Phase with mix → N `[P]` entries + 1 consolidated entry.
- Empty `tasks.md` → 0 entries.

The consolidated-entry-for-a-single-non-`[P]`-task case (functionally identical to a single `[P]` entry but with a `phase/...` branch name) is preserved deliberately — it gives the user a consistent mental model: "non-`[P]` tasks always run in a `phase/` worktree, no matter how many."

### D4. Boot-prompt assembly

Order of sections in the prompt (each separated by a clear delimiter line):

1. **Feature Context** — `spec.md` content verbatim.
2. **Implementation Plan** — `plan.md` content verbatim. Skipped if the file is missing.
3. **Validation Criteria** — for each file in `<feature>/checklists/`, the file content under a heading naming the file. Skipped if directory is empty/missing. Boot prompt SHALL note that checklists are advisory in v0.5.0 (full enforcement is v1.0.0).
4. **Your Task** — for `[P]` entries: the single task description. For consolidated entries: an ordered list of task descriptions, plus the writeback instruction ("flip `- [x]` in `tasks.md` as you complete each task; commit when convenient; publish `agent.done` only when all listed tasks show `- [x]`").

Delimiter convention: `\n\n---\n\n` (matches the boot-prompt assembly used elsewhere in git-paw). The first `---\n\n` opens the agent's task block (after the boot block injected by the supervisor); the SpecKit backend's prompt is *appended* to that block.

### D5. Auto-detection of Spec Kit

Auto-detection runs at config-load time, after explicit config has been parsed. The decision tree:

1. If `[specs]` is explicitly set in `.git-paw/config.toml`, use it as-is. No auto-detection.
2. If `--specs-format` is passed on the CLI, use it. Auto-detection skipped.
3. Otherwise: probe for `.specify/` at the repository root.
   - If present: set `specs.type = "speckit"`, `specs.dir = ".specify/specs"`.
   - Else: probe for `openspec/changes/` (existing OpenSpec auto-detection precedent if any), then fall through to "specs not configured" error.

Probe = `path.is_dir()` and `path.join("specs").is_dir()`. Cheap and synchronous.

### D6. `--specs-format` CLI value extension

The CLI flag's parse function gains `"speckit"` as a valid value (alongside `"openspec"` and `"markdown"`). No new flag, no new variant on the user-facing surface.

### D7. Constitution path probe

The SpecKit backend SHALL expose a method `pub fn detected_constitution_path(&self) -> Option<PathBuf>` (or a free function `pub fn detect_constitution(specs_dir: &Path) -> Option<PathBuf>`). Implementation: `specs_dir.parent().join("memory/constitution.md")` — exists check.

This method is consumed by the `governance-config` change (parallel work). When governance-config sees:
- An empty `[governance.constitution]` field, AND
- A SpecKit backend is active, AND
- `detected_constitution_path()` returns `Some`,

it populates the governance config's constitution slot with that path. If `governance-config` ships first, this method exists but its return value is unused — no harm. If this change ships first, the consumer hooks in cleanly later.

### D8. SpecBackend trait remains unchanged

We considered extending the trait (e.g. adding `governance_signals(&self) -> GovernanceSignals`). Rejected — the constitution probe is SpecKit-specific and a free function fits better than trait pollution. The trait stays narrow; SpecKit's extra capability is a downcast/feature-detect on the SpecKit type only.

### D9. Branch-name slugification

Reuse `slugify_branch` (already in `broker-messages` spec) for description slugs. For the consolidated entry's `phase/...` branch, the slug input is the *concatenation* `<feature-dir>-<phase-name>` (e.g. `003-user-list-Foundational`). The result: `phase/003-user-list-foundational`. Lowercasing and separator collapsing produce a stable, git-friendly branch name.

### D10. Skip empty / fully-complete features

If a feature directory contains a `tasks.md` whose tasks are *all* `- [x]`, OR has no `tasks.md` at all, the backend SHALL emit a warning to stderr and SHALL skip the directory. The warning identifies the path. (Matches the existing OpenSpec backend convention for missing `tasks.md`.)

If `tasks.md` parses cleanly but has zero tasks (parse-recognised lines), the backend SHALL also skip — no entries to make.

## Risks / Trade-offs

- **[Risk] Lenient parser silently drops malformed task lines.** A typo like `- [ ]T009 Add login` (missing space) wouldn't match the regex. → **Mitigation:** the parser logs unrecognised lines to stderr at `--verbose` (or a log target the existing CLI uses). Users notice "expected 5 tasks, got 4" via the entries list. Long-term, a `git paw specs lint` subcommand could validate the file shape — out of scope here.
- **[Risk] Consolidated worktree concentrates too much work into one agent.** A 12-task non-`[P]` phase produces one worktree with one agent walking 12 tasks sequentially. If any task fails, the whole worktree stalls. → **Mitigation:** this is intentional per MILESTONE.md (the non-`[P]` marker means tasks share files / context). Sequential is the right shape; if Spec Kit authors over-mark non-`[P]`, that's a spec-authoring problem revealed by stuck-agent signals (which `learnings-mode` surfaces). The skill update tells agents to ask for help if blocked rather than push through.
- **[Risk] `tasks.md` writeback contention at merge time.** Multiple worktrees flipping different lines in the same file → standard git merge handles line-level edits. Pathological case: same-line edits (two tasks, same TNNN, both flipped). → **Mitigation:** Spec Kit task IDs are unique. The writeback is `- [x]` on a specific TNNN line; concurrent flips of the *same* TNNN imply two worktrees claimed the same task, which `conflict-detection` would have surfaced via `agent.intent`. By the time we hit a merge, the conflict is detected upstream.
- **[Risk] Auto-detection picks `.specify/` even when the user wanted Markdown specs in a sibling directory.** → **Mitigation:** auto-detection runs only when `[specs]` is *unset* in config and `--specs-format` is *unset* on CLI. Any explicit signal wins. The init flow can also write an explicit `[specs] type = "markdown"` section to lock the choice.
- **[Trade-off] One feature → multiple SpecEntry breaks the "one subdirectory = one branch" mental model.** Users tracing a session might see two branches (`task/T009-...` and `phase/003-...`) for one Spec Kit feature. → **Mitigation:** the dashboard, replay, and supervisor surfaces all key on `agent_id` (derived from branch), not feature directory. The grouping-by-feature view is a future UX improvement; v0.5.0 documents the multi-branch shape clearly in the user guide.
- **[Trade-off] `owned_files = None` for SpecKit entries.** The OpenSpec backend extracts `owned_files` from `tasks.md` content; SpecKit doesn't have an equivalent declaration. → **Mitigation:** `conflict-detection` (forward + ownership) handles overlap dynamically via `agent.intent`. Agents publish the files they intend to touch; the detector warns. Static `owned_files` becomes optional once the dynamic path exists.

## Migration Plan

Additive only. Steps:

1. Land `forward-coordination` and `conflict-detection` first (any Spec Kit session benefits from them).
2. Land this change. Existing OpenSpec / Markdown sessions are unchanged — auto-detection only fires when `[specs]` is unset.
3. New Spec Kit users: run `git paw init --from-specs` in a project containing `.specify/`; the init flow detects and writes `specs.type = "speckit"` to the generated config.
4. Existing Spec Kit users with v0.4: editing the generated `.git-paw/config.toml` to add `[specs] type = "speckit"` (or running `git paw init` again) opts in.
5. Rollback: revert. Existing Spec Kit projects need to either downgrade to OpenSpec/Markdown or pin a v0.4 binary; auto-detection becomes a no-op.

Release-notes call-outs:
- New `speckit` value for `--specs-format` and `[specs] type`.
- Auto-detection of `.specify/`.
- Constitution path is auto-wired into `governance.constitution` when both are present.

## Open Questions

- **Should the parser tolerate `- [X]` (uppercase X) as complete?** Spec Kit's convention is lowercase, but copy-paste from Markdown editors sometimes produces uppercase. Decision: yes, case-insensitive on the `x` — minimal complexity, avoids false-incomplete tasks. (Already encoded in D1.)
- **Should the consolidated boot prompt include task IDs in addition to descriptions?** Decision: yes. The agent needs IDs to flip the right `- [x]` line, and including them up-front avoids ambiguity. The boot prompt format becomes "T004 — Setup database schema" per line.
- **Should checklists be hidden behind a feature flag?** Decision: no. v0.5.0 includes them as advisory context unconditionally; gating is unnecessary because they're informational, not enforced. v1.0.0's checklist enforcement is where a flag belongs.
- **Should `feature-dir` slugs preserve leading numbers (`003-user-list`) or strip them (`user-list`)?** Decision: preserve. The leading number is part of Spec Kit's naming and matters for sort order in `--spec` narrowing (separate change). Stripping would also collide with feature names that share a slug after the number.

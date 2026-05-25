## 1. SpecKit backend module

- [x] 1.1 Create `src/specs/speckit.rs` (or `src/specs/speckit/mod.rs` if it grows beyond ~600 lines).
- [x] 1.2 Define internal types: `Task { id: String, p_marker: bool, complete: bool, description: String, phase: u32 }`, `Phase { number: u32, name: String, tasks: Vec<Task> }`, `Feature { dir: PathBuf, phases: Vec<Phase>, spec_md: Option<String>, plan_md: Option<String>, checklists: Vec<(String, String)> }`.
- [x] 1.3 Implement `pub struct SpecKitBackend;` and `impl SpecBackend for SpecKitBackend`. The `scan` method walks immediate subdirectories of `dir`, parses each as a feature, calls the decomposition function, and returns `Vec<SpecEntry>`.
- [x] 1.4 Register `SpecKitBackend` in `src/specs/mod.rs` dispatch table (the `match specs.type` arm).

## 2. tasks.md parser

- [x] 2.1 Implement `pub fn parse_tasks_md(content: &str) -> Vec<Phase>` using line-by-line scanning with three line patterns:
  - Phase heading regex: `^##\s+Phase\s+(\d+)\s*[:—-]\s*(.+)$`
  - Incomplete task regex: `^-\s+\[\s\]\s+(T\d+)(\s+\[P\])?\s+(.+?)$`
  - Complete task regex (case-insensitive `x`): `^-\s+\[xX\]\s+(T\d+)(\s+\[P\])?\s+(.+?)$`
- [x] 2.2 Tasks attach to the most recently seen phase heading. If a task appears before any heading, it joins an implicit phase 0 (or a single implicit phase if the whole file has no headings).
- [x] 2.3 Lines that match no pattern are silently dropped (no error). At verbose log level, emit them on stderr for debugging.
- [x] 2.4 Unit-test the parser: standard task line, `[P]` marker, complete task with both `x` and `X`, all phase-heading separator variants (`:`, `—`, `-`), tasks attached to preceding heading, free-form prose ignored, phase-less file produces single implicit phase.

## 3. Current-phase identification

- [x] 3.1 Implement `pub fn current_phase(phases: &[Phase]) -> Option<&Phase>` returning the lowest-numbered phase with at least one incomplete task. Returns `None` if all phases are complete.
- [x] 3.2 Unit-test: phase 1 fully `[x]`, phase 2 mixed → returns phase 2; all-`[x]` feature → returns `None`; phase-less file with one incomplete task → returns the implicit phase.

## 4. Feature decomposition into SpecEntry

- [x] 4.1 Implement `pub fn decompose_feature(feature: &Feature) -> Vec<SpecEntry>`. For the current phase only, produce one `SpecEntry` per `[P]` task plus one consolidated entry for the union of non-`[P]` tasks (if any).
- [x] 4.2 `SpecEntry` field assembly:
  - `id` for `[P]`: `"<feature-dir>-<task-id>"` (e.g. `003-user-list-T009`).
  - `id` for consolidated: `"<feature-dir>-phase-<N>"` (e.g. `003-user-list-phase-2`).
  - `branch` for `[P]`: `"task/" + slugify_branch(format!("{task-id}-{description}"))`.
  - `branch` for consolidated: `"phase/" + slugify_branch(format!("{feature-dir}-{phase-name}"))`.
  - `prompt`: assembled by the boot-prompt builder (group 5).
  - `cli`: `None` (Spec Kit doesn't carry per-task CLI override).
  - `owned_files`: `None`.
- [x] 4.3 Edge cases:
  - Phase with only `[P]` → no consolidated entry.
  - Phase with only non-`[P]` (any count, including 1) → exactly one consolidated entry.
  - All-complete or empty phase → no entries (caught by `current_phase` returning `None`).
- [x] 4.4 Unit-test all edge cases plus the `id` and `branch` shape assertions.

## 5. Boot-prompt assembly

- [x] 5.1 Implement `pub fn build_prompt(feature: &Feature, entry_kind: EntryKind) -> String` where `EntryKind` is either `Single { task: &Task }` or `Consolidated { tasks: &[&Task], phase_number: u32 }`.
- [x] 5.2 Sections in order, each separated by `\n\n---\n\n`:
  1. `## Feature Context` followed by `feature.spec_md` content (skip header line if absent).
  2. `## Implementation Plan` followed by `feature.plan_md` content (omit entire section if `None`).
  3. `## Validation Criteria (advisory)` followed by each checklist file's heading + content (omit entire section if `feature.checklists` is empty). Preamble notes that checklists are advisory in v0.5.0.
  4. `## Your Task` followed by:
     - For `Single`: the task ID and description on one line.
     - For `Consolidated`: an ordered list of `T<NNN> — <description>` lines + the sequential-execution + writeback + `agent.done` timing instructions.
- [x] 5.3 Unit-test prompt content: spec/plan inclusion, checklist inclusion-when-present + omission-when-empty, single-task format, consolidated-task list ordering and per-task ID prefix, presence of writeback instruction in consolidated prompts only.

## 6. Constitution path probe

- [x] 6.1 Implement `pub fn detect_constitution(specs_dir: &Path) -> Option<PathBuf>`. Returns `Some(specs_dir.parent()?.join("memory/constitution.md"))` when the file exists; `None` otherwise.
- [x] 6.2 Expose the function from `src/specs/speckit.rs` so the parallel `governance-config` change can call it.
- [x] 6.3 Unit-test: detected when present, `None` when missing, `None` when `specs_dir` has no parent (defensive).

## 7. Auto-detection of `.specify/`

- [x] 7.1 In `src/config.rs` (or wherever the spec config is finalised), add an auto-detection probe that runs after explicit config has been parsed and before backend dispatch.
- [x] 7.2 Probe rules: if both `[specs]` is unset in TOML AND `--specs-format` is unset on CLI, check for `.specify/` directory at the repository root with a `specs/` subdirectory. If present: synthesize `specs.type = "speckit"` and `specs.dir = ".specify/specs"`.
- [x] 7.3 Probe is a no-op when explicit config or CLI flag is present; document precedence in the inline comment at the call site.
- [x] 7.4 Unit/integration tests: auto-detect happy path, TOML config wins, CLI flag wins, `.specify/` without `specs/` does not auto-detect.

## 8. CLI flag extension

- [x] 8.1 In `src/cli.rs` (or wherever `--specs-format` is defined as a `clap` enum value), add `Speckit` (or matching naming convention) as a third variant alongside `Openspec` and `Markdown`.
- [x] 8.2 Update the parse error text for invalid values to list all three valid values.
- [x] 8.3 Update CLI help text for the flag to mention Spec Kit support.
- [x] 8.4 Test: `--specs-format speckit` is accepted; `--specs-format unknown` is rejected with the expected error message.

## 9. Init flow

- [x] 9.1 In `src/init.rs`, when `git paw init --from-specs` runs and `.specify/` is detected, default the generated config's `[specs]` section to `type = "speckit"`, `dir = ".specify/specs"`. The user can still override interactively.
- [x] 9.2 Test: init in a `.specify/`-bearing repo writes the SpecKit defaults to `.git-paw/config.toml`.

## 10. Embedded skill updates

- [x] 10.1 In `assets/agent-skills/coordination.md`, add a new section `### When working in a Spec Kit consolidated worktree` (visible only by content; the agent recognises it by branch name). Section content: sequential task list, `- [x]` writeback per task, `agent.intent` covering the next 1–2 tasks with generous TTL, `agent.done` timing, plus an explicit clarification that `[P]` worktrees follow the standard before/while-editing pattern.
- [x] 10.2 Mirror coordination-skill updates into `docs/src/user-guide/coordination.md`. The supervisor skill is NOT modified by this change — supervisor-side checkbox auditing was dropped during scope review.

## 11. Skill-content tests

- [x] 11.1 Coordination skill contains a heading or section referring to Spec Kit consolidated worktrees / `phase/...` branches.
- [x] 11.2 Coordination skill instructs sequential work + `- [x]` writeback + `agent.done`-only-when-all-done.
- [x] 11.3 Coordination skill clarifies `[P]` worktrees follow the standard pattern.

## 12. Integration tests with fixture .specify/ tree

- [x] 12.1 Create test fixture `tests/fixtures/specify-multi-feature/` containing 3 features with varied phase/`[P]` shapes:
  - Feature 1: phase 1 fully complete, phase 2 mixed `[P]` + non-`[P]`.
  - Feature 2: phase 1 only `[P]`, phase 2 only non-`[P]`.
  - Feature 3: fully complete (all `[x]`).
  Plus `.specify/memory/constitution.md` for the constitution probe.
- [x] 12.2 Test: scan returns the expected `SpecEntry` count and shape — no entries from feature 3, N+1 entries for feature 1's phase 2, only N entries for feature 2's phase 1, etc.
- [x] 12.3 Test: each entry's prompt contains `spec.md` and `plan.md` content from its source feature.
- [x] 12.4 Test: the constitution probe returns the expected path for the fixture root.
- [x] 12.5 Test: changing one task line in fixture `tasks.md` to `- [x]` via the test setup re-scan moves the active phase as expected.

## 13. Documentation

- [x] 13.1 New chapter or section in `docs/src/user-guide/spec-formats.md` (or wherever existing OpenSpec/Markdown integration is documented) covering Spec Kit support: layout, auto-detection, `[P]` vs non-`[P]` decomposition with a worked example, branch naming, boot-prompt structure, constitution wiring, and limitations (no glob support, advisory checklists, multi-branch-per-feature mental model).
- [x] 13.2 Configuration reference: document `[specs] type = "speckit"` and explain that `dir` for SpecKit points at the `specs/` subdirectory inside `.specify/`.
- [x] 13.3 Document the "one Spec Kit feature → multiple branches" mental model in the user guide so users understand they'll see `task/T009-...` and `phase/003-...` for one logical feature.

## 14. Release notes & MILESTONE upkeep

- [x] 14.1 Add v0.5.0 release-notes bullet: "Spec Kit (`.specify/`) is now a third spec format alongside OpenSpec and plain Markdown. Auto-detected when `.specify/` is present; explicitly enabled with `--specs-format speckit` or `[specs] type = \"speckit\"`."
- [x] 14.2 Note the multi-branch-per-feature implication in the user-facing release-notes.
- [x] 14.3 Cross-reference `governance-config` for the constitution wiring (handshake described in design doc).

## 15. Quality gates

- [x] 15.1 `just check` — fmt, clippy, all tests green.
- [x] 15.2 `just deny` — supply chain clean.
- [x] 15.3 No new `unwrap()` / `expect()` in non-test code added by this change.
- [x] 15.4 `mdbook build docs/` succeeds.
- [x] 15.5 `openspec validate spec-kit-format` passes.

## Why

**GP-30.** The boot task prompt for an OpenSpec agent (`assets/boot-task-prompts/openspec.md`, chosen
by `build_task_prompt`) ends `run /opsx:apply {{SPEC_ID}}`. The definitions behind that command are
installed by the spec tool into an agent-CLI directory at the repository root (in this repository:
`.claude/commands/opsx/apply.md` and `.claude/skills/openspec-apply-change/`), and that directory is
gitignored here (`.gitignore`: `.claude/`). A fresh worktree contains only tracked files, so the
worktree has no definition of its own. It works today only because `worktree_placement = "child"` puts
the worktree under the repository root, so the CLI's upward search finds the root's directory; with
sibling placement (the default when unset) the instructed command does not exist, and the agent
silently replaces the spec workflow with an ad-hoc one — the same failure class as GP-22/23.

The roadmap asks for a diagnostic, not a launch-path change: `git paw doctor` should warn when the
instructed workflow is unreachable from where worktrees are placed. Independent of the relocation
seams, so it archives early (position 4); it belongs to this release because it validates the boot
prompt the orchestrator relies on.

## What Changes

- A new `git paw doctor` check in the "Spec system" group, a pure function over a probe:
  - **Ok (not applicable)** when the resolved spec format's task prompt names no workflow command
    (Markdown, Spec Kit, Superpowers, no spec system), or when no spec-driven worker CLI resolves
    (the released `SpecCliProbe.resolved_clis` is a `Vec`, empty when no specs are discovered).
  - The spec-driven worker CLIs are a list (each discovered spec may pin its own `paw_cli`), so the
    check evaluates **each** resolved CLI and aggregates:
  - **Ok** for a CLI when the command's definition is tracked by git (every worktree checks it out),
    or when worktree placement is `child` and the definition exists at the repository root
    (reachable by ancestor search).
  - **Warn** when, for any CLI with known locations, placement is `sibling` and the definition exists
    at the root only untracked, or no definition is found — naming the command, each unreachable CLI,
    the locations checked, and the two remedies (track the definitions, or set
    `worktree_placement = "child"`).
  - **Ok (informational)** for a CLI whose definition locations git-paw does not know, stating that
    reachability could not be verified for that CLI.
- Where the definition lives is per-CLI knowledge, held in one table with a generic "unknown"
  fallback; only the locations verified against a real install are listed (see design).
- Read-only, like every doctor check; included in `--json`.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `preflight-diagnostics`: adds the spec-workflow reachability check.

## Impact

- **Predecessors (implementation order):** archive position 4, after `orchestrator-read-surface` (3)
  and before `agent-id-slug-safety` (5); no hard v0.18 dependency (it touches only `src/doctor.rs`,
  and no earlier v0.18 change modifies the requirements it builds on). v0.17:
  `start-spec-launch-parity` (spec-driven launches without supervisor now inject the same task prompt,
  so the check covers those launches too); `mcp-superpowers-support` (Superpowers resolution — a
  format with no workflow command, so the check reports not applicable).
- **Code:** `src/doctor.rs` (`SpecWorkflowProbe`, `check_spec_workflow`, the probe in `collect_probes`,
  the per-CLI location table), reuse of `specs::resolved_spec_type`, `PawConfig::worktree_placement()`
  (already probed into `ConfigProbe.placement`), the effective spec-driven worker CLIs
  (`SpecCliProbe.resolved_clis`, from `probe_spec_cli`), and a `git ls-files` probe through
  `CommandRunner`.
- **Tests:** pure-check table tests for every branch; one probe test against a temp repository
  (tracked vs ignored definition, child vs sibling placement); `--json` shape.
- **Docs:** `docs/src/user-guide/doctor.md` (the check and its remedies),
  `docs/src/user-guide/worktree-placement.md` (placement affects workflow reachability).
- **Enum-variant ripple:** `SpecBackendKind` — the check dispatches on the resolved backend with an
  exhaustive match co-located with the check; list it with the other `SpecBackendKind` sites in AGENTS.md
  §7. `launch-prompt-entry-point` (11, task 2.5) later replaces this per-format command match with its
  entry-point selector, so the check reads the instructed command through one small function that
  change can swap without touching the check.
- **Security / safety:** read-only; runs `git ls-files` with discrete arguments through `CommandRunner`.
- **Dependencies:** none.

## Rebase note (2026-10-25): audit against released v0.17.0

- The released spec-driven worker CLI is not a single value: `SpecCliProbe.resolved_clis`
  (`src/doctor.rs:239-249`, filled by `probe_spec_cli` at `:1330-1360`) is a deduplicated `Vec`,
  empty when no specs are discovered. The check now evaluates every resolved CLI.
- Zero CLIs → `Ok` (not applicable); several → each checked, the warning names only the unreachable
  ones; unknown-location CLIs stay informational per CLI. New scenarios cover both.
- Placement comes from `PawConfig::worktree_placement()` (`src/config/mod.rs:593`), already captured
  in `ConfigProbe.placement` — the proposal named a nonexistent `config::worktree_placement`.
- Ordering prose updated to archive position 4 (was "step 7").
- `launch-prompt-entry-point` (11, task 2.5) replaces the check's per-format command match with its
  entry-point selector; the check derives the instructed command in one small function so that
  change swaps it without touching the check.

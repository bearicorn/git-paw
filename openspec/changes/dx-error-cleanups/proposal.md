## Why

The v0.19.0 release reviews left five small places where git-paw says one thing and does another,
or reports a failure under the wrong name:

- **`start --from-specs` warns and launches anyway.** The uncommitted-spec warning ends "Commit your
  changes or use --force to proceed", but the launch goes ahead with or without `--force`; the flag
  only changes which line is printed. The check also looks only under `specs/<id>`, so OpenSpec
  changes (`openspec/changes/<id>/`) and Spec Kit features are never checked. The `supervisor-launch`
  spec already says the launch SHALL NOT proceed without `--force`; the code drifted from it.
- **Budget errors are mislabelled.** `BudgetCost::new` returns `Result<_, String>`. A run abandoned on
  a budget cap exits as `Session: …`, which reads like a broken session file. The budget escalation
  is built with `format!` in Rust and, having no arm of its own, is framed as "orchestrator is stalled
  and needs a human decision".
- **A stopped rebase at another path is missed.** Creation-time rebase finds a stopped rebase only
  at the path the current `worktree_placement` computes. A worktree created under the other
  placement, with a rebase stopped in it, is invisible to the check.
- **The config loader prints its own warnings.** It writes to stderr and deduplicates through a
  process-global set, which the agent-CLI layer stopped doing in v0.19.0. An in-process caller (MCP,
  the `cmd_*` library) cannot capture or suppress them, and the worktree-runtime warnings repeat
  each time a command loads the config.
- **The struct-literal ripple (DF-23, GP-40).** Adding one field to a widely built struct breaks
  every literal site, and workers fix them with mass `sed` / `perl` edits.

## What Changes

- **`start` refuses uncommitted specs without `--force`.** The launch stops before any worktree, tmux
  session or session file is created. It exits with the not-attempted code (2), and the error names
  each spec and `--force`. With `--force` it launches and prints one line saying it ignored them. The
  rule is the same on the bare and the coordinated paths.
- **The check reads each spec where its format keeps it,** using the backend's `spec_location`, not
  a hard-coded `specs/<id>`.
- **A budget abandon gets its own error variant,** `PawError::BudgetExhausted`. It prints `Budget: …`
  and keeps exit code 1.
- **`BudgetCost::new` returns a typed error.** The user-visible config error text is unchanged.
- **Budget escalation text comes from bundled assets.** The budget-exhausted and
  cost-unenforceable questions and hand-offs are rendered from `assets/` templates and say that a
  budget cap stopped the run, never that an agent is stalled.
- **A stopped rebase is found at any registered worktree path.** The rebase check reads every
  detached worktree that `git worktree list` registers, whatever its path, and reuses the one whose
  stopped rebase is rebasing the branch.
- **The config loader returns warnings as values.** The process-global dedupe goes. The command
  that loads the config emits each warning once, through its output sink.
- **An architecture rule for struct literals:** a struct built at more than a few sites has a
  `Default`, and test and fixture sites outside its module use `..Default::default()` (or a fixture
  builder). Production construction of persisted records stays exhaustive.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `supervisor-launch`: MODIFIED *Validate specs are committed before launching* (refuse without
  `--force`, exit 2, both launch paths, reworded message); ADDED the per-format spec location for the
  check.
- `core-error-handling`: ADDED the budget-exhausted error variant; ADDED typed errors from config
  value constructors.
- `git-operations`: ADDED detection of a stopped rebase at any registered worktree path.
- `core-configuration`: ADDED config-loader warnings returned as values and emitted once per command.
- `code-architecture`: ADDED escalation text for budget verdicts rendered from bundled assets; ADDED
  the struct-literal default rule.

## Impact

- **Code:**
  - `src/commands/start.rs` (`start_with_specs` guard) and `src/commands/supervisor.rs`
    (`SupervisorOptions` gains `force`); `src/main.rs` (dispatch passes `force`; `run_drive_loop`
    error);
  - `src/git.rs` (`check_uncommitted_specs*` takes spec locations; `create_worktree_with` /
    `add_or_reuse_worktree` find a stopped rebase at any path);
  - `src/error.rs` (`BudgetExhausted`); `src/config/supervisor.rs` (`BudgetCost::new`);
  - `src/supervisor/drive.rs` and `src/supervisor/drive/budget.rs` (asset-rendered budget text);
  - `src/config/mod.rs` (`load_config_file` returns warnings; `warn_once_for` deleted), and every
    `load_config` caller, which emits through its sink;
  - test fixtures for the structs the DF-23 audit names.
- **Assets:** new budget escalation templates under `assets/` (exact path in design D4).
- **Tests:** one or more per scenario (see tasks).
- **Docs:** CLI reference (`--force` on `start`), user guide (uncommitted specs, budget exit),
  `docs/src/architecture.md` (warnings as values; the struct-literal rule).
- **Enum-variant ripple:** `PawError` gains `BudgetExhausted`. Every exhaustive match over `PawError`
  (`exit_code`, the MCP error mapper, tests) gains an arm.
- **Breaking:** `start --from-specs` with uncommitted specs and no `--force` now stops (exit 2) where
  it launched before. Scripts that relied on the old behaviour add `--force`. This is the behaviour
  the spec and the warning text already promised.
- **Cross-version dependencies:**
  - `supervisor-vocabulary-split` (this release) renames supervisor→orchestrator. Its deprecation
    helper prints per process. This change's config warnings feed that helper or the sink (design
    D6), so whichever lands second adapts. The budget keys move to `[drive]`, so the
    `budget_max_cost` error names the table it was read from.
  - `uniform-json-output`: the not-attempted refusal must keep stdout clean under `--json`.
  - v0.24.0 `error-exit-code-contract` inherits `BudgetExhausted` at exit 1.

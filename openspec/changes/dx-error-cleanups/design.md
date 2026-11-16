## Context

Line numbers refer to `main` at v0.20.0 (`e213707`).

**Uncommitted-spec guard.**
- `src/commands/start.rs:682-691` (`start_with_specs`) calls `git::check_uncommitted_specs`. With
  hits and no `force` it warns "Commit your changes or use --force to proceed". With `force` it warns
  "Proceeding with --force flag". Neither branch returns, so execution continues into CLI detection
  (:693) and the launch.
- `src/git.rs:1287-1307` (`check_uncommitted_specs_with`) probes only `<root>/specs/<id>` and
  `<root>/specs/<id>.md`, and `continue`s past anything else (:1305-1306). OpenSpec keeps a change at
  `openspec/changes/<id>/`, so the check never fires for the default format.
- The backends already know where a spec lives: `SpecBackend::spec_location` returns a
  `SpecLocation { path, .. }` (`src/specs/mod.rs:61-70`, :134-151).
- `--force` is defined at `src/cli.rs:235-237`. It reaches only `StartWithSpecsOptions`
  (`src/main.rs:121-135`). The coordinated dispatch builds `SupervisorOptions` without it
  (`src/main.rs:104-117`).
- The spec already requires the refusal: `openspec/specs/supervisor-launch/spec.md:442-474`
  (*Validate specs are committed before launching*: "SHALL NOT launch unless the user confirms or
  `--force` is passed"; it also names `openspec/changes/`).

**Budget.**
- `BudgetCost::new(value) -> Result<Self, String>` is at `src/config/supervisor.rs:363-371`. Its
  `Deserialize` wraps the string with `serde::de::Error::custom` (:380-384). The message surfaces as
  `PawError::ConfigError("<path>: …")` (`src/config/mod.rs:726-727`).
- A run that stops on a cap under `budget_on_exhausted = "abandon"` returns
  `PawError::SessionError("the unattended run ended on a budget cap …")` (`src/main.rs:647-652`).
  That prints as `error: Session: …` (`src/error.rs:44-45`, :134-137) and exits 1 through the
  catch-all arm (:125-131).
- The budget text is built in Rust:
  - `breach()` (`src/supervisor/drive/budget.rs:164-182`) and `start_report()` (:187-208);
  - the unenforceable-cost escalation (:459-469) and the breach escalation (:475-495), both with
    `agent_id = SUPERVISOR_AGENT_ID`.
- `Escalation::question()` has arms for correction and permission verdicts only
  (`src/supervisor/drive.rs:695-721`). A budget verdict falls to the catch-all at :715-719, which
  renders "supervisor is stalled and needs a human decision (budget-exhausted). Please review the
  pane." There is no pane to review. `handoff_text()` likewise uses its generic arm (:767-772).
- Bundled prose already lives under `assets/` and is pulled in with `include_str!` (for example
  `src/task_prompts.rs:69`, `assets/boot-task-prompts/no-spec.md`).

**Stopped rebase at another path.**
- `create_worktree_with` (`src/git.rs:1014-1087`) computes `worktree_path` from the current placement
  only (:1021, `resolve_worktree_path`).
- It asks `find_worktree_for_branch_with` (:1043), which matches only `branch refs/heads/<b>` lines.
  A stopped rebase detaches HEAD, so that lookup returns `None` (:979-980 says so) and the code sets
  `rebase_after_create` (:1057).
- `add_or_reuse_worktree` (:1092-1151) then checks for a detached, rebasing worktree only when the
  computed path exists, and only for the porcelain block whose path equals it (:1105, :1126-1145).
- So a worktree created under the other placement (`Sibling` vs `Child`, `src/config/mod.rs:199-210`,
  :638-640) with a rebase stopped in it is never seen. The code then runs `git worktree add <new
  path> <branch>` (:1156). Git is expected to refuse to check out a branch that is being rebased
  in another worktree (task 5.1 pins this with a real-repo test), so the launch fails with raw git
  stderr ("git worktree add failed …", :1186-1188) instead of the spec's "a rebase is already in
  progress" skip (`openspec/specs/git-operations/spec.md:462-485`).

**Config warnings.**
- `load_config_file` (`src/config/mod.rs:723-749`) `eprintln!`s every `worktree_runtime_warnings`
  line on every load (:736-738), and the deprecated-table warning once per path per process through
  `warn_once_for`, a `static OnceLock<Mutex<HashSet<PathBuf>>>` (:739-743, :785-800).
- The agent-CLI layer returns its warning as a value: `Resolved { profile, warning }`, marked
  `#[must_use]` (`src/agent_cli/profile.rs:626-654`), and `resolve_for_launch` "prints nothing"
  (:664-681). Callers emit through `out.warn` (`src/commands/start.rs:321-329`, :351-353).
- `openspec/specs/code-architecture/spec.md:635-688` (*Core modules report warnings as values*)
  covers only the agent-CLI module and launch resolution.

**Struct-literal ripple (DF-23 / GP-40).**
- `WorktreeEntry` (`src/session.rs:160-161`) and `Session` (:281) derive no `Default`. Production
  builds them with every field spelled out (`src/commands/start.rs:412`, :475), and so do the test
  fixtures.
- `PawConfig` (`src/config/mod.rs:373`) and `SupervisorConfig` (`src/config/supervisor.rs:57`)
  derive `Default`.
- The pattern this change asks for already exists: `DriveConfig { …, ..DriveConfig::default() }`
  (`src/supervisor/drive.rs:3382-3406`).
- DF-23 (v0.19.0 dogfood) recorded workers rewriting literal sites with `sed` / `perl` each time a
  field was added. The exact per-struct site counts are taken by task 7.1, not here.

## Goals / Non-Goals

**Goals:** each message matches what the command does; each failure has a type that names its
cause; the budget text can be read and edited as prose; launch never trips over a stopped rebase it
could have found; warnings reach the caller's sink once.

**Non-Goals:**
- An interactive "launch anyway?" prompt.
- A new exit code for budget exhaustion. That is v0.24.0 `error-exit-code-contract`'s call.
- Moving every Rust-built prompt into `assets/`. Only the budget verdicts move here.
- `Default` on the persisted records (`Session`, `WorktreeEntry`) for production use.

## Decisions

### D1 — Refuse without `--force`; no prompt
The roadmap offered two fixes: refuse, or reword the warning to match today's behaviour. We refuse.
An uncommitted spec is invisible to every worker, because each worktree is cut from a commit that
lacks it, so the launch spends tokens on agents that cannot see their task. The spec and the
message already promise the refusal.

We also drop the spec's "the user confirms" clause instead of adding a prompt:
- `--unattended` and MCP launches have no terminal to prompt on;
- `remove --force` and `purge --force` already set the house pattern of a flag, not a question.

The refusal is `PawError::NotAttempted` (exit 2): nothing was written, and the message names the
flag that proceeds (`src/error.rs:59-69`). The text:

```
2 spec(s) have uncommitted changes and worktree agents would not see them: add-x, fix-y
commit them first, or pass --force to launch anyway
```

The check runs before any worktree, session or tmux state is created. `SupervisorOptions` gains
`force`, so the coordinated path applies the same check.

*Alternative:* reword to "warning: … workers will not see them" and keep launching. Rejected: it
documents a footgun instead of removing it, and leaves the spec wrong.

### D2 — Check each spec at its own location
`check_uncommitted_specs` takes the `SpecLocation` map from `backend.spec_locations(specs_dir,
&entries)` and runs `git status --porcelain -- <location.path>` relative to the repository root. A
spec with no location (gone from disk) is skipped, as today.

### D3 — `PawError::BudgetExhausted(String)` at exit 1
A new variant with `#[error("Budget: {0}")]`, built at `src/main.rs:647-652`. `exit_code()` keeps it
on `ERROR` (1), so scripts see the same status. Only the prefix stops saying "Session".

`BudgetCost::new` returns `Result<Self, InvalidBudgetCost>`. That is a small error type holding
the rejected value, whose `Display` produces today's message byte for byte. Serde still wraps it
with `Error::custom`, so the config error is unchanged.

### D4 — Budget text from `assets/escalation/`
Two templates, `assets/escalation/budget-exhausted.md` and
`assets/escalation/budget-cost-unenforceable.md`, each with a `question` and a `handoff` section and
`{{CAP}}`, `{{USAGE}}`, `{{DETAIL}}` placeholders. They are loaded with `include_str!` and rendered
by a pure function in `budget.rs`.

`Escalation::question()` and `handoff_text()` gain arms for `BUDGET_EXHAUSTED_VERDICT` and
`COST_CAP_UNENFORCEABLE_VERDICT` that call it. The `breach()` / `start_report()` lines stay
`format!`. They are single summary lines, not framing prose, and the rule is about text an agent or
human acts on.

### D5 — Find a stopped rebase by branch, not by path
A new `find_rebasing_worktree_with(runner, repo_root, branch)` walks `git worktree list --porcelain`.
For each `detached` block it reads that worktree's `rebasing_branch_ref` (`src/git.rs:981-988`)
and returns the path whose `head-name` is `refs/heads/<branch>`.

`create_worktree_with` consults it when `find_worktree_for_branch_with` returns `None` (:1057). When
it finds a worktree, that worktree is treated as held: it is judged by `worktree_skip_reason`, reused
at its real path, never re-added at the computed path, and the session records the real path. This
extends the existing reuse at the computed path (:1138-1145) to any path. The spec's "never
re-discovering it" sentence is about the rebase target, which stays the held path.

### D6 — Config warnings as values
`load_config_file` returns `Loaded { config, warnings: Vec<ConfigWarning> }`, where each warning
carries its file path and text. `load_config` merges the global and repository warnings, and
returns them deduplicated within the one call. `warn_once_for` is deleted.

The CLI and each `cmd_*` entry point emit them through their output sink, once per command. A
command that loads the config twice emits from one load only; the others discard the warnings
explicitly. `#[must_use]` on the return value makes a silent drop a compile warning, as with
`Resolved`.

When `supervisor-vocabulary-split` lands, its deprecation entries for config keys ride in the same
`warnings` list. The deprecation helper is then called by the sink, not by the loader, which keeps
that change's "one stderr line per item per process" guarantee.

### D7 — The struct-literal rule
An architecture rule, enforced by a source-audit test like the existing audits
(`code-architecture`, *Source audits read every production line*):
- A struct that is built by a literal at more than three sites outside its defining module has a
  `Default`, derived or written by hand, or a fixture builder in its module's `test_support`.
- A literal of it outside that module ends with `..Default::default()` or uses the builder.

The rule does not cover structs that persist to disk or cross the wire (`Session`, `WorktreeEntry`,
the broker and config wire types). Production code that builds them keeps exhaustive literals, so
the compiler still flags a forgotten field where it matters. Their test sites go through a fixture
builder in the module's `test_support`.

The audit lists the structs it checks (a `const` table), so adding one is a one-line change.

## Risks / Trade-offs

- [A script that ran `start --from-specs` with uncommitted specs now stops] → exit 2 with the flag
  named. The CHANGELOG notes the change.
- [`..Default::default()` hides a new field from a test that should set it] → the rule covers test
  and fixture sites only. The persisted records keep exhaustive production literals.
- [Reusing a worktree at a path other than the configured placement] → that is what the session
  file already records for it. Moving it is the user's call, and `doctor` can report the mismatch
  later.
- [Warnings no longer printed by a forgotten caller] → `#[must_use]`, plus a test that `start` prints
  the deprecated-table warning exactly once.

## Migration Plan

One commit per decision, the suite green after each: D2 then D1 (the guard), D3, D4, D5, D6, D7 (the
audit with a baseline of zero violations, after converting the fixtures). Rollback is a revert; no
persisted state changes.

## Open Questions

- D7's threshold (more than three external literal sites) is a judgment call. The audit's first run
  prints the real counts; adjust the number then if it catches too much or too little.
- Whether `doctor` should flag a worktree that sits outside the configured placement. Out of scope
  here; noted for `session-state-reconciliation`.

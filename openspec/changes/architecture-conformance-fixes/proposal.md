## Why

An architecture-conformance audit of v0.19 against `docs/src/architecture-direction.md`, the `code-standards` skill and the `concept-decoupling` skill found places where the code drifts from the target. The source audit that guards spec-format dispatch is blind to most of some files. One CLI's menu shapes are hardcoded in the app layer. A core module prints to stderr and keeps process-global state. Two commands resolve the same CLI twice. A terminal-adapter operation has no golden argv test. A keystroke wrapper duplicates a blanket impl. The architecture page's status table misses two Strategies that now exist. Each fix is small, and all but F2 preserve behaviour.

## What Changes

- **F1 — source audits read every production line.** `tests/source_audit.rs` `production_part` cuts each file at its first `#[cfg(test)]`, so code after an early `#[cfg(test)]` item escapes the spec-format audit. This affects `src/main.rs`, `src/lib.rs`, `src/supervisor/drive.rs` (about 1,400 production lines follow the first `#[cfg(test)]` at ~1947), `src/terminal/tmux.rs`, `src/tmux/mod.rs` and `src/mcp/query/learnings.rs`. Share `tests/command_library_audit.rs`'s `production_code` (it blanks each `#[cfg(test)]` item by its braces and skips comment lines) through a test helper module, and use it in both audits. The audit also catches `if let SpecBackendKind::X =`, and `src/main.rs` becomes a positive control: a known production line after its first `#[cfg(test)]` must be visible to the audit.
- **F1 twin — CLI names are compared only in `src/agent_cli/`.** A new source audit fails on any production comparison, match arm or `matches!` against `"claude"`, `"codex"`, `"agy"`, `"qwen"` or `"cursor-agent"` outside `src/agent_cli/`. One allowlisted pre-existing site is `src/doctor.rs` `workflow_definition_locations`. Its allowlist entry carries a comment that points at roadmap item F10.
- **F2 — selection-menu shapes come from the CLI prompt profile.** `src/commands/send.rs` `menu_showing` hardcodes the cursor glyphs `❯ › >` and the `enter to select` footer. The pure screen readers `composer_text_is_dim`, `styled_cells` and `apply_sgr` also live in `commands/`. Move all four to a screen module beside `src/supervisor/permission_prompt.rs`. Read the glyphs and the footer from two new profile fields, `menu_cursor_glyphs` and `menu_footer_markers`, whose embedded defaults are today's literals. The fields are appearance only. For `claude` and every CLI without an override, behaviour is unchanged. A `[clis.<name>].prompt_profile` override can now change them, which is new behaviour.
- **F3 — the CLI-profile core returns warnings instead of printing them.** `agent_cli::resolve_for_launch` prints the unsandboxed-full-auto warning with `eprintln!`, deduped by a process-global `static Mutex<HashSet>`. `AgentProfiles::record` also prints with `eprintln!`, and `locate` reads `std::env::current_dir()`. After the change, both return their warning as a value. The launch callers emit it through their `Output` sink, deduped per launch. The drive-loop shell prints the warning from `record`. `locate` takes the working directory as a parameter. A command run through `CaptureOutput` (the MCP path) no longer writes the warning to the process's stderr.
- **F4 — one CLI resolution per command.** `src/commands/send.rs` `deliver` and `src/commands/add.rs` resolve the agent's CLI twice: once through `CliProfile::resolve` and once through the `config::resolve_submit_delay_ms` shim. Each will read both the prompt profile and the settle delay from one `CliProfile`. The structural cycle between `agent_cli` and `config` is roadmap F12 and is out of scope.
- **F5 — rebase-skip policy.** After the rebase onto `start-skips-rebase-on-dirty-worktree`, `src/git.rs` `worktree_skip_reason` calls `crate::agents::is_managed_path`, so the VCS adapter depends on an app-level module. The skip decision stays pure, and the managed-path predicate is supplied from outside the adapter if that is small. Otherwise a note goes on the roadmap.
- **F6 — styled capture is pinned.** `TmuxBackend::capture_styled` (`capture-pane -p -e`) gets a golden argv test. `FakeBackend` records styled captures as their own operation.
- **F7 — one keystroke dispatcher.** `KeyDispatcher`'s methods take `&self`, because every terminal backend's methods already do. `SubmitPort::dispatcher` and the drive deps' `dispatcher` become `&dyn KeyDispatcher`. `send.rs`'s `BackendDispatcher` wrapper and the drive tests' `SharedFakeDispatcher` are deleted, and the backend's blanket impl in `src/supervisor/keys.rs` is used directly.
- **F16 — git-operations spec drift (spec and docs only).** `start-skips-rebase-on-dirty-worktree` left the older "Create worktrees as siblings of the repository" requirement saying the creation-time rebase and its abort run "from the repository root", "before the existence check", and that the creation-time contract is "unchanged". It also left "Mid-session rebase of a live worker branch" with the same "unchanged" wording. Both contradict the newer requirements: the rebase runs only inside the branch's own worktree, and a stopped rebase is never aborted. Both are MODIFIED to state the current rule and keep every scenario. Three `git-add-remove-branch` requirements that restate the git-operations rules now reference them instead. The "add keeps its notice text" scenario names the exact notice. `docs/src/user-guide/mcp.md` says that `start_session` returns skipped-rebase notices as warnings.
- **F17 — worker-usage-budget post-merge audit.**
  - Spec contradiction: `supervisor-unattended-operation` "Every live permission prompt is escalated without blocking the wave" says any recorded escalation makes a wave `escalated-for-review`. The newer budget requirement and `budget::awaits_review` exempt the unevaluable-cost-cap notice; the older requirement now names that exception.
  - Missing scenarios: headless orchestrator spend counts toward a breach; a cost cap breached by reported cost alone; an unknown `budget_on_exhausted` is rejected; a CLI session git-paw did not launch is excluded from status usage.
  - The doctor's cost-cap requirement names its configured-CLI set: `default_cli`, `default_spec_cli` and the presets.
  - Docs: the budget exit is added to the supervisor guide's exit list, and one sentence about the budget goes in the `--unattended` help and the CLI reference.
  - Two code comments are corrected: `status.rs` "Exactly the figure `budget_max_tokens` caps", and `doctor.rs` "Same condition as the drive loop".
- **F18 — worker-friction-fixes post-merge hygiene.**
  - `skill-agent` "Supervisor skill — Merge orchestration" item 3 still mandates `git checkout main` / `git merge --ff-only feat/<branch>`. That contradicts "Orchestrator skill confirms a merge landed before archiving" and the shipped skill. It now says to check out the resolved merge target, run `git merge --ff-only <agent-branch>` and confirm with `git merge-base --is-ancestor`; the per-branch loop scenario gains the ancestor check.
  - `docs/src/user-guide/supervisor.md` "Per-branch merge" is rewritten to match the skill's Step 2.
  - `docs/src/user-guide/coordination.md` "Activity phrases" and "Declaring regions" lead with `broker.sh status --activity …` / `broker.sh intent … --region …`. The curl body stays only as a wire-shape aside.
  - `src/agents.rs` `WORKER_ROLE_LINE` (agent-facing prose) moves to an asset loaded with `include_str!`.
  - The test `pointer_is_scoped_to_the_root_session` normalises whitespace before matching.
  - `docs/src/user-guide/doctor.md` says workflow locations resolve by CLI family.
  - The `assets/scripts/broker.sh` usage heredoc aligns its `[--region …]` line.
  - Three scenario retitles were briefed: preflight-diagnostics "An unknown CLI is reported, not warned", and skill-agent "Coordination skill retains blocked and artifact curl examples" and "Table includes claude and claude-oss explicitly". OpenSpec cannot express them: a MODIFIED block that renames a scenario fails strict validation as an omitted scenario, and a REMOVED plus ADDED pair of the same requirement is rejected. The titles stay, as the existing in-spec note on the claude-oss scenario already does, and the scenario bodies already assert the current behaviour.
- **F9 — docs.** The "Where the code is today" table in `docs/src/architecture-direction.md` gains rows for the per-CLI Strategy (`agent_cli`: `CliFamily` / `CliProvider`, the generic Null Object, `AgentProfiles` resolved once per agent) and for the spec-format Strategy (`SpecBackend`, the single `backend()` match, enforced by the source audit). The Drive loop row notes that `submit_and_verify` is shared with `__send`. `docs/src/architecture.md` is kept consistent with both. It also gains the usage facet on the Agent CLIs row (`agent_cli/usage.rs`, `CliProvider::usage` / `reports_cost`, `UsageCache`), and a `supervisor/drive/budget.rs` row (`BudgetGuard`, the `UsageReader` port, `UsageLedger`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `code-architecture`: three new requirements: source audits read every production line (F1), core modules report warnings as values (F3), and keystrokes reach a surface through the backend's own dispatcher (F7). A fourth new requirement says that every terminal-adapter read the drive loop and `__send` use is pinned by a golden argv test (F6).
- `cli-provider`: a new requirement that per-CLI names are compared only inside the agent CLI module (F1 twin).
- `cli-prompt-profiles`: a new requirement that a selection menu's shape comes from the profile (F2).
- `git-operations` (F16): MODIFIED "Create worktrees as siblings of the repository", "Mid-session rebase of a live worker branch" and "Every launch path reports a skipped rebase".
- `git-add-remove-branch` (F16): MODIFIED "add never rebases over uncommitted work", "add judges uncommitted work as remove does" and "add treats an unreadable worktree as dirty", which now reference git-operations.
- `supervisor-unattended-operation` (F17): MODIFIED "Every live permission prompt is escalated without blocking the wave" (the cost-cap exception) and "The drive loop detects a budget breach" (two scenarios).
- `supervisor-config` (F17): MODIFIED "The budget exhaustion policy" (invalid-value scenario) and "Doctor flags a cost cap no worker CLI can satisfy" (the configured set).
- `session-state` (F17): MODIFIED "Status JSON exposes attributed usage" (excluded-session scenario).
- `skill-agent` (F18): MODIFIED "Supervisor skill — Merge orchestration" (resolved branch names, ancestor check).

## Impact

- **Code**:
  - `tests/source_audit.rs`, `tests/command_library_audit.rs` and a new shared helper under `tests/helpers/` (F1, F1 twin).
  - `src/commands/send.rs` and a new `src/supervisor/screen.rs` (or similar sibling of `permission_prompt.rs`) for F2, F4 and F7.
  - `src/config/prompt_profile.rs` for the two new fields, their defaults, override merge and validation (F2).
  - `src/agent_cli/profile.rs` and `src/agent_cli/mod.rs` (F3).
  - Callers of `resolve_for_launch` (F3): `src/commands/start.rs` (3 sites: ~277, ~776, ~1330), `src/commands/recover.rs:39`, `src/commands/supervisor.rs:623`, `src/commands/launch.rs` (~204, ~334).
  - Callers of `AgentProfiles::record` (F3): `src/supervisor/drive.rs` (~1351, ~1398).
  - `src/commands/add.rs` (~178, ~416) for F4.
  - `src/supervisor/keys.rs` and `src/supervisor/drive.rs` (`DriveDeps`-style struct ~1081 and `SubmitPort` ~2239, and the test dispatchers ~3620 and ~5530) for F7.
  - `src/terminal/tmux.rs` and `src/terminal/fake.rs` for F6.
  - `src/git.rs` and its caller, only if F5 applies after the rebase.
- **Struct-literal ripple**: `CliPromptProfile` gains `menu_cursor_glyphs` and `menu_footer_markers`. The literal sites are `claude_prompt_profile` (`src/config/prompt_profile.rs` ~183) and the merge constructor (~501). `CliPromptProfileOverride` gains two `Option` fields; its literals are in `src/config/prompt_profile.rs` tests (~647, ~680, ~731, ~777) and any other `CliPromptProfileOverride {` site found by grep after the rebase. `FakeBackend`'s `Operation` enum gains a `CaptureStyled` variant; its exhaustive `match` (~146) must place it.
- **Wire / config**: two new optional `prompt_profile` keys, `#[serde(default)]` with `skip_serializing_if`. Existing configs load unchanged. `git paw __prompt-profile` can print the new fields. No broker, session-file or CLI flag changes.
- **F17 code**: comments only, in `src/commands/status.rs` and `src/doctor.rs`, plus the `--unattended` help in `src/cli.rs`. Tests cover the new scenarios; tests that already exist get scenario references.
- **Docs**: `docs/src/architecture-direction.md`, `docs/src/architecture.md`, `docs/src/user-guide/cli-prompt-profiles.md`, the configuration reference (the two new profile fields), `docs/src/user-guide/mcp.md` (F16), `docs/src/user-guide/supervisor.md` and `docs/src/cli-reference.md` (F17), `docs/src/user-guide/supervisor.md` "Per-branch merge", `docs/src/user-guide/coordination.md` and `docs/src/user-guide/doctor.md` (F18).
- **F18 code**: `src/agents.rs` (`WORKER_ROLE_LINE` to a new asset under `assets/`, plus one test) and `assets/scripts/broker.sh` (usage text only).
- **Out of scope**: the `agent_cli` ↔ `config` cycle (F12) and the `doctor.rs` per-CLI table (F10).
- **Concurrent branches**: `start-skips-rebase-on-dirty-worktree`, `mcp-spec-artifacts-per-format`, `worker-usage-budget`, `v019-review-followups` and `worker-friction-fixes` touch overlapping files. This change is rebased onto them before implementation, and the file:line references above are re-located then.

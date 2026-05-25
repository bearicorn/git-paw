## Why

In v0.4, `git paw start --supervisor` runs the supervisor agent in the user's foreground terminal — `cmd_supervisor` blocks on `Command::new(supervisor_cli).status()` until the user exits the supervisor CLI, then runs the merge loop and returns. Three problems with this surfaced during v0.4 dogfood:

1. **Non-TTY launches can't run the supervisor.** Without an interactive TTY, the supervisor CLI can't render. Today's workaround (the `from-specs-launch-fixes` change) skips the supervisor CLI entirely on non-TTY, leaving the session half-initialized.
2. **No way to follow the supervisor's reasoning passively.** The user IS the supervisor's terminal — to look at agent panes you switch via `tmux attach`, but you can't have both the supervisor's reasoning and the agent panes visible side-by-side without manual tmux gymnastics.
3. **Reattach loses the supervisor.** Close your terminal accidentally and the supervisor CLI dies (it was a child of your shell, not of tmux). Reattaching to the tmux session shows the dashboard + agents but the supervisor is gone.

This change moves the supervisor agent into a tmux pane like every other agent. The user attaches to the tmux session and sees the supervisor + dashboard in the top row (50/50 split), with the coding agent grid below. The user interacts with the supervisor by switching to its pane and typing — same model as any other agent CLI in a tmux pane.

## What Changes

**Pane layout.** The supervisor session's pane structure becomes:

```
┌─────────────────────────────┬──────────────────────────────┐
│  pane 0: supervisor (50%)   │  pane 1: dashboard (50%)     │  TOP ROW
├──┬──┬──┬──┬──┬──────────────┴──────────────────────────────┤
│ 2│ 3│ 4│ 5│ 6│  ← agent row 1 (up to 5 panes)              │
├──┴──┴──┴──┴──┤                                             │  AGENT GRID
│ 7│ 8│ 9│..│ N│  ← agent row 2..M                           │
└──┴──┴──┴──┴──┴─────────────────────────────────────────────┘
```

- **Top row** is split 50/50 horizontally between `pane 0` (supervisor agent) and `pane 1` (dashboard TUI).
- **Agent grid** below uses up to 5 columns per row. Total agent rows = `ceil(agents / 5)`.

**Top-row vs agent-row height proportions** (dynamic by total-row count):

| Total rows | Top row | Each agent row |
|---|---|---|
| 2 (≤5 agents) | 60% | 40% (single agent row) |
| 3 (6-10 agents) | 40% | 30% each (2 agent rows) |
| 4 (11-15 agents) | 28% | 24% each (3 agent rows) |
| 5 (16-20 agents) | 28% | 18% each (4 agent rows) |
| 6 (21-25 agents) | 28% | 14.4% each (5 agent rows) |

**Hard cap at 25 agents.** Above 25, the launch SHALL refuse with an actionable error pointing at session-splitting via `--branches <subset>`. The configurable `[layout] max_agents` override and beyond-25-agent extrapolation rule are deferred to v1.0.0 (v1.0.0 issue #17).

**`cmd_supervisor` returns control immediately after launching.** v0.4's blocking-on-foreground-CLI model goes away. Once the panes are set up and boot prompts injected, `cmd_supervisor` returns `Ok(())` and the user gets their terminal back. They attach to the tmux session via `tmux attach -t paw-<project>` to interact with the supervisor pane like any other agent.

**Supervisor pane gets a boot prompt via `tmux send-keys`.** Same mechanism as the coding agents: after a 2-second sleep, the supervisor pane receives the rendered supervisor skill content + the boot block + a "Begin observing" prompt. The supervisor agent's `agent_id = "supervisor"` self-registers in the broker the same way coding agents do (via the post-prompt curl-status flow that the supervisor skill describes).

**Dashboard moves from pane 0 to pane 1.** All downstream pane-index references (`pane_offset` calculations, send-keys targets, auto-approve pane-map keys) shift by +1 since pane 0 is now the supervisor.

**Auto-approve thread relocates.** v0.4's `spawn_auto_approve_thread` runs inside `cmd_supervisor`'s process; it dies when `cmd_supervisor` returns. With early-return, the auto-approve thread needs to live elsewhere — relocated into the dashboard's `__dashboard` subprocess (which already runs the broker + TUI as a long-lived process inside the dashboard pane). The auto-approve thread becomes part of that process.

**Non-TTY launches now work end-to-end.** With no foreground supervisor CLI, the entire launch flow is detached. The non-TTY check inserted by `from-specs-launch-fixes` becomes a no-op for supervisor mode (the launch always succeeds in detached mode regardless of TTY state).

**Supervisor skill — interactive user input.** With the supervisor running in a pane the user can attach to and type in directly, the existing `supervisor.md` skill (which assumes the supervisor runs autonomously between user check-ins) needs a small addition explaining how to handle user input mid-flow. The mechanisms are unchanged (the skill already covers `curl /status`, `tmux capture-pane`, `agent.feedback`, `tmux send-keys`, `agent.question`); the new content is *when to use which* in response to user input. Three cases, all using existing mechanisms:

1. **Status questions** ("how's X going?") — answer conversationally using `/status` + `tmux capture-pane`; don't publish to the broker.
2. **Directives** ("ask X to use bcrypt") — publish `agent.feedback` to X (or `tmux send-keys` for low-stakes nudges) AND confirm to the user conversationally.
3. **Judgment-call asks** — apply normal escalation rules; only `agent.question` to the dashboard if the call is genuinely ambiguous beyond what the user provided.

The autonomous loop continues alongside user input — the skill states explicitly that the supervisor finishes the current step (e.g. spec audit) before responding, then resumes.

**Merge orchestration moves into the supervisor skill (not Rust code).** v0.4 had a Rust function `run_merge_loop` that `cmd_supervisor` called after the foreground CLI exited. The function: reads `agent.artifact` + `agent.blocked` events from the broker, builds a dependency graph, topologically sorts, and for each branch in order does `git merge --ff-only` + runs the configured test command + publishes verify/feedback messages.

In v0.5.0 the supervisor *agent* — running in its own pane with skill access to shell commands, curl, and git — can do all of this via skill instructions. Reading messages: `curl /messages/supervisor`. Topological sort: the supervisor agent reasons about it. Merging: `git merge <branch>`. Running tests: `<test_command>`. Publishing results: `curl /publish`. Same outcome, no Rust subsystem.

This change therefore:
- Removes `run_merge_loop` (and `MergeResult`, related types) from `cmd_supervisor` — no Rust merge subsystem in supervisor mode any more.
- Adds a "Merge orchestration" section to `supervisor.md` covering: when to merge (after all spec'd agents publish `agent.verified`), how to compute the merge order from `agent.blocked` dependency events, the per-branch merge + test + publish loop, what to do on regression (revert the merge, publish `agent.feedback`).
- Aligns with the v1.0.0 theme: supervisor does supervising; git-paw provides plumbing.

Trade-off: supervisor LLM is now responsible for correctness of the merge ordering and regression handling. The skill provides examples and a step-by-step procedure, but the agent applies judgment. v0.4's deterministic Rust implementation handled this with topological-sort code; the supervisor LLM does it via skill prose. Acceptable for v0.5.0 — fits the broader "trust the LLM, give it tools" stance the v0.5.0 governance changes already set.

Not in scope:
- A standalone `git paw merge` subcommand — superseded by the "merge orchestration is supervisor-skill territory" decision above. If users want non-supervisor auto-merge in some future release, that's its own design exercise.
- Configurable `[layout] max_agents` and beyond-25 layout extrapolation (v1.0.0 #17).
- Solving D9 (boot prompts not submitted because of Claude paste-handling) — separate change.
- Solving D3 layout for non-supervisor sessions (`cmd_start`, `cmd_start_from_specs`) — those keep their existing layouts in this change.
- Per-CLI supervisor pane styling (e.g. larger pane for talkier CLIs).
- **Dashboard inbox / log panel rationalisation** — the user pointed out that with the supervisor as a pane, the dashboard's prompt-inbox panel becomes mostly redundant (the supervisor reads agent.question messages and surfaces them via the supervisor pane; agents needing input show via their `status` field on the dashboard table, which is enough signal). Restructuring the dashboard to drop the inbox panel and possibly add a broker-log view is **deferred to v0.6.0** since MCP integration may reshape what the dashboard surfaces anyway. v0.5.0 leaves dashboard internals unchanged; the dashboard pane just moves from index 0 to index 1.

## Capabilities

### New Capabilities
*(none — all changes modify existing capabilities)*

### Modified Capabilities
- `supervisor-launch`: pane structure, height proportions, return semantics (no blocking on foreground CLI), the "Supervisor self-registration" + "All agents receive boot blocks" requirements extend to the supervisor pane being one of the boot-block recipients.
- `supervisor-cli`: the resolution chain still drives `cmd_supervisor` invocation, but `cmd_supervisor`'s return semantics change — returns immediately after launching the session, doesn't block on a foreground CLI.
- `supervisor-injection`: the "boot block prepended to each agent's task prompt" requirement now applies to the supervisor pane as well as the coding agent panes.
- `broker-lifecycle`: dashboard pane moves from pane 0 to pane 1; the `__dashboard` subcommand grows the auto-approve thread responsibility (or, alternatively, the auto-approve subsystem becomes broker-internal — design D-decision).
- `tmux-orchestration`: new layout shape (50/50 top row + dynamic agent grid with the height-proportion table); 25-agent hard cap.
- `agent-skills`: two additive sections in `supervisor.md`:
  - **"When the user types in your pane"** — three cases (status questions, directives, judgment-call asks) mapped to existing mechanisms (`curl`, `tmux capture-pane`, `agent.feedback`, `tmux send-keys`, `agent.question`). The autonomous loop is unchanged; the addition is "what to do when the user types in your pane mid-loop."
  - **"Merge orchestration"** — replaces the v0.4 Rust `run_merge_loop`. Tells the supervisor agent how to read `agent.artifact` + `agent.blocked` events to compute merge order, perform per-branch `git merge` + test runs, and publish verify/feedback messages. Includes regression-handling guidance (revert + agent.feedback).

## Impact

**Code**:
- `src/main.rs::cmd_supervisor` — significant restructure:
  - Build session with pane 0 = supervisor (Claude), pane 1 = dashboard, panes 2..N+1 = agents.
  - Compute layout proportions from agent count.
  - Inject boot prompts into all panes including supervisor (pane 0).
  - Remove the foreground-CLI launch (`Command::new(supervisor_cli).status()`).
  - Remove `run_merge_loop` call.
  - Return `Ok(())` after session save + send-keys + supervisor self-register.
- `src/main.rs::cmd_dashboard` (and the `__dashboard` subcommand entry point in `src/lib.rs` or wherever) — gain the auto-approve thread spawn.
- `src/tmux.rs` — new layout-builder methods that produce the 50/50 top row + dynamic agent grid. Hard cap of 25 enforced at session-build time with a `PawError` if exceeded.
- `src/main.rs::run` dispatcher — `cmd_supervisor` is invoked the same way; only its internal behaviour changes.
- `src/main.rs::recover_session` — recovery logic must rebuild the new layout shape.
- `assets/agent-skills/supervisor.md` — minor: the existing skill content already targets a supervisor that observes other agents. The "in foreground terminal" framing in any prose should be removed.
- `docs/src/user-guide/supervisor.md` (or wherever) — restructure the user-facing model: "supervisor is now a pane; attach to interact."

**Tests**:
- Pane layout: with N agents (1, 5, 10, 15, 20, 25), the resulting tmux session has the expected pane count, pane 0 is supervisor, pane 1 is dashboard, agent panes start at index 2.
- Pane proportions: top row's height proportion matches the table for each agent-count bucket.
- Hard cap: launch with 26 branches errors with the expected message; launch with 25 succeeds.
- Boot-block injection: supervisor pane (pane 0) receives a send-keys with the supervisor skill content + boot block.
- Return semantics: `cmd_supervisor` returns `Ok(())` without blocking on any process; observable via `assert_cmd::Command::output()` returning promptly even with `--supervisor`.
- Self-registration: after launch, broker `/status` lists `agent_id = "supervisor"` AND each `feat-<change>` agent.
- Recovery: `recover_session` rebuilds the new layout when reconnecting to a previously-stopped session.
- Non-TTY: `assert_cmd` end-to-end with stdin redirected from `/dev/null` succeeds (no special-case for non-TTY needed since the new flow is detached unconditionally).

**Backward compatibility**:
- Loud release-notes call-out: **the supervisor mode UX changes from "your terminal is the supervisor" to "the supervisor is a pane in tmux."** Users who scripted around the foreground-CLI model need to update.
- The merge loop is removed from `cmd_supervisor`. Users relying on auto-merge after supervisor exits get a regression until the follow-up `git paw merge` change ships. Document loudly.
- v0.4 saved sessions (in `.git-paw/sessions/`) reference the old pane-index assumptions. Recovery logic SHALL detect old session state and rebuild with the new layout, OR the next `git paw recover` SHALL warn that the session was created with v0.4 layout and is being restarted with v0.5 layout.

**Mismatches surfaced by this change**:
- D2 partial-fix in `from-specs-launch-fixes` (non-TTY supervisor skip) becomes mostly redundant once supervisor is a pane — the launch is always detached. The `from-specs-launch-fixes` non-TTY hint code remains for `cmd_start` and `cmd_start_from_specs` but the `cmd_supervisor`-specific `is_interactive_stdin()` check + supervisor-CLI skip can be simplified or removed.
- The auto-approve thread relocation is a subscope; the design doc clarifies whether it lives in the dashboard process, the broker process, or as its own subsystem.

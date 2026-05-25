## Context

v0.4 supervisor mode runs the supervisor agent (Claude with `supervisor.md` as `AGENTS.md`) in the user's foreground terminal. `cmd_supervisor` blocks on `Command::new(supervisor_cli).status()` until the user exits the supervisor CLI; on exit it runs the merge loop and returns. The dashboard runs as pane 0 of the tmux session, coding agents in panes 1..N.

Three problems with that:
1. Non-TTY launches can't run the supervisor (no terminal to render Claude into).
2. The user can't see the supervisor's reasoning AND the agent panes side-by-side without ad-hoc tmux gymnastics.
3. Closing the user's terminal accidentally kills the supervisor process; the tmux session survives but is now without supervision.

This change moves the supervisor agent into a tmux pane like every other agent. `cmd_supervisor`'s job becomes "build the right tmux layout, inject boot prompts everywhere, return control to the user." The supervisor pane is structurally identical to any other agent pane — same `tmux send-keys` injection, same broker integration, same skill resolution.

The user's mental model shifts from "my terminal IS the supervisor" to "the supervisor is a pane in tmux, alongside the dashboard and the agent grid."

## Goals / Non-Goals

**Goals:**
- Move the supervisor agent into pane 0 of the tmux session; dashboard moves to pane 1; coding agents start at pane 2.
- Top row splits 50/50 horizontally between supervisor and dashboard; agent grid below uses up to 5 cols/row with the height-proportion table from the proposal.
- Hard cap at 25 agents (5 cols × 5 rows) for v0.5.0; configurable extension deferred to v1.0.0 #17.
- `cmd_supervisor` returns immediately after launching the session — no foreground-CLI blocking, no merge loop call.
- All agent panes (including supervisor) get their boot prompts injected via `tmux send-keys` after a 2-second boot delay.
- Auto-approve thread relocates so it survives `cmd_supervisor`'s early return.
- Non-TTY launches now work end-to-end — the launch flow is unconditionally detached.
- Small additive section in `supervisor.md` covering interactive user input.

**Non-Goals:**
- The `git paw merge` follow-up subcommand. v0.5.0 ships without auto-merge in supervisor mode; document the regression.
- Configurable `[layout] max_agents` / `agents_per_row` / extrapolation beyond 25 — v1.0.0 #17.
- Solving D9 (boot prompts not submitted because of Claude paste-handling) — separate change.
- Layout changes for non-supervisor `cmd_start` / `cmd_start_from_specs` paths — those keep their existing layouts.
- Per-CLI supervisor pane styling.
- Replacing the existing `tmux::TmuxSessionBuilder` pattern; the new layout is built by extending it.

## Decisions

### D1. Pane assignment: supervisor at index 0, dashboard at index 1

Why pane 0 for the supervisor and not pane 1 (which would keep the dashboard at 0 unchanged)? Two reasons:

- **Convention preserved**: pane 0 has historically been "the most prominent pane the user looks at first." In v0.4 supervisor mode, the dashboard was the operator's primary observability surface. With this change, the supervisor *agent* takes over that role — it's where the user types, what they read most. Pane 0 reflects that primacy.
- **Index stability for downstream code**: the offset that determines where coding agents start (`pane_offset`) was `1` in v0.4 (offset by 1 for the dashboard). It stays `2` in this change (offset by 2: supervisor at 0, dashboard at 1). One number bump in code; downstream `pane_idx = idx + pane_offset` arithmetic continues to work uniformly.

Considered alternatives:
- Pane 1 for supervisor, dashboard at 0: keeps v0.4 dashboard-at-0 contract, but then pane 1 is supervisor (singular role) sitting between dashboard and agents — less intuitive layout.
- Two windows (window 0 = dashboard + supervisor; window 1 = agents): operator has to switch windows to see agents — defeats the "side-by-side" goal.

### D2. Layout construction: split + resize, not `select-layout`

tmux's built-in `select-layout tiled` (or `even-horizontal`, `even-vertical`) is appealing because it auto-arranges panes, but it has two issues for our case:
- It doesn't natively support "one big top row + multiple shorter agent rows of different heights."
- The pane-index ordering after a layout switch isn't always predictable, which breaks our `pane_offset` arithmetic.

Instead, build the layout manually using `split-window` with explicit percentages:

```
1. tmux new-session -d -s paw-<project> ... (creates window 0 with pane 0 = supervisor)
2. tmux split-window -h -t paw-<project>:0.0 -p 50 ... (splits horizontally, dashboard at pane 1)
3. tmux split-window -v -t paw-<project>:0.0 -p <100-top%> ... (splits vertically below the top row)
4. For each subsequent agent row, split-window -v at the appropriate position.
5. Within each agent row, split-window -h to add agents up to 5 per row.
6. tmux send-keys ... (per pane)
7. tmux resize-pane -t <pane> -y <height%> ... (final pass to enforce exact proportions)
```

The `tmux::TmuxSessionBuilder` pattern grows a new builder method `build_supervisor_layout(supervisor_pane, dashboard_pane, agent_panes, agents_per_row, height_table)` that produces the right sequence of `split-window` and `resize-pane` commands. Pane indices end up assigned predictably: supervisor=0, dashboard=1, agents=2..N+1.

### D3. Height proportions: lookup table by total-row count

```rust
fn top_row_height_pct(total_rows: usize) -> u8 {
    match total_rows {
        2 => 60,       // 1 agent row (1-5 agents)
        3 => 40,       // 2 agent rows (6-10 agents)
        4 => 28,       // 3 agent rows (11-15 agents)
        5 => 28,       // 4 agent rows (16-20 agents)
        6 => 28,       // 5 agent rows (21-25 agents)
        _ => unreachable!("agent count > 25 must be rejected upstream"),
    }
}

fn agent_row_height_pct(total_rows: usize) -> u8 {
    let top = top_row_height_pct(total_rows);
    let agent_rows = total_rows - 1;
    (100 - top as u16 / agent_rows as u16) as u8
}
```

Layout calculation:
1. `agents_per_row = 5` (hard-coded for v0.5.0; configurable in v1.0.0 #17).
2. `agent_rows = ceil(agents / agents_per_row)`.
3. `total_rows = agent_rows + 1`.
4. Pull `top_row_height_pct(total_rows)` and `agent_row_height_pct(total_rows)` from the table.
5. Apply via `resize-pane -y <pct>` after creating panes.

### D4. Hard cap at 25 agents

```rust
const MAX_AGENTS: usize = 25;

if agents.len() > MAX_AGENTS {
    return Err(PawError::ConfigError(format!(
        "{count} agents requested; maximum is {MAX_AGENTS} per session.\n\n\
         Split into multiple sessions:\n\
           git paw start --branches <subset>\n\n\
         (Configurable max_agents is planned for v1.0.0 — see milestone.)",
        count = agents.len()
    )));
}
```

Enforced in `cmd_supervisor` before any tmux commands run. The error includes the session-splitting hint so users have a clear path forward.

### D5. `cmd_supervisor` flow restructure

New flow (compare to v0.4 flow described in `supervisor-launch` spec step 1-11):

```
1. Load config; resolve supervisor CLI; validate.
2. Resolve branches (--branches > scan_specs > error).
3. Hard-cap check: agents.len() <= 25.
4. Compute layout: agents_per_row, agent_rows, total_rows, height proportions.
5. Build tmux session in detached mode using the new layout helper:
   - Pane 0: supervisor agent (Claude with supervisor.md AGENTS.md, in repo_root)
   - Pane 1: dashboard (git-paw __dashboard, in repo_root)
   - Panes 2..N+1: coding agents (Claude with agent AGENTS.md, in worktree)
6. Inject GIT_PAW_BROKER_URL into the session env (broker enabled case).
7. Save session state.
8. Sleep ~2s for panes to boot.
9. tmux send-keys boot prompts to all panes including supervisor (pane 0).
10. Self-register supervisor via broker POST (agent.status with "Supervisor booting").
11. Print "Supervisor session 'paw-<project>' launched. Attach with: tmux attach -t paw-<project>"
12. Return Ok(()).
```

Step 11 (auto-attach when TTY) is dropped — the user always gets their terminal back regardless of TTY state. They explicitly attach when ready. This unifies TTY and non-TTY launches.

**The Rust merge loop is removed entirely** (not relocated). v0.4's `run_merge_loop` function and its `MergeResult` types come out of `cmd_supervisor`. Merge orchestration moves to the supervisor agent's skill: the agent reads `agent.artifact` + `agent.blocked` events from the broker, computes the dependency-aware merge order, runs `git merge` + test commands per branch, and publishes results. See D11 for the skill content.

### D6. Auto-approve thread relocation

v0.4: `spawn_auto_approve_thread` runs as a thread inside `cmd_supervisor`'s process. It joins after the foreground supervisor CLI exits.

New: `cmd_supervisor` returns immediately, so a thread inside its process dies. The auto-approve thread needs to live in a longer-lived process. Two candidates:

| Option | Where the thread lives | Pros | Cons |
|---|---|---|---|
| **A. Dashboard process (chosen)** | The `git-paw __dashboard` subprocess running in pane 1 | Already long-lived; has direct broker state access; dashboard panel can show auto-approve hits | Auto-approve dies when dashboard pane closes (acceptable — user expects supervision off when dashboard's gone) |
| B. Broker-internal | A new task in the broker process at startup | Cleanest separation | Requires broker to know about CLI panes; tight coupling between broker and tmux |

Chose A. The dashboard process already runs the broker (per `dashboard` capability — `__dashboard` subcommand starts the broker via `start_broker(...)` and runs the TUI). Adding `spawn_auto_approve_thread` to that process is a small addition; the thread reads the same broker state the TUI is rendering.

The pane_map (agent_id → pane_idx) needed by the auto-approve thread now needs +1 offset for the supervisor pane. Pass it from cmd_supervisor when launching the dashboard via env vars or a session-state file.

### D7. Boot prompt for the supervisor pane

The supervisor pane's boot prompt mirrors the agent boot prompt shape:

```
<boot block — set BRANCH_ID=supervisor, GIT_PAW_BROKER_URL=..., publish-status patterns>

Begin observing the v0.5.0 spec implementation session. Your skill (AGENTS.md)
describes your role — read it, then start the autonomous loop.

The user can type questions or directives directly into your pane. Distinguish
status questions, directives, and judgment-call asks per the skill's
"interactive user input" section.
```

The `BRANCH_ID = supervisor` is what makes the supervisor publish under its canonical agent_id. The skill resolution writes `supervisor.md` content into the repo-root `AGENTS.md` (existing v0.4 behaviour); the boot prompt tells Claude to read that AGENTS.md.

### D8. Supervisor self-registration: keep the explicit POST

v0.4's `publish_to_broker_http(broker_url, build_status_message("supervisor", "working", Some("Supervisor booting")))` happens in `cmd_supervisor` before the foreground CLI launch. Keep that POST in the new flow at step 10. It pre-populates the broker's agent record so the dashboard shows `supervisor` immediately, before the supervisor pane's Claude has finished booting.

The supervisor pane's Claude later publishes its own `agent.status` updates per the skill's autonomous loop — those just add to the existing record.

### D9. Recovery flow: rebuild with new layout

`recover_session` (in `src/main.rs`) currently rebuilds the v0.4 supervisor session shape. With the new layout, recovery needs to:
- Detect that the saved session was a supervisor session (via the `Session` struct's existing fields, possibly extended with a `mode: SessionMode` enum if not already present).
- Apply the new layout helper instead of the v0.4 layout.

For v0.4 saved sessions encountered during a v0.5 binary run: simplest behaviour is "warn and rebuild with v0.5 layout" (the saved state is mostly worktrees + branches; the tmux session itself is recreated from scratch). No state migration needed since the layout is determined at recreation time.

### D10. Skill addition #1: "When the user types in your pane"

The new section in `supervisor.md` is purely additive — three named cases (status question, directive, judgment-call ask), each mapping to existing mechanisms (curl /status, agent.feedback publish, agent.question publish). The existing autonomous-loop content is unchanged.

Adding ~30 lines of skill prose. Part of the `agent-skills` capability delta.

### D11. Skill addition #2: "Merge orchestration"

Replaces the v0.4 Rust `run_merge_loop` function with skill-level instructions. The supervisor agent already has all the tools needed: `curl` for broker reads + publishes, shell for `git` and the configured test command, `tmux capture-pane` for any pane introspection. Adding merge orchestration is just more skill prose, not new mechanisms.

Skill content covers:

1. **When to run merge orchestration** — when all expected agents have published `agent.verified` (or after the user explicitly asks).
2. **Compute merge order** — read `agent.blocked` events from the broker. For each `agent.blocked` from X with `payload.from = Y`, that's an X-depends-on-Y edge. Topologically sort. Cycles fall back to arbitrary order (with a `agent.question` to the user surfacing the cycle).
3. **Per-branch merge loop**:
   - `git checkout main && git merge --ff-only feat/<branch>` — only fast-forward; never create merge commits.
   - On conflict / non-FF: `agent.feedback` to the branch's agent listing the conflict; SKIP merging that branch.
   - On success: run the configured `test_command`. If it fails, `git reset --hard <prev-HEAD>` (revert), `agent.feedback` to the branch's agent. If it passes, proceed to the next branch.
4. **Final summary** — when all eligible branches are merged (or skipped), publish a final `agent.status` summarising the result.

This shifts merge correctness from deterministic Rust code to LLM-driven reasoning. The skill provides the procedure; the LLM applies judgment for edge cases. Acceptable trade-off for v0.5.0 — fits the broader v0.5.0 stance (e.g. governance-context, conflict-detection skill side) of "give the LLM the tools and the procedure; trust its judgment with human escalation as a fallback."

`run_merge_loop`, `MergeResult`, `MergeResults` types in `src/main.rs` (and any related helpers) are deleted by this change.

Adding ~80 lines of skill prose for the merge procedure.

## Risks / Trade-offs

- **[Risk] LLM-driven merge orchestration loses determinism.** v0.4's Rust merge loop produced deterministic ordering; supervisor LLM reasoning may produce slightly different orderings across runs given the same input. → **Mitigation:** the topological sort itself isn't ambiguous given clean dependency edges; the skill's procedure is explicit. Edge cases (cycles, unclear precedence) escalate to the user via `agent.question`. Worst case = same as any other LLM-driven supervisor decision.
- **[Risk] User runs `git paw start --supervisor` then immediately closes their terminal without attaching.** The session is alive in tmux but the user can't see anything. → **Mitigation:** explicit "Session ... started. Attach with: tmux attach -t paw-<project>" message at end of cmd_supervisor; same as `from-specs-launch-fixes`'s non-TTY behaviour.
- **[Risk] `cmd_supervisor` early return changes user expectations.** Users used to "supervisor blocks until I exit it" now get their terminal back immediately. → **Mitigation:** explicit "Attach with: tmux attach -t paw-<project>" message printed at end of `cmd_supervisor`. Documented in user-guide.
- **[Risk] Auto-merge regression on launch.** v0.4 auto-merged after supervisor exit; v0.5 doesn't unless the supervisor LLM follows the skill's merge procedure. A user attaching, supervising, then detaching might forget to ask the supervisor to merge. → **Mitigation:** the supervisor skill's autonomous loop instructs the agent to begin merge orchestration once it observes all expected `agent.verified` messages — so under happy-path supervision the merge happens without explicit user prompting. Documented; if dogfood shows users want explicit "you're done; merge now" affordance, add a `tmux send-keys` driven prompt or an inbox message in v0.6.0+.
- **[Risk] Auto-approve thread dies if dashboard pane is killed.** The user might `Ctrl-C` the dashboard pane intending only to free that pane; auto-approve silently stops. → **Mitigation:** documented; if dogfood reveals friction, move to broker-internal in a follow-up.
- **[Risk] Layout calculation off-by-one.** Pane indices, pane_offset, agent_id → pane_idx mappings all shift by +1 (was offset 1 for dashboard, now 2 for supervisor + dashboard). Easy to miss a callsite. → **Mitigation:** named constant `SUPERVISOR_PANE_OFFSET = 2` used everywhere; grep audit pre-merge.
- **[Trade-off] tmux `split-window` sequence vs `select-layout`.** Manual splits are more code but produce predictable pane ordering and the exact custom shape we need. `select-layout` is shorter but doesn't support our row-with-different-height pattern.
- **[Trade-off] Hard 25-agent cap.** Some users may have wide terminals and 30+ agents. v0.5.0 says "split into multiple sessions"; v1.0.0 #17 introduces configurability.
- **[Trade-off] Supervisor and dashboard sharing top row 50/50.** If the user resizes their tmux window, tmux's percentage-based layout reflows. Worst case: very narrow terminal → both top panes are too narrow to be useful. Same trade-off as any tmux session; not introducing new behaviour.

## Migration Plan

1. Land `from-specs-launch-fixes` first (already shipped on `feat/v0.5.0-specs`). Provides the prerequisite dispatcher fix + non-TTY handling.
2. Implement this change. Existing v0.4 supervisor invocations now drop into the new flow.
3. Document the merge-loop regression and the new attach-then-interact UX in release notes.
4. Follow up with the `git paw merge` change to restore auto-merge as an explicit on-demand command.
5. Rollback: revert. v0.4 supervisor flow returns. Sessions saved during the v0.5 transition might have the new layout shape; recovery via revert would mismatch. Treat the rollback window as "purge sessions, don't recover."

Release-notes call-outs:
- **Breaking-ish:** supervisor mode UX changes from "your terminal is the supervisor" to "the supervisor is a pane in tmux." Run `tmux attach -t paw-<project>` after `git paw start --supervisor` to interact.
- **Regression:** auto-merge after supervisor exits is gone in v0.5.0; coming back as `git paw merge` in a follow-up.
- New non-TTY support: supervisor mode now works from CI, scripts, and harness tools — the launch is always detached.
- Hard cap at 25 agents per supervisor session. >25 → split into multiple sessions or wait for v1.0.0 configurable layout.

## Open Questions

- **Should the dashboard pane be writable (i.e., can the user type in it) or read-only?** Decision: read-only as today (the `__dashboard` subcommand renders a TUI; user input is captured for navigation but not for sending arbitrary text). The supervisor pane is the chat surface.
- **What happens if the user kills the supervisor pane?** The supervisor agent dies. The session continues with dashboard + coding agents. The user can `tmux respawn-pane` to restart, OR there's a v1.0.0 candidate for "supervisor restart" UX. v0.5.0: documented behaviour, no special handling.
- **Should `cmd_supervisor` fail loudly when no `[supervisor]` config is present?** Yes — same as v0.4. The supervisor CLI must be specified somewhere (`[supervisor].cli`, `default_cli`, or `--cli`). Existing error path applies.
- **Auto-approve thread location revisited:** broker-internal might still be cleaner long-term. v0.5.0 ships dashboard-internal; v0.6.0 (alongside MCP) is a natural moment to revisit since MCP needs broker-side hooks anyway.

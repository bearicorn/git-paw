## Context

The v0.5.0 dogfood pass surfaced D9: with `--from-specs --supervisor` finally engaging supervisor mode end-to-end (after `from-specs-launch-fixes` shipped), all 10 agents stayed `idle` even though the boot block was being injected. Pane capture confirmed the prompt was sitting in the input area as a "Pasted text #N" placeholder, with Claude Code's UI explicitly saying:

> After any paste operation, send an additional Enter key to ensure the full content is processed.

The current `cmd_supervisor` send-keys loop sends `tmux send-keys -t <target> <prompt> Enter` — one Enter, which the CLI consumes confirming the paste buffer instead of submitting. The prompt never runs.

This is a **single-pass code fix plus a skill update**:
- The code fix engages on every supervisor launch, ensuring the boot block always submits.
- The skill update gives the supervisor agent (introduced as a tmux pane in `supervisor-as-pane`) a written recovery path for when paste-buffer-stuck panes appear mid-session — for example, when one agent pastes a long context block to another via `tmux send-keys` and gets stuck the same way.

The change is intentionally narrow: no new config knobs, no per-CLI submit-keystroke tables, no broker-level pane-content polling. Those belong to v1.0.0 alongside per-CLI hook providers (see MILESTONE drift item 17).

## Goals / Non-Goals

**Goals:**
- Boot prompts injected by `cmd_supervisor` actually submit on Claude Code v2.1.x without requiring the user to press Enter manually.
- The fix is CLI-agnostic — works (or no-ops harmlessly) on Codex, Gemini, Aider, opencode, and any other terminal LLM the user might configure as the coding CLI.
- The supervisor agent has a documented recovery procedure for paste-buffer-stuck peer panes (Detect via `tmux capture-pane`; recover via `tmux send-keys ... Enter`).
- No regression for sessions that previously submitted on a single Enter.

**Non-Goals:**
- Per-CLI submit-keystroke configuration (e.g. `[supervisor.cli_input] claude_submit_keys = ["Enter", "Enter"]`). Defer to v1.0.0.
- Per-CLI paste-buffer pattern dictionaries hard-coded in Rust. Skill-level CLI-agnostic recovery is enough for v0.5.0.
- Broker-side / watcher-side pane-content polling for paste-buffer detection across all panes. Adds polling cost and only matters when supervisor mode is off; out of scope.
- Configurable inter-Enter delay. 300ms is a fixed v0.5.0 default (matches existing auto-approve precedent); revisit only if dogfood shows it's too short.
- A second double-Enter at boot for `cmd_start` and `cmd_start_from_specs` non-supervisor flows. Those flows use `-l` literal mode without a trailing Enter — the boot block is buffered for the user to read and submit themselves. No fix needed.

## Decisions

### D1. Launch sends exactly one Enter; recovery is the supervisor skill's job

**Choice:** The launch flow (`cmd_supervisor`'s send-keys loop) invokes `tmux send-keys -t <target> <prompt> Enter` exactly once per pane. No retries, no delay, no second Enter. Recovery from the paste-buffer state on paste-aware CLIs is handled by the supervisor agent via a proactive launch-time pane-state sweep + the paste-buffer-recovery sub-case under stall detection (see the `agent-skills` capability delta).

**Why (revised again, after the second dogfood iteration):**
- The first revision (2 Enters, 300ms delay) was insufficient on Claude Code v2.1.138 — 7 of 11 panes stayed paste-buffer-stuck.
- The second revision (3 Enters, 600ms total) fixed the immediate symptom but introduced a real risk: on faster CLIs or shorter prompts where Enter #1 submits directly, Enters #2 and #3 could accidentally accept a follow-on permission prompt that the agent's first action triggered (e.g. the broker-registration `curl` prompt seen in dogfood). The 600ms window is shorter than the prompt-to-first-permission latency we observed in this run, but the risk exists in principle and grows on faster hardware / shorter prompts.
- The pivot: the launch flow is fundamentally a blind keystroke injector. Disambiguating "still in paste buffer" from "submitted and now at a permission prompt" requires inspecting pane content via `tmux capture-pane` — which is exactly what the supervisor agent already does in its monitoring loop. Moving recovery into the skill means **every Enter is informed by state**, eliminating the blind-retry race entirely.
- Dogfood evidence that this works: during the v0.5.0 11-agent session, the supervisor (acting as a human-driven manual agent in this case) inspected each stuck pane via `tmux capture-pane`, classified the state (paste-buffer placeholder vs permission prompt vs working), and acted accordingly. All 11 agents unstuck and progressed to publishing `agent.status` and `agent.question` within seconds.
- The skill grows two complementary responsibilities at launch time:
  1. **Paste-buffer recovery** — applied proactively (not just on `last_seen` stall) when the pane shows a paste-buffer indicator.
  2. **Permission-prompt classification + approval** — for the boot-time broker `curl`, `git -C <worktree>`, and similar safe-by-confinement operations the first agent action triggers. Selects "Yes, don't ask again" so the pattern is permanently allowed.

**Alternatives considered (and rejected with dogfood evidence):**
- *Keep the 3-Enter pattern with capture-pane gating between Enters* — Plausible but moves capture-pane logic into the launch path. Mixes responsibilities: the launch flow becomes a state-aware loop instead of a blind injector, and the supervisor skill still needs its own loop for mid-session recovery. Cleaner to put all state-aware recovery in one place (the skill).
- *Detect the CLI and only send extra Enters for known paste-aware CLIs* — Adds a CLI dictionary in code; brittle as new CLIs ship; per-CLI logic belongs in v1.0.0 hook providers alongside the per-CLI permission allowlist seeding work (see MILESTONE v1.0.0 "Per-CLI Broker-Curl Allowlist Seeding").
- *Use a `tmux paste-buffer set` + `paste-buffer` sequence instead of `send-keys`* — Tmux's paste-buffer commands paste the contents but don't submit; the submit problem on paste-aware CLIs would persist. Not a fix.

**Cost:** Launch wall-clock returns to v0.4 baseline (no inter-Enter sleeps). Per-pane keystroke count is back to 1. The "cost" moves to the supervisor side: an immediate post-attach sweep that inspects all panes and applies recovery. Sweep cost is N × `tmux capture-pane` invocations (~10-50ms each) + per-pane classification — negligible compared to tmux session construction itself.

### D2. Supervisor skill — proactive launch-time pane sweep

**Choice:** The supervisor skill grows an explicit launch-time pane-inspection sweep. Immediately after attaching, the supervisor agent iterates every coding-agent pane via `tmux capture-pane`, classifies the pane state into one of four categories (paste-buffer, permission prompt, working, idle), and acts:

- **Paste-buffer** → single `tmux send-keys Enter` (the paste-buffer recovery action).
- **Permission prompt** → classify the pending command per the safe-by-pattern / confined-to-worktree / unknown trichotomy. Safe → select "Yes, don't ask again" (typically `Down` + `Enter`). Confined → "Yes, allow all edits" (`Down` + `Enter`). Unknown → escalate via `agent.question`.
- **Working** → leave alone.
- **Idle** → investigate (agent may have crashed or never started).

**Why:**
- The existing skill content already covers stall detection on `last_seen > 5 min` and a manual approve-or-guidance bullet for permission prompts. The launch sweep makes both checks happen *immediately* on attach instead of waiting for stall thresholds to elapse.
- Mirrors the existing Rust `[supervisor.auto_approve]` background thread but operates at the LLM-judgement layer: the auto-approve thread is fast but limited to a hard-coded safe-command regex; the supervisor skill can apply judgment about novel commands and confined-to-worktree operations the regex can't catch.
- Dogfood evidence: during the v0.5.0 11-agent session, all 11 agents hit the same broker-curl permission prompt simultaneously. The auto-approve thread would have eventually approved them (after 30s+ stall), but the human-driven supervisor approved them immediately by selecting "Yes, don't ask again" — letting the agents register with the broker within seconds instead of half a minute.

**Alternatives considered:**
- *Rely entirely on the existing auto-approve thread* — Works but is reactive (30s+ latency) and limited to the hard-coded safe-command regex. The skill-driven sweep is proactive (0s latency once supervisor attaches) and can apply judgment.
- *Make the auto-approve thread poll every second instead of every 30s* — Adds CPU + tmux load for every supervisor session regardless of need. The skill-driven sweep is a one-shot at launch + ongoing every-iteration check, which is more efficient.

### D3. Skill change is additive under existing stall-detection section, plus a new launch-sweep section

**Choice:** The paste-buffer recovery sub-case lives under the existing stall-detection section (additive). The launch-time pane-inspection sweep is a new top-level section in the skill (it's a different lifecycle moment than stall detection).

**Why:**
- Stall detection runs continuously on the agent monitoring loop and acts when `last_seen` ages out. Paste-buffer recovery is one *cause* of stall when an agent has never published anything; listing it as a sub-case keeps the skill organised.
- The launch-time sweep is conceptually different: it runs once, immediately on attach, before any stall thresholds matter. It deserves its own heading so the supervisor agent knows to do it FIRST.

### D4. CLI indicator patterns are illustrative, not exhaustive

**Choice:** The skill lists known indicators (Claude Code: `Pasted text #N`) with explicit "TBD" notes for CLIs not yet observed. Detection relies on the supervisor agent's judgment, not a closed regex list.

**Why:**
- An exhaustive list would go stale as CLIs ship UI updates.
- Markdown bullet form is easy to extend during dogfood ("D11: also seen on Codex as `[Multiline input]`") without code changes.
- The fallback heuristic (long buffered text in input area without follow-up response) catches unseen indicators by structure even when literal text differs.

## Risks / Trade-offs

- **[Second Enter creates blank message on non-paste-aware CLIs]** → Mitigation: surveyed common CLIs (Aider, opencode, Codex CLI, Gemini CLI) all treat empty-Enter as a no-op or a single blank prompt; none treat it as destructive (e.g. accept-current-suggestion or commit-current-state). If a future CLI does treat it destructively, that user can disable the second Enter only when the per-CLI v1.0.0 hook providers land.

- **[300ms is too short on slow systems]** → Mitigation: dogfood the next round on the same hardware that hit D9. If the second Enter still races (i.e. arrives before paste-buffer placeholder rendered), bump to 500ms before v0.5.0 ships. The constant is a single named source of truth (`SUBMIT_DELAY_MS`).

- **[300ms cumulatively slows large supervisor launches]** → 25-agent cap × 300ms = 7.5s added wall-clock at the absolute worst case. Tmux session construction itself runs in the same ballpark; this isn't a noticeable regression.

- **[Supervisor mis-identifies a non-paste-buffer pane state and Enters into legitimate input]** → The fallback action (Enter) is benign on every surveyed CLI: it either submits a complete prompt, submits an empty prompt (recoverable), or is ignored. Worst case is a minor visual scroll. The skill instructs the supervisor to be lenient about indicator detection precisely because the cost of a false positive is so low.

- **[Code path change interacts with `supervisor-as-pane`]** → `supervisor-as-pane` restructures `cmd_supervisor` to attach to the supervisor pane and adds the supervisor itself to the loop. The double-Enter pattern composes — the supervisor pane gets the same treatment as agent panes. Both changes can land independently as long as both apply the double-Enter pattern in the final unified loop. Document this composition in `tasks.md` of whichever ships second so the second author knows to preserve the pattern.

- **[`cmd_start` and `cmd_start_from_specs` non-supervisor paths not updated]** → Intentional. Those use `-l` literal mode without `Enter`; the boot block is buffered for the user, not auto-submitted. No paste-buffer trap to hit.

## Migration Plan

This is a code fix + skill content update. No data, no config, no schema changes.

1. **Code change** in `src/main.rs::cmd_supervisor` send-keys loop — extend the existing `tmux send-keys` block with a 300ms sleep + second Enter. Add `const SUBMIT_DELAY_MS: u64 = 300;` next to existing supervisor constants.
2. **Skill change** in `assets/agent-skills/supervisor.md` — add the paste-buffer recovery sub-case under the existing stall-detection section, with example indicator patterns and the corrective `tmux send-keys ... Enter` action.
3. **Tests**:
   - Argv-recording behavioural test: assert the loop emits two send-keys invocations per pane (existing pattern from `from-specs-launch-fixes` for `tmux::build_boot_inject_args`).
   - Skill-content tests: assert `supervisor.md` contains the paste-buffer-recovery sub-case heading and references `tmux capture-pane` + `tmux send-keys ... Enter`.
   - Optional integration test (real tmux + Claude install): launch a supervisor session with a fixture spec, capture the agent pane after `BOOT_DELAY_MS + SUBMIT_DELAY_MS + ~5s`, assert the paste-buffer placeholder is gone. Gate behind a feature flag because Claude isn't always installed in CI.
4. **Rollback** — revert the code change. Boot prompts will buffer-but-not-submit again on Claude Code v2.1.x, but no other behaviour regresses. Skill content can be reverted independently.

No flag, no opt-in. The fix is universally beneficial (or universally no-op) per the CLI-agnosticism argument in D1.

## Open Questions

- *Do Codex CLI / Gemini CLI / opencode actually need the second Enter today?* Answer in the next dogfood round, after the user adds `CLAUDE_CONFIG_DIR=~/.claude-oss claude` as a custom CLI. If yes, this design needed no change. If their behaviour differs (e.g. second Enter creates an unwanted blank message), feed back into D1 risk and consider a per-CLI exemption.

- *Is 300ms enough?* Confirmed against Claude Code v2.1.x on the dogfood hardware. Re-verify in the next round; if not, bump and document the new value before v0.5.0 ships.

- *Should the paste-buffer recovery action use `M-Enter` or a CLI-specific submit hotkey instead of plain Enter?* Currently no CLI surveyed needs more than Enter to confirm-paste-and-submit. Revisit if a CLI uses `M-Enter` (Alt+Enter) for "send" by default — the skill would need to know the per-CLI hotkey, which moves the work into v1.0.0 hook-provider territory.

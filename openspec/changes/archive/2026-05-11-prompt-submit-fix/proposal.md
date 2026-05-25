## Why

A v0.5.0 dogfood pass (after `from-specs-launch-fixes` shipped, when `--from-specs --supervisor` finally engaged supervisor mode end-to-end) showed all 10 coding agents stuck `idle` despite boot prompts being correctly injected. Root cause: modern terminal LLMs (verified on Claude Code v2.1.x; likely applies to Codex, Gemini, opencode, and any other paste-aware CLI) recognise long pasted content as a "paste buffer" and need additional `Enter` keystrokes — typically two more — to actually submit. The v0.4 `cmd_supervisor` code sends `tmux send-keys -t <target> <prompt> Enter` exactly once; that single `Enter` is consumed confirming the paste buffer and the prompt never submits.

Pane content observed during dogfood, with Claude's UI explicitly asking for the extra Enter:

```
Pasted text #N
[content]

After any paste operation, send an additional Enter key to ensure
the full content is processed.
```

The first instinct — and the design that this proposal initially landed with — was to make the launch flow send extra `Enter` keystrokes after a short delay. That version shipped, was dogfooded a second time, and the user identified a real risk: on fast CLIs (or short prompts that already submitted on `Enter` #1) any extra blind `Enter` could accidentally accept a follow-on permission prompt that the agent's first action triggered. The 600-millisecond window between `Enter` #1 and `Enter` #3 was shorter than the boot-prompt → first-action latency we observed in that run, but the risk exists in principle and grows on faster hardware or shorter prompts.

This change pivots to a different split of responsibilities. The launch flow stays as a blind keystroke injector — exactly one `Enter` per pane, same as v0.4 — and **all** state-aware recovery (paste-buffer recovery, permission-prompt approval) moves into the supervisor agent's skill. The skill already has a stall-detection loop with `tmux capture-pane` access; teaching it to *also* run a proactive launch-time pane sweep is a small extension that handles paste-buffer recovery and safe-command auto-approval together. Every `Enter` the supervisor sends is informed by the actual pane state, so the blind-retry race disappears.

## What Changes

**Code revert in `cmd_supervisor` send-keys loop** (`src/main.rs`):

Today, after the v0.4 + earlier-attempt code:
```rust
let (first_argv, second_argv, third_argv) =
    tmux::build_supervisor_submit_argv_triple(&tmux_session.name, *pane_idx, prompt);
let _ = std::process::Command::new("tmux").args(&first_argv).status();
std::thread::sleep(std::time::Duration::from_millis(SUBMIT_DELAY_MS));
let _ = std::process::Command::new("tmux").args(&second_argv).status();
std::thread::sleep(std::time::Duration::from_millis(SUBMIT_DELAY_MS));
let _ = std::process::Command::new("tmux").args(&third_argv).status();
```

After:
```rust
let target = format!("{}:0.{pane_idx}", tmux_session.name);
let _ = std::process::Command::new("tmux")
    .args(["send-keys", "-t", &target, prompt, "Enter"])
    .status();
```

Drops the `SUBMIT_DELAY_MS` constant, the `build_supervisor_submit_argv_triple` helper, the 5 unit tests on the triple, and the constant-guard test. The launch flow is back to its v0.4 shape: one `tmux send-keys` per pane.

**Supervisor skill extension** (`assets/agent-skills/supervisor.md`):

The skill grows two complementary responsibilities for the supervisor agent.

1. **Launch-time pane sweep** — a new workflow step (1.5, between the baseline-test step and the monitoring loop). Immediately after attaching, the supervisor SHALL `tmux capture-pane` every coding-agent pane and classify the pane state into one of four categories, each with a default action:

   | Pane state | Indicator examples | Action |
   |---|---|---|
   | **Paste-buffer** | `Pasted text #N`, long buffered text in input area without rendered LLM response | `tmux send-keys -t <pane> Enter` to submit |
   | **Permission prompt** | `This command requires approval`, `Do you want to proceed?`, `❯ 1. Yes` | Classify the pending command and act per the safe-command policy |
   | **Working** | `esc to interrupt`, spinner glyphs | Leave alone |
   | **Idle** | `? for shortcuts`, blank input | Investigate; agent may have crashed or never started |

   For permission prompts the safe-command policy classifies the pending command into:
   - Safe-by-pattern (`curl http://127.0.0.1:<broker_port>/...`, `cargo fmt|clippy|test|build`, `git commit`, `git push`, plus any user-configured `safe_commands`) → `Down` + `Enter` to select "Yes, don't ask again".
   - Confined-to-worktree (file edits, `git -C <worktree>` ops on the agent's own worktree) → `Down` + `Enter` to select "Yes, allow all edits".
   - Unknown / wider scope → escalate via `agent.question`; do NOT auto-approve.

   The sweep complements (does not replace) the existing `[supervisor.auto_approve]` background poll thread. The poll thread is reactive (acts when an agent's `last_seen` ages past `stall_threshold_seconds`); the sweep is proactive (acts within seconds of the supervisor attaching), covering the first-few-seconds window before the poll thread's threshold elapses.

2. **Paste-buffer recovery sub-case under stall detection** — an additional sub-case alongside the existing "idle prompt → likely done" and "thinking/waiting → prompt to self-report" cases. When the supervisor sees a paste-buffer indicator in a peer pane via `tmux capture-pane` (either at launch time per the sweep above, or mid-session when an agent or supervisor itself has pasted a long block), it sends `tmux send-keys -t <pane> Enter` to submit. The Enter recovery is safe-by-default — on a non-paste-aware CLI or a misclassified pane it is a no-op or produces a benign blank prompt. Indicator detection is lenient: known indicators are listed (Claude Code's `Pasted text #N`) but the supervisor SHOULD apply judgment to unfamiliar patterns when a pane shows long buffered text in the input area without a follow-up response.

**Affected sites NOT changed:**

- `cmd_start` and `cmd_start_from_specs` boot-block injection use `tmux::build_boot_inject_args` with `-l` literal mode and no trailing `Enter`. The boot block is buffered for the user to read and submit themselves; no paste-buffer trap to hit.
- The `[supervisor.auto_approve]` background poll thread keeps doing what it already does — runs every `stall_threshold_seconds`, classifies pending commands against the safe-command whitelist, approves safe ones. The new skill content is a faster-acting proactive overlay, not a replacement.

**Not in scope (deferred):**

- Per-CLI configurable submit-keystroke sequences (e.g. `[supervisor.cli_input] claude_submit_keys = ["Enter", "Enter"]`) — belongs in v1.0.0 alongside the per-CLI hook providers.
- Per-CLI paste-indicator pattern dictionaries in code — skill-level CLI-agnostic recovery is enough for v0.5.0.
- Detecting paste buffers proactively in non-supervisor mode — the filesystem watcher could in principle do this, but adding broker-internal pane-content polling is scope expansion. In non-supervisor mode the user is attached to the session and can hit Enter themselves.
- An explicit polling loop in the *coding-agent* coordination skill so agents check their inbox for `agent.feedback` after publishing `agent.question` — captured as MILESTONE drift item 34, scheduled into `forward-coordination` (the supervisor `tmux send-keys` push) for v0.5.0; the agent-side polling loop moves to v0.6.0 alongside MCP.

## Capabilities

### New Capabilities
*(none — both prongs extend existing capabilities)*

### Modified Capabilities
- `supervisor-launch`: the existing "Initial prompt injection via tmux send-keys" requirement keeps its single-`Enter` shape and grows two new scenarios stating (i) the launch flow sends exactly one `Enter` per pane, and (ii) paste-buffer recovery is delegated to the supervisor skill.
- `agent-skills`: the embedded `supervisor.md` skill gains two new requirements — paste-buffer recovery as a sub-case under stall detection (with proactive launch-time application), and a proactive launch-time pane sweep with the four-category classification + safe-command policy.

## Impact

**Code**:
- `src/main.rs::cmd_supervisor` — supervisor send-keys loop reverts to a single `tmux send-keys ... Enter` per pane. ~10 lines deleted (the loop body + the `SUBMIT_DELAY_MS` constant + its doc comment). New doc comment above the loop explains why no retry happens here.
- `src/tmux.rs` — `build_supervisor_submit_argv_triple` helper deleted. Existing `build_boot_inject_args` retained (used by `cmd_start` non-supervisor flows).
- `src/main.rs::tests` — the constant-guard test on `SUBMIT_DELAY_MS` is removed (constant no longer exists).
- `src/tmux.rs::tests` — the 5 unit tests on the argv-triple are removed.
- `assets/agent-skills/supervisor.md` — gains the launch-time pane sweep section (step 1.5) and the paste-buffer-recovery sub-case under stall detection.

**Tests**:
- 5 skill-content tests verify the launch sweep section is present, enumerates the four pane categories, references the safe-command keystroke pattern, mentions the `Pasted text` indicator, and notes the recovery is safe-by-default. `src/skills.rs::tests`.
- Existing supervisor skill tests (Spec Audit, etc.) continue to pass.

**Backward compatibility**: fully additive on the skill side; on the code side it's a revert to v0.4 behavior so existing tests that asserted the v0.4 single-send-keys shape pass unchanged. Sessions launched under this change behave identically to v0.4 at the keystroke level; the user-visible improvement comes from the supervisor agent applying recovery actions when it attaches.

**Mismatches resolved**:
- Dogfood D9 (boot prompts injected but never submitted) — resolved by the supervisor's launch-time pane sweep, which performs the paste-buffer recovery + safe-command approvals within seconds of attach.
- Eliminates the second-Enter risk identified during the first iteration of this change (potentially accepting a follow-on permission prompt on fast CLIs).

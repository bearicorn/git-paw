## Why

`[clis.<name>].prompt_profile` overrides are mostly inert. `claude_prompt_profile()` — the embedded
Claude Code default — was called directly at **16 production sites** (verified 2026-10-18 against
the pre-v0.17.0 tree). v0.17.0 `classifier-retirement` moves the detection half of
`supervisor/auto_approve.rs` into `supervisor/permission_prompt.rs`, deletes `detect_prompt_shape`
with the classifier, and collapses `classify_capture` / `detect_permission_prompt` into
`has_approval_marker(captured, profile)`. The sites that still read the embedded default after it
are: `supervisor/permission_prompt.rs` ×6 (`file_prompt_regex`, `option_line_regex`,
`is_confirmation_boundary`, `is_bash_command_header`, `extract_command_slice`, `is_live_prompt`,
plus any capture-and-scan wrapper kept around `has_approval_marker`), `supervisor/drive.rs` ×3
(`cli_unavailable_line`, `input_box_text`, `pane_is_mid_response`), and one each in
`tmux/readiness.rs` (`classify_pane_readiness`), `supervisor/manual_approvals.rs`
(`is_prompt_boilerplate`), `supervisor/approval_gate.rs` (`live_prompt_in_tail`) and
`coordination/inventory.rs` (`detect_mode`). With the dashboard auto-approve wiring deleted, only
two sites resolve a profile at all (`main.rs::cmd_prompt_profile`, which resolves the *session* CLI,
not the pane's, and `readiness::gate_pane_for_injection`). The bundled `sweep.sh` likewise reads one
profile for every pane, including the `broad_grant_marker` its `approve` option-1 guard checks.

So a non-Claude worker's pane is read with Claude's prompt shapes even when the user wrote a profile
for it: its live prompts can go unescalated, be escalated with the wrong command slice, or pass
`sweep.sh approve`'s option-1 guard with the wrong marker. `approval-command-safety`'s existing
requirement *Prompt markers are sourced from the resolved CLI profile* ("the CLI running in the pane
being inspected") is not met. The same single-CLI assumption is compiled into the launch gate's
first-run dialogs (`tmux/readiness.rs::DIALOG_MARKERS`), which are Claude's although
`tmux-orchestration` already says dialog detection is per-CLI.

## What Changes

- Every prompt-shape decision takes the profile of the CLI on the surface it inspects, resolved once
  per agent from the `CliProfile` record (`cli-provider-record`). The embedded default is read only
  by resolution, as the `generic` fallback.
- A surface's CLI comes from git-paw's own launch record (session state), never from the
  agent-publishable `cli` field of `agent.status`.
- Pattern fields are compiled once per resolved profile; an invalid override pattern fails config
  load naming the field instead of panicking or being skipped at detection time.
- The decisions become pure functions of (profile, captured text) — the seam v0.23.0's per-CLI
  conformance suite replays recorded captures through.
- `sweep.sh` resolves profile fields per pane CLI, including the live-prompt re-confirm and the
  option-1 guard of `sweep.sh approve`.
- First-run dialog recognition (bypass acceptance, folder trust) moves into the provider layer —
  compiled, not overridable; a provider without dialogs of its own inherits today's set, so no
  existing CLI loses its dialog handling.
- With no `prompt_profile` overrides every decision is byte-identical to today.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `cli-prompt-profiles`: new requirements — detectors receive the surface's resolved profile; the
  surface's CLI comes from the launch record; decisions are pure functions of profile and capture;
  override patterns are validated at load; the helper resolves per pane.
- `tmux-orchestration`: first-run dialog recognition comes from the pane CLI's provider.

## Impact

- **Code:** the sites above plus their callers — `supervisor/drive.rs` (`sweep_pane`, the
  escalation command slice / `dedup_shape`, `send_nudge` verification, `submit_buffered_input`),
  `supervisor/permission_prompt.rs`, `config/prompt_profile.rs` (compiled patterns, load-time
  validation), `tmux/readiness.rs`, `assets/scripts/sweep.sh` + `_paw_common.sh` (and the tracked
  `.git-paw/scripts/` copies).
- **Behaviour:** unchanged without overrides. With overrides, they now apply to the pane whose CLI
  they name, everywhere — including where the session CLI's profile was used for all panes.
- **Enum-variant ripple:** none.
- **Security / safety:** a profile still changes only how text is *recognised*. git-paw makes no
  safety decision about any command (v0.17.0 `classifier-retirement`), and the live-prompt gate's
  anchoring windows, the `sweep.sh approve` re-confirm and its option-1-only rule stay compiled
  (`cli-prompt-profiles`, *Security decisions are never profile-sourced*). Over-broad markers fail
  toward escalation: a false live prompt yields an extra escalation, never a keystroke. Taking the
  CLI from the agent-publishable broker field would let an agent choose the profile used to read its
  own pane — hence the launch record.
- **Cross-version dependencies:** requires `cli-provider-record` (the record). Rebases over v0.17.0
  `classifier-retirement` (detectors moved to `permission_prompt.rs`; `__classify`, `ProtectedPaths`,
  the dashboard auto-approve thread and `poll.rs` deleted, so none of them is threaded here) and the
  archived v0.17.0 `cli-unavailable-detection-gaps` (already resolves `cli_unavailable_markers` per
  pane — this change generalises it), `live-prompt-tail-window` (tail helper) and
  `idle-pane-nudge-submission` (input-box reader, `paste_buffer_pattern` use); v0.18.0
  `terminal-backend-seam` (surfaces, not pane indices) and `supervisor-pane-removal` (no pane-0
  role).
- **Dependencies:** none.

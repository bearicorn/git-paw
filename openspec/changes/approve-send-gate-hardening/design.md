## Context

The live-prompt gate keys on `Esc to cancel` appearing within the last ~4 non-blank lines of a pane capture. That window is too tight for Claude Code's multi-option permission prompt (`Do you want to proceed?` + N options + footer), so `sweep.sh approve` — which looks for the proceed-marker — misses it and reports "no keys sent." Separately, sending keys without re-checking liveness lets digits land as chat when the prompt has cleared (F5).

## Goals / Non-Goals

**Goals:** multi-option prompts are detected and auto-clearable; no keystroke is ever sent to a pane without a live prompt at send time.
**Non-Goals:** not the broker-mediated in-band approval redesign (that is the separate `broker-mediated-approvals` line); this hardens the existing pane-scrape path.

## Decisions

- **D1 — Detect the prompt by its stable structural markers, not a fixed tail window.** Recognise a live prompt when the capture's tail contains the numbered Yes/No option glyphs AND/OR the `Do you want to proceed?` marker AND the `Esc to cancel` footer — widening the inspected window enough to span the whole prompt block. Rationale: the option list + footer are always at the tail of a live prompt, so matching them (rather than requiring the proceed-marker inside a 4-line window) reliably catches both single- and multi-option prompts. Keep `auto_approve.rs` and `sweep.sh` using the same markers.
- **D2 — Re-confirm liveness immediately before send.** The sender takes a fresh capture right before dispatching keys; if the live-prompt markers are absent, it sends nothing and reports "cleared before send." Rationale: closes the F5 race window between decision and send. *Alternative:* a broker-mediated signal — deferred to `broker-mediated-approvals`; this is the cheap pane-scrape hardening.

## Risks / Trade-offs

- Widening the window could match a prompt-shaped block in scrollback → mitigated by requiring the footer/options at the *tail* and re-confirming at send time.
- `auto_approve.rs` (Rust) and `sweep.sh` (bash) must stay in lockstep on the marker set → a shared, documented marker list + tests on both sides (mirrors the existing classifier/sweep.sh lockstep discipline).

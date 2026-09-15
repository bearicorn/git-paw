## 1. Characterise the defect with failing tests

- [ ] 1.1 Add a unit fixture to `src/supervisor/auto_approve.rs` tests: a live multi-option prompt capture with a six-line trailing task region appended below `Esc to cancel`. Assert `live_prompt_markers_at_tail` returns true — this SHALL fail before the fix
- [ ] 1.2 Add the mirror-image fixture: the identical prompt with no trailing region, asserting it still returns true (guards against over-correcting)
- [ ] 1.3 Add a scrolled-away fixture: a prompt separated from the end of the capture by more trailing content than the bound allows, asserting it returns false

## 2. Anchor the Rust scan on the modal block

- [ ] 2.1 Replace the fixed `LIVE_PROMPT_TAIL` / `LIVE_PROMPT_BLOCK` window test in `live_prompt_markers_at_tail` with an upward search from the end of the non-blank lines for the structural markers, allowing a bounded trailing non-prompt region
- [ ] 2.2 Choose the trailing bound from real captures (the dogfood task footer is 6 non-blank lines; allow headroom) and document the chosen value and its justification in a doc comment
- [ ] 2.3 Keep the multi-option block width so `Do you want to proceed?` + N options + footer still spans; verify the existing multi-option scenario test still passes
- [ ] 2.4 Confirm `is_live_prompt` and `live_prompt_in_tail` need no signature change and that `approval_send_gate` is unchanged

## 3. Mirror the change in the bundled helper

- [ ] 3.1 Update `is_live_capture` in `assets/scripts/sweep.sh` to the same anchored-search shape
- [ ] 3.2 Sync the tracked `.git-paw/scripts/sweep.sh` copy from `assets/scripts/sweep.sh` — both exist and have drifted before
- [ ] 3.3 Run `bash -n` on both scripts after editing (the heredoc-quoting hazard in these files is real)

## 4. Parity and regression

- [ ] 4.1 Extend `tests/sweep_sh_live_gate_parity.rs` with the trailing-region fixture, asserting the Rust gate and the bash mirror return the same verdict
- [ ] 4.2 Extend it with the scrolled-away fixture, asserting both still refuse
- [ ] 4.3 Verify every scenario in `specs/approval-auto/spec.md` maps to at least one test (Gate 3 traceability)

## 5. End-to-end confirmation

- [ ] 5.1 Reproduce the original A/B against the built binary: `sweep.sh classify` on a prompt fixture with and without a trailing task region SHALL now return the same verdict (`approve option=N (safe)`), where before the footer variant returned `no-op (not live)`
- [ ] 5.2 Confirm an `--unattended` run approves a classifier-safe prompt in a pane whose task list is visible

## 6. Docs and gates

- [ ] 6.1 Update the supervisor / unattended chapter if it documents the tail-window behaviour; check `docs/src/user-guide/` for any statement about how liveness is detected
- [ ] 6.2 `cargo fmt`, then `just lint`
- [ ] 6.3 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code rather than piped output
- [ ] 6.4 `openspec validate live-prompt-tail-window --strict`

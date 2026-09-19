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

## 7. Amendment — boundary with the cli-unavailable scan (folded in 2026-10-18)

- [ ] 7.1 Do NOT modify `src/supervisor/drive.rs`. Its `cli_unavailable_line` also scans a fixed `LIVE_PROMPT_TAIL` window and has the same blind spot, but it is fixed by a separate follow-up change; `idle-pane-nudge-submission` is editing `drive.rs` concurrently in this wave
- [ ] 7.2 Keep `LIVE_PROMPT_TAIL` defined and compiling so `cli_unavailable_line` is unaffected by this change
- [ ] 7.3 Expose the bounded-trailing-region logic as a reusable `pub(crate)` helper (not inlined into `live_prompt_markers_at_tail`), so the follow-up can adopt it for the banner scan rather than re-deriving it

## 8. Amendment — case-insensitive stuck-on-prompt matching (folded in 2026-10-18)

- [ ] 8.1 Make the stuck-on-prompt marker match in `assets/scripts/sweep.sh` case-insensitive (the `re.search(markers, cap)` in the stuck-eval Python block); sync the tracked `.git-paw/scripts/sweep.sh` and run `bash -n` on both
- [ ] 8.2 Make `tests/sweep_sh_stuck_detection.rs` exercise the binary-sourced path: put the freshly built `git-paw` first on the fixture's `PATH`, so `git paw __prompt-profile` resolves. Today CI has no `git-paw` on `PATH`, so these tests only ever exercise the fallback literal
- [ ] 8.3 Add a test that evaluates one capture with binary-sourced markers and with the fallback markers and asserts the same classification
- [ ] 8.4 Replace the vacuous `out.contains("stuck")` assertion in `permission_prompt_with_stale_heartbeat_publishes_stuck` — `not-stuck` satisfies it — with an exact phase assertion
- [ ] 8.5 Confirm the three previously-failing tests (`permission_prompt_with_stale_heartbeat_publishes_stuck`, `permission_marker_is_stuck_on_prompt_not_no_progress`, `repeated_detection_is_deduped`) pass on a machine with `git-paw` installed

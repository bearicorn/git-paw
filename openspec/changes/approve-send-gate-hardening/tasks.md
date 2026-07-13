## 1. Multi-option prompt detection

- [ ] 1.1 Widen the live-prompt detection in `src/supervisor/auto_approve.rs` to match the prompt's structural markers at the tail (numbered option glyphs and/or `Do you want to proceed?` + `Esc to cancel`), spanning a full multi-option block rather than a fixed ~4-line window
- [ ] 1.2 Mirror the same marker set in `assets/scripts/sweep.sh`'s `approve` re-capture so its detection agrees with the Rust gate (lockstep)

## 2. Re-confirm before send

- [ ] 2.1 In the send path (auto-approver and `sweep.sh approve`), take a fresh capture immediately before dispatching keys; if the live-prompt markers are absent, send nothing and report "cleared before send"

## 3. Option-index selection in sweep.sh approve

- [ ] 3.1 Replace the blind `Down`+`Enter` sequence in `sweep.sh approve` with option-list parsing: read the numbered options from the fresh pre-send capture, resolve the option index (2-option → `1`; 3-option → `2` only when the broad-grant rule permits, else `1`), and send the digit + `Enter`
- [ ] 3.2 Reuse the helper's existing classifier mirrors (EXPLICIT_SAFE / READ_MOSTLY) for the broad-grant arbitrary-code check so the helper and the Rust auto-approver resolve the same index

## 4. Tests

- [ ] 4.1 Live-prompt gate detects a multi-option prompt fixture (with a "don't ask again" option); rejects a prose-only capture
- [ ] 4.2 A capture that goes from live → cleared between decision and send results in zero keystrokes sent
- [ ] 4.3 `sweep.sh` marker detection agrees with the Rust gate on the same fixtures (a `bash`-level test or shared-fixture assertion; run `bash -n` on sweep.sh after editing)
- [ ] 4.4 `sweep.sh approve` on a 2-option fixture sends `1` + `Enter` (not `Down`); on a 3-option arbitrary-code fixture sends `1` + `Enter` (not the broad grant)

## 5. Docs

- [ ] 5.1 Update the supervisor / auto-approval docs to note multi-option prompts are auto-clearable, the send path re-confirms liveness, and `sweep.sh approve` selects options by parsed index

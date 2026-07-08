## 1. Multi-option prompt detection

- [ ] 1.1 Widen the live-prompt detection in `src/supervisor/auto_approve.rs` to match the prompt's structural markers at the tail (numbered option glyphs and/or `Do you want to proceed?` + `Esc to cancel`), spanning a full multi-option block rather than a fixed ~4-line window
- [ ] 1.2 Mirror the same marker set in `assets/scripts/sweep.sh`'s `approve` re-capture so its detection agrees with the Rust gate (lockstep)

## 2. Re-confirm before send

- [ ] 2.1 In the send path (auto-approver and `sweep.sh approve`), take a fresh capture immediately before dispatching keys; if the live-prompt markers are absent, send nothing and report "cleared before send"

## 3. Tests

- [ ] 3.1 Live-prompt gate detects a multi-option prompt fixture (with a "don't ask again" option); rejects a prose-only capture
- [ ] 3.2 A capture that goes from live → cleared between decision and send results in zero keystrokes sent
- [ ] 3.3 `sweep.sh` marker detection agrees with the Rust gate on the same fixtures (a `bash`-level test or shared-fixture assertion; run `bash -n` on sweep.sh after editing)

## 4. Docs

- [ ] 4.1 Update the supervisor / auto-approval docs to note multi-option prompts are auto-clearable and the send path re-confirms liveness

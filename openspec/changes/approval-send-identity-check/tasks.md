## 0. Preconditions

- [ ] 0.1 Rebase onto the release branch. Check that the MODIFIED *Approval keystrokes require a re-confirmed live prompt* body equals the current `openspec/specs/approval-auto/spec.md` text (as archived by v0.17 `live-prompt-tail-window`) plus only this change's identity clauses. Re-copy if a later change edited it.
- [ ] 0.2 Find out which send paths exist at the tip. If v0.18 `supervisor-pane-removal` (D3) has landed, `drive::send_approval` goes through `approval_send_gate`; put the check in the gate once and skip the separate drive-loop wiring in 4.2. If v0.18 `drive-loop-functional-core` has landed, add `expected: PromptIdentity` to its `ApproveIfStillLive` effect and a "prompt changed" outcome to its effect log.
- [ ] 0.3 Confirm the identity's extractors at the tip: `extract_command_slice` (all header-form lines after v0.17 `classifier-compound-segments`), `extract_path_from_file_prompt` (header path after v0.17 `broad-grant-scope-safety` GP-56), and the option-line pattern. Grep each one's call sites in `classify_prompt` and `poll::drive_outcomes`, so the identity reads exactly what the classifier reads.

## 1. Functional core: prompt identity and the re-confirm decision

- [ ] 1.1 In `src/supervisor/approval_gate.rs`, add `PromptIdentity` (command, file target, option list) and a pure `PromptIdentity::from_capture(&str)`. It uses the classifier's extractors. Options are the numbered option lines after the prompt's last header, with `strip_decoration` and the caret removed. When there is no command and no file target, the command part is the trimmed capture. Add doc comments.
- [ ] 1.2 Add the pure `reconfirm_prompt(expected: &PromptIdentity, fresh_capture: &str) -> Reconfirm { Unchanged, Changed, Cleared }`. It returns `Cleared` when `live_prompt_in_tail` fails, `Changed` when the identity differs, and `Unchanged` otherwise.
- [ ] 1.3 Add a table test for scenarios "An unchanged prompt is approved", "A different command that replaced the classified one receives no keystrokes", "A cleared prompt receives no keystrokes", "A file-operation prompt whose target changed receives no keystrokes" and "A changed option list receives no keystrokes". Assert on `reconfirm_prompt`, one row per scenario.
- [ ] 1.4 Test scenario "Separate captures of one unchanged prompt match": two captures of one live modal, different only in caret position and in output above the header, give equal identities. Include a real Claude Code capture fixture for both a header-form `Bash command` prompt and a file-edit prompt.
- [ ] 1.5 Test the invariant behind D1 over a fixture set: two captures with equal identity give equal `classify_prompt` verdicts and option indices. Leave out the documented R3 file-op body case.

## 2. The approval-send gate

- [ ] 2.1 `approval_send_gate` takes `expected: &PromptIdentity` and decides with `reconfirm_prompt` on its one fresh capture. Add `GateOutcome::PromptChanged`. Keep the pane-0 refusal first and unchanged.
- [ ] 2.2 Gate tests: keys sent on `Unchanged`, none on `Changed` or `Cleared`, exactly one capture per call. Update the existing tests to pass an expected identity; existing assertions stay unchanged.

## 3. Dashboard poll approver

- [ ] 3.1 Add `expected: &PromptIdentity` to `ApprovalRequest` (`src/supervisor/approve.rs`). `auto_approve_pane` passes it to the gate and maps `PromptChanged` to `Ok(false)`. Update the in-crate constructors and `tests/auto_approve_integration.rs`.
- [ ] 3.2 In `poll::drive_outcomes`, build the identity from `captured` (capture #2, the one classified) and pass it through `dispatch_approval`. An aborted send must not produce `TickOutcome::Approved`.
- [ ] 3.3 Poll tests with a sequenced inspector (capture #2 is the classified prompt, capture #3 the pre-send one): an `ls -la src` → `rm -rf ~/projects` swap sends no keys and is not `Approved`. On the next tick, the `rm -rf ~/projects` capture is forwarded as danger and never approved. Covers scenarios "A different command…", "The replacement prompt is classified afresh on the next sweep" and "Danger and unknown verdicts never reach the send step".
- [ ] 3.4 Test scenario "The claim is released after an aborted send": after a `PromptChanged` abort, `PaneClaim::try_acquire` on the same repo root and pane succeeds.

## 4. Unattended drive loop

- [ ] 4.1 `sweep_pane` builds the identity from the same `capture` it passes to `classify_prompt`, and hands it to `send_approval` with the option index.
- [ ] 4.2 `send_approval` decides with `reconfirm_prompt` on its fresh capture (taken under the claim) and returns `Ok(false)` on `Changed`. Leave its pane-0 behavior as it is (skip if 0.2 moved it into the gate).
- [ ] 4.3 `drive_loop` tests with `SequencedCapturer`: a `cargo test` → `git push --force origin main` swap sends no keys and the other panes are still swept. On the next sweep the push prompt is escalated as danger and not approved. An unchanged prompt is still approved. Covers the `supervisor-unattended-operation` scenarios "Classifier-safe prompt is auto-approved", "A prompt replaced between classification and send is not approved" and "The replacement prompt is escalated on the next sweep".
- [ ] 4.4 If v0.24 `unattended-heartbeat-progress` has landed, assert that an aborted send does not count as a dispatched approval for heartbeat progress.

## 5. Cross-approver parity

- [ ] 5.1 Test scenario "Both approvers reach the same decision for the same captures": for a table of (classified capture, pre-send capture) pairs, covering unchanged, command swapped, target swapped, options swapped and cleared, run the poll approver and the drive loop with fakes. Assert both send or both send nothing.

## 6. Docs

- [ ] 6.1 In `docs/src/user-guide/supervisor.md`, section "Re-confirm before send, and the pane 0 exclusion": the send-time check also requires the live prompt to be the classified one (command, file target, options). Otherwise nothing is sent and the next sweep re-classifies. Say that `sweep.sh approve` is operator-directed and not covered.
- [ ] 6.2 In `docs/src/user-guide/security-posture.md`, *Auto-approve classifier* row: keys reach only the classified prompt. Add the residual risk: a human or the CLI changing the prompt within the final tens of milliseconds before the keys are sent.
- [ ] 6.3 Confirm that no `--help`, README or configuration-reference change is needed. Nothing user-configurable changed.

## 7. Verification and quality gates

- [ ] 7.1 To verify (design R2, Open Question 4): against a live Claude Code pane, check whether the option digit alone confirms the selection. Record the result in design.md. If it does, open a follow-up for the trailing `Enter`.
- [ ] 7.2 Security review per `.agents/skills/security-and-safety-review`: the diff only removes sends, keeps the keys unchanged, and adds no allowlist entry or dependency.
- [ ] 7.3 `just check` passes. Check the real exit code, not piped output. Also run `cargo test --no-fail-fast` with `GIT_PAW_ALLOW_LIVE_SESSION=1` when a live session is present.
- [ ] 7.4 `just deny` passes.
- [ ] 7.5 `mdbook build docs/` succeeds.
- [ ] 7.6 `openspec validate approval-send-identity-check --strict` passes.

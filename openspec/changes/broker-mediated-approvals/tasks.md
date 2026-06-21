## 1. Approval-send gate (in-binary)

- [ ] 1.1 Add a prompt-marker tail check: given a captured pane string, return whether a live permission-prompt marker is present within the last 4 non-blank lines (reuse the prompt-marker set shared with `permission-detection`/`stuck-prompt-detection`)
- [ ] 1.2 Implement the approval-send gate: capture the target pane immediately before send, run the tail check, and only dispatch keystrokes when a live prompt is re-confirmed; send nothing otherwise
- [ ] 1.3 Make the gate refuse pane index 0 (supervisor pane) with no keystrokes and a "pane 0 excluded" report
- [ ] 1.4 Implement identity-keyed dedup (command/agent or wait-for-clear); ensure the dedup key is never derived from prompt boilerplate/footer text

## 2. Wire the auto-approver through the gate

- [ ] 2.1 Route the `automatic-approval` `BTab Down Enter` keystroke path through the gate (re-confirm immediately before send)
- [ ] 2.2 Ensure the auto-approver dispatches nothing when the re-confirm capture shows the prompt cleared
- [ ] 2.3 Ensure the auto-approver never sends into pane 0 via the blind send-keys path

## 3. sweep.sh approve subcommand

- [ ] 3.1 Update `cmd_approve` in `assets/scripts/sweep.sh` to run a fresh `tmux capture-pane` and confirm a live prompt in the last 4 non-blank lines before sending `Down`/`Enter`
- [ ] 3.2 Make `cmd_approve` report "prompt cleared, no keys sent" and send nothing when the tail check fails
- [ ] 3.3 Make `cmd_approve` refuse pane index 0 with a "pane 0 excluded" message and no keystrokes
- [ ] 3.4 Update the `approve <pane>` usage/help line in sweep.sh to document the live-prompt requirement

## 4. Documentation

- [ ] 4.1 Update `assets/agent-skills/supervisor.md` approval guidance: approvals fire only on a re-confirmed live prompt and never target pane 0 via blind send-keys
- [ ] 4.2 Confirm no new broker message variant is referenced anywhere (trigger = `agent.status` phase `stuck-on-prompt`; escalation = `agent.question`); update mdBook/user-guide approval sections if they describe the send path
- [ ] 4.3 Run `mdbook build docs/`

## 5. Tests

- [ ] 5.1 Unit test the tail check: marker in last 4 non-blank lines → live; marker only in scrollback above the tail → not live
- [ ] 5.2 Unit/behavioural test: gate dispatches keys for a re-confirmed live prompt and dispatches nothing for a cleared prompt
- [ ] 5.3 Behavioural test: gate refuses pane 0 (no keystrokes) and approves a coding-agent pane (index 2)
- [ ] 5.4 Behavioural test: dedup treats `cargo test` and `git push` prompts (identical footer) as distinct, and dedups repeated capture of the same unanswered prompt
- [ ] 5.5 Test the `automatic-approval` modified scenarios (cleared-prompt suppression, pane-0 exclusion) via the auto-approve path
- [ ] 5.6 Test `sweep.sh approve` scenarios: sends on live prompt, sends nothing on cleared prompt, refuses `approve 0` (assert against a tmux session)

## 6. Quality gates

- [ ] 6.1 `just check` (fmt + clippy + tests)
- [ ] 6.2 `just deny`
- [ ] 6.3 `openspec validate "broker-mediated-approvals" --strict`

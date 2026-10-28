## 1. Baseline (before any edit)

- [ ] 1.1 Re-grep `claude_prompt_profile()` on the tree after v0.17.0 `classifier-retirement` and the v0.18.0 changes this lands after, and confirm the production sites listed in the proposal (detection now in `supervisor/permission_prompt.rs`); record the list in the PR
- [ ] 1.2 Add a table-driven fixture test that runs every detector over the existing capture fixtures with the embedded default and pins the decisions — the byte-identical guard for this change

## 2. Seam: add the parameter (behaviour-preserving)

- [ ] 2.1 Give each detector a profile parameter (`&CliProfile` or its prompt profile) and pass the embedded default at every existing call site; suite green with no decision changed
- [ ] 2.2 Compile pattern fields into the resolved profile (design D3); remove the `file_prompt_regex` / `option_line_regex` `OnceLock`s and their `.expect`
- [ ] 2.3 Validate override patterns at config load with an error naming `[clis.<name>].prompt_profile.<field>`; tests for an invalid and a valid pattern
- [ ] 2.4 Make the drive loop's escalation path (`dedup_shape`, `extract_command_slice`, the file-prompt path match) take the profile; delete the remaining no-profile wrappers (`classify_pane_readiness`, and any capture-and-scan wrapper around `has_approval_marker`) so the compiler lists every remaining caller

## 3. Resolve per agent from the launch record

- [ ] 3.1 Build the `agent id → CliProfile` map from the session's recorded CLIs at drive-loop start; resolve late-added agents on first sight; unknown → `generic`
- [ ] 3.2 Replace the drive loop's `/status`-derived `worker_cli` for profile and settle-delay lookups with the launch record; keep the broker field for display only
- [ ] 3.3 Tests: an override reaches the live-prompt gate for its pane and the prompt is escalated with no keystroke sent; a mixed `claude` + `mycli` session reads each pane with its own profile; a worker publishing a different `cli` keeps its launch profile; an unrecorded CLI resolves `generic`
- [ ] 3.4 Add a guard test that no module outside profile resolution reads the embedded default (source audit in `tests/source_audit.rs`, alongside its existing guards)

## 4. Pure decisions for replay

- [ ] 4.1 Expose each decision as a pure function of (profile, capture, observed inputs); move any capture or clock access it still does into its caller
- [ ] 4.2 Test: a recorded capture evaluated with no terminal or broker yields the same live-prompt result and escalation command slice as the drive loop's path for that capture

## 5. Helper parity

- [ ] 5.1 `sweep.sh` / `_paw_common.sh`: resolve fields per distinct roster CLI (`__prompt-profile --cli`), including the fields `approve` reads for its live-prompt re-confirm and its option-1 guard (`broad_grant_marker`); no-CLI path unchanged; keep the script apostrophe-free and `bash -n` clean; sync the tracked `.git-paw/scripts/` copies
- [ ] 5.2 Extend the `sweep_sh_*` parity tests with a two-CLI fixture roster, including an `approve` run whose option-1 guard uses the `mycli` pane's overridden `broad_grant_marker`

## 6. Provider-owned dialogs

- [ ] 6.1 Move `DIALOG_MARKERS` into the provider layer as the embedded default set; tests for a Claude-family pane, a `generic` pane (unchanged handling) and an override that cannot declare a dialog

## 7. Spec traceability

- [ ] 7.1 Map every scenario in `specs/cli-prompt-profiles` and `specs/tmux-orchestration` to a named test
- [ ] 7.2 Confirm `approval-command-safety`'s existing *Prompt markers are sourced from the resolved CLI profile* scenarios are now met and point their tests at the per-pane path

## 8. Docs

- [ ] 8.1 `docs/src/user-guide/cli-prompt-profiles.md`: overrides apply per pane; patterns validated at load; how the helper resolves per pane
- [ ] 8.2 `docs/src/supported-clis.md` "Prompt-shape detection": profile per agent, generic fallback
- [ ] 8.3 `docs/src/architecture.md`: detector signatures and the per-agent profile map

## 9. Quality gates

- [ ] 9.1 `cargo fmt`, then `just lint`
- [ ] 9.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 9.3 `just deny`
- [ ] 9.4 `mdbook build docs/`
- [ ] 9.5 `openspec validate cli-profile-threading --strict`

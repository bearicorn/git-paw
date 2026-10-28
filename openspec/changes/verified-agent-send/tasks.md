## 1. Reproduce

- [ ] 1.1 Fixture-driven test (fake terminal backend) showing a raw text+`Enter` send of a 400-character message leaving a paste placeholder unsubmitted — the failure this verb exists to prevent

## 2. Reusable actuator (behaviour-preserving)

- [ ] 2.1 Factor `send_nudge`'s send-and-verify body into a function over the terminal port and a resolved `CliProfile`; the loop's callers (`hand_to_orchestrator`, `send_reengagement`, correction) call it unchanged
- [ ] 2.2 Existing nudge tests pass unchanged, including the v0.17.0 `idle-pane-nudge-submission` cases

## 3. Hidden `__send`

- [ ] 3.1 Add the hidden subcommand (`--agent <id>`, message on stdin; `about` / `long_about` like `__prompt-profile`); test it is absent from `git paw --help`
- [ ] 3.2 Refuse when run from a linked worktree (the main-working-tree check `__authority` uses); resolve session, roster surface and the agent's profile from the launch record; refuse unknown or non-agent targets
- [ ] 3.3 Take the per-pane claim for the whole sequence; refuse when held
- [ ] 3.4 Refuse on a live permission prompt using the live-prompt gate `sweep.sh approve` re-confirms with; refuse messages over the injected-text limit
- [ ] 3.5 Print the result line; exit `0` / `2` / `3`
- [ ] 3.6 Tests (fake backend) for delivered, wedged, each refusal reason (including a linked-worktree cwd), and that no keystroke is sent on any refusal

## 4. `sweep.sh tell`

- [ ] 4.1 Add the `tell` verb: join the message arguments and pipe them to `git paw __send --agent`; pass the exit code through; keep the script apostrophe-free and `bash -n` clean; sync the tracked `.git-paw/scripts/sweep.sh`
- [ ] 4.2 `sweep_sh_*` test asserting the message never appears in `__send`'s argv and the exit codes pass through

## 5. Skill and setup-cli grant table

- [ ] 5.1 Update the bundled supervisor (orchestrator) skill: `/tell` send-keys delivery and paste-buffer recovery call `sweep.sh tell`; describe `wedged` and `refused` (a live prompt is relayed to the human, never answered)
- [ ] 5.2 Content test anchored on `sweep.sh tell` and on the absence of a raw send-keys instruction for messages; `tests/lang_agnostic_skill_audit.rs` passes
- [ ] 5.3 `src/cli_setup/facts.rs`: list `sweep.sh tell` as never granted and add `Bash(.git-paw/scripts/sweep.sh tell *)` to the planned `permissions.ask` entries; extend the grant-table tests (subcommand coverage guard, ask entries, no allow entry names `tell`) and the doctor out-of-date test for a record made without it

## 6. Spec traceability

- [ ] 6.1 Map every scenario in `specs/broker-agent-helper`, `specs/supervisor-skill-discipline` and `specs/cli-setup` to a named test

## 7. Docs

- [ ] 7.1 `docs/src/user-guide/supervisor.md` (or the v0.18.0 orchestrator chapter): sending a message to a worker with `sweep.sh tell`, results and refusals
- [ ] 7.2 `docs/src/user-guide/coordination.md`: `/tell` send-keys mode now verified
- [ ] 7.3 `docs/src/user-guide/cli-setup.md` and `docs/src/user-guide/security-posture.md`: the grant table lists `sweep.sh tell` as never granted with an `ask` rule; the upgrade step (`--undo` then `--allowlist`)

## 8. Quality gates

- [ ] 8.1 `cargo fmt`, then `just lint`
- [ ] 8.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 8.3 `just deny`
- [ ] 8.4 `mdbook build docs/`
- [ ] 8.5 `openspec validate verified-agent-send --strict`

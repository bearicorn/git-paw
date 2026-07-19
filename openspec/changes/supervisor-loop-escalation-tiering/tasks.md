## 1. Boot-context injection (the "loop is running" fact)

- [x] 1.1 Write a failing test: the supervisor boot context assembled for an `--unattended` session contains the drive-loop coordination directive
- [x] 1.2 Write a failing test: the boot context for a non-unattended supervisor session does NOT contain the directive
- [x] 1.3 Implement: `skills::with_drive_loop_directive(supervisor_md, unattended)` (+ `DRIVE_LOOP_DIRECTIVE` const); wired into `cmd_supervisor` at the supervisor `skill_content` assembly, gated on `unattended`
- [x] 1.4 Run the injection tests — green (`drive_loop_directive_present_when_unattended`, `_absent_when_attended`)

## 2. Supervisor skill reframe (escalation-first, no blanket-approve)

- [x] 2.1 Grepped the `*_skill_content.rs` pins before editing
- [x] 2.2 Updated `assets/agent-skills/supervisor.md`: when a loop is running, drain the loop's escalations first (targeted approve / feedback), then sweep for verify/merge/conflicts/status, and do NOT blanket-approve safe prompts; when no loop, sole approver (full sweep + approve)
- [x] 2.3 Added skill-content pin test `supervisor_skill_defers_safe_approvals_to_drive_loop`
- [x] 2.4 Ran the coordination/supervisor skill-content tests — green (33 passed)

## 3. Loop escalation is uniform + drainable (confirmation — loop code unchanged)

- [x] 3.1 Confirmed the loop escalates `danger`/`unknown` uniformly with no supervisor-presence input (`drive_loop` takes no such arg; `loop_escalates_danger_without_blocking_other_agent` guards it)
- [x] 3.2 Confirmed the loop approves only the safe set (`loop_approves_safe_prompt...` + the danger-escalate test); no loop code changed this wave
- [x] 3.3 Confirmed escalations already route to the supervisor inbox: `BrokerAlertSink::escalate` (`drive.rs:961-963`) publishes a `Question` addressed to `SUPERVISOR_AGENT_ID`, which the supervisor's existing inbox-drain consumes. No routing change needed

## 4. Supersede supervisor-auto-approve-hardening

- [x] 4.1 Removed the `supervisor-auto-approve-hardening` change (commit `3ef45bf`); supersession noted in the commit body

## 5. Verification

- [x] 5.1 `openspec validate supervisor-loop-escalation-tiering --strict` passes
- [x] 5.2 `just check` green — skills lib 217, skill-content pins 33, clippy "No issues found", fmt clean
- [x] 5.3 No config surface change (boot-context injection only); attended behaviour unchanged (directive omitted when not unattended)

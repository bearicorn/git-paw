## 1. Boot-context injection (the "loop is running" fact)

- [ ] 1.1 Write a failing test: the supervisor boot context assembled for an `--unattended` session contains the drive-loop coordination directive
- [ ] 1.2 Write a failing test: the boot context for a non-unattended supervisor session does NOT contain the directive
- [ ] 1.3 Implement: inject the directive into the supervisor boot-context assembly, gated on the unattended flag
- [ ] 1.4 Run the injection tests — green

## 2. Supervisor skill reframe (escalation-first, no blanket-approve)

- [ ] 2.1 Grep the `*_skill_content.rs` pins for the current `sweep.sh` approve/sweep prose before editing
- [ ] 2.2 Update `assets/agent-skills/supervisor.md`: when a loop is running, drain the loop's escalations first (targeted approve / feedback), then sweep for verify/merge/conflicts/status, and do NOT blanket-approve safe prompts; when no loop, full sweep + approve as today
- [ ] 2.3 Add/adjust skill-content pin tests asserting the new guidance is present
- [ ] 2.4 Run the coordination/supervisor skill-content tests — green

## 3. Loop escalation is uniform + drainable (mostly confirmation)

- [ ] 3.1 Write a failing/confirming test: the drive loop escalates a `danger`/`unknown` prompt to the broker as a review item, identically regardless of supervisor presence (no liveness branch)
- [ ] 3.2 Confirm the loop approves only the safe set and escalates the rest (regression guard for the disjoint-sets invariant)
- [ ] 3.3 Confirm the escalation lands where the supervisor inbox drains it; adjust routing only if it does not

## 4. Supersede supervisor-auto-approve-hardening

- [ ] 4.1 Remove the `supervisor-auto-approve-hardening` change (its #2/#3/#4 already implemented in v0.10/v0.11; #1 replaced by this change). Note the supersession in the commit body

## 5. Verification

- [ ] 5.1 `openspec validate supervisor-loop-escalation-tiering --strict` passes
- [ ] 5.2 `just check` green (fmt + clippy + tests); every scenario maps to a test
- [ ] 5.3 Confirm no config surface change and attended behaviour is unchanged

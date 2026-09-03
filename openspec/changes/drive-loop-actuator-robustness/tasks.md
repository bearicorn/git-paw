## 1. GP-15 — reliable nudge submission

- [ ] 1.1 On the drive-loop nudge path, apply the per-CLI settle delay (`[clis.<name>].submit_delay_ms`, the `cli-resolution` resolver) between sending the text and the `Enter`
- [ ] 1.2 Verify submission via `capture-pane` after the `Enter`; on a still-unsubmitted input line, recover with `C-u` + re-type + `Enter`, bounded retries; escalate if still wedged
- [ ] 1.3 Guard the recovery with the per-pane approval claim; only `C-u` a line the loop itself nudged (never agent-authored input)
- [ ] 1.4 Tests: nudge waits the settle delay before Enter; a simulated stale-line pane triggers the `C-u`+re-type+`Enter` recovery; a bare second Enter is not treated as sufficient

## 2. GP-16 — stop nudging during wind-down

- [ ] 2.1 Check the loop's exit/wind-down state before dispatching any nudge; suppress nudges once winding down
- [ ] 2.2 Tests: after wind-down begins, a sweep dispatches no nudge keystrokes; idle panes gain no further unsubmitted text

## 3. Supervisor skill correction

- [ ] 3.1 Update `assets/agent-skills/supervisor.md` paste-buffer recovery: `C-u` + re-type + `Enter` (state a lone Enter/C-m is a no-op against a stale line); keep the indicators / lenient / proactive-launch points
- [ ] 3.2 Update the skill-content test to assert the corrected recovery sequence
- [ ] 3.3 Mirror in `docs/src/user-guide/supervisor.md`

## 4. Gates

- [ ] 4.1 `just check` green; no `unwrap()`/`expect()` in non-test code
- [ ] 4.2 `mdbook build docs/` succeeds
- [ ] 4.3 Every spec scenario maps to a test

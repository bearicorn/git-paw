# Tasks

## 1. CLI disclosure notice

- [ ] 1.1 In `src/main.rs` session-start path (where `learnings_enabled` is computed), print a concise notice when learnings mode is enabled: local `.git-paw/session-learnings.md` path, "no telemetry / nothing is sent anywhere", and the optional-share-via-GitHub-issue invitation with the review/anonymise caveat
- [ ] 1.2 Ensure the notice is gated strictly on the enabled flag — no output when learnings is disabled or the `[supervisor]` section is absent
- [ ] 1.3 Keep the GitHub issues URL consistent with the canonical repo link used elsewhere (README)

## 2. Documentation

- [ ] 2.1 Add a privacy & sharing section to `docs/src/user-guide/learnings.md`: no telemetry, local + opt-in, optional sharing via GitHub issue, review-and-anonymise caveat (LLM can assist)
- [ ] 2.2 Add/confirm a README pointer to the privacy & sharing stance
- [ ] 2.3 `mdbook build docs/` succeeds

## 3. Tests

- [ ] 3.1 Behavioral test: session start with `[supervisor] learnings = true` prints the disclosure notice (assert it names the local path, the no-telemetry statement, and the share-via-issue invitation)
- [ ] 3.2 Behavioral test: session start with learnings disabled / absent prints no disclosure notice (output identical to pre-change)

## 4. Quality gates

- [ ] 4.1 `just check` (fmt + clippy + tests) passes
- [ ] 4.2 `just deny` passes
- [ ] 4.3 No `unwrap()`/`expect()` in non-test code; public items documented
- [ ] 4.4 Confirm every spec scenario in `specs/learnings-mode/spec.md` maps to a test

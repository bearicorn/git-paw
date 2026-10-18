## 1. Pure check

- [ ] 1.1 `SpecWorkflowProbe` (resolved format, instructed command, placement, the resolved worker CLIs as a list, per-CLI per-location `{present, tracked}`) and `check_spec_workflow(&SpecWorkflowProbe) -> CheckResult` in `src/doctor.rs`, group "Spec system"; the instructed command derived from the format by an exhaustive `SpecBackendKind` match in one small function co-located with the check (the seam `launch-prompt-entry-point` swaps for its entry-point selector); zero CLIs → Ok (not applicable); each CLI judged on its own, Warn naming only the unreachable CLIs, unknown-location CLIs listed as unverified
- [ ] 1.2 Per-CLI location table with a generic unknown fallback; only the verified row (`claude`: `.claude/commands/opsx/apply.md`, `.claude/skills/openspec-apply-change/SKILL.md`); each further row added only after verifying the spec tool's generated layout for that CLI
- [ ] 1.3 Table tests for every verdict branch (the nine scenarios: sibling + untracked warns, child ok, tracked ok, missing warns, no-command format ok, zero CLIs not applicable, several CLIs warn naming only the unreachable ones, unknown CLI ok-unverified, JSON presence)

## 2. Probe and wiring

- [ ] 2.1 Probe in `collect_probes`: resolved spec type, placement from `PawConfig::worktree_placement()` (the value already in `ConfigProbe.placement`), the spec-driven worker CLIs from `SpecCliProbe.resolved_clis` (reuse `probe_spec_cli`; do not re-resolve), file presence at the root per CLI's known locations, `git ls-files --error-unmatch -- <path>` through `CommandRunner` for tracked state
- [ ] 2.2 Temp-repository probe test: ignored vs tracked definition × child vs sibling placement; `--json` includes the check
- [ ] 2.3 Add the new `SpecBackendKind` match site to AGENTS.md §7's ripple list

## 3. Docs and gates

- [ ] 3.1 `docs/src/user-guide/doctor.md` (the check, verdicts, remedies), `docs/src/user-guide/worktree-placement.md` (placement affects workflow reachability); `mdbook build docs/` succeeds
- [ ] 3.2 Every scenario maps to a named test; `just check` + `just deny` green; full suite green vs merge-base

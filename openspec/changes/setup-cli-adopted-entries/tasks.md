## 1. Setup check judges the file

- [ ] 1.1 `src/doctor.rs::probe_cli_setup`: also read each fact row's allowlist target (main checkout) into `SetupProbe.files`, so the check never depends on which entries are recorded
- [ ] 1.2 `src/cli_setup/doctor.rs::check_against_current_plan`: a planned entry is applied when the target file holds it at its key path (reuse `entry_present`-style lookup), whoever wrote it; also warn when a recorded allowlist value is one the running binary no longer plans; remedy text unchanged
- [ ] 1.3 Unit tests: `an_entry_the_user_already_had_counts_as_applied`, `a_recorded_entry_the_binary_no_longer_plans_is_reported`; the existing out-of-date test still warns when the file lacks the entry
- [ ] 1.4 E2E `tests/doctor_e2e.rs::undo_and_reapply_converge_when_entries_pre_existed`: pre-seed `Bash(.git-paw/scripts/broker.sh *)`, apply with `--yes`, doctor ✓; undo + re-apply, doctor ✓ and the pre-seeded entry still on disk

## 2. Grant-residue check

- [ ] 2.1 `src/cli_setup/facts.rs`: the `claude` row records its project-shared settings file (`.claude/settings.json`) beside `allowlist_target`
- [ ] 2.2 Pure `check_grant_residue(&ResidueProbe) -> Vec<CheckResult>` in `src/cli_setup/doctor.rs`: blanket curl, never-granted helper subcommand or wildcard subcommand (derived from `SWEEP_NEVER_GRANTED` and the `docs-fetch.sh` row, any path form), top-level `allowed_bash_prefixes`; ⚠ with per-kind reason and by-hand remedy, ✓ when clean, ⚠ "not inspected" for invalid JSON; entries via `abbreviate`
- [ ] 2.3 `probe_grant_residue` in `src/doctor.rs` (both files, main checkout, read-only) and wiring beside `check_broad_grants`
- [ ] 2.4 Unit tests, one per scenario (self-approval, blanket curl, inert seed in `settings.json`, own entries ✓, loopback curl not listed, never ✗); e2e: both files byte-identical after `doctor`

## 3. Docs and gates

- [ ] 3.1 `docs/src/user-guide/doctor.md` (setup check counts pre-existing entries; the residue check and its remedy), `docs/src/user-guide/cli-setup.md` (upgrade note), `docs/src/user-guide/security-posture.md` (self-approval residue); `mdbook build docs/`
- [ ] 3.2 `cargo fmt`, `just lint`, `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast` (real exit code), `just deny`; security review per `security-and-safety-review` (read-only, no echo of secrets)
- [ ] 3.3 `openspec validate setup-cli-adopted-entries --strict`

## 0. Preconditions

- [ ] 0.1 Confirm `config-check-command`'s doctor "not evaluated" work (its task 3b) is in or landing alongside; init's load-error rule (D6) mirrors it

## 1. Tracked generated files (#27)

- [ ] 1.1 `tracked_generated_files(runner, repo_root)`: `git ls-files -z` over `required_gitignore_entries()` in the main checkout, through `CommandRunner`; unit tests with a sandbox repo (tracked script found; `config.toml` never listed; untracked ignored file not listed)
- [ ] 1.2 `src/doctor.rs`: `HygieneProbe.tracked_generated`; `check_hygiene` ⚠ row with the `git rm -r --cached <entries>` remedy; unit tests
- [ ] 1.3 `src/init.rs`: after reconciling, print the same command when tracked files exist; never touch the index; not counted as a change

## 2. Upgrade summary (#24)

- [ ] 2.1 `check_hygiene` receives the count of init-repairable findings (missing `.gitignore` entries, missing/stale helpers and compatibility copies, stale orchestrator pointer) and adds one ⚠ with the remedy `git paw init`
- [ ] 2.2 `tests/doctor_e2e.rs`: a 0.19-shaped repository (five `.gitignore` entries missing, one stale compatibility copy) → summary row naming `git paw init`; after `git paw init` → no summary row, no `.gitignore` or helper finding
- [ ] 2.3 `tests/cli_init_*.rs`: second `git paw init` prints "Already initialized. Nothing to do."

## 3. Init hardening

- [ ] 3.1 `install_packages`: write `.git-paw/scripts/.gitignore` (`*`) with the skills one; extend `runtime_state_is_not_staged_by_git_add` with a `.gitignore` that lacks `.git-paw/scripts/`
- [ ] 3.2 `run_init`: on an existing config that fails to load, create directories and reconcile `.gitignore`, skip migration and package render, print the error with its path, exit non-zero; test that `config.toml` and `.git-paw/skills/` are byte-identical afterwards

## 4. Docs

- [ ] 4.1 `docs/src/user-guide/doctor.md` (tracked-files and upgrade rows); "Upgrading git-paw" section (doctor → init → untrack); CHANGELOG pointer

## 5. Gates

- [ ] 5.1 `cargo fmt`; `just check`; `just deny`; `mdbook build docs/`; `openspec validate upgrade-hygiene --strict`

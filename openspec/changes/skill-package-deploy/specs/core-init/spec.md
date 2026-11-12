## MODIFIED Requirements

### Requirement: Init appends logs directory to .gitignore

The system SHALL ensure the repo's `.gitignore` lists the git-paw runtime/scratch entries: `.git-paw/logs/`, `.git-paw/tmp/`, `.git-paw/worktrees/`, `.git-paw/skills/`, `.git-paw/scripts/`, `.git-paw/sessions/`, `.git-paw/session-summary.md`, `.git-paw/session-learnings.md`, `.git-paw/AGENTS.local.md`, `.git-paw/.sweep-progress`, `.git-paw/.sweep-stuck-dedup` and `.git-paw/.add-remove.lock`, so nothing git-paw writes under `.git-paw/` except `config.toml` is committable. Each entry SHALL appear at most once and SHALL be added only if absent.

#### Scenario: Gitignore includes the repo-local tmp scratch after init

- **GIVEN** `git paw init` is run in a repo without `.git-paw/tmp/` in
  `.gitignore`
- **WHEN** init completes
- **THEN** `.gitignore` SHALL contain `.git-paw/tmp/`

Test: `init::tests::repo_local_tmp_added_to_gitignore_and_not_duplicated`

#### Scenario: Gitignore includes the worktrees directory after init

- **GIVEN** `git paw init` is run in a repo without `.git-paw/worktrees/` in
  `.gitignore`
- **WHEN** init completes
- **THEN** `.gitignore` SHALL contain `.git-paw/worktrees/`

Test: `init::tests::worktrees_dir_added_to_gitignore_and_not_duplicated`

#### Scenario: Gitignore includes session-summary.md after init

- **GIVEN** `git paw init` is run in a repo without `.git-paw/session-summary.md` in `.gitignore`
- **WHEN** init completes
- **THEN** `.gitignore` SHALL contain `.git-paw/session-summary.md`

Test: `init::tests::session_summary_added_alongside_logs`

#### Scenario: Gitignore not duplicated on repeated init

- **GIVEN** `.gitignore` already contains `.git-paw/tmp/`,
  `.git-paw/worktrees/`, and `.git-paw/session-summary.md`
- **WHEN** `git paw init` is run again
- **THEN** each managed entry SHALL appear exactly once in `.gitignore`

Test: `init::tests::idempotent_gitignore`

#### Scenario: Runtime state is not staged by git add .git-paw/

- **GIVEN** a repository where `git paw init` ran and a session wrote `.git-paw/sessions/<name>.json`, `.git-paw/scripts/sweep.sh`, `.git-paw/AGENTS.local.md` and `.git-paw/.sweep-progress`
- **WHEN** `git add .git-paw/` is run
- **THEN** the index SHALL hold `.git-paw/config.toml` and no other path under `.git-paw/`

Test: `tests/cli_init_installs_packages.rs::runtime_state_is_not_staged_by_git_add`

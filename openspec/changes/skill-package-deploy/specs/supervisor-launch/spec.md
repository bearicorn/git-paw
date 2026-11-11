## MODIFIED Requirements

### Requirement: A supervised launch refreshes the bundled root helper scripts

Wherever a supervised launch re-exports the orchestrator skill — a fresh launch, or a drive loop bound to an existing session — it SHALL also install the bundled packages at the repository root under `.git-paw/skills/<package>/`, together with their helpers' compatibility copies under `.git-paw/scripts/`, the same set `git paw init` installs. A file whose content already matches SHALL NOT be rewritten.

#### Scenario: A stale sweep.sh is replaced on a supervised launch

- **GIVEN** an initialized repository whose `.git-paw/skills/orchestrator/scripts/sweep.sh` and `.git-paw/scripts/sweep.sh` hold an older script
- **WHEN** the user runs `git paw start --supervisor --branches a`
- **THEN** both files SHALL hold the running binary's bundled script
- **AND** both SHALL be executable

Test: `tests/e2e_supervisor_launch.rs::a_supervised_launch_refreshes_a_stale_root_helper_script`, `src/init.rs::refresh_root_scripts_replaces_a_stale_script`

#### Scenario: An up-to-date script is not rewritten

- **GIVEN** an initialized repository whose `.git-paw/skills/` and `.git-paw/scripts/` already hold the bundled packages and compatibility copies
- **WHEN** the root scripts are refreshed
- **THEN** each script SHALL report no change and keep its modification time

Test: `src/init.rs::refresh_root_scripts_leaves_current_scripts_untouched`

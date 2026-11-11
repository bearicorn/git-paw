## ADDED Requirements

### Requirement: Bundled packages are installed under `.git-paw/skills/`

The system SHALL install a bundled package as the directory `.git-paw/skills/<name>/` holding the package's `SKILL.md` rendered for that location, its `scripts/` with executable mode `0o755` on Unix, and its `references/` rendered like `SKILL.md`. A package's `assets/` SHALL NOT be installed; they are render-time inputs. `.git-paw/skills/` SHALL be among the `.gitignore` entries `git paw init` guarantees.

#### Scenario: The worktree's SKILL.md is rendered for that agent

- **WHEN** an agent worktree for branch `feat/x` is provisioned with the coordination package
- **THEN** its `.git-paw/skills/coordination/SKILL.md` SHALL contain `feat-x` where the skill's branch placeholder renders
- **AND** SHALL contain no unsubstituted known placeholder

#### Scenario: Package directory is gitignored

- **WHEN** `git paw init` runs
- **THEN** `.gitignore` SHALL contain an entry ignoring `.git-paw/skills/`

### Requirement: Init installs every package at the repository root

`git paw init` SHALL install every registered package at the repository root, writing each package file that is missing or differs from the running binary's copy and reporting each such file as created or updated, as *Init reports updates only when content changed, and its closing line agrees* requires.

#### Scenario: Init installs every package at the root

- **WHEN** `git paw init` runs in a fresh repository
- **THEN** `.git-paw/skills/<name>/SKILL.md` SHALL exist for every registered package
- **AND** each package's helpers SHALL exist under `.git-paw/skills/<name>/scripts/` and be executable on Unix
- **AND** no package `assets/` directory SHALL be written

#### Scenario: Re-install refreshes a stale package

- **GIVEN** a `.git-paw/skills/coordination/scripts/broker.sh` with stale content
- **WHEN** `git paw init` runs again
- **THEN** the file SHALL equal the embedded helper
- **AND** stdout SHALL contain `Updated .git-paw/skills/coordination/scripts/broker.sh`

### Requirement: Every start installs the bundled packages at the repository root

Every `git paw start` — bare, spec-driven or supervised — SHALL, before any agent pane is launched, install every registered package at the repository root with its helpers' compatibility copies, writing only files that are missing or differ from the running binary's copy. A fresh clone SHALL therefore get its root helpers without `git paw init`. This SHALL NOT write the config, `.gitignore` or `AGENTS.md`, which stay `git paw init`'s.

#### Scenario: A bare start after a fresh clone installs the root packages

- **GIVEN** a clone of an initialized repository whose `.git-paw/config.toml` is committed and which has no `.git-paw/skills/` and no `.git-paw/scripts/`
- **WHEN** `git paw start --branches a` is run without `git paw init`
- **THEN** `.git-paw/skills/orchestrator/scripts/sweep.sh` and `.git-paw/scripts/sweep.sh` SHALL exist and be executable before the agent's pane is launched
- **AND** `.git-paw/skills/<name>/SKILL.md` SHALL exist for every registered package

#### Scenario: A start leaves current root packages untouched

- **GIVEN** a repository whose root packages and compatibility copies already match the running binary
- **WHEN** `git paw start --branches a` is run
- **THEN** every root package file and compatibility copy SHALL keep its modification time

#### Scenario: A start does not take over init's files

- **GIVEN** a clone with no `.git-paw/skills/` and a committed `.gitignore` and `AGENTS.md`
- **WHEN** `git paw start --branches a` is run
- **THEN** `.gitignore`, `AGENTS.md` and `.git-paw/config.toml` SHALL be byte-identical afterwards

### Requirement: Every launch path installs the worktree's selected packages

Every launch path that creates or re-attaches an agent worktree — bare, spec-driven and supervisor-mode `git paw start`, and `git paw add` — SHALL install exactly the packages selected for that worktree into `<worktree>/.git-paw/skills/` before the agent's boot prompt is submitted.

#### Scenario: A worktree receives exactly its selected packages

- **GIVEN** a session with the broker enabled and `docs_base_url` configured
- **WHEN** any launch path sets up an agent worktree
- **THEN** `<worktree>/.git-paw/skills/coordination/` and `<worktree>/.git-paw/skills/docs-fetch/` SHALL exist before the agent's prompt is submitted
- **AND** `<worktree>/.git-paw/skills/orchestrator/` SHALL NOT exist

### Requirement: Installing packages never touches setup grants

Installing or provisioning packages SHALL NOT create, modify or delete any agent CLI settings file or the `git paw setup-cli` record. Grants recorded for the pre-package `.git-paw/scripts/` paths SHALL stay as they are until the user re-runs `git paw setup-cli`.

#### Scenario: Installing packages leaves setup grants untouched

- **GIVEN** a repository where an earlier version applied `git paw setup-cli claude --allowlist`
- **WHEN** `git paw init` installs the packages and `git paw start` provisions a worktree
- **THEN** the main checkout's `.claude/settings.local.json` and the setup record SHALL be byte-identical before and after

### Requirement: Installed package files are replaced atomically

The system SHALL write each installed package file (`SKILL.md`, references, scripts) and each compatibility copy under `.git-paw/scripts/` by writing the content to a temporary file in the target's directory, setting its mode, and renaming it over the target, so a reader such as a running `sweep.sh` sees the old file or the new one, never a partial one. A symlink at the target SHALL still be refused.

#### Scenario: A reader of the old script is not cut off by a refresh

- **GIVEN** an installed `.git-paw/skills/orchestrator/scripts/sweep.sh` with stale content, opened for reading by another process
- **WHEN** the root packages are refreshed
- **THEN** the open handle SHALL still read the complete stale content
- **AND** the path SHALL then hold the complete bundled script, already executable

#### Scenario: No temporary file is left behind

- **WHEN** `git paw init` installs the packages and compatibility copies
- **THEN** each `.git-paw/skills/<name>/` and `.git-paw/scripts/` directory SHALL hold only the installed files

#### Scenario: A symlink at an install target is refused

- **GIVEN** `.git-paw/scripts/broker.sh` is a symlink
- **WHEN** a worktree is provisioned or `git paw init` runs
- **THEN** the write SHALL fail naming that path
- **AND** the symlink's target SHALL be unchanged

### Requirement: Bundled prose names helpers by their package path

The system SHALL treat a helper's package path, `.git-paw/skills/<package>/scripts/<helper>`, as its invocation path. Bundled agent-facing prose — every registered package's `SKILL.md`, references and assets, the orchestrator session context and the escalation framing — SHALL name a helper only through `{{BROKER_HELPER}}`, `{{SWEEP_HELPER}}` or `{{DOCS_FETCH_HELPER}}`, which every renderer SHALL substitute from the registry and the unknown-placeholder check SHALL know.

#### Scenario: The rendered boot block invokes the package helper

- **WHEN** the helper form of the boot block is rendered for branch `feat/x`
- **THEN** the REGISTER section SHALL contain `.git-paw/skills/coordination/scripts/broker.sh --agent feat-x status booting`

#### Scenario: No bundled template carries a literal helper path

- **WHEN** every bundled prose file named above is scanned
- **THEN** none SHALL contain `.git-paw/scripts/` or a literal `.git-paw/skills/<package>/scripts/` path

#### Scenario: Helper placeholders raise no warning

- **WHEN** a template using `{{BROKER_HELPER}}`, `{{SWEEP_HELPER}}` and `{{DOCS_FETCH_HELPER}}` is rendered
- **THEN** no unsubstituted-placeholder warning SHALL be written to standard error

#### Scenario: The exported orchestrator skill names the package helper

- **WHEN** `git paw init` writes `.git-paw/skills/orchestrator/SKILL.md`
- **THEN** it SHALL contain `.git-paw/skills/orchestrator/scripts/sweep.sh snapshot`
- **AND** it SHALL NOT contain `{{SWEEP_HELPER}}`

### Requirement: Helpers the system invokes itself use the package path

Code that invokes a bundled helper on the system's own behalf, such as the drive loop's learnings sink, SHALL invoke that helper's package path at the repository root, not its compatibility copy.

#### Scenario: The drive loop records learnings through the package helper

- **GIVEN** an unattended session whose drive loop records a learning
- **WHEN** the record is written
- **THEN** it SHALL be written by invoking `.git-paw/skills/orchestrator/scripts/sweep.sh learn` at the repository root

### Requirement: The legacy helper location is a deprecated compatibility copy

Wherever the system installs a package's helpers, it SHALL also write each of those helpers, and `_paw_common.sh`, to `.git-paw/scripts/<helper>` with content byte-identical to the package copy and the same executable mode, so invocations of the pre-package path keep working. No bundled template SHALL name the compatibility path. The compatibility copy SHALL be documented as deprecated, with its removal deferred to the v1.0.0 deprecation policy.

#### Scenario: The compatibility copy matches the package copy

- **WHEN** `git paw init` runs
- **THEN** `.git-paw/scripts/sweep.sh` SHALL exist and be byte-identical to `.git-paw/skills/orchestrator/scripts/sweep.sh`

#### Scenario: An invocation of the old path still works

- **GIVEN** a provisioned agent worktree
- **WHEN** `.git-paw/scripts/broker.sh --help` is run there
- **THEN** it SHALL exit 0 and print the helper's usage

### Requirement: Requirements naming the legacy deployed helper path are satisfied by the package path

The system SHALL satisfy a requirement that bundled prose, a boot block, a sidecar section or the system itself name or invoke a helper as `.git-paw/scripts/<helper>` by naming or invoking its package path; a requirement that a helper be installed there SHALL be met by the compatibility copy. This SHALL NOT apply to the `cli-setup` requirements or to doctor's setup, broad-grant and grant-residue checks, where each path means what it says.

#### Scenario: A skill-content requirement on the old path is checked against the package path

- **GIVEN** a requirement that the orchestrator skill name `.git-paw/scripts/sweep.sh` as its detection mechanism
- **WHEN** its covering test runs against the rendered skill
- **THEN** the test SHALL find `.git-paw/skills/orchestrator/scripts/sweep.sh` in that position
- **AND** SHALL NOT require the literal `.git-paw/scripts/sweep.sh`

#### Scenario: The setup-cli grant table is not remapped

- **GIVEN** the `cli-setup` requirement that puts `Bash(.git-paw/scripts/sweep.sh approve *)` in `permissions.ask`
- **WHEN** the allowlist item is planned
- **THEN** that compatibility-path ask rule SHALL be planned as written, alongside the package-path one

#### Scenario: Doctor's grant checks are not remapped

- **GIVEN** a main-checkout `.claude/settings.local.json` whose `permissions.allow` holds `Bash(.git-paw/scripts/sweep.sh approve *)`
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL report that entry

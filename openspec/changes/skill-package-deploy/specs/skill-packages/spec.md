## ADDED Requirements

### Requirement: Bundled packages are installed under `.git-paw/skills/`

The system SHALL install each bundled package as a directory `.git-paw/skills/<name>/` containing the package's `SKILL.md` rendered for that location, its `scripts/` written with executable mode `0o755` on Unix, and its `references/` rendered like `SKILL.md`. A package's `assets/` SHALL NOT be installed (they are render-time inputs). `git paw init` SHALL install every registered package at the repository root, overwriting existing package files and reporting each package as created or updated. Every launch path that creates or re-attaches an agent worktree — bare `git paw start`, spec-driven `git paw start`, supervisor-mode `git paw start`, and `git paw add` — SHALL install exactly the packages selected for that worktree into `<worktree>/.git-paw/skills/` before the agent's boot prompt is submitted. `.git-paw/skills/` SHALL be among the `.gitignore` entries `git paw init` guarantees. Installing or provisioning packages SHALL NOT create, modify or delete any agent CLI settings file or the `git paw setup-cli` record: grants recorded for the pre-package `.git-paw/scripts/` paths stay as they are until the user re-runs `git paw setup-cli`.

#### Scenario: Init installs every package at the root

- **WHEN** `git paw init` runs in a fresh repository
- **THEN** `.git-paw/skills/<name>/SKILL.md` SHALL exist for every registered package
- **AND** each package's helpers SHALL exist under `.git-paw/skills/<name>/scripts/` and be executable on Unix
- **AND** no package `assets/` directory SHALL be written

#### Scenario: A worktree receives exactly its selected packages

- **GIVEN** a session with the broker enabled and `docs_base_url` configured
- **WHEN** any launch path sets up an agent worktree
- **THEN** `<worktree>/.git-paw/skills/coordination/` and `<worktree>/.git-paw/skills/docs-fetch/` SHALL exist before the agent's prompt is submitted
- **AND** `<worktree>/.git-paw/skills/orchestrator/` SHALL NOT exist

#### Scenario: The worktree's SKILL.md is rendered for that agent

- **WHEN** an agent worktree for branch `feat/x` is provisioned with the coordination package
- **THEN** its `.git-paw/skills/coordination/SKILL.md` SHALL contain `feat-x` where the skill's branch placeholder renders
- **AND** SHALL contain no unsubstituted known placeholder

#### Scenario: Re-install refreshes a stale package

- **GIVEN** a `.git-paw/skills/coordination/scripts/broker.sh` with stale content
- **WHEN** `git paw init` runs again
- **THEN** the file SHALL equal the embedded helper
- **AND** init SHALL report the package as updated

#### Scenario: Installing packages leaves setup grants untouched

- **GIVEN** a repository where an earlier version applied `git paw setup-cli claude --allowlist`
- **WHEN** `git paw init` installs the packages and `git paw start` provisions a worktree
- **THEN** the main checkout's `.claude/settings.local.json` and the setup record SHALL be byte-identical before and after

#### Scenario: Package directory is gitignored

- **WHEN** `git paw init` runs
- **THEN** `.gitignore` SHALL contain an entry ignoring `.git-paw/skills/`

### Requirement: Bundled prose names helpers by their package path

The system SHALL treat a helper's package path — `.git-paw/skills/<package>/scripts/<helper>`, relative to the worktree (or repository root) — as that helper's invocation path. Bundled templates (every registered `SKILL.md`, reference and asset, including both boot-block forms) SHALL name helpers only through the placeholders `{{BROKER_HELPER}}`, `{{SWEEP_HELPER}}` and `{{DOCS_FETCH_HELPER}}`, which rendering SHALL substitute with the package path derived from the package registry; no bundled template SHALL contain a literal helper path. These placeholders SHALL be known to the unknown-placeholder check. Code that invokes a helper on the system's own behalf (the drive loop's learnings sink) SHALL invoke the package path.

#### Scenario: The rendered boot block invokes the package helper

- **WHEN** the helper form of the boot block is rendered for branch `feat/x`
- **THEN** the REGISTER section SHALL contain `.git-paw/skills/coordination/scripts/broker.sh --agent feat-x status booting`

#### Scenario: No bundled template carries a literal helper path

- **WHEN** every registered template file is scanned
- **THEN** none SHALL contain `.git-paw/scripts/` or a literal `.git-paw/skills/<package>/scripts/` path

#### Scenario: Helper placeholders raise no warning

- **WHEN** a template using `{{BROKER_HELPER}}`, `{{SWEEP_HELPER}}` and `{{DOCS_FETCH_HELPER}}` is rendered
- **THEN** no unsubstituted-placeholder warning SHALL be written to standard error

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

The system SHALL satisfy any requirement that a bundled skill, boot block, sidecar section, or the system itself *names or invokes* a helper as `.git-paw/scripts/<helper>` by naming or invoking that helper's package path instead. Any requirement that a helper be *installed* at `.git-paw/scripts/<helper>` SHALL remain satisfied by the compatibility copy. This mapping SHALL NOT apply to the `cli-setup` capability's requirements (the `git paw setup-cli` grant table, its protection rules, undo and the doctor setup check): they name the package path and the compatibility path explicitly, and each path there means exactly what it says.

#### Scenario: A skill-content requirement on the old path is checked against the package path

- **GIVEN** a requirement that the orchestrator skill name `.git-paw/scripts/sweep.sh` as its detection mechanism
- **WHEN** its covering test runs against the rendered skill
- **THEN** the test SHALL find `.git-paw/skills/orchestrator/scripts/sweep.sh` in that position
- **AND** SHALL NOT require the literal `.git-paw/scripts/sweep.sh`

#### Scenario: The setup-cli grant table is not remapped

- **GIVEN** the `cli-setup` requirement that puts `Bash(.git-paw/scripts/sweep.sh approve *)` in `permissions.ask`
- **WHEN** the allowlist item is planned
- **THEN** that compatibility-path ask rule SHALL be planned as written, alongside the package-path one

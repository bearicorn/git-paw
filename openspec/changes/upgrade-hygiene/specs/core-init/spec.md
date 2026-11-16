## ADDED Requirements

### Requirement: Init reconciles a repository set up by an older version

On a repository set up by an earlier git-paw release, one `git paw init` run SHALL add every missing `.gitignore` entry, rewrite every missing or stale bundled package file and compatibility copy, and rewrite a stale orchestrator pointer, reporting each change. A second run SHALL change nothing and SHALL print "Already initialized. Nothing to do.".

#### Scenario: Upgrading from an older release

- **GIVEN** a repository whose `.gitignore` lacks five managed entries and whose `.git-paw/scripts/sweep.sh` holds an older release's content
- **WHEN** `git paw init` is run
- **THEN** `.gitignore` SHALL contain every managed entry and `sweep.sh` SHALL match the bundled copy
- **AND** `git paw doctor` SHALL then report no `.gitignore`, stale-helper or outdated-setup finding

#### Scenario: A second run is a no-op

- **GIVEN** the repository after that `git paw init`
- **WHEN** `git paw init` is run again
- **THEN** no file SHALL change and stdout SHALL contain "Already initialized. Nothing to do."

### Requirement: Init reports tracked generated files without untracking them

When files under the paths `git paw init` ignores are tracked in the index, init SHALL print them with the `git rm -r --cached <paths>` command and a note to commit. Init SHALL NOT modify the index, and the report SHALL NOT count as a change for its summary line.

#### Scenario: Committed helper copies are reported

- **GIVEN** a repository where `.git-paw/scripts/broker.sh` is tracked and every other setup item is current
- **WHEN** `git paw init` is run
- **THEN** stdout SHALL name `.git-paw/scripts/` with `git rm -r --cached`
- **AND** `git ls-files .git-paw/scripts/` SHALL still list `broker.sh`

### Requirement: Init self-ignores the compatibility scripts directory

Wherever the bundled helpers' compatibility copies are installed (the repository root and each agent worktree), the system SHALL also write `.git-paw/scripts/.gitignore` containing `*`, as it does for `.git-paw/skills/`.

#### Scenario: Copies stay unstaged under an old .gitignore

- **GIVEN** a branch whose committed `.gitignore` does not list `.git-paw/scripts/`
- **WHEN** an agent worktree is created from that branch, the helpers are provisioned into it, and `git add -A` is run there
- **THEN** no file under `.git-paw/scripts/` SHALL be staged

### Requirement: Init does not render from defaults when the config fails to load

When `.git-paw/config.toml` exists and the configuration fails to load, `git paw init` SHALL create any missing `.git-paw/` directories and reconcile `.gitignore`, SHALL leave `config.toml` unchanged, SHALL NOT write any root package file, and SHALL exit non-zero with the load error and the file path.

#### Scenario: A syntax error stops the render

- **GIVEN** an initialised repository whose `.git-paw/config.toml` has a syntax error
- **WHEN** `git paw init` is run
- **THEN** the process SHALL exit non-zero naming the config path and the error
- **AND** `.git-paw/config.toml` and every file under `.git-paw/skills/` SHALL be byte-identical afterwards

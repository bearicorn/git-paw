## MODIFIED Requirements

### Requirement: `git paw init` SHALL install the bundled sweep helper

`src/init.rs::run_init` SHALL write the embedded `assets/scripts/sweep.sh` (referenced via `include_str!`) to `<repo>/.git-paw/scripts/sweep.sh` and set executable permissions (`0o755` on Unix) when the resulting configuration enables the Coordinated layer (`default_mode = "coordinated"`, per `core-layer-isolation`). The write SHALL be additive: if the file already exists, `git paw init` overwrites it with the bundled version. When the configuration does not enable the Coordinated layer, `git paw init` SHALL NOT write the file and SHALL remove a copy a previous run wrote, reporting the removal. A coordinated `git paw start` SHALL write the file before any agent boots when it is absent, whatever the configuration says.

The script SHALL be generalized — no hardcoded session name, repo parent path, broker port, or test command. The script SHALL:

- Read the session name from `<repo>/.git-paw/sessions/*.json` (the most recently modified entry when multiple exist).
- Read the broker URL from `<repo>/.git-paw/config.toml` `[broker].port` (default 9119), constructing `http://127.0.0.1:<port>`.
- Read the test command from `<repo>/.git-paw/config.toml` `[orchestrator.gates].test_command`, falling back to `[supervisor].test_command` for a version-1 config. When unset, commands that depend on the test command SHALL no-op gracefully with a message.
- Detect the project root via `git rev-parse --show-toplevel` from inside the script's cwd.

#### Scenario: `git paw init` writes the sweep helper

- **GIVEN** a fresh git repository whose `.git-paw/config.toml` sets `default_mode = "coordinated"`
- **WHEN** `git paw init` is invoked
- **THEN** the file `<repo>/.git-paw/scripts/sweep.sh` SHALL exist
- **AND** the file SHALL be executable (mode `0o755` on Unix)
- **AND** the file content SHALL be byte-identical to the embedded `assets/scripts/sweep.sh`

#### Scenario: `git paw init` overwrites an existing sweep.sh

- **GIVEN** coordinated mode as the default and a repo where `.git-paw/scripts/sweep.sh` already exists with modified content
- **WHEN** `git paw init` is invoked again
- **THEN** the file SHALL be overwritten with the bundled embedded content
- **AND** the file SHALL remain executable

#### Scenario: `git paw init` without coordinated mode writes no sweep helper

- **GIVEN** a configuration with `default_mode = "bare"`
- **WHEN** `git paw init` is invoked
- **THEN** `<repo>/.git-paw/scripts/sweep.sh` SHALL NOT exist, and a copy written by an earlier init SHALL have been removed and reported

#### Scenario: A coordinated start provisions a missing sweep helper

- **GIVEN** a repository with no `.git-paw/scripts/sweep.sh`
- **WHEN** `git paw start --coordinated --branches feat/a` launches
- **THEN** `.git-paw/scripts/sweep.sh` SHALL exist, byte-identical to the embedded script, before the agent's first prompt is sent

#### Scenario: sweep.sh reads session name from session JSON

- **GIVEN** `<repo>/.git-paw/sessions/paw-myproject.json` exists with `session_name: "paw-myproject"`
- **WHEN** `.git-paw/scripts/sweep.sh status` is invoked from the repo root
- **THEN** the script SHALL query `tmux` against session `paw-myproject` (not the hardcoded `paw-git-paw`)

#### Scenario: sweep.sh reads broker port from config

- **GIVEN** `<repo>/.git-paw/config.toml` contains `[broker]\nport = 9200`
- **WHEN** `.git-paw/scripts/sweep.sh status` is invoked
- **THEN** the script SHALL curl `http://127.0.0.1:9200/status` (not the hardcoded 9119)

#### Scenario: sweep.sh status filters phantom agents

- **GIVEN** the broker `/status` returns agents `[orchestrator, feat-x, a, <agent-id>]`
- **WHEN** `.git-paw/scripts/sweep.sh status` is invoked
- **THEN** the rendered output SHALL include rows only for `orchestrator` and `feat-x`
- **AND** the output SHALL include a trailing line naming the filtered phantoms (e.g. `phantoms (use --all to show): a, <agent-id>`)
- **AND** invoking `.git-paw/scripts/sweep.sh status --all` SHALL include every agent in the rendered output and SHALL NOT print the phantoms summary line

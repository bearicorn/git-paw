## ADDED Requirements

### Requirement: `git paw init` SHALL install the bundled sweep helper

`src/init.rs::run_init` SHALL write the embedded `assets/scripts/sweep.sh` (referenced via `include_str!`) to `<repo>/.git-paw/scripts/sweep.sh` and set executable permissions (`0o755` on Unix). The write SHALL be additive: if the file already exists, `git paw init` overwrites it with the bundled version.

The script SHALL be generalized — no hardcoded session name, repo parent path, broker port, or test command. The script SHALL:

- Read the session name from `<repo>/.git-paw/sessions/*.json` (the most recently modified entry when multiple exist).
- Read the broker URL from `<repo>/.git-paw/config.toml` `[broker].port` (default 9119), constructing `http://127.0.0.1:<port>`.
- Read the test command from `<repo>/.git-paw/config.toml` `[supervisor].test_command`. When unset, commands that depend on the test command SHALL no-op gracefully with a message.
- Detect the project root via `git rev-parse --show-toplevel` from inside the script's cwd.

#### Scenario: `git paw init` writes the sweep helper

- **GIVEN** a fresh git repository with no `.git-paw/` directory
- **WHEN** `git paw init` is invoked
- **THEN** the file `<repo>/.git-paw/scripts/sweep.sh` SHALL exist
- **AND** the file SHALL be executable (mode `0o755` on Unix)
- **AND** the file content SHALL be byte-identical to the embedded `assets/scripts/sweep.sh`

#### Scenario: `git paw init` overwrites an existing sweep.sh

- **GIVEN** a repo where `.git-paw/scripts/sweep.sh` already exists with modified content
- **WHEN** `git paw init` is invoked again
- **THEN** the file SHALL be overwritten with the bundled embedded content
- **AND** the file SHALL remain executable

#### Scenario: sweep.sh reads session name from session JSON

- **GIVEN** `<repo>/.git-paw/sessions/paw-myproject.json` exists with `session_name: "paw-myproject"`
- **WHEN** `.git-paw/scripts/sweep.sh status` is invoked from the repo root
- **THEN** the script SHALL query `tmux` against session `paw-myproject` (not the hardcoded `paw-git-paw`)

#### Scenario: sweep.sh reads broker port from config

- **GIVEN** `<repo>/.git-paw/config.toml` contains `[broker]\nport = 9200`
- **WHEN** `.git-paw/scripts/sweep.sh status` is invoked
- **THEN** the script SHALL curl `http://127.0.0.1:9200/status` (not the hardcoded 9119)

#### Scenario: sweep.sh status filters phantom agents

- **GIVEN** the broker `/status` returns agents `[supervisor, feat-x, a, <agent-id>]`
- **WHEN** `.git-paw/scripts/sweep.sh status` is invoked
- **THEN** the rendered output SHALL include rows only for `supervisor` and `feat-x`
- **AND** the output SHALL include a trailing line naming the filtered phantoms (e.g. `phantoms (use --all to show): a, <agent-id>`)
- **AND** invoking `.git-paw/scripts/sweep.sh status --all` SHALL include every agent in the rendered output and SHALL NOT print the phantoms summary line

### Requirement: Supervisor skill SHALL reference the bundled sweep helper

The embedded `assets/agent-skills/supervisor.md` skill SHALL invoke `.git-paw/scripts/sweep.sh <subcommand>` for the operations covered by the helper instead of raw tmux + curl pipelines. The supervisor pane's cwd is the repo root by construction, so the relative path resolves directly. The subcommands taught by the skill SHALL include at minimum: `snapshot`, `capture <pane>`, `approve <pane>`, `status`, `worktrees-status`, `inbox`, `feedback-gate <agent> <gate> <msg>`, `verified <agent> <msg>`, `status-publish <msg>`.

The skill MAY retain a single curl example for the supervisor's initial `agent.status` self-registration, because the script's session-name discovery depends on the session JSON existing — which it does not on the very first publish in a fresh session. All subsequent broker interactions in the skill SHALL go through the helper.

The skill SHALL NOT contain `for p in ... ; do tmux capture-pane ...` style loops over pane indices. Those loops trip Claude CLI per-pattern approval prompts on every sweep iteration and defeat the helper's purpose.

#### Scenario: Rendered supervisor skill references sweep.sh

- **WHEN** the embedded supervisor skill is inspected
- **THEN** every example invoking `tmux capture-pane` across multiple panes SHALL use `.git-paw/scripts/sweep.sh snapshot` or `.git-paw/scripts/sweep.sh capture <pane>` instead
- **AND** every example publishing `agent.verified`, `agent.feedback`, or `agent.status` SHALL use the corresponding `sweep.sh <subcommand>` form
- **AND** no `for p in <list>; do tmux capture-pane ...` loop SHALL appear in the rendered skill content

#### Scenario: Rendered supervisor skill does not contain phantom-prone curl placeholders

- **WHEN** the embedded supervisor skill is inspected
- **THEN** the skill SHALL NOT contain the literal string `<agent-id>` or `<your question>` or `<your specific question>` inside any documented curl payload `agent_id` or payload-text field
- **AND** any remaining placeholder syntax in examples SHALL use clearly-broken forms like `__FILL_IN__` so accidental submission produces an obvious error rather than phantom agents

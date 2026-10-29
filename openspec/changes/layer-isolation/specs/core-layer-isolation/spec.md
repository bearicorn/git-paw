## ADDED Requirements

### Requirement: A session's layers are resolved once, with their dependencies

git-paw SHALL compose every session from the layers below on top of the Manual baseline, and SHALL decide which layers a session has with one pure resolution — configuration, `start` flags and the answer to the interactive coordinated-mode prompt in, the layer set out — that every launch path (bare, spec-driven, coordinated, unattended), `git paw add`, session recovery and `git paw init` use. No launch path SHALL decide a layer by reading configuration directly.

| Layer | Enabled by | Requires |
|-------|------------|----------|
| Manual | always | — |
| Skills | `docs_base_url` configured | — |
| Broker | `[broker] enabled = true`, or implied by Coordinated | — |
| Coordinated | `--coordinated`; or `default_mode = "coordinated"` (or a "yes" to the coordinated-mode prompt) without `--bare`; or implied by Unattended | Broker |
| Unattended | `--unattended` | Coordinated |

The dashboard is an optional view of the Broker layer, off by default, and not a separate layer. `[dashboard] enabled = true` or `git paw start --dashboard` turns it on for a session that has the Broker layer (per `broker-runtime`'s "Start flow conditionally creates dashboard pane"). `git paw mcp` is a client that installs nothing and reads whatever layers a session has; it is not a layer. `[worktree.*]` runtime provisioning and `[logging]` are independent opt-ins of the Manual baseline.

#### Scenario: Resolution table

- **WHEN** the layers are resolved for each row's configuration and flags
- **THEN** the result SHALL be:
  - no configuration, no flags → Manual
  - `[broker] enabled = true` → Manual, Broker
  - `docs_base_url` set → Manual, Skills
  - `--coordinated` with no `[broker]` section → Manual, Broker, Coordinated
  - `--unattended` → Manual, Broker, Coordinated, Unattended
  - `default_mode = "coordinated"`, `[broker] enabled = true`, `--bare` → Manual, Broker

#### Scenario: The resolution is pure

- **GIVEN** the same configuration, flags and prompt answer
- **WHEN** the layers are resolved twice
- **THEN** both results SHALL be equal, with no file, terminal or network access during resolution

### Requirement: A coordinated session enables the broker

When a session resolves the Coordinated layer and `[broker] enabled` is false or absent, git-paw SHALL enable the Broker layer for that session — starting the broker, exporting its URL and installing the Broker layer's surface — and SHALL print one notice line stating that the coordinated session enabled the broker. Wherever another capability conditions behaviour on `[broker] enabled = true` (the broker condition of the dashboard pane, the broker host, `GIT_PAW_BROKER_URL`, the git hooks, `broker.sh` provisioning, the coordination skill, boot-block injection, the per-agent watch targets), that condition SHALL be evaluated as "the session's resolved layers include Broker".

#### Scenario: The init-default coordinated config gets a working broker

- **GIVEN** a `.git-paw/config.toml` with `default_mode = "coordinated"` and no `[broker]` section
- **WHEN** `git paw start --branches feat/a` launches the session
- **THEN** the broker SHALL answer `GET /status`
- **AND** the worktree SHALL have a `paw-agent-id` marker, `.git-paw/scripts/broker.sh`, and the coordination skill in its managed block
- **AND** the launch output SHALL carry the implied-broker notice exactly once

#### Scenario: No notice when the broker is configured

- **GIVEN** `default_mode = "coordinated"` and `[broker] enabled = true`
- **WHEN** a coordinated session starts
- **THEN** no implied-broker notice SHALL be printed

#### Scenario: A non-coordinated session without the broker stays manual

- **GIVEN** no `[broker]` section, no `default_mode`, and a non-interactive terminal
- **WHEN** `git paw start --branches feat/a` launches the session
- **THEN** no broker SHALL run and no Broker-layer artifact SHALL be installed

### Requirement: Each layer installs exactly its declared surface

git-paw SHALL derive what a session installs from its layers with one pure surface plan, and every launch path SHALL install exactly the plan — nothing from a layer the session does not have. The plan for a layer set SHALL be the union of its layers' surfaces:

- **Manual:** one worktree and one agent surface per agent; the worktree's sidecar `.git-paw/AGENTS.local.md` with the assignment section (branch and CLI, plus spec content and file ownership on a spec-driven launch); the task prompt on a spec-driven launch.
- **Skills:** the docs-fetch skill in the managed block; `docs-fetch.sh` and `_paw_common.sh` in the repository's and each worktree's `.git-paw/scripts/`.
- **Broker:** the broker host; the dashboard pane, only when the dashboard is enabled for the launch; `GIT_PAW_BROKER_URL` in the session environment; the `post-commit`, `pre-commit` and `pre-push` hooks in the common git directory and each worktree's `paw-agent-id` marker; `broker.sh` and `_paw_common.sh` in the repository's and each worktree's `.git-paw/scripts/`; the coordination skill in the managed block; the boot block, in its helper or MCP form, in each agent's first prompt.
- **Coordinated:** the inter-agent rules in each managed block; the orchestrator context — the rendered orchestrator skill, the repository-root sidecar session context and the root `AGENTS.md` pointer section; `sweep.sh` and `_paw_common.sh` in the repository's `.git-paw/scripts/`; the broker-side conflict detector and learnings aggregator.
- **Unattended:** the foreground drive loop and the drive-loop directive in the orchestrator context.

#### Scenario: The manual plan installs nothing beyond the worktree and its assignment

- **WHEN** the surface plan is computed for the Manual layer alone
- **THEN** it SHALL contain the worktrees, agent surfaces and sidecar assignment sections
- **AND** it SHALL contain no git hook, no marker, no helper script, no skill, no boot block, no broker host or dashboard, and no `GIT_PAW_BROKER_URL`

#### Scenario: The broker plan has no coordinated surface

- **WHEN** the surface plan is computed for Manual and Broker
- **THEN** it SHALL contain the Broker layer's artifacts
- **AND** it SHALL contain no inter-agent rules, no orchestrator context and no repository-level `sweep.sh`

#### Scenario: The plan is the union of its layers

- **WHEN** the surface plan is computed for every combination of layers the resolution can produce
- **THEN** each plan SHALL equal the union of the per-layer surfaces listed in this requirement

#### Scenario: The dashboard pane follows its own switch

- **WHEN** the surface plan is computed for Manual and Broker, once with the dashboard not enabled and once with it enabled
- **THEN** the first plan SHALL contain the broker host and no dashboard pane
- **AND** the second plan SHALL contain the broker host and the dashboard pane

### Requirement: Instruction content names only enabled layers

The managed block in each worktree's sidecar, the skills rendered into it, and each agent's first prompt SHALL reference only surfaces of the session's layers. Without the Broker layer they SHALL contain no broker URL, no `/publish`, `/messages` or `/status` endpoint, no `broker.sh` invocation, no `curl` to the broker, no broker message type such as `agent.status` or `agent.artifact`, no mention of a post-commit hook, and no boot block. Without the Coordinated layer they SHALL contain no `## Inter-Agent Rules` section and no orchestrator procedure.

#### Scenario: A manual session's instructions carry no broker text

- **GIVEN** a session with only the Manual layer, launched from specs
- **WHEN** its worktree sidecars and first prompts are inspected
- **THEN** none SHALL contain `broker.sh`, `/publish`, `/messages`, `agent.status`, `agent.artifact`, `post-commit`, `curl` or `GIT_PAW_BROKER_URL`
- **AND** each first prompt SHALL still carry the task prompt

#### Scenario: A skills-only session carries the docs skill without broker text

- **GIVEN** a session with the Manual and Skills layers
- **WHEN** its worktree sidecars are inspected
- **THEN** each managed block SHALL contain the docs-fetch skill
- **AND** none SHALL contain a broker token listed in this requirement

#### Scenario: A broker session carries coordination but no coordinated surface

- **GIVEN** a session with the Manual and Broker layers
- **WHEN** its worktree sidecars are inspected
- **THEN** each managed block SHALL contain the coordination skill
- **AND** none SHALL contain `## Inter-Agent Rules`

### Requirement: Repository-level files follow the enabled layers

`git paw init` SHALL install the git-paw-managed repository-level files of the layers its resulting configuration enables, SHALL remove the git-paw-managed repository-level files of layers it does not enable, and SHALL report each file it creates, updates or removes. `git paw start` SHALL install — and SHALL NOT remove — the repository-level files of its session's layers before any agent boots, so a layer enabled only by a flag works in a repository whose configuration leaves it off. The repository-level files are the helper scripts under `.git-paw/scripts/`, the orchestrator skill under `.git-paw/skills/`, and the orchestrator pointer section in the root `AGENTS.md`; removing the pointer SHALL delete only its marker-delimited section. Files outside this managed set SHALL NOT be touched.

#### Scenario: A pure-manual init writes no layer files

- **GIVEN** a fresh repository and a non-interactive `git paw init` (which records `default_mode = "bare"` and no broker)
- **WHEN** init completes
- **THEN** `.git-paw/scripts/` SHALL contain no helper script, `.git-paw/skills/` SHALL not exist, and `AGENTS.md` SHALL carry no orchestrator pointer section
- **AND** `.git-paw/config.toml` and the `.gitignore` entries SHALL be written as before

#### Scenario: Turning a layer off and re-running init removes its files

- **GIVEN** a repository initialised with `[broker] enabled = true`, so `.git-paw/scripts/broker.sh` exists
- **WHEN** the config is changed to `[broker] enabled = false` and `git paw init` runs again
- **THEN** `.git-paw/scripts/broker.sh` SHALL be removed
- **AND** init SHALL report the removal

#### Scenario: A flag-enabled layer is provisioned by start

- **GIVEN** a repository whose config leaves coordinated mode off and whose `.git-paw/scripts/` has no `sweep.sh`
- **WHEN** `git paw start --coordinated --branches feat/a` launches
- **THEN** `.git-paw/scripts/sweep.sh` and the orchestrator skill SHALL exist before the agent's first prompt is sent

#### Scenario: Removing the pointer keeps the rest of AGENTS.md

- **GIVEN** a root `AGENTS.md` with user content above and below the orchestrator pointer section
- **WHEN** init runs with coordinated mode off
- **THEN** the pointer section SHALL be gone and every other byte of `AGENTS.md` SHALL be unchanged

### Requirement: A helper removed with its layer is reported as not installed, not as modified

When a helper script is absent because its layer is not enabled — `git paw init` removed it, or never installed it — and the `git paw setup-cli` allowlist item is applied, the CLI setup check of `git paw doctor` (`preflight-diagnostics`, *Doctor reports CLI setup state*) SHALL treat that helper as not installed: it SHALL NOT report it as a helper that differs from the bundled copy, and its detail SHALL name the helper's path as not installed. A helper that is present and differs from the bundled copy SHALL still be reported ⚠ as modified. Turning a layer off SHALL NOT remove or rewrite any entry in the agent CLI's settings file: setup-cli grants change only through `git paw setup-cli`.

#### Scenario: A helper removed with its layer is not a modified helper

- **GIVEN** the `claude` allowlist item applied with `git paw setup-cli claude --allowlist`
- **AND** a config without `docs_base_url`, so `git paw init` removed `.git-paw/scripts/docs-fetch.sh`
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL NOT be ⚠ because of `docs-fetch.sh`
- **AND** its detail SHALL name `.git-paw/scripts/docs-fetch.sh` as not installed
- **AND** the project's `.claude/settings.local.json` SHALL be byte-identical to before the init

#### Scenario: A present but modified helper is still flagged

- **GIVEN** the `claude` allowlist item applied and the Broker layer enabled
- **AND** `.git-paw/scripts/broker.sh` in the main checkout differs from the bundled copy
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL be ⚠ naming `.git-paw/scripts/broker.sh`

### Requirement: Add and recovery reuse the session's layers

`git paw add` and the recovery of a stopped session SHALL install the layers the session was launched with — Coordinated from the saved session mode and Broker from the saved broker port — rather than layers re-resolved from the current configuration, so every agent of a session has the same surface. The Skills layer SHALL follow the current configuration, since the saved session does not record it.

#### Scenario: add matches the running session after a config edit

- **GIVEN** a session launched with `[broker] enabled = true`
- **AND** the config edited afterwards to `[broker] enabled = false`
- **WHEN** `git paw add feat/b` attaches an agent
- **THEN** the new worktree SHALL have a `paw-agent-id` marker, `broker.sh` and the coordination skill, like the session's other agents

#### Scenario: Recovery reinstalls the launch-time layers

- **GIVEN** a stopped coordinated session
- **WHEN** `git paw start` recovers it
- **THEN** the recovered session SHALL have the Broker and Coordinated surfaces it was launched with

### Requirement: A CI matrix proves each layer combination installs exactly its plan

CI SHALL run a layer-isolation smoke matrix with one entry per combination — Manual; Manual and Skills; Manual and Broker; Manual, Skills and Broker; Coordinated; Unattended — in which each entry runs `git paw init` and a real `git paw start` with a stub agent CLI in an isolated environment (a private tmux socket, an isolated HOME, an ephemeral broker port, a throwaway repository) and asserts that the files, git hooks, session environment, panes, sidecar blocks and first prompts present afterwards equal the surface plan for that combination. The Manual entry SHALL additionally assert that no file git-paw wrote contains a broker token listed in *Instruction content names only enabled layers*. A failure SHALL name the combination and each missing or unexpected artifact.

#### Scenario: The manual entry is the strictest check

- **WHEN** the Manual matrix entry runs
- **THEN** it SHALL pass only if no hook, marker, helper script, skill, dashboard, broker process or broker token is present

#### Scenario: A leaked artifact fails the entry that leaked it

- **GIVEN** a change that installs `broker.sh` into a Manual session's worktree
- **WHEN** the matrix runs
- **THEN** the Manual entry SHALL fail naming `broker.sh` as unexpected
- **AND** the other entries SHALL be unaffected

#### Scenario: Each combination is a separate CI result

- **WHEN** the CI workflow runs the matrix
- **THEN** each combination SHALL be reported as its own job or test result

### Requirement: A modes chapter documents every layer

The user guide SHALL contain a modes chapter (`docs/src/user-guide/modes.md`) that, for each layer, states what enables it, what it requires, every artifact it installs, and how to turn it off and remove its surface, and that explains the dashboard and the MCP server's place relative to the layers. A test SHALL fail when a layer or a surface-plan artifact is missing from the chapter.

#### Scenario: The chapter covers the plan

- **WHEN** the modes chapter is checked against the layer list and the surface plan
- **THEN** every layer and every artifact SHALL be named in it

#### Scenario: An undocumented artifact fails the check

- **GIVEN** an artifact added to a layer's surface plan
- **WHEN** the documentation check runs before the chapter names it
- **THEN** the check SHALL fail naming the artifact

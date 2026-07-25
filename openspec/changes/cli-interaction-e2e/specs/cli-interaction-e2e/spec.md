## ADDED Requirements

### Requirement: Interactive contract is verified end-to-end

The interactive prompt surface SHALL be verified end-to-end by driving the real binary
through a PTY (a detached tmux pane), asserting on rendered prompts and resulting written
state — not only through in-process `Prompter`/`SupervisorPrompt` trait mocks. These tests
SHALL be socket-isolated, `#[serial]`, and gated on `tmux` availability (skipping with a
notice when tmux is absent). Prompt presence SHALL be detected by polling until the prompt
renders (no fixed sleep as the primary synchronisation gate).

#### Scenario: PTY tests skip cleanly without tmux

- **GIVEN** `tmux` is not available
- **WHEN** the prompt-matrix tests run
- **THEN** they SHALL skip with a notice rather than fail

#### Scenario: Outcomes assert on written state, not pixels

- **WHEN** a prompt-matrix test drives a command to completion
- **THEN** it SHALL assert on observable outcomes (written config, `session.json`, tmux panes created)
- **AND** SHALL use pane capture only for prompt-presence and synchronisation

### Requirement: `git paw init` prompt gating

For `git paw init`, the following prompts SHALL be shown or bypassed according to existing
config and TTY.

#### Scenario: Supervisor confirm shown on fresh config with a TTY

- **GIVEN** no existing `[supervisor]` config and a TTY
- **WHEN** `git paw init` runs interactively
- **THEN** the "Enable supervisor?" `Confirm` SHALL be shown

#### Scenario: Supervisor confirm bypassed without a TTY

- **GIVEN** a non-TTY invocation
- **WHEN** `git paw init` runs
- **THEN** the supervisor confirm SHALL be bypassed and the generated config SHALL have `enabled = false`

#### Scenario: Test-command input shown only after supervisor yes

- **GIVEN** a TTY and the supervisor confirm answered "yes"
- **WHEN** `git paw init` runs
- **THEN** the test-command `Input` SHALL be shown
- **AND** WHEN supervisor is answered "no" or the invocation is non-TTY, the test-command input SHALL be bypassed

#### Scenario: Spec-system select shown on fresh config with a TTY

- **GIVEN** a fresh config and a TTY
- **WHEN** `git paw init` runs
- **THEN** the spec-system `Select` (4 formats) SHALL be shown
- **AND** WHEN non-TTY, it SHALL be bypassed and a commented template written

#### Scenario: Migrate-supervisor confirm gated on an existing config

- **GIVEN** an existing config missing a `[supervisor]` section and a TTY
- **WHEN** `git paw init` runs
- **THEN** the migrate-supervisor `Confirm` SHALL be shown
- **AND** WHEN the section is already present, or the invocation is non-TTY, it SHALL be bypassed

### Requirement: `git paw start` prompt gating

For interactive `git paw start`, prompts SHALL be shown or bypassed according to the flags
provided (via `interactive::run_selection` + the supervisor-mode chain).

#### Scenario: Branch picker gated on `--branches`

- **GIVEN** no `--branches` flag
- **WHEN** `git paw start` runs interactively
- **THEN** the fuzzy multi-select branch picker SHALL be shown
- **AND** WHEN `--branches` is given, it SHALL be bypassed

#### Scenario: Mode picker gated on `--cli`

- **GIVEN** no `--cli` flag
- **WHEN** `git paw start` runs interactively
- **THEN** the Uniform/PerBranch mode picker SHALL be shown
- **AND** WHEN `--cli` is given, it SHALL be bypassed

#### Scenario: Uniform CLI picker

- **GIVEN** mode = Uniform and no `--cli`
- **WHEN** `git paw start` runs
- **THEN** a single CLI picker SHALL be shown; WHEN `--cli` is given it SHALL be bypassed

#### Scenario: Per-branch CLI picker

- **GIVEN** mode = PerBranch and no `--cli`
- **WHEN** `git paw start` runs over N branches
- **THEN** a CLI picker SHALL be shown once per branch; WHEN `--cli` is given all SHALL be bypassed

#### Scenario: Supervisor confirm resolves through the mode chain

- **GIVEN** the `resolve_supervisor_mode` chain resolves to *prompt*
- **WHEN** `git paw start` runs
- **THEN** the "Start in supervisor mode?" `Confirm` SHALL be shown
- **AND** WHEN `--supervisor`, `--no-supervisor`, `--unattended`, or a non-TTY invocation short-circuits the chain, it SHALL be bypassed

### Requirement: `git paw start --from-specs` prompt gating

For `git paw start --from-specs`, prompts and errors SHALL follow the spec picker and the
CLI-resolution chain (via `interactive::resolve_cli_for_specs` + `select_specs`).

#### Scenario: Spec picker shown with a TTY

- **GIVEN** `--from-specs` and a TTY
- **WHEN** `git paw start` runs
- **THEN** the spec picker (`select_specs`) SHALL be shown

#### Scenario: CLI picker short-circuits through the resolution chain

- **GIVEN** `--from-specs`
- **WHEN** a CLI is resolvable via `--cli`, a spec's `paw_cli`, or `default_spec_cli`
- **THEN** the CLI picker SHALL be bypassed (short-circuited)
- **AND** WHEN the chain falls through with none of these, the CLI picker SHALL be shown

#### Scenario: Spec-format resolution error when unconfigured

- **GIVEN** neither `--specs-format` nor `[specs]` is configured
- **WHEN** `git paw start --from-specs` runs
- **THEN** it SHALL error with the explicit-only guidance (per the v0.12.0 rule), not silently auto-detect

### Requirement: Destructive-confirmation gating

`git paw purge` SHALL prompt for confirmation unless `--force` is passed. `git paw stop`
SHALL stop the session without an interactive confirmation (its `--force` flag is currently
inert); this documents shipped behaviour and resolves a known contradiction — the
`cli-parsing` spec mandates a stop confirmation that `cmd_stop` does not render — which the
`spec-traceability-audit` change reconciles by amending that spec to match code. `git paw
remove` is guarded by `--force` / a dirty-check, not a prompt.

#### Scenario: Purge confirm gated on `--force`

- **GIVEN** state to purge and no `--force`
- **WHEN** `git paw purge` runs
- **THEN** a confirmation `Confirm` SHALL be shown; WHEN `--force` is given it SHALL be bypassed

#### Scenario: Stop does not prompt

- **GIVEN** an active session
- **WHEN** `git paw stop` runs, with or without `--force`
- **THEN** the session SHALL be stopped without an interactive confirmation prompt

## ADDED Requirements

### Requirement: Override markers are never empty

Every marker set in a `[clis.<name>].prompt_profile` override SHALL contain at least one non-whitespace character. An empty or whitespace-only marker SHALL fail the configuration load with an error naming `[clis.<name>].prompt_profile.<field>`, because under the helper's `grep -E` an empty alternative matches every line.

#### Scenario: An empty marker fails the load

- **GIVEN** `[clis.mycli].prompt_profile.approval_markers = ["ok", ""]`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `[clis.mycli].prompt_profile.approval_markers`

#### Scenario: A whitespace-only broad-grant marker fails the load

- **GIVEN** `[clis.mycli].prompt_profile.broad_grant_marker = " "`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `[clis.mycli].prompt_profile.broad_grant_marker`

### Requirement: Shell-matched patterns stay within the grep -E subset

An override of a pattern the bundled helper also matches with `grep -E` (`option_line_pattern`, `context_bloat_pattern`, `paste_buffer_pattern`) SHALL use only constructs both engines run. A pattern containing an inline flag group `(?…)`, a class escape `\d` `\D` `\s` `\S` `\w` `\W` `\p` `\P`, an anchor `\A` or `\z`, or a lazy quantifier SHALL fail the load with an error naming the field and `grep -E`.

#### Scenario: A Rust-only construct fails the load

- **GIVEN** `[clis.mycli].prompt_profile.option_line_pattern = '^\d[.)]'`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `[clis.mycli].prompt_profile.option_line_pattern` and `grep -E`

#### Scenario: Inline flags and lazy quantifiers fail the load

- **GIVEN** `context_bloat_pattern = '(?i)clear to save (\d+)k'` in one config and `paste_buffer_pattern = 'Pasted text #[0-9]+?'` in another
- **WHEN** each config is loaded
- **THEN** each load SHALL fail with an error naming its field and `grep -E`

#### Scenario: A Rust-only field keeps the full syntax

- **GIVEN** `[clis.mycli].prompt_profile.file_prompt_pattern = '(?i)write to\s+(.+)$'`
- **WHEN** the config is loaded
- **THEN** loading SHALL succeed, because only the compiled code matches that field

### Requirement: The profile helper command fails on a configuration error

`git paw __prompt-profile` SHALL exit non-zero, printing nothing on stdout, when the configuration fails to load. It SHALL NOT print the embedded defaults as if they were the configured profile.

#### Scenario: An invalid config makes the helper command fail

- **GIVEN** a repository whose `[clis.mycli].prompt_profile.option_line_pattern = '^[0-9'`
- **WHEN** `git paw __prompt-profile --cli mycli --field option_line_pattern` is run
- **THEN** it SHALL exit non-zero with empty stdout

## MODIFIED Requirements

### Requirement: A CLI prompt-shape profile describes one CLI's terminal surface

The system SHALL define a per-CLI prompt-shape profile of the literals and patterns showing how one CLI's terminal surface looks: readiness, approval-prompt, live-prompt, mid-response and permission-mode markers; the command-header, option-line and file-prompt forms; the broad-grant marker; the input sigil; stream-error, context-bloat and paste-buffer markers; and `cli_unavailable_markers` (usage limit or lost authentication). It SHALL NOT carry any decision about what is permitted.

#### Scenario: Profile carries prompt-shape fields

- **WHEN** a CLI prompt-shape profile is resolved
- **THEN** it SHALL provide readiness markers, approval markers, mid-response markers, the command-header form, the file-prompt pattern, the option-line form, the input sigil, the mode markers, and the CLI-unavailability markers for that CLI

#### Scenario: Profile carries no permission policy

- **WHEN** a CLI prompt-shape profile is inspected
- **THEN** it SHALL NOT contain the danger list, protected-path rules, worktree-boundary rules, the broad-grant eligibility rule, or any other determination of whether a command may run

#### Scenario: CLI-unavailability markers are overridable and readable by the helper script

- **GIVEN** a `[clis.<name>].prompt_profile` override that sets `cli_unavailable_markers`
- **WHEN** the profile is resolved for that CLI and `git paw __prompt-profile --cli <name> --field cli_unavailable_markers` is run
- **THEN** the resolved markers SHALL be the override's values
- **AND** the hidden command SHALL print them, so the bundled helper script and the compiled loop read one source

### Requirement: The embedded default profile is read only by profile resolution

The embedded default profile SHALL be read only by profile resolution, as the fallback for a CLI with
no profile of its own; no detector SHALL read it directly. A profile SHALL be resolved once per agent
and reused for every decision about that agent's surface.

#### Scenario: No overrides reproduces current decisions

- **GIVEN** no `prompt_profile` override for any CLI
- **WHEN** every detector runs against the existing capture fixtures
- **THEN** each decision SHALL equal the decision the embedded default profile yields for that fixture

### Requirement: A surface's CLI is taken from git-paw's launch record

The system SHALL determine which CLI runs on a surface from the launch record git-paw wrote when it
launched that agent (the session state's recorded CLI for the agent), and SHALL NOT take it from any
field an agent can publish to the broker. An agent with no recorded CLI SHALL resolve the `generic`
family's profile.

#### Scenario: A published status cannot change the profile

- **GIVEN** a worker launched with `cli = "claude"` that publishes `agent.status` with
  `cli = "mycli"`
- **WHEN** the drive loop resolves the profile for that worker's surface
- **THEN** it SHALL resolve the profile for `claude`

#### Scenario: An unrecorded CLI falls back to generic

- **GIVEN** an agent whose launch record carries no CLI
- **WHEN** its profile is resolved
- **THEN** the `generic` family's profile SHALL be used and detection SHALL remain operative

#### Scenario: A CLI swap on a kept worktree re-resolves the profile

- **GIVEN** a worker `feat/x` launched with `cli = "claude"`, then `git paw remove feat/x --keep-worktree` and `git paw add feat/x --cli mycli`, where `mycli` has a `prompt_profile` override
- **WHEN** the drive loop and the helper resolve the profile for that worker's surface
- **THEN** both SHALL resolve `mycli`'s profile, not `claude`'s

### Requirement: The helper script resolves the profile per pane

The bundled supervisor helper script SHALL resolve prompt-shape fields for each pane from that
pane's recorded CLI, through the same resolution the compiled detectors use, resolving each field at
most once per distinct CLI per invocation. This covers the live-prompt re-confirm and the option-1
guard of `sweep.sh approve`. When a pane has no recorded CLI the helper SHALL resolve the field
without naming a CLI, which resolves the session's configured CLI; the compiled loop resolves `generic`.

#### Scenario: The helper and the loop read a pane the same way

- **GIVEN** a session with a `mycli` worker whose override sets `approval_markers`
- **WHEN** the helper evaluates that worker's pane
- **THEN** it SHALL match the override's markers, as the compiled loop does for the same pane

#### Scenario: The helper re-confirm reads a pane's own live-prompt markers

- **GIVEN** a session with a `mycli` worker whose override sets `live_prompt_markers`
- **WHEN** the helper's live-prompt re-confirm evaluates that worker's pane
- **THEN** it SHALL match the override's markers, as the compiled loop does for the same pane

#### Scenario: The approve guard reads the pane's own broad-grant marker

- **GIVEN** a session with a `claude` worker and a `mycli` worker whose override sets
  `broad_grant_marker = "remember this"`, and the `mycli` pane showing a live prompt whose option-1
  line contains `remember this`
- **WHEN** `sweep.sh approve <pane>` runs against the `mycli` pane
- **THEN** it SHALL send no keystrokes and report that option 1 is a durable grant
- **AND** the `claude` pane's guard SHALL still use the embedded default marker

#### Scenario: A pane with no recorded CLI resolves the session CLI in the helper

- **GIVEN** a session whose configured CLI is `mycli` and a pane with no recorded CLI
- **WHEN** the helper resolves `approval_markers` for that pane
- **THEN** it SHALL run `__prompt-profile --field approval_markers` without `--cli` and use `mycli`'s profile
- **AND** the compiled loop SHALL resolve the `generic` profile for the same pane

## ADDED Requirements

### Requirement: Detectors receive the profile of the surface they inspect

Every production prompt-shape decision SHALL use the profile resolved for the CLI running on the
surface it inspects: launch readiness, approval-marker detection, the live-prompt gate and the
`sweep.sh approve` re-confirm, command-slice extraction for escalation text, dedup and learnings,
file-prompt path matching, option-line recognition (including the `sweep.sh approve` option-1
guard), mid-response and input-box detection, CLI-unavailability detection, permission-mode
inference, and manual-approval boilerplate filtering. The embedded
default profile SHALL be read only by profile resolution, as the fallback for a CLI with no profile
of its own; no detector SHALL read it directly. A profile SHALL be resolved once per agent and reused
for every decision about that agent's surface.

#### Scenario: An override reaches the drive loop for that pane

- **GIVEN** `[clis.mycli].prompt_profile.live_prompt_markers = ["approve this?"]` and a worker
  running `mycli`
- **WHEN** the drive loop evaluates that worker's capture whose tail shows `approve this?`
- **THEN** the pane SHALL be treated as showing a live prompt
- **AND** the prompt SHALL be escalated for a human decision, with no keystroke sent to the pane

#### Scenario: A mixed session uses each pane's own profile

- **GIVEN** one worker running `claude` and one running `mycli` with a `prompt_profile` override
- **WHEN** both panes are swept
- **THEN** the `claude` pane SHALL be evaluated with the embedded default
- **AND** the `mycli` pane SHALL be evaluated with its override

#### Scenario: No overrides reproduces current decisions

- **GIVEN** no `prompt_profile` override for any CLI
- **WHEN** every detector runs against the existing capture fixtures
- **THEN** each decision SHALL equal the decision made before this change

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

### Requirement: Prompt-shape decisions are pure functions of a profile and a capture

Each prompt-shape decision SHALL be computable from a resolved profile, the captured surface text,
and any other observed inputs it needs, with no terminal, broker, network, clock or filesystem
access of its own. Evaluating a decision on a recorded capture SHALL produce the same result the
live loop produces for the same text, so recorded captures for any CLI can be replayed through the
detectors.

#### Scenario: A recorded capture replays to the live decision

- **GIVEN** a recorded capture of a permission prompt and the profile of the CLI that produced it
- **WHEN** the detectors are evaluated on the recording with no terminal or broker present
- **THEN** whether a live prompt is present and the command slice it would be escalated with SHALL
  equal those the drive loop produces for the same capture

### Requirement: Override patterns are validated when the configuration loads

Every regular-expression field set in a `[clis.<name>].prompt_profile` override SHALL be compiled
when the configuration loads. An invalid pattern SHALL fail the load with an error naming
`[clis.<name>].prompt_profile.<field>`. A pattern SHALL be compiled once per resolved profile, and a
detector SHALL NOT panic on, or silently skip, an overridden pattern.

#### Scenario: An invalid pattern fails the load

- **GIVEN** `[clis.mycli].prompt_profile.option_line_pattern = "^[0-9"`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `[clis.mycli].prompt_profile.option_line_pattern`

#### Scenario: A valid pattern is used by the detector

- **GIVEN** `[clis.mycli].prompt_profile.option_line_pattern = "^\\([0-9]\\)"`
- **WHEN** a `mycli` prompt with option lines `(1) Yes` and `(2) No` is evaluated
- **THEN** the option lines SHALL be recognised

### Requirement: The helper script resolves the profile per pane

The bundled supervisor helper script SHALL resolve prompt-shape fields for each pane from that
pane's recorded CLI, through the same resolution the compiled detectors use, resolving each field at
most once per distinct CLI per invocation. This covers the live-prompt re-confirm and the option-1
guard of `sweep.sh approve`. When a pane has no recorded CLI the helper SHALL resolve the field
without naming a CLI, as before this change.

#### Scenario: The helper and the loop read a pane the same way

- **GIVEN** a session with a `mycli` worker whose override sets `approval_markers`
- **WHEN** the helper evaluates that worker's pane
- **THEN** it SHALL match the override's markers, as the compiled loop does for the same pane

#### Scenario: The approve guard reads the pane's own broad-grant marker

- **GIVEN** a session with a `claude` worker and a `mycli` worker whose override sets
  `broad_grant_marker = "remember this"`, and the `mycli` pane showing a live prompt whose option-1
  line contains `remember this`
- **WHEN** `sweep.sh approve <pane>` runs against the `mycli` pane
- **THEN** it SHALL send no keystrokes and report that option 1 is a durable grant
- **AND** the `claude` pane's guard SHALL still use the embedded default marker

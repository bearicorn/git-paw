# cli-prompt-profiles Specification

## Purpose
Defines a per-CLI prompt-shape profile — the readiness, approval, live-prompt, mid-response,
command-header, file-prompt, option-line, broad-grant, input-sigil, permission-mode, and
stream/context/paste-buffer markers describing how one agent CLI's terminal surface *looks*. A
profile carries appearance only, never permission policy: the danger list, protected-path and
worktree-boundary rules, and broad-grant eligibility stay compiled and un-overridable, so a profile
can never widen the gate that governs an agent. The Claude Code profile is embedded as the default
with per-field fallback (a partial profile never disables detection), and both the compiled code and
the bundled supervisor helper script read the profile as their single source of prompt-shape
literals; the detection algorithms are unchanged — only the origin of the literals moves.

## Requirements

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

### Requirement: Security decisions are never profile-sourced

The live-prompt gate's anchoring discipline (its tail and block windows and the bounded trailing region), the approval send gate (the fresh-capture re-confirm and the pane-0 refusal), and the `sweep.sh approve` rule that only option `1` is ever sent SHALL remain compiled, or for the bundled helper fixed in its script. They SHALL NOT be readable or overridable from any profile, configuration file, or worktree-writable location. A profile supplies only the literals these gates match. A profile that could change the gates themselves would let a gated agent widen the gate that governs it.

#### Scenario: A profile cannot widen the live-prompt window

- **GIVEN** a profile whose markers and option-line form are overridden
- **WHEN** the live-prompt gate evaluates a capture
- **THEN** the tail window, the block window and the trailing-region bound SHALL be those compiled into the gate

#### Scenario: A profile cannot make approve select another option

- **GIVEN** a profile that overrides `option_line_pattern` and `broad_grant_marker`
- **WHEN** `sweep.sh approve <pane>` runs against a live prompt
- **THEN** it SHALL send either `1` followed by `Enter`, or nothing
- **AND** it SHALL NOT send any other option digit

#### Scenario: The pane-0 refusal ignores profile content

- **GIVEN** any resolved profile
- **WHEN** `sweep.sh approve 0` runs
- **THEN** it SHALL send no keystrokes and report that pane 0 is excluded

### Requirement: The Claude Code profile is embedded as the default

The system SHALL embed a Claude Code profile compiled into the binary, and SHALL use it as the default for every field. When no profile configuration is present, resolved behaviour SHALL be identical to the behaviour before profiles existed. The embedded profile's `cli_unavailable_markers` SHALL include Claude Code's session/usage-limit, organisation spend-limit and authentication-failure phrasings, including `You've hit your session limit` and `org's monthly spend limit`. The spend-limit marker SHALL be scoped to that organisation-specific phrasing rather than the bare phrase `monthly spend limit`, which is plausible wording in an unrelated consumer application a worker might legitimately have open and would wrongly read as CLI unavailability. The embedded profile's `stream_error_markers` SHALL additionally include the mid-response connection-drop phrasings `API Error` and `Connection closed mid-response`, which denote a transient drop whose conversation context survives — a nudge resumes the worker — and SHALL NOT be placed in `cli_unavailable_markers`, which is reserved for terminal conditions requiring human intervention.

#### Scenario: Absent configuration reproduces current behaviour

- **GIVEN** an installation with no profile configuration
- **WHEN** readiness, approval detection, mid-response detection, mode detection, and prompt-shape identification run
- **THEN** each SHALL resolve the same literals it used before profiles were introduced

#### Scenario: Single-binary distribution preserved

- **WHEN** the binary is distributed without any accompanying profile file
- **THEN** the embedded Claude Code profile SHALL still resolve

#### Scenario: The embedded profile recognises the Claude Code session limit

- **GIVEN** no profile configuration and a pane capture whose tail contains `You've hit your session limit`
- **WHEN** the capture is matched against the resolved `cli_unavailable_markers`
- **THEN** it SHALL match

#### Scenario: The embedded profile recognises an organisation spend limit

- **GIVEN** no profile configuration and a pane capture whose tail contains `You've hit your org's monthly spend limit · run /usage-credits to ask your admin for a higher limit`
- **WHEN** the capture is matched against the resolved `cli_unavailable_markers`
- **THEN** it SHALL match
- **AND** it SHALL NOT match `stream_error_markers`

#### Scenario: A mid-response connection drop is a stream error, not CLI unavailability

- **GIVEN** no profile configuration and a pane capture whose tail contains `API Error: Connection closed mid-response`
- **WHEN** the capture is matched against the resolved marker sets
- **THEN** it SHALL match `stream_error_markers`
- **AND** it SHALL NOT match `cli_unavailable_markers`
- **AND** the wave SHALL NOT exit with outcome `cli-unavailable` on account of it

### Requirement: Per-field fallback to the embedded default

A profile SHALL be permitted to specify a subset of fields. Any field a profile does not specify SHALL fall back to the embedded default rather than resolving empty, so a partial profile can never silently disable detection.

#### Scenario: Partial profile falls back per field

- **GIVEN** a profile that specifies only approval markers
- **WHEN** readiness markers are resolved for that CLI
- **THEN** they SHALL resolve to the embedded default values
- **AND** SHALL NOT resolve to an empty set

#### Scenario: Empty field does not disable detection

- **GIVEN** a profile field that is absent
- **WHEN** the corresponding detection runs
- **THEN** detection SHALL behave as though the embedded default were configured

### Requirement: Rust and the supervisor helper script read one source

The profile SHALL be the single source of prompt-shape literals for both the compiled code paths and the bundled supervisor helper script. The helper script SHALL NOT carry its own copies of the markers or regexes. A prompt-shape literal SHALL NOT be duplicated between the two.

#### Scenario: Helper script sources markers from the profile

- **WHEN** the bundled supervisor helper script evaluates a prompt shape
- **THEN** it SHALL read the markers from the resolved profile
- **AND** it SHALL NOT use a locally defined copy of those markers

#### Scenario: No duplicated literal between shell and Rust

- **WHEN** the bundled helper script and the compiled code are compared for prompt-shape literals
- **THEN** no prompt-shape literal SHALL appear independently defined in both

### Requirement: Detection algorithms are unchanged by profile sourcing

Moving the literals into a profile SHALL NOT change any detection algorithm: the tail and block capture windows, the last-header-wins command extraction, the option-index selection rule, the rate limiting, and the re-confirm gate SHALL behave as before. Only the origin of the literals changes.

#### Scenario: Extraction semantics preserved

- **GIVEN** a capture that previously yielded a particular extracted command
- **WHEN** the same capture is processed with profile-sourced markers resolving to the same literals
- **THEN** the extracted command SHALL be identical

#### Scenario: Rate limiting preserved

- **WHEN** captures are taken with profile-sourced markers
- **THEN** the capture rate limiting SHALL behave as it did before profiles were introduced

### Requirement: An unrecognised CLI resolves a usable profile

When a CLI has no profile of its own, the system SHALL resolve the embedded default rather than failing or resolving an empty profile, so an unrecognised CLI degrades to current behaviour rather than to no detection at all.

#### Scenario: Unknown CLI falls back to the embedded default

- **GIVEN** a CLI with no profile defined
- **WHEN** its prompt-shape profile is resolved
- **THEN** the embedded default profile SHALL be returned
- **AND** detection SHALL remain operative

### Requirement: Detectors receive the profile of the surface they inspect

Every production prompt-shape decision SHALL use the profile resolved for the CLI on the surface it
inspects. This covers launch readiness, approval-marker and live-prompt detection (including the
`sweep.sh approve` re-confirm), escalation command slices, dedup and learnings, file-prompt paths,
option lines (including the `sweep.sh approve` option-1 guard), mid-response, input-box and
CLI-unavailability detection, mode inference and manual-approval boilerplate filtering.

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

### Requirement: A selection menu's shape comes from the profile

The profile SHALL carry `menu_cursor_glyphs` and `menu_footer_markers`, the glyphs that mark a menu's selected option and the footer phrases that mark a menu. The embedded default SHALL be `❯`, `›`, `>` and `enter to select`, footers matched case-insensitively. A `[clis.<name>].prompt_profile` override MAY replace either field; an empty list SHALL fail the load naming the field. The fields describe appearance only.

#### Scenario: The default profile detects a menu as before

- **GIVEN** an agent whose CLI has no prompt-profile override
- **WHEN** `__send` reads a screen ending in `❯ 1. Send`, `2. Wait` and `Enter to select`
- **THEN** it SHALL refuse with `selection menu showing` and send no keystroke

Test: `src/commands/send.rs::a_selection_menu_blocks_the_send`, `src/supervisor/screen.rs::the_default_glyphs_and_footer_mark_a_menu`

#### Scenario: An overridden cursor glyph is honoured

- **GIVEN** `[clis.mycli].prompt_profile` with `menu_cursor_glyphs = ["»"]`
- **WHEN** `__send` to an agent running `mycli` reads a screen whose last lines are `» 1. Send` and `2. Wait`
- **THEN** it SHALL refuse with `selection menu showing`

Test: `src/commands/send.rs::an_overridden_cursor_glyph_is_honoured`, `src/supervisor/screen.rs::an_overridden_cursor_glyph_replaces_the_defaults`

#### Scenario: An overridden footer replaces the default

- **GIVEN** `[clis.mycli].prompt_profile` with `menu_footer_markers = ["press space to choose"]`
- **WHEN** `__send` to an agent running `mycli` reads a screen whose last line is `Press space to choose`
- **THEN** it SHALL refuse with `selection menu showing`

Test: `src/commands/send.rs::an_overridden_footer_replaces_the_default`, `src/supervisor/screen.rs::an_overridden_footer_replaces_the_default`

#### Scenario: The helper command prints the menu fields

- **WHEN** `git paw __prompt-profile --cli claude --field menu_cursor_glyphs` is run with no override
- **THEN** it SHALL print the embedded default glyphs

Test: `tests/prompt_profile_threading.rs::prompt_profile_subcommand_prints_the_menu_fields`

#### Scenario: An empty menu list fails the load

- **GIVEN** `[clis.mycli].prompt_profile` with `menu_cursor_glyphs = []` or `menu_footer_markers = []`
- **WHEN** the configuration is loaded
- **THEN** loading SHALL fail with an error naming that field

Test: `tests/prompt_profile_threading.rs::an_empty_menu_list_fails_the_load_naming_the_field`

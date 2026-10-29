## ADDED Requirements

### Requirement: Every CLI is assessed against the twelve worker-lifecycle steps

The support matrix SHALL assess every row against the same twelve worker-lifecycle steps, identified
by these stable ids:

- `detection` — the binary found on PATH is the agent CLI, and git-paw can read its version
- `launch` — git-paw's launch command starts it in a worktree surface, with the flags git-paw renders
- `readiness` — start-up dialogs are handled (first-run and trust dialogs, and notices the CLI shows
  at launch, such as a post-update notice or a usage-limit notice), and launch readiness is
  recognised
- `boot-block` — the boot block reaches the agent (over MCP or the helper scripts) and the agent
  registers
- `prompt-shapes` — permission prompts, the live prompt, working and idle screens, and stuck or
  CLI-unavailable shapes are recognised, and each permission prompt yields the command (or file
  target) its escalation names; the drive loop escalates only the prompts this step recognises
- `approval` — an explicit approve answers once: on each permission-prompt form, option `1` is
  present and is not a durable "don't ask again" grant, as `sweep.sh approve`'s option-1 guard reads
  it
- `send-submit` — text sent to the agent is submitted, including the paste-buffer and follow-up-Enter
  cases
- `native-modes` — the approval flags git-paw renders for each approval level, and the CLI's native
  auto-mode, sandbox, and hooks, behave as git-paw configures them; a level documented as
  prompt-free raises no permission prompt
- `resume` — a session id is assigned at launch and a cold resume restores the conversation
- `headless` — the headless one-shot invocation reads its prompt from stdin and exits
- `exit` — a running CLI reads as a live agent, and its exit or crash reads as not alive
- `usage` — git-paw reads the CLI's token and cost usage

Every row SHALL record exactly one status per step: `verified`, `gap` (with a non-empty note saying
what does not work), or `unverified`.

#### Scenario: Every row states every step

- **GIVEN** the compiled support matrix
- **WHEN** its rows are enumerated
- **THEN** each row SHALL carry a status for each of the twelve step ids and for no other id

#### Scenario: A gap without a note is rejected

- **GIVEN** a row that records `gap` for a step with an empty note
- **WHEN** the matrix parity suite runs
- **THEN** it SHALL fail, naming the row and the step

### Requirement: Support tiers are derived from step statuses

A row's support tier SHALL be computed from its step statuses and SHALL NOT be recorded on its own:

- **first-class** — every step is `verified`
- **supported** — `detection` and `launch` are `verified`, and every other step is `verified` or `gap`
- **launch-only** — `detection` and `launch` are `verified`, and at least one other step is
  `unverified`; git-paw launches the CLI and automates nothing else for it

A row whose `detection` or `launch` step is not `verified` SHALL derive no tier.

#### Scenario: Tiers follow the step statuses

- **WHEN** a tier is derived for each of these rows
  - every step `verified`
  - one step `gap`, the rest `verified`
  - `detection` and `launch` `verified`, one other step `unverified`
  - `launch` `gap`, the rest `verified`
  - `detection` `unverified`, the rest `verified`
- **THEN** the derived tiers SHALL be first-class, supported, launch-only, none, and none
  respectively

### Requirement: A verified step requires recorded evidence at the row's version

A step SHALL be recorded `verified` only when its evidence exists in the row's fixture set for the
row's recorded CLI version. Evidence comes in two forms:

- a reviewed fixture case for that step that replays green, required for every part of the step that
  git-paw decides from recorded CLI output
- a dated attestation for that step, required for every part that can be observed only in a live
  run

The evidence each step requires SHALL be:

- `detection`: a fixture (the version banner)
- `launch`: a fixture (help text containing every flag and subcommand token git-paw renders to
  launch the CLI)
- `readiness`: fixtures (the ready screen and each start-up dialog the CLI shows), plus, for each
  dialog git-paw dismisses with a key, an attestation that the key dismissed it and the CLI then
  reached readiness
- `boot-block`: an attestation
- `prompt-shapes`: fixtures (each permission-prompt form with the command or file target its
  escalation names, a working screen, an idle screen, and each stuck or CLI-unavailable shape the CLI
  exhibits)
- `approval`: fixtures (each permission-prompt form with its option-1 reading)
- `send-submit`: fixtures (an input box holding unsubmitted text, and the same box after submit)
- `native-modes`, `resume`, `headless`: a fixture (help text containing the tokens git-paw renders
  for that step) and an attestation; for `native-modes` the attestation covers each approval level
  git-paw renders, including that a level documented as prompt-free raised no permission prompt
- `exit`: a fixture (the CLI's foreground process name, read as a live agent) and an attestation
- `usage`: a fixture (a usage artifact and the values git-paw reads from it)

A step that has a known-gap fixture case SHALL NOT be recorded `verified`.

#### Scenario: A verified step without fixtures fails the suite

- **GIVEN** a row that records `prompt-shapes` as `verified`
- **AND** its fixture set has no reviewed `prompt-shapes` case
- **WHEN** the matrix parity suite runs
- **THEN** it SHALL fail, naming the row and the step

#### Scenario: A live-only part needs an attestation

- **GIVEN** a row that records `boot-block` as `verified`
- **AND** its fixture set has no `boot-block` attestation
- **WHEN** the matrix parity suite runs
- **THEN** it SHALL fail, naming the row and the step

#### Scenario: A known gap contradicts a verified status

- **GIVEN** a fixture set with a `known_gap` case for `approval`
- **AND** the row records `approval` as `verified`
- **WHEN** the matrix parity suite runs
- **THEN** it SHALL fail, naming the row and the step

### Requirement: Each matrix row records the CLI version and date it was verified against

Every matrix row SHALL record the exact CLI version string its statuses were verified against and
the date of that verification. The row's evidence SHALL live in the fixture directory for that
version. Moving a row to a new CLI version SHALL re-establish every `verified` step's evidence in
the new version's directory. Older version directories MAY remain, and while they remain they SHALL
keep replaying.

#### Scenario: The recorded version must have a fixture set

- **GIVEN** a row that records version `1.4.0`
- **AND** there is no fixture directory for that CLI at `1.4.0`
- **WHEN** the matrix parity suite runs
- **THEN** it SHALL fail, naming the row and the missing version directory

#### Scenario: An older version keeps replaying

- **GIVEN** fixture directories for a CLI at `1.3.0` and `1.4.0`
- **AND** the row records `1.4.0`
- **WHEN** the conformance suite runs
- **THEN** the cases in both directories SHALL be replayed

### Requirement: Conformance fixtures are stored per CLI and version in a declared format

Conformance fixtures SHALL live under `tests/fixtures/clis/<binary>/<version>/`. Each directory
SHALL hold a `manifest.toml` and the case files that manifest names. The manifest SHALL carry:

- `format` (currently `1`), `cli` (the binary name), `version`, and `captured_on`
- an optional `process_name`: the foreground process name observed while the CLI runs
- `[[case]]` entries, each with:
  - `name` (kebab-case, unique in the manifest)
  - `step` (a lifecycle step id)
  - `kind` (`screen` for a captured surface, `text` for any other recorded CLI output)
  - `file`
  - `reviewed`
  - an `expect` table of expected readings
  - an optional `known_gap` note
- `[[attestation]]` entries, each with `step`, `outcome` (`verified` or `gap`), `note`, and
  `attested_on`

A manifest with an unknown key, an unknown step id, a `cli` or `version` that does not match its
directory, an entry naming a missing file, or a case file the manifest does not name SHALL fail the
conformance suite.

#### Scenario: A well-formed fixture set loads

- **GIVEN** a fixture directory whose manifest names two existing case files with valid step ids
- **WHEN** the conformance suite loads it
- **THEN** both cases SHALL be replayed

#### Scenario: An unknown step id is rejected

- **GIVEN** a manifest whose case declares `step = "launching"`
- **WHEN** the conformance suite loads it
- **THEN** the suite SHALL fail, naming the manifest and the unknown step id

#### Scenario: An orphan case file is rejected

- **GIVEN** a fixture directory holding a `.txt` file that no manifest entry names
- **WHEN** the conformance suite loads it
- **THEN** the suite SHALL fail, naming the orphan file

### Requirement: Conformance fixtures are replayed offline through git-paw's own decisions

The conformance suite SHALL replay every fixture case in CI through git-paw's own decisions, with no
network, no terminal multiplexer, and no CLI binary installed. Before replay,
`{{WORKTREE_ROOT}}` and `{{HOME}}` placeholders SHALL be replaced with fixed synthetic paths.

- A `screen` case SHALL be read with the profile git-paw resolves for the fixture's CLI when no user
  configuration is present. The reading covers readiness, the start-up dialog (`none`, or for a
  recognised dialog its answer: the option its affirmative text selects, the dismiss key, or
  `blocking`), approval marker present, live prompt, the
  escalation command (the command slice, or the file target when the prompt has no command header),
  the option-1 reading (`once` when option `1` is present and is not a durable grant, `durable` when
  its line carries the profile's broad-grant marker, `missing` when there is no option `1`),
  mid-response, buffered input, stuck shape, and CLI-unavailable line.
- A `text` case SHALL be read by the decision its step names: version parsing, rendered-token
  presence, or usage parsing.
- A manifest's `process_name` SHALL be read by the liveness decision.

Every key in a case's `expect` table SHALL equal git-paw's reading. Keys absent from `expect` SHALL
NOT be asserted. Where the bundled helper reads the same profile field as a compiled detector, the
case SHALL also be replayed through the helper, and the two readings SHALL agree. This covers the
stuck shapes, and the live prompt and option-1 reading of `sweep.sh approve`'s guard: an `approval`
case SHALL be replayed through that guard with a stand-in terminal that serves the capture and
records keystrokes, and the guard SHALL send `1` then `Enter` exactly when the reading is `once` and
send nothing otherwise. A case that carries `known_gap` SHALL be replayed and SHALL fail its
expectation. If it passes, the suite SHALL fail and ask for the `known_gap` to be cleared and the
step re-recorded.

#### Scenario: A matching case passes

- **GIVEN** a reviewed `screen` case whose `expect` gives `live_prompt = true` and
  `option_one = "once"`
- **AND** git-paw's reading of the capture agrees on both keys
- **WHEN** the conformance suite runs
- **THEN** the case SHALL pass

#### Scenario: A mismatch names the fixture, key, and values

- **GIVEN** a reviewed `screen` case whose `expect` gives `readiness = "ready"`
- **AND** git-paw reads the capture as `indeterminate`
- **WHEN** the conformance suite runs
- **THEN** the suite SHALL fail, naming the fixture, the key, the expected value, and the actual
  value

#### Scenario: Replay needs no CLI, multiplexer, or network

- **GIVEN** a CI runner with none of the matrix's CLI binaries and no tmux on PATH
- **WHEN** the conformance suite runs
- **THEN** every reviewed case SHALL be replayed and judged

#### Scenario: The bundled helper must agree

- **GIVEN** a reviewed stuck-shape case
- **AND** the compiled detector and the bundled helper's detector classify it differently
- **WHEN** the conformance suite runs
- **THEN** the suite SHALL fail, naming the case and both readings

#### Scenario: The approve guard must agree on option 1

- **GIVEN** a reviewed `approval` case whose capture is a live prompt with `1. Yes` and
  `2. Yes, and don't ask again`, and whose `expect` gives `option_one = "once"`
- **WHEN** the conformance suite replays it through `sweep.sh approve`'s guard
- **THEN** the stand-in terminal SHALL record `1` then `Enter` and no other keystroke
- **AND** if the guard sent nothing, or the compiled reading differed, the suite SHALL fail, naming
  the case and both readings

#### Scenario: A known gap that starts passing fails the suite

- **GIVEN** a case that carries `known_gap`
- **AND** git-paw's reading now matches its `expect`
- **WHEN** the conformance suite runs
- **THEN** the suite SHALL fail and ask for the `known_gap` to be cleared

### Requirement: Only reviewed fixtures count as evidence

A fixture case SHALL count as evidence only when its manifest marks it `reviewed = true`, and the
conformance suite SHALL fail while any case is unreviewed, so an unreviewed capture cannot be
committed silently. Review SHALL confirm two things: the `expect` table states the correct behaviour,
not merely git-paw's reading at capture time, and the captured text holds no secret or personal data.

#### Scenario: An unreviewed case fails the suite

- **GIVEN** a fixture set with one case marked `reviewed = false`
- **WHEN** the conformance suite runs
- **THEN** the suite SHALL fail, naming the unreviewed case

### Requirement: Fixtures are captured through the drive loop's observation path

The system SHALL provide a hidden maintainer subcommand, `git paw __capture-fixture`, that records
one fixture case into a fixture directory given explicitly on the command line.

- For a `screen` case, it SHALL capture the named agent's surface with the same capture-tail
  operation the drive loop observes with. It SHALL replace the agent's worktree root with
  `{{WORKTREE_ROOT}}` and the user's home directory with `{{HOME}}`, and it SHALL pre-fill `expect`
  with git-paw's reading under the agent's resolved profile.
- For a `text` case, it SHALL read the content from stdin.

The case SHALL be written with `reviewed = false`. The subcommand SHALL NOT send any input to the
surface, SHALL NOT overwrite an existing case, and SHALL reject a case name that is not kebab-case.

#### Scenario: A screen capture is written unreviewed with placeholders

- **GIVEN** a running session whose agent `feat-x` is showing a permission prompt that quotes a path
  inside its worktree
- **WHEN** `git paw __capture-fixture --agent feat-x --case bash-prompt --step approval --out <dir>`
  is run
- **THEN** `<dir>` SHALL gain the case file and a manifest entry with `reviewed = false` and a
  pre-filled `expect`
- **AND** the case file SHALL contain `{{WORKTREE_ROOT}}` where the worktree path appeared
- **AND** the agent's surface SHALL be unchanged and SHALL have received no input

#### Scenario: An existing case is not overwritten

- **GIVEN** a fixture directory that already has a case named `bash-prompt`
- **WHEN** the capture subcommand is run again with `--case bash-prompt`
- **THEN** it SHALL fail without changing the directory

#### Scenario: A traversing case name is rejected

- **WHEN** the capture subcommand is run with `--case ../escape`
- **THEN** it SHALL fail without writing any file

### Requirement: The support matrix is one compiled table that the docs are checked against

The support matrix SHALL be a single table compiled into the binary. The detection roster, `git paw
doctor`'s tier reporting, and the evidence checks SHALL all read it. The Supported AI CLIs docs page
SHALL publish the tier definitions and, for every row, the CLI, its binary, derived tier, verified
version, and verification date, plus every `gap` note. A parity test SHALL fail in either of these
cases:

- the page and the table disagree on a row's binary, tier, or verified version
- either one lists a binary the other lacks

#### Scenario: A wrong tier on the docs page fails

- **GIVEN** a matrix row that derives supported
- **AND** the docs page lists that CLI as first-class
- **WHEN** the parity test runs
- **THEN** it SHALL fail, naming the binary and both tiers

#### Scenario: A binary missing from either side fails

- **GIVEN** a docs page that omits one matrix binary and lists one binary the matrix lacks
- **WHEN** the parity test runs
- **THEN** it SHALL fail, naming both binaries

### Requirement: Every matrix row reaches at least launch-only

Every matrix row SHALL derive a tier of at least launch-only. A CLI whose `detection` or `launch`
step cannot be verified at any version the audit can obtain SHALL be removed from the matrix, and so
from auto-detection, rather than published below the floor. The removal SHALL be recorded in the
changelog and on the Supported AI CLIs page, with the reason and the `[clis.<name>]` route for users
who still want that CLI. An explicit `default_cli`, `[clis.<name>]` entry, or saved-session CLI that
names a removed binary SHALL continue to resolve as a pass-through.

#### Scenario: A row below the floor fails the suite

- **GIVEN** a matrix row whose `launch` step is `unverified`
- **WHEN** the matrix parity suite runs
- **THEN** it SHALL fail, naming the row

#### Scenario: A removed CLI still resolves when named explicitly

- **GIVEN** a binary that was removed from the matrix
- **AND** a config with `default_cli` naming that binary
- **WHEN** the CLI is resolved for a launch
- **THEN** it SHALL resolve to that name as a pass-through
- **AND** auto-detection SHALL NOT list the binary

### Requirement: Built-in per-CLI data ships only for steps with evidence

git-paw SHALL ship a built-in per-CLI datum only for a CLI whose matrix row records the step that
datum serves as `verified` or `gap`, never as `unverified`. The data and the steps they serve are:

- a prompt-shape profile serves `prompt-shapes`
- approval flags serve `launch`
- session-id and resume templates serve `resume`
- a headless template serves `headless`
- trust-marker data serves `readiness`
- start-up dialog data (each dialog's recognition text and its answer) serves `readiness`
- native auto-mode, sandbox, and hook data serve `native-modes`
- the `git paw setup-cli` fact rows serve the step of each item they declare: the allowlist and
  hooks items serve `native-modes`, the trust item serves `readiness`
- usage parsing serves `usage`

The `git paw setup-cli` fact table carries its own verification status per item (verified, or
plan-only when unverified). That status SHALL agree with the matrix: an item SHALL be recorded
verified only when the CLI's row records the item's step as `verified`, and an item whose step is
`gap` or `unverified` SHALL be plan-only.

Every marker literal in a CLI's own built-in prompt-shape profile or start-up dialog data (data
other than the embedded default that a CLI without its own inherits) SHALL appear verbatim in a
reviewed `screen` case of that CLI's fixture set at the row's recorded version. Such literals SHALL
be copied from captures, never written from documentation, release notes, or memory.

A CLI without a built-in prompt-shape profile SHALL resolve the generic profile. A parity test SHALL
fail when a built-in datum exists for a step that the CLI's row records as `unverified`, when a
setup-cli item's verification status disagrees with its step's status, and when a CLI's own marker
literal appears in none of its reviewed `screen` cases.

#### Scenario: An unverified step cannot carry built-in data

- **GIVEN** a CLI with built-in approval flags
- **AND** its matrix row records `launch` as `unverified`
- **WHEN** the matrix parity suite runs
- **THEN** it SHALL fail, naming the CLI, the datum, and the step

#### Scenario: A verified setup item needs a verified step

- **GIVEN** a setup-cli fact row whose allowlist item is recorded verified
- **AND** the CLI's matrix row records `native-modes` as `gap`
- **WHEN** the matrix parity suite runs
- **THEN** it SHALL fail, naming the CLI, the item, and the step

#### Scenario: A launch-only CLI resolves the generic profile

- **GIVEN** a CLI whose row derives launch-only and that has no built-in prompt-shape profile
- **WHEN** its prompt-shape profile is resolved with no user configuration
- **THEN** the generic profile SHALL be returned

#### Scenario: A built-in marker absent from every capture fails

- **GIVEN** a CLI's own built-in profile whose `readiness_markers` hold a literal
- **AND** none of that CLI's reviewed `screen` cases at the row's version contains the literal
- **WHEN** the matrix parity suite runs
- **THEN** it SHALL fail, naming the CLI, the field, and the literal

### Requirement: opencode is read with its own profile and start-up dialogs, built from captures

git-paw SHALL give `opencode` its own in-tree provider family, so that opencode panes are not read
with the Claude Code profile's markers. Its prompt-shape profile and start-up dialog data SHALL be
built only from opencode's reviewed fixture captures, under "Built-in per-CLI data ships only for
steps with evidence". Subject to the capture rule in the next paragraph:

- the profile SHALL cover at least readiness markers, the input-line sigil, mid-response markers,
  and `cli_unavailable_markers`, each taken from a capture of the screen it describes (idle, input
  holding text, working, usage limit reached)
- the start-up dialog data SHALL cover the post-update notice opencode shows on its first launch
  after an update, as a notice dismissed with the key the `readiness` attestation records, and its
  usage-limit notice, as a blocking dialog that git-paw never answers

A field or dialog with no reviewed capture at the row's version SHALL NOT ship. The step it serves
SHALL be recorded `gap`, with a note naming the missing capture, when other evidence for that step
exists, and `unverified` otherwise. The text of the usage-limit notice SHALL be in opencode's
`cli_unavailable_markers` only when it was captured.

#### Scenario: opencode's idle screen reads as ready under its own profile

- **GIVEN** opencode's reviewed fixture set with a `screen` case of its idle screen whose `expect`
  gives `readiness = "ready"`
- **WHEN** the case is replayed under the profile git-paw resolves for `opencode` with no user
  configuration
- **THEN** the case SHALL pass
- **AND** the resolved profile SHALL be opencode's own, not the embedded default

#### Scenario: The post-update notice reads as a dismiss-key dialog

- **GIVEN** a reviewed `screen` case of opencode's post-update notice
- **AND** a `readiness` attestation recording the key that dismissed it
- **WHEN** the case is replayed
- **THEN** the start-up dialog reading SHALL be that notice with that dismiss key

#### Scenario: The usage-limit notice reads as blocking and unavailable

- **GIVEN** a reviewed `screen` case of opencode's usage-limit notice
- **WHEN** the case is replayed
- **THEN** the start-up dialog reading SHALL be `blocking`
- **AND** the CLI-unavailable line SHALL be the notice's line

#### Scenario: An uncaptured shape ships no data

- **GIVEN** opencode's fixture set at the row's version has no reviewed capture of the usage-limit
  notice
- **WHEN** the matrix parity suite runs
- **THEN** opencode's built-in data SHALL contain no usage-limit marker or blocking dialog
- **AND** the row SHALL record the step it serves as `gap` with a note naming the missing capture,
  or as `unverified`

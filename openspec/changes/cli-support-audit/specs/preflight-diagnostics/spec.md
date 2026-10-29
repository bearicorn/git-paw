## ADDED Requirements

### Requirement: Doctor reports each detected CLI's support tier

`git paw doctor` SHALL report one check per detected CLI in the CLIs group, giving its support tier,
verified version, and verification date from the compiled support matrix. It SHALL do this without
spawning the CLI. These checks are informational (✓).

- A custom `[clis.<name>]` CLI with no matrix row SHALL be reported as not in the support matrix.
- A custom entry that overrides a matrix binary SHALL be reported with that row's tier, marked as
  applying to the built-in launch only.
- With no CLI detected, the existing ⚠ SHALL be unchanged.

Under `--json`, the report SHALL also carry a top-level `support` array with one object per detected
CLI: `binary`, `tier` (null when the CLI is not in the matrix), `verified_version`, `verified_on`,
and `custom`. This array is additive. The existing `status` and `checks` fields SHALL be unchanged.

#### Scenario: A detected matrix CLI shows its tier and version

- **GIVEN** a matrix CLI whose row derives supported at version `1.4.0` on PATH
- **WHEN** `git paw doctor` is run
- **THEN** the CLIs group SHALL carry a ✓ check for that CLI naming `supported` and `1.4.0`

#### Scenario: A custom CLI outside the matrix is labelled

- **GIVEN** a `[clis.my-agent]` entry whose command resolves on PATH
- **WHEN** `git paw doctor` is run
- **THEN** the CLIs group SHALL report `my-agent` as not in the support matrix

#### Scenario: The JSON report carries the support array

- **GIVEN** one matrix CLI and one custom CLI on PATH
- **WHEN** `git paw doctor --json` is run
- **THEN** stdout SHALL be one JSON document whose `support` array holds two objects
- **AND** the custom CLI's object SHALL have `tier` null and `custom` true
- **AND** the `status` and `checks` fields SHALL still be present

#### Scenario: Tier reporting does not run the CLI

- **GIVEN** a matrix CLI binary on PATH that records each time it is executed
- **WHEN** `git paw doctor` is run without `--live`
- **THEN** the binary SHALL NOT have been executed

### Requirement: Per-CLI live smoke arm

`git paw doctor` SHALL accept a repeatable `--cli <NAME>` option, valid only together with `--live`.
For each named CLI, one at a time and after the session-lifecycle smoke, it SHALL run a per-CLI live
smoke that:

1. resolves the binary (known or custom) and reads its version banner within a bounded timeout
2. launches it in a throwaway repository on a private terminal-backend instance, at the `manual`
   approval level, with no boot block and no prompt sent
3. runs the launch-readiness gate with the CLI's resolved profile, handling a recognised start-up
   dialog (first-run, trust, notice, or blocking) exactly as a worker launch does
4. confirms that the liveness check reads the running CLI as a live agent
5. ends the CLI and confirms that the liveness check reads it as exited
6. removes everything it created

Each outcome SHALL be a check in the Live-smoke group, named for the CLI and the step. The report
SHALL name the installed version and, when the CLI has a matrix row, the verified version.

- **✗** when a named CLI is not detected, a launch never starts, a dialog cannot be resolved, or a
  running CLI reads as not alive.
- **⚠** when a non-generic profile does not recognise readiness, or when the CLI cannot be made to
  exit without killing its surface.
- **✓ (informational)** when readiness runs under the generic profile; the check reports readiness
  as not detectable.

git-paw itself SHALL write only inside `.git-paw/tmp/`. The `--help` text and the docs SHALL state
that the CLI under test runs with the invoking user's environment and may write its own state.

`--cli` without `--live` SHALL be a usage error. `--live` without `--cli` SHALL behave exactly as
before and SHALL NOT spawn an AI CLI.

#### Scenario: A CLI that reaches readiness passes

- **GIVEN** a stub CLI on PATH, configured with a prompt-profile readiness marker that it prints on
  start, and exiting on end-of-input
- **WHEN** `git paw doctor --live --cli stub` is run
- **THEN** the Live-smoke group SHALL report ✓ for the stub's launch, readiness, liveness, and exit
  checks
- **AND** no surface, socket, or directory created by the smoke SHALL remain afterwards

#### Scenario: A named CLI that is not installed fails

- **WHEN** `git paw doctor --live --cli not-installed` is run
- **THEN** the Live-smoke group SHALL report ✗ for `not-installed` with a remedy
- **AND** the process SHALL exit non-zero

#### Scenario: An unresolvable first-run dialog fails

- **GIVEN** a stub CLI that shows a recognised first-run dialog and never becomes ready after it is
  answered
- **WHEN** `git paw doctor --live --cli stub` is run
- **THEN** the stub's readiness check SHALL report ✗

#### Scenario: Readiness under the generic profile is informational

- **GIVEN** a stub CLI that resolves the generic profile
- **WHEN** `git paw doctor --live --cli stub` is run
- **THEN** the stub's readiness check SHALL report ✓ stating that readiness is not detectable

#### Scenario: `--cli` requires `--live`

- **WHEN** `git paw doctor --cli claude` is run
- **THEN** it SHALL fail with a usage error

#### Scenario: Plain `--live` still spawns no AI CLI

- **GIVEN** a matrix CLI binary on PATH that records each time it is executed
- **WHEN** `git paw doctor --live` is run without `--cli`
- **THEN** the binary SHALL NOT have been executed

#### Scenario: `--json --live --cli` stays parseable

- **WHEN** `git paw doctor --json --live --cli stub` is run
- **THEN** stdout SHALL be a single parseable JSON document holding the per-CLI checks and the
  installed version

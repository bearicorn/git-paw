# cold-start-ci-parity Specification

## Purpose
TBD - created by archiving change cold-start-ci-parity. Update Purpose after archive.
## Requirements
### Requirement: just smoke recipe

The justfile SHALL provide a `smoke` target that runs the
integration test suite in a cold-start environment. The target
SHALL refuse to run when a `paw-*` tmux session is detected on the
default socket, instructing the developer to kill or pause it.
The target SHALL set `TMUX=` and unset `GIT_PAW_ALLOW_LIVE_SESSION`
for the test invocation so the tests do not inherit any live tmux
or broker context.

#### Scenario: Smoke refuses with a paw-* session active

- **GIVEN** a developer machine where `tmux ls` shows a session
  matching `paw-*`
- **WHEN** the developer runs `just smoke`
- **THEN** the recipe SHALL exit non-zero with a message
  identifying the offending session and SHALL NOT run any tests

#### Scenario: Smoke runs cleanly with no paw-* session

- **GIVEN** no `paw-*` session on the default tmux socket
- **WHEN** the developer runs `just smoke`
- **THEN** the recipe SHALL run the integration tests with
  `TMUX` empty and `GIT_PAW_ALLOW_LIVE_SESSION` unset

### Requirement: just smoke-container recipe

The justfile SHALL provide a `smoke-container` target that runs
the integration tests inside a containerised Ubuntu 24.04 image
matching CI's tmux version. The recipe SHALL detect `podman` or
`docker` on PATH and SHALL fail with a clear error when neither
is present. The recipe SHALL build a `git-paw-ci` image
idempotently (skip rebuild when the image already exists locally)
and SHALL mount a named cargo cache volume so re-runs reuse
compiled dependencies.

#### Scenario: smoke-container uses podman when available

- **GIVEN** a host with `podman` on PATH
- **WHEN** the developer runs `just smoke-container`
- **THEN** the recipe SHALL invoke `podman` (not `docker`) to
  build the image and run the tests

#### Scenario: smoke-container falls back to docker

- **GIVEN** a host with `docker` on PATH and no `podman`
- **WHEN** the developer runs `just smoke-container`
- **THEN** the recipe SHALL invoke `docker` to build and run

#### Scenario: smoke-container refuses with no engine on PATH

- **GIVEN** a host with neither `podman` nor `docker` on PATH
- **WHEN** the developer runs `just smoke-container`
- **THEN** the recipe SHALL exit non-zero with a message
  instructing the developer to install either engine

#### Scenario: Image build is skipped when image already exists

- **GIVEN** the `git-paw-ci` image already exists locally
- **WHEN** the developer runs `just smoke-container`
- **THEN** the recipe SHALL skip the build step and run the
  existing image

### Requirement: just smoke-all recipe

The justfile SHALL provide a `smoke-all` target that runs
`smoke` and, on macOS hosts, additionally runs `smoke-container`.
On Linux hosts the recipe SHALL run only `smoke` (the host
matches CI's environment).

#### Scenario: smoke-all on macOS runs both layers

- **GIVEN** a macOS development host
- **WHEN** the developer runs `just smoke-all`
- **THEN** the recipe SHALL invoke `just smoke` followed by
  `just smoke-container`

#### Scenario: smoke-all on Linux runs only the host suite

- **GIVEN** a Linux development host
- **WHEN** the developer runs `just smoke-all`
- **THEN** the recipe SHALL invoke `just smoke` and SHALL NOT
  invoke the container layer

### Requirement: tmux convention-enforcement unit tests

The `tmux` module SHALL include unit tests that walk every
`new-session` command produced by the module's builders
(including `TmuxSession::command_strings`, `build_supervisor_session`,
and any future builder) and SHALL assert each command contains
both `-x` and `-y` flags. A parallel test SHALL assert each
`new-session` command contains a `-c <cwd>` argument.

#### Scenario: every_new_session_passes_x_and_y passes today

- **WHEN** the unit test suite runs against current source
- **THEN** the `every_new_session_passes_x_and_y` test SHALL
  pass

#### Scenario: missing -x on a new builder fails the test

- **GIVEN** a hypothetical new builder that constructs a
  `new-session` argv without `-x`
- **WHEN** the unit test suite runs
- **THEN** the `every_new_session_passes_x_and_y` test SHALL
  fail and SHALL identify the offending builder

#### Scenario: missing -c on a new builder fails the test

- **GIVEN** a hypothetical new builder that constructs a
  `new-session` argv without `-c`
- **WHEN** the unit test suite runs
- **THEN** the `every_new_session_passes_c` test SHALL fail and
  SHALL identify the offending builder

### Requirement: sweep.sh convention-enforcement test

The test suite SHALL include a test that reads
`assets/scripts/sweep.sh` and asserts no stdin-claiming pipe
pattern (`python3 - <<`, `sh - <<`, or equivalent) remains in
the file. This pins the v0-5-0-audit-cleanup §10 fix against
regression.

#### Scenario: Current sweep.sh passes the convention test

- **WHEN** the convention test runs against the current
  `sweep.sh`
- **THEN** the test SHALL pass

#### Scenario: A reintroduced python3 - << pattern fails the test

- **GIVEN** a `sweep.sh` edit that adds a `python3 - <<EOF` block
- **WHEN** the convention test runs
- **THEN** the test SHALL fail and SHALL identify the offending
  line

### Requirement: CI matrix expansion

The project CI workflow SHALL include a headless Linux job and a
macOS job in addition to the existing Linux runner. The headless
job SHALL set `TERM=dumb` and SHALL ensure `TMUX` is unset for
the test invocation. The macOS job SHALL run on a runner shape
equivalent to the cargo-dist macOS release-build runner.

#### Scenario: Headless Linux job runs the test suite with TERM=dumb

- **WHEN** the project CI runs against a pushed branch
- **THEN** the workflow SHALL include a job that runs the test
  suite with `TERM=dumb` and `TMUX` unset

#### Scenario: macOS job runs cargo test

- **WHEN** the project CI runs against a pushed branch
- **THEN** the workflow SHALL include a macOS job that runs
  `cargo test --workspace`

### Requirement: CONTRIBUTING workflow rule

The CONTRIBUTING.md document SHALL include a section instructing
contributors to run `just smoke-all` before pushing branches that
touch tmux, broker, or worktree code. The section SHALL describe
how to pause or kill a dogfood `paw-git-paw` session before
running the recipes.

#### Scenario: CONTRIBUTING describes the smoke-before-push rule

- **WHEN** a contributor reads CONTRIBUTING.md
- **THEN** they SHALL find an explicit instruction to run
  `just smoke-all` (or `just smoke` on Linux) before pushing
  changes to tmux/broker/worktree code, with the dogfood-session
  cleanup steps included


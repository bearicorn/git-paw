# lang-agnostic-skills (delta)

## ADDED Requirements

### Requirement: Bundled skills nudge against exit-code-probe wrappers

The bundled supervisor and coordination skills SHALL include guidance
instructing agents to run dev commands **bare** and read the process exit
status directly, rather than wrapping commands in exit-code-probe shells
such as `<cmd> && echo "EXIT $?"`, `<cmd>; echo $?`, or `RC=$?; …`.

The guidance SHALL explain the rationale: the probe text varies per
invocation, which defeats the CLI's command-string permission whitelisting
and forces a fresh permission prompt every run — whereas a bare,
prefix-matchable command is approved once and generalises across runs.

The guidance SHALL be authored as **stack-neutral prose** and SHALL NOT
name a specific implementation language or toolchain, so that it passes the
existing no-language-leak audit (per the "No language-leak audit" and
"Tone and example discipline in bundled skills" requirements).

#### Scenario: Supervisor skill contains the no-exit-probe guidance

- **WHEN** the bundled supervisor skill body is inspected
- **THEN** it SHALL contain guidance directing agents to run dev commands
  bare and read the exit status directly
- **AND** it SHALL contain the rationale that an exit-code-probe wrapper
  varies per run and defeats command-string permission whitelisting

#### Scenario: The nudge is stack-neutral and passes the no-leak audit

- **WHEN** the no-language-leak audit renders the supervisor and
  coordination skills for each spec backend
- **THEN** the exit-probe-nudge prose SHALL NOT introduce any forbidden
  stack-specific token (e.g. `cargo`, `rustc`, `Cargo.toml`) outside the
  explicitly-allowed allowlist-prose span
- **AND** the audit SHALL pass

#### Scenario: Guidance discourages the probe shape, not exit-status reading

- **GIVEN** the bundled supervisor or coordination skill guidance on dev
  commands
- **WHEN** an agent follows the guidance
- **THEN** the guidance SHALL direct the agent to run the command without an
  appended `echo "… $?"` / `$?`-printing wrapper
- **AND** SHALL NOT discourage the agent from observing or acting on the
  command's actual exit status

## ADDED Requirements

### Requirement: Usage is a provider facet with an unknown fallback

A provider SHALL be able to declare how its CLI reports a session's token usage — input, output,
cache-read and cache-write tokens, and a cost figure only when the CLI itself reports one. The
`claude` family SHALL declare a usage source that locates a worker's report through the session id
git-paw assigned at launch; the location and format SHALL be verified against the installed CLI at
implementation time, and if they cannot be verified the family SHALL declare none. Every other
family, and any session without an assigned id, SHALL resolve usage as unknown with a stated reason.
Turning a report's text into figures SHALL be a pure function. A report that cannot be parsed SHALL
yield unknown, never a partial or guessed figure. Only the usage figures SHALL be read into memory;
no other report content SHALL be logged or stored.

#### Scenario: A known report yields figures

- **GIVEN** a recorded Claude-family usage report for a session
- **WHEN** it is parsed
- **THEN** the input, output, cache-read and cache-write token totals SHALL equal the sums in the
  report

#### Scenario: A generic CLI's usage is unknown

- **GIVEN** a worker running a `generic`-family CLI
- **WHEN** its usage is resolved
- **THEN** the result SHALL be unknown with a reason naming the CLI

#### Scenario: A malformed report yields unknown

- **GIVEN** a Claude-family report whose usage records cannot be parsed
- **WHEN** it is parsed
- **THEN** the result SHALL be unknown, not a partial total

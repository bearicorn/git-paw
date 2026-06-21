## MODIFIED Requirements

### Requirement: Error-shape recognition

The skill SHALL describe the visible symptoms of an API
stream timeout (mid-stream cutoff, transport error in the CLI
output, or equivalent) so the supervisor LLM names the failure
rather than continuing in silence. The phrasing SHALL be
generic enough to apply across CLI variants (claude,
claude-oss, future entries). The skill SHALL distinguish two
cases: the supervisor's OWN stream timeout (recovered via the
checkpoint/replay flow below) and a CODING AGENT's stream
timeout observed in that agent's pane, which the supervisor
detects via `.git-paw/scripts/sweep.sh detect-stuck` and which
surfaces as a synthetic `agent.status` with
`phase: "stuck-stream-timeout"`. A coding agent in
`stuck-stream-timeout` SHALL be flagged for recovery (nudge or
restart) rather than left stalled.

#### Scenario: Symptoms are named generically across CLIs

- **WHEN** the error-shape subsection is read
- **THEN** the prose SHALL describe at least two visible
  symptom patterns (e.g. "mid-stream cutoff" and "transport
  error / stream error in the CLI output") and SHALL NOT name
  a specific CLI's exact error string

#### Scenario: Coding-agent stream timeout is a detected, recoverable state

- **WHEN** the error-shape subsection is read
- **THEN** the prose SHALL state that a coding agent whose
  pane shows a stream-timeout / transport-error marker is
  detected by `sweep.sh detect-stuck` and surfaced as
  `phase: "stuck-stream-timeout"`, and that such an agent
  SHALL be flagged for recovery rather than treated as
  progressing

## ADDED Requirements

### Requirement: N re-verify cycles is not a stall

The bundled supervisor skill SHALL state explicitly that
multiple feedback→fix→re-verify cycles per agent are normal
progress, not a stuck state. The skill SHALL teach the
supervisor that an agent which is "not yet verified after N
cycles" (observed examples: mcp-server took 7 cycles,
dev-allowlist took 6) SHALL NOT be flagged, nudged, or wound
down on the cycle count alone. The supervisor SHALL judge
stall by the detected stuck shapes (stuck-on-prompt,
stuck-stream-timeout, context-bloat, no-progress,
blocked-on-supervisor) — never by how many verify rounds an
agent has consumed.

#### Scenario: Skill prose states re-verify cycles are normal

- **WHEN** the supervisor skill is inspected
- **THEN** the prose SHALL state that multiple
  feedback→fix→re-verify cycles per agent are normal progress
  and SHALL cite that real agents have taken 6–7 cycles

#### Scenario: Cycle count alone SHALL NOT trigger a stall verdict

- **WHEN** the same prose is read
- **THEN** it SHALL state that "not yet verified after N
  cycles" SHALL NOT by itself cause the supervisor to flag,
  nudge, or wind down the agent, and that stall judgement uses
  the detected stuck shapes instead

## ADDED Requirements

### Requirement: Configurable context-bloat threshold

The system SHALL expose a configurable context-bloat token
threshold so the supervisor can flag a context-bloated agent
proactively. The threshold SHALL default to ~250 (thousand
tokens, matching the observed v0.8.0 freeze point) when unset.
The threshold SHALL be readable from `[supervisor]` config by
the bundled `sweep.sh` helper; when the field is absent the
helper SHALL fall back to the documented default. The
threshold SHALL NOT change the existing agent-facing
"Context budget" coordination prose, which remains a heuristic
in prose form.

#### Scenario: Threshold defaults when unset

- **GIVEN** a config with no context-bloat threshold field set
- **WHEN** the helper resolves the threshold
- **THEN** the helper SHALL use the documented default (~250k
  tokens)

#### Scenario: Configured threshold is honoured

- **GIVEN** a `[supervisor]` config setting the context-bloat
  threshold to a non-default value
- **WHEN** the helper resolves the threshold
- **THEN** the helper SHALL use the configured value rather
  than the default

### Requirement: Context-bloat is flagged proactively from the clear hint

The `sweep.sh` detector SHALL flag an agent as `context-bloat`
when its pane shows a `/clear to save <N>k tokens` hint (or
equivalent CLI clear/compact token hint) where `N` meets or
exceeds the configured context-bloat threshold. The detection
SHALL be proactive: the agent is still responsive at the
threshold, so the flag exists to let the supervisor (or the
unattended drive loop) pre-empt the eventual freeze rather
than waiting for the agent to stall. On detection the helper
SHALL publish a synthetic `agent.status` with
`phase: "context-bloat"` and a `detail` object recording the
parsed token figure.

#### Scenario: Token hint past the threshold is flagged

- **GIVEN** an agent whose pane shows `/clear to save 250k
  tokens` (or higher) AND a configured threshold of 250
- **WHEN** the next sweep evaluates the agent
- **THEN** the detector SHALL classify the agent as
  `context-bloat` and publish `phase: "context-bloat"` with
  the parsed token figure in `detail`

#### Scenario: Token hint below the threshold is not flagged

- **GIVEN** an agent whose pane shows a clear/compact token
  hint below the configured threshold
- **WHEN** the sweep evaluates the agent
- **THEN** the detector SHALL NOT classify the agent as
  `context-bloat`

### Requirement: Coordination skill notes proactive bloat flagging

The bundled `coordination.md` "Context budget" section SHALL
note that the supervisor proactively flags an agent whose
context bloats past the configured threshold (surfaced as
`phase: "context-bloat"`), and SHALL reinforce that the agent
SHOULD commit or publish an `agent.artifact` before clearing —
consistent with the existing commit-before-compact discipline.

#### Scenario: Coordination prose references proactive bloat flagging

- **WHEN** the "Context budget" section of coordination.md is
  read
- **THEN** the prose SHALL mention that the supervisor flags
  context bloat past the threshold and SHALL tie the agent's
  response back to the commit-before-compact discipline

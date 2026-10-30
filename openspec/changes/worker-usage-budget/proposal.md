## Why

The tiered-model thesis — a capable orchestrator directing cheap workers — has no economics on
screen. git-paw records nothing about what a run costs: no token counts, no cost, no cap (the only
token-related signal in the code is the screen-scraped context-bloat hint). An unattended fleet
driven from a phone can therefore spend without bound, which is the spend-side counterpart of the
blast radius the v0.15.0 sandbox bounds.

v0.17.0 gave every Claude-family worker a known session id (`WorktreeEntry.cli_session_id`,
assigned with `--session-id`), and `cli-provider-record` gives every CLI a provider record. Together
they make usage attributable per worker: the provider knows where its CLI reports usage, and the
session id says which report belongs to which worker.

After v0.18.0 the "orchestrator" is the operator's own session outside git-paw plus the headless
one-shots the drive loop launches. git-paw can attribute the one-shots; it cannot see the operator's
session, and says so rather than pretending.

## What Changes

- **A usage facet per provider.** A provider may declare where its CLI reports a session's token
  usage (and cost, if the CLI reports one). The Claude family declares one, located through the
  worker's session id and verified against the installed CLI at implementation time; every other
  family resolves "unknown". Parsing is a pure function, so recorded usage reports can be replayed.
- **Attribution by role.** Usage is attributed per worker (and so per `[clis.*]` entry — the model
  tier) and to the drive loop's headless orchestrator one-shots. The operator's own session is never
  included, and output says so.
- **Visibility.** `git paw status --json` gains each worker's `usage` (or `null`) and a session
  total; the drive loop's exit summary gains a usage section. Human `git paw status` is unchanged.
- **Budget guardrail.** Optional `[supervisor] budget_max_tokens` / `budget_max_cost`. On breach the
  drive loop escalates once and stops every spend it initiates (nudges, re-engagement, correction
  cycles, one-shots); with `[supervisor.correction] on_exhausted =
  "abandon"` it also exits with outcome `budget-exhausted`. It never interrupts or kills a worker.
  A cap it cannot evaluate (unknown usage, or a cost cap with no cost source) is reported at start.
- **Orchestrator skill.** The exported orchestrator skill gains a budget step: read the usage from
  `git paw status --json`, stop opening work on a breach, and tell the operator.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `cli-provider`: the usage facet with an unknown fallback (added to the capability
  `cli-provider-record` introduces).
- `session-state`: status JSON exposes attributed usage.
- `supervisor-config`: the two budget fields.
- `supervisor-unattended-operation`: the budget guardrail; *Exit summary* gains the
  `budget-exhausted` outcome and the usage section.
- `skill-agent`: the supervisor (orchestrator) skill carries the budget procedure.

## Impact

- **Code:** `src/agent_cli/` (usage facet; pure parser), `src/commands/status.rs` (JSON fields),
  `src/config/supervisor.rs` (`budget_max_tokens`, `budget_max_cost`), `src/supervisor/drive.rs`
  (periodic usage read, breach handling, `DriveOutcome::BudgetExhausted`, summary section),
  the v0.18.0 escalation one-shot (assign a session id when the provider has a template, keep the
  ids for attribution), `assets/agent-skills/` orchestrator skill.
- **Behaviour:** unchanged unless a budget field is set, apart from the additive JSON fields and the
  summary's usage section.
- **Enum-variant ripple:** none for `BrokerMessage` (the breach is an existing escalation review
  item) or `SpecBackendKind`. `DriveOutcome` gains a variant (private to the drive loop).
- **Security / safety:** read-only access to the CLI's own usage reports for sessions git-paw
  started; no content of those reports beyond the usage figures is read into memory or logs. The
  guardrail only removes actions (no new keystroke, no kill). Export-agnosticism: the skill step
  names no CLI or vendor.
- **Cross-version dependencies:** requires `cli-provider-record`; uses v0.17.0
  `worker-session-id-capture` (shipped: session ids) and `classifier-retirement` (the loop answers
  no permission prompt, so the guardrail has no approval path to stop), and v0.18.0
  `orchestrator-escalation-handoff` (one-shots) and `orchestrator-skill-export` (the skill).
  **`Exit summary` is also modified by v0.18.0 `supervisor-pane-removal`** (surface wording, the
  permission-prompt scenario); this change archives after it and its delta must be rebased onto that
  text before archiving. v0.18.0 `orchestrator-escalation-handoff` adds a separate requirement
  (*Exit summary reports headless invocation outcomes*) and does not touch this one.
- **Dependencies:** none.

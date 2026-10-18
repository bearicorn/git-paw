## ADDED Requirements

### Requirement: Drive-loop decisions are pure functions with effects returned as data

The drive loop SHALL be structured as observe → decide → apply. Its prompt decision (CLI unavailability, the live-prompt gate, the buffered-input case, escalation dedup, and the escalation's manual-decision record, broker escalation and friction learning), its message decision (over the broker messages observed since the loop's cursor: gate-failure feedback marking the correction state, merge events triggering the branch refresh, and a coding agent's `agent.question` or merge-candidate `agent.artifact` outside wind-down returned as a judgment call naming the agent and the question or merge status), its nudge decision (correction re-engagement, buffered-input submission and nudge recovery), and its exit-summary decision SHALL each be a pure function that takes the current decision state and the observed inputs — the surface's resolved role, its captured tail, the worker's broker status, the observed broker messages, the resolved prompt profile and the loop's configuration, each branch's merge state as reported by git, and the current time — and returns the next state together with an ordered list of effects drawn from a closed effect enum. The message decision SHALL consider a message other than a merge event only when its sender is one of the session's coding agents, so the loop's own escalations never feed back into it; noting a judgment call SHALL change no surface, broker message or file (it is recorded in the effect log only). The heartbeat decisions that already are pure functions (`heartbeat_due`, `tick_made_progress`) SHALL remain pure and SHALL be called by the shell. These functions SHALL perform no I/O and SHALL NOT read a port, the clock, git or the broker. No decision SHALL return an effect that sends a keystroke answering a permission prompt; a live permission prompt yields only escalation effects (per `approval-auto`, *Approval keystrokes are sent only by an explicit approve action*). The imperative shell SHALL gather the observations, call the decisions, apply each returned effect in order through the existing ports (with the per-surface claim executed by the shell), and append every applied effect with its outcome to an ordered effect log returned with the run's summary. The refactor SHALL be behavior-preserving: the keystrokes sent, their order, the manual-decision records written, the broker messages published, the learnings recorded and the summary text SHALL be identical to the behavior before this requirement.

#### Scenario: A prompt decision is tested with data alone

- **GIVEN** a captured tail ending in a live permission prompt for `ls -la`, a coding-agent role and an empty dedup state
- **WHEN** the prompt decision is called
- **THEN** it SHALL return, in order, effects that record a manual decision for that agent, publish a `permission-prompt` escalation naming the agent and `ls -la`, and record a `tooling_friction` learning
- **AND** it SHALL return no keystroke effect for that surface
- **AND** the test SHALL need no fake terminal, no fake broker and no fake clock

#### Scenario: A repeated prompt within the dedup window yields no effect

- **GIVEN** a dedup state that already holds the escalation key for an agent's live prompt recorded one minute earlier
- **WHEN** the prompt decision is called with the same captured prompt and a time inside the window
- **THEN** it SHALL return no effects and an unchanged dedup state

#### Scenario: A repeated CLI-unavailable banner within the dedup window yields no effect

- **GIVEN** a dedup state that already holds the CLI-unavailable key for an agent and the matched marker, recorded one minute earlier
- **WHEN** the prompt decision is called with a captured tail still showing the same banner and a time inside the window
- **THEN** it SHALL return no effects and an unchanged dedup state

#### Scenario: A question and a merge candidate are judgment calls in the message decision

- **GIVEN** observed broker messages: an `agent.question` from coding agent `feat-a` asking "which config file?" and an `agent.artifact` with status `committed` from coding agent `feat-b`, with the loop not winding down
- **WHEN** the message decision is called
- **THEN** it SHALL return, in order, a judgment call for `feat-a` carrying the question and a judgment call for `feat-b` carrying the merge status `committed`
- **AND** it SHALL return no keystroke effect and no broker-publish effect for either
- **AND** the test SHALL need no fake terminal, no fake broker and no fake clock

#### Scenario: Messages from non-coding senders yield nothing

- **GIVEN** an observed `agent.question` whose sender is not one of the session's coding agents
- **WHEN** the message decision is called
- **THEN** it SHALL return no effects and an unchanged correction state

#### Scenario: The exit summary takes merge truth as an input

- **GIVEN** a final loop state in which every worker reached `verified`, and an observed merge state reporting one branch as not an ancestor of the default branch
- **WHEN** the summary decision is called
- **THEN** the summary SHALL NOT report the wave `completed`
- **AND** SHALL carry an `unmerged` escalation for that branch

#### Scenario: Effects are applied in order and logged

- **GIVEN** a sweep whose prompt decision returns a manual-decision record, an escalation and a friction learning
- **WHEN** the shell applies them
- **THEN** the manual decision SHALL be recorded before the escalation is published, and the escalation published before the learning is recorded
- **AND** the effect log SHALL list the three effects in that order with their outcomes

#### Scenario: Decisions perform no I/O

- **WHEN** a source audit inspects the drive-loop decision module
- **THEN** it SHALL contain no use of the terminal port, the broker client, `std::process`, `std::fs`, `std::time::Instant::now` or `SystemTime::now`

#### Scenario: Drive-loop behavior is unchanged

- **GIVEN** the functional-core refactor
- **WHEN** the existing drive-loop unit tests and the unattended-drive e2e suite run without modification
- **THEN** they SHALL pass

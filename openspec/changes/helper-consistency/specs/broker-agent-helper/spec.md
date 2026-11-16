## MODIFIED Requirements

### Requirement: sweep.sh approve refuses pane 0

`sweep.sh approve <target>` SHALL accept an agent id or a pane index and SHALL send keys only to a pane the discovery roster lists as a coding agent's `pane_index`, after resolving an id to its pane. Any other pane SHALL get no keys and a report that it is not a coding agent; with no readable roster, pane 0 SHALL be refused. A target that is neither digits nor a resolvable roster agent id SHALL be refused with a non-zero exit and no keys, and never matched as a pattern.

#### Scenario: approve of the dashboard pane is rejected

- **GIVEN** a broker-enabled session whose discovery roster lists coding agents at panes 1 and 2
- **WHEN** `sweep.sh approve 0` is invoked
- **THEN** it SHALL send NO keystrokes to pane 0
- **AND** it SHALL report that pane 0 is excluded from blind send-keys because it is not a coding agent

#### Scenario: approve of a pane no roster entry claims is rejected

- **GIVEN** a discovery roster listing coding agents at panes 1 and 2
- **WHEN** `sweep.sh approve 5` is invoked
- **THEN** it SHALL send NO keystrokes

#### Scenario: a non-numeric pane argument is refused

- **GIVEN** a discovery roster listing coding agents at panes 1 and 2
- **WHEN** `sweep.sh approve .` or `sweep.sh approve '[0-2]'` is invoked
- **THEN** it SHALL exit non-zero
- **AND** it SHALL send NO keystrokes to any pane

#### Scenario: approve of a roster agent's pane proceeds

- **GIVEN** a discovery roster listing a coding agent at pane 1 showing a live permission prompt whose option `1` is a one-time "Yes"
- **WHEN** `sweep.sh approve 1` is invoked
- **THEN** it SHALL send `1` and then `Enter` to pane 1

#### Scenario: Without a roster pane 0 is still refused

- **GIVEN** no readable discovery roster for the session
- **WHEN** `sweep.sh approve 0` is invoked
- **THEN** it SHALL send NO keystrokes to pane 0

#### Scenario: approve by agent id reaches that agent's pane

- **GIVEN** roster agent `feat-x` whose worktree owns pane 2, showing a live permission prompt whose option `1` is a one-time "Yes"
- **WHEN** `sweep.sh approve feat-x` is invoked
- **THEN** it SHALL send `1` and then `Enter` to pane 2
- **AND** its result line SHALL name `feat-x` and pane 2

#### Scenario: approve of an unknown agent id sends nothing

- **GIVEN** no roster agent `feat-zzz`
- **WHEN** `sweep.sh approve feat-zzz` is invoked
- **THEN** it SHALL exit non-zero and send NO keystrokes

## ADDED Requirements

### Requirement: An agent id resolves to the pane its worktree owns

The bundled helper SHALL resolve an agent id to a pane, and a pane to an agent id, through the binary, using the same owner rule `git paw __send` uses: the live coding-agent surface whose working directory lies in the agent's worktree. It SHALL NOT target keys at a pane index taken from the roster's recorded `pane_index` alone. A digits-only argument SHALL be read as a pane index and any other argument as an agent id.

#### Scenario: An id resolves after panes were renumbered

- **GIVEN** a session where `remove` and `add` moved agent `feat-x` from pane 3 to pane 2
- **WHEN** the helper resolves `feat-x`
- **THEN** it SHALL resolve pane 2

#### Scenario: A pane owned by no agent does not resolve

- **GIVEN** the dashboard pane
- **WHEN** the helper resolves that pane index to an agent
- **THEN** it SHALL report that the pane belongs to no coding agent

### Requirement: sweep.sh capture accepts an agent id

`sweep.sh capture <target>` SHALL accept an agent id, captured from the pane that agent's worktree owns, or a pane index, captured as before. A target that is neither digits nor a resolvable agent id SHALL be refused with a non-zero exit and SHALL NOT reach the multiplexer as a target.

#### Scenario: capture by agent id

- **GIVEN** roster agent `feat-x` whose worktree owns pane 2
- **WHEN** `sweep.sh capture feat-x` is invoked
- **THEN** it SHALL print pane 2's recent lines

#### Scenario: capture by pane index is unchanged

- **WHEN** `sweep.sh capture 0` is invoked
- **THEN** it SHALL print pane 0's recent lines

#### Scenario: A pattern target is refused

- **WHEN** `sweep.sh capture .` is invoked
- **THEN** it SHALL exit non-zero and capture no pane

### Requirement: sweep.sh tell accepts a pane index

`sweep.sh tell <target>` SHALL accept a pane index as well as an agent id. A pane index SHALL be resolved to the agent whose worktree owns that pane, and the message SHALL be sent to that agent id. A pane that belongs to no coding agent SHALL be refused with exit `3` and no keystroke.

#### Scenario: tell by pane index reaches the owning agent

- **GIVEN** an idle roster agent `feat-x` whose worktree owns pane 2
- **WHEN** a message is told to `2`
- **THEN** the message SHALL be delivered to `feat-x`, and the result SHALL print `delivered` and exit `0`

#### Scenario: tell to the dashboard pane is refused

- **WHEN** a message is told to the dashboard's pane index
- **THEN** it SHALL print a `refused:` line, exit `3` and send no keystroke

### Requirement: A turn that started counts as delivered

`git paw __send` SHALL report `delivered` when, after the `Enter`, the pane that was idle before the send shows a mid-response marker from the agent's profile, even if the message text does not appear as a new line. Before reporting `wedged` for an emptied input, it SHALL re-read the pane a bounded number of times, one settle delay apart. An emptied input with no turn evidence and no mid-response marker after those reads SHALL still be `wedged`.

#### Scenario: A reflowed echo of a started turn is delivered

- **GIVEN** an idle agent whose CLI echoes a submitted turn without the message's first characters on one line
- **WHEN** a message is told to it and its pane then shows a mid-response marker
- **THEN** it SHALL print `delivered` and exit `0`

#### Scenario: A late echo is delivered

- **GIVEN** an agent whose submitted turn first appears on the second read after the `Enter`
- **WHEN** a message is told to it
- **THEN** it SHALL print `delivered`

#### Scenario: An emptied input with no turn is still wedged

- **GIVEN** a pane whose input empties after the `Enter` but which shows no new turn and no mid-response marker on any read
- **WHEN** a message is told to it
- **THEN** it SHALL print `wedged` and exit `2`

### Requirement: sweep.sh status lists roster agents the broker has not seen

`sweep.sh status` SHALL print a row for every coding agent in the discovery roster. An agent the broker's `/status` does not list yet SHALL be shown with status `not-seen` and no age, after the broker's rows. Agents the broker lists but the roster does not SHALL still be reported as phantoms.

#### Scenario: A booting worker is shown

- **GIVEN** a discovery roster with `feat-a` and `feat-b`, and a broker `/status` listing only `feat-a`
- **WHEN** `sweep.sh status` is invoked
- **THEN** it SHALL show a row for `feat-a` with its broker status
- **AND** a row for `feat-b` with status `not-seen`

#### Scenario: Without a roster nothing is added

- **GIVEN** no discovery file and a broker `/status` listing `feat-a`
- **WHEN** `sweep.sh status` is invoked
- **THEN** it SHALL show only the broker's rows

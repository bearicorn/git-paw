## ADDED Requirements

### Requirement: Supervisor skill — permission decisions belong to the agent CLI and the human

The embedded supervisor skill SHALL document who decides a worker's permission prompt, in this order:

1. The worker's CLI-native permission system runs first: its resolved permission mode and its own allow rules. git-paw adds no rules of its own.
2. Any prompt the CLI raises is relayed to the human. Under `--unattended` the drive loop escalates it to the supervisor's pane and the broker. Otherwise the supervisor's own sweep finds it.
3. The human decides. The supervisor carries out an approval with `sweep.sh approve <pane>` only on the human's explicit instruction.

The skill SHALL state that `sweep.sh approve` answers a prompt once (option 1) and never grants a durable "don't ask again". It SHALL state that a prompt which should not recur belongs in the agent CLI's own allow rules, configured by the user. The skill SHALL NOT describe any command, command family or helper path as safe to approve, SHALL NOT reference a git-paw command classifier, and SHALL NOT enumerate any toolchain's verbs as approvable.

#### Scenario: The supervisor skill documents the permission ladder

- **WHEN** the supervisor skill is rendered
- **THEN** it SHALL describe the order: the CLI's own permission system, then relay to the human, then an explicit `sweep.sh approve` on the human's instruction
- **AND** it SHALL state that `sweep.sh approve` answers once and never grants "don't ask again"

#### Scenario: The supervisor skill names no command as safe

- **WHEN** the rendered supervisor skill is inspected
- **THEN** it SHALL NOT contain `git paw __classify`, a dev-command allowlist enumeration, or an instruction to absorb routine approvals
- **AND** it SHALL NOT instruct the supervisor to select a "don't ask again" option

## MODIFIED Requirements

### Requirement: Supervisor skill — proactive permission-prompt handling at launch

The embedded `supervisor.md` skill SHALL include explicit guidance for the supervisor agent's initial launch-time monitoring sweep. The guidance SHALL:

1. State that immediately after attaching to the session, the supervisor SHALL inspect every coding-agent pane via `tmux capture-pane` (through `sweep.sh snapshot` / `sweep.sh capture <pane>`).
2. Classify what each pane is showing into the categories:
   - `paste-buffer state` → apply the paste-buffer recovery sub-case (above).
   - `permission prompt` (e.g. `This command requires approval`, `Do you want to proceed?`) → relay the prompt to the human, naming the agent and the pending command, and approve it with `sweep.sh approve <pane>` only on the human's explicit instruction.
   - `working / esc to interrupt` → leave alone, agent is doing its thing.
   - `idle / ? for shortcuts` → agent has finished or never started; investigate.
3. State that the same handling applies on every later sweep, not only at launch. No git-paw component approves prompts in the background, so the supervisor's sweep (or, under `--unattended`, the drive loop's escalation) is how a prompt reaches the human.

#### Scenario: Supervisor skill mentions proactive launch-time permission sweep

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or section instructing the supervisor to inspect every pane immediately after attaching (e.g. "launch-time sweep", "initial pane inspection", or equivalent)

#### Scenario: Supervisor skill enumerates the four pane categories

- **WHEN** the launch-time sweep section is inspected
- **THEN** it identifies the four pane categories: paste-buffer state, permission prompt, working, and idle
- **AND** it maps each category to a default action

#### Scenario: Supervisor skill relays permission prompts to the human

- **WHEN** the launch-time sweep section is inspected
- **THEN** it instructs the supervisor to relay every permission prompt to the human, naming the agent and the pending command
- **AND** it instructs the supervisor to run `sweep.sh approve <pane>` only on the human's explicit instruction

#### Scenario: Supervisor skill does not rely on a background approver

- **WHEN** the launch-time sweep section is inspected
- **THEN** it SHALL NOT reference an `[supervisor.auto_approve]` background poll thread
- **AND** it SHALL state that no git-paw component approves prompts in the background

## REMOVED Requirements

### Requirement: Supervisor skill — continuous-sweep absorption doctrine

**Reason**: The doctrine made the supervisor LLM a rubber-stamp approver of "routine" commands. That is a pattern classifier exposed to prompt injection, and it is what this change retires.
**Migration**: The supervisor relays prompts to the human on every sweep (*Supervisor skill — proactive permission-prompt handling at launch*, item 3, and *Supervisor skill — permission decisions belong to the agent CLI and the human*).

### Requirement: Supervisor skill — tiered permission model and safe-command policy

**Reason**: Its middle tier was `git paw __classify`, which is removed. Its policy classes described the retired classifier.
**Migration**: Replaced by *Supervisor skill — permission decisions belong to the agent CLI and the human*.

### Requirement: Skill-content tests SHALL cover all rendered subsections named in archived prompt-submit-fix scenarios

**Reason**: A bookkeeping requirement whose third item pins the removed `[supervisor.auto_approve]` poll-thread cross-reference and whose second item pins safe-command pattern matching.
**Migration**: The scenarios of *Supervisor skill — proactive permission-prompt handling at launch* carry the test mapping. The launch-sweep test (`supervisor_skill_documents_proactive_launch_sweep`) is kept, and the escalation and poll-thread tests are rewritten to the modified scenarios.

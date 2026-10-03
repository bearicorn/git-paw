## ADDED Requirements

### Requirement: git-paw makes no safety decision about an agent's command

The system SHALL NOT decide whether a command an agent CLI is asking permission to run is safe, dangerous, or allowed. It SHALL keep no safe-command whitelist, no danger list, no command normalizer, and no path-based safety rule for permission prompts, and it SHALL NOT expose any such verdict through a subcommand, a helper script, or a configuration table. Whether a command runs SHALL be decided by the agent CLI's own permission system (its resolved permission mode, its own allow and deny rules, and any sandbox around it) or by the human who answers the prompt. git-paw SHALL treat every detected live permission prompt the same way, whatever the command text: it reports the prompt for a human decision (see `supervisor-unattended-operation`) and never answers it on its own.

#### Scenario: A command formerly classified safe is escalated, not approved

- **GIVEN** an `--unattended` session whose agent pane shows a live permission prompt for `ls -la`
- **WHEN** the drive loop sweeps that pane
- **THEN** the prompt SHALL be escalated for a human decision
- **AND** no keystroke SHALL be sent to that pane

#### Scenario: Command text cannot steer the outcome

- **GIVEN** live permission prompts whose commands are, in turn, a command block of `ls` followed by `rm -rf /important # [y/N]`, `git reset $'--hard' HEAD~1`, `export GIT_SSH_COMMAND=id; git fetch`, `awk 'BEGIN{system("id")}'`, and `cat ~/.ssh/id_rsa`
- **WHEN** the drive loop sweeps each pane
- **THEN** each prompt SHALL be escalated exactly as any other live prompt is
- **AND** no keystroke SHALL be sent to any of those panes

#### Scenario: No classifier subcommand exists

- **WHEN** `git paw __classify` is invoked
- **THEN** it SHALL fail as an unrecognised subcommand
- **AND** the bundled `sweep.sh` SHALL reject `classify` as an unknown subcommand

### Requirement: Prompt-free operation is attributed to the agent CLI's permission system

Documentation and bundled guidance SHALL attribute prompt-free agent operation to the agent CLI's own permission system — its resolved permission mode (the launch flags git-paw maps from the approval level) and the allow rules configured in that CLI — and SHALL NOT attribute it to a git-paw command classifier or to any permission grant git-paw writes on its own.

#### Scenario: Supervisor guidance credits the CLI's permission system

- **WHEN** the bundled supervisor skill's permission guidance is inspected
- **THEN** it SHALL attribute prompt-free operation to the agent CLI's resolved permission mode and its own allow rules
- **AND** it SHALL NOT state that a git-paw classifier approves, recognises, or allows any command

#### Scenario: User docs credit the CLI's permission system

- **WHEN** the mdBook supervisor and coordination chapters are inspected
- **THEN** they SHALL explain that a helper call or dev command runs without a prompt only when the agent CLI's own permission mode or allow rules permit it
- **AND** they SHALL NOT credit a git-paw classifier

### Requirement: CLI settings path keeps parsing and round-tripping

The system SHALL continue to accept `[clis.<name>].settings_path` and SHALL preserve it when the configuration is loaded and re-serialized, so existing configurations load unchanged after the protected-path set it fed is removed.

#### Scenario: Settings path still parses and round-trips

- **WHEN** a configuration declaring a CLI settings path is loaded and re-serialized
- **THEN** the settings path SHALL be preserved

## MODIFIED Requirements

### Requirement: Permission prompt detection via tmux capture-pane

The system SHALL detect agent CLI permission prompts by capturing pane output and matching it against the approval-prompt markers of the resolved CLI prompt profile. Detection SHALL report only whether an approval-prompt marker is present. It SHALL NOT assign the prompt a command class or any other judgement about whether the pending command may run.

#### Scenario: Approval prompt detected in working agent

- **GIVEN** a tmux pane running an agent CLI that has produced a permission prompt
- **WHEN** the pane is captured via `tmux capture-pane -p -t <session>:<pane>` and scanned
- **THEN** detection SHALL report that an approval-prompt marker is present
- **AND** SHALL report that no marker is present for a capture that contains none

#### Scenario: Detection carries no command class

- **GIVEN** two captures whose approval prompts ask about different commands, one a `curl` call and one an `rm -rf`
- **WHEN** detection runs on each
- **THEN** both SHALL produce the same result, that an approval-prompt marker is present
- **AND** neither result SHALL carry a command class

#### Scenario: Detection is non-invasive

- **GIVEN** any agent CLI (claude, aider, codex, etc.)
- **WHEN** detection runs
- **THEN** detection SHALL only read pane output and SHALL NOT modify the agent's process or input

### Requirement: Prompt markers are sourced from the resolved CLI profile

Permission-prompt detection SHALL obtain its approval markers, live-prompt markers, command-header form, file-prompt pattern, option-line form and broad-grant marker from the prompt-shape profile resolved for the CLI running in the pane being inspected, rather than from constants compiled into the detector. When no profile is configured for that CLI, the embedded default SHALL resolve, so detection outcomes are unchanged from before profiles existed.

This changes only where the literals come from. The live-prompt gate's anchoring discipline, the approval send gate, and the `sweep.sh approve` option-1 guard SHALL remain compiled logic (or its shell mirror) and SHALL NOT be altered by profile content.

#### Scenario: Detection uses the pane's CLI profile

- **GIVEN** a pane running a CLI with a configured prompt-shape profile
- **WHEN** permission-prompt detection runs against that pane
- **THEN** the markers used SHALL come from that CLI's resolved profile

#### Scenario: Unconfigured CLI detects as before

- **GIVEN** a pane running a CLI with no configured profile
- **WHEN** permission-prompt detection runs against that pane
- **THEN** the embedded default markers SHALL be used
- **AND** the detection outcome SHALL match the outcome before profiles were introduced

#### Scenario: Gates unaffected by profile sourcing

- **GIVEN** any resolved profile
- **WHEN** the live-prompt gate and the approval send gate evaluate a capture
- **THEN** the anchoring window, the trailing-region bound and the pane-0 refusal SHALL come from compiled logic
- **AND** SHALL NOT be influenced by profile content

## REMOVED Requirements

### Requirement: Whitelist of safe command classes

**Reason**: git-paw no longer decides that a command is safe. Prefix whitelists produced false-SAFE verdicts that three successive security reviews could not close (see proposal Why).
**Migration**: Pre-approve routine commands in the agent CLI's own permission rules, or run the CLI with `full-auto` inside a sandbox. Every live prompt is escalated (*git-paw makes no safety decision about an agent's command*).

### Requirement: Configurable whitelist extension

**Reason**: There is no whitelist left to extend. `[supervisor.auto_approve] safe_commands` still parses but has no effect (`supervisor-config`, *Deprecated auto-approval tables are parsed but have no effect*).
**Migration**: Move entries into the agent CLI's own allow rules.

### Requirement: Prefix matching semantics

**Reason**: Prefix matching existed only for the whitelist, and prefix grants are the false-SAFE problem.
**Migration**: None in git-paw. The agent CLI's own rule engine decides how its allow rules match.

### Requirement: Curated danger-list escalates to human

**Reason**: Every live prompt now escalates, so a list of commands that must escalate is redundant. A denylist also leaked (the `git` family, `$'…'` quoting).
**Migration**: Dangerous commands reach the human like every other prompt. Add deny rules in the agent CLI's own settings for commands that must never run.

### Requirement: Scratch-path exception for rm -rf

**Reason**: An exception to the danger list cannot outlive the danger list.
**Migration**: None. `rm -rf` prompts are escalated like every other prompt.

### Requirement: Worktree-confined dev-test commands classify safe

**Reason**: git-paw no longer classifies commands. Interpreter runs of worktree scripts execute code the agent wrote, so approving them is a decision for the CLI's permission system or the human.
**Migration**: Use the agent CLI's own allow rules, or `full-auto` inside a sandbox.

### Requirement: Operator config-path writes escalate as danger

**Reason**: The rule existed to stop the classifier from approving a write into operator configuration. With no automatic approval, such a write reaches the human like every other prompt.
**Migration**: The coordination and supervisor skills' memory-isolation doctrine stays (`core-memory-isolation`). Deny rules in the agent CLI's own settings, or a sandbox, enforce the boundary mechanically.

### Requirement: Prompt class identification

**Reason**: Permission types (`Curl`, `Git`, `SafeCommand`, `WorktreeFileOp`, `Unknown`) existed only to decide auto-approval. Detection now reports only that an approval marker is present (*Permission prompt detection via tmux capture-pane*).
**Migration**: None.

### Requirement: Capture is rate-limited

**Reason**: The stall-keyed capture schedule belonged to the deleted dashboard auto-approve thread.
**Migration**: The drive loop's capture cadence is governed by `supervisor-unattended-operation` (*Unattended drive loop runs a supervisor wave to completion*, approximately 15 seconds, one explicit capture per pane).

### Requirement: Standard preset content

**Reason**: The dev-allowlist preset fed only the whitelist and the skill prose that taught approving it. Its git verbs (`push`, `pull`, `merge` run hooks) are part of the false-SAFE problem.
**Migration**: `src/supervisor/dev_allowlist.rs` and `{{DEV_ALLOWLIST_PRESET}}` are deleted. `[supervisor.common_dev_allowlist]` still parses with no effect.

### Requirement: User-extensible allowlist via `extra` field

**Reason**: There is no preset left to extend.
**Migration**: Move `extra` entries into the agent CLI's own allow rules. The field still parses with no effect.

### Requirement: Named stack presets

**Reason**: The stack presets carried false-SAFE classes (`npm run`, `pip install`, `cargo test` all execute project code) and fed only the retired whitelist.
**Migration**: `stacks` still parses with no effect. A later feature that needs a stack declaration gets its own field.

### Requirement: Classifier normalizes a trailing exit-code probe before matching

**Reason**: There is no classifier left to normalize for.
**Migration**: None. The skills' "run dev commands bare" nudge stays (`core-lang-agnostic`), because it is about the CLI's own command-string rules.

### Requirement: Safe-command classification is exposed via a hidden __classify subcommand

**Reason**: The hidden subcommand was the IPC seam for a classifier that no longer exists.
**Migration**: `git paw __classify` and `sweep.sh classify` are removed. `sweep.sh approve` sends option `1` without consulting anything (`broker-agent-helper`, *sweep.sh approve re-confirms a live prompt before sending keys*).

### Requirement: Classifier normalizes leading assignment and wrapper prefixes before matching

**Reason**: There is no classifier left to normalize for. `export GIT_SSH_COMMAND=…` and similar prefixes were themselves a false-SAFE class.
**Migration**: None.

### Requirement: git-paw managed helper-script invocations classify as safe

**Reason**: Trust by path was a classifier rule, and an agent can rewrite a helper in its own worktree.
**Migration**: Pre-approving git-paw's own helpers is the explicit, consented `git paw setup-cli` (`cli-setup-consent`). Otherwise a helper call prompts and is escalated.

### Requirement: Writes under a repository `.git/` directory escalate as danger

**Reason**: The rule existed to stop automatic approval of `.git/` writes. With no automatic approval, such a write reaches the human like every other prompt.
**Migration**: Keep `.git/hooks` and `.git/config` read-only through the documented sandbox (`core-security-posture`, *Documented FS-scoped sandbox workflow*).

### Requirement: Prompt-free operation is attributed to the permission mode and classifier

**Reason**: It credits a classifier that no longer exists.
**Migration**: Replaced by *Prompt-free operation is attributed to the agent CLI's permission system*.

### Requirement: Settings path remains available to memory isolation

**Reason**: The memory-isolation protected-path set it fed was enforced only by the classifier and is removed (`core-memory-isolation`).
**Migration**: Replaced by *CLI settings path keeps parsing and round-tripping*.

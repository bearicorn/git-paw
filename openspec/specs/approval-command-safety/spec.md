# approval-command-safety Specification

## Purpose
Detects agent-CLI permission prompts non-invasively via rate-limited `tmux capture-pane`, classifying each stalled prompt into a fixed permission type (Curl, Cargo, Unknown, etc.), and classifies a captured command's command slice as auto-approvable or escalate-to-human by prefix-matching a configurable safe-command whitelist, while a terminal, curated per-OS danger-list (force-push, hard reset, sudo, device writes, process kills, etc.) always escalates — subject only to a scratch-path exception for `rm -rf` targeting repo/OS scratch paths — so callers can decide whether to auto-approve. The curated, stack-neutral dev-command prefix allowlist — with opt-in named stack presets (`rust`, `node`, `python`, `go`) and a user `extra` list — feeds that whitelist and the bundled skill guidance. git-paw does not write permission grants into any agent CLI's settings files (seeding was removed in v0.16.0); routine agent actions are approved by the classifier and the resolved approval flags instead.
## Requirements
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

### Requirement: No permission grants are seeded into agent CLI settings files

The system SHALL NOT automatically write permission rules, trust markers or hook configuration into any agent CLI's settings or state file, SHALL NOT automatically create a vendor-specific settings directory inside a repository or a worktree, and SHALL NOT automatically add a version-control exclusion entry for such a directory or file. Such writes SHALL happen only when the user explicitly runs `git paw setup-cli` (the `cli-setup` capability), and running it is the consent. No other command, process or interface — including `start`, `add`, `init`, session recovery, the drive loop, the dashboard and the MCP server — SHALL invoke it, apply its items, or write the setup record. Outside that command git-paw writes no permission rule of its own.

#### Scenario: No settings file is written at session start

- **GIVEN** a session starting with any agent CLI
- **WHEN** the session initialises
- **THEN** no permission grants SHALL be written into that CLI's settings file

Test: `tests/cli_setup_launch_guards.rs::starting_a_session_writes_no_settings_file_and_no_record`

#### Scenario: No vendor settings directory is created in a worktree

- **GIVEN** a worktree provisioned for any agent CLI
- **WHEN** the worktree is provisioned
- **THEN** no vendor-specific settings directory SHALL be created inside it
- **AND** no version-control exclusion entry SHALL be added for such a directory

Test: `tests/cli_setup_launch_guards.rs::starting_a_session_writes_no_settings_file_and_no_record`

#### Scenario: Pre-existing settings files are left untouched

- **GIVEN** a settings file that a previous version of git-paw wrote
- **WHEN** a session starts
- **THEN** that file SHALL be left byte-identical
- **AND** it SHALL NOT be deleted

Test: `tests/cli_setup_launch_guards.rs::a_settings_file_a_previous_version_wrote_is_left_byte_identical`

#### Scenario: Init writes no setup entry

- **GIVEN** a repository with no `.claude/` directory and no setup record
- **WHEN** `git paw init` runs
- **THEN** no `.claude/` directory and no setup record SHALL exist afterwards

Test: `tests/cli_setup_e2e.rs::init_creates_no_settings_directory_and_no_record`

#### Scenario: Earlier setup is not extended by a later launch

- **GIVEN** `git paw setup-cli claude --allowlist --yes` was applied in a repository
- **WHEN** `git paw add` creates a new worktree in that repository's session
- **THEN** no settings directory SHALL be created in the new worktree
- **AND** the main checkout's settings file and the setup record SHALL be byte-identical before and after

Test: `tests/cli_setup_launch_guards.rs::adding_a_worktree_after_setup_extends_nothing`

#### Scenario: Recovery and the drive loop write no setup entries

- **GIVEN** a repository with a setup record
- **WHEN** a stopped session is recovered and the drive loop runs to completion
- **THEN** the setup record and every file it names SHALL be byte-identical before and after

Test: `tests/cli_setup_launch_guards.rs::recovering_a_stopped_session_writes_no_setup_entries, tests/source_audit.rs::only_the_setup_cli_command_reaches_the_setup_writers, tests/source_audit.rs::no_launch_or_drive_path_invokes_the_setup_use_case`

#### Scenario: No MCP tool applies setup

- **WHEN** the tool list of `git paw mcp` is read
- **THEN** no tool SHALL invoke `git paw setup-cli`, write an agent CLI's settings file or write the setup record

Test: `mcp::server::tests::no_tool_applies_cli_setup_or_writes_cli_settings`

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


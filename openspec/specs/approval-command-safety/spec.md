# approval-command-safety Specification

## Purpose
Detects agent-CLI permission prompts non-invasively via rate-limited `tmux capture-pane`, classifying each stalled prompt into a fixed permission type (Curl, Cargo, Unknown, etc.), and classifies a captured command's command slice as auto-approvable or escalate-to-human by prefix-matching a configurable safe-command whitelist, while a terminal, curated per-OS danger-list (force-push, hard reset, sudo, device writes, process kills, etc.) always escalates — subject only to a scratch-path exception for `rm -rf` targeting repo/OS scratch paths — so callers can decide whether to auto-approve. It also seeds least-privilege command allowlists into agent CLI settings at session startup so routine agent actions run without permission prompts — covering the path-based broker-helper (curl) allowlist grant and its updates, config-driven seeding of the broker allowlist into each session CLI's configured `settings_path` (no hardcoded CLI names or paths), and the curated, stack-neutral dev-command prefix allowlist with opt-in named stack presets (`rust`, `node`, `python`, `go`) and a user `extra` list; all seeding is idempotent, dedup-preserving, per-path-once, non-fatal on failure, and applied both at the repo root and per agent worktree.

## Requirements
### Requirement: Whitelist of safe command classes

The system SHALL maintain an explicit whitelist of command prefixes that are eligible for auto-approval, and SHALL NOT auto-approve anything outside the whitelist. The effective whitelist SHALL be composed from:

1. the **built-in stack-neutral entries**: the read-mostly verb allowlist — commands whose leading verb is one of `curl`, `cat`, `ls`, `grep`, `rg`, `git`, `echo`, `sed`, `awk`, `find`, `wc`, `head`, `tail`, `jq`, `mkdir`, `touch`, `export`, `tmux`, `env` — plus `git commit` and the broker-localhost prefix `curl http://127.0.0.1:`;
2. the **resolved dev-allowlist patterns**: `effective_patterns(stacks, extra)` per `dev-command-allowlist` — the universal preset, the named stack presets selected by `[supervisor.common_dev_allowlist] stacks`, and its `extra` entries;
3. the **configured extension**: `[supervisor.auto_approve] safe_commands`.

The built-in entries SHALL NOT contain stack- or tool-specific patterns. In particular, the previously hardcoded `cargo fmt`, `cargo clippy`, `cargo test`, `cargo build`, `openspec`, and `just` SHALL NO LONGER be built in — projects receive their toolchain verbs through the resolved stack presets and/or configured extensions. The stack presets SHALL be consumed from the dev-allowlist module's exported constants (single source of truth — no duplicated pattern lists).

A whitelist match SHALL be subordinate to the danger-list: when the curated danger-list (see "Curated danger-list escalates to human") matches the same command, the command SHALL escalate regardless of any whitelist match.

The bundled `sweep.sh classify` helper SHALL compose its whitelist from the same three sources (reading the resolved stacks and extensions from `.git-paw/config.toml`) so the Rust classifier and the helper agree.

#### Scenario: Default whitelist is stack-neutral

- **GIVEN** a supervisor configuration with no stacks declared and no `safe_commands`
- **WHEN** the effective whitelist is composed
- **THEN** it SHALL NOT contain `cargo`, `openspec`, or `just` entries
- **AND** it SHALL contain the read-mostly verbs, `git commit`, and `curl http://127.0.0.1:`

#### Scenario: Declared stack contributes its toolchain verbs

- **GIVEN** `[supervisor.common_dev_allowlist] stacks = ["rust"]`
- **WHEN** classification runs against `cargo test --workspace`
- **THEN** `is_safe_command(...)` SHALL return `true` (the rust stack preset contributes `cargo test`)

#### Scenario: Undeclared stack's verbs stay unknown

- **GIVEN** `[supervisor.common_dev_allowlist] stacks = ["node"]`
- **WHEN** classification runs against `cargo test`
- **THEN** `is_safe_command(...)` SHALL return `false`
- **AND** the auto-approver SHALL NOT send approval keystrokes

#### Scenario: Read-mostly verb is whitelisted

- **GIVEN** the default supervisor configuration
- **WHEN** the captured command is `grep -rn "foo" src/`
- **THEN** the classifier SHALL treat `grep` as a read-mostly safe verb
- **AND** `is_safe_command(...)` SHALL return `true`

#### Scenario: Unknown command not in whitelist

- **GIVEN** a captured permission prompt for `someprog --do-thing`
- **WHEN** the classifier runs
- **THEN** `is_safe_command("someprog --do-thing", &whitelist)` SHALL return `false`
- **AND** the auto-approver SHALL NOT send approval keystrokes

#### Scenario: Danger match overrides a whitelist match

- **GIVEN** the captured command is `git push origin main`
- **WHEN** the classifier runs
- **THEN** although `git` is a read-mostly safe verb, the danger-list match on `git push` SHALL win
- **AND** the classifier SHALL escalate to the human rather than auto-approve

#### Scenario: sweep.sh composes the same whitelist

- **GIVEN** `[supervisor.common_dev_allowlist] stacks = ["rust"]` in `.git-paw/config.toml`
- **WHEN** `sweep.sh classify` evaluates a capture whose command slice is `cargo fmt --check`
- **THEN** its decision SHALL agree with the Rust classifier (safe)
- **AND** a list-parity guard SHALL assert the helper's built-in verb lists equal the Rust classifier's

### Requirement: Configurable whitelist extension

The whitelist SHALL be extendable by user configuration so projects can add their own safe patterns without modifying the binary.

#### Scenario: Config adds project-specific patterns

- **GIVEN** `[supervisor.auto_approve] safe_commands = ["just lint", "just test"]` in `.git-paw/config.toml`
- **WHEN** the supervisor loads its configuration
- **THEN** the effective whitelist SHALL be the union of the defaults and the configured entries

#### Scenario: Config does not weaken defaults

- **GIVEN** a config that omits `safe_commands` or sets it to `[]`
- **WHEN** the supervisor loads its configuration
- **THEN** the default whitelist SHALL still apply

### Requirement: Prefix matching semantics

The classifier SHALL use prefix matching against the captured command text so that flag variations are accepted without per-flag whitelist entries.

#### Scenario: Flag variation matches prefix

- **GIVEN** whitelist entry `cargo test`
- **WHEN** the captured command is `cargo test --no-run --workspace`
- **THEN** `is_safe_command(...)` SHALL return `true`

#### Scenario: Different program does not match

- **GIVEN** whitelist entry `cargo test`
- **WHEN** the captured command is `cargotest --foo` (no space)
- **THEN** `is_safe_command(...)` SHALL return `false`

### Requirement: Curated danger-list escalates to human

The classifier SHALL maintain a curated **danger-list** of command patterns that SHALL ALWAYS escalate to the human and SHALL NEVER be auto-approved, even when a whitelisted verb or a worktree-confined rule would otherwise match. The danger-list SHALL be evaluated before any allowlist or safe-by-pattern rule, and a danger match SHALL be a terminal escalate decision.

The classifier SHALL match the prompted **command slice** — the text between the `Bash command` / `Bash(` header and the confirmation question — NOT the surrounding supervisor narration or prose elsewhere in the capture.

The shared (OS-independent) danger base SHALL include at minimum:

- `rm -rf` / `rm -fr` (subject to the scratch-path exception below)
- `git push`, any `--force` / `force-push`, `reset --hard`, `git rebase`, branch-switching `git checkout ` (with a trailing space / argument), `branch -D`
- `git worktree remove`, `clean -fd`, `clean -fdx`
- `sudo `, `mkfs`, `dd if=`, `> /dev/`, `chmod -R`, `chown -R`
- `pkill` / `kill`

The classifier SHALL extend the shared base with a small **per-OS addendum** (macOS and Linux only; Windows is treated as WSL = Linux):

- macOS addendum: `diskutil`, deletes targeting `/Volumes/…`, `rm -rf ~/Library/…`
- Linux addendum: `mkfs*`, raw block devices `/dev/sd*`, `/dev/nvme*`

#### Scenario: Force push escalates

- **GIVEN** a live prompt whose command slice is `git push --force origin main`
- **WHEN** the classifier runs
- **THEN** the danger-list SHALL match
- **AND** the classifier SHALL escalate to the human (no auto-approval)

#### Scenario: Hard reset escalates

- **GIVEN** a live prompt whose command slice is `git reset --hard HEAD~3`
- **WHEN** the classifier runs
- **THEN** the danger-list SHALL match and the classifier SHALL escalate

#### Scenario: Branch switch escalates

- **GIVEN** a live prompt whose command slice is `git checkout main`
- **WHEN** the classifier runs
- **THEN** the branch-switching `git checkout ` pattern SHALL match and the classifier SHALL escalate

#### Scenario: Privileged and device-destroying commands escalate

- **GIVEN** a live prompt whose command slice is any of `sudo apt install x`, `dd if=/dev/zero of=disk.img`, `chmod -R 777 /etc`, or `mkfs.ext4 /dev/sda1`
- **WHEN** the classifier runs
- **THEN** each SHALL match the danger-list and the classifier SHALL escalate

#### Scenario: Process-killing commands escalate

- **GIVEN** a live prompt whose command slice is `pkill -9 node` or `kill -9 1234`
- **WHEN** the classifier runs
- **THEN** the danger-list SHALL match and the classifier SHALL escalate

#### Scenario: macOS-specific destructive command escalates on macOS

- **GIVEN** the host OS is macOS
- **AND** a live prompt whose command slice is `diskutil eraseDisk JHFS+ x /dev/disk2`
- **WHEN** the classifier runs
- **THEN** the macOS addendum SHALL match and the classifier SHALL escalate

#### Scenario: Linux-specific device write escalates on Linux

- **GIVEN** the host OS is Linux (or WSL)
- **AND** a live prompt whose command slice writes to `/dev/sda` or `/dev/nvme0n1`
- **WHEN** the classifier runs
- **THEN** the Linux addendum SHALL match and the classifier SHALL escalate

#### Scenario: Narration about a dangerous command is not classified as danger

- **GIVEN** a capture in which the supervisor prose reads "I will avoid running rm -rf /" but the live command slice is `cargo test`
- **WHEN** the classifier runs against the command slice (not the prose)
- **THEN** the danger-list SHALL NOT match and `cargo test` SHALL classify as safe

### Requirement: Scratch-path exception for rm -rf

The classifier SHALL NOT escalate an `rm -rf` / `rm -fr` command when **every** target it removes is repo or OS scratch. The recognised scratch set SHALL be: paths matching `/tmp/paw-*`, `/private/tmp/paw-*` (macOS symlinks `/tmp` to `/private/tmp`), `$TMPDIR`-rooted `paw-*`, and any path under `.git-paw/tmp/`. The exception SHALL also cover `rm -rf "$VAR"` when `$VAR` resolves (via the captured environment or a preceding `VAR=…` assignment on the same prompt) to a scratch path. When the variable cannot be resolved, or ANY target lies outside the scratch set, the command SHALL escalate (fail-safe).

#### Scenario: Scratch temp delete auto-approves

- **GIVEN** a live prompt whose command slice is `rm -rf /tmp/paw-build-123`
- **WHEN** the classifier runs
- **THEN** the scratch-path exception SHALL apply
- **AND** the classifier SHALL NOT escalate; the command SHALL classify as safe

#### Scenario: macOS /private/tmp scratch matches the whitelist

- **GIVEN** a live prompt whose command slice is `rm -rf /private/tmp/paw-cache`
- **WHEN** the classifier runs
- **THEN** the `/private/tmp/paw-*` form SHALL match the scratch set
- **AND** the classifier SHALL NOT escalate

#### Scenario: Repo-local scratch delete auto-approves

- **GIVEN** a live prompt whose command slice is `rm -rf .git-paw/tmp/wave-7`
- **WHEN** the classifier runs
- **THEN** the `.git-paw/tmp/` form SHALL match the scratch set and the classifier SHALL NOT escalate

#### Scenario: rm -rf "$VAR" resolving to scratch auto-approves

- **GIVEN** a live prompt whose command slice is `SCRATCH=/tmp/paw-x rm -rf "$SCRATCH"`
- **WHEN** the classifier resolves `$SCRATCH` to `/tmp/paw-x`
- **THEN** the scratch-path exception SHALL apply and the classifier SHALL NOT escalate

#### Scenario: Non-scratch rm -rf still escalates

- **GIVEN** a live prompt whose command slice is `rm -rf ~/Documents`
- **WHEN** the classifier runs
- **THEN** the scratch-path exception SHALL NOT apply and the danger-list SHALL escalate

#### Scenario: Mixed scratch and non-scratch targets escalate

- **GIVEN** a live prompt whose command slice is `rm -rf /tmp/paw-x /etc/important`
- **WHEN** the classifier runs
- **THEN** because not every target is scratch, the command SHALL escalate

### Requirement: Worktree-confined dev-test commands classify safe

The classifier SHALL extend its worktree-confinement rules (per `auto-approve-file-edits`) to common dev-test command shapes, classifying them safe-by-pattern when every referenced path resolves inside the agent's worktree:

- `bash -n <path>` (shell syntax check) — safe when `<path>` is worktree-resident
- non-recursive `chmod <mode> <path...>` — safe when all paths are worktree-resident; `chmod -R` SHALL remain danger-listed
- `mktemp` / `mktemp -d` — safe
- interpreter execution of a worktree-resident script (`bash`, `sh`, `python3`, `python`, `node` followed by a worktree-resident file path, with no path argument resolving outside the worktree) — safe for ONE-TIME approval; per the broad-grant rule such commands SHALL NEVER receive the permanent broad grant

Inline code strings (`-c` flags) SHALL NOT match these rules. Path resolution SHALL use the same canonicalized, fail-closed worktree boundary check as file operations. These rules SHALL apply only when a worktree root is known (agent panes); the supervisor pane, which has none, is unaffected.

#### Scenario: bash -n on a worktree script is safe

- **GIVEN** an agent whose worktree contains `scripts/helper.sh`
- **WHEN** the prompt's command slice is `bash -n scripts/helper.sh`
- **THEN** the classifier SHALL return safe-by-pattern

#### Scenario: chmod on own file is safe, recursive stays danger

- **WHEN** the command slice is `chmod +x scripts/helper.sh` (worktree-resident)
- **THEN** the classifier SHALL return safe-by-pattern
- **WHEN** the command slice is `chmod -R 755 .`
- **THEN** the danger-list SHALL match and the command SHALL escalate

#### Scenario: mktemp is safe

- **WHEN** the command slice is `mktemp -d`
- **THEN** the classifier SHALL return safe-by-pattern

#### Scenario: Interpreter run of a worktree script is one-time safe

- **GIVEN** an agent whose worktree contains `tools/gen.py`
- **WHEN** the prompt's command slice is `python3 tools/gen.py`
- **THEN** the classifier SHALL return safe-by-pattern
- **AND** on a 3-option prompt the auto-approver SHALL select the one-time option, never the permanent broad grant

#### Scenario: Inline code strings do not match

- **WHEN** the command slice is `python3 -c "import os; os.remove('x')"`
- **THEN** these rules SHALL NOT match (existing classification applies)

#### Scenario: Out-of-worktree script does not match

- **WHEN** the command slice is `bash /etc/init.d/thing`
- **THEN** these rules SHALL NOT match and the command SHALL NOT be auto-approved by them

### Requirement: Operator config-path writes escalate as danger

When a filesystem prompt (write / edit / create / delete) or a shell command slice targets a path that resolves inside the protected-path set (per `agent-memory-isolation`), the classifier SHALL classify it as a danger-class escalation — terminal, never auto-approved — evaluated with the same precedence as the curated danger-list (before any allowlist or safe-by-pattern rule). Read-only operations SHALL NOT match this rule.

Target paths SHALL be canonicalized before matching, with the same fail-closed posture as the worktree boundary check: a path that cannot be canonicalized but syntactically contains `..` or `~` components reaching into the protected set SHALL be treated as matching.

#### Scenario: Write to operator memory escalates as danger

- **GIVEN** an agent pane prompt "Do you want to allow this write to /Users/op/.claude/projects/-x-repo/memory/MEMORY.md?"
- **WHEN** the classifier runs
- **THEN** the verdict SHALL be a danger-class escalation
- **AND** no auto-approval keystrokes SHALL ever be dispatched for it

#### Scenario: Shell append to a configured settings file escalates

- **GIVEN** a config with `[clis.claude-oss] settings_path = "~/.claude-oss/settings.json"`
- **WHEN** a prompt's command slice is `echo '{}' >> ~/.claude-oss/settings.json`
- **THEN** the verdict SHALL be a danger-class escalation

#### Scenario: In-worktree writes are unaffected

- **GIVEN** an agent whose worktree root is `/repo/.git-paw/worktrees/feat-x`
- **WHEN** a prompt targets a write to `notes/memory.md` inside that worktree
- **THEN** this rule SHALL NOT match
- **AND** the existing worktree-confined safe-by-pattern classification SHALL apply

#### Scenario: Reads of operator config are not matched by this rule

- **WHEN** a prompt's command slice is `cat ~/.claude/settings.json`
- **THEN** this rule SHALL NOT match (other classification rules decide the verdict)

#### Scenario: Path-escape into the protected set is caught

- **GIVEN** a prompt targeting `<worktree>/../../../.claude/settings.json`
- **WHEN** the classifier canonicalizes the path
- **THEN** the resolved target SHALL match the protected set and escalate as danger

### Requirement: Permission prompt detection via tmux capture-pane

The system SHALL detect agent CLI permission prompts by capturing pane output and matching it against known prompt patterns.

#### Scenario: Approval prompt detected in working agent

- **GIVEN** a tmux pane running an agent CLI that has produced a permission prompt
- **WHEN** the supervisor polls the pane via `tmux capture-pane -p -t <session>:<pane>`
- **THEN** the system SHALL return `Some(PermissionType::<class>)` when the captured content contains an approval-prompt marker
- **AND** SHALL return `None` when no marker is present

#### Scenario: Detection is non-invasive

- **GIVEN** any agent CLI (claude, aider, codex, etc.)
- **WHEN** detection runs
- **THEN** detection SHALL only read pane output and SHALL NOT modify the agent's process or input

### Requirement: Prompt class identification

The detector SHALL classify each detected prompt into one of a fixed set of permission types so callers can decide whether to auto-approve.

#### Scenario: Curl prompts classified as Curl

- **GIVEN** captured pane content containing both an approval marker and `curl`
- **WHEN** classification runs
- **THEN** the result SHALL be `PermissionType::Curl`

#### Scenario: Cargo prompts classified as Cargo

- **GIVEN** captured pane content containing an approval marker and one of `cargo fmt`, `cargo clippy`, `cargo test`, or `cargo build`
- **WHEN** classification runs
- **THEN** the result SHALL be `PermissionType::Cargo`

#### Scenario: Unknown prompts classified as Unknown

- **GIVEN** captured pane content containing an approval marker but no recognised command class
- **WHEN** classification runs
- **THEN** the result SHALL be `PermissionType::Unknown`
- **AND** auto-approval SHALL NOT be triggered for `Unknown`

### Requirement: Capture is rate-limited

The detector SHALL NOT capture pane output more often than necessary to avoid load on tmux.

#### Scenario: Capture only on stall

- **GIVEN** an agent whose `last_seen` timestamp has not exceeded the stall threshold
- **WHEN** the supervisor's poll loop runs
- **THEN** detection SHALL NOT call `tmux capture-pane` for that agent

#### Scenario: Capture during stall

- **GIVEN** an agent whose status is `working` but whose `last_seen` is older than the configured stall threshold
- **WHEN** the supervisor's poll loop runs
- **THEN** detection SHALL call `tmux capture-pane` for that pane exactly once per poll tick

### Requirement: Curl allowlist setup

The system SHALL automatically create and configure an allowlist during
session startup to prevent permission prompts for broker communication.
The seeded grant SHALL be the single stable path of the bundled
agent-broker helper (`.git-paw/scripts/broker.sh`, the
`agent-broker-helper` capability) — a least-privilege, path-based grant.
The system SHALL NOT seed a broad `curl *` grant, and SHALL NOT depend
on per-endpoint `curl <broker-url><endpoint>` prefixes for the agent's
boot-time broker interactions.

#### Scenario: Allowlist created on session start

- **GIVEN** supervisor mode session with broker enabled
- **WHEN** `cmd_supervisor()` starts the session
- **THEN** an allowlist SHALL be created
- **AND** it SHALL grant the agent-broker helper path

#### Scenario: Allowlist grants the helper path, not broad curl

- **GIVEN** broker URL `http://127.0.0.1:9119`
- **WHEN** the allowlist is created
- **THEN** it SHALL contain a prefix authorising
  `.git-paw/scripts/broker.sh`
- **AND** it SHALL NOT contain a `curl *` (broad curl) grant

#### Scenario: Helper grant removes the boot-publish dead-stall

- **GIVEN** an agent whose first boot action publishes its register
  status via `.git-paw/scripts/broker.sh status booting`
- **WHEN** the agent runs that boot action with the helper-path grant
  seeded
- **THEN** no permission prompt SHALL appear
- **AND** the agent SHALL register with the broker without stalling

### Requirement: Allowlist file format

The system SHALL write the curl allowlist to the appropriate agent CLI configuration file with the correct format.

#### Scenario: Allowlist written to Claude settings

- **GIVEN** Claude CLI is used as supervisor
- **WHEN** allowlist is created
- **THEN** it SHALL be written to `.claude/settings.json`
- **AND** use the `allowed_bash_prefixes` format

#### Scenario: Allowlist format is valid JSON

- **WHEN** allowlist file is created
- **THEN** it SHALL be valid JSON
- **AND** contain an `allowed_bash_prefixes` array

### Requirement: Allowlist prevents permission prompts

The curl allowlist SHALL effectively prevent permission prompts for whitelisted commands.

#### Scenario: No permission prompt for allowlisted curl

- **GIVEN** curl command in allowlist
- **WHEN** agent executes the command
- **THEN** no permission prompt SHALL appear
- **AND** command executes immediately

#### Scenario: Permission prompt for non-allowlisted commands

- **GIVEN** curl command not in allowlist
- **WHEN** agent executes the command
- **THEN** permission prompt SHALL appear normally

### Requirement: Allowlist updates

The system SHALL support updating the curl allowlist when broker URL changes or new endpoints are added.

#### Scenario: Allowlist updated on broker URL change

- **GIVEN** session with broker URL change
- **WHEN** allowlist is regenerated
- **THEN** it SHALL contain the new broker URL

#### Scenario: New endpoints added to allowlist

- **GIVEN** new broker endpoint `/feedback`
- **WHEN** allowlist is updated
- **THEN** it SHALL include the new endpoint

### Requirement: Helper allowlist seeded per agent worktree

Under the same gating that governs the repo-root helper allowlist (broker enabled for the broker/sweep helper prefixes; docs base URL configured for the docs-fetch prefix), the system SHALL merge the helper-path allowlist into `<worktree>/.claude/settings.json` for every agent worktree at start, add, and session recovery — the same events that provision the helper scripts themselves. Merge semantics match the repo-root target; failures are non-fatal warnings.

#### Scenario: Worktree carries the helper grants next to the helper scripts

- **GIVEN** a broker-enabled session
- **WHEN** an agent worktree is attached
- **THEN** its `.claude/settings.json` `allowed_bash_prefixes` SHALL include the `.git-paw/scripts/broker.sh` path-scoped prefix
- **AND** the worktree SHALL also contain the provisioned helper scripts (per `agent-broker-helper`)

#### Scenario: Broker disabled seeds no broker prefix

- **GIVEN** a session with `[broker] enabled = false`
- **WHEN** an agent worktree is attached
- **THEN** the worktree settings SHALL NOT gain the broker helper prefix from this seeder

### Requirement: Config-driven broker-curl seeding for custom CLIs

When the broker is enabled, the system SHALL seed the
broker-curl allowlist into each session CLI's configured
settings file, given by `[clis.<name>].settings_path`. The
seeding target is CONFIG-DRIVEN — the system SHALL NOT
hardcode any CLI's settings path or name. A leading `~` in
the configured path SHALL be expanded to the home directory.
This is in addition to the always-seeded repo-local
`.claude/settings.json`.

#### Scenario: Configured settings_path is seeded

- **GIVEN** `[clis.mycli].settings_path = "~/.mycli/settings.json"`
  with the `~/.mycli/` directory present, and a session using
  `mycli` with the broker enabled
- **WHEN** the session launches
- **THEN** the broker endpoints SHALL be seeded into
  `~/.mycli/settings.json` so the CLI's boot-time
  `curl .../publish` does not raise a permission prompt

#### Scenario: No hardcoded CLI name or path

- **WHEN** the seeding code is inspected
- **THEN** it SHALL NOT reference any specific CLI name or
  settings path; custom-CLI seeding targets come only from
  `[clis.<name>].settings_path`

#### Scenario: CLI without settings_path seeds nothing extra

- **GIVEN** a session CLI that has no `[clis.<name>]` entry, or
  one without `settings_path`
- **WHEN** the session launches
- **THEN** only the repo-local `.claude/settings.json` SHALL be
  seeded; no other settings file is written for that CLI

### Requirement: Never create a CLI's config directory

The system SHALL seed a configured `settings_path` only when
its parent directory already exists, mirroring the
dev-allowlist seeder's caution — git-paw SHALL NOT create a
CLI's config directory.

#### Scenario: Missing parent directory is skipped

- **GIVEN** `[clis.mycli].settings_path` whose parent
  directory does not exist
- **WHEN** the session launches
- **THEN** the system SHALL NOT create the directory and SHALL
  NOT write the settings file for that path

### Requirement: Seeding is idempotent, deduped, and non-fatal

Seeding SHALL be idempotent (re-seeding never duplicates
allowlist entries and preserves pre-existing entries), SHALL
seed each distinct settings path at most once per launch even
when supervisor and agent CLIs resolve to the same path, and
SHALL be non-fatal (a write failure logs a stderr warning and
session launch continues).

#### Scenario: Re-attach does not duplicate entries

- **GIVEN** a CLI whose configured settings file was already
  seeded
- **WHEN** seeding runs again on re-attach
- **THEN** the broker-endpoint entries SHALL appear exactly
  once and pre-existing unrelated entries SHALL remain

#### Scenario: Same path for supervisor and agent seeds once

- **GIVEN** the supervisor CLI and the agent CLI resolve to the
  same configured `settings_path`
- **WHEN** the session launches
- **THEN** that path SHALL be seeded exactly once

#### Scenario: Unwritable settings file warns and continues

- **GIVEN** a configured `settings_path` whose parent exists
  but the file cannot be written
- **WHEN** seeding attempts to run
- **THEN** the system SHALL emit a stderr warning and continue
  launching the session

### Requirement: Common dev allowlist seeded on supervisor start

The system SHALL seed a curated set of common dev-command prefix
patterns into the Claude CLI's `allowed_bash_prefixes` configuration
when a supervisor mode session starts.

The seeding SHALL occur when **both** of the following hold:

- The session is a supervisor mode session (i.e. `cmd_supervisor()` is
  the entry point for the session start or recovery).
- The effective `[supervisor.common_dev_allowlist] enabled` config
  value is `true` (the default; per the `supervisor-config` delta).

The seeding SHALL apply the **built-in preset** described in the
"Standard preset content" requirement below, plus any user-supplied
`extra` patterns from `[supervisor.common_dev_allowlist] extra`.

The seeding SHALL run **independently of** the broker enable status —
non-broker supervisor sessions also benefit from suppressed dev-command
prompts.

When the seeding fails (e.g. unreadable `.claude/settings.json`,
invalid JSON, disk error), the failure SHALL be logged to stderr but
SHALL NOT abort session start. This matches the existing
`curl-allowlist` non-fatal failure contract.

#### Scenario: Preset seeded on supervisor start with default config

- **GIVEN** a `.git-paw/config.toml` with `[supervisor] enabled = true`
  and no `[supervisor.common_dev_allowlist]` section
- **WHEN** `cmd_supervisor()` starts a session
- **THEN** the file `<repo>/.claude/settings.json` SHALL be created
  (or merged-into) with the built-in dev allowlist preset appended to
  the `allowed_bash_prefixes` array

#### Scenario: Preset not seeded when feature disabled

- **GIVEN** a `.git-paw/config.toml` with `[supervisor.common_dev_allowlist] enabled = false`
- **WHEN** `cmd_supervisor()` starts a session
- **THEN** the file `<repo>/.claude/settings.json` SHALL NOT receive
  any new entries from the dev-allowlist seeder
- **AND** any entries already present in the file (from prior sessions
  or hand-edits) SHALL be left unchanged

#### Scenario: Seeding runs regardless of broker enable status

- **GIVEN** a `.git-paw/config.toml` with `[supervisor] enabled = true`
  and `[broker] enabled = false`
- **WHEN** `cmd_supervisor()` starts a session
- **THEN** the dev allowlist preset SHALL still be merged into
  `<repo>/.claude/settings.json`

#### Scenario: Seeding failure does not abort session start

- **GIVEN** `<repo>/.claude/settings.json` exists but contains invalid JSON
- **WHEN** `cmd_supervisor()` starts a session
- **THEN** a warning SHALL be written to stderr identifying the file
  and the parse error
- **AND** the supervisor session SHALL continue to start normally

### Requirement: Standard preset content

The system SHALL define the built-in dev allowlist preset as a constant
list of **prefix-matchable** patterns in source code (not config-driven).
Each seeded pattern SHALL be a command **prefix** (a verb, or verb plus
subcommand) that subsumes all per-invocation argument variations, NOT a
full command line. For example the preset SHALL seed `git diff` (which
prefix-matches `git diff --stat HEAD~1`), never a fully-argumented form
such as `git diff --stat HEAD~1`. This ensures a routine dev-loop command
prompts at most once regardless of argument variation.

The built-in preset SHALL contain **only universal commands** — commands
that are safe and useful in essentially any repository independent of its
language or toolchain. The preset SHALL contain exactly the following
patterns (order is irrelevant; the set is what matters):

- **Git read-only**: `git status`, `git log`, `git diff`, `git show`,
  `git fetch`
- **Git write (non-destructive)**: `git commit`, `git push`,
  `git pull`, `git merge`, `git stash`, `git add`, `git restore`,
  `git rm`
- **Search (read-only)**: `find`, `grep`, `sed -n`

The built-in preset SHALL NOT contain any stack-specific patterns. The
following were part of the previous (over-opinionated) preset and SHALL
NO LONGER be hardcoded into the universal preset; they are contributed via
named stack presets and/or `extra` (per the "Named stack presets"
requirement):

- **Cargo / Rust**: `cargo build`, `cargo test`, `cargo clippy`,
  `cargo fmt`, `cargo check`, `cargo tree`, `cargo deny`, `cargo update`
- **Just**: `just`
- **mdBook**: `mdbook build`
- **OpenSpec**: `openspec validate`, `openspec new`, `openspec archive`,
  `openspec list`, `openspec status`, `openspec instructions`

The preset SHALL continue to exclude (intentional exclusions; rationale in
`design.md` D3) the following destructive patterns from BOTH the universal
preset and any curated stack preset:

- `cargo install`, `cargo run`, `cargo bench`
- `git rebase`, `git reset`, `git checkout`, `git branch -D`
- `git push --force`, `git push -f`
- `find ... -exec` patterns (the bare `find` prefix is included; users
  wanting `-exec` patterns add them via `extra`)
- `sed` without `-n` (write-mode sed)

The constant SHALL be exported from the dev-allowlist module so tests
can assert its content. The constant SHALL be the single source of
truth: no other location in the codebase may hard-code preset patterns.

#### Scenario: Universal preset contains only stack-neutral patterns

- **GIVEN** the dev-allowlist module's exported universal preset constant
- **WHEN** the test inspects its contents
- **THEN** every universal pattern listed above (git read-only, git
  non-destructive write, `find`, `grep`, `sed -n`) SHALL be present
- **AND** no stack-specific pattern (`cargo *`, `just`, `mdbook build`,
  `openspec *`) SHALL be present
- **AND** no pattern from the exclusions list SHALL be present

#### Scenario: Seeded entries match the universal preset constant

- **GIVEN** a fresh `.claude/settings.json` (file absent or empty)
- **WHEN** the dev-allowlist seeder runs with empty `extra` and no stack
  presets selected
- **THEN** the resulting `allowed_bash_prefixes` array SHALL contain
  exactly the patterns from the universal preset constant (no extra, no
  missing)

#### Scenario: Seeded entries are prefix forms, not full command lines

- **GIVEN** the dev-allowlist seeder runs on a fresh `.claude/settings.json`
- **WHEN** the resulting `allowed_bash_prefixes` entries are inspected
- **THEN** every seeded universal entry SHALL be a bare command prefix
  (e.g. `git diff`) that prefix-matches its argument variants
- **AND** no seeded universal entry SHALL embed run-specific arguments
  (e.g. no `git diff --stat HEAD~1`)

#### Scenario: Non-Rust project does not receive cargo grants by default

- **GIVEN** a repository with no Rust toolchain and a
  `.git-paw/config.toml` that selects no stack presets and sets empty
  `extra`
- **WHEN** the dev-allowlist seeder runs on supervisor start
- **THEN** the resulting `allowed_bash_prefixes` SHALL NOT contain
  `cargo build`, `cargo test`, `just`, `mdbook build`, or any
  `openspec *` pattern

### Requirement: User-extensible allowlist via `extra` field

The system SHALL append any patterns from
`[supervisor.common_dev_allowlist] extra` to the built-in universal preset
(and to any selected stack presets) when seeding. The `extra` field SHALL
accept arbitrary strings; the system SHALL NOT validate or filter them.

User-supplied `extra` patterns SHALL be appended **after** the preset
in the resulting `allowed_bash_prefixes` array (order is
informational; Claude's allowlist is a set).

Duplicate patterns (an `extra` entry that matches an existing entry,
whether from the preset, a selected stack preset, or a prior session's
seeding) SHALL NOT be added a second time. This matches the existing
`curl-allowlist` de-duplication contract.

#### Scenario: Extra patterns appended to preset

- **GIVEN** a `.git-paw/config.toml` with
  `[supervisor.common_dev_allowlist] extra = ["pnpm test", "deno fmt"]`
- **WHEN** the dev-allowlist seeder runs on a fresh `.claude/settings.json`
- **THEN** the resulting `allowed_bash_prefixes` SHALL contain every
  universal preset pattern PLUS `"pnpm test"` AND `"deno fmt"`

#### Scenario: Duplicate extra entry not added twice

- **GIVEN** an existing `.claude/settings.json` already containing
  `"git diff"` in `allowed_bash_prefixes`
- **AND** `extra = ["git diff"]` (matches an existing entry)
- **WHEN** the seeder runs
- **THEN** `"git diff"` SHALL appear exactly once in the resulting
  array (no duplicate)

#### Scenario: Extra entries not validated

- **GIVEN** `extra = ["this is a nonsense string $$"]`
- **WHEN** the seeder runs
- **THEN** the seeder SHALL succeed
- **AND** the nonsense entry SHALL be present in
  `allowed_bash_prefixes` (Claude's matcher will simply never hit it)

### Requirement: Merge semantics preserve existing entries

The system SHALL merge new entries into the target settings file
without overwriting unrelated content, using the same semantics as the
existing curl-allowlist seeder:

- When the target file does not exist, a fresh JSON object SHALL be
  created with `allowed_bash_prefixes` set to the merged entries.
- When the target file exists with valid JSON, existing fields SHALL
  be preserved unchanged. The `allowed_bash_prefixes` array SHALL be
  extended with any missing entries from the preset + `extra`.
- When the target file exists but contains invalid JSON, the seeder
  SHALL return an error WITHOUT modifying the file. The error SHALL be
  logged to stderr (per the "Common dev allowlist seeded on supervisor
  start" requirement's non-fatal contract) and supervisor start SHALL
  continue.
- Duplicate entries SHALL NOT be added.
- Parent directories SHALL be created when missing.

#### Scenario: Preserves user's existing settings.json content

- **GIVEN** `<repo>/.claude/settings.json` exists with
  `{"some_custom_field": "value", "allowed_bash_prefixes": ["my-tool"]}`
- **WHEN** the seeder runs with the default preset
- **THEN** `some_custom_field` SHALL still equal `"value"` after seeding
- **AND** `allowed_bash_prefixes` SHALL still contain `"my-tool"`
- **AND** `allowed_bash_prefixes` SHALL also contain every preset
  pattern

#### Scenario: Re-seeding is idempotent

- **GIVEN** the seeder has previously run against `.claude/settings.json`
- **WHEN** the seeder runs again with the same preset and `extra`
- **THEN** no entry from the preset SHALL appear more than once in
  the resulting `allowed_bash_prefixes`

#### Scenario: Invalid JSON in target file does not abort session

- **GIVEN** `<repo>/.claude/settings.json` contains malformed JSON
- **WHEN** the seeder runs
- **THEN** the file SHALL NOT be overwritten
- **AND** a warning SHALL be logged to stderr identifying the file
  and the parse error
- **AND** the supervisor session SHALL continue to start normally

### Requirement: Per-CLI placement (Claude / config-driven settings paths)

The system SHALL write the merged allowlist to
`<repo>/.claude/settings.json` on every supervisor start where the
feature is enabled.

The system SHALL ALSO merge the same allowlist into each configured
`[clis.<name>].settings_path` whose parent directory already exists at
session start, using the same merge semantics. The set of alternate
targets is resolved from configuration only — there is no hardcoded
CLI name or path. When a configured `settings_path`'s parent directory
does not exist, the system SHALL NOT create it and SHALL skip that
target. When no `[clis.<name>].settings_path` is configured, only
`<repo>/.claude/settings.json` is written.

The system SHALL NOT write to any other CLI's configuration file in
this change (Codex, Gemini, opencode, Cursor, etc. are deferred to the
v1.0.0 hook-providers capability).

#### Scenario: Writes to `<repo>/.claude/settings.json`

- **GIVEN** the feature is enabled and a supervisor session starts in
  repository `<repo>`
- **WHEN** the seeder runs
- **THEN** the file `<repo>/.claude/settings.json` SHALL contain the
  merged allowlist
- **AND** any parent directory `<repo>/.claude/` that did not exist
  SHALL be created

#### Scenario: Writes to a configured settings_path when its parent exists

- **GIVEN** config defines `[clis.my-variant].settings_path =
  "~/.config/my-variant/settings.json"` and the directory
  `~/.config/my-variant/` exists at session start
- **AND** the feature is enabled
- **WHEN** the seeder runs
- **THEN** the file `~/.config/my-variant/settings.json` SHALL ALSO
  contain the merged allowlist with the same entries as
  `<repo>/.claude/settings.json`

#### Scenario: Skips a configured settings_path when its parent is absent

- **GIVEN** config defines `[clis.my-variant].settings_path =
  "~/.config/my-variant/settings.json"` but `~/.config/my-variant/`
  does not exist at session start
- **WHEN** the seeder runs
- **THEN** no `~/.config/my-variant/` directory SHALL be created
- **AND** only `<repo>/.claude/settings.json` SHALL be written

#### Scenario: No hardcoded CLI path is seeded without config

- **GIVEN** the directory `~/.claude-oss/` exists at session start
- **AND** no `[clis.<name>].settings_path` points into it
- **WHEN** the seeder runs
- **THEN** `~/.claude-oss/settings.json` SHALL NOT be written by the
  seeder (the alternate target set is config-driven only)

#### Scenario: No write to other CLI configs

- **GIVEN** the user has `~/.codex/config.toml` and `~/.gemini/`
  present at session start
- **WHEN** the seeder runs
- **THEN** neither file/directory SHALL be modified by this seeder

### Requirement: Named stack presets

The system SHALL provide a set of named, curated stack presets that a
repository opts into to seed the prefix grants for a particular toolchain.
The system SHALL define at minimum the named presets `rust`, `node`,
`python`, and `go`, each as a constant list of prefix-matchable patterns
exported from the dev-allowlist module (single source of truth, reviewable
in PRs).

A repository SHALL select stack presets through configuration (e.g. a
`[supervisor.common_dev_allowlist] stacks = [...]` list; the exact key name
follows local serde conventions). When one or more stack presets are
selected, the seeder SHALL seed the **union** of: the universal preset, each
selected stack preset, and any `extra` patterns, de-duplicated. When no
stack preset is selected, the seeder SHALL seed only the universal preset
plus `extra`.

Each curated stack preset SHALL obey the inclusion/exclusion rubric
(`design.md` D3): only bounded-side-effect build/test/lint verbs; no
destructive verbs (e.g. `cargo install`/`run`/`bench`, package-manager
uninstall/publish, force-push). Selecting a stack preset SHALL be the only
implicit grant; the system SHALL NOT auto-detect a repository's toolchain
and select a preset on its behalf.

#### Scenario: Selecting the rust stack seeds cargo prefixes

- **GIVEN** a `.git-paw/config.toml` with
  `[supervisor.common_dev_allowlist] stacks = ["rust"]`
- **WHEN** the dev-allowlist seeder runs on a fresh `.claude/settings.json`
- **THEN** the resulting `allowed_bash_prefixes` SHALL contain the
  universal preset patterns
- **AND** SHALL contain the curated `rust` stack prefixes (e.g.
  `cargo build`, `cargo test`, `cargo clippy`)

#### Scenario: Selecting the node stack does not seed cargo prefixes

- **GIVEN** a `.git-paw/config.toml` with
  `[supervisor.common_dev_allowlist] stacks = ["node"]`
- **WHEN** the dev-allowlist seeder runs on a fresh `.claude/settings.json`
- **THEN** the resulting `allowed_bash_prefixes` SHALL contain the curated
  `node` stack prefixes (e.g. `npm`, `pnpm`)
- **AND** SHALL NOT contain any `cargo *` pattern

#### Scenario: No stack selected seeds only the universal preset

- **GIVEN** a `.git-paw/config.toml` with no `stacks` entry and empty
  `extra`
- **WHEN** the dev-allowlist seeder runs on a fresh `.claude/settings.json`
- **THEN** the resulting `allowed_bash_prefixes` SHALL equal exactly the
  universal preset (no stack-specific patterns)

#### Scenario: Multiple stacks compose as a union

- **GIVEN** a `.git-paw/config.toml` with
  `[supervisor.common_dev_allowlist] stacks = ["rust", "python"]`
- **WHEN** the dev-allowlist seeder runs on a fresh `.claude/settings.json`
- **THEN** the resulting `allowed_bash_prefixes` SHALL contain both the
  curated `rust` prefixes and the curated `python` prefixes
- **AND** any pattern present in more than one selected set SHALL appear
  exactly once

### Requirement: Per-worktree placement for agent panes

When `[supervisor.common_dev_allowlist]` is enabled, the system SHALL merge the resolved dev-command patterns (universal preset + named stacks + `extra`) into `<worktree>/.claude/settings.json` for EVERY agent worktree, using the same merge semantics as the repo-root target (preserve existing entries, dedup, non-fatal per-target errors reported as warnings). Seeding SHALL run:

- for each worktree attached by `git paw start`;
- for a worktree attached by `git paw add`;
- for every restored worktree during session recovery.

The seeder SHALL create `<worktree>/.claude/` when absent (it lies inside a worktree git-paw created). It SHALL ensure the seeded path is excluded from version control via the WORKTREE-LOCAL ignore mechanism (`info/exclude` for that worktree) — never by editing any tracked `.gitignore`. When the feature is disabled, no worktree settings file SHALL be written by this seeder.

#### Scenario: Start seeds every agent worktree

- **GIVEN** the feature is enabled with `stacks = ["rust"]` and a supervisor session starting with two branches
- **WHEN** the session is started
- **THEN** each agent worktree SHALL contain `.claude/settings.json` whose `allowed_bash_prefixes` include the universal preset and the rust-stack patterns

#### Scenario: Add seeds the new worktree

- **GIVEN** a running session and `git paw add feat-new`
- **WHEN** the new agent attaches
- **THEN** `<new-worktree>/.claude/settings.json` SHALL contain the merged patterns

#### Scenario: Recovery re-seeds restored worktrees

- **GIVEN** a recoverable session with the feature enabled
- **WHEN** the session is recovered
- **THEN** every restored agent worktree SHALL carry the merged patterns (picking up preset updates)

#### Scenario: Existing worktree settings entries are preserved

- **GIVEN** an agent worktree whose `.claude/settings.json` already contains a custom `allowed_bash_prefixes` entry
- **WHEN** the seeder runs
- **THEN** the custom entry SHALL remain and the merged patterns SHALL be appended without duplicates

#### Scenario: Seeded file cannot be committed by the agent

- **GIVEN** a seeded agent worktree
- **WHEN** the agent runs `git status` / `git add .` inside the worktree
- **THEN** `.claude/` SHALL be excluded via the worktree-local ignore
- **AND** no tracked `.gitignore` SHALL have been modified

#### Scenario: Disabled feature writes nothing

- **GIVEN** `[supervisor.common_dev_allowlist] enabled = false`
- **WHEN** a session starts
- **THEN** no agent worktree `.claude/settings.json` SHALL be written by this seeder

# approval-command-safety Specification

## Purpose
Detects agent-CLI permission prompts non-invasively via rate-limited `tmux capture-pane`, classifying each stalled prompt into a fixed permission type (Curl, Cargo, Unknown, etc.), and classifies a captured command's command slice as auto-approvable or escalate-to-human by prefix-matching a configurable safe-command whitelist, while a terminal, curated per-OS danger-list (force-push, hard reset, sudo, device writes, process kills, etc.) always escalates — subject only to a scratch-path exception for `rm -rf` targeting repo/OS scratch paths — so callers can decide whether to auto-approve. The curated, stack-neutral dev-command prefix allowlist — with opt-in named stack presets (`rust`, `node`, `python`, `go`) and a user `extra` list — feeds that whitelist and the bundled skill guidance. git-paw does not write permission grants into any agent CLI's settings files (seeding was removed in v0.16.0); routine agent actions are approved by the classifier and the resolved approval flags instead.
## Requirements
### Requirement: Whitelist of safe command classes

The system SHALL maintain an explicit whitelist of command prefixes that are eligible for auto-approval, and SHALL NOT auto-approve anything outside the whitelist. The effective whitelist SHALL be composed from:

1. the **built-in stack-neutral entries**: the read-mostly verb allowlist — commands whose leading verb is one of `curl`, `cat`, `ls`, `grep`, `rg`, `git`, `echo`, `sed`, `awk`, `find`, `wc`, `head`, `tail`, `jq`, `mkdir`, `touch`, `export`, `tmux`, `env` — plus `git commit` and the broker-localhost prefix `curl http://127.0.0.1:`;
2. the **resolved dev-allowlist patterns**: `effective_patterns(stacks, extra)` per `dev-command-allowlist` — the universal preset, the named stack presets selected by `[supervisor.common_dev_allowlist] stacks`, and its `extra` entries;
3. the **configured extension**: `[supervisor.auto_approve] safe_commands`.

The built-in entries SHALL NOT contain stack- or tool-specific patterns. In particular, the previously hardcoded `cargo fmt`, `cargo clippy`, `cargo test`, `cargo build`, `openspec`, and `just` SHALL NO LONGER be built in — projects receive their toolchain verbs through the resolved stack presets and/or configured extensions. The stack presets SHALL be consumed from the dev-allowlist module's exported constants (single source of truth — no duplicated pattern lists).

A whitelist match SHALL be subordinate to the danger-list: when the curated danger-list (see "Curated danger-list escalates to human") matches the same command, the command SHALL escalate regardless of any whitelist match.

The bundled `sweep.sh classify` helper SHALL NOT maintain its own copy of the whitelist or the classification logic. It SHALL obtain its verdict by delegating to the single Rust classifier via the hidden `git paw __classify` subcommand (see "Safe-command classification is exposed via a hidden __classify subcommand"), so the helper and the in-tool auto-approver are the same implementation and cannot drift.

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

#### Scenario: sweep.sh classifies by delegating to the Rust classifier

- **GIVEN** `[supervisor.common_dev_allowlist] stacks = ["rust"]` in `.git-paw/config.toml`
- **WHEN** `sweep.sh classify` evaluates a capture whose command slice is `cargo fmt --check`
- **THEN** it SHALL obtain its verdict by invoking `git paw __classify` and render that verdict (safe)
- **AND** a guard SHALL assert `sweep.sh` contains no parallel classifier verb lists — classification is single-source

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

The detector SHALL classify each detected prompt into one of a fixed set of permission types so callers can decide whether to auto-approve. The set SHALL NOT include a class defined by a specific language toolchain's build commands. Toolchain-specific verbs, where they are relevant to a safety decision, SHALL be sourced from the resolved stack preset rather than hard-coded into the classifier, so the detector stays stack-neutral for every consumer.

#### Scenario: Curl prompts classified as Curl

- **GIVEN** captured pane content containing both an approval marker and `curl`
- **WHEN** classification runs
- **THEN** the result SHALL be `PermissionType::Curl`

#### Scenario: Unknown prompts classified as Unknown

- **GIVEN** captured pane content containing an approval marker but no recognised command class
- **WHEN** classification runs
- **THEN** the result SHALL be `PermissionType::Unknown`
- **AND** auto-approval SHALL NOT be triggered for `Unknown`

#### Scenario: No toolchain-specific permission class exists

- **WHEN** the set of permission types is inspected
- **THEN** it SHALL NOT contain a class defined by a specific language toolchain's build commands

#### Scenario: Build-tool prompt is not specially classified

- **GIVEN** captured pane content containing an approval marker and a language build-tool invocation
- **WHEN** classification runs
- **THEN** it SHALL NOT be assigned a toolchain-specific class
- **AND** the auto-approval decision SHALL be made by the safe-command classifier, whose behaviour is unchanged

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

### Requirement: Standard preset content

The system SHALL define the built-in dev allowlist preset as a constant
list of **prefix-matchable** patterns in source code (not config-driven).
Each pattern SHALL be a command **prefix** (a verb, or verb plus
subcommand) that subsumes all per-invocation argument variations, NOT a
full command line. For example the preset SHALL contain `git diff` (which
prefix-matches `git diff --stat HEAD~1`), never a fully-argumented form
such as `git diff --stat HEAD~1`. This ensures a routine dev-loop command
is matched once regardless of argument variation.

The universal preset SHALL feed two consumers. It is composed into the supervisor's
auto-approve whitelist through `effective_patterns(stacks, extra)` (see *Whitelist of safe
command classes*), and it is rendered into the bundled supervisor skill's permission guidance.
It SHALL NOT be written into any agent CLI's settings file (see *No permission grants are seeded
into agent CLI settings files*).

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

#### Scenario: Resolved patterns match the universal preset constant

- **GIVEN** no stack presets selected and empty `extra`
- **WHEN** the dev-allowlist patterns are resolved with `effective_patterns`
- **THEN** the result SHALL contain exactly the patterns from the
  universal preset constant (no extra, no missing)

#### Scenario: Preset entries are prefix forms, not full command lines

- **GIVEN** the universal preset constant
- **WHEN** its entries are inspected
- **THEN** every entry SHALL be a bare command prefix (e.g. `git diff`)
  that prefix-matches its argument variants
- **AND** no entry SHALL embed run-specific arguments
  (e.g. no `git diff --stat HEAD~1`)

#### Scenario: Universal preset feeds the auto-approve whitelist and the skill guidance

- **GIVEN** a supervisor configuration with no stacks, empty `extra`, and no `safe_commands`
- **WHEN** the effective auto-approve whitelist is composed and the bundled supervisor skill is rendered
- **THEN** the whitelist SHALL contain every universal preset pattern
- **AND** the rendered permission guidance SHALL enumerate every universal preset pattern

#### Scenario: Non-Rust project does not receive cargo patterns by default

- **GIVEN** a repository with no Rust toolchain and a
  `.git-paw/config.toml` that selects no stack presets and sets empty
  `extra`
- **WHEN** the supervisor's effective auto-approve whitelist is composed
- **THEN** it SHALL NOT contain `cargo build`, `cargo test`, `just`,
  `mdbook build`, or any `openspec *` pattern

### Requirement: User-extensible allowlist via `extra` field

The system SHALL append any patterns from
`[supervisor.common_dev_allowlist] extra` to the built-in universal preset
(and to any selected stack presets) when resolving the dev-allowlist
patterns that feed the auto-approve whitelist. The `extra` field SHALL
accept arbitrary strings; the system SHALL NOT validate or filter them.

User-supplied `extra` patterns SHALL be appended **after** the preset
in the resolved pattern list (order is informational; matching treats the
list as a set).

Duplicate patterns (an `extra` entry that matches an existing entry,
whether from the preset or a selected stack preset) SHALL NOT be added a
second time.

#### Scenario: Extra patterns appended to preset

- **GIVEN** a `.git-paw/config.toml` with
  `[supervisor.common_dev_allowlist] extra = ["pnpm test", "deno fmt"]`
- **WHEN** the dev-allowlist patterns are resolved
- **THEN** the result SHALL contain every universal preset pattern PLUS
  `"pnpm test"` AND `"deno fmt"`
- **AND** the `extra` entries SHALL come after the universal preset entries

#### Scenario: Duplicate extra entry not added twice

- **GIVEN** `extra = ["git diff"]` (matches a universal preset entry)
- **WHEN** the dev-allowlist patterns are resolved
- **THEN** `"git diff"` SHALL appear exactly once in the result

#### Scenario: Extra entries not validated

- **GIVEN** `extra = ["this is a nonsense string $$"]`
- **WHEN** the dev-allowlist patterns are resolved
- **THEN** resolution SHALL succeed
- **AND** the nonsense entry SHALL be present in the result (the
  classifier will simply never match it)

### Requirement: Named stack presets

The system SHALL provide a set of named, curated stack presets that a
repository opts into to contribute the prefix patterns for a particular
toolchain to the supervisor's auto-approve whitelist. The system SHALL
define at minimum the named presets `rust`, `node`, `python`, and `go`,
each as a constant list of prefix-matchable patterns exported from the
dev-allowlist module (single source of truth, reviewable in PRs).

A repository SHALL select stack presets through configuration (e.g. a
`[supervisor.common_dev_allowlist] stacks = [...]` list; the exact key name
follows local serde conventions). When one or more stack presets are
selected, `effective_patterns` SHALL resolve the **union** of: the
universal preset, each selected stack preset, and any `extra` patterns,
de-duplicated; that union is what the auto-approve whitelist composes.
When no stack preset is selected, it SHALL resolve only the universal
preset plus `extra`. An unknown stack name SHALL contribute nothing.

Each curated stack preset SHALL obey the inclusion/exclusion rubric
(`design.md` D3): only bounded-side-effect build/test/lint verbs; no
destructive verbs (e.g. `cargo install`/`run`/`bench`, package-manager
uninstall/publish, force-push). Selecting a stack preset SHALL be the only
implicit grant; the system SHALL NOT auto-detect a repository's toolchain
and select a preset on its behalf.

#### Scenario: Selecting the rust stack contributes cargo prefixes

- **GIVEN** a `.git-paw/config.toml` with
  `[supervisor.common_dev_allowlist] stacks = ["rust"]`
- **WHEN** the dev-allowlist patterns are resolved
- **THEN** the result SHALL contain the universal preset patterns
- **AND** SHALL contain the curated `rust` stack prefixes (e.g.
  `cargo build`, `cargo test`, `cargo clippy`)

#### Scenario: Selecting the node stack does not contribute cargo prefixes

- **GIVEN** a `.git-paw/config.toml` with
  `[supervisor.common_dev_allowlist] stacks = ["node"]`
- **WHEN** the dev-allowlist patterns are resolved
- **THEN** the result SHALL contain the curated `node` stack prefixes
  (e.g. `npm`, `pnpm`)
- **AND** SHALL NOT contain any `cargo *` pattern

#### Scenario: No stack selected resolves only the universal preset

- **GIVEN** a `.git-paw/config.toml` with no `stacks` entry and empty
  `extra`
- **WHEN** the dev-allowlist patterns are resolved
- **THEN** the result SHALL equal exactly the universal preset (no
  stack-specific patterns)

#### Scenario: Multiple stacks compose as a union

- **GIVEN** a `.git-paw/config.toml` with
  `[supervisor.common_dev_allowlist] stacks = ["rust", "python"]`
- **WHEN** the dev-allowlist patterns are resolved
- **THEN** the result SHALL contain both the curated `rust` prefixes and
  the curated `python` prefixes
- **AND** any pattern present in more than one selected set SHALL appear
  exactly once

#### Scenario: Unknown stack name contributes nothing

- **GIVEN** `stacks = ["cobol"]` (not a built-in stack preset)
- **WHEN** the dev-allowlist patterns are resolved
- **THEN** resolution SHALL succeed and the result SHALL equal the universal preset

### Requirement: Classifier normalizes a trailing exit-code probe before matching

The safe-command classifier SHALL strip a trailing exit-code probe or output
redirect from a command before prefix-matching it against the allowlist, so that an
otherwise-safe command is not forced `unknown` merely because it was wrapped for
gate reporting. Specifically it SHALL remove a trailing `; echo …$?` / `; RC=$?`
clause and a trailing `>/dev/null 2>&1` (or `> /dev/null 2>&1`) redirect, then
classify the remaining verb. The danger-list, worktree-confinement, and
config-path rules SHALL be applied to the normalized command exactly as before —
normalization SHALL NOT weaken any escalation.

#### Scenario: A safe command with a trailing exit-code probe classifies safe

- **GIVEN** the command `cargo test --lib; echo test-exit=$?`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL classify the same as bare `cargo test --lib`

#### Scenario: A trailing redirect is normalized away

- **GIVEN** the command `mdbook build docs/ >/dev/null 2>&1`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL classify the same as bare `mdbook build docs/`

#### Scenario: Normalization does not rescue a danger command

- **GIVEN** a danger-listed command with a trailing `; echo $?` appended
- **WHEN** the classifier evaluates it
- **THEN** it SHALL still escalate as danger — stripping the probe SHALL NOT downgrade it

### Requirement: Safe-command classification is exposed via a hidden __classify subcommand

The system SHALL provide a hidden `git paw __classify` subcommand that runs the one safe-command classifier and emits its verdict, so the bundled `sweep.sh` helper classifies by delegating to it rather than re-implementing the classifier in embedded Python. `__classify` SHALL read a pane capture on stdin and take the worktree root as an argument; it SHALL emit on stdout the classification (`safe`, `danger`, or `unknown`) and the option index to select, computed by the exact classifier path the in-tool auto-approver uses, so the two cannot disagree. `__classify` SHALL be read-only — it classifies and SHALL NOT send keystrokes, contact the broker, or write any file. It SHALL be hidden from `git paw --help`, like `__dashboard`. The supervisor session's allowlist SHALL grant `git paw __classify` path-scoped (never a broad `git paw *`).

#### Scenario: __classify emits the classifier's verdict for a danger command

- **GIVEN** a pane capture whose command slice is a danger-listed command
- **WHEN** `git paw __classify` classifies it
- **THEN** it SHALL emit `danger`
- **AND** the verdict SHALL match what the in-tool auto-approver would decide for the same capture

#### Scenario: __classify emits a safe verdict with the option index

- **GIVEN** a pane capture for a classifier-safe prompt offering a durable option
- **WHEN** `git paw __classify` classifies it
- **THEN** it SHALL emit `safe` and the option index the in-tool auto-approver would select

#### Scenario: __classify is read-only

- **WHEN** `git paw __classify` runs against a capture
- **THEN** it SHALL emit a verdict on stdout
- **AND** SHALL NOT send any tmux keystroke, issue any broker request, or write any file

#### Scenario: __classify is hidden from help

- **WHEN** `git paw --help` is inspected
- **THEN** `__classify` SHALL NOT appear as a listed subcommand

### Requirement: Classifier normalizes leading assignment and wrapper prefixes before matching

The safe-command classifier SHALL strip a run of leading `NAME=value`
environment-variable assignments and leading `env` / `nohup` invocation wrappers
from a command before prefix-matching it against the allowlist, so that an
otherwise-safe command is not forced `unknown` merely because it was prefixed for
the run environment. Normalization SHALL remove: zero or more leading `NAME=value`
assignments (shell assignment syntax, values optionally quoted); a leading `env`
wrapper together with any `NAME=value` assignments and `-i` / `-u NAME` options it
carries; and a leading `nohup` wrapper. It SHALL then classify the remaining verb.
This normalization SHALL compose with the existing trailing exit-code-probe /
redirect normalization. The danger-list, worktree-confinement, config-path, and
`.git/`-write rules SHALL be applied to the fully normalized command exactly as
before — normalization SHALL NOT weaken any escalation.

#### Scenario: A leading VAR=value assignment is normalized away

- **GIVEN** the command `TMPDIR=/tmp/x cargo test --lib`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL classify the same as bare `cargo test --lib`

#### Scenario: Multiple leading assignments plus a trailing probe are both normalized

- **GIVEN** the command `GIT_PAW_ALLOW_LIVE_SESSION=1 TMPDIR=/tmp/x cargo test; echo exit=$?`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL classify the same as bare `cargo test`

#### Scenario: A leading env wrapper is normalized away

- **GIVEN** the command `env FOO=bar cargo build`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL classify the same as bare `cargo build`

#### Scenario: A leading nohup wrapper is normalized away

- **GIVEN** the command `nohup just check`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL classify the same as bare `just check`

#### Scenario: Normalization does not rescue a danger command behind assignments

- **GIVEN** a danger-listed command prefixed with an assignment (e.g. `FOO=bar rm -rf /`)
- **WHEN** the classifier evaluates it
- **THEN** it SHALL still escalate as danger — stripping the leading prefix SHALL NOT downgrade it

### Requirement: git-paw managed helper-script invocations classify as safe

The classifier SHALL classify as safe a command whose normalized leading verb
resolves to one of git-paw's own managed helper scripts under `.git-paw/scripts/`
(`broker.sh`, `sweep.sh`, `docs-fetch.sh`), because git-paw authors these scripts
and they perform only bounded coordination actions. The match SHALL be on the managed-script path (as
identified by `is_managed_path`), whether invoked directly (`.git-paw/scripts/broker.sh …`)
or via an interpreter (`bash .git-paw/scripts/broker.sh …`). This rule SHALL be
subject to danger-list precedence: a slice that chains a managed-script invocation
with a danger-class operation SHALL still escalate as danger.

#### Scenario: A bundled broker.sh boot call classifies safe

- **GIVEN** the command `.git-paw/scripts/broker.sh --agent feat-x status booting`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL classify as safe (no escalation)

#### Scenario: A bundled sweep.sh call classifies safe

- **GIVEN** the command `.git-paw/scripts/sweep.sh status-publish`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL classify as safe

#### Scenario: A managed-script invocation chained with a danger command still escalates

- **GIVEN** the command `.git-paw/scripts/sweep.sh snapshot && rm -rf /`
- **WHEN** the classifier evaluates it
- **THEN** it SHALL escalate as danger

### Requirement: Writes under a repository `.git/` directory escalate as danger

The classifier SHALL classify as a danger-class escalation — terminal, never
auto-approved, with the same precedence as the curated danger-list — any filesystem
prompt (write / edit / create / delete) or shell command slice that targets a path
resolving inside a repository `.git/` directory (git's own metadata, including
`.git/info/exclude`, `.git/config`, and `.git/hooks/`).
Read-only operations SHALL NOT match this rule. This prevents an agent, or an
auto-approval sweep, from silently mutating git's local configuration — invisible
to teammates and able to subvert the repository's committed ignore/hook scoping.
Target paths SHALL be canonicalized before matching, with the same fail-closed
posture as the worktree boundary check (a path that cannot be canonicalized but
syntactically reaches into a `.git/` directory SHALL be treated as matching). This
rule targets file-path writes; ordinary `git` subcommands are classified by the
git-verb rules and are unaffected.

#### Scenario: Append to .git/info/exclude escalates as danger

- **GIVEN** a prompt whose command slice is `echo '.git-paw/' >> .git/info/exclude`
- **WHEN** the classifier runs
- **THEN** the verdict SHALL be a danger-class escalation
- **AND** no auto-approval keystrokes SHALL ever be dispatched for it

#### Scenario: Write to .git/config escalates as danger

- **GIVEN** a prompt to write `<worktree>/.git/config`
- **WHEN** the classifier runs
- **THEN** the verdict SHALL be a danger-class escalation

#### Scenario: Reading .git metadata is not matched by this rule

- **GIVEN** a prompt whose command slice is `cat .git/config`
- **WHEN** the classifier runs
- **THEN** this rule SHALL NOT match (other classification rules decide the verdict)

### Requirement: No permission grants are seeded into agent CLI settings files

The system SHALL NOT write permission grants into any agent CLI's settings file, SHALL NOT create a vendor-specific settings directory inside a worktree, and SHALL NOT add a version-control exclusion entry for such a directory. Prompt-free agent operation is provided by the CLI's own resolved permission mode together with git-paw's command classifier, which recognises the bundled helper scripts by their stable paths.

#### Scenario: No settings file is written at session start

- **GIVEN** a session starting with any agent CLI
- **WHEN** the session initialises
- **THEN** no permission grants SHALL be written into that CLI's settings file

#### Scenario: No vendor settings directory is created in a worktree

- **GIVEN** a worktree provisioned for any agent CLI
- **WHEN** the worktree is provisioned
- **THEN** no vendor-specific settings directory SHALL be created inside it
- **AND** no version-control exclusion entry SHALL be added for such a directory

#### Scenario: Pre-existing settings files are left untouched

- **GIVEN** a settings file that a previous version of git-paw wrote
- **WHEN** a session starts
- **THEN** that file SHALL be left byte-identical
- **AND** it SHALL NOT be deleted

### Requirement: Prompt-free operation is attributed to the permission mode and classifier

Documentation and bundled guidance SHALL attribute prompt-free agent operation to the CLI's resolved permission mode and git-paw's command classifier, and SHALL NOT attribute it to a seeded settings-file allowlist.

#### Scenario: Supervisor guidance does not credit a seeded allowlist

- **WHEN** the bundled supervisor skill's permission guidance is inspected
- **THEN** it SHALL NOT state that a seeded settings-file allowlist is why an agent's first broker call avoids a permission prompt
- **AND** it SHALL attribute prompt-free operation to the resolved permission mode and the classifier

### Requirement: Settings path remains available to memory isolation

Removing permission seeding SHALL NOT remove or alter the use of a CLI's configured settings path for memory isolation. The parent directory of a configured settings path SHALL continue to participate in the memory-isolation set.

#### Scenario: Memory isolation still uses the settings path parent

- **GIVEN** a configured CLI with a settings path
- **WHEN** the memory-isolation set is computed
- **THEN** the parent directory of that settings path SHALL still be included

#### Scenario: Settings path still parses and round-trips

- **WHEN** a configuration declaring a CLI settings path is loaded and re-serialized
- **THEN** the settings path SHALL be preserved

### Requirement: Prompt markers are sourced from the resolved CLI profile

Permission-prompt detection SHALL obtain its approval markers, command-header form, file-prompt pattern, option-line form and broad-grant marker from the prompt-shape profile resolved for the CLI running in the pane being inspected, rather than from constants compiled into the detector. When no profile is configured for that CLI, the embedded default SHALL resolve, so detection outcomes are unchanged from before profiles existed.

This changes only where the literals come from. The detection algorithm, the permission-type classification outcomes, the capture rate limiting, and every safety gate SHALL be unaffected.

#### Scenario: Detection uses the pane's CLI profile

- **GIVEN** a pane running a CLI with a configured prompt-shape profile
- **WHEN** permission-prompt detection runs against that pane
- **THEN** the markers used SHALL come from that CLI's resolved profile

#### Scenario: Unconfigured CLI detects as before

- **GIVEN** a pane running a CLI with no configured profile
- **WHEN** permission-prompt detection runs against that pane
- **THEN** the embedded default markers SHALL be used
- **AND** the detection outcome SHALL match the outcome before profiles were introduced

#### Scenario: Safety gates unaffected by profile sourcing

- **GIVEN** any resolved profile
- **WHEN** a captured prompt is classified
- **THEN** the danger determination, protected-path rules, worktree-boundary rules and broad-grant eligibility SHALL be evaluated from compiled logic
- **AND** SHALL NOT be influenced by profile content


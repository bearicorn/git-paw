## MODIFIED Requirements

### Requirement: Whitelist of safe command classes

The system SHALL maintain an explicit whitelist of command prefixes that are eligible for auto-approval, and SHALL NOT auto-approve anything outside the whitelist. The built-in whitelist SHALL include the **read-mostly verb allowlist** — commands whose leading verb is one of `curl`, `cat`, `ls`, `grep`, `rg`, `git`, `echo`, `sed`, `awk`, `find`, `wc`, `head`, `tail`, `jq`, `mkdir`, `touch`, `openspec`, `just`, `export`, `tmux`, `env`. A whitelist match SHALL be subordinate to the danger-list: when the curated danger-list (see "Curated danger-list escalates to human") matches the same command, the command SHALL escalate regardless of any whitelist match.

#### Scenario: Default whitelist

- **GIVEN** the default supervisor configuration
- **WHEN** `default_safe_commands()` is queried
- **THEN** the result SHALL contain at minimum:
  - `cargo fmt`
  - `cargo clippy`
  - `cargo test`
  - `cargo build`
  - `git commit`
  - `git push`
  - `curl http://127.0.0.1:` (broker localhost)

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

## ADDED Requirements

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

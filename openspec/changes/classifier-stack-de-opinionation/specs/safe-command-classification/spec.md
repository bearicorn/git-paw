## MODIFIED Requirements

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

## ADDED Requirements

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

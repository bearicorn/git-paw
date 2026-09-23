## MODIFIED Requirements

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

## REMOVED Requirements

### Requirement: Allowlist prevents permission prompts

**Reason**: Removed in v0.16.0. git-paw no longer writes permission grants into CLI settings
files, so there is no seeded curl allowlist to stop a prompt. This contradicts *No permission
grants are seeded into agent CLI settings files*.
**Migration**: None needed. Agent CLIs avoid prompts through their resolved permission mode
(`[supervisor] agent_approval` and the approval flags), and the classifier recognises the bundled
`broker.sh` / `sweep.sh` helpers by path (*git-paw managed helper-script invocations classify as
safe*).

### Requirement: Allowlist updates

**Reason**: Removed in v0.16.0. There is no seeded curl allowlist left to regenerate when the
broker URL or endpoints change.
**Migration**: None needed. The bundled helpers find the broker URL at run time, and the
classifier matches them by script path, so endpoint or URL changes need no allowlist update.

### Requirement: Helper allowlist seeded per agent worktree

**Reason**: Removed in v0.16.0. git-paw no longer writes `<worktree>/.claude/settings.json`.
This contradicts *No permission grants are seeded into agent CLI settings files* (no vendor
settings directory is created in a worktree).
**Migration**: None needed. The helper scripts are still provisioned into each worktree, and the
classifier plus the CLI's resolved permission mode replace the per-worktree grant.

### Requirement: Config-driven broker-curl seeding for custom CLIs

**Reason**: Removed in v0.16.0. git-paw does not write to any `[clis.<name>].settings_path`.
That field is now used only for memory isolation (*Settings path remains available to memory
isolation*).
**Migration**: None needed. Keep `settings_path` configured for memory isolation, and set the
custom CLI's permission behaviour with `[clis.<name>]` approval flags or `agent_approval`.

### Requirement: Never create a CLI's config directory

**Reason**: Removed in v0.16.0. The constraint only made sense alongside settings-file seeding,
which no longer exists. *No permission grants are seeded into agent CLI settings files* is
stricter: no settings write at all.
**Migration**: None needed.

### Requirement: Seeding is idempotent, deduped, and non-fatal

**Reason**: Removed in v0.16.0. There is no seeding step left to be idempotent, deduped or
non-fatal.
**Migration**: None needed. Settings files an older git-paw wrote are left byte-identical
(*No permission grants are seeded into agent CLI settings files*); delete them by hand if they
are unwanted.

### Requirement: Common dev allowlist seeded on supervisor start

**Reason**: Removed in v0.16.0. The supervisor no longer merges the dev allowlist into
`<repo>/.claude/settings.json`. `[supervisor.common_dev_allowlist] enabled` is parsed for
compatibility and has no effect.
**Migration**: The same patterns (universal preset, `stacks`, `extra`) are composed into the
supervisor's auto-approve whitelist (*Whitelist of safe command classes*), so declared dev-loop
commands auto-approve in a stalled pane without any settings-file entry.

### Requirement: Merge semantics preserve existing entries

**Reason**: Removed in v0.16.0. Without settings-file seeding, there is nothing to merge.
**Migration**: None needed. Existing settings files are never modified.

### Requirement: Per-CLI placement (Claude / config-driven settings paths)

**Reason**: Removed in v0.16.0. git-paw writes the dev allowlist to no settings file, neither
`<repo>/.claude/settings.json` nor a configured `settings_path`.
**Migration**: Declare toolchain verbs with `[supervisor.common_dev_allowlist] stacks` / `extra`
(or `[supervisor.auto_approve] safe_commands`). They feed the auto-approve whitelist.

### Requirement: Per-worktree placement for agent panes

**Reason**: Removed in v0.16.0. git-paw does not write `<worktree>/.claude/settings.json` or add
a worktree-local exclude for it. This contradicts *No permission grants are seeded into agent CLI
settings files*.
**Migration**: None needed. Agent panes run under the CLI's resolved permission mode, and stalled
prompts are decided by the classifier using the same resolved dev-allowlist patterns.

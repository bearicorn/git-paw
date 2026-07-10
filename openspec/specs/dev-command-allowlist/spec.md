# dev-command-allowlist Specification

## Purpose
Seeds a curated, stack-neutral set of common dev-command prefix grants into the CLI's `allowed_bash_prefixes` on supervisor start (config-gated, broker-independent, non-fatal on failure) so routine dev-loop commands prompt at most once, with opt-in named stack presets (`rust`, `node`, `python`, `go`) and a user `extra` list layered on top via idempotent, dedup-preserving merge.
## Requirements
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


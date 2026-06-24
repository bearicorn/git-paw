# dev-command-allowlist (delta)

## MODIFIED Requirements

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

## ADDED Requirements

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

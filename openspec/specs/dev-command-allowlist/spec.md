# dev-command-allowlist Specification

## Purpose
TBD - created by archiving change common-dev-allowlist-preset. Update Purpose after archive.
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
list of prefix patterns in source code (not config-driven). The preset
SHALL contain exactly the following patterns (order is irrelevant; the
set is what matters):

- **Cargo**: `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt`,
  `cargo check`, `cargo tree`, `cargo deny`, `cargo update`
- **Git read-only**: `git status`, `git log`, `git diff`, `git show`,
  `git fetch`
- **Git write (non-destructive)**: `git commit`, `git push`,
  `git pull`, `git merge`, `git stash`, `git add`, `git restore`,
  `git rm`
- **Just**: `just`
- **mdBook**: `mdbook build`
- **OpenSpec**: `openspec validate`, `openspec new`, `openspec archive`,
  `openspec list`, `openspec status`, `openspec instructions`
- **Search (read-only)**: `find`, `grep`, `sed -n`

The preset SHALL NOT contain any of the following (intentional
exclusions; rationale in `design.md` D3):

- `cargo install`, `cargo run`, `cargo bench`
- `git rebase`, `git reset`, `git checkout`, `git branch -D`
- `git push --force`, `git push -f`
- `find ... -exec` patterns (the bare `find` prefix is included; users
  wanting `-exec` patterns add them via `extra`)
- `sed` without `-n` (write-mode sed)
- Package managers other than cargo (`npm`, `pnpm`, `yarn`, `deno`,
  `bun`, `uv`, `pip`, `pipx`, `gem`) — users add via `extra`

The constant SHALL be exported from the dev-allowlist module so tests
can assert its content. The constant SHALL be the single source of
truth: no other location in the codebase may hard-code preset patterns.

#### Scenario: Preset constant contains all required patterns

- **GIVEN** the dev-allowlist module's exported preset constant
- **WHEN** the test inspects its contents
- **THEN** every pattern in the "Standard preset content" list above
  SHALL be present
- **AND** no pattern from the exclusions list SHALL be present

#### Scenario: Seeded entries match the preset constant

- **GIVEN** a fresh `.claude/settings.json` (file absent or empty)
- **WHEN** the dev-allowlist seeder runs with empty `extra`
- **THEN** the resulting `allowed_bash_prefixes` array SHALL contain
  exactly the patterns from the preset constant (no extra, no missing)

### Requirement: User-extensible allowlist via `extra` field

The system SHALL append any patterns from
`[supervisor.common_dev_allowlist] extra` to the built-in preset when
seeding. The `extra` field SHALL accept arbitrary strings; the system
SHALL NOT validate or filter them.

User-supplied `extra` patterns SHALL be appended **after** the preset
in the resulting `allowed_bash_prefixes` array (order is
informational; Claude's allowlist is a set).

Duplicate patterns (an `extra` entry that matches an existing entry,
whether from the preset or from a prior session's seeding) SHALL NOT
be added a second time. This matches the existing
`curl-allowlist` de-duplication contract.

#### Scenario: Extra patterns appended to preset

- **GIVEN** a `.git-paw/config.toml` with
  `[supervisor.common_dev_allowlist] extra = ["pnpm test", "deno fmt"]`
- **WHEN** the dev-allowlist seeder runs on a fresh `.claude/settings.json`
- **THEN** the resulting `allowed_bash_prefixes` SHALL contain every
  preset pattern PLUS `"pnpm test"` AND `"deno fmt"`

#### Scenario: Duplicate extra entry not added twice

- **GIVEN** an existing `.claude/settings.json` already containing
  `"cargo build"` in `allowed_bash_prefixes`
- **AND** `extra = ["cargo build"]` (matches an existing entry)
- **WHEN** the seeder runs
- **THEN** `"cargo build"` SHALL appear exactly once in the resulting
  array (no duplicate)

#### Scenario: Extra entries not validated

- **GIVEN** `extra = ["this is a nonsense string $$"]`
- **WHEN** the seeder runs
- **THEN** the seeder SHALL succeed
- **AND** the nonsense entry SHALL be present in
  `allowed_bash_prefixes` (Claude's matcher will simply never hit it)

### Requirement: Per-CLI placement (Claude / `~/.claude-oss` in v0.5.0)

The system SHALL write the merged allowlist to
`<repo>/.claude/settings.json` on every supervisor start where the
feature is enabled.

When the directory `~/.claude-oss/` exists at session start (the
alt-config dogfood pattern from `prompt-submit-fix`), the system
SHALL ALSO merge the same allowlist into
`~/.claude-oss/settings.json` using the same merge semantics.

When `~/.claude-oss/` does not exist, the system SHALL NOT create it.

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

#### Scenario: Writes to `~/.claude-oss/settings.json` when directory exists

- **GIVEN** the directory `~/.claude-oss/` exists at session start
- **AND** the feature is enabled
- **WHEN** the seeder runs
- **THEN** the file `~/.claude-oss/settings.json` SHALL ALSO contain
  the merged allowlist with the same entries as
  `<repo>/.claude/settings.json`

#### Scenario: Does not create `~/.claude-oss/` when absent

- **GIVEN** the directory `~/.claude-oss/` does not exist at session start
- **WHEN** the seeder runs
- **THEN** no `~/.claude-oss/` directory SHALL be created
- **AND** only `<repo>/.claude/settings.json` SHALL be written

#### Scenario: No write to other CLI configs

- **GIVEN** the user has `~/.codex/config.toml` and `~/.gemini/`
  present at session start
- **WHEN** the seeder runs
- **THEN** neither file/directory SHALL be modified by this seeder

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


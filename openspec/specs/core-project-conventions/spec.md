# core-project-conventions Specification

## Purpose
Repo-wide contributor conventions and the doc accuracy they demand: the `AGENTS.md` approved-dependencies table stays in sync with `[dependencies]`/`[dev-dependencies]` in `Cargo.toml` (including `dirs` recorded as an intentional exclusion), the commit-scope enumeration and its compound-scope form, the mdBook architecture chapter's module list and its changelog include, and the README Features section's enumeration of the current user-facing surface — so contributors can trust the project docs to match the shipped code.

## Requirements
### Requirement: README features section enumerates the current user-facing surface

`README.md`'s Features section SHALL include entries describing
every user-facing capability so users can
discover the feature set from a single page. The Features list
SHALL include (at minimum) entries for: `--specs-format`,
`--no-supervisor`, `start --force`, the Spec Kit backend
(`[specs] type = "speckit"`), the `[governance]` config table,
the `[supervisor.conflict]` config table, the
`[supervisor.auto_approve]` config table, the
`[supervisor.learnings_config]` config table, `agent.intent`
forward coordination, automatic conflict detection
(forward / in-flight / ownership), and learnings mode (the
`.git-paw/session-learnings.md` output file).

#### Scenario: README Features section mentions Spec Kit

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `Spec Kit` (case-insensitive)
- **AND** it contains the substring `speckit` (the TOML value)

#### Scenario: README Features section mentions forward coordination and conflict detection

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `agent.intent`
- **AND** it contains the substring `conflict detection` (case-insensitive)

#### Scenario: README Features section mentions learnings mode

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `learnings` (case-insensitive)
- **AND** it contains the substring `.git-paw/session-learnings.md`

#### Scenario: README Features section mentions every CLI flag

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `--specs-format`
- **AND** it contains the substring `--no-supervisor`
- **AND** it contains the substring `--force` (in a `start --force` context)

### Requirement: AGENTS.md dependency table matches `Cargo.toml`

`AGENTS.md`'s approved-dependencies table SHALL list every
production dependency declared in `[dependencies]` of `Cargo.toml`
at archive time and every dev dependency declared in
`[dev-dependencies]`. The production additions SHALL be
present: `schemars`, `serde_yaml`, `chrono`, and `regex` (with
appropriate version suffixes matching the manifest).

#### Scenario: AGENTS.md table lists prod dependencies

- **WHEN** the AGENTS.md Dependencies table is inspected
- **THEN** it contains the substring `schemars`
- **AND** it contains the substring `serde_yaml`
- **AND** it contains the substring `chrono`
- **AND** it contains the substring `regex`

### Requirement: AGENTS.md documents the `dirs` crate removal

`AGENTS.md` SHALL NOT list the upstream `dirs` crate as an
approved production dependency. Instead, AGENTS.md SHALL include a
paragraph under the Dependencies section explaining that the
`dirs` crate was removed because its transitive license
chain fails `just deny`, and that the project now uses an
in-tree `src/dirs.rs` module for platform XDG paths. The
paragraph SHALL instruct future contributors NOT to re-add the
`dirs` crate.

#### Scenario: AGENTS.md does not list dirs as an approved dep

- **WHEN** the AGENTS.md Dependencies table is inspected
- **THEN** the table does NOT contain `dirs` as an approved
  production dependency row

#### Scenario: AGENTS.md explains the dirs swap

- **WHEN** the AGENTS.md Dependencies section is inspected
- **THEN** it contains text describing the `dirs` crate's removal
  and its replacement by `src/dirs.rs`
- **AND** it instructs contributors not to re-add the `dirs` crate

### Requirement: AGENTS.md scopes list covers the scopes and compound forms

`AGENTS.md`'s Commit Conventions section's scope enumeration SHALL include the scopes used in shipped commits. At minimum the scope list SHALL include: `user-guide`, `worktree`, `governance`, `learnings`, and `pause`. The section SHALL explicitly document compound scopes of the form `<a>,<b>,<c>` (comma-separated scopes inside the parentheses) as permitted.

#### Scenario: AGENTS.md scopes list mentions the scopes

- **WHEN** the AGENTS.md Commit Conventions scope list is inspected
- **THEN** it contains the substring `user-guide`
- **AND** it contains the substring `worktree`
- **AND** it contains the substring `governance`
- **AND** it contains the substring `learnings`

#### Scenario: AGENTS.md documents compound scopes

- **WHEN** the AGENTS.md Commit Conventions section is inspected
- **THEN** it contains an example or explicit statement permitting
  compound scope form `(<a>,<b>,...)`
- **AND** the example uses commas (no whitespace between scopes is
  required, but the comma separator is explicit)

### Requirement: mdBook architecture chapter has an accurate module list

`docs/src/architecture.md` SHALL describe every Rust module that
ships in `src/` at archive time. The chapter SHALL NOT reference
modules that do not exist in `src/`. Specifically, the chapter
SHALL NOT contain the substrings `src/broker/state.rs` or
`src/broker/flush.rs` (modules that were referenced in v0.4 docs
but do not exist in the current source tree). The chapter SHALL
include references to the module additions
(`src/supervisor/`, `src/broker/conflict.rs`,
`src/broker/learnings.rs`, `src/broker/watcher.rs`,
`src/broker/delivery.rs`, `src/broker/publish.rs`,
`src/specs/resolve.rs`, `src/specs/speckit.rs`).

#### Scenario: Architecture chapter does NOT reference nonexistent broker modules

- **WHEN** `docs/src/architecture.md` is inspected
- **THEN** it does NOT contain the substring `src/broker/state.rs`
- **AND** it does NOT contain the substring `src/broker/flush.rs`

#### Scenario: Architecture chapter references the module additions

- **WHEN** `docs/src/architecture.md` is inspected
- **THEN** it contains the substring `src/broker/conflict.rs`
- **AND** it contains the substring `src/broker/learnings.rs`
- **AND** it contains the substring `src/specs/speckit.rs`
- **AND** it contains a reference to the `src/supervisor/` subtree

### Requirement: mdBook changelog chapter includes the project changelog

`docs/src/changelog.md` SHALL render the contents of the
project's root `CHANGELOG.md` rather than maintaining a separate
copy. The chapter file SHALL contain the mdBook
`{{#include ../../CHANGELOG.md}}` directive (or equivalent
preprocessor directive) so that future `git cliff` regenerations
of `CHANGELOG.md` automatically flow through to the rendered
mdBook output.

#### Scenario: Changelog chapter is an include of the root CHANGELOG.md

- **WHEN** `docs/src/changelog.md` is inspected
- **THEN** it contains the substring `{{#include ../../CHANGELOG.md}}`

#### Scenario: Changelog chapter does NOT hand-maintain `[Unreleased]` content

- **WHEN** `docs/src/changelog.md` is inspected
- **THEN** it does NOT contain a hand-maintained `[Unreleased]` section header (the rendered output is sourced entirely from the included file)

### Requirement: AGENTS.md dependency table SHALL list the dependencies and note `dirs` as intentionally absent

The top-level `AGENTS.md` "Dependencies" table SHALL include rows for `schemars`, `serde_yaml`, `chrono`, and `regex` — the four dependencies added during the cycle. Each row's "Purpose" cell SHALL be one sentence (e.g. `schemars` → "JSON Schema derivation for governance config validation").

The `dirs` row SHALL be moved out of the approved-dependencies table and into a "Notable exclusions" sub-section beneath the table. The exclusion entry SHALL state: "Replaced by a homegrown `src/dirs.rs` because the upstream crate's license is not FOSS-compatible. Do not re-add."

#### Scenario: AGENTS.md lists schemars in dependency table

- **WHEN** `AGENTS.md` is read
- **THEN** the dependencies table SHALL contain a row with `schemars` in the Crate column
- **AND** the Purpose column for that row SHALL mention JSON Schema or governance config

#### Scenario: AGENTS.md lists serde_yaml, chrono, regex

- **WHEN** the dependencies table is inspected
- **THEN** rows for `serde_yaml`, `chrono`, and `regex` SHALL exist
- **AND** each row's Purpose cell SHALL be non-empty

#### Scenario: AGENTS.md notes `dirs` as intentionally absent

- **WHEN** `AGENTS.md` is read
- **THEN** the `dirs` row SHALL NOT appear in the approved-dependencies table
- **AND** a separate "Notable exclusions" section SHALL describe `dirs` as replaced by `src/dirs.rs` due to a non-FOSS license

### Requirement: AGENTS.md commit-conventions scopes SHALL include the scope names and document the compound-scope form

The "Commit Conventions" section of `AGENTS.md` SHALL list the following scope names in addition to the v0.4 set: `user-guide`, `worktree`, `governance`, `learnings`, `pause`. The section SHALL also document the compound-scope syntax `(scope1,scope2,...)` with at least one inline example (e.g. `feat(cli,config): ...`).

#### Scenario: Scopes line includes the scope names

- **WHEN** the AGENTS.md Scopes line is inspected
- **THEN** it SHALL contain each of `user-guide`, `worktree`, `governance`, `learnings`, `pause` (as inline-code-style backtick names)

#### Scenario: Compound-scope syntax is documented

- **WHEN** the Commit Conventions section is inspected
- **THEN** there SHALL be at least one example using compound-scope syntax `(scope1,scope2)`
- **AND** the section SHALL state that compound scopes are permitted when a commit legitimately touches more than one scope

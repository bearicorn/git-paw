## Purpose

Record every deprecated git-paw surface in one registry, warn once per use on stderr without changing output or exit codes, and keep deprecated items from outliving their scheduled removal.

## ADDED Requirements

### Requirement: Deprecated items are recorded in one registry

git-paw SHALL record every deprecated user-visible item in a single registry in code. Each entry SHALL carry the item as users write it, its kind (flag, configuration key, skill name, wire value, or output field), the release that deprecated it, its replacement (or, for an item retired with no effect and nothing to replace it, such as the `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` tables deprecated in 0.17.0, a no-effect marker instead), and the release that removes it, or no removal release when the item stays readable for the life of the current format version. Every warning, `git paw config check` finding, `git paw doctor` row and documentation table about a deprecated item SHALL be produced from this registry, and no other list of deprecated items SHALL exist in the code.

#### Scenario: Each entry is complete

- **WHEN** the registry is enumerated
- **THEN** every entry SHALL have a non-empty item, a kind, a deprecating release and either a replacement or the no-effect marker
- **AND** every removal release that is present SHALL be a valid semantic version greater than the deprecating release

#### Scenario: Config findings come from the registry

- **GIVEN** a registry entry for a configuration key
- **AND** a config file that sets that key
- **WHEN** `git paw config check` runs
- **THEN** its finding for the key SHALL name the replacement and the deprecating release recorded in the registry

### Requirement: Using a deprecated item warns once through one helper

When a deprecated input item is used — a flag, a configuration key, a skill name, or a wire value received by the broker — git-paw SHALL print exactly one line to stderr per item and context per process, through a single helper, of the form `warning: [<context>: ]<item> is deprecated since <release>[ and will be removed in <release>]; use <replacement> instead`, where the context names the file for a configuration key; for an entry with the no-effect marker the line SHALL end `; it has no effect and can be removed` instead of naming a replacement. Configuration keys that `core-configuration` and `supervisor-config` group into one warning line per file (a version-1 key family, or the deprecated auto-approval tables) SHALL share that one line, which names each grouped item. The warning SHALL NOT be written to stdout, SHALL NOT change the exit code, and SHALL NOT appear when the item is not used. A deprecated flag spelling SHALL be detected from the raw command-line arguments up to a `--` terminator, since the parser does not report which spelling matched. Output fields SHALL be published in the registry but SHALL NOT produce a runtime warning.

#### Scenario: A deprecated flag warns once

- **WHEN** `git paw start --supervisor --dry-run --branches feat/a` runs
- **THEN** stderr SHALL contain exactly one line naming `--supervisor`, its deprecating release and `--coordinated`

#### Scenario: The warning leaves stdout and the exit code alone

- **GIVEN** a version-1 config that sets `[supervisor] test_command`
- **WHEN** `git paw status --json` runs, and again after the key is renamed to `[orchestrator.gates] test_command`
- **THEN** both runs SHALL print byte-identical stdout and exit with the same code
- **AND** only the first SHALL print a deprecation warning on stderr

#### Scenario: A no-effect item warns without a replacement

- **GIVEN** a repository config that declares `[supervisor.auto_approve] enabled = true`
- **WHEN** `git paw status` runs
- **THEN** stderr SHALL contain one line naming the file, `[supervisor.auto_approve]` and release 0.17.0, ending `it has no effect and can be removed`

#### Scenario: Arguments after a terminator are not inspected

- **GIVEN** a raw argument list `start -- --supervisor`
- **WHEN** the deprecated-flag scan runs
- **THEN** it SHALL report no deprecated flag

#### Scenario: No warning without use

- **WHEN** `git paw start --coordinated --dry-run --branches feat/a` runs with a version-2 config
- **THEN** stderr SHALL contain no deprecation warning

### Requirement: A deprecated item cannot outlive its removal release

A test SHALL fail when the registry holds an entry whose removal release is less than or equal to the crate's own version, naming the entry, so an item scheduled for removal in a release is removed in that release.

#### Scenario: An overdue entry fails the build

- **GIVEN** a registry entry with removal release `1.0.0`
- **WHEN** the guard runs with crate version `1.0.0`
- **THEN** the guard SHALL fail naming the entry
- **AND** with crate version `0.21.0` it SHALL pass

### Requirement: The registry is published

The CLI reference SHALL carry a "Deprecations" section listing every registry entry with its kind, deprecating release, replacement (or "no effect" for an entry with the no-effect marker) and removal release (or "read through migration" when it has none). A test SHALL fail when an entry is missing from that section.

#### Scenario: Every entry is documented

- **WHEN** the CLI reference's Deprecations section is compared with the registry
- **THEN** every registry entry SHALL appear in it with its replacement

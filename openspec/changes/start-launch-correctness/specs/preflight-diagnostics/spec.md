## ADDED Requirements

### Requirement: Doctor surfaces the effective spec-driven worker CLI

`git paw doctor` SHALL report the CLI that spec-driven worker panes would resolve to (per the spec-driven CLI resolution chain, evaluated for the current config and any discoverable specs) and SHALL warn when that effective CLI does not match a configured `default_spec_cli`. This surfaces a silent mis-resolution — such as supervisor-mode workers launching with an unexpected, unsandboxed CLI — rather than passing green. The check SHALL be read-only and SHALL degrade to informational (not a hard failure) when no specs or no `default_spec_cli` are configured.

#### Scenario: Doctor warns when the effective spec CLI differs from default_spec_cli

- **GIVEN** a config with `default_spec_cli = "claude-oss"` and a resolution path that would launch spec workers with a different CLI
- **WHEN** `git paw doctor` runs
- **THEN** it SHALL warn that the effective spec-worker CLI does not match `default_spec_cli`

#### Scenario: Doctor passes when the effective spec CLI matches

- **GIVEN** a config with `default_spec_cli = "claude-oss"` and a resolution path that launches spec workers with `"claude-oss"`
- **WHEN** `git paw doctor` runs
- **THEN** the spec-CLI check SHALL pass (no warning)

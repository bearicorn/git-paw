## ADDED Requirements

### Requirement: Coordination skill teaches file-reading and scratch-file discipline

The embedded coordination skill SHALL include a section instructing the agent how to inspect files and capture command output efficiently, without naming any specific CLI's tools.

The section SHALL instruct the agent to prefer its own CLI's native file-read and search facilities over shell text-extraction pipelines (constructs that pipe output through line-slicing or field-extracting utilities) when inspecting source files. It SHALL explain the two costs that motivate this: such a pipeline typically requires a permission approval, stalling the agent; and when that approval is granted durably it is keyed on the exact command, producing an allowlist entry too specific to ever match again.

The section SHALL instruct the agent, when capturing command output to a file across repeated iterations, to reuse one scratch path rather than writing a fresh numbered file each time, because a durable approval keyed on the exact path does not carry over to a new filename and each iteration therefore costs another approval.

The guidance SHALL be expressed in terms of capability classes rather than named tools, so it applies to any CLI the consumer runs. The embedded content SHALL NOT name a specific CLI, a specific tool belonging to one CLI, or a specific vendor.

#### Scenario: The skill teaches preferring native read tools

- **WHEN** the embedded coordination skill content is inspected
- **THEN** it SHALL contain guidance to prefer the CLI's own file-read and search facilities over shell text-extraction pipelines for inspecting source
- **AND** it SHALL state that such pipelines stall on approval

#### Scenario: The skill explains the dead-grant cost

- **WHEN** the embedded coordination skill content is inspected
- **THEN** it SHALL explain that a durable approval keyed on an exact command produces an allowlist entry that will not match a subsequent, differing command

#### Scenario: The skill teaches scratch-path reuse

- **WHEN** the embedded coordination skill content is inspected
- **THEN** it SHALL instruct the agent to reuse a single scratch path across iterations rather than writing numbered files
- **AND** it SHALL state that a new filename costs a fresh approval

#### Scenario: The guidance names no specific CLI or tool

- **WHEN** the embedded coordination skill's file-reading section is inspected
- **THEN** it SHALL NOT contain the name of any specific AI CLI
- **AND** it SHALL NOT contain the name of any tool belonging to one specific CLI
- **AND** it SHALL express the instruction in terms of capability classes available to any CLI

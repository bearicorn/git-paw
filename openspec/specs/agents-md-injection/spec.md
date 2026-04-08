## Purpose

Inject and manage a marker-delimited git-paw section in AGENTS.md files, supporting detection, generation, replacement, and file-level injection of git-paw configuration content for AI coding CLIs.

## Requirements

### Requirement: Detect existing git-paw section

The system SHALL detect whether a markdown string contains a git-paw section by checking for the `<!-- git-paw:start` prefix.

#### Scenario: Content with git-paw section
- **WHEN** `has_git_paw_section()` is called with content containing `<!-- git-paw:start`
- **THEN** it SHALL return `true`

#### Scenario: Content without git-paw section
- **WHEN** `has_git_paw_section()` is called with content that does not contain `<!-- git-paw:start`
- **THEN** it SHALL return `false`

#### Scenario: Empty content
- **WHEN** `has_git_paw_section()` is called with an empty string
- **THEN** it SHALL return `false`

### Requirement: Generate git-paw section content

The system SHALL generate a marker-delimited section containing git-paw instructions for AI coding CLIs.

#### Scenario: Generated section has markers
- **WHEN** `generate_git_paw_section()` is called
- **THEN** the result SHALL start with a line containing `<!-- git-paw:start` and end with a line containing `<!-- git-paw:end -->`

#### Scenario: Generated section contains guidance
- **WHEN** `generate_git_paw_section()` is called
- **THEN** the result SHALL contain guidance about git-paw configuration and parallel sessions

### Requirement: Replace existing git-paw section

The system SHALL replace the content between markers (inclusive) with a new section.

#### Scenario: Replace section with both markers present
- **WHEN** `replace_git_paw_section()` is called with content that has both start and end markers
- **THEN** the content from start marker through end marker (inclusive) SHALL be replaced with the new section

#### Scenario: Content before and after markers is preserved
- **WHEN** content exists both before `<!-- git-paw:start` and after `<!-- git-paw:end -->`
- **THEN** `replace_git_paw_section()` SHALL preserve all content outside the markers

#### Scenario: Replace when end marker is missing
- **WHEN** `replace_git_paw_section()` is called with content that has a start marker but no end marker
- **THEN** everything from the start marker to EOF SHALL be replaced with the new section

### Requirement: Inject section into content string

The system SHALL append a section if no git-paw section exists, or replace the existing one.

#### Scenario: Inject into content without existing section
- **WHEN** `inject_into_content()` is called with content that has no git-paw section
- **THEN** the section SHALL be appended to the content

#### Scenario: Inject into content with existing section
- **WHEN** `inject_into_content()` is called with content that already has a git-paw section
- **THEN** the existing section SHALL be replaced with the new one

#### Scenario: Inject into empty content
- **WHEN** `inject_into_content()` is called with an empty string
- **THEN** the result SHALL contain only the new section

### Requirement: Inject section into file

The system SHALL read a file, inject the section, and write the result back.

#### Scenario: File exists without git-paw section
- **WHEN** `inject_section_into_file()` is called on a file without a git-paw section
- **THEN** the section SHALL be appended and the file written

#### Scenario: File exists with git-paw section
- **WHEN** `inject_section_into_file()` is called on a file with an existing git-paw section
- **THEN** the section SHALL be replaced and the file written

#### Scenario: File does not exist
- **WHEN** `inject_section_into_file()` is called with a path that does not exist
- **THEN** the file SHALL be created containing only the section

#### Scenario: File is not writable
- **WHEN** `inject_section_into_file()` is called on a read-only file
- **THEN** it SHALL return `PawError::AgentsMdError` with a message mentioning the file path

### Requirement: Appended section is separated from existing content

When appending a section to existing content, the system SHALL ensure proper spacing.

#### Scenario: Existing content ends with newline
- **WHEN** content ends with `\n` and the section is appended
- **THEN** a blank line SHALL separate the existing content from the section

#### Scenario: Existing content does not end with newline
- **WHEN** content does not end with `\n` and the section is appended
- **THEN** a newline and blank line SHALL separate the existing content from the section

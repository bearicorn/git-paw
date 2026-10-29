## ADDED Requirements

### Requirement: Headless agent-facing texts are bundled agnostic assets

The headless boot addendum, the interrupted-turn continuation text and the inbox wake text SHALL be authored in bundled assets (`assets/headless-boot-addendum.md`, `assets/headless-continue.md`, `assets/headless-wake.md`) and loaded at compile time, not written as string literals in code. They SHALL name no agent CLI, vendor, programming-language stack, build tool, commit convention or git-paw helper-script path, and SHALL refer to the worker's coordination channel only as its instructions, so the same text is correct with or without a broker. The no-language-leak audit SHALL cover all three.

#### Scenario: The headless texts pass the audit

- **WHEN** the no-language-leak audit runs over the three headless assets
- **THEN** it SHALL find no forbidden stack token, commit-convention prefix, vendor name or helper-script path in any of them

#### Scenario: The texts are not compiled literals

- **WHEN** the rendering code for the addendum, continuation and wake texts is inspected
- **THEN** each text SHALL be loaded from its asset with `include_str!`

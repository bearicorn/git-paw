## ADDED Requirements

### Requirement: Budget escalation text is rendered from bundled assets

The question and hand-off text for the budget-exhausted and cost-cap-unenforceable escalations SHALL be rendered from bundled templates under `assets/`, with the cap, the usage and the detail substituted. The text SHALL say that a budget cap stopped or cannot govern the run, and SHALL NOT describe an agent as stalled or ask anyone to review a pane.

#### Scenario: A budget breach question names the cap

- **GIVEN** an escalation with the budget-exhausted verdict and the detail `budget_max_tokens 1000000 reached: 1000400 tokens of known usage`
- **WHEN** its question text is rendered
- **THEN** the text SHALL contain that detail and say the budget cap was reached
- **AND** it SHALL NOT contain `stalled` or `review the pane`

#### Scenario: The template is the bundled asset

- **WHEN** the budget-exhausted question is rendered with placeholders left empty
- **THEN** its fixed prose SHALL equal the bundled asset's text

### Requirement: Widely built structs are built from a default outside their module

A struct built by a struct literal at more than three sites outside its defining module SHALL have a `Default` or a fixture builder, and those literals SHALL end with `..Default::default()` or use the builder, so adding a field edits only the struct, its default and the code that reads the field. Structs that persist to disk or cross the wire SHALL keep exhaustive literals in production code. A source-audit test SHALL enforce this.

#### Scenario: Adding an optional field touches no test literal

- **GIVEN** a struct covered by the rule
- **WHEN** a field with a default value is added to it
- **THEN** the test and fixture code SHALL compile without edits

#### Scenario: The audit flags an exhaustive literal outside the module

- **GIVEN** a test that builds a covered struct with every field listed and no `..`
- **WHEN** the source audit runs
- **THEN** it SHALL fail naming the file, the line and the struct

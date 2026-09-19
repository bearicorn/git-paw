## ADDED Requirements

### Requirement: Publish tool descriptions do not invite peer cherry-picking

The MCP publish tools' parameter and tool descriptions SHALL describe `exports` as public API items announced for peers, without suggesting that peers cherry-pick them. These descriptions are shown to every MCP-capable agent as part of the tool schema and SHALL NOT contradict the coordination skill's prohibition on cherry-picking a peer's commits.

#### Scenario: The exports description does not mention cherry-picking

- **WHEN** the schema of each MCP publish tool that accepts `exports` is inspected
- **THEN** no description in it SHALL say that peers cherry-pick the exported items

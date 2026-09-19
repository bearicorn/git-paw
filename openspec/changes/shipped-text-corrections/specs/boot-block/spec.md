## ADDED Requirements

### Requirement: Boot text does not invite peer cherry-picking

The bundled boot-block templates SHALL describe the `exports` field of a manual artifact publish without suggesting that peers will cherry-pick the announced items. Taking a peer's work onto one's own branch by cherry-picking is prohibited by the coordination skill, because it creates a second copy of a commit the supervisor later merges from the peer's branch and leaves the branch unmergeable; boot text shipped to every agent SHALL NOT contradict that rule. The templates SHALL NOT contain the phrase "cherry-pick" except as a prohibition.

#### Scenario: Boot templates describe exports without cherry-picking

- **WHEN** each bundled boot-block template is inspected
- **THEN** its description of `exports` SHALL NOT say peers cherry-pick the announced items
- **AND** the template SHALL NOT contain "cherry-pick" outside a statement prohibiting it

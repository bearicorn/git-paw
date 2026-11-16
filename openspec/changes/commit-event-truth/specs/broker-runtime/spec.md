## ADDED Requirements

### Requirement: Watch-target withdrawal endpoint

The broker SHALL serve `DELETE /watch` with a JSON body naming a `worktree_path`. For a registered target it SHALL stop that target's watcher before answering, forget the target, and withdraw its agent from the conflict detector, then answer `204`. An unknown path SHALL answer `204` with no effect.

#### Scenario: A withdrawn worktree is no longer watched

- **GIVEN** a registered watch target for `feat-x`
- **WHEN** `DELETE /watch` names its worktree path and the worktree is then deleted
- **THEN** no `agent.status` for `feat-x` SHALL be published after the `204` response

#### Scenario: Withdrawing an unknown path is harmless

- **GIVEN** no watch target at `/tmp/nowhere`
- **WHEN** `DELETE /watch` names `/tmp/nowhere`
- **THEN** the response SHALL be `204` and the broker state SHALL be unchanged

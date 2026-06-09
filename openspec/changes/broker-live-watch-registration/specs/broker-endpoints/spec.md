## ADDED Requirements

### Requirement: Live watch-target registration endpoint

The broker SHALL expose `POST /watch` accepting a JSON body with an
agent id, worktree path, and cli label, and SHALL add that path to its
live filesystem-watch-target set so the watcher begins surfacing the
worktree's activity without a broker restart. The endpoint SHALL be
idempotent: registering an already-watched path SHALL NOT create a
duplicate target. It SHALL bind to loopback only, consistent with the
other broker endpoints.

A hot-added agent (via `git paw add`) registered through `POST /watch`
SHALL appear in `/status` from worktree activity on the same terms as
an agent seeded at `git paw start`, independent of whether its CLI has
self-published a status.

#### Scenario: Registering a target surfaces the worktree via the watcher

- **GIVEN** a running broker and a worktree not among its start-time
  watch targets
- **WHEN** a client `POST`s the worktree (agent id + path + cli) to
  `/watch`
- **AND** that worktree subsequently has uncommitted changes
- **THEN** the watcher SHALL surface the agent in `/status` without a
  broker restart and without requiring the agent's CLI to have
  self-published

#### Scenario: Registration is idempotent

- **GIVEN** a worktree already registered as a watch target
- **WHEN** the same path is `POST`ed to `/watch` again
- **THEN** the broker SHALL NOT create a duplicate target and SHALL
  return success

#### Scenario: Endpoint binds to loopback only

- **WHEN** the broker starts with `/watch` enabled
- **THEN** the endpoint SHALL be reachable only on the loopback
  interface, consistent with `/publish`, `/status`, and `/messages`

## Why

v0.3.0 introduces an HTTP broker so parallel agents can coordinate directly (publish progress, share artifacts, request help from peers). Before any server, dashboard, or skill template can be built, the wire format those components exchange must be defined. This change establishes the broker's message schema as the foundation that every other v0.3.0 change depends on.

## What Changes

- Define three broker message types as a tagged Rust enum:
  - `agent.status` — periodic progress reports (status, modified files, free-form message)
  - `agent.artifact` — completion notice with exported symbols and changed files
  - `agent.blocked` — blocked-on-peer notice with what is needed and from whom
- Implement serde serialization/deserialization with the `type` discriminator matching the wire format above
- Implement validation rules: required fields per variant, non-empty `agent_id`, payload shape constraints
- Implement `Display` formatting suitable for the dashboard's status table (one-line per message)
- Implement a deterministic `BRANCH_ID` slug function used as `agent_id` throughout the broker:
  - lowercase the input
  - replace `/` with `-`
  - replace any character outside `[a-z0-9-_]` with `-`
  - collapse runs of `-` to a single `-`
  - trim leading/trailing `-`
- Add unit tests covering: round-trip serde for each variant, validation success/failure cases, slug edge cases (uppercase, nested paths, unicode, leading/trailing slashes)

## Capabilities

### New Capabilities

- `broker-messages`: Wire format and validation for messages exchanged between agents and the git-paw broker, plus the agent identity slug rule that derives stable `agent_id` values from git branch names.

### Modified Capabilities

<!-- None -->

## Impact

- **New file:** `src/broker/messages.rs` (owned by this change)
- **New file:** `src/broker/mod.rs` will be created here as a minimal module root that re-exports `messages`. The `http-broker` change will extend this `mod.rs` later — this change only adds the `pub mod messages;` line.
- **No new runtime dependencies.** Uses existing `serde` + `serde_json` from the project's approved dependency set.
- **Dependents (Wave 1 and Wave 2):** `http-broker`, `peer-messaging`, `dashboard-tui`, and `skill-templates` all consume types from this module. This change MUST merge first in Wave 1 to unblock the others.
- **No user-facing CLI surface.** Pure library code; no commands, flags, or config fields added.

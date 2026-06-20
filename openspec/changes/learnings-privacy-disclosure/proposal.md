## Why

Learnings mode already does no telemetry — the aggregator only ever writes a local `.git-paw/session-learnings.md`, the broker binds to `127.0.0.1`, and there is no outbound HTTP client in the dependency set. But nothing *tells the user that*. The docs describe learnings as an internal coordination artifact; they never frame its actual purpose: a file the user can **optionally** share with the maintainers to improve git-paw, with the tool collecting nothing on its own. Making the privacy stance and the opt-in sharing invitation explicit is what turns "a file that appears in my repo" into a deliberate, trustworthy feedback loop.

## What Changes

- **Docs (`docs/src/user-guide/learnings.md` + README pointer):** add a clearly-headed privacy & sharing section stating that learnings mode performs **no telemetry**, the output is a purely local file, and it is fully opt-in. Invite users who hit friction worth fixing to open a GitHub issue and share the file — with the explicit caveat to **review it first**, since it contains repo-specific details (branch names, file paths, spec IDs), and a hint that they can use their own LLM/agent CLI to strip or anonymise sensitive details before sharing. Link to the generic GitHub issues page (no special template).
- **CLI notice:** when a session starts with learnings mode enabled (`[supervisor] learnings = true`), git-paw SHALL print a one-time, concise notice to the user stating where the local file is written, that nothing is sent anywhere, and that the file can be reviewed and optionally shared via a GitHub issue to help improve the tool. The notice SHALL NOT print when learnings mode is disabled (the default).
- No change to what the aggregator collects, the file format, the broker, or any wire format. This is a disclosure/UX change only.

## Capabilities

### New Capabilities
<!-- None. -->

### Modified Capabilities
- `learnings-mode`: add a requirement that learnings mode performs no telemetry and that its output is local + opt-in + user-shared-only, surfaced via documentation and a session-start CLI notice when the mode is enabled.

## Impact

- Affected docs: `docs/src/user-guide/learnings.md`, `README.md` (pointer).
- Affected code: `src/main.rs` session-start path (where `learnings_enabled` is already computed) prints the notice; no new modules, no new dependencies.
- Backward compatible: when learnings mode is off (default), behavior is identical — no notice, no new output. The notice is purely additive stdout when the user has already opted in.

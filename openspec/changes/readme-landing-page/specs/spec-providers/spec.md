## REMOVED Requirements

### Requirement: README documents `[specs] type` accepts all three backends

**Reason**: The README becomes a landing page and carries no configuration excerpt. Its configuration section points at the configuration reference.

**Migration**: `core-configuration` already requires `docs/src/configuration/README.md`'s `[specs]` example to document every accepted `type` value. That example is the single place the values are listed.

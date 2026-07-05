## Why

The `git paw init` default `config.toml` template (`generate_default_config`) documents a narrower set of sections than the configuration reference: it has commented stanzas for `[dashboard]`, `[specs]`, `[logging]`, `[broker]`, `[supervisor]` (+ `.tell`/`.conflict`/`.common_dev_allowlist`) and `[opsx]`, but none for `[mcp]`, `[layout]`, `[broker.watcher]`, `[supervisor.auto_approve]`, `[supervisor.learnings_config]`, or `[governance]` — all of which the config reference documents as supported. A user who treats the init template as the discoverable schema won't see these knobs (consistency-sweep finding #8). Not a correctness bug — absent sections load fine — but template and reference disagree on completeness.

## What Changes

- Extend `generate_default_config` so the generated template includes a commented example stanza for every config section the configuration reference documents — adding `[mcp]`, `[layout]`, `[broker.watcher]`, `[supervisor.auto_approve]`, `[supervisor.learnings_config]`, and `[governance]`.
- Keep every added stanza commented (defaults unchanged; nothing new is activated), and preserve `worktree_placement = "child"` plus the idempotent "do not overwrite an existing config" behavior.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `project-initialization`: MODIFY **Init generates default config.toml** — the generated template SHALL present a commented example for every documented config section, not a subset.

## Impact

- `src/config.rs` (`generate_default_config`): add the missing commented stanzas.
- `docs/src/configuration` is the canonical section list; keep template and reference in sync.
- No behavior change when the new sections are absent (all commented) — backward compatible with existing configs.

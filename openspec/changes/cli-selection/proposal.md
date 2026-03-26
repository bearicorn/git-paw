## Why

git-paw v0.1.0 has a simple CLI picker: the user selects a CLI interactively or passes `--cli`. v0.2.0 adds `--from-specs` where each spec can declare its own CLI via `paw_cli`, and the config can set `default_spec_cli` to bypass the picker entirely for spec-driven launches. The CLI resolution needs a clear priority chain so users can control CLI assignment at multiple levels without surprises.

## What Changes

- New config field `default_spec_cli` — bypasses the picker for `--from-specs` when a spec has no `paw_cli`
- Existing `default_cli` — now pre-selects in the interactive picker (user confirms or changes) instead of just being a fallback
- CLI resolution chain for `--from-specs` (highest to lowest priority):
  1. `--cli` flag → all branches, no picker
  2. `paw_cli` in spec → that branch only, no picker
  3. `default_spec_cli` in config → remaining branches, no picker
  4. `default_cli` in config → remaining branches, picker pre-selected
  5. Nothing → remaining branches, full picker
- New `resolve_cli_for_specs()` function that applies this chain to a list of `SpecEntry` results and returns branch-to-CLI mappings
- Update `select_cli()` in the `Prompter` trait to accept an optional `default` parameter for pre-selection

## Capabilities

### New Capabilities
- `cli-selection`: Multi-level CLI resolution chain for spec-driven launches with `default_spec_cli` bypass and `default_cli` pre-selection

### Modified Capabilities
- `interactive-selection`: `select_cli()` updated to accept optional pre-selected default
- `configuration`: New `default_spec_cli` field on `PawConfig`

## Impact

- **Modified files**: `src/interactive.rs` (pre-selection support, new resolution function), `src/config.rs` (add `default_spec_cli` field)
- **No new files** — resolution logic lives in `interactive.rs` alongside the existing picker
- **No new dependencies**
- **Depends on**: `spec-scanner` (provides `SpecEntry` with `cli` field), `init-command` (adds `default_spec_cli` to config struct)

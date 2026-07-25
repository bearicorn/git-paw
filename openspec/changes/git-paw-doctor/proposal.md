## Why

Users hit opaque launch failures — no `tmux` on PATH, no AI CLIs detected, a missing
or stale bundled script (the `sweep.sh`-gone case this cycle surfaced), an unparseable
config, or an unconfigured spec system — with no single command to answer "why won't it
launch?". v0.13.0 is a quality cycle before the v1.0.0 freeze; a read-only preflight
diagnostics command is one clean, additive command that fits the theme, adds direct
dogfood value, and complements `selftest` (v0.9.0): `doctor` is a static env/config
health check, `selftest` is a live dummy-CLI session smoke.

## What Changes

- New additive **`git paw doctor`** command: read-only preflight diagnostics grouped as
  Environment / CLIs / Config / Spec system / Bundled scripts / Broker / Supervisor /
  Hygiene. Each check reports **✓ / ⚠ / ✗** with an actionable remedy for every non-✓.
- **Exit code** reflects the worst check: non-zero on any ✗ (hard failure), zero when
  only ✓/⚠.
- **`--json`** machine-readable output (agent-friendly, consistent with the v0.10.0 docs
  theme).
- **Diagnose, don't mutate.** No state is written. An optional `--fix` was considered and
  is **explicitly OUT of scope for v0.13.0** (diagnose-only ships first).
- **Export-agnostic** (per `AGENTS.md`): the Supervisor gate-command check sources
  toolchain verbs from the resolved stack preset — it never hard-codes git-paw's own
  cargo/just toolchain as universal.

New capability, no breaking changes.

## Capabilities

### New Capabilities
- `preflight-diagnostics`: the `git paw doctor` command — its check catalogue, the ✓/⚠/✗
  status model, `--json` surface, and the exit-code contract.

### Modified Capabilities
_None._ Doctor is purely additive; it reads existing config/detection/script state but
changes no existing requirement.

## Impact

- **Code:** new diagnostics module (`src/doctor.rs`; if the v0.13.0 code-analysis
  workstream introduces a `commands/` module it can move there). New `Doctor` variant in
  the `src/cli.rs` `Commands` enum + `about`/`long_about`/flag help. Dispatch arm in
  `main.rs`. Reuses `detect::detect_clis`, `PawConfig` load + `worktree_placement()`,
  the bundled-script embedding + version, `[broker]` bind/port, `[supervisor]` gate
  commands, and the `purge --stale` staleness probe (`session.rs`).
- **Not enum-variant ripple:** adding a `Commands` variant touches only clap dispatch,
  not `BrokerMessage`/`SpecBackendKind`.
- **Docs:** `--help`; README CLI table; a new mdBook page (`docs/src/`); `mdbook build
  docs/` must pass. No new config fields, so the configuration reference is unchanged
  except to mention doctor.
- **Tests:** per-check unit tests over injected state (each check is a pure function
  from probed inputs → status+remedy, so no real env needed); an `assert_cmd`
  integration test asserting exit codes and the `--json` shape.

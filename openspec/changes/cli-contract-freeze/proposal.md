## Why

v1.0.0 promises no breaking changes within a major version, but nothing records what that
promise covers or stops a release from breaking it. The only written exit-code contract is a
three-row table in `docs/src/cli-reference.md` (0 success, 1 error, 2 cancelled) that omits
argument errors — clap's parse failures also exit 2 — and every per-command outcome scripts
rely on (`git paw doctor && deploy`, `start --unattended` in CI). There is no deprecation
path either, so the first post-1.0 rename would have to choose between breaking users and
keeping dead surface forever. "Semver from 1.0" needs a recorded, machine-checked contract
and a deprecation rule, or it is a slogan.

## What Changes

- A **stability chapter** (`docs/src/stability.md`) defines the stable surface — commands,
  flags, config keys, exit codes, `--json` outputs, persisted and wire formats, MCP tools,
  bundled-helper interfaces, CLI support tiers — and the surfaces explicitly outside it
  (human-readable output, hidden `__*` subcommands, test-only `GIT_PAW_*` overrides, skill and
  boot-prompt prose, the dashboard layout, the Rust library API).
- A **contract manifest** checked into the repository and rendered in that chapter, plus a
  guard test: removing or renaming a stable entry fails the build within the same major; an
  addition fails until the manifest records it.
- The **exit-code table is frozen** with concrete per-command rows: `0` done (including the
  no-op outcomes), `1` failed, `2` nothing attempted — an argument rejected by parsing, an
  interactive prompt cancelled, or a confirmation declined — and `3` ran but stopped short and
  reported the outcome (an `--unattended` wave that did not complete; a `finish` refused by its
  merge policy). The codes are the ones v0.24.0's `error-exit-code-contract` settles (with
  v0.21.0's `finish` refusal); no command's code changes at the freeze itself.
- A **deprecation policy**: deprecate in a minor release, keep the item working and warning
  for at least two minor releases, remove only in a later major. The deprecation registry and
  its single stderr warn helper already exist (v0.21.0 `supervisor-vocabulary-split`, capability
  `core-deprecation`, which also feeds `git paw config check` and doctor's deprecated-key ⚠);
  this change adds the policy and renders the registry in the manifest and the stability chapter.
- **Pre-1.0 deprecations are resolved at the freeze.** Everything v0.21.0 and v0.24.0 scheduled
  for 1.0.0 — `--supervisor`, `--no-supervisor`, the `supervisor` skill name (v0.18.0's alias of
  `orchestrator`), the broker root spelling `supervisor`, `supervisor.verify-now`, MCP's `mode`
  field, `--from-specs` — is removed in 1.0.0 by `remove-supervisor-vocabulary` and
  `remove-from-specs-alias`. The registry the first manifest records holds only the version-1
  `[supervisor]` config paths, which keep loading through migration with no removal release.
- The **coordinated-session vocabulary is frozen**: `start --coordinated`, `start --bare`,
  `default_mode`, `[orchestrator]` (+ `.gates`, `.escalation`), `[workers]`, `[drive]`, and the
  broker root identity `orchestrator` (renamed in v0.21.0 by user decision, 2026-10-19). The
  frozen surface contains no "supervisor".
- The **support tiers are frozen**: v0.23.0's tier definitions (first-class / supported /
  launch-only) and the first-class CLI list, not the whole detection list.
- Not itself breaking: nothing is removed here and no runtime output is added (the deprecation
  warnings exist since v0.21.0). (The release's breaking changes are `remove-from-specs-alias`,
  `remove-supervisor-vocabulary` and the behaviour change in `layer-isolation`.)

## Capabilities

### New Capabilities

- `core-stability-contract`: the public contract git-paw keeps within a major version — what
  is stable and what is not, the machine-checked manifest, the frozen exit-code table,
  additive evolution of serialized formats, the frozen support tiers and coordinated-session
  vocabulary, the deprecation policy, and the resolution of pre-1.0 deprecations. Purpose for the
  archived spec: "Define the public contract git-paw keeps within a major version — the stable and
  unstable surfaces, how the stable surface is recorded and checked, the exit-code table, and how a
  stable item is deprecated and eventually removed."

### Modified Capabilities

<!-- none: doctor's deprecated-key ⚠ is sourced from the registry since v0.21.0
(`supervisor-vocabulary-split` with `config-check-command`). -->

## Impact

- **Code:** `src/deprecation.rs` (exists since v0.21.0; gains the major-only removal rule for
  entries deprecated in 1.x); `src/error.rs` (`exit_code` module documents the table; no value
  changes); `src/main.rs` (no dispatch change).
- **Tests:** new `tests/contract_manifest.rs` (manifest guard, including the deprecations and
  vocabulary sections), `tests/exit_code_contract.rs` (one row per table entry, real process exit
  status), a registry policy row in `src/deprecation.rs`'s unit tables.
- **Docs:** new `docs/src/stability.md` + the manifest file it includes; `SUMMARY.md`;
  `cli-reference.md` Exit Codes section; README link; `supported-clis.md` points at the frozen
  tiers; `AGENTS.md` release checklist gains "manifest updated, deprecation windows honoured".
- **Enum-variant ripple:** none (`BrokerMessage`, `SpecBackendKind` untouched; the manifest
  *reads* their type strings).
- **Dependencies:** none new (`schemars`, already approved, generates the config-key section).
- **Cross-version dependencies:**
  - v0.17.0 `stop-without-confirmation` — `stop --force` is gone before the manifest is cut.
  - v0.17.0 `classifier-retirement` — `__classify` is gone, so it is not listed among the hidden
    subcommands; `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` are parse-only
    with no effect, and v0.21.0's migration drops them, so they never enter the configuration
    section and appear only under deprecations (no removal release). Its escalation rule (any
    escalated prompt makes the outcome `escalated-for-review`) is why the frozen table says such a
    run exits 3 even when every branch merged. v0.17.0 `cli-setup-consent` — `setup-cli`'s
    `NotAttempted` outcomes are code 2.
  - v0.18.0 `orchestrator-skill-export` (the `supervisor` → `orchestrator` skill alias, deprecated
    in v0.21.0 and removed in 1.0.0), `supervisor-pane-removal` (the session meaning frozen here),
    `cmd-library-surface` and `broker-host-separation` (new hidden `__broker`; MCP write tools
    enter the manifest).
  - v0.21.0 `supervisor-vocabulary-split` (the deprecation registry, warn helper and the
    coordinated-session vocabulary this change freezes), config schema versioning (version 2), the
    wire DTOs and uniform `--json` — the manifest's config and format sections are generated from
    those DTOs; `git paw finish` gets its row in the exit-code table.
  - v1.0.0 `remove-supervisor-vocabulary` and `remove-from-specs-alias` (they empty the registry of
    1.0.0 removals before the manifest is cut).
  - v0.22.0 replay rethink — whether `replay` is frozen or hidden (design.md, gated).
  - v0.23.0 per-CLI support audit — the tier definitions and first-class list frozen here.
  - v0.24.0 errors-and-exit-codes pass and back-compat matrix — the exit-code table and the
    format-compatibility rows are rebased on its outcome before this change is applied.

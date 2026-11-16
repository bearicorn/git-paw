## Why

A committed `.git-paw/config.toml` should work on every machine that clones the repository and in
every repository on one machine. Four dogfood findings show it does not:

- **DF-03** — the committed config names a CLI (`default_cli`, a preset, a `[clis.*]` entry) that is
  not installed on the maintainer's other machine. `git paw doctor` only prints
  `warning: custom CLI '…' not found …, skipping` on stderr above the report (it comes from detection,
  not from a check), the CLIs group still says ✓, and a launch silently drops the pre-selection.
- **DV-01** — every repository defaults the broker to port 9119, so the second repository's
  `git paw start` refuses while the first one's session is live, until the user picks a port by hand
  and commits it (which then clashes on someone else's machine).
- **PK-20** — a repository-local CLI wrapper needs an absolute `[clis.*].command`, so the committed
  config carries one machine's home directory.
- **PK-02** — the init comment and the interactive init point Spec Kit at `.specify/specs`, but Spec
  Kit projects keep their features in `specs/<NNN-feature>/`.

## What Changes

- **Configured CLIs become a doctor check.** One CLIs-group row per CLI the config names
  (`default_cli`, `default_spec_cli`, `[supervisor].cli`, presets, worker profiles, `[clis.*]`
  entries): ✓ when it resolves on this machine, ⚠ with a remedy when it does not. The detection
  warning is captured into the check instead of being printed above the report.
- **Launch behaviour for an absent configured CLI.** The interactive picker opens without that
  pre-selection and prints one notice naming the CLI and the key; a tier that would launch it
  without a prompt (`default_spec_cli`, a preset, a profile) refuses before any worktree is created,
  naming the CLI, the key and the remedies (`--cli`, install it, or define `[clis.<name>]`).
- **Per-repository default broker port.** When no config file sets `[broker] port`, the port is
  derived from the repository's main working tree path (stable hash into 9120–9919). `git paw start`
  steps to the next port in that range when another repository's broker or a foreign service holds
  it, records the chosen URL in the session, and every reader (status, add, dashboard, doctor, the
  helpers through `GIT_PAW_BROKER_URL`) uses the recorded URL. An explicit `port` keeps today's exact
  behaviour, including the refusal.
- **Relative `[clis.*].command`.** A `command` containing a path separator but not absolute is
  resolved against the repository's main working tree; a bare name still goes through `PATH`.
- **Spec Kit's conventional directory becomes `specs`.** Init's comment and interactive choice write
  `dir = "specs"` for `speckit`; with no `dir` configured, `specs` is used, falling back to the legacy
  `.specify/specs` only when `specs/` does not exist and `.specify/specs/` does.

No config key is added, renamed or removed; every config that loads today still loads.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `preflight-diagnostics`: ADDED *Configured CLIs are checked on this machine*; ADDED *Broker check
  reports how the port was chosen*.
- `cli-resolution`: MODIFIED *Resolve and merge custom CLIs from configuration* (relative command,
  no stray stderr under doctor); ADDED *A configured CLI absent on this machine*.
- `broker-runtime`: MODIFIED *Broker configuration schema* (absent `port` means the per-repository
  default); ADDED *Per-repository default broker port*, *Start steps past a held derived port*,
  *Readers use the session's recorded broker URL*, *Helpers obtain the effective broker URL*.
- `spec-providers`: MODIFIED *Spec-system selection is explicit (config or CLI only)* (Spec Kit's
  conventional directory and the legacy fallback).
- `core-init`: MODIFIED *Init records the spec system explicitly, never by detection* (Spec Kit
  `dir = "specs"`).

## Impact

- **Code:** `src/detect.rs` (resolution returns unresolved entries instead of printing; relative
  command against a root), `src/doctor.rs` (`probe_clis` / new `check_configured_clis`;
  `probe_broker` reports the effective port and its source), `src/config/broker.rs` (`port` becomes
  "unset" in the internal model; derivation helper), `src/broker/` (start-time port selection),
  session receipt (`broker_url`), `src/interactive/resolver.rs` and the non-prompt tiers (absent-CLI
  rule), `src/agent_cli/profile.rs` (relative command → absolute launch path),
  `src/specs/speckit.rs` (`default_dir`), `src/init.rs` (`COMMENTED_SPECS_TEMPLATE`),
  `src/config/mod.rs` (template comment for `port`), the helpers' `discover_broker_url` in
  `_paw_common.sh`, and a hidden `__broker-url` subcommand.
- **Docs:** configuration reference (`[broker] port` default, `[clis.*].command` relative paths, Spec
  Kit `dir`), `docs/src/user-guide/doctor.md` (configured-CLI rows), broker docs (port selection),
  CHANGELOG (port default moves off 9119 for configs that do not pin it).
- **Cross-version dependencies:** `config-schema-versioning` — `[broker].port` is a script-read key
  (its D7); its meaning is unchanged, only the absent default changes, so no version bump; the
  wire/model split (its D4) is where `port` becomes optional in the model. `config-check-command` —
  its validator checks CLI *names* against configuration (D3); this change checks the *machine*, in
  doctor only, so the two do not overlap. `uniform-json-output` — the new doctor rows are ordinary
  checks in the existing `--json` document.

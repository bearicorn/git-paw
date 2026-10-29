## Why

A typo in `.git-paw/config.toml` is silently ignored: serde drops unknown keys, a `default_cli`
naming a CLI that is not installed just never matches, and `[worktree.ports]` stride problems are an
`eprintln!` during load. `git paw doctor` reports parse errors and unknown keys, but only for the repo
file (never the global one), with a round-trip heuristic that can misflag explicitly-empty values, and
it does not report deprecated keys although its spec says it does (`Config check`). Since v0.17.0 the
`[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` tables are deprecated with no
effect (`classifier-retirement`); only a load-time stderr line says so. There is no CI-friendly way to ask "is this config valid?" without running the whole
environment diagnosis.

Scope against `doctor` (MILESTONE: "diagnose vs. validate"): `doctor` **diagnoses the environment** —
tools, CLIs, broker, scripts, hygiene — and shows a config summary. `git paw config check`
**validates the configuration files** — every file, every key, every value git-paw would otherwise
ignore — and is safe to run in CI. They share one validator, so the overlap is the validator, not the
commands.

## What Changes

- New `git paw config check [--strict] [--json]`: validates the global and the repo config separately,
  reporting per finding the file, the key path, a severity (error / warning) and a remedy.
- Findings: TOML syntax and type errors (error, with line/column); values git-paw rejects at load
  (approval levels, session templates — error); unknown keys (warning, both files, no false positive
  for explicitly-empty tables/lists); deprecated keys (warning; the `[supervisor.auto_approve]` and
  `[supervisor.common_dev_allowlist]` tables, deprecated in 0.17.0 with no effect, are one finding
  per whole table); `default_cli` / `default_spec_cli` naming a CLI that is neither built in nor defined under
  `[clis]` (warning); `[worktree.ports]` stride warnings (warning); the schema-version findings of the
  config-versioning change, obtained from its `config::migrate` API (no second version parser).
- Exit 0 when there is no error; 1 when there is an error, or any warning under `--strict`.
- `--json` follows the `cli-machine-output` contract from `uniform-json-output` (`config check` is
  already on its roster): `schema_version`, `files` (path, kind, present, `config_version`) and
  `findings`; the document is printed even when findings are errors (exit 1).
- Validate only — no rewriting/normalising: `toml` cannot round-trip comments, and in-memory migration
  belongs to config schema versioning.
- `doctor`'s Config group runs the same validator, so it finally reports deprecated keys and the
  global file too; its remedy points at `git paw config check` for the full list.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `preflight-diagnostics`: ADDED `git paw config check` requirements; **Config check** (doctor) uses
  the shared validator and covers both files and deprecated keys.

## Impact

- **Code:** new pure `config::validate` (`ConfigFinding { file, key_path, severity, message, remedy }`,
  `validate_file(path, raw) -> Vec<ConfigFinding>`), replacing `doctor::unknown_config_keys`'
  round-trip diff with a schema walk over the known key tree; detect's known-CLI roster for CLI names; `WorktreePortsConfig::stride_warning`;
  `src/cli.rs` (`Command::Config { Check { strict, json } }`); `src/main.rs` dispatch; `src/doctor.rs`
  `check_config` delegates.
- **Enum-variant ripple:** none.
- **Docs:** `--help`, `docs/src/cli-reference.md` (`config check`), `docs/src/user-guide/doctor.md`
  (diagnose vs validate), configuration reference intro (how to validate), README command table.
- **Cross-version dependencies:** this release's `config-schema-versioning` (other half) — config check
  reports versions and version findings through its `config::migrate` API and walks its `config::wire`
  types as the known-key tree; it must land first;
  `uniform-json-output` — `config check --json` follows its `cli-machine-output` contract and is already
  on its roster (no roster delta needed); it must land first;
  v0.17.0's `classifier-retirement` removes `supervisor-config`'s common-dev-allowlist and auto-approve
  requirements, deletes the stack presets (`src/supervisor/dev_allowlist.rs`), and adds *Deprecated
  auto-approval tables are parsed but have no effect* — the deprecation findings must match that
  requirement (whole tables, no effect, deprecated in 0.17.0); this release's
  `supervisor-vocabulary-split` folds the deprecation table (D4) into its deprecation registry.
  v0.18 `cmd-library-surface` — the validator is library code; the command is a thin entry.

## Context

`config::load_config_file`: `toml::from_str` → `validate_approval_args` (hard error) →
`validate_session_templates` (hard error) → `worktree_runtime_warnings` (`eprintln!` only). No
`deny_unknown_fields` anywhere. `doctor::check_config` (Config group): parse via the merged
`load_config` (so a broken global file is reported under the repo path), presence, resolved
`worktree_placement`, and `unknown_config_keys(raw)` — a TOML → `PawConfig` → TOML round-trip diff over
the repo file only, which (inferred from `skip_serializing_if`) misreports explicit empty values such as
`[worktree.env] copy = []`. Deprecated keys are not detected anywhere.

**Load errors fall back to defaults (feedback #21).** `doctor::collect_probes` loads the merged
config and, on any error, substitutes `PawConfig::default()` (`src/doctor.rs:1820-1826`). Every later
probe reads that default: `probe_broker` sees `enabled = false` (`src/doctor.rs:2422-2444`) and
`check_broker` prints "disabled — the pure-manual baseline" (`src/doctor.rs:1339-1346`);
`probe_supervisor` returns the default (`src/doctor.rs:2459-2462`) and `check_supervisor` prints
"disabled" (`src/doctor.rs:1393-1400`); `probe_spec_system` finds no `[specs]`
(`src/doctor.rs:2013-2016`) and `check_spec_system` prints "no spec system configured"
(`src/doctor.rs:884-893`). The same default feeds `probe_clis`, `probe_sandbox_mode`,
`probe_spec_cli` and `probe_spec_workflow` (`src/doctor.rs:1839-1846`). The Config ✗ is correct; the
other groups describe a configuration nobody wrote. (`git paw init` has the same fallback,
`src/init.rs:137-139`, `:177`; `upgrade-hygiene` covers it.)

## Goals / Non-Goals

**Goals:** a deterministic, file-attributed validator usable from CI and from doctor; close doctor's
unmet "deprecated fields" clause.

**Non-Goals:** rewriting or normalising files (comment loss, and migration belongs to
`config-schema-versioning`); `deny_unknown_fields` at load (would break forward compatibility of old
binaries reading newer configs); validating environment facts (CLI installed on PATH — that is doctor).

## Decisions

- **D1 — One pure validator, two consumers.** `validate_file(kind, path, raw) -> Vec<ConfigFinding>`.
  `config check` renders all findings; doctor maps them into its Config group (errors → ✗, warnings →
  ⚠, one line per file plus a "run `git paw config check` for details" remedy when there are several).
  Alternative — fold into doctor (MILESTONE's fallback) — rejected: doctor needs tmux/git/CLIs and
  answers "can I run here?"; CI wants "is this file valid?" with a file-scoped exit code.
- **D2 — Known-key tree, not round-trip.** Walk the parsed `toml::Value` against a static description
  of the accepted keys derived from the `config::wire` types (`config-schema-versioning`), guarded by
  a test that every wire field appears in it. Removes the empty-value false positive and gives exact
  key paths.
- **D3 — CLI names are checked against configuration, not PATH.** `default_cli` must be a built-in
  roster name or a `[clis.<name>]` entry; whether it is installed is doctor's CLIs group. Keeps
  `config check` hermetic for CI.
- **D4 — Deprecation table.** A small static table `(key path, since, replacement or "no effect")`;
  first entries the whole tables `supervisor.auto_approve` and `supervisor.common_dev_allowlist`
  (since 0.17.0, "no effect", from `classifier-retirement`), each reported once per file with no
  per-field findings. New deprecations append to it (`supervisor-vocabulary-split` turns it into its
  registry). There is no stack-name check: v0.17.0 deleted the stack presets, and the stack list has
  no effect anyway.
- **D5a — Versions through `config::migrate`; JSON through `cli-machine-output`.** The validator
  asks `config::migrate` (from `config-schema-versioning`) for each file's version and its findings
  (too-new, malformed `version`), and walks the `config::wire` types for known keys — no second
  version parser. `--json` emits a typed `ConfigCheckReport` (`schema_version`, `files`, `findings`)
  in the shared output module from `uniform-json-output`; `config check` is already on that
  contract's roster. Error findings are a result (document printed, exit 1), like failing doctor
  checks.
- **D5 — Exit codes.** 0 / 1, with `--strict` promoting warnings. Matches doctor's "worst check" rule.
- **D6 — "Not evaluated", not defaults.** `collect_probes` keeps `Result<PawConfig, ConfigLoadError>`;
  the config-derived probes take `Option<&PawConfig>` and return a `NotEvaluated` variant when it is
  `None`. Each affected group renders one ⚠ whose detail starts with "not evaluated — the
  configuration could not be loaded" and whose remedy points at the Config ✗. Groups that never read
  the config (Environment, roster detection, Python, helper presence/mode/content, Hygiene) run as
  today; detection runs with the roster only. ⚠ rather than a new status keeps the doctor status set
  (and the `--json` `status` values) closed; the Config ✗ already makes the exit non-zero.
  *Alternative:* a fourth `skipped` status — clearer, but it widens a machine contract that
  `uniform-json-output` is freezing (open question 1).
- **D7 — An unparseable file stops at its syntax finding.** The validator returns the parse error and
  nothing else for that file (no key walk over a partial tree); the other file is validated on its
  own, as D1 already does.

## Risks / Trade-offs

- [Known-key table drifts from the structs] → a test enumerates every serde field (via the wire
  types or a serialize-all-fields fixture) and fails on a missing table entry.
- [Warnings for third-party keys a consumer adds on purpose] → warnings only; `--strict` is opt-in.
- [A "not evaluated" ⚠ hides a real problem in that group] → it is shown next to a ✗ that must be
  fixed first; re-running doctor after the fix evaluates the group.

## Open Questions

1. Add a `skipped` doctor status for "not evaluated" instead of ⚠ (D6)? It must be decided before
   `uniform-json-output` freezes the `status` values.

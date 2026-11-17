## 0. Preconditions

- [ ] 0.1 `config-schema-versioning` (`config::wire`, `config::migrate`) and `uniform-json-output` (`cli-machine-output`, shared output module) have landed; build on their APIs
- [ ] 0.2 `session-state-reconciliation` is archived first (this change's *`git paw doctor` runs read-only preflight checks* delta is written on its text); if it is not, stop and sequence it

## 1. Validator (pure)

- [ ] 1.1 `src/config/validate.rs`: `ConfigFinding`, `Severity`, `validate_file(kind, path, raw) -> Vec<ConfigFinding>`; syntax/type errors with position; load-time rejections reused from `validate_approval_args` / `validate_session_templates` (refactor them to return findings, load keeps erroring)
- [ ] 1.2 Known-key tree walk derived from the `config::wire` types, with a test that every wire field is in it; explicit empty tables/lists accepted
- [ ] 1.3 Deprecation table (first entries the whole tables `supervisor.auto_approve` and `supervisor.common_dev_allowlist`, since 0.17.0, no effect; one finding per table, none per field); CLI names against the built-in roster + `[clis]`; stride warnings via `WorktreePortsConfig::stride_warning`; per-file version and schema-version findings via `config::migrate` (no local version parsing)
- [ ] 1.4 Table tests for every finding kind, both files

## 2. Command

- [ ] 2.1 `src/cli.rs`: `Command::Config { action: ConfigAction::Check { strict, json } }` with `about`/`long_about`/examples (diagnose vs validate wording); parse tests
- [ ] 2.2 Human renderer (grouped by file) and typed `ConfigCheckReport` (`schema_version`, `files`, `findings`) in the shared output module; document printed on exit 0 and on error findings (exit 1), stdout empty only on a command failure; `--strict`; golden key set for the `cli-machine-output` guard
- [ ] 2.3 `doctor::check_config` delegates to the validator for both files; replace `unknown_config_keys`; remedy points at `git paw config check` when a file has several findings

## 3. Tests per scenario

- [ ] 3.1 `tests/config_check_e2e.rs` (isolated `HOME`/`XDG_CONFIG_HOME`): unknown key in global file with path; both deprecated auto-approval tables one finding each (no `elixir` or per-field finding); unknown default CLI; empty `copy = []` not flagged; syntax error names the line; files untouched (mtime + content); exit codes incl. `--strict`; `--json` lists both findings with `files`/`config_version`; `--json` with invalid TOML prints the document and exits 1
- [ ] 3.2 `doctor_tests.rs`: deprecated `[supervisor.auto_approve]` flagged ⚠; broken global config attributed to the global path; existing Config scenarios unchanged

## 3b. Doctor on a config load error (feedback #21)

- [ ] 3b.1 `src/doctor.rs`: `collect_probes` keeps the load result instead of `PawConfig::default()` (`:1823-1826`); config-derived probes (`probe_broker`, `probe_supervisor`, `probe_spec_system`, `probe_spec_cli`, `probe_spec_workflow`, `probe_sandbox_mode`, `probe_cli_identities`, root-skill staleness) return `NotEvaluated` without a config; detection runs with the roster only
- [ ] 3b.2 `CheckStatus::Skipped`: glyph `○`, serialized `"skipped"`, excluded from the worst-check computation (overall status and exit code); the human summary line gains an `N ○` count after the `✗` count; unit tests for the overall status with and without skipped checks
- [ ] 3b.3 Each affected `check_*` renders one skipped check "not evaluated — the configuration could not be loaded" per group, remedy pointing at the Config ✗; unit tests per group
- [ ] 3b.4 Validator: an unparseable file yields only its syntax finding; table test with a broken repo file and a global file with an unknown key
- [ ] 3b.5 `tests/doctor_e2e.rs`: config with `[broker] enabled = true` and a syntax error → Broker/Supervisor/Spec-system "not evaluated", no "disabled" / "no spec system" text, Environment and Hygiene still reported; `--json` check statuses `skipped`, overall status `fail` (from the Config ✗), exit non-zero

## 4. Docs

- [ ] 4.1 `--help`; `docs/src/cli-reference.md` `config check`; `docs/src/user-guide/doctor.md` (diagnose vs validate; the `○` skipped status and "not evaluated" after a config error; skipped checks never change the exit code); configuration reference intro; README command table row

## 5. Gates

- [ ] 5.1 `cargo fmt`; `just check`; `just deny`; `mdbook build docs/`

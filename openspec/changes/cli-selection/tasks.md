## 1. Config Field

- [ ] 1.1 Add `default_spec_cli: Option<String>` to `PawConfig` in `src/config.rs` with `serde(default)`
- [ ] 1.2 Update `PawConfig::merged_with()` to handle `default_spec_cli`
- [ ] 1.3 Unit tests: parse, default when absent, merge override

## 2. Prompter Trait Update

- [ ] 2.1 Update `Prompter::select_cli()` signature to accept `default: Option<&str>` parameter
- [ ] 2.2 Update `DialoguePrompter::select_cli()` — find matching CLI index, pass to `dialoguer::Select::default()`
- [ ] 2.3 Handle case where default CLI name is not in the available list (fall back to no pre-selection)
- [ ] 2.4 Update `MockPrompter` in tests to accept the new parameter
- [ ] 2.5 Update all existing call sites of `select_cli()` to pass `None` (preserve existing behavior)

## 3. CLI Resolution Function

- [ ] 3.1 Implement `resolve_cli_for_specs(specs, cli_flag, config, available_clis, prompter) -> Result<Vec<(String, String)>, PawError>` in `src/interactive.rs`
- [ ] 3.2 Priority 1: if `cli_flag` is Some → map all specs to that CLI, return
- [ ] 3.3 Priority 2: for each spec with `spec.cli` Some → map that branch to `spec.cli`
- [ ] 3.4 Priority 3: for remaining specs, if `config.default_spec_cli` is Some → map to it
- [ ] 3.5 Priority 4+5: for remaining specs, call `select_cli(default: config.default_cli)` once → map all remaining to the result
- [ ] 3.6 After resolution: validate each resolved CLI name exists in `available_clis`, return `PawError::CliNotFound` if not

## 4. Unit Tests

- [ ] 4.1 Test: --cli flag overrides all specs
- [ ] 4.2 Test: paw_cli per-spec overrides config
- [ ] 4.3 Test: default_spec_cli fills remaining without prompt
- [ ] 4.4 Test: default_cli pre-selects in picker (mock verifies default parameter)
- [ ] 4.5 Test: no defaults → picker fires with None default
- [ ] 4.6 Test: mixed paw_cli + default_spec_cli
- [ ] 4.7 Test: mixed paw_cli + interactive
- [ ] 4.8 Test: picker fires at most once for multiple remaining branches
- [ ] 4.9 Test: all resolved via flag → no prompt call
- [ ] 4.10 Test: all resolved via paw_cli + default_spec_cli → no prompt call
- [ ] 4.11 Test: paw_cli references unknown CLI → CliNotFound error
- [ ] 4.12 Test: default_spec_cli references unknown CLI → CliNotFound error
- [ ] 4.13 Test: select_cli with default present and in list → pre-selected
- [ ] 4.14 Test: select_cli with default not in list → no pre-selection (graceful)
- [ ] 4.15 Run `cargo clippy -- -D warnings` and `cargo fmt --check` — clean

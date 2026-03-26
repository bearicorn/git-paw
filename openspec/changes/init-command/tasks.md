## 1. Error Variants

- [ ] 1.1 Add `InitError(String)` variant to `PawError` in `src/error.rs` with error message `"Init error: {0}"`
- [ ] 1.2 Add `AgentsMdError(String)` variant to `PawError` in `src/error.rs` with error message `"AGENTS.md error: {0}"`
- [ ] 1.3 Add unit tests for new error variants (message content, exit code)

## 2. Config Struct Extensions

- [ ] 2.1 Add `SpecsConfig` struct with `specs_dir: Option<String>` and `enabled: Option<bool>` fields
- [ ] 2.2 Add `LoggingConfig` struct with `enabled: Option<bool>` and `log_dir: Option<String>` fields
- [ ] 2.3 Add `default_spec_cli: Option<String>`, `branch_prefix: Option<String>`, `specs: Option<SpecsConfig>`, `logging: Option<LoggingConfig>` to `PawConfig`
- [ ] 2.4 Update `PawConfig::merged_with()` to handle new scalar and struct fields
- [ ] 2.5 Add `generate_default_config() -> String` function that returns a TOML string with active defaults and commented-out v0.2.0 fields
- [ ] 2.6 Add unit tests: parse config with new fields, all new fields optional, merge new scalars, round-trip with new fields, generated config is valid TOML and contains comments

## 3. CLI Subcommand

- [ ] 3.1 Add `Init` variant to `Command` enum in `src/cli.rs` with `about`, `long_about`, and examples
- [ ] 3.2 Add unit test: `init_parses` — verify `init` is parsed as `Command::Init`
- [ ] 3.3 Update help text assertions if any existing tests check for exhaustive subcommand lists

## 4. AGENTS.md Module

- [ ] 4.1 Create `src/agents.rs` with module-level doc comment
- [ ] 4.2 Add `mod agents;` to `src/main.rs` (or `src/lib.rs`)
- [ ] 4.3 Implement `generate_git_paw_section() -> String` — returns the marker-delimited git-paw section content
- [ ] 4.4 Implement `inject_section(repo_root: &Path) -> Result<(), PawError>` — detects AGENTS.md/CLAUDE.md state and injects or updates the section
- [ ] 4.5 Implement marker detection: `has_git_paw_section(content: &str) -> bool`
- [ ] 4.6 Implement section replacement: `replace_git_paw_section(content: &str, new_section: &str) -> String`
- [ ] 4.7 Implement CLAUDE.md compatibility logic (symlink creation, append-to-CLAUDE.md, both-exist handling)
- [ ] 4.8 Unit tests: inject into empty file, append to existing AGENTS.md, replace existing section, preserve surrounding content
- [ ] 4.9 Unit tests: CLAUDE.md only → symlink + append, both exist → AGENTS.md only, neither → create AGENTS.md
- [ ] 4.10 Unit tests: already-a-symlink case, read-only file error, symlink creation failure error

## 5. Init Module

- [ ] 5.1 Create `src/init.rs` with module-level doc comment
- [ ] 5.2 Add `mod init;` to `src/main.rs` (or `src/lib.rs`)
- [ ] 5.3 Implement `run_init(repo_root: &Path) -> Result<(), PawError>` — orchestrates all init steps
- [ ] 5.4 Implement directory creation: `.git-paw/` and `.git-paw/logs/`
- [ ] 5.5 Implement config generation: write `generate_default_config()` output to `.git-paw/config.toml` (skip if exists)
- [ ] 5.6 Implement gitignore management: append `.git-paw/logs/` to `.gitignore` (handle missing file, missing newline, already-present)
- [ ] 5.7 Call `agents::inject_section(repo_root)` for AGENTS.md injection
- [ ] 5.8 Implement status reporting: print summary of actions taken vs skipped
- [ ] 5.9 Unit tests: fresh repo init creates all expected files, double init is idempotent, existing config not overwritten
- [ ] 5.10 Unit tests: gitignore edge cases (no file, no newline, already present)

## 6. Wire Up

- [ ] 6.1 Add `Command::Init` match arm in `src/main.rs` dispatch, calling `init::run_init()`
- [ ] 6.2 Integration test: `git paw init` in a fresh git repo creates `.git-paw/`, config, logs dir, updates gitignore, creates AGENTS.md
- [ ] 6.3 Integration test: `git paw init` outside a git repo fails with "Not a git repository"
- [ ] 6.4 Integration test: double `git paw init` is idempotent
- [ ] 6.5 Run `cargo clippy -- -D warnings` and `cargo fmt --check` — clean

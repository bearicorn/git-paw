## 1. Error Variant

- [ ] 1.1 Add `SpecError(String)` variant to `PawError` in `src/error.rs` with error message `"Spec error: {0}"`
- [ ] 1.2 Add unit test for `SpecError` (message content, exit code)

## 2. Module Setup

- [ ] 2.1 Create `src/specs.rs` with module-level doc comment (`//! Spec scanning and discovery`)
- [ ] 2.2 Add `mod specs;` declaration in `src/lib.rs` or `src/main.rs`

## 3. Data Types

- [ ] 3.1 Define `SpecEntry` struct with fields: `id: String`, `branch: String`, `cli: Option<String>`, `prompt: String`, `owned_files: Option<Vec<String>>`
- [ ] 3.2 Add doc comments to `SpecEntry` and all fields
- [ ] 3.3 Define `SpecBackend` trait with `fn scan(&self, dir: &Path) -> Result<Vec<SpecEntry>, PawError>`

## 4. Backend Dispatch

- [ ] 4.1 Implement `backend_for_type(spec_type: &str) -> Result<Box<dyn SpecBackend>, PawError>` — match on `"openspec"` and `"markdown"`, return `SpecError` for unknown types
- [ ] 4.2 Create stub `OpenSpecBackend` struct implementing `SpecBackend` (returns empty vec with `// TODO: implement in openspec-integration`)
- [ ] 4.3 Create stub `MarkdownBackend` struct implementing `SpecBackend` (returns empty vec with `// TODO: implement in markdown-integration`)

## 5. Branch Derivation

- [ ] 5.1 Implement `derive_branch(prefix: &str, id: &str) -> String` — concatenates prefix + id, inserting `/` between if prefix doesn't end with one
- [ ] 5.2 Default `branch_prefix` to `"spec/"` when not set in config

## 6. Scanner Entry Point

- [ ] 6.1 Implement `scan_specs(config: &PawConfig, repo_root: &Path) -> Result<Vec<SpecEntry>, PawError>`
- [ ] 6.2 Return `SpecError` if no `[specs]` section in config
- [ ] 6.3 Resolve `specs.dir` relative to `repo_root`
- [ ] 6.4 Validate `specs_dir` exists and is a directory
- [ ] 6.5 Select backend via `backend_for_type()`
- [ ] 6.6 Call backend `scan()`, then apply `derive_branch()` to each returned entry

## 7. Unit Tests

- [ ] 7.1 Test `SpecEntry`: construct with all fields, construct with optionals absent
- [ ] 7.2 Test `derive_branch`: default prefix `"spec/"` + id, custom prefix with trailing slash, custom prefix without trailing slash
- [ ] 7.3 Test `backend_for_type`: `"openspec"` succeeds, `"markdown"` succeeds, `"unknown"` returns SpecError
- [ ] 7.4 Test `scan_specs`: no `[specs]` config → SpecError
- [ ] 7.5 Test `scan_specs`: nonexistent directory → SpecError with path
- [ ] 7.6 Test `scan_specs`: file instead of directory → SpecError
- [ ] 7.7 Test `scan_specs`: valid config with stub backend → returns empty vec
- [ ] 7.8 Run `cargo clippy -- -D warnings` and `cargo fmt --check` — clean

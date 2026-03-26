## 1. Module Setup

- [ ] 1.1 Convert `src/specs.rs` to `src/specs/mod.rs` (if not already done by `spec-scanner`)
- [ ] 1.2 Create `src/specs/openspec.rs` with module-level doc comment (`//! OpenSpec-format backend for spec scanning`)
- [ ] 1.3 Add `mod openspec;` to `src/specs/mod.rs` and replace the stub `OpenSpecBackend` with the real import

## 2. Directory Scanning

- [ ] 2.1 Implement `OpenSpecBackend::scan()` — iterate immediate subdirectories of the given path
- [ ] 2.2 Skip `archive` directory and any non-directory entries
- [ ] 2.3 Skip changes that don't have `tasks.md` (print warning to stderr)
- [ ] 2.4 Set `SpecEntry.id` to the subdirectory name

## 3. Prompt Extraction

- [ ] 3.1 Read `tasks.md` content as the primary prompt
- [ ] 3.2 Strip frontmatter (if present) from prompt content — detect `---` delimiters, exclude the block
- [ ] 3.3 If `specs/` subdirectory exists, iterate `specs/<capability>/spec.md` files
- [ ] 3.4 Append each spec file's content under a `## Spec: <capability>` heading
- [ ] 3.5 Concatenate tasks content + spec content into `SpecEntry.prompt`

## 4. Frontmatter Parsing

- [ ] 4.1 Implement `parse_frontmatter(content: &str) -> (Option<HashMap<String, String>>, &str)` — returns parsed fields and remaining content
- [ ] 4.2 Detect frontmatter: file starts with `---` line, ends at next `---` line
- [ ] 4.3 Parse `key: value` lines within frontmatter (simple line-by-line, no YAML dependency)
- [ ] 4.4 Extract `paw_cli` field → `SpecEntry.cli`

## 5. File Ownership Extraction

- [ ] 5.1 Implement `extract_owned_files(content: &str) -> Option<Vec<String>>` — scan for `Files owned:` or `Owned files:` pattern
- [ ] 5.2 Parse the markdown list following the pattern (lines starting with `- `)
- [ ] 5.3 Return `None` if pattern not found

## 6. Unit Tests

- [ ] 6.1 Test `parse_frontmatter`: with frontmatter → returns fields and remaining content
- [ ] 6.2 Test `parse_frontmatter`: without frontmatter → returns None and full content
- [ ] 6.3 Test `parse_frontmatter`: with frontmatter but no paw_cli → returns fields without cli
- [ ] 6.4 Test `extract_owned_files`: with file list → returns files
- [ ] 6.5 Test `extract_owned_files`: without pattern → returns None

## 7. Integration Tests (tempfile)

- [ ] 7.1 Create temp directory with 3 change subdirectories (each with tasks.md), scan → returns 3 entries
- [ ] 7.2 Change without tasks.md → skipped, warning printed
- [ ] 7.3 Change with tasks.md + specs/ → prompt contains both
- [ ] 7.4 Change with paw_cli frontmatter → SpecEntry.cli populated
- [ ] 7.5 Change with file ownership in tasks.md → SpecEntry.owned_files populated
- [ ] 7.6 Empty changes directory → returns empty vec
- [ ] 7.7 Archive directory → ignored
- [ ] 7.8 Frontmatter excluded from prompt content
- [ ] 7.9 Run `cargo clippy -- -D warnings` and `cargo fmt --check` — clean

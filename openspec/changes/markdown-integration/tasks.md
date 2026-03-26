## 1. Module Setup

- [ ] 1.1 Create `src/specs/markdown.rs` with module-level doc comment (`//! Markdown-format backend for spec scanning`)
- [ ] 1.2 Add `mod markdown;` to `src/specs/mod.rs` and replace the stub `MarkdownBackend` with the real import

## 2. Shared Frontmatter Parser

- [ ] 2.1 Move `parse_frontmatter()` to `src/specs/mod.rs` as a shared function (if not already there from `openspec-integration`)
- [ ] 2.2 Ensure both `openspec.rs` and `markdown.rs` import from `mod.rs`

## 3. Backend Implementation

- [ ] 3.1 Implement `MarkdownBackend::scan()` — iterate immediate children of the directory, filter for `.md` files
- [ ] 3.2 For each `.md` file: read content, call `parse_frontmatter()`
- [ ] 3.3 Skip files without frontmatter or without `paw_status` field
- [ ] 3.4 Skip files where `paw_status` is not `"pending"`
- [ ] 3.5 Extract `paw_branch` → `SpecEntry.id` (fallback to filename stem)
- [ ] 3.6 Extract `paw_cli` → `SpecEntry.cli`
- [ ] 3.7 Set `SpecEntry.prompt` to file body after frontmatter
- [ ] 3.8 Set `SpecEntry.owned_files` to `None` always

## 4. Documentation

- [ ] 4.1 Update `generate_default_config()` in `src/config.rs` to include a commented example showing `type = "markdown"` alongside `type = "openspec"`, with a note about frontmatter fields
- [ ] 4.2 Update `generate_git_paw_section()` in `src/agents.rs` to include a brief reference to the markdown spec format and available frontmatter fields (`paw_status`, `paw_branch`, `paw_cli`)

## 5. Unit Tests

- [ ] 5.1 Test scan: directory with 3 pending files → returns 3 entries
- [ ] 5.2 Test scan: mix of pending, done, and in-progress → only pending returned
- [ ] 5.3 Test scan: files without frontmatter → ignored
- [ ] 5.4 Test scan: non-markdown files → ignored
- [ ] 5.5 Test scan: empty directory → empty vec
- [ ] 5.6 Test scan: subdirectories → not traversed
- [ ] 5.7 Test id derivation: paw_branch present → uses paw_branch
- [ ] 5.8 Test id derivation: paw_branch absent → uses filename stem
- [ ] 5.9 Test cli extraction: paw_cli present → Some, absent → None
- [ ] 5.10 Test prompt: frontmatter stripped, body content preserved
- [ ] 5.11 Test prompt: file with only frontmatter → empty prompt
- [ ] 5.12 Test: unknown frontmatter fields are silently ignored
- [ ] 5.13 Test: all three frontmatter fields present → all mapped correctly
- [ ] 5.14 Run `cargo clippy -- -D warnings` and `cargo fmt --check` — clean

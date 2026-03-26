## 1. Error Variant

- [ ] 1.1 Add `AgentsMdError(String)` variant to `PawError` in `src/error.rs` with error message `"AGENTS.md error: {0}"`
- [ ] 1.2 Add unit test for `AgentsMdError` (message content, exit code)

## 2. Module Setup

- [ ] 2.1 Create `src/agents.rs` with module-level doc comment (`//! AGENTS.md generation and injection`)
- [ ] 2.2 Add `mod agents;` declaration and `pub use` in `src/lib.rs` or `src/main.rs`

## 3. Pure Functions

- [ ] 3.1 Implement `generate_git_paw_section() -> String` — returns the full marker-delimited section with git-paw instructions
- [ ] 3.2 Implement `has_git_paw_section(content: &str) -> bool` — checks for `<!-- git-paw:start` prefix
- [ ] 3.3 Implement `replace_git_paw_section(content: &str, new_section: &str) -> String` — replaces start-to-end markers inclusive, falls back to start-to-EOF if end marker missing
- [ ] 3.4 Implement `inject_into_content(content: &str, section: &str) -> String` — appends if absent, replaces if present, handles spacing

## 4. File I/O Wrapper

- [ ] 4.1 Implement `inject_section_into_file(path: &Path, section: &str) -> Result<(), PawError>` — reads file (or empty if missing), injects, writes back
- [ ] 4.2 Map I/O errors to `PawError::AgentsMdError` with file path context

## 5. Unit Tests (pure functions)

- [ ] 5.1 Test `has_git_paw_section`: content with marker returns true, without returns false, empty returns false
- [ ] 5.2 Test `generate_git_paw_section`: output contains start marker, end marker, and guidance content
- [ ] 5.3 Test `replace_git_paw_section`: both markers present — content replaced, surrounding preserved
- [ ] 5.4 Test `replace_git_paw_section`: missing end marker — replaces start to EOF
- [ ] 5.5 Test `inject_into_content`: no existing section — appends with spacing
- [ ] 5.6 Test `inject_into_content`: existing section — replaces in place
- [ ] 5.7 Test `inject_into_content`: empty content — returns section only
- [ ] 5.8 Test spacing: content ending with newline gets blank line separator, content without newline gets newline + blank line

## 6. File I/O Tests (tempfile)

- [ ] 6.1 Test `inject_section_into_file`: file exists without section — appends
- [ ] 6.2 Test `inject_section_into_file`: file exists with section — replaces
- [ ] 6.3 Test `inject_section_into_file`: file does not exist — creates
- [ ] 6.4 Test `inject_section_into_file`: read-only file — returns AgentsMdError
- [ ] 6.5 Run `cargo clippy -- -D warnings` and `cargo fmt --check` — clean

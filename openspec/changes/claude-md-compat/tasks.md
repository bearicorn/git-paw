## 1. Config Types

- [ ] 1.1 Add `ClaudeMdMode` enum to `src/config.rs` with variants `Symlink`, `Copy`, `Skip` (default), deriving `Serialize`, `Deserialize`, `Default`, `Clone`, `PartialEq`, `Eq`
- [ ] 1.2 Add `claude_md: Option<ClaudeMdMode>` field to `PawConfig` with `serde(default)`
- [ ] 1.3 Update `PawConfig::merged_with()` to handle `claude_md` field
- [ ] 1.4 Unit tests: parse config with all three `claude_md` values, default when absent, merge override, round-trip

## 2. File State Detection

- [ ] 2.1 Add `AgentFileState` enum to `src/agents.rs` with variants: `ClaudeMdOnly`, `AgentsMdOnly`, `BothExist`, `NeitherExists`
- [ ] 2.2 Implement `detect_agent_file_state(dir: &Path) -> AgentFileState` using `symlink_metadata()`

## 3. Init-Time Prompt

- [ ] 3.1 In `src/init.rs`, after detecting `CLAUDE.md` exists: check if `claude_md` is already set in config
- [ ] 3.2 If not set → prompt user with dialoguer: "CLAUDE.md detected but no AGENTS.md. Other AI CLIs read AGENTS.md. How should git-paw handle this?" with three options:
  - "Symlink — Create AGENTS.md as a symlink to CLAUDE.md (same content for all CLIs)"
  - "Copy — Copy CLAUDE.md to AGENTS.md (separate files, you manage both)"
  - "Skip — Create a fresh AGENTS.md (other CLIs won't see your CLAUDE.md content)"
- [ ] 3.3 Adjust prompt text when both files exist (no need to explain missing AGENTS.md)
- [ ] 3.4 Persist the choice to `.git-paw/config.toml`
- [ ] 3.5 If already set in config → skip prompt, use existing value
- [ ] 3.6 If no CLAUDE.md exists → skip prompt, no mode needed

## 4. Root Compatibility

- [ ] 4.1 Implement `handle_claude_md_compat_root(repo_root: &Path, mode: ClaudeMdMode) -> Result<(), PawError>` in `src/agents.rs`
- [ ] 4.2 Symlink + ClaudeMdOnly: inject section into CLAUDE.md, create AGENTS.md symlink → CLAUDE.md
- [ ] 4.3 Copy + ClaudeMdOnly: copy CLAUDE.md to AGENTS.md, inject section into both
- [ ] 4.4 Skip + ClaudeMdOnly: create fresh AGENTS.md with section, inject section into CLAUDE.md
- [ ] 4.5 BothExist + Symlink: inject into AGENTS.md only (can't symlink when both are regular files)
- [ ] 4.6 BothExist + Copy: inject into both independently
- [ ] 4.7 BothExist + Skip: inject into AGENTS.md only
- [ ] 4.8 AgentsMdOnly / NeitherExists: inject into AGENTS.md (create if needed), no CLAUDE.md action
- [ ] 4.9 Handle existing symlink: inject into symlink target, preserve symlink

## 5. Worktree Compatibility

- [ ] 5.1 Implement `handle_claude_md_compat_worktree(worktree_root: &Path, cli: &str, mode: ClaudeMdMode) -> Result<(), PawError>` in `src/agents.rs`
- [ ] 5.2 Symlink + cli=claude → symlink `CLAUDE.md → AGENTS.md` in worktree
- [ ] 5.3 Copy + cli=claude → copy worktree AGENTS.md content to CLAUDE.md
- [ ] 5.4 Skip + cli=claude → create CLAUDE.md with only the git-paw assignment section
- [ ] 5.5 Non-claude CLI → no-op regardless of mode
- [ ] 5.6 Existing CLAUDE.md → no-op (don't overwrite)

## 6. Symlink Helpers

- [ ] 6.1 Implement `create_symlink_safe(link: &Path, target: &Path) -> Result<bool, PawError>` — Ok(true) if created, Ok(false) if skipped, Err on failure
- [ ] 6.2 Check `symlink_metadata(link)` before creating: skip if correct symlink, skip if regular file
- [ ] 6.3 Use `#[cfg(unix)]` for `std::os::unix::fs::symlink` import

## 7. Unit Tests

- [ ] 7.1 Test `detect_agent_file_state`: all four variants detected correctly
- [ ] 7.2 Test `detect_agent_file_state`: symlink counts as existing (BothExist)
- [ ] 7.3 Test `create_symlink_safe`: creates, skips correct symlink, skips regular file

## 8. Integration Tests (tempfile)

- [ ] 8.1 Symlink root: CLAUDE.md only → section in CLAUDE.md + AGENTS.md symlink
- [ ] 8.2 Copy root: CLAUDE.md only → CLAUDE.md copied to AGENTS.md, section in both
- [ ] 8.3 Skip root: CLAUDE.md only → fresh AGENTS.md with section, section in CLAUDE.md
- [ ] 8.4 Symlink root: both exist → section in AGENTS.md only
- [ ] 8.5 Copy root: both exist → section in both
- [ ] 8.6 Skip root: both exist → section in AGENTS.md only
- [ ] 8.7 Existing symlink → injects into target, symlink preserved
- [ ] 8.8 Symlink worktree: cli=claude → CLAUDE.md symlink
- [ ] 8.9 Copy worktree: cli=claude → CLAUDE.md copy
- [ ] 8.10 Skip worktree: cli=claude → CLAUDE.md with assignment only
- [ ] 8.11 Non-claude CLI → no CLAUDE.md regardless of mode
- [ ] 8.12 Idempotent: init after file state transition
- [ ] 8.13 Run `cargo clippy -- -D warnings` and `cargo fmt --check` — clean

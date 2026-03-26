## 1. Data Types

- [ ] 1.1 Add `WorktreeAssignment` struct to `src/agents.rs` with fields: `branch: String`, `cli: String`, `spec_content: Option<String>`, `owned_files: Option<Vec<String>>`
- [ ] 1.2 Add doc comments to `WorktreeAssignment` and all fields

## 2. Worktree Section Generation

- [ ] 2.1 Implement `generate_worktree_section(assignment: &WorktreeAssignment) -> String` in `src/agents.rs` — generates marker-delimited assignment section
- [ ] 2.2 Include branch, CLI, optional Spec section (if `spec_content` is Some), and optional File Ownership section (if `owned_files` is Some)
- [ ] 2.3 Use the same `<!-- git-paw:start -->` / `<!-- git-paw:end -->` markers as the root section

## 3. Worktree AGENTS.md Setup

- [ ] 3.1 Implement `setup_worktree_agents_md(repo_root: &Path, worktree_root: &Path, assignment: &WorktreeAssignment) -> Result<(), PawError>` in `src/agents.rs`
- [ ] 3.2 Read root AGENTS.md from `repo_root` (empty string if file does not exist)
- [ ] 3.3 Use `inject_into_content()` to combine root content with the worktree section (replaces root git-paw section if present)
- [ ] 3.4 Write combined content to `worktree_root/AGENTS.md`
- [ ] 3.5 Call `exclude_from_git()` to prevent committing

## 4. Git Exclude Management

- [ ] 4.1 Implement `exclude_from_git(worktree_root: &Path, filename: &str) -> Result<(), PawError>` in `src/agents.rs`
- [ ] 4.2 Create `.git/info/` directory if it does not exist
- [ ] 4.3 Read `.git/info/exclude` (empty if missing), check if entry already present, append if not
- [ ] 4.4 Map I/O errors to `PawError::AgentsMdError`

## 5. Unit Tests

- [ ] 5.1 Test `generate_worktree_section`: all fields present — output contains branch, CLI, spec, files, markers
- [ ] 5.2 Test `generate_worktree_section`: no spec content — Spec section omitted
- [ ] 5.3 Test `generate_worktree_section`: no owned files — File Ownership section omitted
- [ ] 5.4 Test `generate_worktree_section`: minimal (branch + CLI only)

## 6. File I/O Tests (tempfile)

- [ ] 6.1 Test `setup_worktree_agents_md`: root AGENTS.md exists — worktree gets combined content
- [ ] 6.2 Test `setup_worktree_agents_md`: root AGENTS.md missing — worktree gets assignment only
- [ ] 6.3 Test `setup_worktree_agents_md`: root has git-paw section — section replaced not duplicated
- [ ] 6.4 Test `exclude_from_git`: exclude file created when missing
- [ ] 6.5 Test `exclude_from_git`: entry appended when not present
- [ ] 6.6 Test `exclude_from_git`: entry not duplicated when already present
- [ ] 6.7 Test `exclude_from_git`: .git/info directory created when missing
- [ ] 6.8 Run `cargo clippy -- -D warnings` and `cargo fmt --check` — clean

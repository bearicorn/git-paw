# Design: --force Flag Implementation

## Technical Approach

### 1. CLI Layer Changes (`src/cli.rs`)

**Add force flag to Start struct:**

```rust
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Launch a new session or reattach to an existing one
    Start {
        /// AI CLI to use (e.g., claude, codex, gemini). Skips CLI picker if provided.
        #[arg(long, help = "AI CLI to use (skips CLI picker)")]
        cli: Option<String>,

        /// Comma-separated branch names. Skips branch picker if provided.
        #[arg(
            long,
            value_delimiter = ',',
            help = "Comma-separated branches (skips branch picker)"
        )]
        branches: Option<Vec<String>>,

        /// Launch from spec files instead of interactive selection.
        #[arg(
            long,
            help = "Launch from spec files (reads .git-paw/config.toml [specs])"
        )]
        from_specs: bool,

        /// Preview the session plan without executing.
        #[arg(long, help = "Preview the session plan without executing")]
        dry_run: bool,

        /// Use a named preset from config.
        #[arg(long, help = "Use a named preset from config")]
        preset: Option<String>,

        /// Enable supervisor mode for this session.
        #[arg(
            long,
            default_value_t = false,
            help = "Enable supervisor mode for this session"
        )]
        supervisor: bool,

        /// Bypass uncommitted-spec validation warning.
        #[arg(
            long,
            help = "Bypass uncommitted-spec validation warning"
        )]
        force: bool,
    },
    // ... rest of enum
}
```

### 2. Validation Logic (`src/main.rs`)

**Add validation to `cmd_start_from_specs`:**

```rust
fn cmd_start_from_specs(cli_flag: Option<&str>, dry_run: bool, force: bool) -> Result<(), PawError> {
    let cwd = std::env::current_dir()?;
    let repo_root = git::validate_repo(&cwd)?;

    // Check for existing session (skip reattach/recovery during dry-run)
    let existing_session = session::find_session_for_repo(&repo_root)?;
    if !dry_run && let Some(existing) = &existing_session {
        // ... existing session handling
    }

    // Fresh launch from specs (or dry-run preview)
    tmux::ensure_tmux_installed()?;
    let config = config::load_config(&repo_root)?;

    // Scan for pending specs
    let specs = git_paw::specs::scan_specs(&config, &repo_root)?;

    if specs.is_empty() {
        println!("No pending specs found.");
        return Ok(());
    }

    // NEW: Validate that specs are committed
    if !force {
        let uncommitted_specs = check_for_uncommitted_specs(&repo_root, &specs)?;
        if !uncommitted_specs.is_empty() {
            eprintln!(
                "warning: Uncommitted spec changes detected in: {}\n         Commit your changes or use --force to proceed",
                uncommitted_specs.join(", ")
            );
            // Continue with warning but don't block
        }
    } else {
        eprintln!("Proceeding with --force flag (uncommitted spec changes ignored)");
        // Log force usage for audit
        log::warn!("User bypassed uncommitted-spec validation with --force flag");
    }

    // ... rest of existing launch logic
}
```

### 3. Git Status Helper Function

**Add to `src/git.rs`:**

```rust
/// Checks if spec directories have uncommitted changes.
///
/// Returns a list of spec IDs that have uncommitted changes.
pub fn check_for_uncommitted_specs(repo_root: &Path, specs: &[SpecEntry]) -> Result<Vec<String>, PawError> {
    let mut uncommitted = Vec::new();

    for spec in specs {
        let spec_path = repo_root.join("openspec").join("changes").join(&spec.id);
        if !spec_path.exists() {
            continue;
        }

        // Check git status for this spec directory
        let output = StdCommand::new("git")
            .args(["status", "--porcelain", spec_path.to_str().unwrap_or("")])
            .current_dir(repo_root)
            .output()
            .map_err(|e| PawError::GitError(format!("failed to check git status: {e}")))?;

        if !output.status.success() {
            return Err(PawError::GitError(format!(
                "git status failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let status_output = String::from_utf8_lossy(&output.stdout);
        if !status_output.trim().is_empty() {
            uncommitted.push(spec.id.clone());
        }
    }

    Ok(uncommitted)
}
```

### 4. Error Handling

**Add to `src/error.rs`:**

```rust
#[derive(Debug, thiserror::Error)]
pub enum PawError {
    // ... existing variants

    #[error("Git operation failed: {0}")]
    GitError(String),

    #[error("Uncommitted spec changes detected: {0}")]
    UncommittedSpecs(String),
}
```

### 5. Test Strategy

**Unit Tests:**

1. **CLI Parsing**: Test that `--force` flag is parsed correctly
2. **Validation Logic**: Test uncommitted spec detection
3. **Force Behavior**: Test that force bypasses warning
4. **Integration**: Test end-to-end flow with mock git repo

**Test Cases:**

```rust
#[test]
fn start_with_force_flag() {
    let cli = parse(&["start", "--from-specs", "--force"]);
    match cli.command.unwrap() {
        Command::Start { force, .. } => assert!(force),
        other => panic!("expected Start, got {other:?}"),
    }
}

#[test]
fn check_uncommitted_specs_detects_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(tmp.path()).unwrap();

    // Create a spec with uncommitted changes
    let spec_dir = tmp.path().join("openspec").join("changes").join("test-spec");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::write(spec_dir.join("tasks.md"), "uncommitted content").unwrap();

    let specs = vec![SpecEntry {
        id: "test-spec".to_string(),
        branch: "spec/test-spec".to_string(),
        cli: None,
        prompt: "test".to_string(),
        owned_files: None,
    }];

    let uncommitted = check_for_uncommitted_specs(tmp.path(), &specs).unwrap();
    assert_eq!(uncommitted, vec!["test-spec"]);
}
```

## Implementation Sequence

1. ✅ Create OpenSpec change structure
2. ✅ Write proposal.md
3. ✅ Write design.md
4. Add formal specification
5. Implement CLI changes
6. Implement validation logic
7. Add helper functions
8. Write tests
9. Update documentation

## Risk Assessment

**Low Risk:**
- Follows existing patterns (`purge --force`)
- Minimal changes to core logic
- Backward compatible
- Easy to test

**Mitigation:**
- Comprehensive test coverage
- Feature flag approach (opt-in)
- Clear documentation
- Audit logging

## Rollback Plan

If issues arise:
1. Feature can be disabled by removing the flag
2. No database migrations or breaking changes
3. Simple code removal if needed
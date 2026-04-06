//! Project initialization.
//!
//! Implements `git paw init` — creates `.git-paw/` directory, generates
//! default config, and manages `.gitignore`.

use std::fs;
use std::path::Path;

use crate::config;
use crate::error::PawError;
use crate::git;

/// Gitignore entry for session logs.
const GITIGNORE_ENTRY: &str = ".git-paw/logs/";

/// Runs the `git paw init` command.
///
/// Creates `.git-paw/` directory structure, generates a default config,
/// and manages `.gitignore`. Idempotent — running twice produces identical
/// results.
pub fn run_init() -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::InitError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    let paw_dir = repo_root.join(".git-paw");
    let logs_dir = paw_dir.join("logs");
    let config_path = paw_dir.join("config.toml");

    // 1. Create .git-paw/ directory
    let created_dir = create_dir_if_missing(&paw_dir)?;
    if created_dir {
        println!("  Created .git-paw/");
    }

    // 2. Create .git-paw/logs/ directory
    let created_logs = create_dir_if_missing(&logs_dir)?;
    if created_logs {
        println!("  Created .git-paw/logs/");
    }

    // 3. Generate default config (only if not present)
    let created_config = write_config_if_missing(&config_path)?;
    if created_config {
        println!("  Created .git-paw/config.toml");
    }

    // 4. Manage .gitignore
    let updated_gitignore = ensure_gitignore_entry(&repo_root)?;
    if updated_gitignore {
        println!("  Updated .gitignore");
    }

    if !created_dir && !created_logs && !created_config && !updated_gitignore {
        println!("Already initialized. Nothing to do.");
    } else {
        println!("Initialized git-paw.");
    }

    Ok(())
}

/// Creates a directory if it doesn't exist. Returns `true` if created.
fn create_dir_if_missing(path: &Path) -> Result<bool, PawError> {
    if path.is_dir() {
        return Ok(false);
    }
    fs::create_dir_all(path)
        .map_err(|e| PawError::InitError(format!("failed to create '{}': {e}", path.display())))?;
    Ok(true)
}

/// Writes the default config if the file doesn't already exist. Returns `true` if written.
fn write_config_if_missing(path: &Path) -> Result<bool, PawError> {
    if path.exists() {
        return Ok(false);
    }
    let content = config::generate_default_config();
    fs::write(path, content)
        .map_err(|e| PawError::InitError(format!("failed to write config: {e}")))?;
    Ok(true)
}

/// Ensures `.gitignore` contains the logs entry. Returns `true` if modified.
fn ensure_gitignore_entry(repo_root: &Path) -> Result<bool, PawError> {
    let gitignore_path = repo_root.join(".gitignore");

    let existing = match fs::read_to_string(&gitignore_path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(PawError::InitError(format!(
                "failed to read .gitignore: {e}"
            )));
        }
    };

    // Check if already present
    if existing.lines().any(|line| line.trim() == GITIGNORE_ENTRY) {
        return Ok(false);
    }

    // Append entry, ensuring proper newline handling
    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(GITIGNORE_ENTRY);
    content.push('\n');

    fs::write(&gitignore_path, content)
        .map_err(|e| PawError::InitError(format!("failed to write .gitignore: {e}")))?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        // Create a minimal .git dir so validate_repo-like checks work
        fs::create_dir(dir.path().join(".git")).unwrap();
        dir
    }

    // --- create_dir_if_missing ---

    #[test]
    fn creates_directory_when_missing() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("new-dir");
        assert!(create_dir_if_missing(&target).unwrap());
        assert!(target.is_dir());
    }

    #[test]
    fn skips_existing_directory() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("existing");
        fs::create_dir(&target).unwrap();
        assert!(!create_dir_if_missing(&target).unwrap());
    }

    // --- write_config_if_missing ---

    #[test]
    fn writes_config_when_missing() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        assert!(write_config_if_missing(&config_path).unwrap());
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("default_cli"));
    }

    #[test]
    fn skips_existing_config() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, "existing").unwrap();
        assert!(!write_config_if_missing(&config_path).unwrap());
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "existing");
    }

    // --- ensure_gitignore_entry ---

    #[test]
    fn creates_gitignore_with_entry() {
        let dir = setup_repo();
        assert!(ensure_gitignore_entry(dir.path()).unwrap());
        let content = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains(GITIGNORE_ENTRY));
    }

    #[test]
    fn appends_to_existing_gitignore() {
        let dir = setup_repo();
        fs::write(dir.path().join(".gitignore"), "node_modules/\n").unwrap();
        assert!(ensure_gitignore_entry(dir.path()).unwrap());
        let content = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains("node_modules/"));
        assert!(content.contains(GITIGNORE_ENTRY));
    }

    #[test]
    fn appends_newline_if_missing() {
        let dir = setup_repo();
        fs::write(dir.path().join(".gitignore"), "node_modules/").unwrap();
        assert!(ensure_gitignore_entry(dir.path()).unwrap());
        let content = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains("node_modules/\n"));
        assert!(content.contains(GITIGNORE_ENTRY));
    }

    #[test]
    fn skips_when_entry_already_present() {
        let dir = setup_repo();
        fs::write(
            dir.path().join(".gitignore"),
            format!("node_modules/\n{GITIGNORE_ENTRY}\n"),
        )
        .unwrap();
        assert!(!ensure_gitignore_entry(dir.path()).unwrap());
    }

    // --- Idempotency ---

    #[test]
    fn idempotent_gitignore() {
        let dir = setup_repo();
        ensure_gitignore_entry(dir.path()).unwrap();
        let first = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        ensure_gitignore_entry(dir.path()).unwrap();
        let second = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(first, second);
    }
}

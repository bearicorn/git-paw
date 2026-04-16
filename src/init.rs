//! Project initialization.
//!
//! Implements `git paw init` — creates `.git-paw/` directory, generates
//! default config, and manages `.gitignore`.

use std::fmt::Write as _;
use std::fs;
use std::io::IsTerminal;
use std::path::Path;

use dialoguer::{Confirm, Input};

use crate::config;
use crate::error::PawError;
use crate::git;

/// Gitignore entries managed by init.
const GITIGNORE_ENTRIES: &[&str] = &[".git-paw/logs/", ".git-paw/session-summary.md"];

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

    // 3. Generate or migrate config. For a fresh config, prompt for supervisor
    //    preferences. For an existing config without a [supervisor] section,
    //    append one (prompting if stdin is interactive). Init never mutates
    //    existing sections — only appends missing ones.
    let (created_config, migrated_config) = if config_path.exists() {
        let migrated = migrate_existing_config(&config_path)?;
        (false, migrated)
    } else {
        let supervisor_section = prompt_supervisor_section()?;
        write_config_if_missing(&config_path, Some(&supervisor_section))?;
        (true, false)
    };
    if created_config {
        println!("  Created .git-paw/config.toml");
    } else if migrated_config {
        println!("  Updated .git-paw/config.toml (added missing sections)");
    }

    // 4. Manage .gitignore
    let updated_gitignore = ensure_gitignore_entry(&repo_root)?;
    if updated_gitignore {
        println!("  Updated .gitignore");
    }

    if !created_dir && !created_logs && !created_config && !migrated_config && !updated_gitignore {
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

/// Appends any missing sections to an existing `config.toml`. Returns `true`
/// if the file was modified. Does not touch any existing field — this is the
/// safe upgrade path for new config sections added across versions.
fn migrate_existing_config(path: &Path) -> Result<bool, PawError> {
    let existing = fs::read_to_string(path)
        .map_err(|e| PawError::InitError(format!("failed to read config: {e}")))?;

    let mut appended = String::new();

    // [supervisor] — the only section currently managed by migration. We
    // detect presence with a simple line-based scan rather than parsing TOML
    // so we don't lose comments or reorder fields on round-trip.
    if !has_section(&existing, "supervisor") {
        let section = prompt_supervisor_section()?;
        appended.push_str(&section);
    }

    if appended.is_empty() {
        return Ok(false);
    }

    let mut new_content = existing;
    if !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    new_content.push_str(&appended);

    fs::write(path, new_content)
        .map_err(|e| PawError::InitError(format!("failed to write config: {e}")))?;
    Ok(true)
}

/// Returns `true` if a non-commented `[section]` header exists in `content`.
fn has_section(content: &str, section: &str) -> bool {
    let header = format!("[{section}]");
    content.lines().any(|line| {
        let trimmed = line.trim_start();
        !trimmed.starts_with('#') && trimmed.trim_end() == header
    })
}

/// Writes the default config if the file doesn't already exist. Returns `true` if written.
///
/// If `supervisor_section` is `Some`, it is appended to the generated config so
/// the user's init-time choice is persisted.
fn write_config_if_missing(
    path: &Path,
    supervisor_section: Option<&str>,
) -> Result<bool, PawError> {
    if path.exists() {
        return Ok(false);
    }
    let mut content = config::generate_default_config();
    if let Some(section) = supervisor_section {
        content.push_str(section);
    }
    fs::write(path, content)
        .map_err(|e| PawError::InitError(format!("failed to write config: {e}")))?;
    Ok(true)
}

/// Prompts the user for their supervisor preferences and returns a TOML
/// `[supervisor]` section to append to the generated config.
///
/// If the user declines, an explicit `enabled = false` section is returned so
/// that future `git paw start` calls do not re-prompt.
fn prompt_supervisor_section() -> Result<String, PawError> {
    // In non-interactive contexts (CI, tests, piped stdin) fall back to an
    // explicit opt-out so init remains scriptable.
    if !std::io::stdin().is_terminal() {
        return Ok("\n[supervisor]\nenabled = false\n".to_string());
    }

    let enabled = Confirm::new()
        .with_prompt("Enable supervisor mode by default?")
        .default(false)
        .interact()
        .map_err(|e| PawError::InitError(format!("prompt failed: {e}")))?;

    if !enabled {
        return Ok("\n[supervisor]\nenabled = false\n".to_string());
    }

    let test_command: String = Input::new()
        .with_prompt("Test command to run after each agent completes (e.g. 'just check', leave empty to skip)")
        .allow_empty(true)
        .interact_text()
        .map_err(|e| PawError::InitError(format!("prompt failed: {e}")))?;

    let mut section = String::from("\n[supervisor]\nenabled = true\n");
    let trimmed = test_command.trim();
    if !trimmed.is_empty() {
        let escaped = trimmed.replace('\\', "\\\\").replace('"', "\\\"");
        writeln!(section, "test_command = \"{escaped}\"")
            .map_err(|e| PawError::InitError(format!("format supervisor section: {e}")))?;
    }
    Ok(section)
}

/// Ensures `.gitignore` contains all managed entries. Returns `true` if modified.
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

    let existing_lines: std::collections::HashSet<&str> = existing.lines().map(str::trim).collect();
    let missing: Vec<&&str> = GITIGNORE_ENTRIES
        .iter()
        .filter(|e| !existing_lines.contains(**e))
        .collect();

    if missing.is_empty() {
        return Ok(false);
    }

    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    for entry in missing {
        content.push_str(entry);
        content.push('\n');
    }

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
        assert!(write_config_if_missing(&config_path, None).unwrap());
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("default_cli"));
    }

    #[test]
    fn skips_existing_config() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, "existing").unwrap();
        assert!(!write_config_if_missing(&config_path, None).unwrap());
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "existing");
    }

    #[test]
    fn appends_supervisor_section_when_provided() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        let section = "\n[supervisor]\nenabled = true\ntest_command = \"just check\"\n";
        assert!(write_config_if_missing(&config_path, Some(section)).unwrap());

        let content = fs::read_to_string(&config_path).unwrap();
        let parsed: crate::config::PawConfig = toml::from_str(&content).unwrap();
        let supervisor = parsed.supervisor.unwrap();
        assert!(supervisor.enabled);
        assert_eq!(supervisor.test_command.as_deref(), Some("just check"));
    }

    #[test]
    fn appends_disabled_supervisor_section() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        let section = "\n[supervisor]\nenabled = false\n";
        assert!(write_config_if_missing(&config_path, Some(section)).unwrap());

        let content = fs::read_to_string(&config_path).unwrap();
        let parsed: crate::config::PawConfig = toml::from_str(&content).unwrap();
        let supervisor = parsed.supervisor.unwrap();
        assert!(!supervisor.enabled);
    }

    // --- ensure_gitignore_entry ---

    #[test]
    fn creates_gitignore_with_entry() {
        let dir = setup_repo();
        assert!(ensure_gitignore_entry(dir.path()).unwrap());
        let content = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        for entry in GITIGNORE_ENTRIES {
            assert!(content.contains(entry), "missing {entry}");
        }
    }

    #[test]
    fn appends_to_existing_gitignore() {
        let dir = setup_repo();
        fs::write(dir.path().join(".gitignore"), "node_modules/\n").unwrap();
        assert!(ensure_gitignore_entry(dir.path()).unwrap());
        let content = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains("node_modules/"));
        for entry in GITIGNORE_ENTRIES {
            assert!(content.contains(entry), "missing {entry}");
        }
    }

    #[test]
    fn appends_newline_if_missing() {
        let dir = setup_repo();
        fs::write(dir.path().join(".gitignore"), "node_modules/").unwrap();
        assert!(ensure_gitignore_entry(dir.path()).unwrap());
        let content = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains("node_modules/\n"));
        for entry in GITIGNORE_ENTRIES {
            assert!(content.contains(entry), "missing {entry}");
        }
    }

    #[test]
    fn skips_when_all_entries_already_present() {
        let dir = setup_repo();
        let mut lines = String::from("node_modules/\n");
        for entry in GITIGNORE_ENTRIES {
            lines.push_str(entry);
            lines.push('\n');
        }
        fs::write(dir.path().join(".gitignore"), lines).unwrap();
        assert!(!ensure_gitignore_entry(dir.path()).unwrap());
    }

    #[test]
    fn session_summary_added_alongside_logs() {
        let dir = setup_repo();
        fs::write(dir.path().join(".gitignore"), ".git-paw/logs/\n").unwrap();
        assert!(ensure_gitignore_entry(dir.path()).unwrap());
        let content = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains(".git-paw/session-summary.md"));
        assert_eq!(content.matches(".git-paw/logs/").count(), 1);
    }

    // --- migrate_existing_config ---

    #[test]
    fn has_section_detects_active_header() {
        assert!(has_section("[supervisor]\nenabled = true\n", "supervisor"));
        assert!(!has_section("# [supervisor]\n", "supervisor"));
        assert!(!has_section("[broker]\n", "supervisor"));
    }

    /// Migration does not touch existing sections. A config already containing
    /// `[supervisor]` plus a custom `[broker]` port must round-trip with both
    /// sections and the custom port intact.
    #[test]
    fn migrate_preserves_existing_supervisor_and_custom_broker_port() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        let initial = r#"[broker]
enabled = true
port = 12345

[supervisor]
enabled = true
cli = "echo"
"#;
        fs::write(&config_path, initial).unwrap();

        let modified = migrate_existing_config(&config_path).unwrap();
        assert!(
            !modified,
            "migrate must be a no-op when [supervisor] already exists"
        );

        let after = fs::read_to_string(&config_path).unwrap();
        assert!(
            after.contains("port = 12345"),
            "custom broker port must be preserved verbatim; got:\n{after}"
        );
        assert!(
            after.contains("[supervisor]"),
            "supervisor header must be preserved; got:\n{after}"
        );
        assert!(
            after.contains("cli = \"echo\""),
            "supervisor cli must be preserved; got:\n{after}"
        );

        // The TOML must still parse to a config with the expected fields.
        let parsed: crate::config::PawConfig = toml::from_str(&after).unwrap();
        let supervisor = parsed.supervisor.expect("supervisor present");
        assert!(supervisor.enabled);
        assert_eq!(supervisor.cli.as_deref(), Some("echo"));
        assert_eq!(parsed.broker.port, 12345);
    }

    /// When `[supervisor]` is missing, migrate appends a section. Stdin in
    /// tests is non-interactive, so the appended section is the explicit
    /// opt-out (`enabled = false`). The pre-existing `[broker]` section and
    /// its custom port must remain untouched.
    #[test]
    fn migrate_appends_supervisor_section_when_missing_and_keeps_broker_port() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        let initial = "[broker]\nenabled = true\nport = 9119\n";
        fs::write(&config_path, initial).unwrap();

        let modified = migrate_existing_config(&config_path).unwrap();
        assert!(
            modified,
            "migrate must report that the file was modified when appending"
        );

        let after = fs::read_to_string(&config_path).unwrap();
        // Original section preserved.
        assert!(
            after.contains("port = 9119"),
            "broker port must survive migration; got:\n{after}"
        );
        // Section appended.
        assert!(
            after.contains("[supervisor]"),
            "supervisor section must be appended; got:\n{after}"
        );

        let parsed: crate::config::PawConfig = toml::from_str(&after).unwrap();
        let supervisor = parsed.supervisor.expect("supervisor present");
        assert!(
            !supervisor.enabled,
            "non-interactive migrate should opt out by default"
        );
        assert_eq!(parsed.broker.port, 9119);
    }

    /// Running migrate twice must produce identical content — the second run
    /// has nothing to do.
    #[test]
    fn migrate_existing_config_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, "[broker]\nenabled = true\nport = 9119\n").unwrap();

        migrate_existing_config(&config_path).unwrap();
        let first = fs::read_to_string(&config_path).unwrap();
        let modified = migrate_existing_config(&config_path).unwrap();
        let second = fs::read_to_string(&config_path).unwrap();

        assert!(!modified, "second migrate must be a no-op");
        assert_eq!(first, second);
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

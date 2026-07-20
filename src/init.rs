//! Project initialization.
//!
//! Implements `git paw init` — creates `.git-paw/` directory, generates
//! default config, and manages `.gitignore`.

use std::fmt::Write as _;
use std::fs;
use std::io::IsTerminal;
use std::path::Path;

use dialoguer::{Confirm, Input, Select};

use crate::config;
use crate::error::PawError;
use crate::git;

/// Gitignore entries managed by init. `.git-paw/tmp/` is the repo-local
/// scratch dir (isolated verify worktrees, self-test sessions) — preferred
/// over OS temp because it is OS-independent, so it must never be committed.
const GITIGNORE_ENTRIES: &[&str] = &[
    ".git-paw/logs/",
    ".git-paw/tmp/",
    ".git-paw/worktrees/",
    ".git-paw/session-summary.md",
    ".git-paw/session-learnings.md",
];

/// Bundled supervisor-sweep helper script, embedded at compile time and
/// written to `<repo>/.git-paw/scripts/sweep.sh` by [`run_init`].
const SWEEP_SCRIPT: &str = include_str!("../assets/scripts/sweep.sh");

/// Bundled agent-side broker helper script, embedded at compile time and
/// written to `<repo>/.git-paw/scripts/broker.sh` by [`run_init`]. The
/// analogue of [`SWEEP_SCRIPT`] for the coding-agent side — it wraps every
/// agent→broker `curl` so the boot block calls one stable script path
/// instead of inlining raw `curl` commands.
const BROKER_SCRIPT: &str = include_str!("../assets/scripts/broker.sh");

/// Bundled agent-side docs-fetch helper script, embedded at compile time and
/// written to `<repo>/.git-paw/scripts/docs-fetch.sh` by [`run_init`]. Like
/// [`BROKER_SCRIPT`], it wraps the agent's `curl` behind one stable script
/// path so the launch path can grant that exact path instead of a broad
/// `curl *` rule.
const DOCS_FETCH_SCRIPT: &str = include_str!("../assets/scripts/docs-fetch.sh");

/// Runs the `git paw init` command.
///
/// Creates `.git-paw/` directory structure, generates a default config,
/// installs the bundled `sweep.sh` supervisor helper at
/// `<repo>/.git-paw/scripts/sweep.sh` (executable mode `0o755` on Unix),
/// and manages `.gitignore`. The script is overwritten on every invocation
/// so re-running `git paw init` picks up updates that ship with new
/// versions of the binary. Idempotent for the other side effects —
/// running twice produces identical results for the directory tree,
/// `config.toml`, and `.gitignore`.
pub fn run_init() -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::InitError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    let paw_dir = repo_root.join(".git-paw");
    let logs_dir = paw_dir.join("logs");
    let tmp_dir = paw_dir.join("tmp");
    let scripts_dir = paw_dir.join("scripts");
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

    // 2b. Create .git-paw/tmp/ — repo-local scratch for isolated verify
    //     worktrees and self-test sessions (preferred over OS temp).
    let created_tmp = create_dir_if_missing(&tmp_dir)?;
    if created_tmp {
        println!("  Created .git-paw/tmp/");
    }

    // 3. Create .git-paw/scripts/ directory and install the bundled helpers
    //    (sweep.sh for the supervisor, broker.sh and docs-fetch.sh for the
    //    coding agents).
    let created_scripts = create_dir_if_missing(&scripts_dir)?;
    if created_scripts {
        println!("  Created .git-paw/scripts/");
    }
    let sweep_path = scripts_dir.join("sweep.sh");
    let sweep_existed = sweep_path.exists();
    install_script(&sweep_path, SWEEP_SCRIPT)?;
    if sweep_existed {
        println!("  Updated .git-paw/scripts/sweep.sh");
    } else {
        println!("  Created .git-paw/scripts/sweep.sh");
    }
    let broker_path = scripts_dir.join("broker.sh");
    let broker_existed = broker_path.exists();
    install_script(&broker_path, BROKER_SCRIPT)?;
    if broker_existed {
        println!("  Updated .git-paw/scripts/broker.sh");
    } else {
        println!("  Created .git-paw/scripts/broker.sh");
    }
    let docs_fetch_path = scripts_dir.join("docs-fetch.sh");
    let docs_fetch_existed = docs_fetch_path.exists();
    install_script(&docs_fetch_path, DOCS_FETCH_SCRIPT)?;
    if docs_fetch_existed {
        println!("  Updated .git-paw/scripts/docs-fetch.sh");
    } else {
        println!("  Created .git-paw/scripts/docs-fetch.sh");
    }

    // 4. Generate or migrate config. For a fresh config, prompt for supervisor
    //    preferences and for the spec system to record in `[specs]`. For an
    //    existing config without a [supervisor] section, append one (prompting
    //    if stdin is interactive). Init never mutates existing sections — only
    //    appends missing ones.
    let (created_config, migrated_config) = if config_path.exists() {
        let migrated = migrate_existing_config(&config_path)?;
        (false, migrated)
    } else {
        let supervisor_section = prompt_supervisor_section()?;
        // The config is the source of truth for the spec system, so ASK the
        // user to choose it at init (no filesystem auto-detection). A
        // non-interactive init writes a commented template to fill in later.
        let specs_section = Some(prompt_specs_section()?);
        write_config_if_missing(
            &config_path,
            Some(&supervisor_section),
            specs_section.as_deref(),
        )?;
        (true, false)
    };
    if created_config {
        println!("  Created .git-paw/config.toml");
    } else if migrated_config {
        println!("  Updated .git-paw/config.toml (added missing sections)");
    }

    // 5. Manage .gitignore
    let updated_gitignore = ensure_gitignore_entry(&repo_root)?;
    if updated_gitignore {
        println!("  Updated .gitignore");
    }

    if !created_dir
        && !created_logs
        && !created_tmp
        && !created_config
        && !migrated_config
        && !updated_gitignore
    {
        println!("Already initialized. Nothing to do.");
    } else {
        println!("Initialized git-paw.");
    }

    Ok(())
}

/// Provisions the agent-side bundled helper scripts into an agent worktree's
/// `.git-paw/scripts/` directory, from the same embedded assets [`run_init`]
/// uses. Called at per-worktree setup (`git paw start` / `git paw add`) so the
/// agent finds the helpers at the relative path its boot block invokes —
/// without hand-copying them from `assets/`.
///
/// A fresh worktree checkout has no `.git-paw/scripts/` (it is gitignored, so
/// not part of the checked-out tree), so an agent's relative
/// `.git-paw/scripts/broker.sh` invocation would otherwise fail until the
/// agent copied it by hand. Sourcing the content from the embedded assets (the
/// same ones `git paw init` installs) guarantees a worktree's helper matches
/// the running binary's version rather than the repo's possibly-stale copy.
///
/// Idempotent — always (re)writes the scripts and re-sets the executable bit,
/// so re-attaching a reused worktree refreshes them without error. `broker.sh`
/// is provisioned when `broker_enabled`; `docs-fetch.sh` when
/// `docs_base_url_configured` (mirroring the docs-fetch skill's injection gate).
pub fn provision_worktree_helpers(
    worktree_root: &Path,
    broker_enabled: bool,
    docs_base_url_configured: bool,
) -> Result<(), PawError> {
    let scripts_dir = worktree_root.join(".git-paw").join("scripts");
    create_dir_if_missing(&scripts_dir)?;
    if broker_enabled {
        install_script(&scripts_dir.join("broker.sh"), BROKER_SCRIPT)?;
    }
    if docs_base_url_configured {
        install_script(&scripts_dir.join("docs-fetch.sh"), DOCS_FETCH_SCRIPT)?;
    }
    Ok(())
}

/// Writes a bundled helper script `content` to `path` and marks it
/// executable (mode `0o755` on Unix). Overwrites any existing file at `path`
/// (the scripts are treated as binary-managed content — users with local
/// edits SHALL back the file up before re-running `git paw init`). Shared by
/// the `sweep.sh` and `broker.sh` installers.
fn install_script(path: &Path, content: &str) -> Result<(), PawError> {
    fs::write(path, content)
        .map_err(|e| PawError::InitError(format!("failed to write '{}': {e}", path.display())))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)
            .map_err(|e| PawError::InitError(format!("failed to stat '{}': {e}", path.display())))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).map_err(|e| {
            PawError::InitError(format!(
                "failed to set executable bit on '{}': {e}",
                path.display()
            ))
        })?;
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
///
/// If `specs_section` is `Some`, it is appended to the generated config — the
/// user's init-time spec-system choice, or a commented `[specs]` template when
/// init runs non-interactively. The config is the source of truth for the spec
/// system; git-paw never auto-detects it from the filesystem.
fn write_config_if_missing(
    path: &Path,
    supervisor_section: Option<&str>,
    specs_section: Option<&str>,
) -> Result<bool, PawError> {
    if path.exists() {
        return Ok(false);
    }
    let mut content = config::generate_default_config();
    if let Some(section) = supervisor_section {
        content.push_str(section);
    }
    if let Some(section) = specs_section {
        content.push_str(section);
    }
    fs::write(path, content)
        .map_err(|e| PawError::InitError(format!("failed to write config: {e}")))?;
    Ok(true)
}

/// The four spec systems git-paw can scan, and each one's conventional
/// directory. Presented at init and written into `[specs]`.
const SPEC_SYSTEMS: [(&str, &str); 4] = [
    ("openspec", "specs"),
    ("markdown", "specs"),
    ("speckit", ".specify/specs"),
    ("superpowers", "docs/superpowers/plans"),
];

/// Commented `[specs]` template written by a non-interactive init, so the user
/// can uncomment and fill it in. The config — never filesystem detection — is
/// the source of truth for which spec system to use.
const COMMENTED_SPECS_TEMPLATE: &str = "\n# Spec system (required to launch from specs). Uncomment and pick one:\n\
# [specs]\n\
# type = \"openspec\"   # \"openspec\", \"markdown\", \"speckit\", or \"superpowers\"\n\
# dir = \"specs\"       # openspec/markdown: \"specs\"; speckit: \".specify/specs\"; superpowers: \"docs/superpowers/plans\"\n";

/// Prompts the user to choose the project's spec system and returns the
/// matching TOML `[specs]` section. git-paw does NOT auto-detect the spec
/// system from the filesystem — the config is the source of truth, so init
/// records an explicit choice. A non-interactive init (CI, tests, piped stdin)
/// writes a commented template instead so init stays scriptable.
fn prompt_specs_section() -> Result<String, PawError> {
    if !std::io::stdin().is_terminal() {
        return Ok(COMMENTED_SPECS_TEMPLATE.to_string());
    }

    let labels: Vec<&str> = SPEC_SYSTEMS.iter().map(|(name, _)| *name).collect();
    let idx = Select::new()
        .with_prompt("Which spec system does this project use? (recorded in config; git-paw never auto-detects it)")
        .items(&labels)
        .default(0)
        .interact()
        .map_err(|e| PawError::InitError(format!("prompt failed: {e}")))?;
    Ok(specs_section_for(idx))
}

/// Builds the active `[specs]` section for the spec system at `idx` in
/// [`SPEC_SYSTEMS`]. Factored out of [`prompt_specs_section`] so the
/// index-to-section mapping is unit-testable without a TTY; `idx` always
/// comes from `Select`, which is bounded to `0..SPEC_SYSTEMS.len()`.
fn specs_section_for(idx: usize) -> String {
    let (spec_type, dir) = SPEC_SYSTEMS[idx];
    format!("\n[specs]\ntype = \"{spec_type}\"\ndir = \"{dir}\"\n")
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
        return supervisor_section(false, "");
    }

    let enabled = Confirm::new()
        .with_prompt("Enable supervisor mode by default?")
        .default(false)
        .interact()
        .map_err(|e| PawError::InitError(format!("prompt failed: {e}")))?;

    if !enabled {
        return supervisor_section(false, "");
    }

    let test_command: String = Input::new()
        .with_prompt("Test command to run after each agent completes (e.g. 'just check', leave empty to skip)")
        .allow_empty(true)
        .interact_text()
        .map_err(|e| PawError::InitError(format!("prompt failed: {e}")))?;

    supervisor_section(true, &test_command)
}

/// Builds the `[supervisor]` section for the chosen preferences. Factored out
/// of [`prompt_supervisor_section`] so the enabled/test-command formatting
/// (trimming and TOML-escaping the command) is unit-testable without a TTY.
/// An empty (or whitespace-only) `test_command` omits the `test_command` key.
fn supervisor_section(enabled: bool, test_command: &str) -> Result<String, PawError> {
    if !enabled {
        return Ok("\n[supervisor]\nenabled = false\n".to_string());
    }
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
        assert!(write_config_if_missing(&config_path, None, None).unwrap());
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("default_cli"));
    }

    #[test]
    fn skips_existing_config() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, "existing").unwrap();
        assert!(!write_config_if_missing(&config_path, None, None).unwrap());
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "existing");
    }

    #[test]
    fn appends_supervisor_section_when_provided() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        let section = "\n[supervisor]\nenabled = true\ntest_command = \"just check\"\n";
        assert!(write_config_if_missing(&config_path, Some(section), None).unwrap());

        let content = fs::read_to_string(&config_path).unwrap();
        let parsed: crate::config::PawConfig = toml::from_str(&content).unwrap();
        let supervisor = parsed.supervisor.unwrap();
        assert!(supervisor.enabled);
        assert_eq!(supervisor.test_command.as_deref(), Some("just check"));
    }

    #[test]
    fn prompt_specs_section_non_interactive_writes_commented_template() {
        // `cargo test` runs with a non-terminal stdin, so this exercises the
        // non-interactive fallback: a commented [specs] template (no filesystem
        // detection). An interactive `Select` cannot be driven from a test.
        let section = prompt_specs_section().expect("section");
        assert!(
            section.contains("# [specs]"),
            "non-interactive init writes a COMMENTED template; got: {section}"
        );
        for (name, _) in SPEC_SYSTEMS {
            assert!(
                section.contains(name),
                "template lists {name}; got: {section}"
            );
        }
    }

    #[test]
    fn spec_systems_cover_the_four_backends_in_order() {
        let names: Vec<&str> = SPEC_SYSTEMS.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, ["openspec", "markdown", "speckit", "superpowers"]);
    }

    #[test]
    fn specs_section_for_maps_each_index_to_its_backend_section() {
        // The interactive `Select` yields the chosen index; this is the pure
        // index -> `[specs]` section mapping it feeds into, unit-tested without
        // a TTY. The end-to-end keystroke path (Select -> written config) is
        // covered by the `tests/init_interactive_specs.rs` tmux integration
        // test.
        assert_eq!(
            specs_section_for(0),
            "\n[specs]\ntype = \"openspec\"\ndir = \"specs\"\n"
        );
        assert_eq!(
            specs_section_for(1),
            "\n[specs]\ntype = \"markdown\"\ndir = \"specs\"\n"
        );
        assert_eq!(
            specs_section_for(2),
            "\n[specs]\ntype = \"speckit\"\ndir = \".specify/specs\"\n"
        );
        assert_eq!(
            specs_section_for(3),
            "\n[specs]\ntype = \"superpowers\"\ndir = \"docs/superpowers/plans\"\n"
        );
    }

    #[test]
    fn supervisor_section_disabled_when_not_enabled() {
        // Both the non-interactive fallback and a "No" answer route here.
        assert_eq!(
            supervisor_section(false, "").unwrap(),
            "\n[supervisor]\nenabled = false\n"
        );
        // A test command is irrelevant when supervisor is disabled.
        assert_eq!(
            supervisor_section(false, "just check").unwrap(),
            "\n[supervisor]\nenabled = false\n"
        );
    }

    #[test]
    fn supervisor_section_enabled_without_test_command() {
        assert_eq!(
            supervisor_section(true, "").unwrap(),
            "\n[supervisor]\nenabled = true\n"
        );
        // Whitespace-only is treated as empty (no test_command line).
        assert_eq!(
            supervisor_section(true, "   ").unwrap(),
            "\n[supervisor]\nenabled = true\n"
        );
    }

    #[test]
    fn supervisor_section_records_and_escapes_test_command() {
        // Trimmed, and round-trips through TOML with quotes/backslashes intact.
        let section = supervisor_section(true, "  just \"check\"\\x  ").unwrap();
        let parsed: crate::config::PawConfig = toml::from_str(&section).unwrap();
        let supervisor = parsed.supervisor.unwrap();
        assert!(supervisor.enabled);
        assert_eq!(
            supervisor.test_command.as_deref(),
            Some("just \"check\"\\x")
        );
    }

    #[test]
    fn write_config_appends_specs_section_when_provided() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        let specs_section = "\n[specs]\ntype = \"speckit\"\ndir = \".specify/specs\"\n";
        assert!(write_config_if_missing(&config_path, None, Some(specs_section)).unwrap());

        let content = fs::read_to_string(&config_path).unwrap();
        let parsed: crate::config::PawConfig = toml::from_str(&content).unwrap();
        let specs = parsed.specs.expect("specs section parsed");
        assert_eq!(specs.spec_type.as_deref(), Some("speckit"));
        assert_eq!(specs.dir.as_deref(), Some(".specify/specs"));
    }

    #[test]
    fn appends_disabled_supervisor_section() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        let section = "\n[supervisor]\nenabled = false\n";
        assert!(write_config_if_missing(&config_path, Some(section), None).unwrap());

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

    #[test]
    fn session_learnings_added_to_gitignore() {
        // The learnings aggregator writes .git-paw/session-learnings.md as
        // per-session runtime output; it must never be committed (an agent's
        // `git add -A` otherwise sweeps it into a PR).
        let dir = setup_repo();
        fs::write(dir.path().join(".gitignore"), ".git-paw/logs/\n").unwrap();
        assert!(ensure_gitignore_entry(dir.path()).unwrap());
        let content = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(
            content.contains(".git-paw/session-learnings.md"),
            "init must gitignore the per-session .git-paw/session-learnings.md output"
        );
    }

    #[test]
    fn repo_local_tmp_added_to_gitignore_and_not_duplicated() {
        // The repo-local scratch dir must be ignored so verify worktrees /
        // self-test sessions are never committed in the consuming repo.
        let dir = setup_repo();
        // Pre-seed only logs/ — tmp/ must be appended.
        fs::write(dir.path().join(".gitignore"), ".git-paw/logs/\n").unwrap();
        assert!(ensure_gitignore_entry(dir.path()).unwrap());
        let content = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(
            content.contains(".git-paw/tmp/"),
            "init must gitignore the repo-local .git-paw/tmp/ scratch dir"
        );
        // Idempotent: a second pass adds nothing and keeps a single entry.
        assert!(!ensure_gitignore_entry(dir.path()).unwrap());
        let content2 = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(
            content2.matches(".git-paw/tmp/").count(),
            1,
            ".git-paw/tmp/ must appear exactly once after repeated init"
        );
    }

    #[test]
    fn worktrees_dir_added_to_gitignore_and_not_duplicated() {
        // Child-placement worktrees live under .git-paw/worktrees/; that path
        // must be ignored so in-repo worktrees are never staged.
        let dir = setup_repo();
        // Pre-seed only logs/ — worktrees/ must be appended.
        fs::write(dir.path().join(".gitignore"), ".git-paw/logs/\n").unwrap();
        assert!(ensure_gitignore_entry(dir.path()).unwrap());
        let content = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(
            content.contains(".git-paw/worktrees/"),
            "init must gitignore the in-repo .git-paw/worktrees/ dir"
        );
        // Idempotent: a second pass adds nothing and keeps a single entry.
        assert!(!ensure_gitignore_entry(dir.path()).unwrap());
        let content2 = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(
            content2.matches(".git-paw/worktrees/").count(),
            1,
            ".git-paw/worktrees/ must appear exactly once after repeated init"
        );
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

    /// Bug F (v0-5-0-audit-cleanup §9d) — a config with an UNCOMMENTED
    /// `[supervisor]` block must survive migrate without growing a
    /// duplicate header. `has_section` is comment-aware: it only
    /// matches active headers, so the uncommented user block is
    /// detected and no stanza is appended. The file MUST still parse as
    /// valid TOML afterwards (no `duplicate key` error).
    #[test]
    fn migrate_against_uncommented_supervisor_does_not_create_duplicate() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        let initial = r#"# user-authored config
branch_prefix = "feat/"

[supervisor]
enabled = true
cli = "claude-oss"
test_command = "just check"
"#;
        fs::write(&config_path, initial).unwrap();

        let modified = migrate_existing_config(&config_path).unwrap();
        assert!(
            !modified,
            "migrate must be a no-op when an uncommented [supervisor] block already exists"
        );

        let after = fs::read_to_string(&config_path).unwrap();
        let header_count = after.lines().filter(|l| l.trim() == "[supervisor]").count();
        assert_eq!(
            header_count, 1,
            "exactly one [supervisor] header must exist; found {header_count} in:\n{after}"
        );

        // Crucially, the file must parse without a duplicate-key error.
        let parsed: crate::config::PawConfig = toml::from_str(&after).expect(
            "config with uncommented [supervisor] must parse cleanly after migrate (no duplicate key)",
        );
        let supervisor = parsed.supervisor.expect("supervisor present");
        assert!(supervisor.enabled);
        assert_eq!(supervisor.cli.as_deref(), Some("claude-oss"));
        assert_eq!(supervisor.test_command.as_deref(), Some("just check"));
    }

    /// 9d.7 sibling — when a user writes `branch_prefix = "feat/"` only
    /// (no sections), running migrate appends the disabled
    /// `[supervisor]` opt-out and preserves the user's `branch_prefix`.
    /// The file parses as valid TOML.
    ///
    /// NOTE: the wider variant of 9d.7 (also appending commented
    /// stanzas for `[broker]`, `[dashboard]`, etc.) is intentionally
    /// deferred — it is a feature addition (richer migration), not a
    /// bug fix. The current scope of `migrate_existing_config` is
    /// limited to the `[supervisor]` section per the existing tests in
    /// this module.
    #[test]
    fn migrate_against_branch_prefix_only_preserves_user_field() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, "branch_prefix = \"feat/\"\n").unwrap();

        let modified = migrate_existing_config(&config_path).unwrap();
        assert!(
            modified,
            "migrate must append the missing [supervisor] section"
        );

        let after = fs::read_to_string(&config_path).unwrap();
        assert!(
            after.contains("branch_prefix = \"feat/\""),
            "user branch_prefix must be preserved verbatim; got:\n{after}"
        );
        assert!(
            after.contains("[supervisor]"),
            "supervisor section must be appended; got:\n{after}"
        );

        // Most importantly: the result parses as valid TOML.
        let parsed: crate::config::PawConfig = toml::from_str(&after)
            .expect("config with branch_prefix + appended supervisor must parse cleanly");
        assert_eq!(parsed.branch_prefix.as_deref(), Some("feat/"));
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

    // --- provision_worktree_helpers ---

    /// Returns `true` if `path` exists and carries the Unix executable bit.
    /// Non-Unix targets only assert existence (mode bits are Unix-only, matching
    /// [`install_script`]'s `#[cfg(unix)]` gate).
    fn is_executable(path: &Path) -> bool {
        if !path.exists() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(path).unwrap().permissions().mode();
            mode & 0o111 == 0o111
        }
        #[cfg(not(unix))]
        {
            true
        }
    }

    // Scenario: start provisions the broker helper into each worktree — after a
    // worktree setup with the broker enabled, broker.sh exists and is executable.
    #[test]
    fn provision_writes_executable_broker_helper_when_broker_enabled() {
        let dir = TempDir::new().unwrap();
        provision_worktree_helpers(dir.path(), true, false).unwrap();
        let broker = dir.path().join(".git-paw/scripts/broker.sh");
        assert!(
            is_executable(&broker),
            "broker.sh must exist and be executable"
        );
        // Version-matched: content is byte-identical to the embedded asset the
        // running binary ships (the same one `git paw init` installs).
        assert_eq!(fs::read_to_string(&broker).unwrap(), BROKER_SCRIPT);
    }

    // Scenario: docs-fetch helper provisioned only when configured — docs-fetch.sh
    // is provisioned iff `docs_base_url` is configured, alongside broker.sh.
    #[test]
    fn provision_writes_docs_fetch_only_when_configured() {
        // docs_base_url unset → docs-fetch.sh is NOT provisioned.
        let unset = TempDir::new().unwrap();
        provision_worktree_helpers(unset.path(), true, false).unwrap();
        assert!(
            !unset.path().join(".git-paw/scripts/docs-fetch.sh").exists(),
            "docs-fetch.sh must not be provisioned when docs_base_url is unset"
        );

        // docs_base_url configured → docs-fetch.sh provisioned alongside broker.sh.
        let set = TempDir::new().unwrap();
        provision_worktree_helpers(set.path(), true, true).unwrap();
        let docs_fetch = set.path().join(".git-paw/scripts/docs-fetch.sh");
        assert!(
            is_executable(&docs_fetch),
            "docs-fetch.sh must be provisioned and executable when configured"
        );
        assert_eq!(fs::read_to_string(&docs_fetch).unwrap(), DOCS_FETCH_SCRIPT);
        assert!(
            is_executable(&set.path().join(".git-paw/scripts/broker.sh")),
            "broker.sh is provisioned alongside docs-fetch.sh"
        );
    }

    // Broker disabled → broker.sh is not provisioned (mirrors the docs-fetch gate
    // for the broker side of the requirement).
    #[test]
    fn provision_skips_broker_helper_when_broker_disabled() {
        let dir = TempDir::new().unwrap();
        provision_worktree_helpers(dir.path(), false, false).unwrap();
        assert!(
            !dir.path().join(".git-paw/scripts/broker.sh").exists(),
            "broker.sh must not be provisioned when the broker is disabled"
        );
    }

    // Scenario: provisioning is idempotent and version-matched — re-attaching an
    // existing worktree refreshes the scripts without error, and a stale on-disk
    // helper is overwritten with the running binary's bundled version.
    #[test]
    fn provision_is_idempotent_and_refreshes_stale_helper() {
        let dir = TempDir::new().unwrap();
        // First attach seeds the helper.
        provision_worktree_helpers(dir.path(), true, true).unwrap();
        // Simulate a stale worktree copy from an older binary.
        let broker = dir.path().join(".git-paw/scripts/broker.sh");
        fs::write(&broker, "#!/usr/bin/env bash\n# stale\n").unwrap();
        // Re-attach: must succeed (no error on the pre-existing scripts dir) and
        // refresh the helper back to the embedded version.
        provision_worktree_helpers(dir.path(), true, true).unwrap();
        assert!(
            is_executable(&broker),
            "broker.sh stays executable after refresh"
        );
        assert_eq!(
            fs::read_to_string(&broker).unwrap(),
            BROKER_SCRIPT,
            "re-attach refreshes the helper to the running binary's version"
        );
    }
}

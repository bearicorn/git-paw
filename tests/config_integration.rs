//! Configuration file discovery and parsing integration tests.
//!
//! Tests config loading from repo-level `.git-paw/config.toml` files using real
//! temporary directories.

use std::fs;

use tempfile::TempDir;

use std::path::Path;

use git_paw::config::{
    PawConfig, add_custom_cli_to, load_config, load_config_from, remove_custom_cli_from,
    repo_config_path,
};

/// Writes content to the repo config path, creating parent directories as needed.
fn write_repo_config(repo_root: &Path, content: &str) {
    let path = repo_config_path(repo_root);
    fs::create_dir_all(path.parent().unwrap()).expect("create config dir");
    fs::write(&path, content).expect("write config");
}

// ---------------------------------------------------------------------------
// No config files
// ---------------------------------------------------------------------------

#[test]
fn load_config_returns_defaults_when_no_files_exist() {
    let tmp = TempDir::new().expect("create temp dir");
    let config = load_config(tmp.path()).expect("load config");

    assert_eq!(config.default_cli, None);
    assert_eq!(config.mouse, None);
    assert!(config.clis.is_empty());
    assert!(config.presets.is_empty());
}

// ---------------------------------------------------------------------------
// Repo config
// ---------------------------------------------------------------------------

#[test]
fn load_config_reads_repo_config() {
    let tmp = TempDir::new().expect("create temp dir");

    write_repo_config(
        tmp.path(),
        r#"
default_cli = "claude"
mouse = false
"#,
    );

    let config = load_config(tmp.path()).expect("load config");

    assert_eq!(config.default_cli, Some("claude".to_string()));
    assert_eq!(config.mouse, Some(false));
}

#[test]
fn repo_config_with_custom_clis() {
    let tmp = TempDir::new().expect("create temp dir");

    write_repo_config(
        tmp.path(),
        r#"
[clis.my-agent]
command = "/usr/local/bin/my-agent"
display_name = "My Agent"

[clis.local-llm]
command = "/usr/bin/ollama-code"
"#,
    );

    let config = load_config(tmp.path()).expect("load config");

    assert_eq!(config.clis.len(), 2);

    let my_agent = config.clis.get("my-agent").expect("my-agent should exist");
    assert_eq!(my_agent.command, "/usr/local/bin/my-agent");
    assert_eq!(my_agent.display_name, Some("My Agent".to_string()));

    let local_llm = config
        .clis
        .get("local-llm")
        .expect("local-llm should exist");
    assert_eq!(local_llm.command, "/usr/bin/ollama-code");
    assert_eq!(local_llm.display_name, None);
}

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

#[test]
fn repo_config_with_presets() {
    let tmp = TempDir::new().expect("create temp dir");

    write_repo_config(
        tmp.path(),
        r#"
[presets.backend]
branches = ["feature/api", "fix/db"]
cli = "claude"

[presets.frontend]
branches = ["feature/ui"]
cli = "gemini"
"#,
    );

    let config = load_config(tmp.path()).expect("load config");

    assert_eq!(config.presets.len(), 2);

    let backend = config.get_preset("backend").expect("backend preset");
    assert_eq!(backend.branches, vec!["feature/api", "fix/db"]);
    assert_eq!(backend.cli, "claude");

    let frontend = config.get_preset("frontend").expect("frontend preset");
    assert_eq!(frontend.branches, vec!["feature/ui"]);
    assert_eq!(frontend.cli, "gemini");
}

#[test]
fn get_preset_returns_none_for_unknown() {
    let config = PawConfig::default();
    assert!(config.get_preset("nonexistent").is_none());
}

// ---------------------------------------------------------------------------
// Config merging behavior
// ---------------------------------------------------------------------------

#[test]
fn repo_config_overrides_default_fields() {
    let tmp = TempDir::new().expect("create temp dir");

    write_repo_config(
        tmp.path(),
        r#"
default_cli = "gemini"
mouse = true
"#,
    );

    let config = load_config(tmp.path()).expect("load config");

    // These should come from the repo config
    assert_eq!(config.default_cli, Some("gemini".to_string()));
    assert_eq!(config.mouse, Some(true));
}

// ---------------------------------------------------------------------------
// Config path
// ---------------------------------------------------------------------------

#[test]
fn repo_config_path_is_in_repo_root() {
    let tmp = TempDir::new().expect("create temp dir");
    let path = repo_config_path(tmp.path());
    assert_eq!(path, tmp.path().join(".git-paw").join("config.toml"));
}

// ---------------------------------------------------------------------------
// Malformed config
// ---------------------------------------------------------------------------

#[test]
fn malformed_toml_returns_error() {
    let tmp = TempDir::new().expect("create temp dir");

    write_repo_config(tmp.path(), "this is not valid { toml [[[");

    let result = load_config(tmp.path());
    assert!(result.is_err(), "malformed TOML should produce an error");
}

// ---------------------------------------------------------------------------
// All fields optional
// ---------------------------------------------------------------------------

#[test]
fn empty_config_file_is_valid() {
    let tmp = TempDir::new().expect("create temp dir");

    write_repo_config(tmp.path(), "");

    let config = load_config(tmp.path()).expect("load config");
    assert_eq!(config, PawConfig::default());
}

// ---------------------------------------------------------------------------
// Custom CLI management (add / remove)
// ---------------------------------------------------------------------------

#[test]
fn add_custom_cli_with_absolute_path() {
    let tmp = TempDir::new().expect("create temp dir");
    let config_path = tmp.path().join("config.toml");

    add_custom_cli_to(&config_path, "my-agent", "/usr/local/bin/my-agent", None).expect("add CLI");

    let config = load_config_from(&config_path, tmp.path()).expect("load");
    let cli = config.clis.get("my-agent").expect("CLI should exist");
    assert_eq!(cli.command, "/usr/local/bin/my-agent");
    assert_eq!(cli.display_name, None);
}

#[test]
fn add_custom_cli_with_display_name() {
    let tmp = TempDir::new().expect("create temp dir");
    let config_path = tmp.path().join("config.toml");

    add_custom_cli_to(
        &config_path,
        "my-agent",
        "/usr/local/bin/my-agent",
        Some("My Custom Agent"),
    )
    .expect("add CLI");

    let config = load_config_from(&config_path, tmp.path()).expect("load");
    let cli = config.clis.get("my-agent").expect("CLI should exist");
    assert_eq!(cli.display_name, Some("My Custom Agent".to_string()));
}

#[test]
fn add_multiple_custom_clis_preserves_all() {
    let tmp = TempDir::new().expect("create temp dir");
    let config_path = tmp.path().join("config.toml");

    add_custom_cli_to(&config_path, "agent-a", "/bin/agent-a", Some("Agent A")).expect("add first");
    add_custom_cli_to(&config_path, "agent-b", "/bin/agent-b", Some("Agent B"))
        .expect("add second");
    add_custom_cli_to(&config_path, "agent-c", "/bin/agent-c", Some("Agent C")).expect("add third");
    add_custom_cli_to(&config_path, "agent-d", "/bin/agent-d", None).expect("add fourth");

    let config = load_config_from(&config_path, tmp.path()).expect("load");
    assert_eq!(config.clis.len(), 4);
    assert!(config.clis.contains_key("agent-a"));
    assert!(config.clis.contains_key("agent-b"));
    assert!(config.clis.contains_key("agent-c"));
    assert!(config.clis.contains_key("agent-d"));

    assert_eq!(
        config.clis.get("agent-a").unwrap().display_name,
        Some("Agent A".to_string())
    );
    assert_eq!(config.clis.get("agent-d").unwrap().display_name, None);
}

#[test]
fn add_cli_overwrites_existing_entry() {
    let tmp = TempDir::new().expect("create temp dir");
    let config_path = tmp.path().join("config.toml");

    add_custom_cli_to(&config_path, "my-agent", "/bin/old-path", Some("Old Name"))
        .expect("add first");
    add_custom_cli_to(&config_path, "my-agent", "/bin/new-path", Some("New Name"))
        .expect("add second");

    let config = load_config_from(&config_path, tmp.path()).expect("load");
    assert_eq!(config.clis.len(), 1);
    let cli = config.clis.get("my-agent").unwrap();
    assert_eq!(cli.command, "/bin/new-path");
    assert_eq!(cli.display_name, Some("New Name".to_string()));
}

#[test]
fn add_cli_with_nonexistent_path_command_fails() {
    let tmp = TempDir::new().expect("create temp dir");
    let config_path = tmp.path().join("config.toml");

    // A non-absolute command that isn't on PATH should fail
    let result = add_custom_cli_to(
        &config_path,
        "bad-agent",
        "definitely-not-on-path-xyz",
        None,
    );
    assert!(result.is_err(), "should fail for command not on PATH");
}

#[test]
fn remove_custom_cli() {
    let tmp = TempDir::new().expect("create temp dir");
    let config_path = tmp.path().join("config.toml");

    add_custom_cli_to(&config_path, "agent-a", "/bin/a", None).expect("add a");
    add_custom_cli_to(&config_path, "agent-b", "/bin/b", None).expect("add b");

    remove_custom_cli_from(&config_path, "agent-a").expect("remove a");

    let config = load_config_from(&config_path, tmp.path()).expect("load");
    assert_eq!(config.clis.len(), 1);
    assert!(!config.clis.contains_key("agent-a"));
    assert!(config.clis.contains_key("agent-b"));
}

#[test]
fn remove_nonexistent_cli_returns_error() {
    let tmp = TempDir::new().expect("create temp dir");
    let config_path = tmp.path().join("config.toml");

    let result = remove_custom_cli_from(&config_path, "nonexistent");
    assert!(result.is_err());
}

#[test]
fn remove_all_custom_clis_leaves_empty_config() {
    let tmp = TempDir::new().expect("create temp dir");
    let config_path = tmp.path().join("config.toml");

    add_custom_cli_to(&config_path, "agent-a", "/bin/a", None).expect("add");
    remove_custom_cli_from(&config_path, "agent-a").expect("remove");

    let config = load_config_from(&config_path, tmp.path()).expect("load");
    assert!(config.clis.is_empty());
}

// ---------------------------------------------------------------------------
// Global + repo config merging with custom CLIs
// ---------------------------------------------------------------------------

#[test]
fn repo_custom_clis_merge_with_global_custom_clis() {
    let tmp = TempDir::new().expect("create temp dir");
    let global_path = tmp.path().join("global.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).expect("create repo dir");

    // Global config has two CLIs
    fs::write(
        &global_path,
        r#"
[clis.global-agent]
command = "/bin/global-agent"
display_name = "Global Agent"

[clis.shared-agent]
command = "/bin/global-shared"
"#,
    )
    .expect("write global");

    // Repo config has one CLI that overlaps and one new
    write_repo_config(
        &repo_root,
        r#"
[clis.shared-agent]
command = "/bin/repo-shared"
display_name = "Repo Shared"

[clis.repo-agent]
command = "/bin/repo-agent"
"#,
    );

    let config = load_config_from(&global_path, &repo_root).expect("load merged");

    // Should have 3 CLIs total: global-agent, shared-agent (repo wins), repo-agent
    assert_eq!(config.clis.len(), 3);

    // Global-only CLI preserved
    assert_eq!(
        config.clis.get("global-agent").unwrap().command,
        "/bin/global-agent"
    );

    // Repo CLI overrides global on collision
    let shared = config.clis.get("shared-agent").unwrap();
    assert_eq!(shared.command, "/bin/repo-shared");
    assert_eq!(shared.display_name, Some("Repo Shared".to_string()));

    // Repo-only CLI included
    assert!(config.clis.contains_key("repo-agent"));
}

// ---------------------------------------------------------------------------
// Config with many custom CLIs (stress test)
// ---------------------------------------------------------------------------

#[test]
fn config_with_many_custom_clis() {
    let tmp = TempDir::new().expect("create temp dir");

    let mut toml = String::new();
    for i in 0..10 {
        use std::fmt::Write;
        write!(
            toml,
            r#"
[clis.agent-{i}]
command = "/bin/agent-{i}"
display_name = "Agent {i}"
"#
        )
        .unwrap();
    }

    write_repo_config(tmp.path(), &toml);

    let config = load_config(tmp.path()).expect("load config");
    assert_eq!(config.clis.len(), 10);

    for i in 0..10 {
        let name = format!("agent-{i}");
        let cli = config
            .clis
            .get(&name)
            .unwrap_or_else(|| panic!("{name} missing"));
        assert_eq!(cli.command, format!("/bin/agent-{i}"));
        assert_eq!(cli.display_name, Some(format!("Agent {i}")));
    }
}

// Supervisor mode config migration tests
//
// The earlier `test_migrate_*` tests in this file did `load_config` +
// `save_repo_config` and asserted on the round-trip; they never invoked
// `migrate_existing_config`, so they only verified TOML round-tripping
// (already covered by `config::tests`). Behavioral coverage of the actual
// migration code path now lives in `src/init.rs::tests`:
//
//   * `migrate_preserves_existing_supervisor_and_custom_broker_port`
//   * `migrate_appends_supervisor_section_when_missing_and_keeps_broker_port`
//   * `migrate_existing_config_is_idempotent`
//
// Those tests call `migrate_existing_config` directly against a temp config
// and assert on the resulting file contents and parsed `PawConfig`.

use super::*;
use tempfile::TempDir;

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
#[allow(clippy::too_many_lines)] // one row per folded backward-compat fixture
fn absent_optional_config_resolves_to_defaults() {
    // Backward-compat contract: a config that omits an optional section or
    // field loads without error and the missing piece resolves to its
    // documented default / None. One row per (formerly per-era) fixture;
    // each closure asserts exactly what its original fixture asserted. A
    // new optional section adds a row, not a fixture.
    type Check = fn(&PawConfig);
    let cases: &[(&str, &str, Check)] = &[
        (
            "no [supervisor] section (with [broker] + [logging]) → supervisor None; default_cli + broker.enabled parse",
            "default_cli = \"claude\"\n\
                 mouse = true\n\
                 [broker]\n\
                 enabled = true\n\
                 [logging]\n\
                 enabled = false\n",
            |c| {
                assert_eq!(c.default_cli.as_deref(), Some("claude"));
                assert!(c.broker.enabled);
                assert!(c.supervisor.is_none());
            },
        ),
        (
            "no [supervisor] / [supervisor.auto_approve] (with [broker]) → supervisor None; broker.enabled",
            "default_cli = \"claude\"\nmouse = true\n[broker]\nenabled = true\n",
            |c| {
                assert!(c.supervisor.is_none());
                assert!(c.broker.enabled);
            },
        ),
        (
            "[supervisor] without approval key → supervisor.approval None",
            "[supervisor]\n\
                 enabled = true\n\
                 cli = \"claude\"\n\
                 test_command = \"just check\"\n\
                 agent_approval = \"full-auto\"\n",
            |c| {
                let supervisor = c.supervisor.as_ref().unwrap();
                assert_eq!(supervisor.approval, None);
            },
        ),
        (
            "[supervisor] without learnings field → learnings false; learnings_config.flush_interval_seconds default 60",
            "default_cli = \"claude\"\n\
                 [supervisor]\n\
                 enabled = true\n\
                 agent_approval = \"auto\"\n",
            |c| {
                let supervisor = c.supervisor.as_ref().unwrap();
                assert!(!supervisor.learnings);
                assert_eq!(supervisor.learnings_config.flush_interval_seconds, 60);
            },
        ),
        (
            "[supervisor] without [supervisor.common_dev_allowlist] → allowlist enabled default true, extra empty",
            "[supervisor]\n\
                 enabled = true\n\
                 cli = \"claude\"\n\
                 test_command = \"just check\"\n\
                 agent_approval = \"auto\"\n\
                 [supervisor.conflict]\n\
                 window_seconds = 60\n",
            |c| {
                let supervisor = c.supervisor.as_ref().unwrap();
                assert!(supervisor.common_dev_allowlist.enabled);
                assert!(supervisor.common_dev_allowlist.extra.is_empty());
            },
        ),
        (
            "no [governance] section → governance == default; all path fields None",
            "default_cli = \"claude\"\n\
                 mouse = true\n\
                 [broker]\n\
                 enabled = true\n\
                 [supervisor]\n\
                 enabled = true\n\
                 [specs]\n\
                 dir = \"specs\"\n\
                 type = \"openspec\"\n\
                 [clis.foo]\n\
                 command = \"/bin/foo\"\n",
            |c| {
                assert_eq!(c.governance, GovernanceConfig::default());
                assert!(c.governance.adr.is_none());
                assert!(c.governance.test_strategy.is_none());
                assert!(c.governance.security.is_none());
                assert!(c.governance.dod.is_none());
                assert!(c.governance.constitution.is_none());
                assert!(c.governance.readme.is_none());
                assert!(c.governance.docs.is_none());
            },
        ),
        (
            "[dashboard] without broker_log table → show_message_log true, broker_log == default",
            "[dashboard]\nshow_message_log = true\n",
            |c| {
                let dashboard = c.dashboard.as_ref().unwrap();
                assert!(dashboard.show_message_log);
                assert_eq!(dashboard.broker_log, BrokerLogConfig::default());
            },
        ),
        (
            "no [layout] section → layout None; border affordances enabled",
            "default_cli = \"claude\"\nmouse = true\n\n[broker]\nenabled = true\nport = 9119\n\n[supervisor]\nenabled = true\n",
            |c| {
                assert!(c.layout.is_none());
                assert!(c.border_affordances_enabled());
            },
        ),
        (
            "no worktree_placement field → resolves to Sibling",
            "default_cli = \"claude\"\nmouse = true\n[broker]\nenabled = true\n",
            |c| {
                assert_eq!(c.worktree_placement(), WorktreePlacement::Sibling);
            },
        ),
    ];

    for (label, toml, check) in cases {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        write_file(&path, toml);
        let config = load_config_file(&path)
            .unwrap_or_else(|e| panic!("{label}: load failed: {e}"))
            .unwrap_or_else(|| panic!("{label}: config was None"));
        check(&config);
    }
}

// --- Parsing behavior ---

#[test]
fn parses_config_with_all_fields() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        r#"
default_cli = "claude"
mouse = false
default_spec_cli = "gemini"
branch_prefix = "spec/"

[clis.my-agent]
command = "/usr/local/bin/my-agent"
display_name = "My Agent"

[clis.local-llm]
command = "ollama-code"

[presets.backend]
branches = ["feature/api", "fix/db"]
cli = "claude"

[specs]
dir = "my-specs"
type = "openspec"

[logging]
enabled = true
"#,
    );

    let config = load_config_file(&path).unwrap().unwrap();
    assert_eq!(config.default_cli.as_deref(), Some("claude"));
    assert_eq!(config.mouse, Some(false));
    assert_eq!(config.default_spec_cli.as_deref(), Some("gemini"));
    assert_eq!(config.branch_prefix.as_deref(), Some("spec/"));
    assert_eq!(config.clis.len(), 2);
    assert_eq!(
        config.clis["my-agent"].display_name.as_deref(),
        Some("My Agent")
    );
    assert_eq!(config.clis["local-llm"].command, "ollama-code");
    assert_eq!(config.presets["backend"].cli, "claude");
    assert_eq!(
        config.presets["backend"].branches,
        vec!["feature/api", "fix/db"]
    );
    let specs = config.specs.unwrap();
    assert_eq!(specs.dir.as_deref(), Some("my-specs"));
    assert_eq!(specs.spec_type.as_deref(), Some("openspec"));
    let logging = config.logging.unwrap();
    assert!(logging.enabled);
}

#[test]
fn all_fields_are_optional() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "default_cli = \"gemini\"\n");

    let config = load_config_file(&path).unwrap().unwrap();
    assert_eq!(config.default_cli.as_deref(), Some("gemini"));
    assert_eq!(config.mouse, None);
    assert!(config.clis.is_empty());
    assert!(config.presets.is_empty());
}

#[test]
fn returns_defaults_when_no_files_exist() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("nonexistent").join("config.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    let config = load_config_from(&global_path, &repo_root).unwrap();
    assert_eq!(config.default_cli, None);
    assert_eq!(config.mouse, None);
    assert!(config.clis.is_empty());
    assert!(config.presets.is_empty());
}

#[test]
fn reports_error_for_invalid_toml() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("bad.toml");
    write_file(&path, "this is not [valid toml");

    let err = load_config_file(&path).unwrap_err();
    assert!(err.to_string().contains("bad.toml"));
}

// --- Merge behavior (through file I/O) ---

#[test]
fn repo_config_overrides_global_scalars() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("global").join("config.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    write_file(&global_path, "default_cli = \"claude\"\nmouse = true\n");
    write_file(
        &repo_config_path(&repo_root),
        "default_cli = \"gemini\"\n", // mouse intentionally absent
    );

    let config = load_config_from(&global_path, &repo_root).unwrap();
    assert_eq!(config.default_cli.as_deref(), Some("gemini")); // repo wins
    assert_eq!(config.mouse, Some(true)); // global preserved when repo absent
}

#[test]
fn repo_config_merges_cli_maps() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("global").join("config.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    write_file(&global_path, "[clis.agent-a]\ncommand = \"/bin/a\"\n");
    write_file(
        &repo_config_path(&repo_root),
        "[clis.agent-b]\ncommand = \"/bin/b\"\n",
    );

    let config = load_config_from(&global_path, &repo_root).unwrap();
    assert_eq!(config.clis.len(), 2);
    assert!(config.clis.contains_key("agent-a"));
    assert!(config.clis.contains_key("agent-b"));
}

#[test]
fn repo_cli_overrides_global_cli_with_same_name() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("global").join("config.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    write_file(&global_path, "[clis.my-agent]\ncommand = \"/old/path\"\n");
    write_file(
        &repo_config_path(&repo_root),
        "[clis.my-agent]\ncommand = \"/new/path\"\ndisplay_name = \"Overridden\"\n",
    );

    let config = load_config_from(&global_path, &repo_root).unwrap();
    assert_eq!(config.clis["my-agent"].command, "/new/path");
    assert_eq!(
        config.clis["my-agent"].display_name.as_deref(),
        Some("Overridden")
    );
}

#[test]
fn load_config_from_reads_global_file_when_no_repo() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("global").join("config.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    write_file(&global_path, "default_cli = \"claude\"\nmouse = false\n");
    // No .git-paw/config.toml in repo_root

    let config = load_config_from(&global_path, &repo_root).unwrap();
    assert_eq!(config.default_cli.as_deref(), Some("claude"));
    assert_eq!(config.mouse, Some(false));
}

#[test]
fn load_config_from_reads_repo_file_when_no_global() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("nonexistent").join("config.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    write_file(&repo_config_path(&repo_root), "default_cli = \"codex\"\n");

    let config = load_config_from(&global_path, &repo_root).unwrap();
    assert_eq!(config.default_cli.as_deref(), Some("codex"));
}

// --- Preset behavior ---

#[test]
fn preset_accessible_by_name() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("global").join("config.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    write_file(
        &repo_config_path(&repo_root),
        "[presets.backend]\nbranches = [\"feat/api\", \"fix/db\"]\ncli = \"claude\"\n",
    );

    let config = load_config_from(&global_path, &repo_root).unwrap();
    let preset = config.get_preset("backend").unwrap();
    assert_eq!(preset.cli, "claude");
    assert_eq!(preset.branches, vec!["feat/api", "fix/db"]);
}

#[test]
fn preset_returns_none_when_not_in_config() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("config.toml");
    write_file(&global_path, "default_cli = \"claude\"\n");

    let config = load_config_file(&global_path).unwrap().unwrap();
    assert!(config.get_preset("nonexistent").is_none());
}

// --- add_custom_cli behavior ---

#[test]
fn add_cli_writes_to_config_file() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("git-paw").join("config.toml");

    // Add a CLI with an absolute path (no PATH resolution needed)
    add_custom_cli_to(
        &config_path,
        "my-agent",
        "/usr/local/bin/my-agent",
        Some("My Agent"),
    )
    .unwrap();

    // Verify by loading the file back
    let config = load_config_file(&config_path).unwrap().unwrap();
    assert_eq!(config.clis.len(), 1);
    assert_eq!(config.clis["my-agent"].command, "/usr/local/bin/my-agent");
    assert_eq!(
        config.clis["my-agent"].display_name.as_deref(),
        Some("My Agent")
    );
}

#[test]
fn add_cli_preserves_existing_entries() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("git-paw").join("config.toml");

    add_custom_cli_to(&config_path, "first", "/bin/first", None).unwrap();
    add_custom_cli_to(&config_path, "second", "/bin/second", None).unwrap();

    let config = load_config_file(&config_path).unwrap().unwrap();
    assert_eq!(config.clis.len(), 2);
    assert!(config.clis.contains_key("first"));
    assert!(config.clis.contains_key("second"));
}

#[test]
fn add_cli_errors_when_command_not_on_path() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");

    let err =
        add_custom_cli_to(&config_path, "bad", "surely-nonexistent-binary-xyz", None).unwrap_err();
    assert!(err.to_string().contains("not found on PATH"));
}

// --- remove_custom_cli behavior ---

#[test]
fn remove_cli_deletes_entry_from_config_file() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("git-paw").join("config.toml");

    // Set up: add two CLIs
    add_custom_cli_to(&config_path, "keep-me", "/bin/keep", None).unwrap();
    add_custom_cli_to(&config_path, "remove-me", "/bin/remove", None).unwrap();

    // Act: remove one
    remove_custom_cli_from(&config_path, "remove-me").unwrap();

    // Verify: only the kept CLI remains
    let config = load_config_file(&config_path).unwrap().unwrap();
    assert_eq!(config.clis.len(), 1);
    assert!(config.clis.contains_key("keep-me"));
    assert!(!config.clis.contains_key("remove-me"));
}

#[test]
fn remove_nonexistent_cli_returns_cli_not_found_error() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");
    // Empty config file
    write_file(&config_path, "");

    let err = remove_custom_cli_from(&config_path, "nonexistent").unwrap_err();
    match err {
        PawError::CliNotFound(name) => assert_eq!(name, "nonexistent"),
        other => panic!("expected CliNotFound, got: {other}"),
    }
}

#[test]
fn remove_cli_from_empty_config_returns_error() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");
    // No file at all

    let err = remove_custom_cli_from(&config_path, "ghost").unwrap_err();
    match err {
        PawError::CliNotFound(name) => assert_eq!(name, "ghost"),
        other => panic!("expected CliNotFound, got: {other}"),
    }
}

// --- Round-trip: config survives write + read ---

// --- default_spec_cli behavior ---

#[test]
fn parses_default_spec_cli_when_present() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "default_spec_cli = \"claude\"\n");

    let config = load_config_file(&path).unwrap().unwrap();
    assert_eq!(config.default_spec_cli.as_deref(), Some("claude"));
}

#[test]
fn default_spec_cli_defaults_to_none() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "default_cli = \"claude\"\n");

    let config = load_config_file(&path).unwrap().unwrap();
    assert_eq!(config.default_spec_cli, None);
}

#[test]
fn repo_overrides_global_default_spec_cli() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("global").join("config.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    write_file(&global_path, "default_spec_cli = \"claude\"\n");
    write_file(
        &repo_config_path(&repo_root),
        "default_spec_cli = \"gemini\"\n",
    );

    let config = load_config_from(&global_path, &repo_root).unwrap();
    assert_eq!(config.default_spec_cli.as_deref(), Some("gemini"));
}

#[test]
fn global_default_spec_cli_preserved_when_repo_absent() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("global").join("config.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    write_file(&global_path, "default_spec_cli = \"claude\"\n");

    let config = load_config_from(&global_path, &repo_root).unwrap();
    assert_eq!(config.default_spec_cli.as_deref(), Some("claude"));
}

// --- Round-trip: config survives write + read ---

#[test]
fn config_survives_save_and_load() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");

    let original = PawConfig {
        default_cli: Some("claude".into()),
        default_spec_cli: None,
        branch_prefix: None,
        mouse: Some(true),
        clis: HashMap::from([(
            "test".into(),
            CustomCli {
                command: "/bin/test".into(),
                display_name: Some("Test CLI".into()),
                submit_delay_ms: None,
                settings_path: None,
                approval_args: HashMap::new(),
            },
        )]),
        presets: HashMap::from([(
            "dev".into(),
            Preset {
                branches: vec!["main".into()],
                cli: "claude".into(),
            },
        )]),
        specs: None,
        logging: None,
        dashboard: None,
        broker: BrokerConfig::default(),
        supervisor: None,
        governance: GovernanceConfig::default(),
        layout: None,
        opsx: None,
        mcp: McpConfig::default(),
        worktree_placement: Some(WorktreePlacement::Child),
        docs_base_url: Some("https://docs.example.test".into()),
    };

    save_config_to(&config_path, &original).unwrap();
    let loaded = load_config_file(&config_path).unwrap().unwrap();
    assert_eq!(original, loaded);
}

// --- CustomCli approval_args (supervisor-native-auto-mode) ---

#[test]
fn custom_cli_approval_args_parses_and_round_trips() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[clis.mycli]\n\
             command = \"mycli\"\n\
             approval_args = { \"full-auto\" = \"--yolo\" }\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    let cli = config.clis.get("mycli").expect("mycli entry");
    assert_eq!(
        cli.approval_args.get("full-auto").map(String::as_str),
        Some("--yolo")
    );

    let round_trip_path = tmp.path().join("round-trip.toml");
    save_config_to(&round_trip_path, &config).unwrap();
    let reloaded = load_config_file(&round_trip_path).unwrap().unwrap();
    assert_eq!(reloaded.clis, config.clis);
}

#[test]
fn custom_cli_without_approval_args_parses_unchanged() {
    // A pre-v0.11.0 [clis.<name>] entry — no approval_args key — must
    // load without error and leave the map empty.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[clis.mycli]\ncommand = \"mycli\"\n");

    let config = load_config_file(&path).unwrap().unwrap();
    let cli = config.clis.get("mycli").expect("mycli entry");
    assert!(cli.approval_args.is_empty());
}

#[test]
fn custom_cli_empty_approval_args_omitted_on_serialize() {
    let cli = CustomCli {
        command: "mycli".into(),
        display_name: None,
        submit_delay_ms: None,
        settings_path: None,
        approval_args: HashMap::new(),
    };
    let serialized = toml::to_string_pretty(&cli).unwrap();
    assert!(
        !serialized.contains("approval_args"),
        "empty approval_args must not serialize, got:\n{serialized}"
    );
}

#[test]
fn custom_cli_invalid_approval_args_key_rejected() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[clis.mycli]\n\
             command = \"mycli\"\n\
             approval_args = { \"yolo-mode\" = \"--x\" }\n",
    );

    let err = load_config_file(&path).unwrap_err();
    assert!(
        err.to_string().contains("yolo-mode"),
        "error should name the invalid key, got: {err}"
    );
}

// --- Gap #1: Parse [specs] section with populated fields ---

#[test]
fn parses_specs_section_with_populated_fields() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[specs]\ndir = \"my-specs\"\ntype = \"openspec\"\n");

    let config = load_config_file(&path).unwrap().unwrap();
    let specs = config.specs.unwrap();
    assert_eq!(specs.dir.as_deref(), Some("my-specs"));
    assert_eq!(specs.spec_type.as_deref(), Some("openspec"));
}

// --- Gap #2: Parse [logging] section with enabled ---

#[test]
fn parses_logging_section_with_enabled() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[logging]\nenabled = true\n");

    let config = load_config_file(&path).unwrap().unwrap();
    let logging = config.logging.unwrap();
    assert!(logging.enabled);
}

// --- Gap #3: Round-trip with specs and logging populated ---

#[test]
fn round_trip_with_specs_and_logging() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");

    let original = PawConfig {
        specs: Some(SpecsConfig {
            dir: Some("specs".into()),
            spec_type: Some("openspec".into()),
        }),
        logging: Some(LoggingConfig { enabled: true }),
        ..Default::default()
    };

    save_config_to(&config_path, &original).unwrap();
    let loaded = load_config_file(&config_path).unwrap().unwrap();
    assert_eq!(original, loaded);
    assert_eq!(loaded.specs.unwrap().dir.as_deref(), Some("specs"));
    assert!(loaded.logging.unwrap().enabled);
}

// --- Gap #4: Generated config is valid TOML ---

#[test]
fn generated_default_config_is_valid_toml() {
    let raw = generate_default_config();
    let stripped: String = raw
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<&str>>()
        .join("\n");

    let parsed: Result<PawConfig, _> = toml::from_str(&stripped);
    assert!(
        parsed.is_ok(),
        "generated config with comments stripped should be valid TOML, got: {:?}",
        parsed.unwrap_err()
    );
}

// --- Gap #5: branch_prefix merge ---

#[test]
fn branch_prefix_repo_overrides_global() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("global").join("config.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    write_file(&global_path, "branch_prefix = \"feat/\"\n");
    write_file(&repo_config_path(&repo_root), "branch_prefix = \"spec/\"\n");

    let config = load_config_from(&global_path, &repo_root).unwrap();
    assert_eq!(config.branch_prefix.as_deref(), Some("spec/"));
}

#[test]
fn generated_default_config_contains_commented_examples() {
    let output = generate_default_config();
    assert!(
        output.contains("default_spec_cli"),
        "should contain default_spec_cli"
    );
    assert!(
        output.contains("branch_prefix"),
        "should contain branch_prefix"
    );
    assert!(output.contains("[specs]"), "should contain [specs]");
    assert!(output.contains("[logging]"), "should contain [logging]");
    assert!(output.contains("[broker]"), "should contain [broker]");
}

#[test]
fn generated_default_config_contains_child_worktree_placement() {
    let output = generate_default_config();
    assert!(
        output.contains("worktree_placement = \"child\""),
        "generated config must set child worktree placement for new repos"
    );
    // The line must be active (not commented) so it actually takes effect.
    let parsed: PawConfig = toml::from_str(&output).expect("generated config parses");
    assert_eq!(
        parsed.worktree_placement(),
        WorktreePlacement::Child,
        "generated config must resolve to child placement"
    );
}

// --- Template completeness (init-config-template-completeness) ---

/// Normalize a TOML table path to its section "family": parameterized
/// tables (`clis.<name>`, `presets.<name>`) collapse to their prefix,
/// while every other path (including sub-tables like `broker.watcher`) is
/// kept verbatim.
fn section_family(path: &str) -> String {
    for family in ["clis", "presets"] {
        if path == family || path.starts_with(&format!("{family}.")) {
            return family.to_string();
        }
    }
    path.to_string()
}

/// Return the TOML table path on a line when the line is a table header,
/// tolerating an optional leading `#` so commented template stanzas and
/// bare TOML examples are both recognized. Non-header lines return `None`.
fn table_header(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let body = trimmed.strip_prefix('#').map_or(trimmed, str::trim_start);
    let inner = body.strip_prefix('[')?.strip_suffix(']')?;
    if inner.starts_with('[')
        || inner.is_empty()
        || inner.contains(' ')
        || inner.contains('=')
        || inner.contains('"')
    {
        return None;
    }
    Some(inner.to_string())
}

/// Section families that appear (commented) in the generated init template.
fn template_section_families() -> std::collections::BTreeSet<String> {
    generate_default_config()
        .lines()
        .filter_map(table_header)
        .map(|path| section_family(&path))
        .collect()
}

#[test]
fn generated_template_documents_the_six_added_sections() {
    let template = generate_default_config();
    for header in [
        "# [mcp]",
        "# [layout]",
        "# [broker.watcher]",
        "# [supervisor.auto_approve]",
        "# [supervisor.learnings_config]",
        "# [governance]",
    ] {
        assert!(
            template.contains(header),
            "generated template should contain a commented stanza for {header}",
        );
    }
}

#[test]
fn generated_template_covers_every_documented_config_section() {
    // Parity guard: every TOML table documented in the configuration
    // reference must have a corresponding commented stanza in the init
    // template, so the two cannot silently drift apart.
    let reference = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/src/configuration/README.md"),
    )
    .expect("configuration reference should be readable");

    let mut documented = std::collections::BTreeSet::new();
    let mut in_toml_fence = false;
    for line in reference.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_toml_fence = trimmed.starts_with("```toml");
            continue;
        }
        if in_toml_fence && let Some(path) = table_header(line) {
            documented.insert(section_family(&path));
        }
    }
    assert!(
        !documented.is_empty(),
        "expected to extract documented config sections from the reference",
    );

    let template = template_section_families();
    let missing: Vec<_> = documented.difference(&template).cloned().collect();
    assert!(
        missing.is_empty(),
        "config sections documented in the reference but missing a commented stanza \
             in the init template: {missing:?}",
    );
}

#[test]
fn untouched_generated_template_resolves_to_unchanged_defaults() {
    // Every added stanza is commented, so parsing the generated template
    // must yield exactly the same effective config as its sole active
    // line (worktree_placement) on its own — no new behavior is activated.
    let generated: PawConfig =
        toml::from_str(&generate_default_config()).expect("generated template parses");
    let baseline: PawConfig =
        toml::from_str("worktree_placement = \"child\"\n").expect("baseline parses");
    assert_eq!(
        generated, baseline,
        "commented stanzas must not change the effective configuration",
    );
}

// --- Struct Default impls (one assertion block per sub-struct) ---

#[test]
fn struct_defaults_match_spec() {
    // BrokerConfig: disabled, loopback host, canonical port.
    let broker = BrokerConfig::default();
    assert!(!broker.enabled, "broker disabled by default");
    assert_eq!(broker.port, 9119, "broker default port");
    assert_eq!(broker.bind, "127.0.0.1", "broker default bind");

    // ConflictConfig: 120s window, both reactions on.
    let conflict = ConflictConfig::default();
    assert_eq!(conflict.window_seconds, 120, "conflict window");
    assert!(conflict.warn_on_intent_overlap, "conflict warn default");
    assert!(conflict.escalate_on_violation, "conflict escalate default");

    // AutoApproveConfig: enabled, no extra safe commands, 30s stall, Safe preset.
    let auto_approve = AutoApproveConfig::default();
    assert!(
        auto_approve.enabled,
        "auto_approve enabled defaults to true"
    );
    assert!(
        auto_approve.safe_commands.is_empty(),
        "auto_approve safe_commands defaults to empty"
    );
    assert_eq!(
        auto_approve.stall_threshold_seconds, 30,
        "auto_approve stall threshold"
    );
    assert_eq!(
        auto_approve.approval_level,
        ApprovalLevelPreset::Safe,
        "auto_approve approval level"
    );

    // DashboardConfig: message log hidden; broker_log carries BrokerLogConfig defaults.
    let dashboard = DashboardConfig::default();
    assert!(!dashboard.show_message_log, "dashboard message log hidden");
    assert_eq!(
        dashboard.broker_log.max_messages, 500,
        "dashboard broker_log max_messages default"
    );
    assert!(
        dashboard.broker_log.default_visible,
        "dashboard broker_log visible default"
    );
    assert!(
        dashboard.broker_log.height_lines > 12,
        "dashboard broker_log height_lines must exceed the v0.6.0 fixed 12"
    );

    // BrokerLogConfig: cap 500, visible, height strictly greater than the v0.6.0 fixed 12.
    let broker_log = BrokerLogConfig::default();
    assert_eq!(
        broker_log.max_messages, 500,
        "broker_log max_messages default"
    );
    assert!(broker_log.default_visible, "broker_log visible default");
    assert!(
        broker_log.height_lines > 12,
        "default height_lines must be strictly greater than the v0.6.0 fixed 12, got {}",
        broker_log.height_lines,
    );

    // LearningsConfig: 60s flush interval.
    assert_eq!(
        LearningsConfig::default().flush_interval_seconds,
        60,
        "learnings flush interval default"
    );
}

// --- BrokerConfig ---

#[test]
fn broker_config_url() {
    let config = BrokerConfig::default();
    assert_eq!(config.url(), "http://127.0.0.1:9119");

    let custom = BrokerConfig {
        enabled: true,
        port: 8080,
        bind: "0.0.0.0".to_string(),
        ..Default::default()
    };
    assert_eq!(custom.url(), "http://0.0.0.0:8080");
}

#[test]
fn empty_config_gets_broker_defaults() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "");

    let config = load_config_file(&path).unwrap().unwrap();
    assert!(!config.broker.enabled);
    assert_eq!(config.broker.port, 9119);
    assert_eq!(config.broker.bind, "127.0.0.1");
}

#[test]
fn parses_full_broker_section() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[broker]\nenabled = true\nport = 8080\nbind = \"0.0.0.0\"\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    assert!(config.broker.enabled);
    assert_eq!(config.broker.port, 8080);
    assert_eq!(config.broker.bind, "0.0.0.0");
}

#[test]
fn parses_partial_broker_section() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[broker]\nenabled = true\n");

    let config = load_config_file(&path).unwrap().unwrap();
    assert!(config.broker.enabled);
    assert_eq!(config.broker.port, 9119);
    assert_eq!(config.broker.bind, "127.0.0.1");
}

// --- Section-absent defaults (one row per optional section) ---

#[test]
fn absent_section_resolves_to_none_or_default() {
    // Each row: a TOML document that omits the named section, plus a check
    // that the corresponding field resolves to its None/default state.
    type Check = fn(&PawConfig) -> bool;
    for (section, fixture, check) in [
        (
            "supervisor",
            "default_cli = \"claude\"\n",
            (|c: &PawConfig| c.supervisor.is_none()) as Check,
        ),
        (
            "dashboard",
            "default_cli = \"claude\"\n",
            |c: &PawConfig| c.dashboard.is_none(),
        ),
        ("mcp", "default_cli = \"claude\"\n", |c: &PawConfig| {
            c.mcp == McpConfig::default()
                && c.mcp.name.is_none()
                && c.mcp_server_name() == "git-paw"
        }),
        (
            "governance",
            "default_cli = \"claude\"\n",
            |c: &PawConfig| {
                let g = &c.governance;
                g.adr.is_none()
                    && g.test_strategy.is_none()
                    && g.security.is_none()
                    && g.dod.is_none()
                    && g.constitution.is_none()
            },
        ),
        ("opsx", "default_cli = \"claude\"\n", |c: &PawConfig| {
            c.opsx.is_none() && c.role_gating_mode() == RoleGatingMode::Warn
        }),
        (
            "supervisor.auto_approve",
            "[supervisor]\nenabled = true\n",
            |c: &PawConfig| {
                c.supervisor
                    .as_ref()
                    .is_some_and(|s| s.auto_approve.is_none())
            },
        ),
    ] {
        let config: PawConfig = toml::from_str(fixture)
            .unwrap_or_else(|e| panic!("[{section}] fixture must parse: {e}"));
        assert!(
            check(&config),
            "with [{section}] absent, its field must resolve to None/default"
        );
    }
}

// --- SupervisorConfig ---

#[test]
fn parses_full_supervisor_section() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\n\
             enabled = true\n\
             cli = \"claude\"\n\
             test_command = \"just check\"\n\
             agent_approval = \"full-auto\"\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert!(supervisor.enabled);
    assert_eq!(supervisor.cli.as_deref(), Some("claude"));
    assert_eq!(supervisor.test_command.as_deref(), Some("just check"));
    assert_eq!(supervisor.agent_approval, ApprovalLevel::FullAuto);
}

#[test]
fn parses_partial_supervisor_section() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[supervisor]\nenabled = true\n");

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert!(supervisor.enabled);
    assert_eq!(supervisor.cli, None);
    assert_eq!(supervisor.test_command, None);
    assert_eq!(supervisor.agent_approval, ApprovalLevel::Auto);
    assert_eq!(supervisor.approval, None);
}

// --- verify_on_commit_nudge (per-commit-verification-v0-6-x) ---

#[test]
fn verify_on_commit_nudge_defaults_true_when_absent() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[supervisor]\nenabled = true\n");

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert_eq!(
        supervisor.verify_on_commit_nudge, None,
        "an omitted field must deserialise as None"
    );
    assert!(
        supervisor.verify_on_commit_nudge_enabled(),
        "an unset verify_on_commit_nudge must resolve to true (default on)"
    );
}

#[test]
fn verify_on_commit_nudge_explicit_false_disables() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\nenabled = true\nverify_on_commit_nudge = false\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert_eq!(supervisor.verify_on_commit_nudge, Some(false));
    assert!(
        !supervisor.verify_on_commit_nudge_enabled(),
        "an explicit `false` must disable the nudge"
    );
}

#[test]
fn verify_on_commit_nudge_explicit_true_enables() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\nenabled = true\nverify_on_commit_nudge = true\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert_eq!(supervisor.verify_on_commit_nudge, Some(true));
    assert!(supervisor.verify_on_commit_nudge_enabled());
}

#[test]
fn rejects_invalid_approval_level() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[supervisor]\nagent_approval = \"yolo\"\n");

    let err = load_config_file(&path).unwrap_err();
    assert!(
        err.to_string().contains("yolo"),
        "error should mention invalid value, got: {err}"
    );
}

// --- supervisor approval (supervisor-native-auto-mode) ---

#[test]
fn supervisor_approval_parses_all_three_levels() {
    let tmp = TempDir::new().unwrap();
    for (value, expected) in [
        ("manual", ApprovalLevel::Manual),
        ("auto", ApprovalLevel::Auto),
        ("full-auto", ApprovalLevel::FullAuto),
    ] {
        let path = tmp.path().join(format!("config-{value}.toml"));
        write_file(&path, &format!("[supervisor]\napproval = \"{value}\"\n"));

        let config = load_config_file(&path).unwrap().unwrap();
        let supervisor = config.supervisor.unwrap();
        assert_eq!(
            supervisor.approval,
            Some(expected),
            "approval = \"{value}\" should parse"
        );
    }
}

#[test]
fn rejects_invalid_supervisor_approval_value() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[supervisor]\napproval = \"yolo\"\n");

    let err = load_config_file(&path).unwrap_err();
    assert!(
        err.to_string().contains("yolo"),
        "error should mention invalid value, got: {err}"
    );
}

#[test]
fn unset_supervisor_approval_omitted_on_round_trip() {
    let supervisor = SupervisorConfig {
        enabled: true,
        ..Default::default()
    };
    let serialized = toml::to_string_pretty(&supervisor).unwrap();
    assert!(
        !serialized
            .lines()
            .any(|l| l.trim_start().starts_with("approval =")),
        "unset approval must not serialize, got:\n{serialized}"
    );
}

#[test]
fn supervisor_round_trips_through_save_and_load() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");

    let original = PawConfig {
        supervisor: Some(SupervisorConfig {
            enabled: true,
            cli: Some("claude".into()),
            test_command: Some("just check".into()),
            lint_command: None,
            build_command: None,
            doc_build_command: None,
            doc_tool_command: None,
            spec_validate_command: None,
            fmt_check_command: None,
            security_audit_command: None,
            agent_approval: ApprovalLevel::FullAuto,
            approval: Some(ApprovalLevel::FullAuto),
            auto_approve: None,
            conflict: ConflictConfig::default(),
            learnings: false,
            learnings_config: LearningsConfig::default(),
            common_dev_allowlist: CommonDevAllowlistConfig::default(),
            verify_on_commit_nudge: None,
            strict_branch_guard: None,
            auto_revert: None,
            manual_approvals_log: None,
            no_progress_window_seconds: None,
            context_bloat_threshold_k: None,
            blocked_on_supervisor_window_seconds: None,
            tell: TellConfig::default(),
        }),
        ..Default::default()
    };

    save_config_to(&config_path, &original).unwrap();
    let loaded = load_config_file(&config_path).unwrap().unwrap();
    assert_eq!(loaded.supervisor, original.supervisor);
}

// --- manual_approvals_log (approval-pattern-surfacing) ---

#[test]
fn manual_approvals_log_defaults_to_true_when_absent() {
    // [supervisor] present without the field → recording on by default.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[supervisor]\nenabled = true\n");
    let cfg = load_config_file(&path).unwrap().unwrap();
    let sup = cfg.supervisor.unwrap();
    assert_eq!(sup.manual_approvals_log, None);
    assert!(
        sup.manual_approvals_log_enabled(),
        "absent field must resolve to true"
    );
}

#[test]
fn manual_approvals_log_explicit_false_opts_out() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\nenabled = true\nmanual_approvals_log = false\n",
    );
    let cfg = load_config_file(&path).unwrap().unwrap();
    let sup = cfg.supervisor.unwrap();
    assert_eq!(sup.manual_approvals_log, Some(false));
    assert!(!sup.manual_approvals_log_enabled());
}

#[test]
fn pre_v050_config_parses_with_manual_approvals_log_absent() {
    // A config produced before this change (no `manual_approvals_log`
    // field) parses cleanly and the resolver still yields true.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\nenabled = true\ncli = \"claude\"\nlearnings = true\n",
    );
    let cfg = load_config_file(&path).unwrap().unwrap();
    let sup = cfg.supervisor.unwrap();
    assert_eq!(sup.manual_approvals_log, None);
    assert!(sup.manual_approvals_log_enabled());
}

// --- Gate-command fields (supervisor-gate-templating-v0-5-x) ---

#[test]
fn strict_branch_guard_defaults_to_true_and_honours_opt_out() {
    // Absent field → enforcement on by default.
    let on = TempDir::new().unwrap();
    let on_path = on.path().join("config.toml");
    write_file(&on_path, "[supervisor]\nenabled = true\n");
    let cfg = load_config_file(&on_path).unwrap().unwrap();
    let sup = cfg.supervisor.unwrap();
    assert_eq!(sup.strict_branch_guard, None);
    assert!(sup.strict_branch_guard(), "default must resolve to true");

    // Explicit opt-out → enforcement off (detection still applies).
    let off = TempDir::new().unwrap();
    let off_path = off.path().join("config.toml");
    write_file(
        &off_path,
        "[supervisor]\nenabled = true\nstrict_branch_guard = false\n",
    );
    let cfg = load_config_file(&off_path).unwrap().unwrap();
    let sup = cfg.supervisor.unwrap();
    assert_eq!(sup.strict_branch_guard, Some(false));
    assert!(!sup.strict_branch_guard());
}

#[test]
fn gate_command_fields_default_to_none() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[supervisor]\nenabled = true\n");

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert_eq!(supervisor.test_command, None);
    assert_eq!(supervisor.lint_command, None);
    assert_eq!(supervisor.build_command, None);
    assert_eq!(supervisor.doc_build_command, None);
    assert_eq!(supervisor.doc_tool_command, None);
    assert_eq!(supervisor.spec_validate_command, None);
    assert_eq!(supervisor.fmt_check_command, None);
    assert_eq!(supervisor.security_audit_command, None);
}

#[test]
fn gate_command_fields_round_trip() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");

    let original = PawConfig {
        supervisor: Some(SupervisorConfig {
            enabled: true,
            cli: Some("claude".into()),
            test_command: Some("just check".into()),
            lint_command: Some("cargo clippy -- -D warnings".into()),
            build_command: Some("cargo build".into()),
            doc_build_command: Some("mdbook build docs/".into()),
            doc_tool_command: Some("cargo doc --no-deps".into()),
            spec_validate_command: Some("openspec validate {{CHANGE_ID}} --strict".into()),
            fmt_check_command: Some("cargo fmt --check".into()),
            security_audit_command: Some("cargo audit".into()),
            ..Default::default()
        }),
        ..Default::default()
    };

    save_config_to(&config_path, &original).unwrap();
    let loaded = load_config_file(&config_path).unwrap().unwrap();
    assert_eq!(loaded.supervisor, original.supervisor);
}

#[test]
fn gate_command_fields_omit_from_toml_when_none() {
    let supervisor = SupervisorConfig {
        enabled: true,
        test_command: None,
        lint_command: None,
        build_command: None,
        doc_build_command: None,
        doc_tool_command: None,
        spec_validate_command: None,
        fmt_check_command: None,
        security_audit_command: None,
        ..Default::default()
    };
    let serialized = toml::to_string_pretty(&supervisor).unwrap();
    for key in [
        "test_command",
        "lint_command",
        "build_command",
        "doc_build_command",
        "doc_tool_command",
        "spec_validate_command",
        "fmt_check_command",
        "security_audit_command",
    ] {
        assert!(
            !serialized.contains(key),
            "TOML serialised with None gate fields should omit `{key}`; got:\n{serialized}",
        );
    }
}

// --- stuck/bloat detection thresholds (supervisor-stuck-bloat-detection) ---

#[test]
fn stuck_detection_fields_default_to_none() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[supervisor]\nenabled = true\n");

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert_eq!(supervisor.no_progress_window_seconds, None);
    assert_eq!(supervisor.context_bloat_threshold_k, None);
    assert_eq!(supervisor.blocked_on_supervisor_window_seconds, None);
}

#[test]
fn stuck_detection_fields_round_trip() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");

    let original = PawConfig {
        supervisor: Some(SupervisorConfig {
            enabled: true,
            no_progress_window_seconds: Some(1800),
            context_bloat_threshold_k: Some(300),
            blocked_on_supervisor_window_seconds: Some(600),
            ..Default::default()
        }),
        ..Default::default()
    };

    save_config_to(&config_path, &original).unwrap();
    let loaded = load_config_file(&config_path).unwrap().unwrap();
    assert_eq!(loaded.supervisor, original.supervisor);
    let supervisor = loaded.supervisor.unwrap();
    assert_eq!(supervisor.no_progress_window_seconds, Some(1800));
    assert_eq!(supervisor.context_bloat_threshold_k, Some(300));
    assert_eq!(supervisor.blocked_on_supervisor_window_seconds, Some(600));
}

#[test]
fn stuck_detection_fields_omit_from_toml_when_none() {
    let supervisor = SupervisorConfig {
        enabled: true,
        no_progress_window_seconds: None,
        context_bloat_threshold_k: None,
        blocked_on_supervisor_window_seconds: None,
        ..Default::default()
    };
    let serialized = toml::to_string_pretty(&supervisor).unwrap();
    for key in [
        "no_progress_window_seconds",
        "context_bloat_threshold_k",
        "blocked_on_supervisor_window_seconds",
    ] {
        assert!(
            !serialized.contains(key),
            "TOML serialised with None stuck-detection fields should omit `{key}`; got:\n{serialized}",
        );
    }
}

#[test]
fn stuck_detection_fields_pre_existing_config_loads() {
    // A config authored before these fields existed SHALL load cleanly with
    // the new fields defaulting to None (backward compatibility).
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\n\
             enabled = true\n\
             test_command = \"just check\"\n\
             strict_branch_guard = true\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert_eq!(supervisor.no_progress_window_seconds, None);
    assert_eq!(supervisor.context_bloat_threshold_k, None);
    assert_eq!(supervisor.blocked_on_supervisor_window_seconds, None);
    assert_eq!(supervisor.test_command.as_deref(), Some("just check"));
}

#[test]
fn stuck_detection_fields_explicit_values_preserved() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\n\
             enabled = true\n\
             no_progress_window_seconds = 900\n\
             context_bloat_threshold_k = 200\n\
             blocked_on_supervisor_window_seconds = 1200\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert_eq!(supervisor.no_progress_window_seconds, Some(900));
    assert_eq!(supervisor.context_bloat_threshold_k, Some(200));
    assert_eq!(supervisor.blocked_on_supervisor_window_seconds, Some(1200));
}

// --- doc_tool_command (lang-agnostic-skills) ---

#[test]
fn doc_tool_command_default_none() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[supervisor]\nenabled = true\n");

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert_eq!(supervisor.doc_tool_command, None);
}

#[test]
fn doc_tool_command_explicit_value_preserved() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\n\
             enabled = true\n\
             doc_tool_command = \"sphinx-build -W docs docs/_build\"\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert_eq!(
        supervisor.doc_tool_command.as_deref(),
        Some("sphinx-build -W docs docs/_build"),
        "explicit doc_tool_command value (including all whitespace) must be preserved verbatim",
    );
}

#[test]
fn doc_tool_command_v0_5_config_parses_without_field() {
    // A v0.5.0 config that predates the doc_tool_command field SHALL
    // load cleanly with the field defaulting to None.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\n\
             enabled = true\n\
             test_command = \"just check\"\n\
             lint_command = \"cargo clippy -- -D warnings\"\n\
             build_command = \"cargo build\"\n\
             doc_build_command = \"mdbook build docs/\"\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert_eq!(supervisor.doc_tool_command, None);
    assert_eq!(supervisor.test_command.as_deref(), Some("just check"));
}

#[test]
fn doc_tool_command_flows_into_gate_commands() {
    let supervisor = SupervisorConfig {
        doc_tool_command: Some("javadoc -d docs/api src/**/*.java".into()),
        ..Default::default()
    };
    let gates = supervisor.gate_commands();
    assert_eq!(
        gates.doc_tool_command,
        Some("javadoc -d docs/api src/**/*.java"),
    );
}

// --- CommonDevAllowlistConfig ---

#[test]
fn supervisor_common_dev_allowlist_defaults_when_section_absent() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[supervisor]\nenabled = true\n");

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert!(supervisor.common_dev_allowlist.enabled);
    assert!(supervisor.common_dev_allowlist.stacks.is_empty());
    assert!(supervisor.common_dev_allowlist.extra.is_empty());
}

#[test]
fn supervisor_common_dev_allowlist_stacks_parsed() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\nenabled = true\n\
             [supervisor.common_dev_allowlist]\nstacks = [\"rust\", \"node\"]\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert_eq!(
        supervisor.common_dev_allowlist.stacks,
        vec!["rust".to_string(), "node".to_string()],
    );
    // extra still defaults to empty; enabled stays true.
    assert!(supervisor.common_dev_allowlist.extra.is_empty());
    assert!(supervisor.common_dev_allowlist.enabled);
}

#[test]
fn supervisor_common_dev_allowlist_disabled_opt_out() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\nenabled = true\n\
             [supervisor.common_dev_allowlist]\nenabled = false\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert!(!supervisor.common_dev_allowlist.enabled);
    // extra still defaults to empty.
    assert!(supervisor.common_dev_allowlist.extra.is_empty());
}

#[test]
fn supervisor_common_dev_allowlist_extra_parsed() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\nenabled = true\n\
             [supervisor.common_dev_allowlist]\nextra = [\"pnpm test\", \"deno fmt\"]\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert_eq!(
        supervisor.common_dev_allowlist.extra,
        vec!["pnpm test".to_string(), "deno fmt".to_string()],
    );
    // enabled stays at default true.
    assert!(supervisor.common_dev_allowlist.enabled);
}

#[test]
fn supervisor_common_dev_allowlist_round_trips_through_save_and_load() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");

    let original = PawConfig {
        supervisor: Some(SupervisorConfig {
            enabled: true,
            common_dev_allowlist: CommonDevAllowlistConfig {
                enabled: false,
                stacks: vec!["rust".into(), "node".into()],
                extra: vec!["pnpm test".into(), "uv pip install".into()],
            },
            ..Default::default()
        }),
        ..Default::default()
    };

    save_config_to(&config_path, &original).unwrap();
    let loaded = load_config_file(&config_path).unwrap().unwrap();
    assert_eq!(loaded.supervisor, original.supervisor);
}

#[test]
fn generated_default_config_template_contains_common_dev_allowlist_section() {
    let template = generate_default_config();
    assert!(
        template.contains("[supervisor.common_dev_allowlist]"),
        "default template should document the new sub-table",
    );
    assert!(
        template.contains("enabled = true"),
        "template should show the enabled default",
    );
    assert!(
        template.contains("extra ="),
        "template should illustrate the extra field",
    );
    assert!(
        template.contains("stacks ="),
        "template should illustrate the stacks field",
    );
}

// --- LearningsConfig (learnings-mode) ---

#[test]
fn learnings_defaults_to_false_when_supervisor_section_absent_field() {
    // [supervisor] present without `learnings` → learnings = false
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[supervisor]\nenabled = true\n");

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert!(!supervisor.learnings);
    assert_eq!(supervisor.learnings_config.flush_interval_seconds, 60);
}

#[test]
fn learnings_true_loads() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[supervisor]\nenabled = true\nlearnings = true\n");

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert!(supervisor.learnings);
    // Defaults still applied for the nested table.
    assert_eq!(supervisor.learnings_config.flush_interval_seconds, 60);
}

#[test]
fn learnings_config_custom_flush_interval_is_honoured() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\n\
             enabled = true\n\
             learnings = true\n\
             [supervisor.learnings_config]\n\
             flush_interval_seconds = 30\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert!(supervisor.learnings);
    assert_eq!(supervisor.learnings_config.flush_interval_seconds, 30);
}

#[test]
fn learnings_round_trips_through_save_and_load() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");

    let original = PawConfig {
        supervisor: Some(SupervisorConfig {
            enabled: true,
            learnings: true,
            learnings_config: LearningsConfig {
                flush_interval_seconds: 90,
                broker_publish: BrokerPublish::ForceOff,
            },
            ..Default::default()
        }),
        ..Default::default()
    };

    save_config_to(&config_path, &original).unwrap();
    let loaded = load_config_file(&config_path).unwrap().unwrap();
    assert_eq!(loaded.supervisor, original.supervisor);
    let supervisor = loaded.supervisor.unwrap();
    assert!(supervisor.learnings);
    assert_eq!(supervisor.learnings_config.flush_interval_seconds, 90);
}

#[test]
fn generated_default_config_contains_commented_supervisor_section() {
    let output = generate_default_config();
    assert!(output.contains("[supervisor]"));
    assert!(output.contains("enabled"));
    assert!(output.contains("test_command"));
    assert!(output.contains("agent_approval"));
    // Stuck/bloat detection thresholds are listed with example values.
    assert!(output.contains("no_progress_window_seconds = 1500"));
    assert!(output.contains("context_bloat_threshold_k = 250"));
    assert!(output.contains("blocked_on_supervisor_window_seconds = 900"));
}

// --- DashboardConfig ---

#[test]
fn parses_dashboard_section_with_show_message_log() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[dashboard]\nshow_message_log = true\n");

    let config = load_config_file(&path).unwrap().unwrap();
    let dashboard = config.dashboard.unwrap();
    assert!(dashboard.show_message_log);
}

#[test]
fn dashboard_merge_repo_wins() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("global").join("config.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    write_file(&global_path, "[dashboard]\nshow_message_log = false\n");
    write_file(
        &repo_config_path(&repo_root),
        "[dashboard]\nshow_message_log = true\n",
    );

    let config = load_config_from(&global_path, &repo_root).unwrap();
    let dashboard = config.dashboard.unwrap();
    assert!(dashboard.show_message_log);
}

#[test]
fn dashboard_round_trip_through_save_and_load() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");

    let original = PawConfig {
        dashboard: Some(DashboardConfig {
            show_message_log: true,
            ..Default::default()
        }),
        ..Default::default()
    };

    save_config_to(&config_path, &original).unwrap();
    let loaded = load_config_file(&config_path).unwrap().unwrap();
    assert_eq!(loaded.dashboard, original.dashboard);
    assert!(loaded.dashboard.unwrap().show_message_log);
}

// --- BrokerLogConfig (dashboard-broker-log task 1.3) ---

#[test]
fn parses_broker_log_section_with_explicit_overrides() {
    // Task 1.3: explicit override load.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[dashboard.broker_log]\nmax_messages = 100\ndefault_visible = false\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    let dashboard = config.dashboard.unwrap();
    assert_eq!(dashboard.broker_log.max_messages, 100);
    assert!(!dashboard.broker_log.default_visible);
}

#[test]
fn broker_log_partial_section_fills_remaining_defaults() {
    // A `[dashboard.broker_log]` table that sets only one field still
    // loads the documented default for the other (per-field
    // `#[serde(default)]`).
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[dashboard.broker_log]\nmax_messages = 42\n");

    let config = load_config_file(&path).unwrap().unwrap();
    let broker_log = config.dashboard.unwrap().broker_log;
    assert_eq!(broker_log.max_messages, 42);
    assert!(
        broker_log.default_visible,
        "default_visible must fall back to true when omitted"
    );
    assert_eq!(
        broker_log.height_lines,
        BrokerLogConfig::default_height_lines(),
        "height_lines must fall back to the documented default when omitted"
    );
}

#[test]
fn height_lines_parses_explicit_value() {
    // Configuration scenario "height_lines explicitly configured": an
    // explicit `[dashboard.broker_log] height_lines = 24` loads as 24.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[dashboard.broker_log]\nheight_lines = 24\n");

    let config = load_config_file(&path).unwrap().unwrap();
    let broker_log = config.dashboard.unwrap().broker_log;
    assert_eq!(broker_log.height_lines, 24);
}

#[test]
fn height_lines_absent_uses_default() {
    // Configuration scenario "height_lines absent uses the default": a
    // `[dashboard.broker_log]` table that omits the field loads the
    // documented default, which is strictly greater than 12.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[dashboard.broker_log]\ndefault_visible = true\n");

    let config = load_config_file(&path).unwrap().unwrap();
    let broker_log = config.dashboard.unwrap().broker_log;
    assert_eq!(
        broker_log.height_lines,
        BrokerLogConfig::default_height_lines()
    );
    assert!(broker_log.height_lines > 12);
}

#[test]
fn broker_log_round_trips_through_save_and_load() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");

    let original = PawConfig {
        dashboard: Some(DashboardConfig {
            show_message_log: false,
            broker_log: BrokerLogConfig {
                max_messages: 250,
                default_visible: false,
                height_lines: 30,
            },
        }),
        ..Default::default()
    };

    save_config_to(&config_path, &original).unwrap();
    let loaded = load_config_file(&config_path).unwrap().unwrap();
    assert_eq!(loaded.dashboard, original.dashboard);
    // Configuration scenario "height_lines round-trips through save and
    // load": the re-parsed value matches what was written.
    assert_eq!(loaded.dashboard.unwrap().broker_log.height_lines, 30);
}

#[test]
fn get_dashboard_returns_none_when_not_configured() {
    let config = PawConfig::default();
    assert!(config.get_dashboard().is_none());
}

#[test]
fn get_dashboard_returns_config_when_present() {
    let config = PawConfig {
        dashboard: Some(DashboardConfig {
            show_message_log: true,
            ..Default::default()
        }),
        ..Default::default()
    };
    let dashboard = config.get_dashboard().unwrap();
    assert!(dashboard.show_message_log);
}

// --- approval_flags mapping ---

#[test]
fn approval_flags_maps_each_cli_and_level() {
    // One row per built-in (cli, level) -> native-flag mapping. `Manual`
    // and unrecognised CLIs resolve to the empty string; `agy`
    // (Antigravity) shares Claude's flag; the retired `gemini` has no
    // built-in row; `qwen` keeps `--yolo`.
    for (cli, level, expected) in [
        (
            "claude",
            ApprovalLevel::FullAuto,
            "--dangerously-skip-permissions",
        ),
        ("codex", ApprovalLevel::Auto, "--sandbox workspace-write"),
        (
            "codex",
            ApprovalLevel::FullAuto,
            "--dangerously-bypass-approvals-and-sandbox",
        ),
        ("qwen", ApprovalLevel::FullAuto, "--yolo"),
        (
            "agy",
            ApprovalLevel::FullAuto,
            "--dangerously-skip-permissions",
        ),
        ("gemini", ApprovalLevel::FullAuto, ""),
        ("some-agent", ApprovalLevel::FullAuto, ""),
        ("claude", ApprovalLevel::Manual, ""),
        ("codex", ApprovalLevel::Manual, ""),
    ] {
        assert_eq!(
            approval_flags(cli, &level),
            expected,
            "approval_flags({cli:?}, {level:?})"
        );
    }
}

// --- resolve_approval_flags (override → built-in table → "") ---

fn cli_with_approval_args(command: &str, args: &[(&str, &str)]) -> CustomCli {
    CustomCli {
        command: command.into(),
        display_name: None,
        submit_delay_ms: None,
        settings_path: None,
        approval_args: args
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect(),
    }
}

#[test]
fn resolve_approval_flags_override_wins_over_built_in_table() {
    let clis = HashMap::from([(
        "claude".to_string(),
        cli_with_approval_args("claude", &[("full-auto", "--my-custom-flag")]),
    )]);
    assert_eq!(
        resolve_approval_flags("claude", &ApprovalLevel::FullAuto, &clis),
        "--my-custom-flag"
    );
}

#[test]
fn resolve_approval_flags_override_enables_cli_without_built_in_row() {
    // The claude-oss scenario: a variant CLI with no built-in table row
    // gets native flags purely from its [clis.<name>] override.
    let clis = HashMap::from([(
        "claude-oss".to_string(),
        cli_with_approval_args(
            "claude-oss",
            &[("full-auto", "--dangerously-skip-permissions")],
        ),
    )]);
    assert_eq!(
        resolve_approval_flags("claude-oss", &ApprovalLevel::FullAuto, &clis),
        "--dangerously-skip-permissions"
    );
}

#[test]
fn resolve_approval_flags_falls_back_to_table_when_level_not_overridden() {
    // An override map that lacks the requested level falls through to
    // the built-in row for that CLI.
    let clis = HashMap::from([(
        "claude".to_string(),
        cli_with_approval_args("claude", &[("auto", "--some-auto-flag")]),
    )]);
    assert_eq!(
        resolve_approval_flags("claude", &ApprovalLevel::FullAuto, &clis),
        "--dangerously-skip-permissions"
    );
}

#[test]
fn resolve_approval_flags_unknown_cli_no_override_is_empty() {
    let clis = HashMap::new();
    assert_eq!(
        resolve_approval_flags("some-agent", &ApprovalLevel::FullAuto, &clis),
        ""
    );
}

#[test]
fn supervisor_merge_repo_wins() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("global").join("config.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    write_file(
        &global_path,
        "[supervisor]\nenabled = false\nagent_approval = \"manual\"\n",
    );
    write_file(
        &repo_config_path(&repo_root),
        "[supervisor]\nenabled = true\nagent_approval = \"full-auto\"\n",
    );

    let config = load_config_from(&global_path, &repo_root).unwrap();
    let supervisor = config.supervisor.unwrap();
    assert!(supervisor.enabled);
    assert_eq!(supervisor.agent_approval, ApprovalLevel::FullAuto);
}

#[test]
fn broker_config_round_trip() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");

    let original = PawConfig {
        broker: BrokerConfig {
            enabled: true,
            port: 9200,
            bind: "127.0.0.1".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    save_config_to(&config_path, &original).unwrap();
    let loaded = load_config_file(&config_path).unwrap().unwrap();
    assert_eq!(loaded.broker.enabled, original.broker.enabled);
    assert_eq!(loaded.broker.port, original.broker.port);
    assert_eq!(loaded.broker.bind, original.broker.bind);
}

// --- AutoApproveConfig (auto-approve-patterns / approval-configuration) ---

#[test]
fn auto_approve_section_parses_full_body() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\n\
             enabled = true\n\
             [supervisor.auto_approve]\n\
             enabled = false\n\
             safe_commands = [\"just smoke\"]\n\
             stall_threshold_seconds = 60\n\
             approval_level = \"conservative\"\n",
    );
    let config = load_config_file(&path).unwrap().unwrap();
    let aa = config.supervisor.unwrap().auto_approve.unwrap();
    assert!(!aa.enabled);
    assert_eq!(aa.safe_commands, vec!["just smoke".to_string()]);
    assert_eq!(aa.stall_threshold_seconds, 60);
    assert_eq!(aa.approval_level, ApprovalLevelPreset::Conservative);
}

#[test]
fn auto_approve_enabled_defaults_to_true_when_omitted() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\n[supervisor.auto_approve]\nstall_threshold_seconds = 30\n",
    );
    let config = load_config_file(&path).unwrap().unwrap();
    let aa = config.supervisor.unwrap().auto_approve.unwrap();
    assert!(aa.enabled, "enabled should default to true");
}

#[test]
fn auto_approve_off_preset_forces_disabled() {
    let cfg = AutoApproveConfig {
        enabled: true,
        approval_level: ApprovalLevelPreset::Off,
        ..AutoApproveConfig::default()
    };
    let resolved = cfg.resolved();
    assert!(!resolved.enabled, "Off preset must force enabled = false");
}

// --- Bug 8: [broker.watcher] republish_working_ttl_seconds ---

#[test]
fn watcher_ttl_defaults_to_sixty_when_absent() {
    let cfg = WatcherConfig::default();
    assert_eq!(cfg.republish_working_ttl_seconds(), 60);
}

#[test]
fn watcher_ttl_zero_disables() {
    let cfg = WatcherConfig {
        republish_working_ttl_seconds: Some(0),
    };
    assert_eq!(cfg.republish_working_ttl_seconds(), 0);
}

#[test]
fn watcher_ttl_below_floor_clamps_to_five() {
    let cfg = WatcherConfig {
        republish_working_ttl_seconds: Some(2),
    };
    assert_eq!(
        cfg.republish_working_ttl_seconds(),
        WatcherConfig::MIN_REPUBLISH_TTL_SECONDS
    );
}

#[test]
fn watcher_ttl_explicit_non_zero_is_preserved() {
    let cfg = WatcherConfig {
        republish_working_ttl_seconds: Some(120),
    };
    assert_eq!(cfg.republish_working_ttl_seconds(), 120);
}

#[test]
fn watcher_ttl_parses_from_broker_table() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[broker]\nenabled = true\n[broker.watcher]\nrepublish_working_ttl_seconds = 0\n",
    );
    let config = load_config_file(&path).unwrap().unwrap();
    assert_eq!(config.broker.watcher.republish_working_ttl_seconds, Some(0));
    assert_eq!(config.broker.watcher.republish_working_ttl_seconds(), 0);
}

#[test]
fn approve_worktree_writes_defaults_to_true_when_absent() {
    // Spec scenario: default true auto-approves (field unset).
    let cfg = AutoApproveConfig::default();
    assert!(
        cfg.approve_worktree_writes(),
        "absent approve_worktree_writes must resolve to true"
    );
}

#[test]
fn approve_worktree_writes_explicit_false_resolves_false() {
    // Spec scenario: explicit false reverts to manual.
    let cfg = AutoApproveConfig {
        approve_worktree_writes: Some(false),
        ..AutoApproveConfig::default()
    };
    assert!(!cfg.approve_worktree_writes());
}

#[test]
fn approve_worktree_writes_parses_from_toml() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\nenabled = true\n[supervisor.auto_approve]\napprove_worktree_writes = false\n",
    );
    let config = load_config_file(&path).unwrap().unwrap();
    let aa = config.supervisor.unwrap().auto_approve.unwrap();
    assert_eq!(aa.approve_worktree_writes, Some(false));
    assert!(!aa.approve_worktree_writes());
}

#[test]
fn auto_approve_threshold_floor_clamps() {
    let cfg = AutoApproveConfig {
        stall_threshold_seconds: 0,
        ..AutoApproveConfig::default()
    };
    let resolved = cfg.resolved();
    assert_eq!(
        resolved.stall_threshold_seconds,
        AutoApproveConfig::MIN_STALL_THRESHOLD_SECONDS
    );
}

#[test]
fn auto_approve_safe_preset_keeps_defaults() {
    let cfg = AutoApproveConfig {
        approval_level: ApprovalLevelPreset::Safe,
        ..AutoApproveConfig::default()
    };
    let wl = cfg.effective_whitelist(&CommonDevAllowlistConfig::default());
    assert!(wl.iter().any(|c| c == "git commit"));
    assert!(wl.iter().any(|c| c == "git push"));
    assert!(wl.iter().any(|c| c.starts_with("curl")));
    // The universal dev-allowlist patterns are folded in.
    assert!(wl.iter().any(|c| c == "git diff"));
}

#[test]
fn auto_approve_conservative_drops_push_and_curl() {
    let cfg = AutoApproveConfig {
        approval_level: ApprovalLevelPreset::Conservative,
        ..AutoApproveConfig::default()
    };
    let wl = cfg.effective_whitelist(&CommonDevAllowlistConfig::default());
    assert!(wl.iter().any(|c| c == "git commit"));
    assert!(
        !wl.iter().any(|c| c.starts_with("git push")),
        "conservative drops git push"
    );
    assert!(
        !wl.iter().any(|c| c.starts_with("curl")),
        "conservative drops curl"
    );
}

#[test]
fn auto_approve_extras_are_unioned_with_defaults() {
    let cfg = AutoApproveConfig {
        safe_commands: vec!["just lint".to_string(), "just test".to_string()],
        ..AutoApproveConfig::default()
    };
    let wl = cfg.effective_whitelist(&CommonDevAllowlistConfig::default());
    assert!(wl.iter().any(|c| c == "grep"));
    assert!(wl.iter().any(|c| c == "just lint"));
    assert!(wl.iter().any(|c| c == "just test"));
}

#[test]
fn auto_approve_empty_extras_keep_defaults() {
    let cfg = AutoApproveConfig::default();
    let wl = cfg.effective_whitelist(&CommonDevAllowlistConfig::default());
    assert!(wl.iter().any(|c| c == "git commit"));
    assert!(wl.iter().any(|c| c == "grep"));
}

/// Spec scenario "Default whitelist is stack-neutral": no stacks declared
/// and no `safe_commands` — the composed whitelist carries no toolchain
/// entries, but keeps the read-mostly verbs, `git commit`, and the
/// broker-localhost curl prefix.
#[test]
fn effective_whitelist_default_is_stack_neutral() {
    let cfg = AutoApproveConfig::default();
    let wl = cfg.effective_whitelist(&CommonDevAllowlistConfig::default());
    for gone in ["cargo", "openspec", "just"] {
        assert!(
            !wl.iter()
                .any(|c| c == gone || c.starts_with(&format!("{gone} "))),
            "stack-neutral default must not contain {gone} entries: {wl:?}"
        );
    }
    for verb in crate::supervisor::auto_approve::READ_MOSTLY_VERBS {
        assert!(wl.iter().any(|c| c == verb), "missing read-mostly {verb}");
    }
    assert!(wl.iter().any(|c| c == "git commit"));
    assert!(wl.iter().any(|c| c == "curl http://127.0.0.1:"));
}

/// Spec scenario "Declared stack contributes its toolchain verbs": the
/// rust stack preset contributes `cargo test` to the composed whitelist.
#[test]
fn effective_whitelist_rust_stack_contributes_cargo() {
    use crate::supervisor::auto_approve::is_safe_command;
    let cfg = AutoApproveConfig::default();
    let dev = CommonDevAllowlistConfig {
        stacks: vec!["rust".to_string()],
        ..CommonDevAllowlistConfig::default()
    };
    let wl = cfg.effective_whitelist(&dev);
    assert!(is_safe_command("cargo test --workspace", &wl));
    assert!(is_safe_command("cargo fmt --check", &wl));
}

/// Spec scenario "Undeclared stack's verbs stay unknown": a node-stack
/// project gets no cargo entries.
#[test]
fn effective_whitelist_node_stack_has_no_cargo() {
    use crate::supervisor::auto_approve::is_safe_command;
    let cfg = AutoApproveConfig::default();
    let dev = CommonDevAllowlistConfig {
        stacks: vec!["node".to_string()],
        ..CommonDevAllowlistConfig::default()
    };
    let wl = cfg.effective_whitelist(&dev);
    assert!(!is_safe_command("cargo test", &wl));
    assert!(is_safe_command("npm test", &wl));
}

/// The `Conservative` strip applies AFTER composition, so it governs
/// stack-contributed and `safe_commands` entries as well as built-ins.
#[test]
fn effective_whitelist_conservative_strips_post_composition() {
    let cfg = AutoApproveConfig {
        approval_level: ApprovalLevelPreset::Conservative,
        safe_commands: vec!["curl -X POST".to_string()],
        ..AutoApproveConfig::default()
    };
    let dev = CommonDevAllowlistConfig {
        stacks: vec!["rust".to_string()],
        ..CommonDevAllowlistConfig::default()
    };
    let wl = cfg.effective_whitelist(&dev);
    assert!(
        !wl.iter().any(|c| c.starts_with("curl")),
        "conservative must strip curl entries from every source: {wl:?}"
    );
    assert!(
        !wl.iter().any(|c| c.starts_with("git push")),
        "conservative must strip git push contributed by the dev preset"
    );
    assert!(
        wl.iter().any(|c| c == "cargo test"),
        "stack-contributed non-push/curl entries survive the strip"
    );
}

/// Spec scenario `auto-approve-patterns/safe-command-classification`:
/// "Config adds project-specific patterns" — a TOML config with
/// `safe_commands = ["just smoke"]` must yield an effective whitelist
/// such that `is_safe_command("just smoke -v", &whitelist)` is true.
/// "Config does not weaken defaults" — `safe_commands = []` must keep
/// the built-in defaults available to `is_safe_command`.
#[test]
fn toml_extras_classify_via_is_safe_command_and_empty_extras_keep_defaults() {
    use crate::supervisor::auto_approve::is_safe_command;

    // (1) Extras case: a project-specific entry parsed from TOML must
    //     classify a command using that prefix as safe.
    let tmp = TempDir::new().unwrap();
    let extras_path = tmp.path().join("extras.toml");
    write_file(
        &extras_path,
        "[supervisor]\n\
             enabled = true\n\
             [supervisor.auto_approve]\n\
             safe_commands = [\"just smoke\"]\n",
    );
    let extras_config = load_config_file(&extras_path).unwrap().unwrap();
    let extras_supervisor = extras_config.supervisor.unwrap();
    let extras_aa = extras_supervisor.auto_approve.unwrap();
    let extras_whitelist = extras_aa.effective_whitelist(&extras_supervisor.common_dev_allowlist);
    assert!(
        is_safe_command("just smoke -v", &extras_whitelist),
        "TOML extra `just smoke` must accept `just smoke -v`"
    );
    // The defaults must still be present alongside the extra.
    assert!(
        is_safe_command("grep -rn \"foo\" src/", &extras_whitelist),
        "extras must not displace built-in defaults"
    );

    // (2) Empty extras: the effective whitelist must still classify the
    //     composed defaults (e.g. `grep`) as safe.
    let empty_path = tmp.path().join("empty.toml");
    write_file(
        &empty_path,
        "[supervisor]\n\
             enabled = true\n\
             [supervisor.auto_approve]\n\
             safe_commands = []\n",
    );
    let empty_config = load_config_file(&empty_path).unwrap().unwrap();
    let empty_supervisor = empty_config.supervisor.unwrap();
    let empty_aa = empty_supervisor.auto_approve.unwrap();
    let empty_whitelist = empty_aa.effective_whitelist(&empty_supervisor.common_dev_allowlist);
    assert!(
        is_safe_command("grep -rn \"foo\" src/", &empty_whitelist),
        "empty safe_commands must keep built-in defaults"
    );
    assert!(
        is_safe_command("git commit -m hi", &empty_whitelist),
        "empty safe_commands must keep `git commit` default"
    );
    // A command outside the defaults must still be rejected.
    assert!(
        !is_safe_command("rm -rf /tmp/foo", &empty_whitelist),
        "empty safe_commands must not whitelist arbitrary commands"
    );
}

// --- ConflictConfig (supervisor.conflict sub-table) ---

#[test]
fn supervisor_with_no_conflict_section_loads_defaults() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[supervisor]\nenabled = true\n");
    let supervisor = load_config_file(&path)
        .unwrap()
        .unwrap()
        .supervisor
        .unwrap();
    assert_eq!(supervisor.conflict.window_seconds, 120);
    assert!(supervisor.conflict.warn_on_intent_overlap);
    assert!(supervisor.conflict.escalate_on_violation);
}

#[test]
fn conflict_section_with_all_fields_overrides_defaults() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\n\
             enabled = true\n\
             [supervisor.conflict]\n\
             window_seconds = 300\n\
             warn_on_intent_overlap = false\n\
             escalate_on_violation = false\n",
    );
    let conflict = load_config_file(&path)
        .unwrap()
        .unwrap()
        .supervisor
        .unwrap()
        .conflict;
    assert_eq!(conflict.window_seconds, 300);
    assert!(!conflict.warn_on_intent_overlap);
    assert!(!conflict.escalate_on_violation);
}

#[test]
fn conflict_section_with_partial_fields_keeps_other_defaults() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[supervisor]\n[supervisor.conflict]\nwindow_seconds = 60\n",
    );
    let conflict = load_config_file(&path)
        .unwrap()
        .unwrap()
        .supervisor
        .unwrap()
        .conflict;
    assert_eq!(conflict.window_seconds, 60);
    assert!(conflict.warn_on_intent_overlap);
    assert!(conflict.escalate_on_violation);
}

#[test]
fn pre_v05_config_without_conflict_section_loads() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    // A v0.4-style config: supervisor enabled but no [supervisor.conflict].
    write_file(
        &path,
        "default_cli = \"claude\"\n\
             [supervisor]\n\
             enabled = true\n\
             agent_approval = \"auto\"\n",
    );
    let config = load_config_file(&path).unwrap().unwrap();
    let supervisor = config.supervisor.unwrap();
    assert!(supervisor.enabled);
    // The conflict sub-table defaults to ConflictConfig::default().
    assert_eq!(supervisor.conflict, ConflictConfig::default());
}

#[test]
fn conflict_config_round_trips_through_save_and_load() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");
    let original = PawConfig {
        supervisor: Some(SupervisorConfig {
            enabled: true,
            conflict: ConflictConfig {
                window_seconds: 90,
                warn_on_intent_overlap: false,
                escalate_on_violation: true,
            },
            ..Default::default()
        }),
        ..Default::default()
    };
    save_config_to(&config_path, &original).unwrap();
    let loaded = load_config_file(&config_path).unwrap().unwrap();
    assert_eq!(loaded.supervisor, original.supervisor);
}

// --- GovernanceConfig (governance-config v0.5.0) ---

/// Helper: lays out a repo with `.git-paw/config.toml` and an optional
/// `SpecKit` `memory/constitution.md` so the `load_config_from`
/// auto-wiring path can be exercised end-to-end.
fn write_repo_config(repo_root: &Path, toml: &str) {
    write_file(&repo_config_path(repo_root), toml);
}

fn missing_global(tmp: &TempDir) -> PathBuf {
    tmp.path().join("nonexistent-global").join("config.toml")
}

// 3.2 All paths populated.
#[test]
fn governance_all_paths_populated() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[governance]\n\
             adr = \"docs/adr\"\n\
             test_strategy = \"docs/test-strategy.md\"\n\
             security = \"docs/security-checklist.md\"\n\
             dod = \"docs/definition-of-done.md\"\n\
             constitution = \".specify/memory/constitution.md\"\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    assert_eq!(
        config.governance.adr.as_deref(),
        Some(Path::new("docs/adr"))
    );
    assert_eq!(
        config.governance.test_strategy.as_deref(),
        Some(Path::new("docs/test-strategy.md"))
    );
    assert_eq!(
        config.governance.security.as_deref(),
        Some(Path::new("docs/security-checklist.md"))
    );
    assert_eq!(
        config.governance.dod.as_deref(),
        Some(Path::new("docs/definition-of-done.md"))
    );
    assert_eq!(
        config.governance.constitution.as_deref(),
        Some(Path::new(".specify/memory/constitution.md"))
    );
}

// 3.3 Partial paths.
#[test]
fn governance_partial_paths_only_some_fields_populated() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[governance]\n\
             dod = \"docs/dod.md\"\n\
             security = \"docs/security.md\"\n",
    );

    let config = load_config_file(&path).unwrap().unwrap();
    assert_eq!(
        config.governance.dod.as_deref(),
        Some(Path::new("docs/dod.md"))
    );
    assert_eq!(
        config.governance.security.as_deref(),
        Some(Path::new("docs/security.md"))
    );
    assert!(config.governance.adr.is_none());
    assert!(config.governance.test_strategy.is_none());
    assert!(config.governance.constitution.is_none());
}

// 3.4 Absolute path preserved as-is.
#[test]
fn governance_absolute_path_preserved_as_is() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[governance]\nadr = \"/absolute/path/to/adr\"\n");

    let config = load_config_file(&path).unwrap().unwrap();
    assert_eq!(
        config.governance.adr,
        Some(PathBuf::from("/absolute/path/to/adr"))
    );
}

// 3.5 Non-existent path loads cleanly without error.
#[test]
fn governance_nonexistent_path_loads_cleanly() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[governance]\ndod = \"docs/never-existed.md\"\n");

    let config = load_config_file(&path).unwrap().unwrap();
    assert_eq!(
        config.governance.dod,
        Some(PathBuf::from("docs/never-existed.md"))
    );
}

// 3.6 Round-trip via save → load.
#[test]
fn governance_round_trips_through_save_and_load() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");

    let original = PawConfig {
        governance: GovernanceConfig {
            adr: Some(PathBuf::from("docs/adr")),
            test_strategy: Some(PathBuf::from("docs/test-strategy.md")),
            security: Some(PathBuf::from("docs/security.md")),
            dod: Some(PathBuf::from("docs/dod.md")),
            constitution: Some(PathBuf::from(".specify/memory/constitution.md")),
            readme: Some(PathBuf::from("README.md")),
            docs: Some(PathBuf::from("docs/src")),
        },
        ..Default::default()
    };

    save_config_to(&config_path, &original).unwrap();
    let loaded = load_config_file(&config_path).unwrap().unwrap();
    assert_eq!(loaded.governance, original.governance);
}

// 3.8 GovernanceConfig::default() exposes only the documented path fields
// (no `gates` field) — compile-time-style assertion via destructuring.
#[test]
fn governance_default_has_only_path_fields() {
    // If a future change adds a `gates` (or any other) field, this
    // destructure stops compiling, forcing the change author to
    // revisit the capability boundary explicitly.
    let GovernanceConfig {
        adr,
        test_strategy,
        security,
        dod,
        constitution,
        readme,
        docs,
    } = GovernanceConfig::default();
    assert!(adr.is_none());
    assert!(test_strategy.is_none());
    assert!(security.is_none());
    assert!(dod.is_none());
    assert!(constitution.is_none());
    assert!(readme.is_none());
    assert!(docs.is_none());
}

// governance-config delta: readme + docs parse from [governance].
#[test]
fn governance_parses_readme_and_docs_fields() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(
        &path,
        "[governance]\n\
             readme = \"README.md\"\n\
             docs = \"docs/src\"\n",
    );
    let config = load_config_file(&path).unwrap().unwrap();
    assert_eq!(config.governance.readme, Some(PathBuf::from("README.md")));
    assert_eq!(config.governance.docs, Some(PathBuf::from("docs/src")));
}

// governance-config delta: readme + docs default to None when omitted.
#[test]
fn governance_readme_and_docs_default_to_none_when_omitted() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    write_file(&path, "[governance]\ndod = \"docs/dod.md\"\n");
    let config = load_config_file(&path).unwrap().unwrap();
    assert!(config.governance.readme.is_none());
    assert!(config.governance.docs.is_none());
    assert_eq!(config.governance.dod, Some(PathBuf::from("docs/dod.md")));
}

// governance-config delta: readme + docs survive round-trip serialization.
#[test]
fn governance_readme_and_docs_round_trip() {
    let original = GovernanceConfig {
        readme: Some(PathBuf::from("README.md")),
        docs: Some(PathBuf::from("docs/src")),
        ..Default::default()
    };
    let toml_str = toml::to_string(&original).unwrap();
    let reparsed: GovernanceConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(reparsed.readme, original.readme);
    assert_eq!(reparsed.docs, original.docs);
}

// 4.1 Auto-wires constitution when SpecKit detected + field unset.
#[test]
fn governance_auto_wires_constitution_when_speckit_detected() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("repo");
    let specify = repo_root.join(".specify");
    let specs = specify.join("specs");
    let memory = specify.join("memory");
    fs::create_dir_all(&specs).unwrap();
    fs::create_dir_all(&memory).unwrap();
    let constitution = memory.join("constitution.md");
    fs::write(&constitution, "# Constitution\n").unwrap();

    write_repo_config(
        &repo_root,
        "[specs]\n\
             type = \"speckit\"\n\
             dir = \".specify/specs\"\n",
    );

    let config = load_config_from(&missing_global(&tmp), &repo_root).unwrap();
    assert_eq!(
        config.governance.constitution.as_deref(),
        Some(constitution.as_path())
    );
}

// 4.2 Explicit governance.constitution preserved unchanged.
#[test]
fn governance_explicit_constitution_preserved_over_auto_wiring() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("repo");
    let specify = repo_root.join(".specify");
    let specs = specify.join("specs");
    let memory = specify.join("memory");
    fs::create_dir_all(&specs).unwrap();
    fs::create_dir_all(&memory).unwrap();
    fs::write(memory.join("constitution.md"), "# Constitution\n").unwrap();

    write_repo_config(
        &repo_root,
        "[specs]\n\
             type = \"speckit\"\n\
             dir = \".specify/specs\"\n\
             [governance]\n\
             constitution = \"docs/principles.md\"\n",
    );

    let config = load_config_from(&missing_global(&tmp), &repo_root).unwrap();
    assert_eq!(
        config.governance.constitution,
        Some(PathBuf::from("docs/principles.md"))
    );
}

// 4.3 Auto-wiring skipped for non-speckit backends.
#[test]
fn governance_auto_wiring_skipped_when_specs_type_is_openspec() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("repo");
    let specify = repo_root.join(".specify");
    let memory = specify.join("memory");
    fs::create_dir_all(&memory).unwrap();
    fs::write(memory.join("constitution.md"), "# Constitution\n").unwrap();
    fs::create_dir_all(repo_root.join("specs")).unwrap();

    write_repo_config(
        &repo_root,
        "[specs]\n\
             type = \"openspec\"\n\
             dir = \"specs\"\n",
    );

    let config = load_config_from(&missing_global(&tmp), &repo_root).unwrap();
    assert!(config.governance.constitution.is_none());
}

// 4.4 Auto-wiring skipped when [specs] is absent entirely.
#[test]
fn governance_auto_wiring_skipped_when_specs_section_absent() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("repo");
    let memory = repo_root.join(".specify").join("memory");
    fs::create_dir_all(&memory).unwrap();
    fs::write(memory.join("constitution.md"), "# Constitution\n").unwrap();
    fs::create_dir_all(repo_root.join(".git-paw")).unwrap();

    write_repo_config(&repo_root, "default_cli = \"claude\"\n");

    let config = load_config_from(&missing_global(&tmp), &repo_root).unwrap();
    assert!(config.governance.constitution.is_none());
}

// 4.5 SpecKit active but constitution.md absent → stays None, no error.
#[test]
fn governance_auto_wiring_skipped_when_constitution_md_absent() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("repo");
    let specs = repo_root.join(".specify").join("specs");
    fs::create_dir_all(&specs).unwrap();
    // No memory/constitution.md.

    write_repo_config(
        &repo_root,
        "[specs]\n\
             type = \"speckit\"\n\
             dir = \".specify/specs\"\n",
    );

    let config = load_config_from(&missing_global(&tmp), &repo_root).unwrap();
    assert!(config.governance.constitution.is_none());
}

// 4.6 Explicit empty-string constitution preserved as Some("").
#[test]
fn governance_explicit_empty_string_constitution_suppresses_auto_wiring() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("repo");
    let specify = repo_root.join(".specify");
    let specs = specify.join("specs");
    let memory = specify.join("memory");
    fs::create_dir_all(&specs).unwrap();
    fs::create_dir_all(&memory).unwrap();
    fs::write(memory.join("constitution.md"), "# Constitution\n").unwrap();

    write_repo_config(
        &repo_root,
        "[specs]\n\
             type = \"speckit\"\n\
             dir = \".specify/specs\"\n\
             [governance]\n\
             constitution = \"\"\n",
    );

    let config = load_config_from(&missing_global(&tmp), &repo_root).unwrap();
    assert_eq!(config.governance.constitution, Some(PathBuf::from("")));
}

// Merge: global and repo each contribute independent paths.
#[test]
fn governance_merge_fields_independently_across_global_and_repo() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("global").join("config.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    write_file(&global_path, "[governance]\nadr = \"docs/adr\"\n");
    write_file(
        &repo_config_path(&repo_root),
        "[governance]\ndod = \"docs/dod.md\"\n",
    );

    let config = load_config_from(&global_path, &repo_root).unwrap();
    assert_eq!(config.governance.adr, Some(PathBuf::from("docs/adr")));
    assert_eq!(config.governance.dod, Some(PathBuf::from("docs/dod.md")));
}

// Merge precedence: repo wins per-field when both set.
#[test]
fn governance_merge_repo_wins_per_field_when_both_set() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("global").join("config.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    write_file(&global_path, "[governance]\nadr = \"docs/global-adr\"\n");
    write_file(
        &repo_config_path(&repo_root),
        "[governance]\nadr = \"docs/repo-adr\"\n",
    );

    let config = load_config_from(&global_path, &repo_root).unwrap();
    assert_eq!(config.governance.adr, Some(PathBuf::from("docs/repo-adr")));
}

// load_repo_config also applies auto-wiring.
#[test]
fn governance_load_repo_config_also_auto_wires_constitution() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("repo");
    let specify = repo_root.join(".specify");
    let specs = specify.join("specs");
    let memory = specify.join("memory");
    fs::create_dir_all(&specs).unwrap();
    fs::create_dir_all(&memory).unwrap();
    let constitution = memory.join("constitution.md");
    fs::write(&constitution, "# Constitution\n").unwrap();

    write_repo_config(
        &repo_root,
        "[specs]\n\
             type = \"speckit\"\n\
             dir = \".specify/specs\"\n",
    );

    let config = load_repo_config(&repo_root).unwrap();
    assert_eq!(
        config.governance.constitution.as_deref(),
        Some(constitution.as_path())
    );
}

// --- load_config user_config_path override (config-test-isolation) ---

#[test]
fn load_config_with_some_pins_global_to_override_path() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    let global_a = tmp.path().join("global-A.toml");
    let global_b = tmp.path().join("global-B.toml");
    write_file(&global_a, "[clis.cli-A]\ncommand = \"/bin/a\"\n");
    write_file(&global_b, "[clis.cli-B]\ncommand = \"/bin/b\"\n");

    let config = load_config(&repo_root, Some(&global_a)).unwrap();
    assert!(config.clis.contains_key("cli-A"));
    assert!(!config.clis.contains_key("cli-B"));
}

#[test]
fn load_config_with_some_nonexistent_returns_defaults() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();
    let missing = tmp.path().join("does-not-exist.toml");

    let config = load_config(&repo_root, Some(&missing)).unwrap();
    assert_eq!(config, PawConfig::default());
}

// Note: a `load_config_with_none_reads_platform_default_global` test is
// intentionally omitted. Asserting that `None` resolves to
// `global_config_path()` would require either writing to the dev
// machine's real `~/Library/Application Support/git-paw/config.toml`
// (polluting it) or `serial_test` + env-var manipulation of `HOME` /
// `XDG_CONFIG_HOME` (brittle, slows the suite). The `None` branch is
// covered behaviourally by the 8 production call sites in `src/main.rs`
// and the v0.4 test suite that continues to pass.

#[test]
fn load_config_override_does_not_affect_repo_resolution() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();
    write_file(&repo_config_path(&repo_root), "default_cli = \"claude\"\n");

    let global_path = tmp.path().join("global.toml");
    write_file(&global_path, "default_cli = \"gemini\"\n");

    let config = load_config(&repo_root, Some(&global_path)).unwrap();
    assert_eq!(config.default_cli.as_deref(), Some("claude"));
}

// Maps to scenario "GovernanceConfig has no gates field" from
// governance-config. The struct does not enable `deny_unknown_fields`, so
// unknown sections deserialise silently; this test asserts the round-trip
// representation omits any `[governance.gates]` section and the loaded
// governance config keeps only the documented document-pointer fields.
// (test-coverage-v0-5-0 task 9.1)
#[test]
fn governance_config_rejects_gates_field() {
    let toml_input = "[governance]\ndod = \"docs/dod.md\"\n[governance.gates]\ndod = true\n";
    let cfg: PawConfig = toml::from_str(toml_input).expect("toml parse");
    let gov = cfg.governance;
    assert_eq!(gov.dod.as_deref(), Some(Path::new("docs/dod.md")));

    let round_trip = toml::to_string(&gov).expect("serialise gov");
    assert!(
        !round_trip.contains("gates"),
        "GovernanceConfig must not round-trip a `gates` field; got: {round_trip}"
    );
    assert!(
        !round_trip.contains("[governance.gates]"),
        "GovernanceConfig must not round-trip a `[governance.gates]` section; got: {round_trip}"
    );
}

// -----------------------------------------------------------------------
// supervisor-pane-affordances: `[layout].border_affordances` config field
// (spec requirement "border_affordances config field").
// -----------------------------------------------------------------------

/// Scenario: Default true applies all affordances — absent `[layout]`
/// section resolves to `true`.
#[test]
fn border_affordances_defaults_to_true_when_layout_absent() {
    let cfg: PawConfig = toml::from_str("default_cli = \"claude\"\n").expect("toml parse");
    assert!(
        cfg.layout.is_none(),
        "no [layout] section should parse as None"
    );
    assert!(
        cfg.border_affordances_enabled(),
        "border affordances default to on when [layout] is absent"
    );
}

/// Scenario: Default true — `[layout]` present but `border_affordances`
/// unset still resolves to `true`.
#[test]
fn border_affordances_defaults_to_true_when_field_unset() {
    let cfg: PawConfig = toml::from_str("[layout]\n").expect("toml parse");
    assert!(
        cfg.border_affordances_enabled(),
        "border affordances default to on when the field is unset"
    );
}

/// Scenario: Explicit false skips all affordances.
#[test]
fn border_affordances_explicit_false_resolves_off() {
    let cfg: PawConfig =
        toml::from_str("[layout]\nborder_affordances = false\n").expect("toml parse");
    assert_eq!(cfg.layout.as_ref().unwrap().border_affordances, Some(false));
    assert!(
        !cfg.border_affordances_enabled(),
        "explicit false must resolve to off"
    );
}

/// Scenario: Explicit true round-trips and resolves on.
#[test]
fn border_affordances_explicit_true_resolves_on() {
    let cfg: PawConfig =
        toml::from_str("[layout]\nborder_affordances = true\n").expect("toml parse");
    assert!(cfg.border_affordances_enabled());
}

/// `merged_with`: an overlay `[layout]` wins over the base layout.
#[test]
fn layout_overlay_wins_in_merge() {
    let base: PawConfig = toml::from_str("[layout]\nborder_affordances = true\n").expect("base");
    let overlay: PawConfig =
        toml::from_str("[layout]\nborder_affordances = false\n").expect("overlay");
    let merged = base.merged_with(&overlay);
    assert!(
        !merged.border_affordances_enabled(),
        "overlay [layout] must win in the merge"
    );
}

/// `merged_with`: an absent overlay `[layout]` preserves the base layout.
#[test]
fn layout_base_preserved_when_overlay_absent() {
    let base: PawConfig = toml::from_str("[layout]\nborder_affordances = false\n").expect("base");
    let overlay: PawConfig = toml::from_str("default_cli = \"claude\"\n").expect("overlay");
    let merged = base.merged_with(&overlay);
    assert!(
        !merged.border_affordances_enabled(),
        "base [layout] must survive when the overlay has none"
    );
}

// --- opsx role-gating config (opsx-role-gating 1.4) ---

#[test]
fn role_gating_section_present_but_field_absent_resolves_warn() {
    let config: PawConfig = toml::from_str("[opsx]\n").expect("parses");
    assert_eq!(config.role_gating_mode(), RoleGatingMode::Warn);
}

#[test]
fn role_gating_parses_each_variant() {
    // One row per RoleGatingMode wire value -> resolved mode.
    for (value, expected) in [
        ("warn", RoleGatingMode::Warn),
        ("block", RoleGatingMode::Block),
        ("off", RoleGatingMode::Off),
    ] {
        let config: PawConfig = toml::from_str(&format!("[opsx]\nrole_gating = \"{value}\"\n"))
            .unwrap_or_else(|e| panic!("role_gating = {value:?} must parse: {e}"));
        assert_eq!(
            config.role_gating_mode(),
            expected,
            "role_gating = {value:?}"
        );
    }
}

#[test]
fn role_gating_invalid_value_is_a_parse_error() {
    let err = toml::from_str::<PawConfig>("[opsx]\nrole_gating = \"loud\"\n").unwrap_err();
    assert!(
        err.to_string().contains("role_gating") || err.to_string().contains("variant"),
        "got: {err}"
    );
}

#[test]
fn role_gating_mode_round_trips_through_toml() {
    let config = PawConfig {
        opsx: Some(OpsxConfig {
            role_gating: Some(RoleGatingMode::Block),
        }),
        ..Default::default()
    };
    let serialized = toml::to_string(&config).expect("serializes");
    assert!(
        serialized.contains("role_gating = \"block\""),
        "got: {serialized}"
    );
    let reparsed: PawConfig = toml::from_str(&serialized).expect("re-parses");
    assert_eq!(reparsed.role_gating_mode(), RoleGatingMode::Block);
}

#[test]
fn opsx_section_merges_with_overlay_winning() {
    let base: PawConfig = toml::from_str("[opsx]\nrole_gating = \"warn\"\n").expect("base parses");
    let overlay: PawConfig =
        toml::from_str("[opsx]\nrole_gating = \"block\"\n").expect("overlay parses");
    let merged = base.merged_with(&overlay);
    assert_eq!(merged.role_gating_mode(), RoleGatingMode::Block);
}

#[test]
fn opsx_section_base_preserved_when_overlay_absent() {
    let base: PawConfig = toml::from_str("[opsx]\nrole_gating = \"off\"\n").expect("base parses");
    let overlay: PawConfig = toml::from_str("default_cli = \"claude\"\n").expect("overlay");
    let merged = base.merged_with(&overlay);
    assert_eq!(merged.role_gating_mode(), RoleGatingMode::Off);
}

#[test]
fn supervisor_auto_revert_defaults_false() {
    let config: PawConfig = toml::from_str("[supervisor]\nenabled = true\n").expect("parses");
    let sup = config.supervisor.expect("supervisor present");
    assert!(!sup.auto_revert(), "auto_revert defaults to false");
}

#[test]
fn supervisor_auto_revert_explicit_true() {
    let config: PawConfig =
        toml::from_str("[supervisor]\nenabled = true\nauto_revert = true\n").expect("parses");
    let sup = config.supervisor.expect("supervisor present");
    assert!(sup.auto_revert());
}

// --- [supervisor.tell] (supervisor-tell change) ---

#[test]
fn tell_config_defaults_when_table_absent() {
    // A v0.5.0 `[supervisor]` with no `[supervisor.tell]` table loads the
    // documented defaults: feedback mode, 60s inventory max age.
    let config: PawConfig = toml::from_str("[supervisor]\nenabled = true\n").expect("parses");
    let sup = config.supervisor.expect("supervisor present");
    assert_eq!(sup.tell.mode, TellMode::Feedback);
    assert_eq!(sup.tell.inventory_max_age_seconds, 60);
    assert!(sup.tell.is_default());
}

#[test]
fn tell_config_parses_each_mode() {
    // One row per TellMode wire value -> (mode, inventory max age,
    // is_default). An explicit `feedback` with no other keys still resolves
    // the default values; `send-keys` with a custom age is a non-default
    // table.
    for (fixture, expected_mode, expected_max_age, expected_is_default) in [
        (
            "[supervisor]\nenabled = true\n[supervisor.tell]\nmode = \"feedback\"\n",
            TellMode::Feedback,
            60,
            true,
        ),
        (
            "[supervisor]\nenabled = true\n[supervisor.tell]\nmode = \"send-keys\"\ninventory_max_age_seconds = 15\n",
            TellMode::SendKeys,
            15,
            false,
        ),
    ] {
        let config: PawConfig =
            toml::from_str(fixture).unwrap_or_else(|e| panic!("{fixture:?} must parse: {e}"));
        let tell = config.supervisor.expect("supervisor present").tell;
        assert_eq!(tell.mode, expected_mode, "mode for {fixture:?}");
        assert_eq!(
            tell.inventory_max_age_seconds, expected_max_age,
            "inventory_max_age_seconds for {fixture:?}"
        );
        assert_eq!(
            tell.is_default(),
            expected_is_default,
            "is_default for {fixture:?}"
        );
    }
}

#[test]
fn tell_config_rejects_unknown_mode() {
    let err = toml::from_str::<PawConfig>(
        "[supervisor]\nenabled = true\n[supervisor.tell]\nmode = \"shout\"\n",
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("shout") || err.to_string().contains("mode"),
        "unknown mode should be a parse error; got {err}"
    );
}

#[test]
fn tell_config_all_default_table_round_trips_without_emitting_tell() {
    // An all-default tell table is skipped on serialize so v0.5.0 configs
    // stay byte-stable.
    let sup = SupervisorConfig {
        enabled: true,
        ..SupervisorConfig::default()
    };
    let config = PawConfig {
        supervisor: Some(sup),
        ..PawConfig::default()
    };
    let serialized = toml::to_string_pretty(&config).expect("serializes");
    assert!(
        !serialized.contains("[supervisor.tell]"),
        "all-default tell table must be omitted; got:\n{serialized}"
    );
    let reparsed: PawConfig = toml::from_str(&serialized).expect("re-parses");
    assert_eq!(config, reparsed);
}

// --- [mcp] configuration section (mcp-server-identity) ---

// configuration delta — Scenario: Config with [mcp] name parses the field.
#[test]
fn mcp_name_parses_to_some() {
    let config: PawConfig = toml::from_str("[mcp]\nname = \"my-project\"\n").expect("parses");
    assert_eq!(config.mcp.name, Some("my-project".to_string()));
    assert_eq!(config.mcp_server_name(), "my-project");
}

// Backward compatibility: a representative pre-v0.7.0 config (no [mcp]
// section) still parses unchanged.
#[test]
fn pre_existing_config_without_mcp_loads() {
    let prior = "default_cli = \"claude\"\nmouse = true\n\n[broker]\nenabled = true\nport = 9119\n\n[supervisor]\nenabled = true\n";
    let config: PawConfig = toml::from_str(prior).expect("prior config must still parse");
    assert_eq!(config.mcp, McpConfig::default());
}

// configuration delta — Scenario: MCP config survives round-trip
// serialization.
#[test]
fn mcp_config_round_trips_through_toml() {
    let config = PawConfig {
        mcp: McpConfig {
            name: Some("my-project".to_string()),
        },
        ..PawConfig::default()
    };
    let serialized = toml::to_string(&config).expect("serializes");
    let reparsed: PawConfig = toml::from_str(&serialized).expect("re-parses");
    assert_eq!(reparsed.mcp, config.mcp);
}

// An all-default [mcp] table (name = None) is omitted on serialize so
// pre-existing configs stay byte-stable.
#[test]
fn mcp_default_omits_name_on_serialize() {
    let config = PawConfig::default();
    let serialized = toml::to_string_pretty(&config).expect("serializes");
    assert!(
        !serialized.contains("name ="),
        "default [mcp] must not emit a name; got:\n{serialized}"
    );
    let reparsed: PawConfig = toml::from_str(&serialized).expect("re-parses");
    assert_eq!(config, reparsed);
}

// merged_with: a repo-level [mcp].name wins over the global one.
#[test]
fn mcp_overlay_name_wins_in_merge() {
    let base: PawConfig = toml::from_str("[mcp]\nname = \"global-name\"\n").expect("base");
    let overlay: PawConfig = toml::from_str("[mcp]\nname = \"repo-name\"\n").expect("overlay");
    let merged = base.merged_with(&overlay);
    assert_eq!(merged.mcp.name, Some("repo-name".to_string()));
}

// merged_with: an absent overlay [mcp].name preserves the base name.
#[test]
fn mcp_base_name_preserved_when_overlay_absent() {
    let base: PawConfig = toml::from_str("[mcp]\nname = \"global-name\"\n").expect("base");
    let overlay: PawConfig = toml::from_str("default_cli = \"claude\"\n").expect("overlay");
    let merged = base.merged_with(&overlay);
    assert_eq!(merged.mcp.name, Some("global-name".to_string()));
}

// --- worktree_placement (worktree-embedded-placement) ---

#[test]
fn worktree_placement_parses_and_resolves() {
    // One row per wire value plus the absent case: the parsed
    // `Option<WorktreePlacement>` field and the resolved placement.
    for (fixture, expected_field, expected_resolved) in [
        (
            "worktree_placement = \"child\"\n",
            Some(WorktreePlacement::Child),
            WorktreePlacement::Child,
        ),
        (
            "worktree_placement = \"sibling\"\n",
            Some(WorktreePlacement::Sibling),
            WorktreePlacement::Sibling,
        ),
        (
            "default_cli = \"claude\"\n",
            None,
            WorktreePlacement::Sibling,
        ),
    ] {
        let cfg: PawConfig =
            toml::from_str(fixture).unwrap_or_else(|e| panic!("{fixture:?} must parse: {e}"));
        assert_eq!(
            cfg.worktree_placement, expected_field,
            "field for {fixture:?}"
        );
        assert_eq!(
            cfg.worktree_placement(),
            expected_resolved,
            "resolved for {fixture:?}"
        );
    }
}

#[test]
fn worktree_placement_repo_overrides_global() {
    let tmp = TempDir::new().unwrap();
    let global_path = tmp.path().join("global").join("config.toml");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    write_file(&global_path, "worktree_placement = \"sibling\"\n");
    write_file(
        &repo_config_path(&repo_root),
        "worktree_placement = \"child\"\n",
    );

    let config = load_config_from(&global_path, &repo_root).unwrap();
    assert_eq!(config.worktree_placement(), WorktreePlacement::Child);
}

#[test]
fn worktree_placement_survives_round_trip() {
    let cfg = PawConfig {
        worktree_placement: Some(WorktreePlacement::Child),
        ..PawConfig::default()
    };
    let serialized = toml::to_string_pretty(&cfg).expect("serialize");
    let reparsed: PawConfig = toml::from_str(&serialized).expect("reparse");
    assert_eq!(reparsed.worktree_placement(), WorktreePlacement::Child);
}

#[test]
fn worktree_placement_default_skipped_on_serialize() {
    // A default (absent) placement must not appear in serialized output so
    // pre-existing configs round-trip byte-stably.
    let cfg = PawConfig::default();
    let serialized = toml::to_string_pretty(&cfg).expect("serialize");
    assert!(
        !serialized.contains("worktree_placement"),
        "absent placement must not be serialized; got:\n{serialized}"
    );
}

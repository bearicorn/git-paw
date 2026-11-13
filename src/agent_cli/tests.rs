//! Behaviour of the provider families and the resolved CLI record.

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::*;
use crate::config::{CustomCli, claude_prompt_profile};

const LEVELS: [ApprovalLevel; 3] = [
    ApprovalLevel::Manual,
    ApprovalLevel::Auto,
    ApprovalLevel::FullAuto,
];

fn entry(command: &str) -> CustomCli {
    CustomCli {
        command: command.to_string(),
        display_name: None,
        submit_delay_ms: None,
        settings_path: None,
        approval_args: HashMap::new(),
        prompt_profile: None,
        session_id_args: None,
        resume_args: None,
        mcp: None,
        headless_args: None,
        family: None,
        args: None,
        sandbox: None,
        sandbox_args: None,
        inherit_operator_config: false,
    }
}

fn clis(entries: &[(&str, CustomCli)]) -> HashMap<String, CustomCli> {
    entries
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

/// A temp dir holding fake executables named `names`.
fn bin_dir(names: &[&str]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    for name in names {
        write_exe(&dir.path().join(name));
    }
    let path = dir.path().to_path_buf();
    (dir, path)
}

fn write_exe(path: &Path) {
    fs::write(path, "#!/bin/sh\n").expect("write exe");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// The built-in approval table exactly as `config::approval_flags` encoded it
/// before the rows moved into the providers.
fn pre_provider_approval_flags(cli: &str, level: ApprovalLevel) -> &'static str {
    match (cli, level) {
        ("claude" | "agy", ApprovalLevel::FullAuto) => "--dangerously-skip-permissions",
        ("claude", ApprovalLevel::Auto) => "--permission-mode acceptEdits",
        ("codex", ApprovalLevel::FullAuto) => "--dangerously-bypass-approvals-and-sandbox",
        ("codex", ApprovalLevel::Auto) => "--sandbox workspace-write",
        ("qwen", ApprovalLevel::FullAuto) => "--yolo",
        _ => "",
    }
}

// --- Task 1.3: the providers return exactly what the old tables returned ---

#[test]
fn provider_rows_equal_the_pre_provider_tables_for_every_known_cli_and_level() {
    let none: HashMap<String, CustomCli> = HashMap::new();
    let names =
        ROSTER
            .iter()
            .map(|e| e.name)
            .chain(["gemini", "cursor", "some-agent", "my-agent", ""]);
    for name in names {
        let profile = CliProfile::resolve(name, &none);
        for level in LEVELS {
            assert_eq!(
                profile.approval_flags(&level),
                pre_provider_approval_flags(name, level),
                "approval flags for ({name:?}, {level:?})"
            );
            assert_eq!(
                crate::config::approval_flags(name, &level),
                pre_provider_approval_flags(name, level),
                "config::approval_flags({name:?}, {level:?})"
            );
        }
        let is_claude = name == "claude";
        assert_eq!(
            profile.session_id_args("ID"),
            is_claude.then(|| "--session-id ID".to_string()),
            "session-id template for {name:?}"
        );
        assert_eq!(
            profile.resume_args("ID"),
            is_claude.then(|| "--resume ID".to_string()),
            "resume template for {name:?}"
        );
        let headless = match name {
            "claude" => Some("-p".to_string()),
            "codex" => Some("exec".to_string()),
            _ => None,
        };
        assert_eq!(profile.headless_args(), headless, "headless for {name:?}");
        assert_eq!(
            profile.mcp_capable(),
            matches!(name, "claude" | "codex" | "cursor-agent" | "agy"),
            "MCP capability for {name:?}"
        );
        assert_eq!(&profile.prompt_profile(), claude_prompt_profile());
    }
}

// --- Families ---

#[test]
fn family_names_round_trip_and_unknown_names_list_the_accepted_values() {
    for family in CliFamily::ALL {
        assert_eq!(family.as_str().parse::<CliFamily>().ok(), Some(family));
        assert_eq!(family.to_string(), family.as_str());
    }
    let err = "gpt".parse::<CliFamily>().expect_err("unknown family");
    let msg = err.to_string();
    for name in ["claude", "codex", "agy", "qwen", "cursor-agent", "generic"] {
        assert!(msg.contains(name), "{msg}");
    }
}

/// Scenario "Every family is constructible without PATH or config".
#[test]
fn every_family_record_is_constructible_without_path_or_config() {
    for family in CliFamily::ALL {
        let profile = CliProfile::for_family(family);
        let provider = family.provider();
        assert_eq!(profile.family(), family);
        assert_eq!(profile.id(), family.as_str());
        for level in LEVELS {
            assert_eq!(
                profile.approval_flags(&level),
                provider.approval_flags(level)
            );
        }
        assert_eq!(
            profile.session_id_args("x"),
            provider
                .session_id_args()
                .map(|t| t.replace("{{SESSION_ID}}", "x"))
        );
        assert_eq!(
            profile.resume_args("x"),
            provider
                .resume_args()
                .map(|t| t.replace("{{SESSION_ID}}", "x"))
        );
        assert_eq!(profile.headless_args().as_deref(), provider.headless_args());
        assert_eq!(profile.mcp_capable(), provider.mcp_capable());
        assert_eq!(&profile.prompt_profile(), provider.prompt_profile());
        assert_eq!(profile.submit_delay_ms(), crate::DEFAULT_SUBMIT_DELAY_MS);
    }
}

// --- Record resolution ---

/// Scenario "A known CLI resolves its record".
#[test]
fn a_known_cli_resolves_its_record() {
    let (_dir, bin) = bin_dir(&["claude"]);
    let profile = CliProfile::resolve("claude", &HashMap::new());
    assert_eq!(profile.id(), "claude");
    assert_eq!(profile.family(), CliFamily::Claude);
    assert_eq!(
        profile.executable_in(Some(bin.as_os_str()), &std::env::temp_dir()),
        Some(bin.join("claude"))
    );
}

/// Scenario "A custom entry resolves its record".
#[test]
fn a_custom_entry_resolves_its_record() {
    let (_dir, bin) = bin_dir(&["my-agent"]);
    let command = bin.join("my-agent");
    let map = clis(&[("my-agent", entry(&command.to_string_lossy()))]);
    let profile = CliProfile::resolve("my-agent", &map);
    assert_eq!(profile.id(), "my-agent");
    assert_eq!(
        profile.executable_in(Some(&OsString::new()), &std::env::temp_dir()),
        Some(command)
    );
}

/// Scenario "The launch command's leading token is never the id".
#[test]
fn the_launch_commands_leading_token_is_never_the_id() {
    let (_dir, bin) = bin_dir(&["claude"]);
    let map = clis(&[("foo", entry("claude"))]);
    let profile = CliProfile::resolve("foo", &map);
    assert_eq!(profile.id(), "foo");
    assert_eq!(profile.family(), CliFamily::Generic);
    assert_eq!(
        profile.executable_in(Some(bin.as_os_str()), &std::env::temp_dir()),
        Some(bin.join("claude"))
    );
}

/// Scenario "Overrides resolve by id when the executable differs".
#[test]
fn overrides_resolve_by_id_when_the_executable_differs() {
    let mut fast = entry("/opt/tools/agent");
    fast.submit_delay_ms = Some(2500);
    fast.prompt_profile = Some(crate::config::CliPromptProfileOverride {
        readiness_markers: Some(vec!["fast-ready".to_string()]),
        ..Default::default()
    });
    let map = clis(&[("fast", fast)]);
    let profile = CliProfile::resolve("fast", &map);
    assert!(
        profile
            .launch_command(&ApprovalLevel::Manual, LaunchSession::None)
            .starts_with("/opt/tools/agent")
    );
    assert_eq!(profile.submit_delay_ms(), 2500);
    assert_eq!(
        profile.prompt_profile().readiness_markers,
        vec!["fast-ready"]
    );
    // The public resolvers read the same record.
    assert_eq!(crate::config::resolve_submit_delay_ms("fast", &map), 2500);
    assert_eq!(
        crate::config::resolve_prompt_profile("fast", &map).readiness_markers,
        vec!["fast-ready"]
    );
}

/// Scenario "An unknown CLI resolves the generic family".
#[test]
fn an_unknown_cli_resolves_the_generic_family() {
    let map = clis(&[("my-agent", entry("my-agent"))]);
    let profile = CliProfile::resolve("my-agent", &map);
    assert_eq!(profile.family(), CliFamily::Generic);
    for level in LEVELS {
        assert_eq!(profile.approval_flags(&level), "");
    }
    assert!(!profile.mcp_capable());
}

/// Scenario "A declared variant inherits its family".
#[test]
fn a_declared_variant_inherits_its_family() {
    let mut oss = entry("claude-oss");
    oss.family = Some("claude".to_string());
    let profile = CliProfile::resolve("claude-oss", &clis(&[("claude-oss", oss)]));
    assert_eq!(profile.family(), CliFamily::Claude);
}

/// Scenario "An undeclared custom entry is unchanged".
#[test]
fn an_undeclared_custom_entry_resolves_generic() {
    let map = clis(&[("claude-oss", entry("claude-oss"))]);
    assert_eq!(
        CliProfile::resolve("claude-oss", &map).family(),
        CliFamily::Generic
    );
}

/// Scenario "The command basename is not used".
#[test]
fn the_command_basename_is_not_used_to_infer_the_family() {
    let map = clis(&[("foo", entry("/usr/local/bin/claude"))]);
    assert_eq!(
        CliProfile::resolve("foo", &map).family(),
        CliFamily::Generic
    );
}

#[test]
fn a_known_name_entry_without_family_keeps_its_own_family() {
    let map = clis(&[("codex", entry("/opt/alt/codex"))]);
    assert_eq!(
        CliProfile::resolve("codex", &map).family(),
        CliFamily::Codex
    );
}

/// `supervisor-config` "A declared family supplies the built-in row" and
/// `cli-provider` "A declared variant resolves its family's rows".
#[test]
fn a_declared_variant_resolves_its_family_rows() {
    let mut oss = entry("claude");
    oss.family = Some("claude".to_string());
    let map = clis(&[("claude-oss", oss)]);
    let profile = CliProfile::resolve("claude-oss", &map);
    assert_eq!(
        profile.approval_flags(&ApprovalLevel::FullAuto),
        "--dangerously-skip-permissions"
    );
    assert_eq!(
        profile.approval_flags(&ApprovalLevel::Auto),
        "--permission-mode acceptEdits"
    );
    assert_eq!(
        profile.session_id_args("<id>").as_deref(),
        Some("--session-id <id>")
    );
    // `cli-resolution` "A declared Claude variant resolves the family's templates".
    assert_eq!(
        profile.resume_args("<id>").as_deref(),
        Some("--resume <id>")
    );
    assert_eq!(
        crate::config::resolve_approval_flags("claude-oss", &ApprovalLevel::Auto, &map),
        "--permission-mode acceptEdits"
    );
}

/// `cli-provider` "An undeclared variant resolves none".
#[test]
fn an_undeclared_variant_resolves_no_rows() {
    let map = clis(&[("claude-oss", entry("claude"))]);
    let profile = CliProfile::resolve("claude-oss", &map);
    assert_eq!(profile.approval_flags(&ApprovalLevel::FullAuto), "");
    assert_eq!(profile.session_id_args("x"), None);
    assert_eq!(profile.resume_args("x"), None);
}

#[test]
fn an_override_wins_over_the_family_row() {
    let mut oss = entry("claude");
    oss.family = Some("claude".to_string());
    oss.approval_args = HashMap::from([("full-auto".to_string(), "--mine".to_string())]);
    oss.resume_args = Some("--continue-session {{SESSION_ID}}".to_string());
    let profile = CliProfile::resolve("claude-oss", &clis(&[("claude-oss", oss)]));
    assert_eq!(profile.approval_flags(&ApprovalLevel::FullAuto), "--mine");
    assert_eq!(
        profile.resume_args("x").as_deref(),
        Some("--continue-session x")
    );
}

/// `boot-block` "A declared family supplies the default" and "An explicit
/// `mcp` wins over a declared family"; `cli-provider` "An explicit mcp key
/// overrides the family default".
#[test]
fn mcp_capability_is_the_explicit_key_else_the_family_default() {
    let mut declared = entry("claude");
    declared.family = Some("claude".to_string());
    let mut declared_off = declared.clone();
    declared_off.mcp = Some(false);
    let mut undeclared_on = entry("my-agent");
    undeclared_on.mcp = Some(true);
    let map = clis(&[
        ("declared", declared),
        ("declared-off", declared_off),
        ("undeclared", entry("claude")),
        ("undeclared-on", undeclared_on),
    ]);
    assert!(CliProfile::resolve("declared", &map).mcp_capable());
    assert!(!CliProfile::resolve("declared-off", &map).mcp_capable());
    assert!(!CliProfile::resolve("undeclared", &map).mcp_capable());
    assert!(CliProfile::resolve("undeclared-on", &map).mcp_capable());
}

#[test]
fn display_name_prefers_entry_then_product_name_then_capitalised_id() {
    let mut named = entry("x");
    named.display_name = Some("Named".to_string());
    let map = clis(&[("named", named)]);
    assert_eq!(CliProfile::resolve("named", &map).display_name(), "Named");
    assert_eq!(
        CliProfile::resolve("agy", &map).display_name(),
        "Antigravity"
    );
    assert_eq!(CliProfile::resolve("aider", &map).display_name(), "Aider");
}

// --- The launch-command builder (task 4.1) ---

#[test]
fn launch_command_orders_executable_args_flags_then_session_args() {
    let none: HashMap<String, CustomCli> = HashMap::new();
    let mut sonnet = entry("claude");
    sonnet.family = Some("claude".to_string());
    sonnet.args = Some("--model sonnet".to_string());
    let mut generic_args = entry("/opt/my agent");
    generic_args.args = Some("--fast".to_string());
    let mut blank_args = entry("my-agent");
    blank_args.args = Some("  ".to_string());
    let map = clis(&[
        ("sonnet", sonnet),
        ("spaced", generic_args),
        ("blank", blank_args),
        ("my-agent", entry("my-agent")),
    ]);
    let cases: &[(&str, ApprovalLevel, LaunchSession<'_>, &str)] = &[
        // Built-in CLI with no entry: launched by name.
        (
            "claude",
            ApprovalLevel::Manual,
            LaunchSession::None,
            "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --setting-sources project,local",
        ),
        (
            "claude",
            ApprovalLevel::FullAuto,
            LaunchSession::Assign("u1"),
            "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --setting-sources project,local --dangerously-skip-permissions --session-id u1",
        ),
        (
            "claude",
            ApprovalLevel::Auto,
            LaunchSession::Resume("u1"),
            "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --setting-sources project,local --permission-mode acceptEdits --resume u1",
        ),
        (
            "codex",
            ApprovalLevel::Auto,
            LaunchSession::Assign("u1"),
            "codex --sandbox workspace-write",
        ),
        // `args` sit between the executable and the approval flags.
        (
            "sonnet",
            ApprovalLevel::Manual,
            LaunchSession::None,
            "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --model sonnet --setting-sources project,local",
        ),
        (
            "sonnet",
            ApprovalLevel::FullAuto,
            LaunchSession::Resume("u2"),
            "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --model sonnet --setting-sources project,local --dangerously-skip-permissions --resume u2",
        ),
        // A configured command is one shell word.
        (
            "spaced",
            ApprovalLevel::FullAuto,
            LaunchSession::None,
            "'/opt/my agent' --fast",
        ),
        // Empty parts leave no separators.
        (
            "blank",
            ApprovalLevel::FullAuto,
            LaunchSession::Assign("u3"),
            "my-agent",
        ),
        (
            "my-agent",
            ApprovalLevel::FullAuto,
            LaunchSession::Resume("u3"),
            "my-agent",
        ),
        // A pass-through identity keeps its trailing text.
        (
            "codex --flag",
            ApprovalLevel::Manual,
            LaunchSession::None,
            "codex --flag",
        ),
    ];
    for (identity, level, session, expected) in cases {
        let map = if map.contains_key(identity.split_whitespace().next().unwrap_or_default()) {
            &map
        } else {
            &none
        };
        assert_eq!(
            CliProfile::resolve(identity, map).launch_command(level, *session),
            *expected,
            "({identity:?}, {level:?}, {session:?})"
        );
    }
}

/// Scenario "Empty parts leave no separators".
#[test]
fn a_generic_cli_without_args_at_full_auto_is_exactly_the_quoted_executable() {
    let map = clis(&[("my-agent", entry("/opt/with space/agent"))]);
    assert_eq!(
        CliProfile::resolve("my-agent", &map)
            .launch_command(&ApprovalLevel::FullAuto, LaunchSession::None),
        "'/opt/with space/agent'"
    );
}

/// Scenarios "Arguments are appended" and "Two profiles of one executable".
#[test]
fn two_profiles_of_one_executable_keep_their_own_args() {
    let mut sonnet = entry("claude");
    sonnet.family = Some("claude".to_string());
    sonnet.args = Some("--model sonnet".to_string());
    let mut opus = sonnet.clone();
    opus.args = Some("--model opus".to_string());
    let map = clis(&[("sonnet", sonnet), ("opus", opus)]);
    let level = ApprovalLevel::Manual;
    assert_eq!(
        CliProfile::resolve("sonnet", &map).launch_command(&level, LaunchSession::None),
        "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --model sonnet --setting-sources project,local"
    );
    assert_eq!(
        CliProfile::resolve("opus", &map).launch_command(&level, LaunchSession::None),
        "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --model opus --setting-sources project,local"
    );
}

/// Scenario "An identity with trailing flags keeps its base CLI's facets".
#[test]
fn an_identity_with_trailing_flags_keeps_its_base_cli_facets() {
    let profile = CliProfile::resolve("claude --model x", &HashMap::new());
    assert_eq!(profile.id(), "claude");
    assert_eq!(profile.family(), CliFamily::Claude);
    assert_eq!(
        profile.launch_command(&ApprovalLevel::FullAuto, LaunchSession::Assign("abc")),
        "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --model x --setting-sources project,local --dangerously-skip-permissions --session-id abc"
    );
}

/// `cli-provider` "Recovery builds the full command".
#[test]
fn recovery_carries_approval_flags_then_resume_args() {
    let cmd = CliProfile::resolve("claude", &HashMap::new())
        .launch_command(&ApprovalLevel::Auto, LaunchSession::Resume("abc"));
    let sources = cmd
        .find("--setting-sources project,local")
        .expect("sources");
    let flags = cmd.find("--permission-mode acceptEdits").expect("flags");
    let resume = cmd.find("--resume abc").expect("resume");
    assert!(sources < flags && flags < resume, "{cmd}");
}

/// Scenario "The claude family declares its isolation".
#[test]
fn the_claude_family_declares_its_isolation() {
    let isolation = CliFamily::Claude.provider().operator_isolation();
    assert_eq!(isolation.args, "--setting-sources project,local");
    assert_eq!(
        isolation.env,
        &[
            ("CLAUDE_CODE_DISABLE_AUTO_MEMORY", "1"),
            ("CLAUDE_CODE_PLUGIN_DIRS", ""),
            ("CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION", "false"),
        ]
    );
}

/// Scenario "A family without isolation facts contributes nothing".
#[test]
fn a_family_without_isolation_facts_contributes_nothing() {
    for family in CliFamily::ALL {
        if family != CliFamily::Claude {
            assert_eq!(
                family.provider().operator_isolation(),
                OperatorIsolation::NONE,
                "{family}"
            );
        }
    }
    let mut agent = entry("/opt/agent");
    agent.args = Some("--quiet".to_string());
    let map = clis(&[("agent", agent)]);
    assert_eq!(
        CliProfile::resolve("agent", &map)
            .launch_command(&ApprovalLevel::Manual, LaunchSession::None),
        "/opt/agent --quiet"
    );
}

/// Scenario "A claude worker is launched isolated".
#[test]
fn a_claude_worker_is_launched_isolated() {
    let mut opus = entry("claude");
    opus.family = Some("claude".to_string());
    opus.args = Some("--model opus".to_string());
    let map = clis(&[("opus", opus)]);
    assert_eq!(
        CliProfile::resolve("opus", &map)
            .launch_command(&ApprovalLevel::Auto, LaunchSession::Assign("abc")),
        "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --model opus \
         --setting-sources project,local --permission-mode acceptEdits --session-id abc"
    );
}

/// Scenario "Recovery keeps the isolation".
#[test]
fn recovery_keeps_the_isolation() {
    let cmd = CliProfile::resolve("claude", &HashMap::new())
        .launch_command(&ApprovalLevel::Manual, LaunchSession::Resume("u9"));
    assert!(
        cmd.starts_with("env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude"),
        "{cmd}"
    );
    let sources = cmd
        .find("--setting-sources project,local")
        .expect("sources");
    let resume = cmd.find("--resume u9").expect("resume");
    assert!(sources < resume, "{cmd}");
}

/// Scenario "Opting out restores the previous launch".
#[test]
fn opting_out_restores_the_previous_launch() {
    let mut claude = entry("claude");
    claude.inherit_operator_config = true;
    let map = clis(&[("claude", claude)]);
    let cmd = CliProfile::resolve("claude", &map)
        .launch_command(&ApprovalLevel::Manual, LaunchSession::None);
    assert_eq!(cmd, "claude");
    for absent in [
        "--setting-sources",
        "CLAUDE_CODE_DISABLE_AUTO_MEMORY",
        "CLAUDE_CODE_PLUGIN_DIRS",
        "CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION",
    ] {
        assert!(!cmd.contains(absent), "{cmd}");
    }
}

/// Scenario "An isolated claude worker gets no prompt suggestions".
#[test]
fn an_isolated_claude_worker_gets_no_prompt_suggestions() {
    let mut sonnet = entry("claude");
    sonnet.family = Some("claude".to_string());
    sonnet.args = Some("--model sonnet".to_string());
    let map = clis(&[("sonnet", sonnet)]);
    for session in [
        LaunchSession::None,
        LaunchSession::Assign("abc"),
        LaunchSession::Resume("abc"),
    ] {
        let cmd = CliProfile::resolve("sonnet", &map).launch_command(&ApprovalLevel::Auto, session);
        let (env_prefix, _) = cmd.split_once(" claude ").expect("env prefix");
        assert!(
            env_prefix.starts_with("env ")
                && env_prefix
                    .split(' ')
                    .any(|a| a == "CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false"),
            "{cmd}"
        );
    }
}

/// Scenario "An opted-out claude worker keeps the operator's setting".
#[test]
fn an_opted_out_claude_worker_keeps_the_operators_setting() {
    let mut claude = entry("claude");
    claude.inherit_operator_config = true;
    let map = clis(&[("claude", claude)]);
    for approval in [ApprovalLevel::Manual, ApprovalLevel::Auto] {
        let cmd = CliProfile::resolve("claude", &map)
            .launch_command(&approval, LaunchSession::Assign("abc"));
        assert!(
            !cmd.contains("CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION"),
            "{cmd}"
        );
    }
}

// --- Sandbox declarations (worker-sandbox-mode) ---

fn sandboxed(command: &str, family: Option<&str>, mode: &str) -> CustomCli {
    let mut cli = entry(command);
    cli.family = family.map(str::to_string);
    cli.sandbox = Some(mode.to_string());
    cli
}

#[test]
fn only_codex_has_a_native_sandbox_and_sandbox_disabling_flags() {
    for family in CliFamily::ALL {
        let provider = family.provider();
        let is_codex = family == CliFamily::Codex;
        assert_eq!(
            provider.native_sandbox_args(),
            is_codex.then_some("--sandbox workspace-write"),
            "{family}"
        );
        let expected: &[&str] = if is_codex {
            &[
                "--dangerously-bypass-approvals-and-sandbox",
                "danger-full-access",
            ]
        } else {
            &[]
        };
        assert_eq!(provider.sandbox_disabling_flags(), expected, "{family}");
    }
}

/// Scenario "An override cannot re-add a sandbox-disabling flag".
#[test]
fn an_approval_args_override_carrying_a_disabling_flag_is_refused() {
    let mut codex = sandboxed("codex", None, "native");
    codex.approval_args = HashMap::from([(
        "auto".to_string(),
        "--dangerously-bypass-approvals-and-sandbox".to_string(),
    )]);
    let map = clis(&[("codex", codex)]);
    let msg = launch("codex", &map, &ApprovalLevel::Auto)
        .expect_err("a disabling flag is refused whatever its source")
        .to_string();
    for needle in [
        "codex",
        "auto",
        "--dangerously-bypass-approvals-and-sandbox",
        "[clis.codex].approval_args",
    ] {
        assert!(msg.contains(needle), "should name {needle}: {msg}");
    }
}

/// Scenario "Conflicting sandbox settings are refused".
#[test]
fn sandbox_args_conflicting_with_the_approval_flags_are_refused() {
    let mut codex = sandboxed("codex", None, "native");
    codex.sandbox_args = Some("--sandbox read-only".to_string());
    let map = clis(&[("codex", codex)]);
    let msg = launch("codex", &map, &ApprovalLevel::Auto)
        .expect_err("two --sandbox values must not both reach the CLI")
        .to_string();
    assert!(
        msg.contains("--sandbox") && msg.contains("sandbox_args"),
        "{msg}"
    );
    let manual = launch("codex", &map, &ApprovalLevel::Manual)
        .expect("no approval flag sets the sandbox at manual");
    assert_eq!(
        manual.launch_command(&ApprovalLevel::Manual, LaunchSession::None),
        "codex --sandbox read-only"
    );
}

/// Scenario "An unsandboxed full-auto launch warns".
#[test]
fn only_an_undeclared_cli_at_full_auto_gets_the_launch_warning() {
    let declared = clis(&[("sbx", sandboxed("/bin/sbx", Some("claude"), "wrapper"))]);
    let none = HashMap::new();
    let warning = CliProfile::resolve("claude", &none)
        .unsandboxed_full_auto_warning(&ApprovalLevel::FullAuto)
        .expect("undeclared full-auto warns");
    assert!(
        warning.contains("claude") && warning.contains("sandbox"),
        "{warning}"
    );
    assert_eq!(
        CliProfile::resolve("claude", &none).unsandboxed_full_auto_warning(&ApprovalLevel::Auto),
        None
    );
    assert_eq!(
        CliProfile::resolve("sbx", &declared)
            .unsandboxed_full_auto_warning(&ApprovalLevel::FullAuto),
        None
    );
}

/// Scenarios "An undeclared CLI launches unchanged", "Codex receives its native
/// sandbox", "The sandbox argument is not duplicated", "An override supplies
/// the arguments".
#[test]
fn native_sandbox_arguments_sit_after_args_and_before_approval_flags() {
    let mut codex = sandboxed("codex", None, "native");
    codex.args = Some("--model o4".to_string());
    let mut foo = sandboxed("foo", None, "native");
    foo.sandbox_args = Some("--confine .".to_string());
    let mut opus = sandboxed("claude", Some("claude"), "native");
    opus.args = Some("--model opus".to_string());
    opus.sandbox_args = Some("--confine .".to_string());
    let map = clis(&[("codex", codex), ("foo", foo), ("opus", opus)]);
    let none = HashMap::new();
    let cases: &[(&str, &HashMap<String, CustomCli>, ApprovalLevel, &str)] = &[
        (
            "codex",
            &map,
            ApprovalLevel::Manual,
            "codex --model o4 --sandbox workspace-write",
        ),
        (
            "codex",
            &map,
            ApprovalLevel::Auto,
            "codex --model o4 --sandbox workspace-write",
        ),
        ("foo", &map, ApprovalLevel::FullAuto, "foo --confine ."),
        // The operator-isolation arguments sit between `args` and the sandbox.
        (
            "opus",
            &map,
            ApprovalLevel::Auto,
            "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --model opus \
             --setting-sources project,local --confine . --permission-mode acceptEdits",
        ),
        (
            "codex",
            &none,
            ApprovalLevel::FullAuto,
            "codex --dangerously-bypass-approvals-and-sandbox",
        ),
        ("codex", &none, ApprovalLevel::Manual, "codex"),
    ];
    for (id, map, level, expected) in cases {
        let cmd = CliProfile::resolve(id, map).launch_command(level, LaunchSession::None);
        assert_eq!(cmd, *expected, "({id}, {level:?})");
        assert!(
            cmd.matches("--sandbox workspace-write").count() <= 1,
            "{cmd}"
        );
    }
}

/// Scenario "Codex full-auto under a native sandbox is refused".
#[test]
fn codex_full_auto_under_a_native_sandbox_is_refused() {
    let map = clis(&[("codex", sandboxed("codex", None, "native"))]);
    let err = launch("codex", &map, &ApprovalLevel::FullAuto)
        .expect_err("a sandbox-disabling row must be refused");
    let msg = err.to_string();
    for needle in [
        "codex",
        "full-auto",
        "--dangerously-bypass-approvals-and-sandbox",
        "[clis.codex].approval_args",
    ] {
        assert!(msg.contains(needle), "should name {needle}: {msg}");
    }
    for level in [ApprovalLevel::Manual, ApprovalLevel::Auto] {
        launch("codex", &map, &level).expect("lower levels launch");
    }
}

/// Scenario "An explicit override is honoured".
#[test]
fn an_approval_args_override_is_used_as_given_under_a_native_sandbox() {
    let mut codex = sandboxed("codex", None, "native");
    codex.approval_args =
        HashMap::from([("full-auto".to_string(), "--example-no-prompts".to_string())]);
    let map = clis(&[("codex", codex)]);
    let profile = launch("codex", &map, &ApprovalLevel::FullAuto)
        .expect("an explicit override is the user's choice");
    assert_eq!(
        profile.launch_command(&ApprovalLevel::FullAuto, LaunchSession::None),
        "codex --sandbox workspace-write --example-no-prompts"
    );
}

/// Scenarios "A wrapper declaration adds nothing to the command", "A
/// declaration does not raise the approval level", "A wrapper sandbox does
/// not restrict full-auto".
#[test]
fn a_wrapper_declaration_changes_neither_the_command_nor_the_level() {
    let declared = clis(&[(
        "sbx",
        sandboxed("/home/u/bin/claude-sandboxed", Some("claude"), "wrapper"),
    )]);
    let mut plain_entry = entry("/home/u/bin/claude-sandboxed");
    plain_entry.family = Some("claude".to_string());
    let plain = clis(&[("sbx", plain_entry)]);
    for level in LEVELS {
        let profile = launch("sbx", &declared, &level).expect("never refused");
        assert_eq!(
            profile.launch_command(&level, LaunchSession::None),
            CliProfile::resolve("sbx", &plain).launch_command(&level, LaunchSession::None),
            "{level:?}"
        );
    }
    let auto = CliProfile::resolve("sbx", &declared)
        .launch_command(&ApprovalLevel::Auto, LaunchSession::None);
    assert!(auto.ends_with("--permission-mode acceptEdits"), "{auto}");
    assert!(!auto.contains("--dangerously-skip-permissions"), "{auto}");
    let full = CliProfile::resolve("sbx", &declared)
        .launch_command(&ApprovalLevel::FullAuto, LaunchSession::None);
    assert!(full.contains("--dangerously-skip-permissions"), "{full}");
}

/// A native declaration never changes the approval level either.
#[test]
fn a_native_declaration_keeps_the_auto_level() {
    let map = clis(&[("codex", sandboxed("codex", None, "native"))]);
    assert_eq!(
        CliProfile::resolve("codex", &map).approval_flags(&ApprovalLevel::Auto),
        "--sandbox workspace-write"
    );
}

// --- The ambiguous-shim refusal (task 3.3) ---

/// Scenario "An ambiguous shim is refused".
#[test]
fn an_ambiguous_shim_is_refused_naming_both_paths() {
    let (_dir, bin) = bin_dir(&["claude", "claude-oss-sonnet"]);
    let map = clis(&[("claude-oss-sonnet", entry("claude"))]);
    let err = CliProfile::resolve("claude-oss-sonnet", &map)
        .ensure_unambiguous_in(Some(bin.as_os_str()), &std::env::temp_dir())
        .expect_err("ambiguous shim must be refused");
    let msg = err.to_string();
    assert!(
        msg.contains(&bin.join("claude").display().to_string()),
        "{msg}"
    );
    assert!(
        msg.contains(&bin.join("claude-oss-sonnet").display().to_string()),
        "{msg}"
    );
    assert!(msg.contains("command"), "names the fix: {msg}");
}

/// Scenario "A known name may point at another install".
#[test]
fn a_known_name_may_point_at_another_install() {
    let (_dir, bin) = bin_dir(&["claude"]);
    let (_alt_dir, alt) = bin_dir(&["claude"]);
    let alt_claude = alt.join("claude");
    let map = clis(&[("claude", entry(&alt_claude.to_string_lossy()))]);
    let profile = CliProfile::resolve("claude", &map);
    profile
        .ensure_unambiguous_in(Some(bin.as_os_str()), &std::env::temp_dir())
        .expect("a known name is an explicit override");
    assert!(
        profile
            .launch_command(&ApprovalLevel::Manual, LaunchSession::None)
            .starts_with(&format!(
                "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false {}",
                alt_claude.to_string_lossy()
            ))
    );
}

#[test]
fn a_shim_that_is_the_same_file_as_command_is_not_ambiguous() {
    let (_dir, bin) = bin_dir(&["agent"]);
    std::os::unix::fs::symlink(bin.join("agent"), bin.join("wrapped")).expect("symlink");
    let map = clis(&[("wrapped", entry(&bin.join("agent").to_string_lossy()))]);
    CliProfile::resolve("wrapped", &map)
        .ensure_unambiguous_in(Some(bin.as_os_str()), &std::env::temp_dir())
        .expect("same file after resolving symlinks");
}

#[test]
fn a_key_not_on_path_is_not_ambiguous() {
    let (_dir, bin) = bin_dir(&["bar"]);
    let map = clis(&[("foo", entry("bar"))]);
    CliProfile::resolve("foo", &map)
        .ensure_unambiguous_in(Some(bin.as_os_str()), &std::env::temp_dir())
        .expect("no foo on PATH");
}

/// An entry whose prompt profile overrides `live_prompt_markers`.
fn with_live_marker(command: &str, marker: &str) -> CustomCli {
    CustomCli {
        prompt_profile: Some(crate::config::CliPromptProfileOverride {
            live_prompt_markers: Some(vec![marker.to_string()]),
            ..Default::default()
        }),
        submit_delay_ms: Some(7),
        ..entry(command)
    }
}

/// Spec scenario "An unrecorded CLI falls back to generic": an agent never
/// recorded, or recorded with no CLI, resolves the `generic` family's profile
/// and settle delay.
#[test]
fn an_agent_without_a_recorded_cli_resolves_generic() {
    let mut profiles = AgentProfiles::new(clis(&[("mycli", with_live_marker("mycli", "go?"))]));
    assert!(profiles.record("feat-blank", "  ").is_none());
    let generic = CliProfile::for_family(CliFamily::Generic);
    for agent in ["feat-unseen", "feat-blank"] {
        let resolved = profiles.get(agent);
        assert_eq!(*resolved.prompt, generic.prompt_profile(), "{agent}");
        assert_eq!(
            resolved.submit_delay_ms,
            generic.submit_delay_ms(),
            "{agent}"
        );
    }
}

/// Each agent resolves the CLI its launch record names.
#[test]
fn an_agent_resolves_its_recorded_cli() {
    let map = clis(&[("mycli", with_live_marker("mycli", "go?"))]);
    let mut profiles = AgentProfiles::new(map);
    assert!(profiles.record("feat-a", "mycli").is_none());
    assert!(profiles.record("feat-b", "claude").is_none());
    assert!(profiles.record("feat-a", "mycli").is_none());
    assert_eq!(
        profiles.get("feat-a").prompt.live_prompt_markers,
        vec!["go?"]
    );
    assert_eq!(profiles.get("feat-a").submit_delay_ms, 7);
    assert_eq!(*profiles.get("feat-b").prompt, *claude_prompt_profile());
}

/// A kept worktree re-added with another CLI (`remove --keep-worktree`, then
/// `add --cli`) keeps its agent id; the running loop must read it with the
/// new CLI's profile, and a record that drops the CLI falls back to generic.
#[test]
fn an_agent_re_recorded_with_another_cli_is_re_resolved() {
    let map = clis(&[("mycli", with_live_marker("mycli", "go?"))]);
    let mut profiles = AgentProfiles::new(map);
    assert!(profiles.record("feat-a", "claude").is_none());
    assert!(profiles.record("feat-a", "mycli").is_none());
    assert_eq!(
        profiles.get("feat-a").prompt.live_prompt_markers,
        vec!["go?"]
    );
    assert!(profiles.record("feat-a", "claude").is_none());
    assert_eq!(*profiles.get("feat-a").prompt, *claude_prompt_profile());
    assert!(profiles.record("feat-a", "").is_none());
    let generic = CliProfile::for_family(CliFamily::Generic);
    assert_eq!(*profiles.get("feat-a").prompt, generic.prompt_profile());
}

/// Spec scenario "Claude's dialogs are recognised for a Claude-family pane"
/// and "A generic CLI keeps today's dialog handling": every family answers
/// the embedded default dialogs, and a configured entry cannot change them.
#[test]
fn every_family_answers_the_default_dialogs() {
    for family in CliFamily::ALL {
        assert_eq!(
            CliProfile::for_family(family).dialog_markers(),
            DEFAULT_DIALOG_MARKERS,
            "{family}"
        );
    }
    let map = clis(&[(
        "mycli",
        with_live_marker("mycli", "Bypass Permissions mode"),
    )]);
    assert_eq!(
        CliProfile::resolve("mycli", &map).dialog_markers(),
        DEFAULT_DIALOG_MARKERS
    );
}

/// An override pattern that does not compile is reported naming its field —
/// never a panic — when a table that bypassed config load is resolved.
#[test]
fn an_uncompilable_override_pattern_is_an_error_naming_the_field() {
    let bad = CustomCli {
        prompt_profile: Some(crate::config::CliPromptProfileOverride {
            paste_buffer_pattern: Some("(".to_string()),
            ..Default::default()
        }),
        ..entry("mycli")
    };
    let err = CliProfile::resolve("mycli", &clis(&[("mycli", bad)]))
        .compiled_prompt_profile()
        .expect_err("an unclosed group does not compile");
    assert!(
        err.to_string()
            .contains("[clis.mycli].prompt_profile.paste_buffer_pattern"),
        "{err}"
    );
}

/// Spec scenario "A profile that fails to compile is reported to the
/// caller": the record call returns a warning naming the agent instead of
/// printing it, and the agent resolves `generic`.
#[test]
fn a_profile_that_fails_to_compile_is_reported_to_the_caller() {
    let broken = CustomCli {
        prompt_profile: Some(crate::config::CliPromptProfileOverride {
            option_line_pattern: Some("^[0-9".to_string()),
            ..Default::default()
        }),
        ..entry("broken")
    };
    let mut profiles = AgentProfiles::new(clis(&[("broken", broken)]));
    let warning = profiles
        .record("feat-a", "broken")
        .expect("a warning for the uncompilable profile");
    assert!(warning.contains("feat-a"), "{warning}");
    let generic = CliProfile::for_family(CliFamily::Generic);
    assert_eq!(*profiles.get("feat-a").prompt, generic.prompt_profile());
}

/// [`resolve_for_launch`] from the test's working directory, dropping the
/// warning (the warning has its own tests).
#[allow(clippy::trivially_copy_pass_by_ref)]
fn launch(
    identity: &str,
    clis: &HashMap<String, CustomCli>,
    level: &ApprovalLevel,
) -> Result<CliProfile, PawError> {
    resolve_for_launch(identity, clis, level, &std::env::current_dir().unwrap())
        .map(|resolved| resolved.into_profile(|_, _| {}))
}

/// A command with a slash is found with no working directory, so `doctor`
/// still lists it when the working directory was deleted.
#[test]
fn a_slash_command_is_located_without_a_working_directory() {
    assert_eq!(
        super::profile::locate("/bin/sh", None, None),
        Some(std::path::PathBuf::from("/bin/sh"))
    );
    assert_eq!(super::profile::locate("sh", None, None), None);
}

/// Resolves against the given directory, so no readable process working
/// directory is needed.
#[test]
fn a_launch_resolves_with_the_given_working_directory() {
    // A custom entry runs the shim check, which looks `x` up with the given
    // cwd; its `/` command is found without one.
    let map = clis(&[("x", entry("/bin/sh"))]);
    let resolved = resolve_for_launch(
        "x",
        &map,
        &ApprovalLevel::Auto,
        std::path::Path::new("/nonexistent/git-paw-test-cwd"),
    )
    .expect("resolves without the process cwd");
    assert_eq!(resolved.into_profile(|_, _| {}).id(), "x");
}

/// The full-auto warning comes back with the resolved profile instead of
/// being printed.
#[test]
fn the_resolved_value_carries_the_full_auto_warning() {
    let none = HashMap::new();
    let resolved = resolve_for_launch(
        "claude",
        &none,
        &ApprovalLevel::FullAuto,
        &std::env::current_dir().unwrap(),
    )
    .unwrap();
    assert!(resolved.warning().is_some_and(|w| w.contains("claude")));
    let mut warned = Vec::new();
    let profile = resolved.into_profile(|_, w| warned.push(w));
    assert_eq!(profile.id(), "claude");
    assert_eq!(warned.len(), 1);
}

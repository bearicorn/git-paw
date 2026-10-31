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
        profile.executable_in(Some(bin.as_os_str())),
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
    assert_eq!(profile.executable_in(Some(&OsString::new())), Some(command));
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
            "claude",
        ),
        (
            "claude",
            ApprovalLevel::FullAuto,
            LaunchSession::Assign("u1"),
            "claude --dangerously-skip-permissions --session-id u1",
        ),
        (
            "claude",
            ApprovalLevel::Auto,
            LaunchSession::Resume("u1"),
            "claude --permission-mode acceptEdits --resume u1",
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
            "claude --model sonnet",
        ),
        (
            "sonnet",
            ApprovalLevel::FullAuto,
            LaunchSession::Resume("u2"),
            "claude --model sonnet --dangerously-skip-permissions --resume u2",
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
        "claude --model sonnet"
    );
    assert_eq!(
        CliProfile::resolve("opus", &map).launch_command(&level, LaunchSession::None),
        "claude --model opus"
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
        "claude --model x --dangerously-skip-permissions --session-id abc"
    );
}

/// `cli-provider` "Recovery builds the full command".
#[test]
fn recovery_carries_approval_flags_then_resume_args() {
    let cmd = CliProfile::resolve("claude", &HashMap::new())
        .launch_command(&ApprovalLevel::Auto, LaunchSession::Resume("abc"));
    let flags = cmd.find("--permission-mode acceptEdits").expect("flags");
    let resume = cmd.find("--resume abc").expect("resume");
    assert!(flags < resume, "{cmd}");
}

// --- The ambiguous-shim refusal (task 3.3) ---

/// Scenario "An ambiguous shim is refused".
#[test]
fn an_ambiguous_shim_is_refused_naming_both_paths() {
    let (_dir, bin) = bin_dir(&["claude", "claude-oss-sonnet"]);
    let map = clis(&[("claude-oss-sonnet", entry("claude"))]);
    let err = CliProfile::resolve("claude-oss-sonnet", &map)
        .ensure_unambiguous_in(Some(bin.as_os_str()))
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
        .ensure_unambiguous_in(Some(bin.as_os_str()))
        .expect("a known name is an explicit override");
    assert!(
        profile
            .launch_command(&ApprovalLevel::Manual, LaunchSession::None)
            .starts_with(&*alt_claude.to_string_lossy())
    );
}

#[test]
fn a_shim_that_is_the_same_file_as_command_is_not_ambiguous() {
    let (_dir, bin) = bin_dir(&["agent"]);
    std::os::unix::fs::symlink(bin.join("agent"), bin.join("wrapped")).expect("symlink");
    let map = clis(&[("wrapped", entry(&bin.join("agent").to_string_lossy()))]);
    CliProfile::resolve("wrapped", &map)
        .ensure_unambiguous_in(Some(bin.as_os_str()))
        .expect("same file after resolving symlinks");
}

#[test]
fn a_key_not_on_path_is_not_ambiguous() {
    let (_dir, bin) = bin_dir(&["bar"]);
    let map = clis(&[("foo", entry("bar"))]);
    CliProfile::resolve("foo", &map)
        .ensure_unambiguous_in(Some(bin.as_os_str()))
        .expect("no foo on PATH");
}

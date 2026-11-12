//! Tests for worker choice resolution.

use super::*;
use crate::config::{ProfileEntry, ProfileKind, ProfileName};
use crate::specs::{SpecBackendKind, SpecEntry};

fn name(n: &str) -> ProfileName {
    ProfileName::new(n).expect("valid name")
}

fn profile(kind: ProfileKind, cli: &str) -> ProfileEntry {
    ProfileEntry {
        kind,
        description: "d".into(),
        cli: cli.into(),
        args: None,
        skill: (kind == ProfileKind::Delegate).then(|| "explorer".to_string()),
        approval: None,
        timeout_seconds: None,
    }
}

/// `routine` (worker on `beta`, `--model small`), `explorer` (delegate).
fn config() -> PawConfig {
    let mut cfg = PawConfig::default();
    let mut routine = profile(ProfileKind::Worker, "beta");
    routine.args = Some("--model small".into());
    cfg.profiles.insert(name("routine"), routine);
    cfg.profiles
        .insert(name("explorer"), profile(ProfileKind::Delegate, "beta"));
    cfg
}

fn spec(cli: Option<&str>, profile: Option<&str>) -> SpecEntry {
    SpecEntry {
        id: "add-x".into(),
        backend: SpecBackendKind::Markdown,
        branch: "spec/add-x".into(),
        cli: cli.map(String::from),
        profile: profile.map(String::from),
        prompt: String::new(),
        owned_files: None,
    }
}

#[test]
fn profile_flag_maps_every_branch_and_skips_the_picker() {
    let cfg = config();
    let w = resolve_start_worker(&cfg, Some("routine"), None, None).expect("resolves");
    assert_eq!(
        w,
        StartWorker::Fixed(WorkerChoice::Profile(name("routine")))
    );
}

#[test]
fn cli_flag_then_preset_then_picker() {
    let mut cfg = config();
    assert_eq!(
        resolve_start_worker(&cfg, None, Some("alpha"), Some("beta")).expect("ok"),
        StartWorker::Fixed(WorkerChoice::cli("alpha"))
    );
    assert_eq!(
        resolve_start_worker(&cfg, None, None, Some("beta")).expect("ok"),
        StartWorker::Fixed(WorkerChoice::cli("beta"))
    );
    cfg.default_cli = Some("alpha".into());
    assert_eq!(
        resolve_start_worker(&cfg, None, None, None).expect("ok"),
        StartWorker::Picker {
            default: Some(WorkerChoice::cli("alpha"))
        }
    );
    cfg.default_profile = Some(name("routine"));
    assert_eq!(
        resolve_start_worker(&cfg, None, None, None).expect("ok"),
        StartWorker::Picker {
            default: Some(WorkerChoice::Profile(name("routine")))
        },
        "default_profile pre-selects ahead of default_cli"
    );
}

#[test]
fn an_unknown_or_delegate_profile_flag_is_refused() {
    let cfg = config();
    let err = resolve_start_worker(&cfg, Some("nope"), None, None)
        .expect_err("unknown")
        .to_string();
    assert!(err.contains("nope") && err.contains("--profile"), "{err}");
    let err = resolve_start_worker(&cfg, Some("explorer"), None, None)
        .expect_err("delegate")
        .to_string();
    assert!(err.contains("'explorer' is a delegate profile"), "{err}");
}

#[test]
fn add_inherits_the_sessions_profile() {
    let cfg = config();
    let choice = resolve_add_worker(&cfg, None, None, None, Some(("beta", Some("routine"))))
        .expect("resolves");
    assert_eq!(choice, Some(WorkerChoice::Profile(name("routine"))));
    let choice =
        resolve_add_worker(&cfg, None, None, None, Some(("beta", None))).expect("resolves");
    assert_eq!(choice, Some(WorkerChoice::cli("beta")));
}

#[test]
fn add_precedence_flag_then_spec_then_session_then_defaults() {
    let mut cfg = config();
    cfg.default_cli = Some("zeta".into());
    let session = Some(("beta", None));
    assert_eq!(
        resolve_add_worker(&cfg, Some("routine"), None, None, session).expect("ok"),
        Some(WorkerChoice::Profile(name("routine")))
    );
    assert_eq!(
        resolve_add_worker(&cfg, None, Some("alpha"), None, session).expect("ok"),
        Some(WorkerChoice::cli("alpha"))
    );
    let s = spec(None, Some("routine"));
    assert_eq!(
        resolve_add_worker(&cfg, None, None, Some(&s), session).expect("ok"),
        Some(WorkerChoice::Profile(name("routine")))
    );
    assert_eq!(
        resolve_add_worker(&cfg, None, None, None, None).expect("ok"),
        Some(WorkerChoice::cli("zeta"))
    );
    cfg.default_profile = Some(name("routine"));
    assert_eq!(
        resolve_add_worker(&cfg, None, None, None, None).expect("ok"),
        Some(WorkerChoice::Profile(name("routine")))
    );
}

#[test]
fn default_profile_naming_a_delegate_fails() {
    let mut cfg = config();
    cfg.default_profile = Some(name("explorer"));
    let err = resolve_add_worker(&cfg, None, None, None, None)
        .expect_err("consulted")
        .to_string();
    assert!(err.contains("default_profile"), "{err}");
    cfg.default_profile = Some(name("gone"));
    let err = configured_default(&cfg).expect_err("unknown").to_string();
    assert!(
        err.contains("default_profile") && err.contains("gone"),
        "{err}"
    );
}

#[test]
fn a_spec_with_both_paw_profile_and_paw_cli_is_rejected() {
    let err = spec_choice(&config(), &spec(Some("claude"), Some("routine")))
        .expect_err("both")
        .to_string();
    assert!(err.contains("add-x"), "{err}");
}

#[test]
fn paw_profile_naming_an_unknown_or_delegate_profile_is_rejected() {
    for bad in ["nonexistent", "explorer"] {
        let err = spec_choice(&config(), &spec(None, Some(bad)))
            .expect_err(bad)
            .to_string();
        assert!(err.contains(bad) && err.contains("add-x"), "{err}");
    }
}

#[test]
fn a_profile_resolves_to_its_cli_args_and_level() {
    let mut cfg = config();
    if let Some(p) = cfg.profiles.get_mut(&name("routine")) {
        p.approval = Some(ApprovalLevel::FullAuto);
        p.skill = Some("reviewer".into());
    }
    let w = WorkerLaunch::resolve(
        &WorkerChoice::Profile(name("routine")),
        &cfg,
        ApprovalLevel::Auto,
    )
    .expect("resolves");
    assert_eq!(w.cli, "beta");
    assert_eq!(w.profile_str(), Some("routine"));
    assert_eq!(w.profile_args.as_deref(), Some("--model small"));
    assert_eq!(w.approval, ApprovalLevel::FullAuto);
    assert_eq!(w.role_skill.as_deref(), Some("reviewer"));
    let plain = WorkerLaunch::resolve(&WorkerChoice::cli("beta"), &cfg, ApprovalLevel::Auto)
        .expect("resolves");
    assert_eq!(plain, WorkerLaunch::plain("beta", ApprovalLevel::Auto));
}

#[test]
fn a_removed_profile_falls_back_with_a_warning() {
    let cfg = config();
    let (w, warning) = WorkerLaunch::recorded(
        "feat/x",
        "beta",
        Some("gone"),
        &cfg.profiles,
        ApprovalLevel::Auto,
    );
    assert_eq!(w, WorkerLaunch::plain("beta", ApprovalLevel::Auto));
    let warning = warning.expect("one warning");
    assert!(
        warning.contains("feat/x") && warning.contains("gone"),
        "{warning}"
    );
    let (w, warning) = WorkerLaunch::recorded(
        "feat/x",
        "beta",
        Some("routine"),
        &cfg.profiles,
        ApprovalLevel::Auto,
    );
    assert_eq!(w.profile_args.as_deref(), Some("--model small"));
    assert!(warning.is_none());
    let (_, warning) = WorkerLaunch::recorded(
        "feat/x",
        "beta",
        Some("explorer"),
        &cfg.profiles,
        ApprovalLevel::Auto,
    );
    assert!(
        warning.is_some(),
        "a delegate is no longer a worker profile"
    );
}

#[test]
fn a_profile_and_a_cli_of_the_same_name_stay_distinct() {
    assert_ne!(
        WorkerChoice::cli("fast"),
        WorkerChoice::Profile(name("fast"))
    );
}

fn cli_entry(command: &str) -> crate::config::CustomCli {
    crate::config::CustomCli {
        command: command.into(),
        ..crate::config::CustomCli::default()
    }
}

/// Spec scenario "Profile arguments follow the CLI's arguments".
#[test]
fn profile_arguments_follow_the_clis_arguments() {
    let mut agent = cli_entry("/opt/agent");
    agent.args = Some("--quiet".into());
    let clis = std::collections::HashMap::from([("agent".to_string(), agent)]);
    let mut cfg = PawConfig::default();
    let mut routine = profile(ProfileKind::Worker, "agent");
    routine.args = Some("--model small".into());
    cfg.profiles.insert(name("routine"), routine);

    let w = WorkerLaunch::resolve(
        &WorkerChoice::Profile(name("routine")),
        &cfg,
        ApprovalLevel::Manual,
    )
    .expect("resolves");
    let command = w
        .cli_profile(&clis)
        .launch_command(&w.approval, crate::agent_cli::LaunchSession::None);
    assert!(
        command.starts_with("/opt/agent --quiet --model small"),
        "{command}"
    );
}

/// Spec scenario "A worker profile keeps its CLI's operator isolation": the
/// profile's arguments sit between the CLI's own and the isolation arguments,
/// and the isolation environment is unchanged.
#[test]
fn a_worker_profile_keeps_its_clis_operator_isolation() {
    let mut opus = cli_entry("claude");
    opus.family = Some("claude".into());
    opus.args = Some("--model opus".into());
    let clis = std::collections::HashMap::from([("opus".to_string(), opus)]);
    let mut cfg = PawConfig::default();
    let mut deep = profile(ProfileKind::Worker, "opus");
    deep.args = Some("--effort high".into());
    cfg.profiles.insert(name("deep"), deep);

    let w = WorkerLaunch::resolve(
        &WorkerChoice::Profile(name("deep")),
        &cfg,
        ApprovalLevel::Manual,
    )
    .expect("resolves");
    let command = w
        .cli_profile(&clis)
        .launch_command(&w.approval, crate::agent_cli::LaunchSession::Assign("abc"));
    assert_eq!(
        command,
        "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= \
         CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --model opus --effort high \
         --setting-sources project,local --session-id abc"
    );
}

/// Spec scenario "Profile approval applies only to its worker".
#[test]
fn profile_approval_applies_only_to_its_worker() {
    let mut agent = cli_entry("/opt/agent");
    agent.approval_args = std::collections::HashMap::from([
        ("full-auto".to_string(), "--yes".to_string()),
        ("auto".to_string(), "--auto".to_string()),
    ]);
    let clis = std::collections::HashMap::from([("agent".to_string(), agent)]);
    let mut cfg = PawConfig::default();
    let mut trusted = profile(ProfileKind::Worker, "agent");
    trusted.approval = Some(ApprovalLevel::FullAuto);
    cfg.profiles.insert(name("trusted"), trusted);

    let commands: Vec<String> = [
        WorkerChoice::Profile(name("trusted")),
        WorkerChoice::cli("agent"),
    ]
    .iter()
    .map(|choice| {
        let w = WorkerLaunch::resolve(choice, &cfg, ApprovalLevel::Auto).expect("resolves");
        w.cli_profile(&clis)
            .launch_command(&w.approval, crate::agent_cli::LaunchSession::None)
    })
    .collect();
    assert!(commands[0].contains("--yes"), "{}", commands[0]);
    assert!(!commands[1].contains("--yes"), "{}", commands[1]);
    assert!(commands[1].contains("--auto"), "{}", commands[1]);
}

/// A profile whose CLI is not available is refused before any launch step,
/// naming the profile and the CLI.
#[test]
fn an_unavailable_profile_cli_is_refused_naming_both() {
    let mut cfg = PawConfig::default();
    cfg.profiles.insert(
        name("routine"),
        profile(ProfileKind::Worker, "missing-agent-xyz"),
    );
    let w = WorkerLaunch::resolve(
        &WorkerChoice::Profile(name("routine")),
        &cfg,
        ApprovalLevel::Auto,
    )
    .expect("resolves");
    let err = w
        .resolve_for_launch(&std::collections::HashMap::new(), Path::new("/"))
        .expect_err("unavailable")
        .to_string();
    assert!(
        err.contains("routine") && err.contains("missing-agent-xyz"),
        "{err}"
    );
}

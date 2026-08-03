//! Unit tests for the pure `doctor` check functions.
//!
//! Each check is a pure function from probed inputs to [`CheckResult`]s, so
//! every ✓/⚠/✗ decision is exercised over injected state — no real git, tmux,
//! config file, port, or session is involved. Extracted into a `#[path]` child
//! file to keep `doctor.rs` production-only.

use super::*;

/// Finds the check named `name`, panicking with the available names when it is
/// absent — a missing check should read as a test failure, not a silent skip.
fn named<'a>(checks: &'a [CheckResult], name: &str) -> &'a CheckResult {
    checks.iter().find(|c| c.name == name).unwrap_or_else(|| {
        let available: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
        panic!("no check named '{name}'; available: {available:?}")
    })
}

/// A tool probe that resolves with the given version banner.
fn tool(banner: &str) -> ToolProbe {
    ToolProbe {
        path: Some(format!(
            "/usr/bin/{}",
            banner.split_whitespace().next().unwrap_or("tool")
        )),
        version: Some(banner.to_string()),
    }
}

/// An environment where both tools are present and current and we are in a repo.
fn healthy_environment() -> EnvironmentProbe {
    EnvironmentProbe {
        git: tool("git version 2.39.3"),
        tmux: tool("tmux 3.4"),
        in_repo: true,
    }
}

// -- Status model ---------------------------------------------------------

#[test]
fn every_non_ok_check_carries_a_remedy() {
    // The report contract: a ⚠ or ✗ is only actionable with a remedy line.
    // Drive every check function into its non-✓ branches at once.
    let probes = Probes {
        environment: EnvironmentProbe::default(),
        clis: Vec::new(),
        config: ConfigProbe {
            path: ".git-paw/config.toml".into(),
            present: true,
            parse_error: Some("expected `=`".into()),
            placement: WorktreePlacement::Sibling,
            unknown_keys: vec!["bogus".into()],
        },
        spec_system: SpecSystemProbe::default(),
        bundled_scripts: BundledScriptsProbe {
            scripts: vec![ScriptProbe {
                name: "sweep.sh",
                present: false,
                executable: false,
                matches_embedded: false,
            }],
            python3: None,
        },
        broker: BrokerProbe {
            enabled: true,
            bind: "127.0.0.1".into(),
            port: 9119,
            port_state: PortState::Foreign,
        },
        supervisor: SupervisorProbe {
            enabled: true,
            gates: vec![GateCommandProbe {
                label: "test_command",
                command: "nope check".into(),
                binary: "nope".into(),
                on_path: false,
            }],
            sweep_installed: false,
        },
        hygiene: HygieneProbe {
            missing_gitignore_entries: vec![".git-paw/logs/".into()],
            stale_sessions: vec!["paw-demo".into()],
            orphaned_worktrees: vec!["/gone".into()],
        },
    };

    let checks = run_checks(&probes);
    assert!(!checks.is_empty(), "the report should not be empty");
    for check in &checks {
        match check.status {
            CheckStatus::Ok => assert!(
                check.remedy.is_none(),
                "a ✓ check should carry no remedy: {check:?}"
            ),
            CheckStatus::Warn | CheckStatus::Fail => {
                let remedy = check.remedy.as_deref().unwrap_or("");
                assert!(
                    !remedy.trim().is_empty(),
                    "every ⚠/✗ check needs an actionable remedy: {check:?}"
                );
            }
        }
    }
}

#[test]
fn report_covers_every_documented_group() {
    let checks = run_checks(&Probes {
        environment: healthy_environment(),
        ..Probes::default()
    });
    for group in [
        GROUP_ENVIRONMENT,
        GROUP_CLIS,
        GROUP_CONFIG,
        GROUP_SPEC_SYSTEM,
        GROUP_BUNDLED_SCRIPTS,
        GROUP_BROKER,
        GROUP_SUPERVISOR,
        GROUP_HYGIENE,
    ] {
        assert!(
            checks.iter().any(|c| c.group == group),
            "the report should cover the '{group}' group"
        );
    }
}

// -- Version parsing ------------------------------------------------------

#[test]
fn parse_version_reads_the_leading_major_minor_pair() {
    for (banner, expected) in [
        ("git version 2.39.3 (Apple Git-146)", Some((2, 39))),
        ("git version 2.5.0", Some((2, 5))),
        ("tmux 3.4", Some((3, 4))),
        ("tmux 3.3a", Some((3, 3))),
        ("tmux next-3.5", Some((3, 5))),
        ("Python 3.12.4", Some((3, 12))),
        ("tmux master", None),
        ("", None),
    ] {
        assert_eq!(parse_version(banner), expected, "banner: {banner}");
    }
}

// -- Environment ----------------------------------------------------------

#[test]
fn environment_check_grades_each_tool_state() {
    for (name, probe, expected) in [
        ("tmux", ToolProbe::default(), CheckStatus::Fail),
        ("tmux", tool("tmux 3.4"), CheckStatus::Ok),
        ("tmux", tool("tmux 1.6"), CheckStatus::Fail),
        (
            "tmux",
            ToolProbe {
                path: Some("/usr/bin/tmux".into()),
                version: None,
            },
            CheckStatus::Warn,
        ),
        ("git", ToolProbe::default(), CheckStatus::Fail),
        ("git", tool("git version 2.39.3"), CheckStatus::Ok),
        ("git", tool("git version 2.4.0"), CheckStatus::Fail),
    ] {
        let mut environment = healthy_environment();
        if name == "tmux" {
            environment.tmux = probe;
        } else {
            environment.git = probe;
        }
        let checks = check_environment(&environment);
        assert_eq!(
            named(&checks, name).status,
            expected,
            "{name} state should grade as {expected:?}"
        );
    }
}

#[test]
fn missing_tmux_remedy_names_the_install_command() {
    let checks = check_environment(&EnvironmentProbe {
        tmux: ToolProbe::default(),
        ..healthy_environment()
    });
    let check = named(&checks, "tmux");
    assert_eq!(check.status, CheckStatus::Fail);
    let remedy = check.remedy.as_deref().unwrap_or_default();
    assert!(
        remedy.contains("install tmux"),
        "the remedy should tell the user to install tmux; got: {remedy}"
    );
}

#[test]
fn outside_a_repository_the_environment_group_fails() {
    let checks = check_environment(&EnvironmentProbe {
        in_repo: false,
        ..healthy_environment()
    });
    let check = named(&checks, "git repository");
    assert_eq!(check.status, CheckStatus::Fail);
    assert!(check.remedy.is_some(), "a ✗ needs a remedy");
}

// -- CLIs -----------------------------------------------------------------

#[test]
fn no_resolving_cli_warns_and_names_the_launch_failure() {
    let checks = check_clis(&[]);
    let check = named(&checks, "detected CLIs");
    assert_eq!(
        check.status,
        CheckStatus::Warn,
        "an empty roster warns rather than fails — it only blocks a launch"
    );
    let remedy = check.remedy.as_deref().unwrap_or_default();
    assert!(
        remedy.contains("add-cli") || remedy.contains("[clis."),
        "the remedy should point at installing or registering a CLI; got: {remedy}"
    );
}

#[test]
fn detected_clis_pass_and_are_listed() {
    let checks = check_clis(&[
        CliProbe {
            name: "claude".into(),
            custom: false,
        },
        CliProbe {
            name: "my-agent".into(),
            custom: true,
        },
    ]);
    let check = named(&checks, "detected CLIs");
    assert_eq!(check.status, CheckStatus::Ok);
    assert!(check.detail.contains("claude"), "detail: {}", check.detail);
    assert!(
        check.detail.contains("my-agent"),
        "detail: {}",
        check.detail
    );
}

// -- Config ---------------------------------------------------------------

#[test]
fn config_check_grades_presence_and_parseability() {
    let base = ConfigProbe {
        path: ".git-paw/config.toml".into(),
        present: true,
        parse_error: None,
        placement: WorktreePlacement::Child,
        unknown_keys: Vec::new(),
    };

    for (probe, expected) in [
        (base.clone(), CheckStatus::Ok),
        (
            ConfigProbe {
                present: false,
                ..base.clone()
            },
            CheckStatus::Warn,
        ),
        (
            ConfigProbe {
                parse_error: Some("expected `=`, found `:`".into()),
                ..base.clone()
            },
            CheckStatus::Fail,
        ),
    ] {
        let checks = check_config(&probe);
        assert_eq!(
            named(&checks, "config.toml").status,
            expected,
            "probe: {probe:?}"
        );
    }
}

#[test]
fn config_check_reports_the_resolved_worktree_placement() {
    for (placement, expected) in [
        (WorktreePlacement::Child, "child"),
        (WorktreePlacement::Sibling, "sibling"),
    ] {
        let checks = check_config(&ConfigProbe {
            path: ".git-paw/config.toml".into(),
            present: true,
            placement,
            ..ConfigProbe::default()
        });
        let check = named(&checks, "worktree placement");
        assert_eq!(check.status, CheckStatus::Ok);
        assert!(
            check.detail.contains(expected),
            "placement detail should name '{expected}'; got: {}",
            check.detail
        );
    }
}

#[test]
fn unknown_config_fields_warn_and_are_named() {
    let checks = check_config(&ConfigProbe {
        path: ".git-paw/config.toml".into(),
        present: true,
        unknown_keys: vec!["broker.frobnicate".into()],
        ..ConfigProbe::default()
    });
    let check = named(&checks, "unknown fields");
    assert_eq!(check.status, CheckStatus::Warn);
    assert!(
        check.detail.contains("broker.frobnicate"),
        "the offending key should be named; got: {}",
        check.detail
    );
}

#[test]
fn unknown_config_keys_round_trip_recognises_real_fields() {
    // Recognised keys — including nested tables and user-keyed maps — must not
    // be flagged, or every valid config would warn.
    let recognised = r#"
default_cli = "claude"
branch_prefix = "spec/"

[broker]
enabled = true
port = 9119

[clis.my-agent]
command = "/usr/local/bin/my-agent"

[specs]
type = "openspec"
"#;
    assert_eq!(unknown_config_keys(recognised), Vec::<String>::new());

    // An unrecognised top-level key and an unrecognised nested key are both
    // reported with their dotted path.
    let with_typos = r#"
defualt_cli = "claude"

[broker]
enabled = true
frobnicate = 3
"#;
    let mut found = unknown_config_keys(with_typos);
    found.sort();
    assert_eq!(found, vec!["broker.frobnicate", "defualt_cli"]);

    // Unparseable input yields no findings rather than a bogus warning — the
    // config check already reports the parse failure as ✗.
    assert_eq!(unknown_config_keys("not = = toml"), Vec::<String>::new());
}

// -- Spec system ----------------------------------------------------------

#[test]
fn spec_system_check_grades_each_resolution_state() {
    let unconfigured = check_spec_system(&SpecSystemProbe::default());
    let check = named(&unconfigured, "spec system");
    assert_eq!(check.status, CheckStatus::Warn);
    let remedy = check.remedy.as_deref().unwrap_or_default();
    assert!(
        remedy.contains("[specs]") && remedy.contains("--specs-format"),
        "the remedy should offer both the config section and the flag; got: {remedy}"
    );

    let configured = check_spec_system(&SpecSystemProbe {
        resolved_type: Some("openspec".into()),
        spec_count: Some(12),
        scan_error: None,
    });
    let check = named(&configured, "spec system");
    assert_eq!(check.status, CheckStatus::Ok);
    assert!(
        check.detail.contains("openspec"),
        "detail: {}",
        check.detail
    );
    assert!(check.detail.contains("12"), "detail: {}", check.detail);

    let broken = check_spec_system(&SpecSystemProbe {
        resolved_type: Some("speckit".into()),
        spec_count: None,
        scan_error: Some("specs directory does not exist".into()),
    });
    assert_eq!(named(&broken, "spec system").status, CheckStatus::Warn);
}

// -- Bundled scripts ------------------------------------------------------

#[test]
fn bundled_script_check_grades_each_on_disk_state() {
    let healthy = ScriptProbe {
        name: "sweep.sh",
        present: true,
        executable: true,
        matches_embedded: true,
    };
    for (probe, expected) in [
        (healthy.clone(), CheckStatus::Ok),
        (
            ScriptProbe {
                present: false,
                executable: false,
                matches_embedded: false,
                ..healthy.clone()
            },
            CheckStatus::Fail,
        ),
        (
            ScriptProbe {
                executable: false,
                ..healthy.clone()
            },
            CheckStatus::Fail,
        ),
        (
            ScriptProbe {
                matches_embedded: false,
                ..healthy.clone()
            },
            CheckStatus::Warn,
        ),
    ] {
        let checks = check_bundled_scripts(&BundledScriptsProbe {
            scripts: vec![probe.clone()],
            python3: Some("Python 3.12.4 (python3)".into()),
        });
        let check = named(&checks, "sweep.sh");
        assert_eq!(check.status, expected, "probe: {probe:?}");
        if expected != CheckStatus::Ok {
            let remedy = check.remedy.as_deref().unwrap_or_default();
            assert!(
                remedy.contains("git paw init"),
                "the remedy should point at `git paw init`; got: {remedy}"
            );
        }
    }
}

#[test]
fn missing_python3_warns_rather_than_fails() {
    let checks = check_bundled_scripts(&BundledScriptsProbe {
        scripts: Vec::new(),
        python3: None,
    });
    let check = named(&checks, "python3");
    assert_eq!(
        check.status,
        CheckStatus::Warn,
        "core start/add/remove needs no Python, so a missing interpreter is ⚠ not ✗"
    );
    let remedy = check.remedy.as_deref().unwrap_or_default();
    assert!(
        remedy.contains("Python 3"),
        "the remedy should say to install Python 3; got: {remedy}"
    );
}

#[test]
fn present_python3_passes() {
    let checks = check_bundled_scripts(&BundledScriptsProbe {
        scripts: Vec::new(),
        python3: Some("Python 3.12.4 (python3)".into()),
    });
    assert_eq!(named(&checks, "python3").status, CheckStatus::Ok);
}

// -- Broker ---------------------------------------------------------------

#[test]
fn broker_check_grades_each_port_state() {
    for (state, expected) in [
        (PortState::Free, CheckStatus::Ok),
        (PortState::LiveBroker, CheckStatus::Ok),
        (PortState::Foreign, CheckStatus::Warn),
        (PortState::Unknown, CheckStatus::Warn),
    ] {
        let checks = check_broker(&BrokerProbe {
            enabled: true,
            bind: "127.0.0.1".into(),
            port: 9119,
            port_state: state,
        });
        assert_eq!(
            named(&checks, "broker").status,
            expected,
            "state: {state:?}"
        );
    }
}

#[test]
fn disabled_broker_is_an_informational_pass() {
    let checks = check_broker(&BrokerProbe {
        enabled: false,
        ..BrokerProbe::default()
    });
    let check = named(&checks, "broker");
    assert_eq!(check.status, CheckStatus::Ok);
    assert!(
        check.detail.contains("manual"),
        "the detail should note the pure-manual baseline; got: {}",
        check.detail
    );
}

// -- Supervisor -----------------------------------------------------------

#[test]
fn missing_gate_binary_fails_with_a_remedy() {
    let checks = check_supervisor(&SupervisorProbe {
        enabled: true,
        gates: vec![GateCommandProbe {
            label: "test_command",
            command: "nope check".into(),
            binary: "nope".into(),
            on_path: false,
        }],
        sweep_installed: true,
    });
    let check = named(&checks, "test_command");
    assert_eq!(check.status, CheckStatus::Fail);
    let remedy = check.remedy.as_deref().unwrap_or_default();
    assert!(
        remedy.contains("nope"),
        "the remedy should name the missing binary; got: {remedy}"
    );
}

#[test]
fn disabled_supervisor_is_an_informational_pass() {
    let checks = check_supervisor(&SupervisorProbe::default());
    assert_eq!(named(&checks, "supervisor").status, CheckStatus::Ok);
}

#[test]
fn missing_sweep_script_fails_the_supervisor_group() {
    let checks = check_supervisor(&SupervisorProbe {
        enabled: true,
        gates: Vec::new(),
        sweep_installed: false,
    });
    let check = named(&checks, "sweep.sh");
    assert_eq!(check.status, CheckStatus::Fail);
    assert!(
        check
            .remedy
            .as_deref()
            .unwrap_or_default()
            .contains("git paw init"),
        "remedy: {:?}",
        check.remedy
    );
}

#[test]
fn supervisor_check_reports_only_the_configured_stack_verbs() {
    // Export-agnosticism (design D7): the verbs probed come from the resolved
    // stack preset, so a Node project's report names `npm` — never git-paw's
    // own cargo/just toolchain.
    let checks = check_supervisor(&SupervisorProbe {
        enabled: true,
        gates: vec![
            GateCommandProbe {
                label: "test_command",
                command: "npm test".into(),
                binary: "npm".into(),
                on_path: true,
            },
            GateCommandProbe {
                label: "lint_command",
                command: "npm run lint".into(),
                binary: "npm".into(),
                on_path: true,
            },
        ],
        sweep_installed: true,
    });

    let rendered = render_human(&checks);
    assert!(rendered.contains("npm"), "rendered: {rendered}");
    for baked_in in ["cargo", "just ", "mdbook", "openspec"] {
        assert!(
            !rendered.contains(baked_in),
            "the supervisor check must not hard-code git-paw's '{baked_in}' toolchain; \
             rendered: {rendered}"
        );
    }
}

// -- Hygiene --------------------------------------------------------------

#[test]
fn missing_gitignore_entry_warns_and_names_the_entry() {
    let checks = check_hygiene(&HygieneProbe {
        missing_gitignore_entries: vec![".git-paw/worktrees/".into()],
        ..HygieneProbe::default()
    });
    let check = named(&checks, ".gitignore");
    assert_eq!(check.status, CheckStatus::Warn);
    assert!(
        check.detail.contains(".git-paw/worktrees/"),
        "detail: {}",
        check.detail
    );
    assert!(check.remedy.is_some(), "a ⚠ needs a remedy");
}

#[test]
fn stale_session_warns_with_the_purge_remedy() {
    let checks = check_hygiene(&HygieneProbe {
        stale_sessions: vec!["paw-demo".into()],
        ..HygieneProbe::default()
    });
    let check = named(&checks, "session state");
    assert_eq!(check.status, CheckStatus::Warn);
    assert!(
        check
            .remedy
            .as_deref()
            .unwrap_or_default()
            .contains("purge --stale"),
        "remedy: {:?}",
        check.remedy
    );
}

#[test]
fn orphaned_worktree_registration_warns_with_the_purge_remedy() {
    let checks = check_hygiene(&HygieneProbe {
        orphaned_worktrees: vec!["/tmp/gone".into()],
        ..HygieneProbe::default()
    });
    let check = named(&checks, "worktree registrations");
    assert_eq!(check.status, CheckStatus::Warn);
    assert!(
        check
            .remedy
            .as_deref()
            .unwrap_or_default()
            .contains("purge --stale"),
        "remedy: {:?}",
        check.remedy
    );
}

#[test]
fn a_clean_repository_passes_every_hygiene_check() {
    for check in check_hygiene(&HygieneProbe::default()) {
        assert_eq!(check.status, CheckStatus::Ok, "check: {check:?}");
    }
}

// -- Live smoke -----------------------------------------------------------

#[test]
fn live_smoke_check_grades_each_lifecycle_verdict() {
    for (probe, expected) in [
        (LiveSmokeProbe::Passed, CheckStatus::Ok),
        (
            // A skip is ⚠, not ✗: the Environment group already reports the
            // missing prerequisite as the hard failure.
            LiveSmokeProbe::Skipped("tmux not available".into()),
            CheckStatus::Warn,
        ),
        (
            LiveSmokeProbe::Failed("Session error: selftest step 'add' failed".into()),
            CheckStatus::Fail,
        ),
    ] {
        let checks = check_live_smoke(&probe);
        let check = named(&checks, "session lifecycle");
        assert_eq!(check.group, GROUP_LIVE_SMOKE);
        assert_eq!(check.status, expected, "probe: {probe:?}");
        if expected == CheckStatus::Ok {
            assert!(check.remedy.is_none());
        } else {
            assert!(
                !check
                    .remedy
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .is_empty(),
                "a non-✓ live-smoke check needs a remedy: {check:?}"
            );
        }
    }
}

#[test]
fn a_failed_live_smoke_names_the_failing_step() {
    let checks = check_live_smoke(&LiveSmokeProbe::Failed(
        "Session error: selftest step 'roster-after-add' failed".into(),
    ));
    let check = named(&checks, "session lifecycle");
    assert!(
        check.detail.contains("roster-after-add"),
        "the detail should carry the failing step; got: {}",
        check.detail
    );
}

#[test]
fn the_live_group_is_absent_from_a_static_report() {
    let checks = run_checks(&Probes {
        environment: healthy_environment(),
        ..Probes::default()
    });
    assert!(
        !checks.iter().any(|c| c.group == GROUP_LIVE_SMOKE),
        "the live-smoke group should only appear under --live"
    );
}

// -- Verdict + exit code --------------------------------------------------

#[test]
fn exit_code_reflects_the_worst_check() {
    let ok = CheckResult::ok(GROUP_ENVIRONMENT, "a", "fine");
    let warn = CheckResult::warn(GROUP_ENVIRONMENT, "b", "iffy", "fix it");
    let fail = CheckResult::fail(GROUP_ENVIRONMENT, "c", "broken", "fix it");

    for (checks, expected_status, expected_code) in [
        (vec![ok.clone()], CheckStatus::Ok, 0),
        (
            vec![ok.clone(), warn.clone()],
            CheckStatus::Warn,
            0, // a ⚠ alone never fails the process
        ),
        (
            vec![ok.clone(), warn.clone(), fail.clone()],
            CheckStatus::Fail,
            1,
        ),
        (vec![fail.clone()], CheckStatus::Fail, 1),
        (Vec::new(), CheckStatus::Ok, 0),
    ] {
        assert_eq!(worst_status(&checks), expected_status);
        assert_eq!(exit_code(&checks), expected_code, "checks: {checks:?}");
    }
}

// -- Rendering ------------------------------------------------------------

#[test]
fn human_report_groups_checks_and_indents_remedies() {
    let checks = vec![
        CheckResult::ok(GROUP_ENVIRONMENT, "git", "git version 2.39.3"),
        CheckResult::fail(
            GROUP_ENVIRONMENT,
            "tmux",
            "tmux is not on PATH",
            "install tmux",
        ),
        CheckResult::ok(GROUP_BROKER, "broker", "disabled"),
    ];

    let rendered = render_human(&checks);
    assert!(rendered.contains(GROUP_ENVIRONMENT), "{rendered}");
    assert!(rendered.contains(GROUP_BROKER), "{rendered}");
    assert!(rendered.contains(CheckStatus::Ok.glyph()), "{rendered}");
    assert!(rendered.contains(CheckStatus::Fail.glyph()), "{rendered}");
    assert!(
        rendered.contains("install tmux"),
        "the remedy should be rendered; {rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|l| l.trim_start().starts_with('\u{21b3}')),
        "the remedy should be on its own indented line; {rendered}"
    );
}

#[test]
fn json_report_carries_every_required_field() {
    let checks = vec![
        CheckResult::ok(GROUP_ENVIRONMENT, "git", "git version 2.39.3"),
        CheckResult::warn(GROUP_CLIS, "detected CLIs", "none", "install one"),
    ];

    let rendered = render_json(&checks).expect("the report should serialise");
    let parsed: serde_json::Value =
        serde_json::from_str(&rendered).expect("--json output should parse");

    assert_eq!(parsed["status"], "warn", "the document carries the verdict");
    let entries = parsed["checks"].as_array().expect("checks array");
    assert_eq!(entries.len(), 2);
    for entry in entries {
        for field in ["group", "name", "status", "detail", "remedy"] {
            assert!(
                entry.get(field).is_some(),
                "every entry needs a '{field}' field; entry: {entry}"
            );
        }
    }
    assert_eq!(entries[0]["status"], "ok");
    assert!(entries[0]["remedy"].is_null(), "a ✓ carries no remedy");
    assert_eq!(entries[1]["remedy"], "install one");
}

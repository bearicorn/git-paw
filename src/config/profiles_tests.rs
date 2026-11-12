//! Tests for `[profiles.<name>]` loading, validation and merging.

use tempfile::TempDir;

use super::super::{PawConfig, load_config_file, load_config_from};
use super::*;

fn load(toml_text: &str) -> Result<PawConfig, String> {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("config.toml");
    std::fs::write(&path, toml_text).expect("write config");
    load_config_file(&path)
        .map(|c| c.expect("file exists"))
        .map_err(|e| e.to_string())
}

#[test]
fn a_worker_and_a_delegate_profile_load() {
    let cfg = load(
        "[profiles.routine]\n\
         kind = \"worker\"\n\
         description = \"Routine implementation\"\n\
         cli = \"my-agent\"\n\
         args = \"--model small\"\n\
         [profiles.explorer]\n\
         kind = \"delegate\"\n\
         description = \"Read-only codebase sweep\"\n\
         cli = \"my-agent\"\n\
         skill = \"explorer\"\n",
    )
    .expect("loads");
    let routine = cfg.profile("routine").expect("routine");
    assert_eq!(routine.kind, ProfileKind::Worker);
    assert_eq!(routine.description, "Routine implementation");
    assert_eq!(routine.cli, "my-agent");
    assert_eq!(routine.args.as_deref(), Some("--model small"));
    let explorer = cfg.profile("explorer").expect("explorer");
    assert_eq!(explorer.kind, ProfileKind::Delegate);
    assert_eq!(explorer.skill.as_deref(), Some("explorer"));
    assert_eq!(
        explorer.effective_approval(ApprovalLevel::FullAuto),
        ApprovalLevel::Manual
    );
    assert_eq!(explorer.effective_timeout_seconds(), 600);
    assert_eq!(
        routine.effective_approval(ApprovalLevel::Auto),
        ApprovalLevel::Auto
    );
}

/// One row per rejected entry: (case, body of `[profiles.<name>]`, profile
/// name, key the error must name, extra substrings the error must contain).
#[test]
#[allow(clippy::too_many_lines)] // one row per rejected entry
fn invalid_profile_entries_fail_the_load() {
    let cases: &[(&str, &str, &str, &str, &[&str])] = &[
        (
            "Missing kind",
            "[profiles.x]\ndescription = \"d\"\ncli = \"c\"\n",
            "x",
            "kind",
            &["worker", "delegate"],
        ),
        (
            "Unknown kind",
            "[profiles.x]\nkind = \"helper\"\ndescription = \"d\"\ncli = \"c\"\n",
            "x",
            "kind",
            &["worker", "delegate"],
        ),
        (
            "A multi-line description",
            "[profiles.x]\nkind = \"worker\"\ndescription = \"a\\nb\"\ncli = \"c\"\n",
            "x",
            "description",
            &[],
        ),
        (
            "An empty description",
            "[profiles.x]\nkind = \"worker\"\ndescription = \" \"\ncli = \"c\"\n",
            "x",
            "description",
            &[],
        ),
        (
            "A missing cli",
            "[profiles.x]\nkind = \"worker\"\ndescription = \"d\"\n",
            "x",
            "cli",
            &[],
        ),
        (
            "A full-auto delegate",
            "[profiles.fixer]\nkind = \"delegate\"\ndescription = \"d\"\ncli = \"c\"\n\
             skill = \"explorer\"\napproval = \"full-auto\"\n",
            "fixer",
            "approval",
            &[],
        ),
        (
            "A delegate without a role skill",
            "[profiles.x]\nkind = \"delegate\"\ndescription = \"d\"\ncli = \"c\"\n",
            "x",
            "skill",
            &[],
        ),
        (
            "The orchestrator skill",
            "[profiles.x]\nkind = \"worker\"\ndescription = \"d\"\ncli = \"c\"\nskill = \"orchestrator\"\n",
            "x",
            "skill",
            &[],
        ),
        (
            "The supervisor alias",
            "[profiles.x]\nkind = \"delegate\"\ndescription = \"d\"\ncli = \"c\"\nskill = \"supervisor\"\n",
            "x",
            "skill",
            &[],
        ),
        (
            "Timeout rules: zero on a delegate",
            "[profiles.x]\nkind = \"delegate\"\ndescription = \"d\"\ncli = \"c\"\n\
             skill = \"explorer\"\ntimeout_seconds = 0\n",
            "x",
            "timeout_seconds",
            &[],
        ),
        (
            "Timeout rules: set on a worker",
            "[profiles.x]\nkind = \"worker\"\ndescription = \"d\"\ncli = \"c\"\ntimeout_seconds = 60\n",
            "x",
            "timeout_seconds",
            &[],
        ),
        (
            "An unknown approval",
            "[profiles.x]\nkind = \"worker\"\ndescription = \"d\"\ncli = \"c\"\napproval = \"yolo\"\n",
            "x",
            "approval",
            &[],
        ),
    ];
    for (case, body, name, key, extra) in cases {
        let err = load(body).expect_err(case);
        assert!(
            err.contains(&format!("profiles.{name}")),
            "{case}: error should name profiles.{name}: {err}"
        );
        assert!(err.contains(key), "{case}: error should name {key}: {err}");
        for want in *extra {
            assert!(
                err.contains(want),
                "{case}: error should list {want}: {err}"
            );
        }
    }
}

#[test]
fn an_invalid_profile_name_is_rejected() {
    let err = load("[profiles.\"../x\"]\nkind = \"worker\"\ndescription = \"d\"\ncli = \"c\"\n")
        .expect_err("invalid name");
    assert!(err.contains("../x"), "error should name the profile: {err}");
}

#[test]
fn profile_name_accepts_the_documented_pattern_only() {
    for ok in ["a", "routine", "spec-audit", "v1.2_x", &"a".repeat(64)] {
        assert!(ProfileName::new(ok).is_ok(), "{ok} should be valid");
    }
    for bad in ["", "-x", ".x", "a/b", "a b", "a$", &"a".repeat(65)] {
        assert!(ProfileName::new(bad).is_err(), "{bad} should be invalid");
    }
}

#[test]
fn default_profile_loads_and_merges_like_default_cli() {
    let dir = TempDir::new().expect("tempdir");
    let global = dir.path().join("global.toml");
    std::fs::write(&global, "default_profile = \"a\"\n").expect("write");
    let repo = dir.path().join("repo");
    let merged = load_config_from(&global, &repo).expect("load");
    assert_eq!(
        merged.default_profile.as_ref().map(ProfileName::as_str),
        Some("a")
    );
    std::fs::create_dir_all(repo.join(".git-paw")).expect("mkdir");
    std::fs::write(
        repo.join(".git-paw/config.toml"),
        "default_profile = \"b\"\n",
    )
    .expect("write");
    let merged = load_config_from(&global, &repo).expect("load");
    assert_eq!(
        merged.default_profile.as_ref().map(ProfileName::as_str),
        Some("b")
    );
}

#[test]
fn the_repository_entry_replaces_the_global_entry() {
    let dir = TempDir::new().expect("tempdir");
    let global = dir.path().join("global.toml");
    std::fs::write(
        &global,
        "[profiles.routine]\nkind = \"worker\"\ndescription = \"g\"\ncli = \"c\"\n\
         args = \"--model a\"\n\
         [profiles.other]\nkind = \"worker\"\ndescription = \"o\"\ncli = \"c\"\n",
    )
    .expect("write");
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(repo.join(".git-paw")).expect("mkdir");
    std::fs::write(
        repo.join(".git-paw/config.toml"),
        "[profiles.routine]\nkind = \"worker\"\ndescription = \"r\"\ncli = \"c\"\n",
    )
    .expect("write");
    let merged = load_config_from(&global, &repo).expect("load");
    let routine = merged.profile("routine").expect("routine");
    assert_eq!(routine.args, None);
    assert_eq!(routine.description, "r");
    assert!(merged.profile("other").is_some(), "global-only entry kept");
}

#[test]
fn profiles_round_trip_through_save_and_load() {
    let cfg = load(
        "default_profile = \"routine\"\n\
         [profiles.routine]\nkind = \"worker\"\ndescription = \"d\"\ncli = \"c\"\n\
         approval = \"full-auto\"\n\
         [profiles.explorer]\nkind = \"delegate\"\ndescription = \"e\"\ncli = \"c\"\n\
         skill = \"explorer\"\ntimeout_seconds = 30\n",
    )
    .expect("loads");
    let text = toml::to_string_pretty(&cfg).expect("serialize");
    let back = load(&text).expect("reloads");
    assert_eq!(back, cfg);
}

/// A config without profiles serializes byte-identically to before profiles
/// existed: neither `default_profile` nor a `[profiles]` table is emitted.
#[test]
fn a_config_without_profiles_serializes_byte_identically() {
    let text =
        "default_cli = \"claude\"\nmouse = true\n\n[clis.my-agent]\ncommand = \"/opt/agent\"\n";
    let cfg = load(text).expect("loads");
    assert!(cfg.profiles.is_empty());
    assert!(cfg.default_profile.is_none());
    let out = toml::to_string_pretty(&cfg).expect("serialize");
    assert!(!out.contains("profile"), "no profile keys emitted: {out}");
    let mut without = cfg.clone();
    without.profiles.clear();
    assert_eq!(toml::to_string_pretty(&without).expect("serialize"), out);
    assert_eq!(load(&out).expect("reload"), cfg);
}

#[test]
fn worker_role_skill_ignores_coordination() {
    let mut p = ProfileEntry {
        kind: ProfileKind::Worker,
        description: "d".into(),
        cli: "c".into(),
        args: None,
        skill: Some("coordination".into()),
        approval: None,
        timeout_seconds: None,
    };
    assert_eq!(p.worker_role_skill(), None);
    p.skill = Some("reviewer".into());
    assert_eq!(p.worker_role_skill(), Some("reviewer"));
}

/// Scenario "The template shows profile examples".
#[test]
fn the_template_shows_profile_examples() {
    let template = super::super::generate_default_config();
    assert!(
        template.contains("\n# default_profile = "),
        "commented default_profile"
    );
    assert!(template.contains("# [profiles.routine]\n# kind = \"worker\""));
    for role in [
        "explorer",
        "spec-auditor",
        "doc-auditor",
        "security-auditor",
    ] {
        let stanza = template
            .split("# [profiles.")
            .find(|s| s.contains(&format!("# skill = \"{role}\"")))
            .unwrap_or_else(|| panic!("no delegate stanza for {role}"));
        assert!(stanza.contains("# kind = \"delegate\""), "{stanza}");
        assert!(stanza.contains("# description = \""), "{stanza}");
        assert!(
            stanza.contains("# cli = \"") && stanza.contains("# args = \""),
            "{stanza}"
        );
    }
}

/// Scenario "The examples are inert and agnostic".
#[test]
fn the_examples_are_inert_and_agnostic() {
    let template = super::super::generate_default_config();
    let cfg: PawConfig = toml::from_str(&template).expect("template loads");
    assert!(cfg.profiles.is_empty());
    assert!(cfg.default_profile.is_none());
    let mut stanzas: Vec<&str> = template
        .split("# [profiles.")
        .skip(1)
        .map(|s| s.split("\n\n").next().unwrap_or(s))
        .collect();
    let default_line = template
        .lines()
        .find(|l| l.starts_with("# default_profile"))
        .expect("default_profile line");
    stanzas.push(default_line);
    for stanza in stanzas {
        let lower = stanza.to_lowercase();
        for token in [
            "claude", "codex", "gemini", "cursor", "aider", "qwen", "agy", "opus", "sonnet",
            "haiku", "gpt", "cargo", "npm", "just ", "make ", "pytest",
        ] {
            assert!(
                !lower.contains(token),
                "a profile stanza names `{token}`: {stanza}"
            );
        }
    }
}

/// `PROFILE_KEYS` is exactly the key set an entry carries: a fully populated
/// entry serializes every one of them and nothing else.
#[test]
fn profile_keys_match_the_parser() {
    let cfg = load(
        "[profiles.x]\nkind = \"delegate\"\ndescription = \"d\"\ncli = \"c\"\nargs = \"a\"\n\
         skill = \"explorer\"\napproval = \"auto\"\ntimeout_seconds = 5\n",
    )
    .expect("loads");
    let value = toml::Value::try_from(cfg.profile("x").expect("x")).expect("serialize");
    let mut keys: Vec<&str> = value
        .as_table()
        .expect("table")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    let mut expected = PROFILE_KEYS.to_vec();
    expected.sort_unstable();
    assert_eq!(keys, expected);
}

/// A delegate profile cannot smuggle its CLI's full-auto or
/// sandbox-disabling flags in through `args`.
#[test]
fn a_delegate_with_permission_widening_args_is_rejected() {
    for (cli, args) in [
        ("claude", "--model small --dangerously-skip-permissions"),
        ("claude", "--permission-mode bypassPermissions"),
        ("claude", "--permission-mode acceptEdits"),
        ("claude", "--permission-mode=acceptEdits"),
        ("codex", "--dangerously-bypass-approvals-and-sandbox"),
        ("codex", "--sandbox danger-full-access"),
        ("codex", "--sandbox workspace-write"),
        ("codex", "-c sandbox_mode=danger-full-access"),
    ] {
        let err = load(&format!(
            "[profiles.sneaky]\nkind = \"delegate\"\ndescription = \"d\"\ncli = \"{cli}\"\n\
             skill = \"explorer\"\nargs = \"{args}\"\n"
        ))
        .expect_err(cli);
        assert!(
            err.contains("profiles.sneaky") && err.contains("args"),
            "{err}"
        );
    }
    // Ordinary arguments, and the same flag on a worker profile, still load.
    assert!(
        load(
            "[profiles.ok]\nkind = \"delegate\"\ndescription = \"d\"\ncli = \"claude\"\n\
         skill = \"explorer\"\nargs = \"--model small\"\n"
        )
        .is_ok()
    );
    assert!(
        load(
            "[profiles.w]\nkind = \"worker\"\ndescription = \"d\"\ncli = \"claude\"\n\
         args = \"--dangerously-skip-permissions\"\n"
        )
        .is_ok()
    );
}

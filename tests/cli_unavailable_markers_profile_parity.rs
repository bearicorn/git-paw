//! `cli-unavailable-detection-gaps` task 3.3: the drive loop's per-pane
//! `cli_unavailable_markers` resolution and the bundled helper's read path
//! (`git paw __prompt-profile --field cli_unavailable_markers`) must agree
//! for a CLI with a `[clis.<name>].prompt_profile` override — both resolve
//! through the same [`resolve_prompt_profile`], never two independently
//! maintained copies.

use std::fs;
use std::process::Command as StdCommand;

use git_paw::config::{load_config, resolve_prompt_profile};
use tempfile::TempDir;

fn init_git_repo(dir: &std::path::Path) {
    let run = |args: &[&str]| {
        StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git command");
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "t@e.st"]);
    run(&["config", "user.name", "Test"]);
    fs::write(dir.join("README.md"), "x").expect("readme");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);
}

/// Spec scenario "A per-CLI marker override is honoured by the loop": the
/// loop's `resolve_prompt_profile("mycli", &config.clis).cli_unavailable_markers`
/// and `git paw __prompt-profile --cli mycli --field cli_unavailable_markers`
/// classify the same overridden CLI identically.
#[test]
fn loop_and_prompt_profile_subcommand_agree_for_overridden_cli() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let paw = repo.path().join(".git-paw");
    fs::create_dir_all(&paw).expect("mk .git-paw");
    fs::write(
        paw.join("config.toml"),
        "default_cli = \"mycli\"\n\n\
         [clis.mycli]\n\
         command = \"mycli\"\n\n\
         [clis.mycli.prompt_profile]\n\
         cli_unavailable_markers = [\"out of tokens\"]\n",
    )
    .expect("write config");

    let out = StdCommand::new(assert_cmd::cargo::cargo_bin("git-paw"))
        .args([
            "__prompt-profile",
            "--cli",
            "mycli",
            "--field",
            "cli_unavailable_markers",
        ])
        .current_dir(repo.path())
        .output()
        .expect("run __prompt-profile");
    assert!(out.status.success(), "{:?}", out.stderr);
    let printed = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let shell_markers: Vec<String> = printed.split('|').map(str::to_string).collect();

    // Isolation hook: an unused TempDir path, never the dev machine's real
    // user-level config.
    let isolated_global = TempDir::new().expect("isolated global config dir");
    let config = load_config(
        repo.path(),
        Some(&isolated_global.path().join("config.toml")),
    )
    .expect("load config");
    let resolved = resolve_prompt_profile("mycli", &config.clis);

    assert_eq!(
        shell_markers, resolved.cli_unavailable_markers,
        "the subcommand's printed field and the loop's resolved markers must match exactly"
    );
    assert_eq!(
        resolved.cli_unavailable_markers,
        vec!["out of tokens".to_string()],
        "the override must actually take effect, not silently fall back to the default"
    );
}

//! The bundled skill package registry (`skill-packages` capability): the
//! registry and the `assets/agent-skills/` tree agree file for file, every
//! package is a standardized skill, helpers have one owner, the binary needs
//! no asset directory at run time, and a skill override never supplies a
//! helper git-paw installs.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use assert_cmd::Command;
use git_paw::skills::{
    BundledPackage, ORCHESTRATOR_SKILL, PAW_COMMON_SCRIPT, ProvisionGate, Source, bundled_package,
    bundled_packages, render_standardized, to_skill_md, validate_skill_dir,
};
use serial_test::serial;
use tempfile::TempDir;

/// Resolves `name` from the embedded defaults only (no override root), so a
/// developer's own overrides never change what this test sees.
fn resolve(name: &str) -> Result<git_paw::skills::SkillTemplate, git_paw::skills::SkillError> {
    git_paw::skills::resolve_from(name, &git_paw::skills::SkillRoots::default())
}

/// The package tree's root.
fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/agent-skills")
}

/// Directory under the package root that holds shared files and is not a
/// package itself.
const SHARED_DIR: &str = "_shared";

/// Every non-hidden file under `dir`, as paths relative to `base` with `/` separators.
fn files_under(dir: &Path, base: &Path, out: &mut BTreeSet<String>) {
    for entry in fs::read_dir(dir).expect("read dir").flatten() {
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            files_under(&path, base, out);
        } else {
            let rel = path.strip_prefix(base).expect("under base");
            out.insert(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// Every source path the registry embeds, relative to the package root.
fn registered_sources(packages: &[BundledPackage]) -> BTreeSet<String> {
    packages
        .iter()
        .flat_map(BundledPackage::files)
        .map(|f| {
            f.source
                .strip_prefix("assets/agent-skills/")
                .unwrap_or_else(|| panic!("{} lies outside the package tree", f.source))
                .to_string()
        })
        .collect()
}

/// Files on disk under `root` that the registry does not list, plus any
/// package directory that is not registered.
fn unregistered_files(root: &Path, packages: &[BundledPackage]) -> Vec<String> {
    let mut on_disk = BTreeSet::new();
    files_under(root, root, &mut on_disk);
    let registered = registered_sources(packages);
    on_disk
        .into_iter()
        .filter(|f| !registered.contains(f))
        .collect()
}

// skill-packages / Scenario: An unregistered package file fails the suite
#[test]
fn every_package_file_is_registered_and_every_registered_file_exists() {
    let root = package_root();
    let unregistered = unregistered_files(&root, bundled_packages());
    assert!(
        unregistered.is_empty(),
        "files under assets/agent-skills/ without a registry entry: {unregistered:?}"
    );

    for file in bundled_packages().iter().flat_map(BundledPackage::files) {
        let on_disk = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(file.source))
            .unwrap_or_else(|e| panic!("registered file {} is missing: {e}", file.source));
        assert_eq!(on_disk, file.content, "{} drifted from disk", file.source);
    }

    // Every package directory is a registered package (or the shared dir).
    for entry in fs::read_dir(&root).expect("read package root").flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        assert!(
            name == SHARED_DIR || bundled_package(&name).is_some(),
            "assets/agent-skills/{name} is not a registered package"
        );
    }
}

// skill-packages / Scenario: An unregistered package file fails the suite —
// the conformance check names a file added without a registry entry.
#[test]
fn the_conformance_check_names_an_unregistered_file() {
    let copy = TempDir::new().expect("tempdir");
    let mut files = BTreeSet::new();
    files_under(&package_root(), &package_root(), &mut files);
    for rel in &files {
        let dst = copy.path().join(rel);
        fs::create_dir_all(dst.parent().expect("parent")).expect("mkdir");
        fs::copy(package_root().join(rel), dst).expect("copy");
    }
    fs::create_dir_all(copy.path().join("coordination/references")).expect("mkdir");
    fs::write(copy.path().join("coordination/references/new.md"), "x")
        .expect("write unregistered file");

    assert_eq!(
        unregistered_files(copy.path(), bundled_packages()),
        vec!["coordination/references/new.md".to_string()]
    );
}

// skill-packages / Scenario: Every bundled package validates as a standardized skill
#[test]
fn every_bundled_package_validates_as_a_standardized_skill() {
    for package in bundled_packages() {
        let tmp = TempDir::new().expect("tempdir");
        let dir = tmp.path().join(package.name);
        fs::create_dir_all(&dir).expect("mkdir");
        fs::write(dir.join("SKILL.md"), package.skill_md.content).expect("write SKILL.md");
        let meta = validate_skill_dir(&dir)
            .unwrap_or_else(|e| panic!("{} does not validate: {e}", package.name));
        assert_eq!(meta.name, package.name);
        assert!(!meta.description.is_empty(), "{} description", package.name);
    }
}

/// The packages whose scripts list the helper `file_name`.
fn owners(file_name: &str) -> Vec<&'static str> {
    bundled_packages()
        .iter()
        .filter(|p| p.scripts.iter().any(|f| f.file_name() == file_name))
        .map(|p| p.name)
        .collect()
}

// skill-packages / Scenario: Each helper belongs to one package
#[test]
fn each_helper_belongs_to_one_package() {
    assert_eq!(owners("broker.sh"), ["coordination"]);
    assert_eq!(owners("sweep.sh"), ["orchestrator"]);
    assert_eq!(owners("docs-fetch.sh"), ["docs-fetch"]);
    for asset in ["assets/boot-block.md", "assets/boot-block-mcp.md"] {
        let holders: Vec<&str> = bundled_packages()
            .iter()
            .filter(|p| p.assets.iter().any(|f| f.path == asset))
            .map(|p| p.name)
            .collect();
        assert_eq!(holders, ["coordination"], "{asset}");
    }
    // Every helper (other than the preamble) has exactly one owner.
    for package in bundled_packages() {
        for helper in package.helpers() {
            assert_eq!(owners(helper.file_name()), [package.name]);
        }
    }
}

// skill-packages / Scenario: The cli-setup skill is a registered package without helpers
#[test]
fn the_cli_setup_skill_is_a_registered_package_without_helpers() {
    let cli_setup = bundled_package("cli-setup").expect("cli-setup is registered");
    assert!(cli_setup.scripts.is_empty(), "cli-setup ships no helper");
    assert_eq!(cli_setup.gate, ProvisionGate::RootOnly);
}

// skill-packages / Scenario: Every helper-shipping package carries the shared preamble
#[test]
fn every_helper_shipping_package_carries_the_shared_preamble() {
    let mut preamble: Option<&str> = None;
    for package in bundled_packages() {
        if package.helpers().next().is_none() {
            continue;
        }
        let common = package
            .scripts
            .iter()
            .find(|f| f.file_name() == PAW_COMMON_SCRIPT)
            .unwrap_or_else(|| panic!("{} ships a helper but no preamble", package.name));
        match preamble {
            None => preamble = Some(common.content),
            Some(seen) => assert_eq!(seen, common.content, "{}", package.name),
        }
    }
    assert!(preamble.is_some(), "some package ships a helper");
}

// skill-packages / Scenario: The binary renders bundled skills with no assets on disk
#[test]
#[serial]
fn the_binary_renders_bundled_skills_with_no_assets_on_disk() {
    // Run a copy of the binary from a directory tree with no `assets/`.
    let bin_dir = TempDir::new().expect("bin dir");
    let bin = bin_dir.path().join("git-paw");
    fs::copy(assert_cmd::cargo::cargo_bin("git-paw"), &bin).expect("copy binary");

    let repo = TempDir::new().expect("repo");
    let st = StdCommand::new("git")
        .args(["init", "-q", "-b", "main"])
        .current_dir(repo.path())
        .status()
        .expect("git init");
    assert!(st.success());
    assert!(!repo.path().join("assets").exists());

    let home = TempDir::new().expect("home");
    Command::new(&bin)
        .arg("init")
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_CONFIG_HOME")
        .assert()
        .success();

    let cli_setup = fs::read_to_string(repo.path().join(".git-paw/skills/cli-setup/SKILL.md"))
        .expect("cli-setup exported");
    let embedded = bundled_package("cli-setup")
        .expect("registered")
        .skill_md
        .content;
    let body = embedded
        .split("\n---\n")
        .nth(1)
        .expect("frontmatter closes");
    assert!(
        cli_setup.contains(body.trim()),
        "exported cli-setup renders from the embedded copy"
    );
    for name in ["_paw_common.sh", "broker.sh", "sweep.sh", "docs-fetch.sh"] {
        assert!(
            repo.path().join(".git-paw/scripts").join(name).is_file(),
            "{name} installed from the embedded copy"
        );
    }
}

// skill-packages / Scenario: An override's script is not installed
#[test]
#[serial]
fn an_overrides_script_is_not_installed() {
    let project = TempDir::new().expect("project");
    // A repository, so production resolution finds its override level.
    assert!(
        StdCommand::new("git")
            .args(["init", "-q", "-b", "main"])
            .current_dir(project.path())
            .status()
            .expect("git init")
            .success()
    );
    let override_dir = project.path().join(".agents/skills/coordination");
    fs::create_dir_all(override_dir.join("scripts")).expect("mkdir override");
    fs::write(
        override_dir.join("SKILL.md"),
        "---\nname: coordination\ndescription: override\n---\n\nOVERRIDE-MARKER {{BRANCH_ID}}\n",
    )
    .expect("write override SKILL.md");
    fs::write(
        override_dir.join("scripts/broker.sh"),
        "#!/usr/bin/env bash\necho decoy\n",
    )
    .expect("write decoy broker.sh");

    let worktree = TempDir::new().expect("worktree");
    let roots = git_paw::skills::SkillRoots {
        repo_root: Some(project.path().to_path_buf()),
        user_config_dir: None,
    };
    let resolved =
        git_paw::skills::resolve_from("coordination", &roots).expect("coordination resolves");
    let packages = git_paw::skills::resolve_worktree_packages(
        git_paw::skills::WorktreeInputs {
            broker_enabled: true,
            docs_base_url_configured: false,
        },
        project.path(),
    )
    .expect("selection resolves");
    let inputs = git_paw::skills::RenderInputs {
        branch: "feat/x",
        root: false,
        broker_url: "http://127.0.0.1:9119",
        project: "proj",
        gates: git_paw::skills::GateCommands::default(),
        backends: &[],
    };
    git_paw::init::provision_worktree_helpers(worktree.path(), &packages, &inputs)
        .expect("provision worktree helpers");

    assert_eq!(resolved.source, Source::AgentsStandard);
    assert!(
        resolved.content.contains("OVERRIDE-MARKER"),
        "the override's skill text is what renders"
    );
    let installed = fs::read_to_string(
        worktree
            .path()
            .join(".git-paw/skills/coordination/scripts/broker.sh"),
    )
    .expect("broker.sh provisioned");
    let compat = fs::read_to_string(worktree.path().join(".git-paw/scripts/broker.sh"))
        .expect("compatibility copy provisioned");
    assert_eq!(compat, installed);
    assert!(
        fs::read_to_string(
            worktree
                .path()
                .join(".git-paw/skills/coordination/SKILL.md")
        )
        .expect("SKILL.md provisioned")
        .contains("OVERRIDE-MARKER feat-x"),
        "the installed SKILL.md renders the override's text"
    );
    let embedded = bundled_package("coordination")
        .expect("registered")
        .scripts
        .iter()
        .find(|f| f.file_name() == "broker.sh")
        .expect("coordination ships broker.sh")
        .content;
    assert_eq!(
        installed, embedded,
        "provisioned broker.sh is the registry copy"
    );
}

/// The pre-package source paths existing requirements still name, mapped to
/// the package file each now means (`skill-packages` *Requirements naming
/// pre-package asset paths refer to package files*).
const PRE_PACKAGE_PATHS: &[(&str, &str)] = &[
    (
        "assets/agent-skills/coordination.md",
        "assets/agent-skills/coordination/SKILL.md",
    ),
    (
        "assets/agent-skills/supervisor.md",
        "assets/agent-skills/orchestrator/SKILL.md",
    ),
    (
        "assets/agent-skills/orchestrator.md",
        "assets/agent-skills/orchestrator/SKILL.md",
    ),
    (
        "assets/agent-skills/docs-fetch.md",
        "assets/agent-skills/docs-fetch/SKILL.md",
    ),
    (
        "assets/agent-skills/cli-setup.md",
        "assets/agent-skills/cli-setup/SKILL.md",
    ),
    (
        "assets/scripts/broker.sh",
        "assets/agent-skills/coordination/scripts/broker.sh",
    ),
    (
        "assets/scripts/sweep.sh",
        "assets/agent-skills/orchestrator/scripts/sweep.sh",
    ),
    (
        "assets/scripts/docs-fetch.sh",
        "assets/agent-skills/docs-fetch/scripts/docs-fetch.sh",
    ),
    (
        "assets/scripts/_paw_common.sh",
        "assets/agent-skills/_shared/scripts/_paw_common.sh",
    ),
    (
        "assets/boot-block-template.md",
        "assets/agent-skills/coordination/assets/boot-block.md",
    ),
    (
        "assets/boot-block-mcp-template.md",
        "assets/agent-skills/coordination/assets/boot-block-mcp.md",
    ),
];

// skill-packages / Scenario: Each pre-package path maps to one package file
#[test]
fn each_pre_package_path_maps_to_one_registered_package_file() {
    let registered: BTreeSet<&str> = bundled_packages()
        .iter()
        .flat_map(BundledPackage::files)
        .map(|f| f.source)
        .collect();
    for (old, new) in PRE_PACKAGE_PATHS {
        assert!(
            registered.contains(new),
            "{old} maps to {new}, which is not registered"
        );
        assert!(
            !Path::new(env!("CARGO_MANIFEST_DIR")).join(old).exists(),
            "{old} still exists beside its package file {new}"
        );
    }
}

// skill-packages / Scenario: A content requirement on a pre-package path is
// checked against the package file
#[test]
fn a_content_requirement_on_a_pre_package_path_is_checked_against_the_package_file() {
    let orchestrator = bundled_package("orchestrator").expect("registered");
    let sweep = orchestrator
        .scripts
        .iter()
        .find(|f| f.file_name() == "sweep.sh")
        .expect("orchestrator ships sweep.sh");
    assert_eq!(
        sweep.source,
        "assets/agent-skills/orchestrator/scripts/sweep.sh"
    );
    // A `sweep.sh` subcommand requirement (e.g. `approve`) holds unchanged
    // against the package file.
    assert!(sweep.content.contains("cmd_approve()"));
}

// skill-packages / Scenario: Init writes the same files — re-pointed by
// *Installation supersedes the packaging identity pin*: the compatibility
// directory holds exactly the bundled helpers, byte-identical to the
// registry, and each root SKILL.md equals a fresh render of its package with
// the config `init` just wrote (not the pre-package output).
#[test]
#[serial]
fn init_writes_the_same_files() {
    let repo = TempDir::new().expect("repo");
    let st = StdCommand::new("git")
        .args(["init", "-q", "-b", "main"])
        .current_dir(repo.path())
        .status()
        .expect("git init");
    assert!(st.success());
    let home = TempDir::new().expect("home");
    Command::cargo_bin("git-paw")
        .expect("binary")
        .arg("init")
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_CONFIG_HOME")
        .assert()
        .success();

    let scripts_dir = repo.path().join(".git-paw/scripts");
    let mut names: Vec<String> = fs::read_dir(&scripts_dir)
        .expect("scripts dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    // Init must keep writing exactly these files.
    assert_eq!(
        names,
        ["_paw_common.sh", "broker.sh", "docs-fetch.sh", "sweep.sh"]
    );
    for (name, content) in git_paw::init::bundled_scripts() {
        let path = scripts_dir.join(name);
        assert_eq!(fs::read_to_string(&path).expect("read"), content, "{name}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
            assert_eq!(mode, 0o755, "{name} mode");
        }
    }

    // Each exported SKILL.md is exactly the render of its embedded package,
    // with the config `init` just wrote.
    let config = git_paw::config::load_config_from(&home.path().join("absent.toml"), repo.path())
        .expect("load config");
    let backends: Vec<_> = git_paw::specs::resolved_spec_kind(&config)
        .into_iter()
        .collect();
    let supervisor = config.supervisor.clone().unwrap_or_default();
    for name in [ORCHESTRATOR_SKILL, "cli-setup"] {
        let template = resolve(name).expect("resolve");
        assert_eq!(template.source, Source::Embedded, "{name} is not shadowed");
        let expected = if name == ORCHESTRATOR_SKILL {
            render_standardized(&template, &supervisor.gate_commands(), &backends)
        } else {
            to_skill_md(&template)
        };
        let rel = format!(".git-paw/skills/{name}/SKILL.md");
        let exported = fs::read_to_string(repo.path().join(&rel)).expect("exported skill");
        assert_eq!(exported, expected, "{rel}");
    }
}

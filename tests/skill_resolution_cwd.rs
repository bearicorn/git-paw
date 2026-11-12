//! `skill-agent` / *Skill resolution order*, scenario "Resolution does not
//! depend on the current directory": an `.agents/skills/` above the
//! repository is never read, whatever the process's current directory.
//!
//! The scenario is about the process's current directory, so this test
//! changes it, and isolates `HOME`/`XDG_CONFIG_HOME` so no user override can
//! answer; it is the sole test in its binary, so mutating process-global
//! state is race-free.

use std::path::Path;
use std::process::Command;

use git_paw::skills::{Source, resolve};

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn resolution_does_not_depend_on_the_current_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let ancestor = tmp.path();
    let skill_dir = ancestor.join(".agents").join("skills").join("coordination");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: coordination\ndescription: above the repo\n---\n\nANCESTOR-MARKER",
    )
    .unwrap();
    let repo = ancestor.join("repo");
    std::fs::create_dir_all(repo.join("sub")).unwrap();
    git(&repo, &["init", "-q"]);

    let home = tempfile::tempdir().unwrap();
    // SAFETY: this is the only test in this binary, so no other thread reads
    // or writes the environment concurrently.
    unsafe {
        std::env::set_var("HOME", home.path());
        std::env::remove_var("XDG_CONFIG_HOME");
    }

    let original = std::env::current_dir().unwrap();
    for cwd in [ancestor.to_path_buf(), repo.join("sub")] {
        std::env::set_current_dir(&cwd).unwrap();
        let result = resolve("coordination", &repo);
        std::env::set_current_dir(&original).unwrap();
        let tmpl = result.unwrap();
        assert_eq!(tmpl.source, Source::Embedded, "cwd {}", cwd.display());
        assert!(!tmpl.content.contains("ANCESTOR-MARKER"));
    }
}

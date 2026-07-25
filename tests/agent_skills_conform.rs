//! Guard: every repository agent skill under `.agents/skills/` conforms to the
//! agentskills.io standard, validated through git-paw's **own** skill resolver
//! (dogfoods the `skill-standardization` / `skill-validation` capabilities).
//!
//! Part of the `test-strategy` change: keeps every `.agents/skills/*` skill
//! (currently `test-strategy`, `code-standards`) spec-conformant so a malformed
//! or drifted skill fails the build. Sole test in its binary, so pinning the
//! process cwd to the crate root is race-free.

use std::fs;
use std::path::Path;

#[test]
fn repo_agent_skills_conform_to_agentskills_standard() {
    // `resolve()` walks up from the process cwd to find `.agents/skills/<name>`.
    // Pin it to the crate root so the guard checks *this repo's* skills.
    std::env::set_current_dir(env!("CARGO_MANIFEST_DIR")).expect("chdir to crate root");

    let skills_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".agents")
        .join("skills");
    assert!(
        skills_dir.is_dir(),
        ".agents/skills/ must exist (the repo's agent skills live here)"
    );

    let mut checked = 0;
    for entry in fs::read_dir(&skills_dir).expect("read .agents/skills") {
        let path = entry.expect("dir entry").path();
        if !path.is_dir() {
            continue;
        }
        let folder = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("utf-8 skill dir name")
            .to_string();

        assert!(
            path.join("SKILL.md").is_file(),
            "{folder}: agent skill directory is missing SKILL.md"
        );

        // Dogfood the real loader + validator (required frontmatter, etc.).
        let template = git_paw::skills::resolve(&folder)
            .unwrap_or_else(|e| panic!("{folder}: skill resolver rejected the skill: {e}"));
        let meta = template.metadata.as_ref().unwrap_or_else(|| {
            panic!("{folder}: not loaded as a standardized skill (no frontmatter metadata)")
        });

        assert_eq!(
            meta.name, folder,
            "{folder}: SKILL.md frontmatter `name` must match the folder name"
        );
        assert!(
            !meta.description.trim().is_empty(),
            "{folder}: SKILL.md `description` must be non-empty"
        );
        checked += 1;
    }

    assert!(
        checked >= 2,
        "expected at least the test-strategy and code-standards skills under .agents/skills/, checked {checked}"
    );
}

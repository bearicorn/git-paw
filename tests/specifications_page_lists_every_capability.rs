//! Guard: the mdBook Specifications page must list every capability spec.
//!
//! The Specifications page (`docs/src/specifications/README.md`) carries a
//! domain-grouped index that must cover all capabilities under
//! `openspec/specs/`. A future spec add or rename could silently drop a
//! capability off the page, leaving the published index stale. This test is
//! the standing guard: it fails if any capability directory name is missing
//! from the page, so the index cannot drift from the spec set.

use std::path::Path;

const PAGE: &str = include_str!("../docs/src/specifications/README.md");

#[test]
fn specifications_page_lists_every_capability() {
    let specs_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("openspec/specs");
    assert!(
        specs_dir.is_dir(),
        "openspec/specs should exist at {}",
        specs_dir.display()
    );

    let mut missing = Vec::new();
    for entry in std::fs::read_dir(&specs_dir).expect("read openspec/specs") {
        let dir = entry.expect("dir entry").path();
        if !dir.is_dir() {
            continue;
        }
        if !dir.join("spec.md").is_file() {
            continue;
        }
        let cap = dir
            .file_name()
            .and_then(|n| n.to_str())
            .expect("capability dir name")
            .to_string();
        if !PAGE.contains(&cap) {
            missing.push(cap);
        }
    }
    missing.sort();
    assert!(
        missing.is_empty(),
        "these capabilities are missing from docs/src/specifications/README.md — \
         add them to the capability index so the page stays in sync with \
         openspec/specs/: {missing:?}"
    );
}

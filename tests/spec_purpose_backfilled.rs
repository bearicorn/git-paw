//! Guard: every archived `OpenSpec` capability spec must carry a real `## Purpose`
//! section, not the `openspec archive` placeholder.
//!
//! `openspec archive` scaffolds each spec's Purpose as
//! `TBD - created by archiving change <X>. Update Purpose after archive.` and
//! relies on the author backfilling it. Historically that backfill was skipped
//! for 78 of 95 specs (surfaced by the v0.10.0 consistency sweep). This test is
//! the standing guard: it fails if any spec under `openspec/specs/` still
//! contains the placeholder, so a future archive that forgets the backfill is
//! caught in CI rather than accumulating silently.

use std::path::Path;

const PLACEHOLDER: &str = "Update Purpose after archive";

#[test]
fn no_spec_retains_the_purpose_placeholder() {
    let specs_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("openspec/specs");
    assert!(
        specs_dir.is_dir(),
        "openspec/specs should exist at {}",
        specs_dir.display()
    );

    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&specs_dir).expect("read openspec/specs") {
        let dir = entry.expect("dir entry").path();
        if !dir.is_dir() {
            continue;
        }
        let spec = dir.join("spec.md");
        if !spec.is_file() {
            continue;
        }
        let body = std::fs::read_to_string(&spec).expect("read spec.md");
        if body.contains(PLACEHOLDER) {
            offenders.push(
                dir.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string(),
            );
        }
    }
    offenders.sort();
    assert!(
        offenders.is_empty(),
        "these specs still carry the '{PLACEHOLDER}' placeholder — backfill their \
         ## Purpose section after archiving: {offenders:?}"
    );
}

//! Doc-content checks for `AGENTS.md`'s approved-dependencies table and its
//! "Notable exclusions" sub-section.
//!
//! Implements openspec/specs/core-project-conventions/spec.md's AGENTS.md
//! dependency-table requirements, including the drop-serde-yaml delta that
//! moves `serde_yaml` from the table into "Notable exclusions".

use std::fs;
use std::path::Path;

/// Returns the `### Dependencies` section of `AGENTS.md`, up to (but not
/// including) the next `##`-level heading.
fn dependencies_section() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("AGENTS.md");
    let agents_md =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let start = agents_md
        .find("### Dependencies")
        .expect("AGENTS.md must have a Dependencies section");
    let rest = &agents_md[start..];
    let end = rest[1..].find("\n## ").map_or(rest.len(), |i| i + 1);
    rest[..end].to_string()
}

#[test]
fn dependency_table_lists_schemars_chrono_and_regex() {
    let section = dependencies_section();
    for crate_name in ["schemars", "chrono", "regex"] {
        assert!(
            section.contains(crate_name),
            "Dependencies section must mention `{crate_name}`"
        );
    }
}

#[test]
fn schemars_row_purpose_mentions_json_schema_or_governance() {
    let section = dependencies_section();
    let row = section
        .lines()
        .find(|l| l.trim_start().starts_with("| `schemars`"))
        .expect("schemars row must exist in the dependency table");
    let lower = row.to_lowercase();
    assert!(
        lower.contains("json schema") || lower.contains("governance"),
        "schemars row must mention JSON Schema or governance config: {row}"
    );
}

#[test]
fn chrono_and_regex_rows_have_non_empty_purpose() {
    let section = dependencies_section();
    for crate_name in ["chrono", "regex"] {
        let prefix = format!("| `{crate_name}`");
        let row = section
            .lines()
            .find(|l| l.trim_start().starts_with(&prefix))
            .unwrap_or_else(|| panic!("{crate_name} row must exist in the dependency table"));
        let purpose = row.rsplit('|').nth(1).unwrap_or("").trim();
        assert!(
            !purpose.is_empty(),
            "{crate_name} row must have a non-empty Purpose cell: {row}"
        );
    }
}

#[test]
fn serde_yaml_is_not_an_approved_dependency_row() {
    let section = dependencies_section();
    let table_end = section.find("Dev:").unwrap_or(section.len());
    let table = &section[..table_end];
    assert!(
        !table
            .lines()
            .any(|l| l.trim_start().starts_with("| `serde_yaml`")),
        "serde_yaml must not appear as an approved-dependencies table row"
    );
}

#[test]
fn notable_exclusions_documents_dirs_and_serde_yaml() {
    let section = dependencies_section();
    let exclusions_start = section
        .find("Notable exclusions")
        .expect("Dependencies section must have a Notable exclusions sub-section");
    let exclusions = &section[exclusions_start..];
    let lower = exclusions.to_lowercase();

    assert!(
        exclusions.contains("`dirs`") && lower.contains("src/dirs.rs"),
        "Notable exclusions must describe the dirs replacement"
    );
    assert!(
        lower.contains("foss"),
        "Notable exclusions must explain the dirs license issue"
    );

    assert!(
        exclusions.contains("`serde_yaml`"),
        "Notable exclusions must mention serde_yaml"
    );
    assert!(
        lower.contains("unsafe-libyaml") || lower.contains("rustsec-2024-0449"),
        "Notable exclusions must cite the unmaintained unsafe-libyaml advisory for serde_yaml"
    );
    assert!(
        lower.contains("do not re-add") || lower.contains("not re-add"),
        "Notable exclusions must instruct contributors not to re-add serde_yaml"
    );
}

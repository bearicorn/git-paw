//! Guard: the Supported AI CLIs lists must match `src/detect.rs`'s roster.
//!
//! The README's one-line Supported CLIs list and the mdBook
//! `docs/src/supported-clis.md` table can drift from the roster: a missing
//! binary under-advertises support, and one left after removal promises
//! detection that no longer happens (as the `cursor` → `cursor-agent` move
//! would have). The guard checks both directions for both lists.

use std::collections::BTreeSet;
use std::path::Path;

/// Parses the backticked binary cells out of the one markdown table in
/// `content` whose header row has a `Binary` column.
///
/// Only that column is read, so backticks appearing anywhere else in the
/// document's prose cannot be mistaken for an advertised binary. The table is
/// located by its header text rather than by line number so the surrounding
/// page can be re-organised freely.
fn binary_column(label: &str, content: &str) -> BTreeSet<String> {
    let lines: Vec<&str> = content.lines().collect();

    let mut header_rows = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        if let Some(col) = cells(trimmed)
            .iter()
            .position(|c| c.eq_ignore_ascii_case("binary"))
        {
            header_rows.push((idx, col));
        }
    }

    assert_eq!(
        header_rows.len(),
        1,
        "{label}: expected exactly one markdown table with a `Binary` header \
         column, found {}. Adjust this guard if the page gained another.",
        header_rows.len()
    );
    let (header_idx, col) = header_rows[0];

    // The row directly under the header is the `|---|---|` separator.
    let separator = lines.get(header_idx + 1).map_or("", |l| l.trim());
    assert!(
        separator.starts_with('|') && separator.contains("---"),
        "{label}: the row after the `Binary` header should be the table \
         separator, got {separator:?}"
    );

    let mut binaries = BTreeSet::new();
    for line in lines.iter().skip(header_idx + 2) {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            break; // end of the table
        }
        let row = cells(trimmed);
        let Some(cell) = row.get(col) else {
            continue;
        };
        let binary = cell.trim().trim_matches('`').trim();
        assert!(
            !binary.is_empty(),
            "{label}: empty binary cell in the Supported AI CLIs table row {trimmed:?}"
        );
        binaries.insert(binary.to_string());
    }

    assert!(
        !binaries.is_empty(),
        "{label}: parsed no binaries out of the Supported AI CLIs table"
    );
    binaries
}

/// The `` `<binary>` `` code spans on the first non-empty line after
/// `## Supported CLIs`. Only that line is read, so code spans elsewhere in
/// the README cannot count as advertised binaries.
fn readme_cli_line(content: &str) -> BTreeSet<String> {
    let mut lines = content.lines();
    assert!(
        lines.by_ref().any(|l| l.trim() == "## Supported CLIs"),
        "README.md: expected a `## Supported CLIs` section"
    );
    let line = lines
        .find(|l| !l.trim().is_empty())
        .expect("README.md: the Supported CLIs section is empty");

    let binaries: BTreeSet<String> = line
        .split('`')
        .skip(1)
        .step_by(2)
        .map(|span| span.trim().to_string())
        .collect();
    assert!(
        !binaries.is_empty(),
        "README.md: parsed no binary code spans from the Supported CLIs line {line:?}"
    );
    binaries
}

/// Splits one markdown table row into its trimmed cells.
fn cells(row: &str) -> Vec<String> {
    row.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

#[test]
fn readme_and_book_tables_match_known_cli_roster() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let roster: BTreeSet<String> = git_paw::detect::known_cli_names()
        .iter()
        .map(|n| (*n).to_string())
        .collect();

    assert!(
        roster.contains("cursor-agent"),
        "the known-CLI roster should carry Cursor's agent CLI as `cursor-agent`"
    );

    let read = |rel: &str| {
        let path = root.join(rel);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    };
    let lists = [
        ("README.md", readme_cli_line(&read("README.md"))),
        (
            "docs/src/supported-clis.md",
            binary_column(
                "docs/src/supported-clis.md",
                &read("docs/src/supported-clis.md"),
            ),
        ),
    ];

    for (label, listed) in lists {
        let missing: Vec<&String> = roster.difference(&listed).collect();
        assert!(
            missing.is_empty(),
            "{label}: the Supported AI CLIs table is missing a row for these \
             binaries from src/detect.rs's known-CLI roster: {missing:?}"
        );

        let extra: Vec<&String> = listed.difference(&roster).collect();
        assert!(
            extra.is_empty(),
            "{label}: the Supported AI CLIs table lists these binaries, which \
             src/detect.rs's known-CLI roster does not auto-detect: {extra:?}"
        );

        assert!(
            !listed.contains("cursor"),
            "{label}: `cursor` is the Cursor editor launcher and is no longer \
             auto-detected — the table should name `cursor-agent` instead"
        );
    }
}

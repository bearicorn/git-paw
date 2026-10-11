//! Guard: the Supported AI CLIs tables must match `src/detect.rs`'s roster.
//!
//! Both the README's Supported AI CLIs table and the mdBook
//! `docs/src/supported-clis.md` table advertise which binaries git-paw
//! auto-detects. Either can silently drift when the roster changes — a binary
//! added to `KNOWN_CLIS` but not to the tables under-advertises support, and a
//! binary dropped from the roster but left in a table promises detection that
//! no longer happens (exactly what the `cursor` → `cursor-agent` move would
//! have left behind). This test is the standing guard in both directions:
//! every roster binary appears in each table, and neither table lists a binary
//! the roster does not carry.

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

    let tables = [
        ("README.md", root.join("README.md")),
        (
            "docs/src/supported-clis.md",
            root.join("docs/src/supported-clis.md"),
        ),
    ];

    for (label, path) in tables {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let listed = binary_column(label, &content);

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

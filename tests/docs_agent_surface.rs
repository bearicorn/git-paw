//! Behavioral tests for the agent-friendly docs surface generator
//! (`docs/generate_agent_metadata.py`).
//!
//! Each test builds a tiny fixture — a `SUMMARY.md`, source pages, and
//! `mdBook`-style rendered HTML — runs the generator, and asserts on its
//! observable output: the `llms.txt` / `sitemap.xml` / `robots.txt` artifacts
//! and the per-page `<head>` metadata injection. One test runs the generator
//! twice and asserts byte-for-byte reproducibility.
//!
//! Maps to the `agent-friendly-docs-site` scenarios: `llms.txt` index (with the
//! source `summary` override), `sitemap.xml`, `robots.txt`, per-page structured
//! metadata, and deterministic/reproducible generation.
//!
//! Skipped (not failed) when `python3` is not on PATH, so unrelated Rust work
//! on a Python-less machine is not blocked; CI always has `python3`.

#![allow(clippy::too_many_lines)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

const BASE_URL: &str = "https://example.test/docs/";
const BUILD_DATE: &str = "2020-01-02";

/// Canonical URLs the fixture's three pages must resolve to.
const INTRO_URL: &str = "https://example.test/docs/intro.html";
const GUIDE_URL: &str = "https://example.test/docs/guide/index.html";
const NESTED_URL: &str = "https://example.test/docs/guide/nested.html";

/// The three text artifacts whose generation must be reproducible.
const TEXT_ARTIFACTS: &[&str] = &["llms.txt", "sitemap.xml", "robots.txt"];

/// Absolute path to the generator script under test.
fn generator() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/generate_agent_metadata.py")
}

/// Whether `python3` is callable; the test is skipped if not.
fn have_python3() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Write a minimal fixture (sources + `mdBook`-style rendered HTML) under `root`.
fn write_fixture(root: &Path) {
    let src = root.join("src");
    let book = root.join("book");
    fs::create_dir_all(src.join("guide")).unwrap();
    fs::create_dir_all(book.join("guide")).unwrap();

    fs::write(
        root.join("book.toml"),
        "[book]\ntitle = \"Test Docs\"\ndescription = \"A test book\"\n",
    )
    .unwrap();

    fs::write(
        src.join("SUMMARY.md"),
        "# Summary\n\n[Intro](intro.md)\n\n- [Guide](guide/README.md)\n  - [Nested Page](guide/nested.md)\n",
    )
    .unwrap();

    // Intro carries a leading `<!-- summary: -->` override.
    fs::write(
        src.join("intro.md"),
        "<!-- summary: The overridden intro summary. -->\n# Intro\n\nFallback sentence that must not appear. Second.\n",
    )
    .unwrap();
    fs::write(
        src.join("guide/README.md"),
        "# Guide\n\nThe guide overview paragraph. Second sentence here.\n",
    )
    .unwrap();
    fs::write(
        src.join("guide/nested.md"),
        "# Nested Page\n\nNested lead paragraph only.\n",
    )
    .unwrap();

    // Rendered pages mirror mdBook: a global description meta, a content
    // `<main>` with heading ids, and (on the guide) a help-popup paragraph
    // OUTSIDE `<main>` that the fallback must ignore.
    fs::write(book.join("intro.html"), rendered_page(
        "Intro",
        "<main>\n<h1 id=\"intro\"><a href=\"#intro\">Intro</a></h1>\n<p>Fallback sentence that must not appear. Second.</p>\n<h2 id=\"details\"><a href=\"#details\">Details</a></h2>\n</main>",
    )).unwrap();
    fs::write(book.join("guide/index.html"), rendered_page(
        "Guide",
        "<div id=\"help\"><p>Press ? to show this help</p></div>\n<main>\n<h1 id=\"guide\"><a href=\"#guide\">Guide</a></h1>\n<p>The guide overview paragraph. Second sentence here.</p>\n<h2 id=\"usage\"><a href=\"#usage\">Usage</a></h2>\n</main>",
    )).unwrap();
    fs::write(book.join("guide/nested.html"), rendered_page(
        "Nested Page",
        "<main>\n<h1 id=\"nested-page\"><a href=\"#nested-page\">Nested Page</a></h1>\n<p>Nested lead paragraph only.</p>\n</main>",
    )).unwrap();
}

/// Wrap a body in an mdBook-shaped document with a global description meta.
fn rendered_page(title: &str, body: &str) -> String {
    format!(
        "<!DOCTYPE HTML>\n<html lang=\"en\">\n    <head>\n        <meta charset=\"UTF-8\">\n        <title>{title} - Test Docs</title>\n        <meta name=\"description\" content=\"A test book\">\n    </head>\n    <body>\n{body}\n    </body>\n</html>\n"
    )
}

/// Run the generator over `root`, asserting a clean exit.
fn run_generator(root: &Path) {
    let output = Command::new("python3")
        .arg(generator())
        .arg("--src-dir")
        .arg(root.join("src"))
        .arg("--book-dir")
        .arg(root.join("book"))
        .arg("--base-url")
        .arg(BASE_URL)
        .arg("--build-date")
        .arg(BUILD_DATE)
        .output()
        .expect("spawn generator");
    assert!(
        output.status.success(),
        "generator failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Build the fixture and run the generator once; `None` if `python3` is absent.
fn setup() -> Option<TempDir> {
    if !have_python3() {
        eprintln!("skipping docs_agent_surface test: python3 not on PATH");
        return None;
    }
    let tmp = TempDir::new().unwrap();
    write_fixture(tmp.path());
    run_generator(tmp.path());
    Some(tmp)
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join("book").join(rel)).unwrap()
}

#[test]
fn llms_txt_indexes_every_page_and_honors_override() {
    let Some(tmp) = setup() else { return };
    let llms = read(tmp.path(), "llms.txt");

    assert!(!llms.trim().is_empty(), "llms.txt is empty");
    assert!(
        llms.starts_with("# Test Docs\n"),
        "missing H1 title: {llms}"
    );
    assert!(
        llms.contains("\n> A test book\n"),
        "missing summary blockquote: {llms}"
    );

    // Every SUMMARY page has a link entry (absolute URL).
    for url in [INTRO_URL, GUIDE_URL, NESTED_URL] {
        assert!(
            llms.contains(url),
            "llms.txt missing entry for {url}:\n{llms}"
        );
    }

    // The source override wins over the derived first sentence.
    assert!(
        llms.contains("The overridden intro summary."),
        "override summary not used:\n{llms}"
    );
    assert!(
        !llms.contains("Fallback sentence that must not appear"),
        "override was ignored:\n{llms}"
    );

    // The fallback is the first sentence of the rendered lead paragraph, not
    // the help-popup text that sits outside <main>.
    assert!(
        llms.contains("The guide overview paragraph."),
        "fallback summary wrong:\n{llms}"
    );
    assert!(
        !llms.contains("Press ? to show this help"),
        "popup leaked into summary:\n{llms}"
    );
}

#[test]
fn sitemap_enumerates_every_page() {
    let Some(tmp) = setup() else { return };
    let sitemap = read(tmp.path(), "sitemap.xml");

    assert!(
        sitemap.starts_with("<?xml"),
        "sitemap is not XML:\n{sitemap}"
    );
    assert!(sitemap.contains("<urlset"), "missing urlset:\n{sitemap}");
    for url in [INTRO_URL, GUIDE_URL, NESTED_URL] {
        assert!(
            sitemap.contains(&format!("<loc>{url}</loc>")),
            "sitemap missing {url}:\n{sitemap}"
        );
    }
    assert_eq!(
        sitemap.matches("<loc>").count(),
        3,
        "expected one <loc> per page:\n{sitemap}"
    );
}

#[test]
fn robots_allows_crawling_and_advertises_sitemap() {
    let Some(tmp) = setup() else { return };
    let robots = read(tmp.path(), "robots.txt");

    assert!(
        robots.contains("User-agent: *"),
        "robots missing user-agent:\n{robots}"
    );
    assert!(
        robots.contains("Allow: /"),
        "robots missing allow:\n{robots}"
    );
    assert!(
        !robots.contains("Disallow"),
        "robots must not disallow doc paths:\n{robots}"
    );
    assert!(
        robots.contains("Sitemap: https://example.test/docs/sitemap.xml"),
        "robots missing sitemap line:\n{robots}"
    );
}

#[test]
fn built_page_carries_description_and_metadata_block() {
    let Some(tmp) = setup() else { return };
    let page = read(tmp.path(), "intro.html");

    // Exactly one description meta, replaced with the page-specific summary.
    assert_eq!(
        page.matches("name=\"description\"").count(),
        1,
        "expected one description meta:\n{page}"
    );
    assert!(
        page.contains("<meta name=\"description\" content=\"The overridden intro summary.\">"),
        "description meta not page-specific:\n{page}"
    );

    // A machine-readable metadata block with title, canonical URL and anchors.
    assert!(
        page.contains("id=\"git-paw-page-metadata\""),
        "missing metadata block:\n{page}"
    );
    assert!(
        page.contains(&format!("\"url\": \"{INTRO_URL}\"")),
        "metadata missing canonical URL:\n{page}"
    );
    assert!(
        page.contains("\"title\": \"Intro\""),
        "metadata missing title:\n{page}"
    );
    assert!(
        page.contains("\"intro\""),
        "metadata missing h1 anchor:\n{page}"
    );
    assert!(
        page.contains("\"details\""),
        "metadata missing h2 anchor:\n{page}"
    );
}

#[test]
fn generation_is_reproducible() {
    let Some(tmp) = setup() else { return };

    let mut before: Vec<String> = TEXT_ARTIFACTS.iter().map(|a| read(tmp.path(), a)).collect();
    before.push(read(tmp.path(), "intro.html"));

    run_generator(tmp.path());

    let mut after: Vec<String> = TEXT_ARTIFACTS.iter().map(|a| read(tmp.path(), a)).collect();
    after.push(read(tmp.path(), "intro.html"));

    let names: Vec<&str> = TEXT_ARTIFACTS
        .iter()
        .copied()
        .chain(["intro.html"])
        .collect();
    for ((name, b), a) in names.iter().zip(before).zip(after) {
        assert_eq!(b, a, "{name} changed between identical runs");
    }
}

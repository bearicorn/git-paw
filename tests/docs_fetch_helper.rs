//! End-to-end tests for the bundled `assets/scripts/docs-fetch.sh` helper,
//! exercised against `file://` fixtures so no network is required.
//!
//! Covers docs-fetch-skill scenarios: discovery via `llms.txt`, whole-page and
//! section-by-anchor retrieval, default vs overridden docs base URL, and
//! non-zero exit with a diagnostic on a missing page.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serial_test::serial;
use tempfile::TempDir;

/// Absolute path to the bundled helper under test.
fn helper_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts/docs-fetch.sh")
}

/// Creates a git repo with fixtures and (optionally) a `docs_base_url` pointing
/// the helper at those fixtures via a `file://` URL. Returns the temp repo.
fn setup_repo(with_base_url: bool) -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path();

    let st = Command::new("git")
        .current_dir(root)
        .args(["init", "-b", "main"])
        .status()
        .expect("git init");
    assert!(st.success());

    // Fixtures: an llms.txt index and one page with the agent-metadata block.
    let fixtures = root.join("fixtures");
    fs::create_dir_all(fixtures.join("user-guide")).expect("mkdir fixtures");
    fs::write(
        fixtures.join("llms.txt"),
        "# git-paw\n\
         \n\
         > Orchestrate parallel AI coding agents across git worktrees.\n\
         \n\
         ## User Guide\n\
         \n\
         - [Agent Coordination](https://example.test/user-guide/coordination.html): How agents coordinate via the broker.\n\
         - [Dashboard](https://example.test/user-guide/dashboard.html): The live TUI dashboard.\n",
    )
    .expect("write llms.txt");
    fs::write(
        fixtures.join("user-guide/coordination.html"),
        r#"<!DOCTYPE html><html><head><title>Coordination</title>
<!-- git-paw:agent-metadata:start -->
<meta name="description" content="How agents coordinate.">
<script type="application/json" id="git-paw-page-metadata">{"title": "Agent Coordination", "url": "https://example.test/user-guide/coordination.html", "description": "How agents coordinate.", "anchors": ["intent", "blocked"]}</script>
<!-- git-paw:agent-metadata:end -->
</head><body>
<nav>sidebar noise ignored</nav>
<main>
<h1 id="coordination">Coordination</h1>
<p>Intro paragraph about coordination.</p>
<h2 id="intent">Publishing intent</h2>
<p>Publish an agent.intent before editing.</p>
<h2 id="blocked">Reporting blocked</h2>
<p>Publish agent.blocked when you wait on a peer.</p>
</main></body></html>
"#,
    )
    .expect("write page");

    fs::create_dir_all(root.join(".git-paw")).expect("mkdir .git-paw");
    if with_base_url {
        let base = format!("file://{}", fixtures.display());
        fs::write(
            root.join(".git-paw/config.toml"),
            format!("docs_base_url = \"{base}\"\n"),
        )
        .expect("write config");
    }
    tmp
}

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new("bash")
        .arg(helper_path())
        .args(args)
        .current_dir(repo)
        .output()
        .expect("run docs-fetch.sh")
}

#[test]
#[serial]
fn find_returns_matching_pages_from_configured_base() {
    let repo = setup_repo(true);
    let out = run(repo.path(), &["find", "coordination broker"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "find should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Discovery reads llms.txt from the configured (overridden) base URL and
    // returns the matching entry: title, absolute URL, summary.
    assert!(
        stdout.contains("Agent Coordination"),
        "missing title; got:\n{stdout}"
    );
    assert!(
        stdout.contains("https://example.test/user-guide/coordination.html"),
        "missing absolute URL; got:\n{stdout}"
    );
    assert!(
        stdout.contains("How agents coordinate"),
        "missing summary; got:\n{stdout}"
    );
}

#[test]
#[serial]
fn get_returns_whole_page() {
    let repo = setup_repo(true);
    let out = run(repo.path(), &["get", "user-guide/coordination.html"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "get should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Whole-page retrieval returns the page's documentation content, and
    // ignores mdBook chrome outside <main>.
    assert!(
        stdout.contains("Publishing intent"),
        "missing intent section; got:\n{stdout}"
    );
    assert!(
        stdout.contains("Reporting blocked"),
        "missing blocked section; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("sidebar noise"),
        "should not include chrome outside <main>; got:\n{stdout}"
    );
}

#[test]
#[serial]
fn get_narrows_to_section_by_anchor() {
    let repo = setup_repo(true);
    let out = run(
        repo.path(),
        &["get", "user-guide/coordination.html", "intent"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "get with anchor should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Section retrieval returns only the requested section, not the whole page.
    assert!(
        stdout.contains("Publishing intent"),
        "should include the requested section; got:\n{stdout}"
    );
    assert!(
        stdout.contains("Publish an agent.intent"),
        "should include the section body; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("Reporting blocked"),
        "section narrowing should exclude sibling sections; got:\n{stdout}"
    );
}

#[test]
#[serial]
fn get_missing_page_exits_nonzero_with_diagnostic() {
    let repo = setup_repo(true);
    let out = run(repo.path(), &["get", "user-guide/does-not-exist.html"]);
    assert!(!out.status.success(), "missing page must exit non-zero");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("docs-fetch.sh") && stderr.contains("continue"),
        "should print a short diagnostic that lets the agent continue; got:\n{stderr}"
    );
}

#[test]
#[serial]
fn overridden_base_url_is_reported() {
    let repo = setup_repo(true);
    // Usage reports the discovered docs base URL — the configured (overridden)
    // file:// fixtures URL, not the built-in default.
    let out = run(repo.path(), &["--help"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("file://"),
        "usage should report the overridden base URL; got:\n{stderr}"
    );
    assert!(
        !stderr.contains("bearicorn.github.io"),
        "overridden config must not fall back to the default; got:\n{stderr}"
    );
}

#[test]
#[serial]
fn default_base_url_when_unconfigured() {
    let repo = setup_repo(false); // no docs_base_url configured
    // With no configured URL, the helper targets git-paw's published site.
    let out = run(repo.path(), &["--help"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("https://bearicorn.github.io/git-paw"),
        "unconfigured helper should default to the published docs site; got:\n{stderr}"
    );
}

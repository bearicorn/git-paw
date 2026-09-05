//! Docs-parity guards for the `cli-attach-completions` change.
//!
//! Maps to the `cli-parsing` spec scenarios "`git paw resume` is not a valid
//! subcommand" (the CLI-rejection half lives in `src/cli.rs`'s
//! `resume_is_rejected_as_unknown_subcommand`) and "Docs do not reference a
//! resume command", plus the Gate-4 doc-completeness requirement that a new
//! CLI command is documented in the CLI reference and README.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// Every `.md` file under `root` (recursive) as `(display_path, contents)`.
fn markdown_files(root: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    collect_markdown(root, root, &mut out);
    out
}

fn collect_markdown(base: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display())) {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_markdown(base, &path, out);
            continue;
        }
        if path.extension().is_some_and(|ext| ext == "md") {
            let contents = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            let display = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            out.push((display, contents));
        }
    }
}

// Scenario: Docs do not reference a resume command.
#[test]
fn no_doc_references_a_resume_command() {
    let mut offenders = Vec::new();

    let readme = read("README.md");
    if readme.contains("paw resume") {
        offenders.push("README.md".to_string());
    }

    for (path, contents) in markdown_files(&repo_root().join("docs/src")) {
        if contents.contains("paw resume") {
            offenders.push(format!("docs/src/{path}"));
        }
    }

    assert!(
        offenders.is_empty(),
        "no doc may reference a `git paw resume` command (reattach is `git paw attach`, \
         revival is `git paw start`); offenders: {offenders:?}"
    );
}

// Scenario: attach and completions are documented (Gate-4: new CLI command
// → CLI reference + README table).
#[test]
fn readme_and_cli_reference_document_attach_and_completions() {
    let readme = read("README.md");
    for needle in ["`attach`", "`completions`"] {
        assert!(
            readme.contains(needle),
            "README's command table should document {needle}"
        );
    }

    let cli_reference = read("docs/src/cli-reference.md");
    for needle in ["## `git paw attach`", "## `git paw completions`"] {
        assert!(
            cli_reference.contains(needle),
            "cli-reference.md should document {needle}"
        );
    }
}

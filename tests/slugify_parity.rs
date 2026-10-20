//! The bundled shell helper derives the same `agent_id` as the Rust side.
//!
//! `agent-id-slug-safety` (`broker-protocol`, "Branch slug function"): the
//! shell `slugify` in `assets/scripts/_paw_common.sh` — the fallback
//! `broker.sh` uses when no `--agent` is given — SHALL return the same string
//! as [`git_paw::broker::messages::slugify_branch`] for every input.
//!
//! This runs the REAL function: the test sources the bundled helper in `bash`
//! and calls `slugify` once per corpus entry (design D4), rather than
//! re-implementing the shell rules in Rust, where a mirror could agree with
//! the test while disagreeing with what agents actually run. Two
//! implementations of one rule are exactly how the derivation drifted from the
//! `/publish` validator in the first place (PP-26).

use std::io::Write;
use std::process::{Command, Stdio};

use git_paw::broker::messages::slugify_branch;

#[path = "helpers/slug_corpus.rs"]
mod slug_corpus;

/// Sources the bundled `_paw_common.sh` and slugifies one corpus entry per
/// input line, echoing one output line each.
///
/// Names arrive on stdin rather than as arguments so that no branch name is
/// ever interpolated into a shell command line. `IFS= read -r` keeps each line
/// byte-exact (leading and trailing spaces included, backslashes not treated
/// as escapes), which matters because the corpus contains entries that are
/// nothing but separator characters.
const DRIVER: &str = r#"
set -u
PAW_SCRIPT_NAME=slugify-parity-test
. "${COMMON_SH}"
while IFS= read -r name; do
  slugify "${name}"
done
"#;

#[test]
fn shell_slugify_matches_slugify_branch_for_every_corpus_entry() {
    let corpus = slug_corpus::branch_corpus();
    let common_sh = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/scripts/_paw_common.sh");

    let mut child = Command::new("bash")
        .arg("-c")
        .arg(DRIVER)
        .env("COMMON_SH", common_sh)
        // `_paw_common.sh` resolves the project root at source time and exits
        // when it is not inside a git repository, so run it in this one.
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn bash to source the bundled _paw_common.sh");

    let stdin_payload = format!("{}\n", corpus.join("\n"));
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(stdin_payload.as_bytes())
        .expect("write corpus to the shell driver");

    let output = child.wait_with_output().expect("wait for the shell driver");
    assert!(
        output.status.success(),
        "the shell driver failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("shell output is utf-8");
    let shell_ids: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        shell_ids.len(),
        corpus.len(),
        "the shell driver emitted {} lines for {} corpus entries",
        shell_ids.len(),
        corpus.len()
    );

    for (branch, shell_id) in corpus.iter().zip(shell_ids) {
        assert_eq!(
            shell_id,
            slugify_branch(branch),
            "shell slugify and slugify_branch disagree on {branch:?}"
        );
    }
}

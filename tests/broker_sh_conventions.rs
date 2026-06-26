//! Convention enforcement for `assets/scripts/broker.sh`.
//!
//! Mirrors `sweep_sh_conventions` for the agent-side helper: the
//! stdin-claiming pipe shape (`python3 - <<`, `sh - <<`, etc.) must not be
//! reintroduced. When a heredoc supplies the interpreter's script via `-`,
//! bash heredocs win over an upstream pipe, silently swallowing the data the
//! script was supposed to read. The fix is to pass the script via
//! `-c "$(cat <<'EOF' ... EOF)"` so the interpreter takes its script from an
//! argument and leaves stdin free for the pipe (`agent-broker-helper` §
//! "Helper convention discipline").

use std::path::Path;

/// Patterns whose presence in `broker.sh` would re-introduce the
/// pipe/heredoc stdin collision.
const FORBIDDEN_SUBSTRINGS: &[&str] = &[
    // Direct interpreter `- <<` shapes — interpreter reads script from
    // stdin (heredoc), which collides with any upstream pipe feeding the
    // same stdin.
    "python3 - <<",
    "python - <<",
    "sh - <<",
    "bash - <<",
    // Variable-indirected forms. The script uses `"${PY}" -c "$(cat <<'PY'
    // … PY)"`; the shapes below would be the regression.
    "\"${PY}\" - <<",
    "${PY} - <<",
];

/// Scan a shell-script body for forbidden stdin-claiming heredoc shapes.
/// Returns one diagnostic per match, in source order. Pure shell comment
/// lines (whitespace + `#`) are skipped so explanatory comments can
/// reference the forbidden shapes without tripping the lint.
fn scan(body: &str) -> Vec<String> {
    let mut hits: Vec<String> = Vec::new();
    for (line_no, line) in body.lines().enumerate() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        for pattern in FORBIDDEN_SUBSTRINGS {
            if line.contains(pattern) {
                hits.push(format!(
                    "line {}: matched `{pattern}`\n    > {}",
                    line_no + 1,
                    line.trim_end()
                ));
            }
        }
    }
    hits
}

#[test]
fn broker_sh_has_no_stdin_claiming_pipe_pattern() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("scripts")
        .join("broker.sh");
    let body = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("read {}: {e}", path.display());
    });

    let hits = scan(&body);

    assert!(
        hits.is_empty(),
        "broker.sh contains stdin-claiming pipe pattern(s); use `-c \"$(cat <<'EOF' ... EOF)\"` \
         instead of `interpreter - <<` to keep stdin free for upstream pipes.\n  in {}:\n{}",
        path.display(),
        hits.join("\n")
    );
}

/// Spec scenario `Reintroduced heredoc shape fails the test`: GIVEN a
/// synthetic body containing a `python3 - <<'PY'` block, WHEN the scanner
/// runs, THEN it SHALL flag the offending shape and identify the line.
#[test]
fn detector_flags_synthesised_python3_heredoc_pattern() {
    let synthetic = "\
#!/usr/bin/env bash
set -u
echo prelude
curl -s http://example.invalid/status | python3 - <<'PY'
import sys
sys.stdin.read()
PY
echo epilogue
";

    let hits = scan(synthetic);
    assert!(
        !hits.is_empty(),
        "detector should flag `python3 - <<` in synthetic body, but found nothing"
    );
    assert!(
        hits.iter().any(|h| h.contains("python3 - <<")),
        "diagnostic should name the matched pattern `python3 - <<`; got: {hits:?}"
    );
    assert!(
        hits.iter().any(|h| h.starts_with("line 4:")),
        "diagnostic should identify the offending line (line 4); got: {hits:?}"
    );
}

/// Sibling negative test: `sh - <<` is flagged the same way.
#[test]
fn detector_flags_synthesised_sh_heredoc_pattern() {
    let synthetic = "\
#!/usr/bin/env bash
cat /tmp/data | sh - <<'EOF'
read line
echo \"$line\"
EOF
";

    let hits = scan(synthetic);
    assert!(
        hits.iter().any(|h| h.contains("sh - <<")),
        "detector should flag `sh - <<` shape; got: {hits:?}"
    );
}

/// Pure-comment lines must not trip the lint, so the script can document
/// *why* the forbidden shape was avoided.
#[test]
fn detector_ignores_pure_comment_lines() {
    let synthetic = "\
#!/usr/bin/env bash
# Historical note: `python3 - <<'PY'` swallowed the pipe content.
echo ok
";
    assert!(
        scan(synthetic).is_empty(),
        "pure-comment lines must not trigger the lint"
    );
}

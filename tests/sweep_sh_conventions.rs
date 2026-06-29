//! Convention enforcement for `assets/scripts/sweep.sh`.
//!
//! Pins the v0-5-0-audit-cleanup §10 fix: stdin-claiming pipe shapes
//! (`python3 - <<`, `sh - <<`, etc.) must not return. When a heredoc
//! supplies the script source via `-`, bash heredocs win over upstream
//! pipes, silently swallowing the data the script was supposed to read.
//! The fix is to pass the script via `-c "$(cat <<'EOF' ... EOF)"` so
//! the interpreter takes its script from an argument and leaves stdin
//! free for the pipe.

use std::path::Path;

/// Patterns whose presence in `sweep.sh` would re-introduce the
/// pipe/heredoc stdin collision documented in
/// `openspec/changes/cold-start-ci-parity/proposal.md`.
const FORBIDDEN_SUBSTRINGS: &[&str] = &[
    // Direct interpreter `- <<` shapes — interpreter reads script from
    // stdin (heredoc), which collides with any upstream pipe feeding the
    // same stdin.
    "python3 - <<",
    "python - <<",
    "sh - <<",
    "bash - <<",
    // Variable-indirected forms used in this script. The current file
    // uses `"${PY}" - <args> <<'PY'` (args between `-` and `<<`); the
    // adjacent shape below would be the regression.
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
fn sweep_sh_has_no_stdin_claiming_pipe_pattern() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("scripts")
        .join("sweep.sh");
    let body = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("read {}: {e}", path.display());
    });

    let hits = scan(&body);

    assert!(
        hits.is_empty(),
        "sweep.sh contains stdin-claiming pipe pattern(s); use `-c \"$(cat <<'EOF' ... EOF)\"` \
         instead of `interpreter - <<` to keep stdin free for upstream pipes.\n  in {}:\n{}",
        path.display(),
        hits.join("\n")
    );
}

/// Spec scenario `A reintroduced python3 - << pattern fails the test`:
/// GIVEN a sweep.sh edit adding a `python3 - <<EOF` block, WHEN the
/// convention test runs, THEN the test SHALL fail and SHALL identify
/// the offending line.
///
/// We exercise the same `scan()` helper that powers the production
/// check, against a synthetic body, so the detector's flagging
/// behaviour is verified independently of the real file's current
/// contents.
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

/// Sibling negative test: `sh - <<` is flagged the same way. Pins the
/// "or equivalent" clause from the spec scenario.
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

/// Confirm pure-comment lines do not trip the lint. This is what allows
/// the explanatory comment in `cmd_status` to reference the forbidden
/// shape while documenting *why* it was replaced.
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

/// Extract a shell function body from `cmd_status_publish() {` up to and
/// including the first bare `}` at column 0 (the function's closing brace).
/// Interior closing braces are always indented or part of a wider token
/// (e.g. `}))` inside the embedded Python), so a line that is exactly `}`
/// marks the function end.
fn extract_function(body: &str, name: &str) -> String {
    let header = format!("{name}() {{");
    let start = body
        .find(&header)
        .unwrap_or_else(|| panic!("function `{name}` not found in sweep.sh"));
    let mut out = String::new();
    for line in body[start..].lines() {
        out.push_str(line);
        out.push('\n');
        if line == "}" {
            break;
        }
    }
    out
}

/// broker-helper-full-surface task 4.6: the widened `cmd_status_publish` SHALL
/// shape its `agent.status` JSON via a `-c "$(cat <<'EOF' … EOF)"` block (stdin
/// stays free) and SHALL NOT reintroduce a stdin-claiming `interpreter - <<`
/// heredoc. The positive assertion also fails if the function reverts to the
/// prior `"${PY}" - <args> <<'PY'` args-heredoc form, since that shape lacks
/// the `-c "$(cat` marker.
#[test]
fn cmd_status_publish_keeps_c_cat_heredoc_shape() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("scripts")
        .join("sweep.sh");
    let body = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("read {}: {e}", path.display());
    });
    let func = extract_function(&body, "cmd_status_publish");

    assert!(
        func.contains("-c \"$(cat <<'"),
        "cmd_status_publish must shape JSON via `-c \"$(cat <<'EOF' … EOF)\"`; body:\n{func}"
    );

    for bad in [
        "\"${PY}\" - <<",
        "${PY} - <<",
        "python3 - <<",
        "python - <<",
    ] {
        assert!(
            !func.contains(bad),
            "cmd_status_publish must not reintroduce the stdin-claiming shape `{bad}`; body:\n{func}"
        );
    }
}

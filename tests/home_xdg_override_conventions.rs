//! Convention enforcement for the `core-ci-hygiene` requirement *Tests
//! invoking git paw subcommands SHALL override HOME and `XDG_DATA_HOME`*.
//!
//! The requirement was previously enforced by a one-time manual audit. It
//! regressed twice — `tests/default_branch_fallback_integration.rs` and
//! `tests/rebase_integration.rs` landed with zero `env("HOME", …)` calls
//! between them, writing roughly a hundred stale session files into the
//! developer's real `~/Library/Application Support/git-paw/sessions/`
//! alongside the live `paw-git-paw.json`. This test makes conformance a
//! standing, mechanical check instead of a reviewer's memory.
//!
//! ## Scope
//!
//! A `git paw` subcommand needs the override only when it can read or write
//! the real sessions directory (`crate::session::sessions_dir()`, which
//! resolves from `HOME`/`XDG_DATA_HOME`). That is: `start`, `stop`, `pause`,
//! `recover`, `add`, `remove`, `attach`, `supervisor`, `purge`, and `status`.
//! `approvals` is deliberately excluded — passing `--session <NAME>` (the
//! pattern every `approvals` test uses) short-circuits
//! `session::find_session_for_repo` entirely, so the subcommand never
//! touches the real directory. Non-session subcommands (`completions`,
//! `classify`, `list-clis`, `init`'s informational session probe, etc.) are
//! out of scope per the capability's non-goal: this check targets the
//! documented harm (writing into / enumerating the user's live session
//! state), not every subprocess invocation of the binary.
//!
//! ## Heuristic
//!
//! Matching `assert_cmd` builders textually has false negatives (an unusual
//! construction) and false positives (a helper that applies the override
//! indirectly). Per the design, false positives are preferred — a spurious
//! failure is cheap to annotate; a missed one writes into the user's real
//! directory.
//!
//! Compliance is checked **per `#[test]` function**, not per file: a file
//! with one compliant test and a later-added test lacking its own override
//! is NOT covered just because an earlier test in the same file happens to
//! apply the pattern — each test fn must itself apply the override, or call
//! a helper (local to the file, or a shared helper under `tests/helpers/`,
//! such as `e2e_tests.rs`'s `cmd_iso` or `pty::create_detached_session`)
//! whose own body does.
//!
//! Two invocation shapes are recognised:
//! - An `assert_cmd::Command` chain ending in `.args([…])`, rooted at
//!   `cmd()` / `Command::cargo_bin("git-paw")` / a tracked local variable.
//! - A PTY/tmux-driven launch: the binary path comes from
//!   `env!("CARGO_BIN_EXE_git-paw")` and is interpolated into a shell-command
//!   string (e.g. `format!("cd '{}' && '{bin}' start --dry-run", …)`) sent
//!   via `send_keys` rather than invoked through an `assert_cmd::Command` at
//!   all — the `.args([` scan alone never sees this shape.
//!
//! A test's own body counts as having the override if it contains at least
//! one `.env("HOME", …)` and one of `.env_remove("XDG_DATA_HOME")` or
//! `.env("XDG_DATA_HOME", …)`. The second form — explicitly pointing
//! `XDG_DATA_HOME` at a path under the same tempdir as `HOME`, as
//! `unattended_drive_e2e.rs`'s `spawn_unattended` helper does — is an
//! equivalent isolation to removal: it guarantees the subprocess's data dir
//! resolves inside the tempdir regardless of the ambient environment, which
//! `env_remove` alone does not (it only restores the HOME-derived fallback,
//! which is what the *absence* of `XDG_DATA_HOME` already means in the
//! developer's real shell, but is not a logical requirement).
//!
//! ## Documented exception
//!
//! A file that genuinely needs the real `HOME` may opt out of this check by
//! including the literal marker comment `git-paw-test: real-home-required`
//! anywhere in the file, together with a comment explaining why. The
//! exception is file-wide (matching the one-time-audit scenario's original
//! scope) rather than per-test — precision there isn't what regressed. No
//! file in the tree uses this marker today (see
//! `no_file_claims_the_real_home_exception_today`); this check exists so an
//! exception stays greppable and reviewable instead of inventing its own
//! silencing convention.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Subcommands that read or write the real sessions directory via
/// `crate::session::{save_session, find_session_for_repo, delete_session,
/// session_state_dir}` and therefore require the `HOME`/`XDG_DATA_HOME` override.
const SESSION_SUBCOMMANDS: &[&str] = &[
    "start",
    "stop",
    "pause",
    "recover",
    "add",
    "remove",
    "attach",
    "supervisor",
    "purge",
    "status",
];

/// Opt-out marker for a file that genuinely needs the real `HOME`.
const EXEMPTION_MARKER: &str = "git-paw-test: real-home-required";

/// Returns `true` if `body` contains the `HOME`/`XDG_DATA_HOME` override pattern
/// somewhere — either inline, or centralized in a per-file or cross-file
/// helper function (the caller is responsible for scoping `body` to the
/// relevant span).
fn has_override_pattern(body: &str) -> bool {
    body.contains(".env(\"HOME\"")
        && (body.contains("env_remove(\"XDG_DATA_HOME\")")
            || body.contains(".env(\"XDG_DATA_HOME\""))
}

/// Extracts the first string literal inside a `.args([` … `])` call starting
/// at byte offset `start` in `body`. Returns `None` if no closing `])` is
/// found within a reasonable window or no string literal is present.
fn first_arg_literal(body: &str, start: usize) -> Option<&str> {
    let window_end = (start + 2000).min(body.len());
    let window = &body[start..window_end];
    let close = window.find("])")?;
    let inner = &window[..close];
    let first_quote = inner.find('"')?;
    let rest = &inner[first_quote + 1..];
    let second_quote = rest.find('"')?;
    Some(&rest[..second_quote])
}

/// `true` if the `.args([` … `])` array starting at byte offset `start` also
/// names `--help` or `--version`. clap intercepts both before the
/// application's own subcommand dispatch runs, so a call like
/// `.args(["start", "--help"])` never reaches `cmd_start` (and therefore
/// never touches the sessions directory) regardless of which subcommand
/// precedes the flag.
fn args_array_requests_help_or_version(body: &str, start: usize) -> bool {
    let window_end = (start + 2000).min(body.len());
    let window = &body[start..window_end];
    let Some(close) = window.find("])") else {
        return false;
    };
    let inner = &window[..close];
    inner.contains("\"--help\"") || inner.contains("\"--version\"")
}

/// Identifiers bound to a `git-paw`-binary `Command` builder somewhere in the
/// file, via `let (mut) <name> = cmd();` or an equivalent direct
/// `cargo_bin("git-paw")` assignment. Used to resolve `.args([` call sites
/// whose receiver is a variable rather than a fresh `cmd()` call — the
/// `let mut command = cmd(); … command.args([...])` shape.
fn collect_git_paw_vars(body: &str) -> HashSet<String> {
    let mut vars = HashSet::new();
    for line in body.lines() {
        let Some(rest) = line.trim_start().strip_prefix("let ") else {
            continue;
        };
        let rest = rest.strip_prefix("mut ").unwrap_or(rest);
        let Some((name, expr)) = rest.split_once('=') else {
            continue;
        };
        let expr = expr.trim_start();
        if expr.starts_with("cmd()") || expr.contains("cargo_bin(\"git-paw\")") {
            vars.insert(name.trim().to_string());
        }
    }
    vars
}

/// Extracts the leading identifier of `line` (the token right after the last
/// `=`, or the first token on the line if there is none) — used to resolve
/// which `Command` a chained call applies to.
fn leading_identifier(line: &str) -> String {
    let after_eq = line.rsplit('=').next().unwrap_or(line);
    after_eq
        .trim_start()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Determines whether the method chain containing `.args([` at line index
/// `args_line` is rooted at a `git-paw`-binary `Command`, by walking backward
/// through continuation lines (lines starting with `.`) to the statement's
/// base, then checking that base against known `git-paw` markers / plain
/// `git`-process markers / tracked variable names.
///
/// Returns `None` when the root can't be determined (favoring a flagged
/// false positive over a missed violation, per this capability's design).
fn chain_is_git_paw_rooted(
    lines: &[&str],
    args_line: usize,
    vars: &HashSet<String>,
) -> Option<bool> {
    let mut i = args_line;
    loop {
        let line = lines[i].trim_start();
        if !line.starts_with('.') {
            if line.contains("cargo_bin(\"git-paw\")") || line.contains("cmd()") {
                return Some(true);
            }
            if line.contains("new(\"git\")") {
                return Some(false);
            }
            let ident = leading_identifier(lines[i]);
            if !ident.is_empty() {
                if vars.contains(&ident) {
                    return Some(true);
                }
                return None;
            }
            return None;
        }
        if i == 0 {
            return None;
        }
        i -= 1;
    }
}

/// A top-level (unindented) `fn` item's source span within a file, as line
/// indices into that file's `lines()`.
struct FnSpan {
    name: String,
    is_test: bool,
    fn_line: usize,
    /// The line containing the function's closing `}` at column 0.
    body_end: usize,
}

/// Prefixes that open a top-level function definition.
const FN_LINE_PREFIXES: &[&str] = &[
    "fn ",
    "pub fn ",
    "pub(crate) fn ",
    "async fn ",
    "pub async fn ",
    "pub(crate) async fn ",
];

/// `true` for a line that opens a top-level (unindented) function
/// definition — rustfmt never indents a top-level item, so "no leading
/// whitespace" reliably distinguishes it from a nested fn/closure.
fn is_top_level_fn_line(line: &str) -> bool {
    if line.is_empty() || line != line.trim_start() {
        return false;
    }
    FN_LINE_PREFIXES.iter().any(|p| line.starts_with(p))
}

fn extract_fn_name(line: &str) -> String {
    line.split("fn ")
        .nth(1)
        .unwrap_or_default()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Splits `lines` into top-level function spans. A function's body is taken
/// to end at the next line that is *exactly* `}` (column 0) — reliable for
/// rustfmt'd code, where nested braces are always indented.
fn function_spans(lines: &[&str]) -> Vec<FnSpan> {
    let mut spans = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if is_top_level_fn_line(lines[i]) {
            let name = extract_fn_name(lines[i]);
            let mut end = lines.len().saturating_sub(1);
            for (j, candidate) in lines.iter().enumerate().skip(i + 1) {
                if *candidate == "}" {
                    end = j;
                    break;
                }
            }
            let mut attr_start = i;
            while attr_start > 0 {
                let prev = lines[attr_start - 1].trim();
                if prev.is_empty()
                    || prev.starts_with("#[")
                    || prev.starts_with("///")
                    || prev.starts_with("//!")
                    || prev.starts_with("//")
                {
                    attr_start -= 1;
                } else {
                    break;
                }
            }
            let is_test = lines[attr_start..i].iter().any(|l| l.trim() == "#[test]");
            spans.push(FnSpan {
                name,
                is_test,
                fn_line: i,
                body_end: end,
            });
            i = end + 1;
        } else {
            i += 1;
        }
    }
    spans
}

fn span_text(lines: &[&str], start: usize, end: usize) -> String {
    lines[start..=end].join("\n")
}

/// Resolves which of a file's own top-level functions are "compliant
/// helpers" — functions whose own body has the override pattern, or that
/// call another compliant helper (local or cross-file). Fixed-point over a
/// few iterations to catch a helper calling another helper.
fn compliant_local_helpers(
    lines: &[&str],
    spans: &[FnSpan],
    cross_file_helpers: &HashSet<String>,
) -> HashSet<String> {
    let mut compliant: HashSet<String> = spans
        .iter()
        .filter(|s| has_override_pattern(&span_text(lines, s.fn_line, s.body_end)))
        .map(|s| s.name.clone())
        .collect();

    for _ in 0..6 {
        let mut changed = false;
        for span in spans {
            if compliant.contains(&span.name) {
                continue;
            }
            let body_text = span_text(lines, span.fn_line, span.body_end);
            let calls_compliant = compliant
                .iter()
                .chain(cross_file_helpers.iter())
                .any(|n| body_text.contains(&format!("{n}(")));
            if calls_compliant {
                compliant.insert(span.name.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    compliant
}

/// Finds PTY/tmux-driven launches of the binary: `env!("CARGO_BIN_EXE_git-paw")`
/// bound to a variable, then interpolated as `'{<var>}' <subcommand>` inside
/// a shell-command string built for `send_keys` rather than invoked through
/// an `assert_cmd::Command`. Returns `(line_idx, subcommand)` for sites
/// naming a session-touching subcommand.
fn find_pty_subcommand_sites(body: &str, lines: &[&str]) -> Vec<(usize, String)> {
    const MARKER: &str = "env!(\"CARGO_BIN_EXE_git-paw\")";
    let mut sites = Vec::new();
    let mut search_from = 0usize;
    while let Some(rel) = body[search_from..].find(MARKER) {
        let pos = search_from + rel;
        let line_idx = body[..pos].matches('\n').count();
        search_from = pos + MARKER.len();

        let var_name = lines[line_idx]
            .split("let ")
            .nth(1)
            .and_then(|rest| rest.split('=').next())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("bin")
            .to_string();

        let needle = format!("{{{var_name}}}'");
        let window_end = (pos + 4000).min(body.len());
        if let Some(nrel) = body[pos..window_end].find(&needle) {
            let after = &body[pos + nrel + needle.len()..];
            let word: String = after
                .trim_start()
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if SESSION_SUBCOMMANDS.contains(&word.as_str()) {
                sites.push((line_idx, word));
            }
        }
    }
    sites
}

/// Scans a test-file body for `#[test]` functions that invoke a
/// session-touching `git paw` subcommand without applying the
/// `HOME`/`XDG_DATA_HOME` override (inline, or via a local or cross-file
/// helper). Returns one diagnostic per matched site, naming the file, the
/// offending test fn, and the construction it matched.
fn scan(filename: &str, body: &str, cross_file_helpers: &HashSet<String>) -> Vec<String> {
    if body.contains(EXEMPTION_MARKER) {
        return Vec::new();
    }

    let lines: Vec<&str> = body.lines().collect();
    let spans = function_spans(&lines);
    let vars = collect_git_paw_vars(body);
    let local_compliant = compliant_local_helpers(&lines, &spans, cross_file_helpers);
    let pty_sites = find_pty_subcommand_sites(body, &lines);

    let mut hits = Vec::new();

    for span in spans.iter().filter(|s| s.is_test) {
        let body_text = span_text(&lines, span.fn_line, span.body_end);
        let is_compliant = has_override_pattern(&body_text)
            || local_compliant.contains(&span.name)
            || cross_file_helpers
                .iter()
                .any(|n| body_text.contains(&format!("{n}(")));
        if is_compliant {
            continue;
        }

        let mut search_from = 0usize;
        while let Some(rel) = body[search_from..].find(".args([") {
            let pos = search_from + rel;
            let line_idx = body[..pos].matches('\n').count();
            search_from = pos + ".args([".len();
            if line_idx < span.fn_line || line_idx > span.body_end {
                continue;
            }
            if let Some(literal) = first_arg_literal(body, pos)
                && SESSION_SUBCOMMANDS.contains(&literal)
                && !args_array_requests_help_or_version(body, pos)
                && chain_is_git_paw_rooted(&lines, line_idx, &vars) != Some(false)
            {
                hits.push(format!(
                    "{filename}:{}: fn `{}` — `.args([\"{literal}\", …])` invokes a \
                     session-touching git paw subcommand but neither this test nor a \
                     helper it calls applies the HOME/XDG_DATA_HOME override (no \
                     `.env(\"HOME\", …)` + `.env_remove(\"XDG_DATA_HOME\")` pair, and no \
                     `{EXEMPTION_MARKER}` exception comment)",
                    line_idx + 1,
                    span.name
                ));
            }
        }

        for (line_idx, subcommand) in &pty_sites {
            if *line_idx < span.fn_line || *line_idx > span.body_end {
                continue;
            }
            hits.push(format!(
                "{filename}:{}: fn `{}` — launches the binary via \
                 `CARGO_BIN_EXE_git-paw` with subcommand `{subcommand}` (a PTY/tmux-driven \
                 invocation, not an assert_cmd::Command) but neither this test nor a helper \
                 it calls applies the HOME/XDG_DATA_HOME override, and no \
                 `{EXEMPTION_MARKER}` exception comment is present",
                line_idx + 1,
                span.name
            ));
        }
    }
    hits
}

/// Collects names of functions defined under `tests/helpers/` whose own body
/// contains the `HOME`/`XDG_DATA_HOME` override pattern — e.g.
/// `pty::create_detached_session`. A test file that calls such a helper by
/// name is treated as covered, since the override lives in the shared
/// module rather than inline at (or local-helper-reachable from) the call
/// site.
fn collect_cross_file_helpers() -> HashSet<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("helpers");
    let mut compliant = HashSet::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return compliant;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let Ok(body) = fs::read_to_string(&path) else {
            continue;
        };
        let lines: Vec<&str> = body.lines().collect();
        for span in function_spans(&lines) {
            if has_override_pattern(&span_text(&lines, span.fn_line, span.body_end)) {
                compliant.insert(span.name);
            }
        }
    }
    compliant
}

/// Reads every top-level file in `tests/` (skipping subdirectories like
/// `tests/helpers/` and `tests/fixtures/`, and this file itself) and returns
/// `(filename, body)` pairs.
fn test_files() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).expect("read tests/ dir") {
        let entry = entry.expect("read dir entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        if filename == "home_xdg_override_conventions.rs" {
            continue;
        }
        let body =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        out.push((filename, body));
    }
    out
}

#[test]
fn every_session_touching_subcommand_test_overrides_home_and_xdg() {
    let cross_file_helpers = collect_cross_file_helpers();
    let mut all_hits = Vec::new();
    for (filename, body) in test_files() {
        all_hits.extend(scan(&filename, &body, &cross_file_helpers));
    }

    assert!(
        all_hits.is_empty(),
        "the following `git paw` subcommand invocations lack the HOME/XDG_DATA_HOME \
         override (core-ci-hygiene: \"Tests invoking git paw subcommands SHALL override \
         HOME and XDG_DATA_HOME\"):\n{}",
        all_hits.join("\n")
    );
}

/// `tests/helpers/pty.rs`'s `create_detached_session` is the one cross-file
/// helper in the tree today that PTY-driven tests rely on for isolation.
/// Pins that the scanner's cross-file collector actually recognises it —
/// without this, every `cli_prompt_matrix.rs`/`init_interactive_specs.rs`
/// caller would be (incorrectly) flagged.
#[test]
fn create_detached_session_is_recognised_as_a_compliant_cross_file_helper() {
    let helpers = collect_cross_file_helpers();
    assert!(
        helpers.contains("create_detached_session"),
        "tests/helpers/pty.rs's create_detached_session must apply the HOME/XDG_DATA_HOME \
         override so PTY-driven callers are recognised as covered; found: {helpers:?}"
    );
}

/// Spec scenario `A test file without the override fails the suite`: a
/// synthetic file with a `start` invocation and no override pattern is
/// flagged, naming the file and the matched construction.
#[test]
fn detector_flags_a_file_with_no_override_pattern() {
    let synthetic = "\
fn cmd() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin(\"git-paw\").unwrap()
}

#[test]
fn starts_without_override() {
    let _ = cmd()
        .args([\"start\", \"--cli\", \"sh\", \"--branches\", \"feat/x\"])
        .output()
        .unwrap();
}
";
    let hits = scan("synthetic_offender.rs", synthetic, &HashSet::new());
    assert!(
        !hits.is_empty(),
        "detector should flag a file with a `start` invocation and no override pattern"
    );
    assert!(
        hits.iter().any(|h| h.contains("synthetic_offender.rs")),
        "diagnostic should name the offending file; got: {hits:?}"
    );
    assert!(
        hits.iter().any(|h| h.contains("\"start\"")),
        "diagnostic should name the matched subcommand; got: {hits:?}"
    );
}

/// Spec scenario (3.5): the same synthetic body, once the override pattern
/// is restored, passes — proving the detector clears when the fix is
/// applied, not just that it fires when the fix is missing.
#[test]
fn detector_clears_once_override_pattern_is_restored() {
    let synthetic = "\
fn cmd() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin(\"git-paw\").unwrap()
}

#[test]
fn starts_with_override() {
    let fake_home = tempfile::TempDir::new().unwrap();
    let _ = cmd()
        .env(\"HOME\", fake_home.path())
        .env_remove(\"XDG_DATA_HOME\")
        .args([\"start\", \"--cli\", \"sh\", \"--branches\", \"feat/x\"])
        .output()
        .unwrap();
}
";
    let hits = scan("synthetic_fixed.rs", synthetic, &HashSet::new());
    assert!(
        hits.is_empty(),
        "detector should not flag a file once the override pattern is present; got: {hits:?}"
    );
}

/// Post-review gap: compliance was checked at the whole-file level, so a
/// file with one compliant test and a second, later-added test lacking its
/// own override was invisible — the compliant test's pattern covered the
/// whole file. Per-function checking must flag the new test on its own,
/// without flagging the already-compliant one.
#[test]
fn detector_flags_a_new_noncompliant_test_fn_added_to_an_otherwise_compliant_file() {
    let synthetic = "\
fn cmd() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin(\"git-paw\").unwrap()
}

#[test]
fn existing_compliant_test() {
    let fake_home = tempfile::TempDir::new().unwrap();
    let _ = cmd()
        .env(\"HOME\", fake_home.path())
        .env_remove(\"XDG_DATA_HOME\")
        .args([\"start\", \"--cli\", \"sh\", \"--branches\", \"feat/x\"])
        .output()
        .unwrap();
}

#[test]
fn newly_added_noncompliant_test() {
    let _ = cmd().args([\"stop\"]).output().unwrap();
}
";
    let hits = scan("synthetic_mixed.rs", synthetic, &HashSet::new());
    assert!(
        !hits.is_empty(),
        "a new non-compliant test fn must be flagged even though an earlier test in the \
         same file already applies the override; got: {hits:?}"
    );
    assert!(
        hits.iter()
            .any(|h| h.contains("newly_added_noncompliant_test")),
        "diagnostic should name the offending test fn; got: {hits:?}"
    );
    assert!(
        !hits.iter().any(|h| h.contains("existing_compliant_test")),
        "the already-compliant test must not be flagged; got: {hits:?}"
    );
}

/// Post-review gap: a PTY/tmux-driven test launches the binary as a
/// shell-command string built from `env!("CARGO_BIN_EXE_git-paw")`, not an
/// `assert_cmd::Command` builder — the `.args([` scan alone never sees it.
#[test]
fn detector_flags_a_pty_launch_via_cargo_bin_exe_without_override() {
    let synthetic = "\
#[test]
fn start_shown_in_pty_without_override() {
    let session = \"paw-matrix-x\";
    let bin = env!(\"CARGO_BIN_EXE_git-paw\");
    let cmd = format!(\"cd '{}' && '{bin}' start --dry-run\", \"/tmp/repo\");
    send_keys(session, &[&cmd, \"Enter\"]);
}
";
    let hits = scan("synthetic_pty.rs", synthetic, &HashSet::new());
    assert!(
        !hits.is_empty(),
        "a CARGO_BIN_EXE_git-paw launch naming a session-touching subcommand must be \
         flagged when the test has no HOME/XDG_DATA_HOME override; got: {hits:?}"
    );
    assert!(
        hits.iter()
            .any(|h| h.contains("start_shown_in_pty_without_override")),
        "diagnostic should name the offending test fn; got: {hits:?}"
    );
}

/// The PTY-launch shape clears once the test calls a known cross-file helper
/// that applies the override — mirrors `create_detached_session`'s fix in
/// `tests/helpers/pty.rs`.
#[test]
fn detector_clears_pty_launch_once_a_compliant_cross_file_helper_is_called() {
    let synthetic = "\
#[test]
fn start_shown_in_pty_with_override() {
    let home = tempfile::TempDir::new().unwrap();
    let session = \"paw-matrix-x\";
    create_detached_session(session, home.path());
    let bin = env!(\"CARGO_BIN_EXE_git-paw\");
    let cmd = format!(\"cd '{}' && '{bin}' start --dry-run\", \"/tmp/repo\");
    send_keys(session, &[&cmd, \"Enter\"]);
}
";
    let mut cross_file = HashSet::new();
    cross_file.insert("create_detached_session".to_string());
    let hits = scan("synthetic_pty_fixed.rs", synthetic, &cross_file);
    assert!(
        hits.is_empty(),
        "a PTY launch must clear once it calls a known cross-file helper that applies the \
         override; got: {hits:?}"
    );
}

/// A documented exception (the `{EXEMPTION_MARKER}` marker) suppresses the
/// check for that file, even with an unresolved `start` call.
#[test]
fn detector_honours_the_documented_exception_marker() {
    let synthetic = "\
// git-paw-test: real-home-required — this test exercises resolving the
// user's actual home directory and cannot use a fake one.
fn cmd() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin(\"git-paw\").unwrap()
}

#[test]
fn starts_against_real_home() {
    let _ = cmd().args([\"start\"]).output().unwrap();
}
";
    let hits = scan("synthetic_exempted.rs", synthetic, &HashSet::new());
    assert!(
        hits.is_empty(),
        "a file carrying the documented-exception marker must not be flagged; got: {hits:?}"
    );
}

/// Non-session subcommands (e.g. `completions`) never require the override,
/// even with zero HOME/XDG handling in the file.
#[test]
fn detector_ignores_non_session_subcommands() {
    let synthetic = "\
fn cmd() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin(\"git-paw\").unwrap()
}

#[test]
fn lists_completions() {
    let _ = cmd().args([\"completions\", \"bash\"]).output().unwrap();
}
";
    let hits = scan("synthetic_completions.rs", synthetic, &HashSet::new());
    assert!(
        hits.is_empty(),
        "non-session subcommands must not be flagged; got: {hits:?}"
    );
}

/// A session-touching subcommand paired with `--help` (or `--version`) is
/// never flagged — clap intercepts both before the application's own
/// dispatch runs, so `start --help` never reaches `cmd_start` regardless of
/// `HOME`/`XDG_DATA_HOME`.
#[test]
fn detector_ignores_help_and_version_invocations() {
    let synthetic = "\
fn cmd() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin(\"git-paw\").unwrap()
}

#[test]
fn start_help_shows_flags() {
    let _ = cmd().args([\"start\", \"--help\"]).output().unwrap();
}

#[test]
fn purge_version_prints_version() {
    let _ = cmd().args([\"purge\", \"--version\"]).output().unwrap();
}
";
    let hits = scan("synthetic_help.rs", synthetic, &HashSet::new());
    assert!(
        hits.is_empty(),
        "a subcommand invocation naming --help or --version must not be flagged; got: {hits:?}"
    );
}

/// Confirms the exemption marker is not in use anywhere in the real tree
/// today — the capability's own scenario records that no such case exists
/// yet, and this keeps that fact mechanically checked rather than asserted
/// in prose.
#[test]
fn no_file_claims_the_real_home_exception_today() {
    let offenders: Vec<String> = test_files()
        .into_iter()
        .filter(|(_, body)| body.contains(EXEMPTION_MARKER))
        .map(|(filename, _)| filename)
        .collect();
    assert!(
        offenders.is_empty(),
        "no test file should need the real-HOME exception today; found it in: {offenders:?}"
    );
}

//! Static source-audit tests.
//!
//! These tests read `src/main.rs` as a string and assert structural
//! properties of named functions. They are runtime tests, not compile-time
//! checks — `cargo test` invokes them like any other `#[test]` function.
//!
//! Maps to scenarios from the v0.5.0 archived spec set; see
//! `openspec/changes/test-coverage-v0-5-0/tasks.md` 12.9 and 13.1.

use std::path::Path;

const MAIN_RS: &str = include_str!("../src/main.rs");

/// Returns the function body for the named function in `src/main.rs`, located
/// by its `fn <name>(` signature and parsed by walking the matching curly
/// brace from the opening `{`. Panics if the function is not found.
fn function_body(name: &str) -> &'static str {
    let signature = format!("fn {name}(");
    let start = MAIN_RS
        .find(&signature)
        .unwrap_or_else(|| panic!("`fn {name}(` not found in src/main.rs"));
    let body_start = MAIN_RS[start..].find('{').map_or_else(
        || panic!("opening brace not found after `fn {name}(`"),
        |o| start + o,
    );
    let mut depth: i32 = 0;
    for (i, ch) in MAIN_RS[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &MAIN_RS[body_start..=body_start + i];
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced braces while extracting `fn {name}(` body");
}

// Maps to scenario `cmd_supervisor does NOT call the Rust merge loop` from
// supervisor-as-pane. The merge loop was removed in v0.5.0 in favour of the
// supervisor skill orchestrating merges. cmd_supervisor MUST NOT invoke the
// Rust-side `run_merge_loop`. (test-coverage-v0-5-0 task 12.9)
#[test]
fn cmd_supervisor_does_not_reference_run_merge_loop() {
    let body = function_body("cmd_supervisor");
    assert!(
        !body.contains("run_merge_loop"),
        "cmd_supervisor body must not reference the removed `run_merge_loop` symbol"
    );
}

// Maps to scenario `Auto-approve thread runs inside the dashboard subprocess`
// from supervisor-as-pane (per design.md D2). The structural property is
// asserted at the source level: cmd_supervisor MUST NOT call the
// auto-approve spawner (`spawn_auto_approve_thread`). The spawner runs
// inside `cmd_dashboard` only. (test-coverage-v0-5-0 task 13.1)
#[test]
fn cmd_supervisor_does_not_spawn_auto_approve_thread() {
    let body = function_body("cmd_supervisor");
    assert!(
        !body.contains("spawn_auto_approve_thread"),
        "cmd_supervisor body must not call the auto-approve spawner; that thread \
         runs inside cmd_dashboard's __dashboard subprocess"
    );

    // Sanity: the spawner must still exist somewhere in main.rs (otherwise
    // the grep target has vanished and this test silently passes).
    assert!(
        MAIN_RS.contains("fn spawn_auto_approve_thread("),
        "spawn_auto_approve_thread function is missing — update this test if it was renamed"
    );
}

// Defence in depth: make sure this test file actually opens the right
// source — if `src/main.rs` ever moves, `include_str!` will fail at
// compile time. This is a smoke assertion the body is non-empty.
#[test]
fn main_rs_source_is_non_empty() {
    assert!(!MAIN_RS.is_empty(), "src/main.rs should not be empty");
    // The constant is referenced so the linter doesn't strip it.
    assert!(
        Path::new("src/main.rs")
            .to_string_lossy()
            .contains("main.rs")
    );
}

// supervisor-as-pane: cmd_supervisor must not self-publish a
// `agent.status` for the supervisor itself. The supervisor pane (the
// human's CLI session) is responsible for publishing its own
// registration as the first action in its skill. A launcher-side
// publish would re-create the phantom-row regression the change
// eliminated. v0-5-0-audit-cleanup task 5.4.
#[test]
fn cmd_supervisor_does_not_publish_supervisor_status() {
    let body = function_body("cmd_supervisor");

    // The two substrings — `publish_to_broker_http(` (the HTTP publish
    // call) and `build_status_message("supervisor"` (the supervisor-
    // targeted status builder) — must not co-occur inside
    // cmd_supervisor's body. Either alone is fine elsewhere; their
    // pairing inside cmd_supervisor is the regression shape.
    let has_publish = body.contains("publish_to_broker_http(");
    let has_supervisor_status = body.contains("build_status_message(\"supervisor\"");
    assert!(
        !(has_publish && has_supervisor_status),
        "cmd_supervisor body must not pair `publish_to_broker_http(` with \
         `build_status_message(\"supervisor\"`; the supervisor pane self-registers"
    );
}

// supervisor-as-pane: with an empty agents snapshot the dashboard must
// not render a supervisor row or a pinned-supervisor divider — the
// supervisor only appears once it has self-registered. v0-5-0-audit-
// cleanup task 5.5.
#[test]
fn dashboard_renders_no_supervisor_row_for_empty_snapshot() {
    let rows = git_paw::dashboard::format_agent_rows(&[], std::time::Instant::now());
    assert!(
        rows.is_empty(),
        "format_agent_rows on an empty snapshot must produce zero rows, got {} rows",
        rows.len(),
    );

    let arranged = git_paw::dashboard::arrange_with_supervisor_pinned(rows.clone());
    assert!(
        arranged.is_empty(),
        "arrange_with_supervisor_pinned on an empty row slice must produce zero entries (no divider), got {} entries",
        arranged.len(),
    );

    // Also assert no entry mentions the literal substring "supervisor"
    // anywhere — defence-in-depth in case a future row factory
    // synthesises a supervisor row from an empty snapshot.
    let rendered_supervisor = rows
        .iter()
        .any(|r| r.agent_id.contains("supervisor") || r.summary.contains("supervisor"));
    assert!(
        !rendered_supervisor,
        "empty snapshot must not produce any row mentioning 'supervisor'",
    );

    // And confirm no Divider row is present in the arranged output.
    let has_divider = arranged
        .iter()
        .any(|r| matches!(r, git_paw::dashboard::AgentTableRow::Divider));
    assert!(
        !has_divider,
        "empty snapshot must not produce a divider row",
    );
}

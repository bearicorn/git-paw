//! Static source-audit tests.
//!
//! These tests read a source file (`src/commands/supervisor.rs`,
//! `src/dashboard.rs`) as a string and assert structural properties of named
//! functions. They are runtime tests, not compile-time checks — `cargo test`
//! invokes them like any other `#[test]` function.
//!
//! Maps to scenarios from the v0.5.0 archived spec set (see
//! `openspec/changes/test-coverage-v0-5-0/tasks.md` 12.9 and 13.1) and to the
//! `dashboard-mvu` frozen-surface scenario.

const SUPERVISOR_RS: &str = include_str!("../src/commands/supervisor.rs");
const DASHBOARD_RS: &str = include_str!("../src/dashboard.rs");
const DASHBOARD_VIEW_RS: &str = include_str!("../src/dashboard/view.rs");

/// Returns the function body for the named function in `src`, located by its
/// `fn <name>(` (or `fn <name><…>(`, for a generic function) signature and
/// parsed by walking the matching curly brace from the opening `{`. `path`
/// names the file for panic messages. Panics if the function is not found.
fn function_body_in(src: &'static str, path: &str, name: &str) -> &'static str {
    let start = src
        .find(&format!("fn {name}("))
        .or_else(|| src.find(&format!("fn {name}<")))
        .unwrap_or_else(|| panic!("`fn {name}(` not found in {path}"));
    let body_start = src[start..].find('{').map_or_else(
        || panic!("opening brace not found after `fn {name}(`"),
        |o| start + o,
    );
    let mut depth: i32 = 0;
    for (i, ch) in src[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[body_start..=body_start + i];
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced braces while extracting `fn {name}(` body");
}

/// Returns the function body for the named function in
/// `src/commands/supervisor.rs`.
fn function_body(name: &str) -> &'static str {
    function_body_in(SUPERVISOR_RS, "src/commands/supervisor.rs", name)
}

/// Returns the function body for the named function in `src/dashboard.rs`.
fn dashboard_body(name: &str) -> &'static str {
    function_body_in(DASHBOARD_RS, "src/dashboard.rs", name)
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

    // Sanity: the spawner must still exist somewhere in supervisor.rs (otherwise
    // the grep target has vanished and this test silently passes).
    assert!(
        SUPERVISOR_RS.contains("fn spawn_auto_approve_thread("),
        "spawn_auto_approve_thread function is missing — update this test if it was renamed"
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
        .any(|r| r.agent_id.contains("supervisor") || r.status.contains("supervisor"));
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

// ---------------------------------------------------------------------------
// dashboard-mvu: the frozen SIGHUP / poll surface
//
// Maps to scenario `The SIGHUP and poll FFI are untouched`. That scenario is a
// source-inspection assertion — "WHEN src/dashboard.rs is inspected" — so it
// belongs here rather than in a behavioural test. The MVU refactor moved the
// draw/format helpers into `src/dashboard/view.rs` and routed state changes
// through `update`; these tests pin the pieces it must NOT have touched.
// ---------------------------------------------------------------------------

#[test]
fn dashboard_orphan_exit_still_polls_getppid_for_reparent_to_init() {
    let body = dashboard_body("orphaned");
    assert!(
        body.contains("fn getppid() -> i32;"),
        "`orphaned` must still declare the `getppid` FFI; got:\n{body}"
    );
    assert!(
        body.contains("== 1"),
        "`orphaned` must still compare the parent PID against init (1); got:\n{body}"
    );
}

#[test]
fn dashboard_poll_tty_still_detects_hangup_and_honours_its_timeout() {
    let body = dashboard_body("poll_tty");
    for bit in ["POLLIN", "POLLERR", "POLLHUP", "POLLNVAL"] {
        assert!(
            body.contains(bit),
            "`poll_tty` must still test the {bit} poll(2) bit; got:\n{body}"
        );
    }
    assert!(
        body.contains("TtyPoll::HangUp"),
        "`poll_tty` must still report a hang-up rather than trapping the loop"
    );
    assert!(
        body.contains("timeout.as_millis()"),
        "`poll_tty` must still honour its caller's timeout"
    );
}

#[test]
fn dashboard_draw_loop_keeps_its_poll_cadence_and_event_drain_bounds() {
    // The 800ms cadence is the CPU-leak fix; the 32-event drain bound and the
    // zero-timeout inner poll are the pre-refactor input path.
    assert!(
        DASHBOARD_RS.contains("const TICK_INTERVAL: Duration = Duration::from_millis(800);"),
        "the idle redraw cadence must stay 800ms"
    );

    let body = dashboard_body("run_dashboard_with_panes");
    for fragment in [
        "poll_tty(TICK_INTERVAL)",
        "for _ in 0..32",
        "event::poll(Duration::ZERO)",
        "event::read()",
        "TtyPoll::HangUp",
        "should_exit(",
        "orphaned(),",
    ] {
        assert!(
            body.contains(fragment),
            "the draw loop must still contain `{fragment}` — the frozen I/O surface"
        );
    }
}

#[test]
fn dashboard_draw_loop_routes_events_through_update_not_inline_mutation() {
    // The other half of the scenario: the loop still drains crossterm events
    // exactly as before, "only routing them through `update`". Inline mutation
    // of the redraw state must be gone from the loop body.
    let body = dashboard_body("run_dashboard_with_panes");
    for fragment in [
        "Msg::Key(key.code)",
        "Msg::Snapshot",
        "Msg::BrokerIngest",
        "view::render(f, &model)",
    ] {
        assert!(
            body.contains(fragment),
            "the draw loop must route events through the Update as `{fragment}`"
        );
    }
    for inline in [
        "broker_log::handle_key(",
        "format_agent_rows(",
        "format_status_line(",
        "draw_frame(",
    ] {
        assert!(
            !body.contains(inline),
            "the draw loop must not mutate/render inline any more — found `{inline}`"
        );
    }
}

#[test]
fn dashboard_update_performs_no_terminal_or_process_io() {
    // Maps to the `Update applies a message exactly as the old inline mutation
    // did` scenario's second clause: `update` SHALL be pure.
    let body = dashboard_body("update");
    for io in [
        "terminal",
        "event::",
        "poll_tty",
        "Instant::now()",
        "delivery::",
        "std::process",
        "println!",
    ] {
        assert!(
            !body.contains(io),
            "`update` must be pure — found the I/O marker `{io}` in its body"
        );
    }
}

#[test]
fn dashboard_view_reads_only_the_model_and_performs_no_io() {
    // Maps to the `The View renders purely from the Model` scenario's second
    // clause: no I/O, no `poll_tty`, no terminal side effects.
    for io in [
        "poll_tty",
        "crossterm::event",
        "enable_raw_mode",
        "disable_raw_mode",
        "EnterAlternateScreen",
        "LeaveAlternateScreen",
        "std::process",
        "delivery::agent_status_snapshot",
    ] {
        // The View's own test module constructs broker state, so restrict the
        // audit to the production half of the file.
        let prod = DASHBOARD_VIEW_RS
            .split("#[cfg(test)]")
            .next()
            .expect("view.rs must have a production section");
        assert!(
            !prod.contains(io),
            "`src/dashboard/view.rs` must be pure — found the I/O marker `{io}`"
        );
    }
}

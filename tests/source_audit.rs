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

/// Spec scenario "An attended session runs no auto-approver"
/// (`approval-auto`), source half: the dashboard auto-approve thread is GONE,
/// not merely disabled. None of its symbols may exist anywhere under `src/`,
/// so no future wiring can resurrect it by accident.
///
/// A source-level audit, because the property is a NEGATIVE about the whole
/// tree: a behavioural test can only show that one particular session did not
/// approve, never that no code path could.
#[test]
fn no_auto_approve_thread_symbol_exists_under_src() {
    let mut offenders: Vec<String> = Vec::new();
    for symbol in [
        "spawn_auto_approve_thread",
        "auto_approve_pane",
        "tick_from_status",
    ] {
        for (path, body) in src_files() {
            if body.contains(symbol) {
                offenders.push(format!("{symbol} in {path}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the retired auto-approve thread's symbols must not exist under src/: {offenders:?}"
    );
}

/// Walks `src/` and yields every `.rs` file's path and contents.
fn src_files() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs")
                && let Ok(body) = std::fs::read_to_string(&path)
            {
                out.push((path.display().to_string(), body));
            }
        }
    }
    let mut out = Vec::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut out,
    );
    assert!(!out.is_empty(), "src/ must contain Rust files");
    out
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

/// Spec scenario "No permission-prompt message variant exists"
/// (`approval-auto`, requirement *Approval gate reuses existing broker
/// variants*): escalating a permission prompt rides on `agent.question`, and
/// the stuck-on-prompt trigger on `agent.status`. No wire variant is
/// dedicated to prompts or approvals.
///
/// A source-level audit because the property is a closed-set negative over
/// the enum: a behavioural test can show that today's escalation uses
/// `agent.question`, but not that no OTHER variant exists for the purpose.
#[test]
fn no_broker_message_variant_is_dedicated_to_permission_prompts() {
    let messages = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/broker/messages.rs"),
    )
    .expect("messages.rs is readable");

    // The wire name of every variant, read from its serde rename.
    let wire_names: Vec<String> = messages
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix("#[serde(rename = \"")?;
            let name = rest.strip_suffix("\")]")?;
            name.starts_with("agent.").then(|| name.to_string())
        })
        .collect();
    assert!(
        wire_names.len() >= 8,
        "expected to find the agent.* wire names, got {wire_names:?}"
    );

    for name in &wire_names {
        for forbidden in ["approv", "permission", "prompt"] {
            assert!(
                !name.contains(forbidden),
                "no BrokerMessage variant may be dedicated to permission prompts, \
                 found {name:?}; escalation reuses agent.question and the \
                 stuck-on-prompt trigger reuses agent.status"
            );
        }
    }
    // The two variants the escalation path actually rides on must exist.
    assert!(wire_names.iter().any(|n| n == "agent.question"));
    assert!(wire_names.iter().any(|n| n == "agent.status"));
}

// ---------------------------------------------------------------------------
// `approval-command-safety` / "No permission grants are seeded into agent CLI
// settings files": nothing but the `setup-cli` command may reach the writing
// layer.
// ---------------------------------------------------------------------------

/// Every file in `src/` that could call into the setup writers, with its
/// repo-relative path.
fn source_files() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                out.push((path.to_string_lossy().to_string(), text));
            }
        }
    }
    let mut out = Vec::new();
    walk(std::path::Path::new("src"), &mut out);
    assert!(out.len() > 20, "the source walk found almost nothing");
    out
}

/// The scenario "Recovery and the drive loop write no setup entries" names two
/// callers, but the guarantee is broader: **no** caller other than the
/// `setup-cli` command may apply setup items or write the setup record.
///
/// An end-to-end test can only drive the paths it thinks to drive — it would
/// miss a new one. This asserts the property at the source level instead, so
/// recovery, the drive loop, the dashboard, the MCP server and anything added
/// later are all covered at once: only the use case and its entry layer may
/// name the writers.
#[test]
fn only_the_setup_cli_command_reaches_the_setup_writers() {
    // The functions that actually mutate a settings file or the record.
    const WRITERS: &[&str] = &["shell::apply", "shell::apply_undo", "shell::save_record"];
    // Where those calls are legitimate: the writers themselves, the use case,
    // and the entry layer that owns the prompt.
    const ALLOWED: &[&str] = &[
        "src/cli_setup/shell.rs",
        "src/cli_setup/mod.rs",
        "src/commands/setup_cli.rs",
    ];

    for (path, text) in source_files() {
        let normalised = path.replace('\\', "/");
        if ALLOWED.iter().any(|a| normalised.ends_with(a)) {
            continue;
        }
        // The whole file is checked, not just the part before the first
        // `#[cfg(test)]`: splitting there would leave everything after a test
        // module unchecked, which is exactly where a planted call would hide.
        // A module that legitimately drives the writers belongs in ALLOWED.
        for writer in WRITERS {
            assert!(
                !text.contains(writer),
                "{normalised} calls `{writer}` — only `git paw setup-cli` may write an \
                 agent CLI's settings or the setup record. If this is deliberate, the \
                 capability's central promise has changed and the spec must change first."
            );
        }
    }
}

/// The same property from the other side: the use case is only ever reached
/// from the command's entry layer, never from a launch, recovery or drive-loop
/// path.
#[test]
fn no_launch_or_drive_path_invokes_the_setup_use_case() {
    for (path, text) in source_files() {
        let normalised = path.replace('\\', "/");
        if normalised.ends_with("src/main.rs")
            || normalised.ends_with("src/commands/setup_cli.rs")
            || normalised.ends_with("src/commands/mod.rs")
        {
            continue;
        }
        assert!(
            !text.contains("setup_cli::"),
            "{normalised} references the setup use case — running setup is the user's \
             explicit act, never a side effect of another command"
        );
    }
}

/// `broker-host-separation`: a session has a second window (the broker
/// host's `broker` window), and `tmux list-panes -t <session>` lists only the
/// CURRENT window. Every pane listing in production code must therefore name
/// the agent window (`<session>:0`) — a bare session target sees only the
/// host's pane once someone selects that window, which reads as "every agent
/// exited" and recovers (kills) a live session.
///
/// Rust: the argument after `"-t"` that follows each `"list-panes"` outside
/// test modules must name window 0. Shell: every `tmux list-panes -t` in the
/// bundled scripts must target `:0`.
#[test]
fn pane_listings_name_the_agent_window_not_the_bare_session() {
    let mut offenders = Vec::new();
    for (path, body) in src_files() {
        if path.ends_with("tests.rs") {
            continue;
        }
        // Production code only: stop at the file's test module.
        let production = body.split("#[cfg(test)]").next().unwrap_or(&body);
        let mut rest = production;
        while let Some(at) = rest.find("\"list-panes\"") {
            let after = &rest[at..];
            let window: String = after.chars().take(800).collect();
            if let Some(t) = window.find("\"-t\"") {
                let target_line = window[t + 4..]
                    .trim_start_matches([',', ' ', '\n'])
                    .lines()
                    .find(|l| !l.trim().starts_with("//"))
                    .unwrap_or("");
                if !target_line.contains(":0") {
                    offenders.push(format!("{path}: list-panes -t {}", target_line.trim()));
                }
            }
            rest = &after["\"list-panes\"".len()..];
        }
    }

    let scripts = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts");
    for entry in std::fs::read_dir(&scripts)
        .expect("read assets/scripts")
        .flatten()
    {
        let path = entry.path();
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (n, line) in body.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with('#') {
                continue;
            }
            if let Some(at) = code.find("tmux list-panes -t ") {
                let target = code[at + "tmux list-panes -t ".len()..]
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                if !target.contains(":0") {
                    offenders.push(format!("{}:{}: {}", path.display(), n + 1, code));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "pane listings must target the agent window `<session>:0`, not the bare \
         session (which lists only the current window):\n{}",
        offenders.join("\n")
    );
}

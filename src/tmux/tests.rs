use super::*;
use crate::error::PawError;
use std::path::PathBuf;
use std::time::Duration;

fn make_pane(branch: &str, worktree: &str, cli: &str) -> PaneSpec {
    PaneSpec {
        branch: branch.to_owned(),
        worktree: worktree.to_owned(),
        cli_command: cli.to_owned(),
    }
}

/// Helper: extract command strings matching a keyword from a session's commands.
fn commands_containing(cmds: &[String], keyword: &str) -> Vec<String> {
    cmds.iter()
        .filter(|c| c.contains(keyword))
        .cloned()
        .collect()
}

// -----------------------------------------------------------------------
// AC: Checks tmux presence with actionable error
// Behavioral: verifies the public contract — does the system detect tmux?
// -----------------------------------------------------------------------

#[test]
#[serial_test::serial]
fn ensure_tmux_installed_succeeds_when_present() {
    // Requires #[serial] because detect tests modify PATH.
    assert!(ensure_tmux_installed().is_ok());
}

// -----------------------------------------------------------------------
// AC: Creates named sessions, handles collision
// Behavioral: session name is a public field used by attach, status, and
// dry-run output. The exact naming convention is the public contract.
// -----------------------------------------------------------------------

#[test]
fn session_is_named_after_project() {
    let session = TmuxSessionBuilder::new("my-project")
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .build()
        .unwrap();

    assert_eq!(session.name, "paw-my-project");
}

#[test]
fn session_creation_command_uses_session_name() {
    let session = TmuxSessionBuilder::new("app")
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    assert!(
        cmds.iter()
            .any(|c| c.contains("new-session") && c.contains("paw-app")),
        "should create a tmux session named paw-app"
    );
}

/// AC: Session creation passes explicit dimensions for headless environments
/// — basic builder.
#[test]
fn new_session_passes_explicit_x_and_y() {
    let session = TmuxSessionBuilder::new("app")
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    let new_session_cmd = cmds
        .iter()
        .find(|c| c.contains("new-session"))
        .expect("new-session command present");
    assert!(
        new_session_cmd.contains("-x 480"),
        "new-session must pass -x 480; got: {new_session_cmd}"
    );
    assert!(
        new_session_cmd.contains("-y 140"),
        "new-session must pass -y 140; got: {new_session_cmd}"
    );
}

/// AC: Session creation sets global default-size after new-session
/// — basic builder.
#[test]
fn basic_builder_sets_default_size_after_new_session() {
    let session = TmuxSessionBuilder::new("app")
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    let new_session_idx = cmds
        .iter()
        .position(|c| c.contains("new-session"))
        .expect("new-session in command list");
    let default_size_idx = cmds
        .iter()
        .position(|c| {
            c.contains("set-option") && c.contains("default-size") && c.contains("480x140")
        })
        .expect("set-option default-size 200x50 in command list");
    assert!(
        default_size_idx > new_session_idx,
        "set-option default-size must come AFTER new-session (set-option needs a running server); got order new={new_session_idx}, default-size={default_size_idx}"
    );
}

#[test]
fn session_name_override_replaces_default() {
    let session = TmuxSessionBuilder::new("my-project")
        .session_name("custom-session-name".to_string())
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .build()
        .unwrap();

    assert_eq!(session.name, "custom-session-name");
    let cmds = session.command_strings();
    assert!(
        cmds.iter()
            .any(|c| c.contains("new-session") && c.contains("custom-session-name")),
        "should use overridden session name"
    );
}

// -----------------------------------------------------------------------
// AC: Dynamic pane count based on input
// Dry-run contract: verifies the number of commands matches the number of
// panes the user requested. Actual pane creation verified by e2e test
// tmux_session_with_five_panes_and_different_clis.
// -----------------------------------------------------------------------

#[test]
fn pane_count_matches_input_for_two_panes() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("feat/auth", "/tmp/wt1", "claude"))
        .add_pane(make_pane("feat/api", "/tmp/wt2", "codex"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    let send_keys: Vec<String> = commands_containing(&cmds, "send-keys")
        .into_iter()
        .filter(|c| !c.trim_end().ends_with("C-u"))
        .collect();
    assert_eq!(
        send_keys.len(),
        2,
        "should send commands to exactly 2 panes"
    );
}

#[test]
fn pane_count_matches_input_for_five_panes() {
    let mut builder = TmuxSessionBuilder::new("proj");
    for i in 0..5 {
        builder = builder.add_pane(make_pane(
            &format!("feat/b{i}"),
            &format!("/tmp/wt{i}"),
            "claude",
        ));
    }
    let session = builder.build().unwrap();

    let cmds = session.command_strings();
    let send_keys: Vec<String> = commands_containing(&cmds, "send-keys")
        .into_iter()
        .filter(|c| !c.trim_end().ends_with("C-u"))
        .collect();
    assert_eq!(
        send_keys.len(),
        5,
        "should send commands to exactly 5 panes"
    );
}

#[test]
fn building_with_no_panes_is_an_error() {
    let result = TmuxSessionBuilder::new("proj").build();
    assert!(result.is_err(), "session with no panes should fail");
}

// -----------------------------------------------------------------------
// AC: Correct commands sent to panes
// Dry-run contract: users see these exact commands in --dry-run output,
// so the format (CLI command in send-keys, worktree on split-window -c)
// is user-facing.
// -----------------------------------------------------------------------

#[test]
fn each_pane_receives_bare_cli_command_and_split_carries_worktree() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("feat/auth", "/home/user/wt-auth", "claude"))
        .add_pane(make_pane("feat/api", "/home/user/wt-api", "gemini"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    let send_keys: Vec<String> = commands_containing(&cmds, "send-keys")
        .into_iter()
        .filter(|c| !c.trim_end().ends_with("C-u"))
        .collect();

    // Pane 0 uses `-c` on `new-session` for its directory and runs only
    // the bare CLI command.
    assert!(
        send_keys[0].contains("claude"),
        "first pane should run claude; got: {}",
        send_keys[0]
    );

    // Subsequent panes must not prefix `cd <worktree> &&` — the cwd is
    // baked into the split via `-c <worktree>` instead, avoiding the
    // send-keys race documented at the call site.
    assert!(
        send_keys[1].contains("gemini"),
        "second pane should run gemini; got: {}",
        send_keys[1]
    );
    assert!(
        !send_keys[1].contains("cd /home/user/wt-api"),
        "second pane send-keys MUST NOT prefix `cd <worktree>`; got: {}",
        send_keys[1]
    );

    // The split-window that creates pane 1 should carry the worktree as
    // `-c <worktree>`.
    let splits = commands_containing(&cmds, "split-window");
    assert!(
        splits.iter().any(|c| c.contains("-c /home/user/wt-api")),
        "split-window for pane 1 should pass -c /home/user/wt-api; got: {splits:?}"
    );
}

#[test]
fn pane_commands_are_submitted_with_enter() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("main", "/tmp/wt", "aider"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    let send_keys: Vec<String> = commands_containing(&cmds, "send-keys")
        .into_iter()
        .filter(|c| !c.trim_end().ends_with("C-u"))
        .collect();
    assert!(
        send_keys[0].contains("Enter"),
        "send-keys should press Enter to submit"
    );
}

#[test]
fn each_pane_targets_a_distinct_pane_index() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("feat/a", "/tmp/a", "claude"))
        .add_pane(make_pane("feat/b", "/tmp/b", "codex"))
        .add_pane(make_pane("feat/c", "/tmp/c", "gemini"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    let send_keys: Vec<String> = commands_containing(&cmds, "send-keys")
        .into_iter()
        .filter(|c| !c.trim_end().ends_with("C-u"))
        .collect();

    assert!(
        send_keys[0].contains(":0.0"),
        "first pane should target :0.0"
    );
    assert!(
        send_keys[1].contains(":0.1"),
        "second pane should target :0.1"
    );
    assert!(
        send_keys[2].contains(":0.2"),
        "third pane should target :0.2"
    );
}

// -----------------------------------------------------------------------
// AC: Pane titles show branch and CLI
// Dry-run contract: title format is user-visible in both --dry-run output
// and tmux pane borders. Actual tmux titles verified by e2e test
// tmux_session_with_five_panes_and_different_clis.
// -----------------------------------------------------------------------

#[test]
fn each_pane_is_titled_with_its_branch() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("feat/auth", "/tmp/wt1", "claude"))
        .add_pane(make_pane("fix/api", "/tmp/wt2", "gemini"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    let select_panes = commands_containing(&cmds, "select-pane");

    assert_eq!(select_panes.len(), 2, "each pane should get a title");
    // The title is the pane's branch id only — the CLI command is no
    // longer part of the title (it reads cleanly in the label strip).
    assert!(
        select_panes[0].ends_with("-T feat/auth"),
        "first pane title should be 'feat/auth', got: {}",
        select_panes[0]
    );
    assert!(
        !select_panes[0].contains("claude"),
        "first pane title should not include the CLI command, got: {}",
        select_panes[0]
    );
    assert!(
        select_panes[1].ends_with("-T fix/api"),
        "second pane title should be 'fix/api', got: {}",
        select_panes[1]
    );
}

/// Scenario: Each pane also gets a pane-scoped `@paw_role` user option
/// carrying its role label. This is the clobber-proof source of the border
/// label: the agent CLI overwrites `#{pane_title}` via OSC sequences, but
/// the `@paw_role` pane option git-paw sets is never overwritten, so the
/// `pane-border-format` conditional keeps showing the role.
#[test]
fn each_pane_gets_a_stable_paw_role_option() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("feat/auth", "/tmp/wt1", "claude"))
        .add_pane(make_pane("fix/api", "/tmp/wt2", "gemini"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    // Pane-scoped option assignments only — exclude the pane-border-format
    // command, which also mentions @paw_role inside its conditional.
    let role_opts: Vec<&String> = cmds
        .iter()
        .filter(|c| c.contains("set-option") && c.contains(" -p ") && c.contains("@paw_role"))
        .collect();
    assert_eq!(
        role_opts.len(),
        2,
        "each pane should get a @paw_role option"
    );
    assert!(
        role_opts.iter().any(|c| c.ends_with("@paw_role feat/auth")),
        "first pane should set `@paw_role feat/auth` pane-scoped; got: {role_opts:#?}"
    );
    assert!(
        role_opts.iter().any(|c| c.ends_with("@paw_role fix/api")),
        "second pane should set `@paw_role fix/api`; got: {role_opts:#?}"
    );
}

#[test]
fn pane_border_status_is_configured() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    assert!(
        cmds.iter()
            .any(|c| c.contains("pane-border-status") && c.contains("top")),
        "should configure pane-border-status to top"
    );
    assert!(
        cmds.iter()
            .any(|c| c.contains("pane-border-format") && c.contains("#{pane_title}")),
        "should configure pane-border-format to show pane title"
    );
}

// -----------------------------------------------------------------------
// supervisor-pane-affordances: heavy borders + per-pane labels + active
// highlight, scoped to the session, with a config opt-out and graceful
// degradation on older tmux.
// -----------------------------------------------------------------------

/// The five affordance `set-option` invocations a session must carry when
/// affordances are on, paired with their exact values.
const AFFORDANCE_OPTIONS: [(&str, &str); 5] = [
    ("pane-border-lines", "double"),
    ("pane-border-style", "fg=colour238"),
    ("pane-active-border-style", "fg=colour45,bold"),
    ("pane-border-status", "top"),
    (
        "pane-border-format",
        "#[fg=colour39,bold,reverse] #{pane_index}: #{?#{@paw_role},#{@paw_role},#{pane_title}} #[default]",
    ),
];

/// Scenario: Heavy border option is set on the session — and the other
/// four affordance options, all scoped with `-t <session>`.
#[test]
fn builder_emits_all_five_affordances_scoped_to_session_by_default() {
    let session = TmuxSessionBuilder::new("aff-default")
        .add_pane(make_pane("feat/a", "/tmp/wt", "claude"))
        .build()
        .unwrap();
    let cmds = session.command_strings();
    for (option, value) in AFFORDANCE_OPTIONS {
        assert!(
            cmds.iter().any(|c| c.contains("set-option")
                && c.contains("-t paw-aff-default")
                && c.contains(option)
                && c.contains(value)),
            "expected `set-option -t paw-aff-default {option} {value}`; cmds:\n{cmds:#?}"
        );
    }
}

/// Scenario: Border format includes index and the role label — the format
/// string is exactly ` #{pane_index}: #{?#{@paw_role},#{@paw_role},#{pane_title}} `
/// (spaces preserved). The conditional prefers the git-paw-set `@paw_role`
/// pane option (not clobbered by the CLI) over `#{pane_title}`.
#[test]
fn border_format_is_index_then_role_with_padding() {
    let session = TmuxSessionBuilder::new("fmt")
        .add_pane(make_pane("feat/a", "/tmp/wt", "claude"))
        .build()
        .unwrap();
    let format_cmd = session
        .command_strings()
        .into_iter()
        .find(|c| c.contains("pane-border-format"))
        .expect("pane-border-format set-option present");
    assert!(
            format_cmd.ends_with(
                "pane-border-format #[fg=colour39,bold,reverse] #{pane_index}: #{?#{@paw_role},#{@paw_role},#{pane_title}} #[default]"
            ),
            "format must be the reverse-video label bar preferring @paw_role; got: {format_cmd}"
        );
}

/// Scenario: Active border style is applied — a bright bold colour for the
/// active border and a dim colour for inactive borders.
#[test]
fn active_and_inactive_border_styles_applied() {
    let session = TmuxSessionBuilder::new("styles")
        .add_pane(make_pane("feat/a", "/tmp/wt", "claude"))
        .build()
        .unwrap();
    let cmds = session.command_strings();
    assert!(
        cmds.iter()
            .any(|c| c.contains("pane-active-border-style") && c.contains("colour45,bold")),
        "active border must be colour45,bold; cmds:\n{cmds:#?}"
    );
    assert!(
        cmds.iter()
            .any(|c| c.contains("pane-border-style") && c.contains("colour238")),
        "inactive border must be colour238; cmds:\n{cmds:#?}"
    );
}

/// Scenario: Explicit false skips all affordances — none of the five
/// `set-option` invocations and none of the per-pane `select-pane -T`
/// title sets are emitted, but the CLI still launches.
#[test]
fn opt_out_omits_every_affordance_and_title() {
    let session = TmuxSessionBuilder::new("opt-out")
        .add_pane(make_pane("feat/a", "/tmp/wt", "claude"))
        .add_pane(make_pane("feat/b", "/tmp/wt2", "gemini"))
        .border_affordances(false)
        .build()
        .unwrap();
    let cmds = session.command_strings();
    for (option, _value) in AFFORDANCE_OPTIONS {
        assert!(
            !cmds
                .iter()
                .any(|c| c.contains("set-option") && c.contains(option)),
            "opt-out must not emit set-option {option}; cmds:\n{cmds:#?}"
        );
    }
    assert!(
        !cmds
            .iter()
            .any(|c| c.contains("select-pane") && c.contains("-T")),
        "opt-out must not set any pane title; cmds:\n{cmds:#?}"
    );
    assert!(
        !cmds.iter().any(|c| c.contains("@paw_role")),
        "opt-out must not set the @paw_role pane option; cmds:\n{cmds:#?}"
    );
    // The CLI still runs in each pane — opt-out only drops the styling.
    assert_eq!(
        commands_containing(&cmds, "send-keys").len(),
        2,
        "both panes still receive their CLI send-keys"
    );
}

/// Scenario: Unsupported option produces a stderr warning, and other
/// affordances still apply (graceful degradation on older tmux, design D4).
#[test]
fn soft_affordance_failure_warns_and_continues() {
    let session = TmuxSessionBuilder::new("degrade")
        .add_pane(make_pane("feat/a", "/tmp/wt", "claude"))
        .build()
        .unwrap();

    let mut ran: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    // Simulate a tmux that rejects only `pane-border-lines double`.
    let result = session.execute_with(
        |cmd| {
            let s = cmd.as_command_string();
            ran.push(s.clone());
            if s.contains("pane-border-lines double") {
                Err(PawError::TmuxError(
                    "unknown option: pane-border-lines".into(),
                ))
            } else {
                Ok(())
            }
        },
        |w| warnings.push(w),
    );

    assert!(result.is_ok(), "soft failure must not abort the build");
    assert!(
        warnings.iter().any(|w| w.contains("pane-border-lines")),
        "a warning naming the unsupported option must be emitted; warnings: {warnings:#?}"
    );
    // The other affordances (shipped since tmux 2.3) still ran.
    assert!(
        ran.iter().any(|c| c.contains("pane-active-border-style")),
        "active-border-style must still be applied after the double-line failure"
    );
    assert!(
        ran.iter().any(|c| c.contains("pane-border-status top")),
        "pane-border-status must still be applied after the double-line failure"
    );
}

/// A non-soft command failure aborts the build (the double-line tolerance is
/// scoped to the soft affordance commands, not every command).
#[test]
fn hard_command_failure_aborts() {
    let session = TmuxSessionBuilder::new("hard-fail")
        .add_pane(make_pane("feat/a", "/tmp/wt", "claude"))
        .build()
        .unwrap();
    let result = session.execute_with(
        |cmd| {
            if cmd.as_command_string().contains("new-session") {
                Err(PawError::TmuxError("server unreachable".into()))
            } else {
                Ok(())
            }
        },
        |_| {},
    );
    assert!(result.is_err(), "a hard command failure must propagate");
}

/// Scenario: Supervisor/dashboard/agent pane titles are their role/branch
/// id, and the supervisor builder also emits all five affordances.
#[test]
fn supervisor_session_titles_are_roles_and_emits_affordances() {
    let layout = crate::supervisor::layout::supervisor_layout(2).expect("layout");
    let supervisor = make_pane("supervisor", "/repo", "claude");
    let dashboard = make_pane("dashboard", "/repo", "git-paw __dashboard");
    let agent = make_pane("feat/foo", "/tmp/wt", "claude");
    let session = build_supervisor_session(
        "sup",
        None,
        &supervisor,
        &dashboard,
        &[agent],
        layout,
        true,
        true,
        &[],
    )
    .expect("session builds");
    let cmds = session.command_strings();

    // All five affordances present and scoped.
    for (option, value) in AFFORDANCE_OPTIONS {
        assert!(
            cmds.iter().any(|c| c.contains("set-option")
                && c.contains("-t paw-sup")
                && c.contains(option)
                && c.contains(value)),
            "supervisor session missing `set-option {option} {value}`; cmds:\n{cmds:#?}"
        );
    }

    let title_for = |target: &str| -> String {
        cmds.iter()
            .find(|c| c.contains("select-pane") && c.contains(target) && c.contains("-T"))
            .unwrap_or_else(|| panic!("no title set for {target}; cmds:\n{cmds:#?}"))
            .clone()
    };
    assert!(title_for(":0.0").ends_with("-T supervisor"), "pane 0 title");
    assert!(title_for(":0.1").ends_with("-T dashboard"), "pane 1 title");
    assert!(
        title_for(":0.2").ends_with("-T feat/foo"),
        "agent pane title"
    );
}

/// W2-2 (supervisor-cli-launch-robustness): the supervisor build suppresses
/// shell startup prompts (so an oh-my-zsh-style update prompt can't eat the
/// CLI-launch keystroke) and clears the input line before each launch.
#[test]
fn supervisor_build_suppresses_startup_prompts_and_clears_input() {
    let layout = crate::supervisor::layout::supervisor_layout(1).expect("layout");
    let supervisor = make_pane("supervisor", "/repo", "claude");
    let dashboard = make_pane("dashboard", "/repo", "git-paw __dashboard");
    let agent = make_pane("feat/foo", "/tmp/wt", "claude");
    let session = build_supervisor_session(
        "sup",
        None,
        &supervisor,
        &dashboard,
        &[agent],
        layout,
        true,
        true,
        &[],
    )
    .expect("session builds");
    let cmds = session.command_strings();

    // Pane 0's shell gets the suppression env via `new-session -e`.
    assert!(
        cmds.iter()
            .any(|c| c.contains("new-session") && c.contains("DISABLE_AUTO_UPDATE=true")),
        "new-session must set DISABLE_AUTO_UPDATE for pane 0; cmds:\n{cmds:#?}"
    );
    // Later split panes inherit it via session environment.
    assert!(
        cmds.iter().any(|c| c.contains("set-environment")
            && c.contains("DISABLE_AUTO_UPDATE")
            && c.contains("true")),
        "session env must carry DISABLE_AUTO_UPDATE for split panes"
    );
    // A `C-u` clear precedes the supervisor pane's CLI-launch command.
    let clear_idx = cmds.iter().position(|c| {
        c.contains("send-keys") && c.contains(":0.0") && c.trim_end().ends_with("C-u")
    });
    let launch_idx = cmds.iter().position(|c| {
        c.contains("send-keys") && c.contains(":0.0") && c.contains("claude") && c.contains("Enter")
    });
    let (clear_idx, launch_idx) = (
        clear_idx.expect("a C-u clear is sent to pane 0"),
        launch_idx.expect("the CLI-launch command is sent to pane 0"),
    );
    assert!(
        clear_idx < launch_idx,
        "the C-u clear must precede the CLI-launch command on pane 0"
    );
}

/// W3-1 (supervisor-first-agent-cwd): the split `-c` cwds are assigned to
/// compensate for the pane-1/2 swap, so the first agent's CLI (sent to
/// index 2 after the swap) runs in its worktree, not the repo root. The
/// agent-area `-v` split takes the dashboard's cwd; the dashboard `-h`
/// split takes the first agent's worktree.
#[test]
fn supervisor_build_compensates_first_agent_cwd_for_swap() {
    let layout = crate::supervisor::layout::supervisor_layout(2).expect("layout");
    let supervisor = make_pane("supervisor", "/repo", "claude");
    let dashboard = make_pane("dashboard", "/repo", "git-paw __dashboard");
    let a0 = make_pane("feat/foo", "/tmp/wt-foo", "claude");
    let a1 = make_pane("feat/bar", "/tmp/wt-bar", "claude");
    let session = build_supervisor_session(
        "sup",
        None,
        &supervisor,
        &dashboard,
        &[a0, a1],
        layout,
        true,
        true,
        &[],
    )
    .expect("session builds");
    let cmds = session.command_strings();

    let vsplit = cmds
        .iter()
        .find(|c| c.contains("split-window") && c.contains("-v") && c.contains("-c"))
        .expect("agent-area -v split with -c");
    let hsplit = cmds
        .iter()
        .find(|c| c.contains("split-window") && c.contains("-h") && c.contains("-c"))
        .expect("dashboard -h split with -c");

    // Agent-area split is born in the dashboard's cwd (it lands at the
    // dashboard's post-swap index); dashboard split is born in the first
    // agent's worktree (it lands at the agent's post-swap index).
    assert!(
        vsplit.contains("-c /repo"),
        "agent-area -v split must use the dashboard cwd (swap compensation); got: {vsplit}"
    );
    assert!(
        hsplit.contains("-c /tmp/wt-foo"),
        "dashboard -h split must use the first agent's worktree (swap compensation); got: {hsplit}"
    );
}

/// Scenario: opt-out applies to the supervisor builder too — no affordance
/// set-options and no `select-pane -T` titles.
#[test]
fn supervisor_session_opt_out_omits_affordances() {
    let layout = crate::supervisor::layout::supervisor_layout(1).expect("layout");
    let supervisor = make_pane("supervisor", "/repo", "claude");
    let dashboard = make_pane("dashboard", "/repo", "git-paw __dashboard");
    let agent = make_pane("feat/foo", "/tmp/wt", "claude");
    let session = build_supervisor_session(
        "sup-off",
        None,
        &supervisor,
        &dashboard,
        &[agent],
        layout,
        true,
        false,
        &[],
    )
    .expect("session builds");
    let cmds = session.command_strings();
    for (option, _value) in AFFORDANCE_OPTIONS {
        assert!(
            !cmds
                .iter()
                .any(|c| c.contains("set-option") && c.contains(option)),
            "opt-out supervisor session must not emit set-option {option}"
        );
    }
    assert!(
        !cmds
            .iter()
            .any(|c| c.contains("select-pane") && c.contains("-T")),
        "opt-out supervisor session must not set pane titles"
    );
}

// -----------------------------------------------------------------------
// AC: Mouse mode (per-session, configurable, default on)
// Dry-run contract: users see mouse config in --dry-run output.
// Actual tmux behavior verified by e2e test tmux_mouse_mode_enabled_by_default.
// -----------------------------------------------------------------------

#[test]
fn mouse_mode_enabled_by_default() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    assert!(
        cmds.iter().any(|c| c.contains("mouse on")),
        "mouse should be enabled by default"
    );
}

#[test]
fn mouse_mode_can_be_disabled() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .mouse_mode(false)
        .build()
        .unwrap();

    let cmds = session.command_strings();
    assert!(
        !cmds.iter().any(|c| c.contains("mouse on")),
        "no mouse-on command should be emitted when disabled"
    );
}

// -----------------------------------------------------------------------
// AC: Session liveness and collision handling
// Behavioral: tests against a real tmux server — verifies observable
// outcomes (session exists, session is killed, names are unique).
// -----------------------------------------------------------------------

/// Helper to create a detached tmux session for testing.
fn create_test_session(name: &str) {
    let output = std::process::Command::new("tmux")
        .args(["new-session", "-d", "-s", name, "-x", "200", "-y", "50"])
        .output()
        .expect("create tmux session");
    assert!(
        output.status.success(),
        "failed to create test session '{name}'"
    );
}

/// Helper to kill a tmux session, ignoring errors.
fn cleanup_session(name: &str) {
    let _ = kill_session(name);
}

#[test]
#[serial_test::serial]
fn is_session_alive_returns_false_for_nonexistent() {
    let alive = is_session_alive("paw-definitely-does-not-exist-12345").unwrap();
    assert!(!alive);
}

#[test]
#[serial_test::serial]
fn session_lifecycle_create_check_kill() {
    let name = "paw-unit-test-lifecycle";
    cleanup_session(name);

    create_test_session(name);
    assert!(is_session_alive(name).unwrap());

    kill_session(name).unwrap();
    assert!(!is_session_alive(name).unwrap());
}

// -----------------------------------------------------------------------
// session-bugfixes Bug 2 — SessionLiveness probe (tasks 3.1–3.3)
// -----------------------------------------------------------------------

#[test]
fn classify_liveness_maps_each_branch() {
    // tmux ran and the session exists.
    assert_eq!(classify_liveness(true, true), SessionLiveness::Alive);
    // tmux ran and the session is gone.
    assert_eq!(classify_liveness(true, false), SessionLiveness::Stale);
    // tmux could not be spawned at all (binary missing) — inconclusive.
    assert_eq!(
        classify_liveness(false, false),
        SessionLiveness::Indeterminate
    );
    assert_eq!(
        classify_liveness(false, true),
        SessionLiveness::Indeterminate
    );
}

#[test]
#[serial_test::serial]
fn session_liveness_reports_stale_for_nonexistent() {
    assert_eq!(
        session_liveness("paw-definitely-does-not-exist-98765"),
        SessionLiveness::Stale
    );
}

#[test]
#[serial_test::serial]
fn session_liveness_reports_alive_then_stale_across_lifecycle() {
    let name = "paw-unit-test-liveness-probe";
    cleanup_session(name);

    create_test_session(name);
    assert_eq!(session_liveness(name), SessionLiveness::Alive);

    kill_session(name).unwrap();
    assert_eq!(session_liveness(name), SessionLiveness::Stale);
}

#[test]
#[serial_test::serial]
fn resolve_session_name_returns_base_when_no_collision() {
    let name = resolve_session_name("unit-test-no-collision-xyz").unwrap();
    assert_eq!(name, "paw-unit-test-no-collision-xyz");
}

#[test]
#[serial_test::serial]
fn resolve_session_name_appends_suffix_on_collision() {
    let base_name = "paw-unit-test-collision";
    cleanup_session(base_name);
    cleanup_session(&format!("{base_name}-2"));

    create_test_session(base_name);

    let resolved = resolve_session_name("unit-test-collision").unwrap();
    assert_eq!(resolved, format!("{base_name}-2"));

    cleanup_session(base_name);
}

// -----------------------------------------------------------------------
// AC: pipe-pane logging integration
// Dry-run contract: verifies the pipe-pane command is queued correctly.
// -----------------------------------------------------------------------

#[test]
fn pipe_pane_queues_correct_command() {
    let mut session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("feat/auth", "/tmp/wt1", "claude"))
        .build()
        .unwrap();

    let log_path = std::path::PathBuf::from("/repo/.git-paw/logs/paw-proj/feat--auth.log");
    session.pipe_pane("paw-proj:0.0", &log_path);

    let cmds = session.command_strings();
    let pipe_cmds: Vec<&String> = cmds.iter().filter(|c| c.contains("pipe-pane")).collect();
    assert_eq!(pipe_cmds.len(), 1);
    assert!(pipe_cmds[0].contains("pipe-pane -o -t paw-proj:0.0"));
    assert!(pipe_cmds[0].contains("cat >> /repo/.git-paw/logs/paw-proj/feat--auth.log"));
}

// --- Gap #10: pipe-pane conditional on logging ---

#[test]
fn session_without_pipe_pane_has_no_pipe_pane_commands() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    assert!(
        !cmds.iter().any(|c| c.contains("pipe-pane")),
        "session built without pipe_pane calls should have no pipe-pane commands"
    );
}

#[test]
fn session_with_pipe_pane_differs_from_without() {
    let session_without = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .build()
        .unwrap();
    let cmds_without = session_without.command_strings();

    let mut session_with = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .build()
        .unwrap();
    let log_path = std::path::PathBuf::from("/repo/.git-paw/logs/paw-proj/main.log");
    session_with.pipe_pane("paw-proj:0.0", &log_path);
    let cmds_with = session_with.command_strings();

    assert_ne!(
        cmds_without, cmds_with,
        "command lists should differ when pipe-pane is added"
    );
    assert!(
        cmds_with.iter().any(|c| c.contains("pipe-pane")),
        "session with pipe_pane should contain pipe-pane command"
    );
}

// --- Gap #11: pipe-pane ordering ---

#[test]
fn pipe_pane_appears_after_send_keys_for_pane() {
    let mut session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("feat/auth", "/tmp/wt1", "claude"))
        .add_pane(make_pane("feat/api", "/tmp/wt2", "codex"))
        .build()
        .unwrap();

    let log0 = std::path::PathBuf::from("/repo/logs/feat--auth.log");
    let log1 = std::path::PathBuf::from("/repo/logs/feat--api.log");
    session.pipe_pane("paw-proj:0.0", &log0);
    session.pipe_pane("paw-proj:0.1", &log1);

    let cmds = session.command_strings();

    // Find the last send-keys index and first pipe-pane index
    let last_send_keys = cmds
        .iter()
        .rposition(|c| c.contains("send-keys"))
        .expect("should have send-keys");
    let first_pipe_pane = cmds
        .iter()
        .position(|c| c.contains("pipe-pane"))
        .expect("should have pipe-pane");

    assert!(
        first_pipe_pane > last_send_keys,
        "pipe-pane commands (index {first_pipe_pane}) should appear after \
             all send-keys commands (last at index {last_send_keys})"
    );
}

#[test]
fn pipe_pane_appears_in_dry_run_output() {
    let mut session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .build()
        .unwrap();

    let log_path = std::path::PathBuf::from("/repo/.git-paw/logs/paw-proj/main.log");
    session.pipe_pane("paw-proj:0.0", &log_path);

    let cmds = session.command_strings();
    assert!(
        cmds.iter().any(|c| c.starts_with("tmux pipe-pane")),
        "dry-run output should include pipe-pane command"
    );
}

// -----------------------------------------------------------------------
// AC: set_environment emits correct commands
// -----------------------------------------------------------------------

#[test]
fn set_environment_emits_correct_tmux_command() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .set_environment("GIT_PAW_BROKER_URL", "http://127.0.0.1:9119")
        .build()
        .unwrap();

    let cmds = session.command_strings();
    let env_cmds = commands_containing(&cmds, "set-environment");
    assert_eq!(env_cmds.len(), 1, "should have exactly one set-environment");
    assert!(
        env_cmds[0]
            .contains("set-environment -t paw-proj GIT_PAW_BROKER_URL http://127.0.0.1:9119"),
        "set-environment command should contain key and value, got: {}",
        env_cmds[0]
    );
}

#[test]
fn set_environment_appears_before_send_keys() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("feat/a", "/tmp/a", "claude"))
        .add_pane(make_pane("feat/b", "/tmp/b", "codex"))
        .set_environment("GIT_PAW_BROKER_URL", "http://127.0.0.1:9119")
        .build()
        .unwrap();

    let cmds = session.command_strings();
    let first_env = cmds
        .iter()
        .position(|c| c.contains("set-environment"))
        .expect("should have set-environment");
    let first_send = cmds
        .iter()
        .position(|c| c.contains("send-keys"))
        .expect("should have send-keys");

    assert!(
        first_env < first_send,
        "set-environment (index {first_env}) should appear before first send-keys (index {first_send})"
    );
}

#[test]
fn multiple_env_vars_both_appear() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .set_environment("A", "1")
        .set_environment("B", "2")
        .build()
        .unwrap();

    let cmds = session.command_strings();
    let env_cmds = commands_containing(&cmds, "set-environment");
    assert_eq!(
        env_cmds.len(),
        2,
        "should have two set-environment commands"
    );
    assert!(env_cmds[0].contains("A 1"));
    assert!(env_cmds[1].contains("B 2"));
}

#[test]
fn set_environment_in_dry_run_output() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .set_environment("MY_VAR", "my_val")
        .build()
        .unwrap();

    let cmds = session.command_strings();
    assert!(
        cmds.iter().any(|c| c.starts_with("tmux set-environment")),
        "dry-run output should include set-environment command"
    );
}

// -----------------------------------------------------------------------
// AC: Dashboard layout selection
// Behavioral: verifies the correct layout is chosen based on pane structure
// -----------------------------------------------------------------------

#[test]
fn session_without_dashboard_uses_tiled_layout() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("feat/a", "/tmp/a", "claude"))
        .add_pane(make_pane("feat/b", "/tmp/b", "codex"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    let layout_cmds: Vec<&String> = cmds
        .iter()
        .filter(|c| c.contains("select-layout"))
        .collect();
    let final_layout = layout_cmds
        .last()
        .expect("should have at least one select-layout");
    assert!(
        final_layout.contains("tiled"),
        "sessions without dashboard should use tiled layout, got: {final_layout}"
    );
}

#[test]
fn session_with_dashboard_uses_main_horizontal_layout() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("dashboard", "/tmp/repo", "git-paw __dashboard"))
        .add_pane(make_pane("feat/a", "/tmp/a", "claude"))
        .add_pane(make_pane("feat/b", "/tmp/b", "codex"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    let layout_cmds: Vec<&String> = cmds
        .iter()
        .filter(|c| c.contains("select-layout"))
        .collect();
    let final_layout = layout_cmds
        .last()
        .expect("should have at least one select-layout");
    assert!(
        final_layout.contains("main-horizontal"),
        "sessions with dashboard should use main-horizontal layout, got: {final_layout}"
    );
}

#[test]
fn single_pane_session_uses_tiled_layout() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("main", "/tmp/wt", "claude"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    let layout_cmds: Vec<&String> = cmds
        .iter()
        .filter(|c| c.contains("select-layout"))
        .collect();
    let final_layout = layout_cmds
        .last()
        .expect("should have at least one select-layout");
    assert!(
        final_layout.contains("tiled"),
        "single pane sessions should use tiled layout, got: {final_layout}"
    );
}

#[test]
fn dashboard_layout_appears_in_dry_run_output() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("dashboard", "/tmp/repo", "git-paw __dashboard"))
        .add_pane(make_pane("feat/a", "/tmp/a", "claude"))
        .build()
        .unwrap();

    let cmds = session.command_strings();
    assert!(
        cmds.iter().any(|c| c.contains("main-horizontal")),
        "dry-run output should include main-horizontal layout command"
    );
}

// -----------------------------------------------------------------------
// AC: detach_client + kill_pane behave idempotently
// -----------------------------------------------------------------------

/// Helper that yields a unique detached test session name and cleans it
/// up on drop. Used to keep pause-related tmux tests hermetic.
struct PausePaneSession {
    name: String,
}

impl PausePaneSession {
    fn new(label: &str) -> Self {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let name = format!("paw-pause-test-{label}-{pid}-{nanos}");
        let output = std::process::Command::new("tmux")
            .args(["new-session", "-d", "-s", &name, "-x", "200", "-y", "50"])
            .output()
            .expect("create tmux test session");
        assert!(
            output.status.success(),
            "failed to create test session '{name}'"
        );
        Self { name }
    }
}

impl Drop for PausePaneSession {
    fn drop(&mut self) {
        let _ = kill_session(&self.name);
    }
}

#[test]
#[serial_test::serial]
fn detach_client_succeeds_on_attached_session() {
    // No client is actually attached in headless test, but a detached
    // session under tmux server is the closest the unit layer can get
    // without a pty; the public contract is "exit Ok" either way.
    let session = PausePaneSession::new("detach-attached");
    detach_client(&session.name).expect("detach should succeed");
    assert!(is_session_alive(&session.name).unwrap());
}

#[test]
#[serial_test::serial]
fn detach_client_is_noop_with_no_clients() {
    let session = PausePaneSession::new("detach-noop");
    // First call: no clients attached.
    detach_client(&session.name).expect("first detach should succeed");
    // Second call: also no clients (still alive).
    detach_client(&session.name).expect("second detach should succeed");
    assert!(is_session_alive(&session.name).unwrap());
}

/// Spec-aligned alias of `detach_client_is_noop_with_no_clients`
/// (task 9.11). A detached test session has no client attached;
/// `detach_client` must still return Ok(()).
#[test]
#[serial_test::serial]
fn detach_client_noop_when_no_clients_attached() {
    let session = PausePaneSession::new("detach-9-11");
    detach_client(&session.name).expect("detach with no clients should be Ok");
    assert!(is_session_alive(&session.name).unwrap());
}

#[test]
#[serial_test::serial]
fn kill_pane_removes_pane() {
    let session = PausePaneSession::new("killpane");
    // Add a second pane so the kill doesn't take down the whole session.
    let _ = std::process::Command::new("tmux")
        .args(["split-window", "-t", &session.name])
        .output();
    let pane_count_before = std::process::Command::new("tmux")
        .args(["list-panes", "-t", &session.name, "-F", "#{pane_index}"])
        .output()
        .map_or(0, |o| String::from_utf8_lossy(&o.stdout).lines().count());
    assert_eq!(pane_count_before, 2, "should have 2 panes before kill");

    kill_pane(&session.name, 1).expect("kill_pane should succeed");

    let pane_count_after = std::process::Command::new("tmux")
        .args(["list-panes", "-t", &session.name, "-F", "#{pane_index}"])
        .output()
        .map_or(0, |o| String::from_utf8_lossy(&o.stdout).lines().count());
    assert_eq!(pane_count_after, 1, "should have 1 pane after kill");
}

#[test]
#[serial_test::serial]
fn kill_pane_is_noop_for_missing_pane() {
    let session = PausePaneSession::new("killpane-missing");
    // Pane index 99 does not exist — should not error.
    kill_pane(&session.name, 99).expect("kill missing pane should be ok");
    assert!(is_session_alive(&session.name).unwrap());
}

#[test]
#[serial_test::serial]
fn built_session_can_be_executed_and_killed() {
    let project = "unit-test-execute";
    let session_name = format!("paw-{project}");
    cleanup_session(&session_name);

    let session = TmuxSessionBuilder::new(project)
        .add_pane(make_pane("main", "/tmp", "echo hello"))
        .build()
        .unwrap();

    session.execute().unwrap();
    assert!(is_session_alive(&session_name).unwrap());

    kill_session(&session_name).unwrap();
    assert!(!is_session_alive(&session_name).unwrap());
}

// -----------------------------------------------------------------------
// AC: Supervisor-mode initial prompt is injected as a paste + two Enters
// Behavioral: callers iterate the argv pair and run each as a separate
// `tmux send-keys` invocation. The pair shape is the public contract.
// -----------------------------------------------------------------------

#[test]
fn supervisor_submit_argv_pair_has_two_invocations() {
    let (first, second) = build_supervisor_submit_argv_pair("paw-proj", 3, "do the thing");
    // Both invocations are non-empty argv vectors.
    assert!(!first.is_empty(), "first send-keys argv must be non-empty");
    assert!(
        !second.is_empty(),
        "second send-keys argv must be non-empty"
    );
}

#[test]
fn supervisor_submit_first_invocation_sends_prompt_and_enter() {
    let (first, _second) = build_supervisor_submit_argv_pair("paw-proj", 3, "do the thing");
    assert_eq!(first[0], "send-keys");
    assert_eq!(first[1], "-t");
    assert_eq!(first[2], "paw-proj:0.3");
    assert_eq!(first[3], "do the thing");
    assert_eq!(first[4], "Enter");
}

#[test]
fn supervisor_submit_second_invocation_is_enter_only() {
    let (_first, second) = build_supervisor_submit_argv_pair("paw-proj", 3, "do the thing");
    assert_eq!(second[0], "send-keys");
    assert_eq!(second[1], "-t");
    assert_eq!(second[2], "paw-proj:0.3");
    assert_eq!(second[3], "Enter");
    assert_eq!(
        second.len(),
        4,
        "second invocation should be send-keys -t <target> Enter (no prompt)"
    );
}

#[test]
fn supervisor_submit_targets_same_pane_in_both_invocations() {
    let (first, second) = build_supervisor_submit_argv_pair("paw-proj", 7, "prompt");
    // The target (third positional arg after `send-keys -t`) must match
    // so the second Enter lands in the same pane the prompt was sent to.
    assert_eq!(first[2], second[2]);
    assert_eq!(first[2], "paw-proj:0.7");
}

#[test]
fn supervisor_submit_argv_pair_preserves_prompt_with_newlines_and_quotes() {
    let prompt = "line1\nline2 with \"quoted\" text";
    let (first, _second) = build_supervisor_submit_argv_pair("paw-proj", 1, prompt);
    // The prompt is passed verbatim as its own argv element; tmux's
    // send-keys treats it as literal text. No shell escaping needed.
    assert_eq!(first[3], prompt);
}

// Maps to scenario `Launch flow sends exactly one Enter per pane`
// (cmd_supervisor invariant) from prompt-submit-fix. The
// `submit_prompt_to_pane` helper in main.rs sends prompt + one Enter
// per pane and is shaped identically to the FIRST argv returned by
// `build_supervisor_submit_argv_pair`. We count Enter tokens across
// the first-argv portion of N=3 invocations to lock in the
// single-Enter-per-pane invariant. (test-coverage-v0-5-0 task 3.1)
#[test]
fn cmd_supervisor_inject_argv_has_single_enter_per_pane() {
    let panes: Vec<(usize, &str)> = vec![(2, "p2"), (3, "p3"), (4, "p4")];

    let mut total_enters = 0;
    for (pane_idx, prompt) in &panes {
        let (first, _second) = build_supervisor_submit_argv_pair("paw-proj", *pane_idx, prompt);
        let enter_positions: Vec<usize> = first
            .iter()
            .enumerate()
            .filter(|(_, tok)| tok.as_str() == "Enter")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            enter_positions.len(),
            1,
            "each per-pane invocation must send exactly one Enter; got argv: {first:?}"
        );
        let enter_pos = enter_positions[0];
        assert!(
            enter_pos > 0,
            "Enter token must follow a prompt-string argument; got argv: {first:?}"
        );
        assert_eq!(
            first[enter_pos - 1].as_str(),
            *prompt,
            "Enter token must directly follow the prompt argument; got argv: {first:?}"
        );
        total_enters += enter_positions.len();
    }
    assert_eq!(
        total_enters, 3,
        "for N=3 panes the launch flow must send exactly N=3 Enters"
    );
}

// -----------------------------------------------------------------------
// build_supervisor_session — layout-shape contract (tasks 9.1–9.7)
//
// Behavioral: we inspect the emitted command strings to verify the layout
// shape. The exact tmux side effects are integration-tested elsewhere;
// here we lock in the deterministic command sequence the supervisor-mode
// pane assumptions depend on (supervisor=0, dashboard=1, agents=2+).
// -----------------------------------------------------------------------

fn make_layout_panes(n: usize) -> (PaneSpec, PaneSpec, Vec<PaneSpec>) {
    let supervisor = make_pane("supervisor", "/repo", "claude");
    let dashboard = make_pane("dashboard", "/repo", "git-paw __dashboard");
    let agents = (0..n)
        .map(|i| make_pane(&format!("feat/b{i}"), &format!("/tmp/wt{i}"), "claude"))
        .collect();
    (supervisor, dashboard, agents)
}

fn build_for(agent_count: usize) -> TmuxSession {
    let layout =
        crate::supervisor::layout::supervisor_layout(agent_count).expect("layout computes");
    let (supervisor, dashboard, agents) = make_layout_panes(agent_count);
    build_supervisor_session(
        "proj",
        None,
        &supervisor,
        &dashboard,
        &agents,
        layout,
        true,
        true,
        &[("GIT_PAW_BROKER_URL".to_string(), "http://x".to_string())],
    )
    .expect("session builds")
}

/// 9.1 — 5-agent layout: 1 agent row, top 60% / agent row 40%.
#[test]
fn supervisor_layout_5_agents_single_row() {
    let session = build_for(5);
    let cmds = session.command_strings();
    let send_keys: Vec<String> = commands_containing(&cmds, "send-keys")
        .into_iter()
        .filter(|c| !c.trim_end().ends_with("C-u"))
        .collect();
    assert_eq!(
        send_keys.len(),
        7,
        "5 agents → 1 supervisor + 1 dashboard + 5 agents = 7 send-keys, got {send_keys:#?}"
    );
    let supervisor_pane = send_keys
        .iter()
        .find(|c| c.contains("0.0 "))
        .unwrap_or(&send_keys[0]);
    assert!(supervisor_pane.contains("claude"));
    let dashboard_pane = send_keys
        .iter()
        .find(|c| c.contains(":0.1 ") && c.contains("__dashboard"))
        .expect("dashboard send-keys at pane :0.1");
    let _ = dashboard_pane;
    // Top row resize-pane uses 60%.
    let resizes = commands_containing(&cmds, "resize-pane");
    assert!(
        resizes
            .iter()
            .any(|c| c.contains(":0.0") && c.contains("60%")),
        "top row resize to 60%, got resizes {resizes:#?}"
    );
    // Single agent row resize at pane :0.2 with 40%.
    assert!(
        resizes
            .iter()
            .any(|c| c.contains(":0.2") && c.contains("40%")),
        "agent-row resize to 40% at :0.2, got resizes {resizes:#?}"
    );
}

/// 9.2 — 10-agent layout: 2 rows of 5, top 40% / each agent row 30%.
#[test]
fn supervisor_layout_10_agents_two_rows() {
    let session = build_for(10);
    let cmds = session.command_strings();
    let send_keys: Vec<String> = commands_containing(&cmds, "send-keys")
        .into_iter()
        .filter(|c| !c.trim_end().ends_with("C-u"))
        .collect();
    assert_eq!(
        send_keys.len(),
        12,
        "10 agents → 1 supervisor + 1 dashboard + 10 agents = 12 send-keys"
    );
    let resizes = commands_containing(&cmds, "resize-pane");
    assert!(
        resizes
            .iter()
            .any(|c| c.contains(":0.0") && c.contains("40%"))
    );
    assert!(
        resizes.iter().filter(|c| c.contains("30%")).count() >= 2,
        "two agent rows at 30% each, got {resizes:#?}"
    );
}

/// 9.3 — 11-agent layout: 3 agent rows (5+5+1), top 28% / each agent row 24%.
#[test]
fn supervisor_layout_11_agents_three_rows() {
    let session = build_for(11);
    let cmds = session.command_strings();
    let resizes = commands_containing(&cmds, "resize-pane");
    assert!(
        resizes
            .iter()
            .any(|c| c.contains(":0.0") && c.contains("28%"))
    );
    assert!(
        resizes.iter().filter(|c| c.contains("24%")).count() >= 3,
        "three agent rows at 24% each, got {resizes:#?}"
    );
    // 11 agents start at pane 2 and run through pane 12.
    let send_keys: Vec<String> = commands_containing(&cmds, "send-keys")
        .into_iter()
        .filter(|c| !c.trim_end().ends_with("C-u"))
        .collect();
    assert_eq!(send_keys.len(), 13);
    assert!(send_keys.iter().any(|c| c.contains(":0.12 ")));
}

/// 9.4 — 20-agent layout: 4 rows of 5, top 28% / each agent row 18%.
#[test]
fn supervisor_layout_20_agents_four_rows() {
    let session = build_for(20);
    let cmds = session.command_strings();
    let resizes = commands_containing(&cmds, "resize-pane");
    assert!(
        resizes
            .iter()
            .any(|c| c.contains(":0.0") && c.contains("28%"))
    );
    assert!(
        resizes.iter().filter(|c| c.contains("18%")).count() >= 4,
        "four agent rows at 18% each, got {resizes:#?}"
    );
}

/// 9.5 — 25-agent layout: 5 rows of 5, top 28% / each agent row 14.4%.
#[test]
fn supervisor_layout_25_agents_five_rows() {
    let session = build_for(25);
    let cmds = session.command_strings();
    let resizes = commands_containing(&cmds, "resize-pane");
    assert!(
        resizes
            .iter()
            .any(|c| c.contains(":0.0") && c.contains("28%"))
    );
    assert!(
        resizes.iter().filter(|c| c.contains("14.4%")).count() >= 5,
        "five agent rows at 14.4% each, got {resizes:#?}"
    );
}

/// 9.6 — 26-agent attempt errors before any tmux command runs.
#[test]
fn supervisor_layout_26_agents_rejected_by_layout_helper() {
    // The layout helper is the single gate for the hard cap; the tmux
    // builder is unreachable when supervisor_layout errors.
    let err = crate::supervisor::layout::supervisor_layout(26).expect_err("26 agents rejected");
    let msg = err.to_string();
    assert!(msg.contains("26 agents requested"));
    assert!(msg.contains("maximum is 25"));
}

/// 9.7 — pane indices follow row-major order. With 7 agents, pane 2 is
/// the first agent (top-left), pane 6 is the fifth (top-right of row 1),
/// pane 7 is the sixth (start of row 2).
#[test]
fn supervisor_layout_7_agents_row_major_indices() {
    let session = build_for(7);
    let cmds = session.command_strings();
    let send_keys: Vec<String> = commands_containing(&cmds, "send-keys")
        .into_iter()
        .filter(|c| !c.trim_end().ends_with("C-u"))
        .collect();
    // pane :0.2 is the first agent — its send-keys must contain its CLI
    // command. Likewise :0.6 (fifth agent) and :0.7 (sixth agent).
    assert!(
        send_keys
            .iter()
            .any(|c| c.contains(":0.2 ") && c.contains("claude")),
        "pane :0.2 is the first agent (top-left); send-keys {send_keys:#?}"
    );
    assert!(
        send_keys
            .iter()
            .any(|c| c.contains(":0.6 ") && c.contains("claude")),
        "pane :0.6 is the fifth agent (top-right of row 1)"
    );
    assert!(
        send_keys
            .iter()
            .any(|c| c.contains(":0.7 ") && c.contains("claude")),
        "pane :0.7 is the sixth agent (start of row 2)"
    );
}

// Maps to scenario `Top row is split 50/50 between supervisor and
// dashboard` from supervisor-as-pane. (test-coverage-v0-5-0 task 12.7)
#[test]
fn supervisor_top_row_split_50_50() {
    let session = build_for(3);
    let cmds = session.command_strings();
    let h_split = cmds
        .iter()
        .find(|c| c.contains("split-window") && c.contains("-h") && c.contains("-l 50%"))
        .unwrap_or_else(|| panic!("expected horizontal 50% split; got cmds: {cmds:#?}"));
    assert!(
        h_split.contains(":0.0") || h_split.contains("split-window -h -t paw-proj"),
        "horizontal split should target the supervisor pane; got: {h_split}"
    );
}

/// AC: Supervisor splits use `-l <N>%` (tmux 3.1+ syntax), not the
/// deprecated `-p <N>` form. Headless Linux tmux 3.4 fails on `-p`
/// with `size missing` because the resolver consults pane geometry
/// (unresolved without an attached client) rather than window
/// geometry. Pin the convention so no future call site regresses.
#[test]
fn supervisor_splits_use_l_percent_not_p() {
    let session = build_for(4);
    let cmds = session.command_strings();
    for cmd in &cmds {
        if cmd.contains("split-window") {
            assert!(
                !cmd.contains(" -p "),
                "split-window must not use deprecated -p flag (fails on Linux tmux 3.4 headless); got: {cmd}"
            );
        }
    }
}

/// AC: Supervisor session passes -x/-y to new-session for headless
/// environments.
#[test]
fn supervisor_new_session_passes_explicit_x_and_y() {
    let session = build_for(2);
    let cmds = session.command_strings();
    let new_session_cmd = cmds
        .iter()
        .find(|c| c.contains("new-session"))
        .expect("supervisor build emits a new-session command");
    assert!(
        new_session_cmd.contains("-x 480"),
        "supervisor new-session must pass -x 480; got: {new_session_cmd}"
    );
    assert!(
        new_session_cmd.contains("-y 140"),
        "supervisor new-session must pass -y 140; got: {new_session_cmd}"
    );
}

/// AC: Supervisor session sets global default-size after new-session.
#[test]
fn supervisor_sets_default_size_after_new_session() {
    let session = build_for(2);
    let cmds = session.command_strings();
    let new_session_idx = cmds
        .iter()
        .position(|c| c.contains("new-session"))
        .expect("new-session in command list");
    let default_size_idx = cmds
        .iter()
        .position(|c| {
            c.contains("set-option") && c.contains("default-size") && c.contains("480x140")
        })
        .expect("set-option default-size 200x50 in command list");
    assert!(
        default_size_idx > new_session_idx,
        "set-option default-size must come AFTER new-session; got order new={new_session_idx}, default-size={default_size_idx}"
    );
}

// Maps to scenario `Broker enabled in bare-start mode adds dashboard as
// pane 0` from supervisor-as-pane. The bare-start tmux build uses
// `TmuxSessionBuilder::add_pane(...)` in source order — production code
// adds the dashboard pane first when broker is enabled. We mirror that
// order in the test fixture so the pane-index contract is asserted.
// (test-coverage-v0-5-0 task 12.1)
#[test]
fn bare_start_with_broker_places_dashboard_at_pane_0() {
    // Mirror cmd_start with broker enabled: dashboard first, then agents.
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("dashboard", "/repo", "git-paw __dashboard"))
        .add_pane(make_pane("feat/a", "/tmp/wt-a", "claude"))
        .add_pane(make_pane("feat/b", "/tmp/wt-b", "claude"))
        .add_pane(make_pane("feat/c", "/tmp/wt-c", "claude"))
        .build()
        .expect("session builds");

    let cmds = session.command_strings();
    let dashboard_send = cmds
        .iter()
        .find(|c| c.contains("send-keys") && c.contains("__dashboard"))
        .expect("dashboard send-keys present");
    assert!(
        dashboard_send.contains(":0.0 "),
        "dashboard pane must be index 0; got: {dashboard_send}"
    );
    // Each agent pane carries its worktree on the `split-window -c`
    // (the pane is created in the worktree directly to avoid the
    // `cd && cli` send-keys race) AND has a `select-pane -T` at the
    // expected pane index.
    for (pane_idx, branch_marker, worktree) in [
        (1, "feat/a", "/tmp/wt-a"),
        (2, "feat/b", "/tmp/wt-b"),
        (3, "feat/c", "/tmp/wt-c"),
    ] {
        let select_target = format!(":0.{pane_idx} ");
        assert!(
            cmds.iter()
                .any(|c| c.contains(&select_target) && c.contains(branch_marker)),
            "agent {branch_marker} should land at pane {pane_idx}; cmds:\n{cmds:#?}"
        );
        let split_marker = format!("-c {worktree}");
        assert!(
            cmds.iter()
                .any(|c| c.contains("split-window") && c.contains(&split_marker)),
            "agent {branch_marker} split should carry {split_marker}; cmds:\n{cmds:#?}"
        );
    }
}

// Maps to scenario `Broker disabled produces no dashboard pane` from
// supervisor-as-pane. (test-coverage-v0-5-0 task 12.2)
#[test]
fn broker_disabled_produces_no_dashboard_pane() {
    let session = TmuxSessionBuilder::new("proj")
        .add_pane(make_pane("feat/a", "/tmp/wt-a", "claude"))
        .add_pane(make_pane("feat/b", "/tmp/wt-b", "claude"))
        .add_pane(make_pane("feat/c", "/tmp/wt-c", "claude"))
        .build()
        .expect("session builds");

    let cmds = session.command_strings();
    assert!(
        !cmds.iter().any(|c| c.contains("__dashboard")),
        "broker disabled must not add a dashboard pane; got cmds:\n{cmds:#?}"
    );
    // Three send-keys (one per agent pane), no dashboard send-keys.
    let send_keys: Vec<&String> = cmds.iter().filter(|c| c.contains("send-keys")).collect();
    assert_eq!(
        send_keys.len(),
        3,
        "broker-disabled launch with 3 agents must emit 3 send-keys; got: {send_keys:#?}"
    );
}

// Maps to scenario `Dashboard pane title` from supervisor-as-pane.
// (test-coverage-v0-5-0 task 12.3)
#[test]
fn dashboard_pane_has_title_dashboard() {
    // Use the supervisor layout (the dashboard-bearing argv builder).
    let session = build_for(2);
    let cmds = session.command_strings();
    let dashboard_select = cmds
        .iter()
        .find(|c| {
            c.contains("select-pane")
                && c.contains(":0.1")
                && c.contains("-T")
                && c.contains("dashboard")
        })
        .unwrap_or_else(|| panic!("expected select-pane -T dashboard at :0.1; cmds:\n{cmds:#?}"));
    // The shipped title shape is `<branch> → <cli_command>` with branch =
    // "dashboard". Confirm the title argument contains the bare word.
    assert!(
        dashboard_select.contains("dashboard"),
        "dashboard pane title must include `dashboard`; got: {dashboard_select}"
    );
}

/// Sanity: `env_vars` surface as set-environment commands BEFORE any
/// agent-pane send-keys, so coding agents inherit `GIT_PAW_BROKER_URL`.
#[test]
fn supervisor_layout_emits_env_before_agent_send_keys() {
    let session = build_for(3);
    let cmds = session.command_strings();
    let first_env = cmds
        .iter()
        .position(|c| c.contains("set-environment") && c.contains("GIT_PAW_BROKER_URL"))
        .expect("set-environment GIT_PAW_BROKER_URL present");
    let first_agent_send = cmds
        .iter()
        .position(|c| c.contains("send-keys") && c.contains(":0.2 "))
        .expect("first agent send-keys at :0.2");
    assert!(
        first_env < first_agent_send,
        "set-environment must come before agent-pane send-keys"
    );
}

// -----------------------------------------------------------------------
// Convention enforcement (cold-start-ci-parity §3): every `new-session`
// command produced by every builder in this module SHALL pass `-x`/`-y`
// (headless tmux needs explicit size) and `-c <cwd>` (avoid the
// send-keys cd race).
//
// Every new builder that emits `new-session` MUST be added to
// `every_new_session_command()` below so these tests cover it.
// -----------------------------------------------------------------------

/// Collect every `new-session` argv string produced by every public
/// builder in this module. Add the next builder's output here when a
/// new entry point is introduced.
fn every_new_session_command() -> Vec<(&'static str, String)> {
    let mut found: Vec<(&'static str, String)> = Vec::new();

    // Builder 1: basic TmuxSessionBuilder.
    let basic = TmuxSessionBuilder::new("conv-basic")
        .add_pane(make_pane("main", "/tmp/wt-basic", "claude"))
        .build()
        .expect("basic builder produces a session");
    for cmd in basic.command_strings() {
        if cmd.contains("new-session") {
            found.push(("TmuxSessionBuilder::build", cmd));
        }
    }

    // Builder 2: supervisor-mode layout. Build a small variant so the
    // sample is fast; the new-session shape doesn't depend on agent
    // count.
    let layout = crate::supervisor::layout::supervisor_layout(2).expect("layout");
    let (supervisor, dashboard, agents) = make_layout_panes(2);
    let supervisor_session = build_supervisor_session(
        "conv-supervisor",
        None,
        &supervisor,
        &dashboard,
        &agents,
        layout,
        true,
        true,
        &[],
    )
    .expect("supervisor builder produces a session");
    for cmd in supervisor_session.command_strings() {
        if cmd.contains("new-session") {
            found.push(("build_supervisor_session", cmd));
        }
    }

    assert!(
        !found.is_empty(),
        "expected at least one new-session command from the audited builders"
    );
    found
}

/// Every `new-session` argv SHALL carry `-x` and `-y` so tmux can size
/// the session without an attached client. Regression guard for the
/// v0.5.0 `Tmux error: size missing` cold-start bug.
#[test]
fn every_new_session_passes_x_and_y() {
    for (builder, cmd) in every_new_session_command() {
        assert!(
            cmd.contains(" -x ") || cmd.ends_with(" -x"),
            "{builder}: new-session must pass -x; got: {cmd}"
        );
        assert!(
            cmd.contains(" -y ") || cmd.ends_with(" -y"),
            "{builder}: new-session must pass -y; got: {cmd}"
        );
    }
}

/// Every `new-session` argv SHALL carry `-c <cwd>` so pane 0 starts in
/// the agent's worktree without a follow-up `cd` send-keys race. Bug B
/// regression guard from the v0.5.0 dogfood report.
#[test]
fn every_new_session_passes_c() {
    for (builder, cmd) in every_new_session_command() {
        assert!(
            cmd.contains(" -c "),
            "{builder}: new-session must pass -c <cwd>; got: {cmd}"
        );
    }
}

/// Bug B regression coverage: every agent pane SHALL be created with
/// `-c <agent.worktree>` on its split, and the follow-up `send-keys`
/// SHALL NOT use the `cd <worktree> && <cli>` race chain.
#[test]
fn supervisor_layout_agent_splits_carry_worktree_no_cd_chain() {
    let layout = crate::supervisor::layout::supervisor_layout(2).expect("layout");
    let supervisor = make_pane("supervisor", "/repo", "claude");
    let dashboard = make_pane("dashboard", "/repo", "git-paw __dashboard");
    let agent_a = make_pane("feat/a", "/tmp/wt-a", "claude");
    let agent_b = make_pane("feat/b", "/tmp/wt-b", "claude");
    let session = build_supervisor_session(
        "proj",
        None,
        &supervisor,
        &dashboard,
        &[agent_a, agent_b],
        layout,
        true,
        true,
        &[],
    )
    .expect("session builds");

    let cmds = session.command_strings();
    let splits = commands_containing(&cmds, "split-window");
    assert!(
        splits.iter().any(|c| c.contains("-c /tmp/wt-a")),
        "split for agent a should pass -c /tmp/wt-a; splits: {splits:#?}"
    );
    assert!(
        splits.iter().any(|c| c.contains("-c /tmp/wt-b")),
        "split for agent b should pass -c /tmp/wt-b; splits: {splits:#?}"
    );

    let send_keys: Vec<String> = commands_containing(&cmds, "send-keys")
        .into_iter()
        .filter(|c| !c.trim_end().ends_with("C-u"))
        .collect();
    for entry in &send_keys {
        assert!(
            !entry.contains("cd /tmp/wt-a &&"),
            "no send-keys should chain `cd /tmp/wt-a &&`; got: {entry}"
        );
        assert!(
            !entry.contains("cd /tmp/wt-b &&"),
            "no send-keys should chain `cd /tmp/wt-b &&`; got: {entry}"
        );
    }
}

// -- add/remove re-tile builders (git-paw-add D1/D6) --

#[test]
fn add_agent_same_row_splits_horizontally_from_previous_pane() {
    // 4 agents already present (single row, indices 2..=5); adding a 5th
    // (agent index 4) stays in the same row -> horizontal split from the
    // immediately-preceding pane (index 5), new pane at index 6.
    let layout = crate::supervisor::layout::layout_for(5).expect("layout");
    let new_agent = make_pane("feat/fifth", "/tmp/wt5", "claude");
    let session = build_add_agent_commands("paw-x", &new_agent, 4, layout, true);
    let cmds = session.command_strings();

    assert!(
        cmds.iter().any(|c| c.contains("split-window")
            && c.contains("-h")
            && c.contains(":0.5")
            && c.contains("-c /tmp/wt5")),
        "5th agent should -h split from pane 5 with -c worktree; cmds:\n{cmds:#?}"
    );
    // New pane is targeted at index 6 for title + launch.
    assert!(
        cmds.iter()
            .any(|c| c.contains("send-keys") && c.contains(":0.6") && c.contains("claude")),
        "new agent CLI should launch in pane 6; cmds:\n{cmds:#?}"
    );
}

#[test]
fn add_agent_new_row_splits_vertically_from_previous_row_first_pane() {
    // 5 agents present (one full row, indices 2..=6); adding a 6th (agent
    // index 5) starts a new row -> vertical split from the previous row's
    // first pane (index 2).
    let layout = crate::supervisor::layout::layout_for(6).expect("layout");
    let new_agent = make_pane("feat/sixth", "/tmp/wt6", "claude");
    let session = build_add_agent_commands("paw-x", &new_agent, 5, layout, false);
    let cmds = session.command_strings();

    assert!(
        cmds.iter().any(|c| c.contains("split-window")
            && c.contains("-v")
            && c.contains(":0.2")
            && c.contains("-c /tmp/wt6")),
        "6th agent should -v split from pane 2 (prev row first); cmds:\n{cmds:#?}"
    );
}

#[test]
fn add_agent_reapplies_row_height_resize_pass() {
    // The re-tile must end with the same per-row resize pass start uses:
    // one resize for the top row (:0.0) at top_row_pct, one per agent row.
    let layout = crate::supervisor::layout::layout_for(5).expect("layout");
    let new_agent = make_pane("feat/fifth", "/tmp/wt5", "claude");
    let session = build_add_agent_commands("paw-x", &new_agent, 4, layout, false);
    let cmds = session.command_strings();

    let top_pct = format!("{}%", layout.top_row_pct);
    assert!(
        cmds.iter()
            .any(|c| c.contains("resize-pane") && c.contains(":0.0") && c.contains(&top_pct)),
        "re-tile should resize the top row to {top_pct}; cmds:\n{cmds:#?}"
    );
}

#[test]
fn remove_retile_emits_resize_pass_for_remaining_count() {
    // After removing one of 5 agents, the grid re-tiles to the 4-agent
    // layout: a top-row resize plus one agent-row resize (single row).
    let layout = crate::supervisor::layout::layout_for(4).expect("layout");
    let session = build_remove_retile_commands("paw-x", 4, layout);
    let cmds = session.command_strings();

    let top_pct = format!("{}%", layout.top_row_pct);
    assert!(
        cmds.iter()
            .any(|c| c.contains("resize-pane") && c.contains(":0.0") && c.contains(&top_pct)),
        "remove re-tile should resize the top row; cmds:\n{cmds:#?}"
    );
    // 4 agents -> 1 agent row -> exactly one agent-row resize (pane :0.2).
    assert!(
        cmds.iter()
            .any(|c| c.contains("resize-pane") && c.contains(":0.2")),
        "remove re-tile should resize the first agent row (pane 2); cmds:\n{cmds:#?}"
    );
}

#[test]
fn remove_retile_with_zero_remaining_is_empty() {
    let layout = crate::supervisor::layout::layout_for(1).expect("layout");
    let session = build_remove_retile_commands("paw-x", 0, layout);
    assert!(
        session.command_strings().is_empty(),
        "removing the last agent leaves the top row untouched (no re-tile)"
    );
}

// -----------------------------------------------------------------------
// G3 — per-row equal-width rebalance arithmetic (design D4)
// -----------------------------------------------------------------------

/// A 3-agent single row resizes the first two agent panes to an equal
/// third (the last absorbs the remainder), so the row is equal thirds —
/// NOT the raw 50/25/25 a chain of `-h` splits produces. At a 480-col
/// window: (480-2)/3 = 159 cols each; the remainder pane lands at 162,
/// within a one-column-per-pane tolerance of the 160 ideal.
#[test]
fn agent_row_widths_three_agents_equal_thirds() {
    let targets = agent_row_widths(480, 3);
    // Panes :0.2 and :0.3 resized; :0.4 omitted (absorbs remainder).
    assert_eq!(targets, vec![(2, 159), (3, 159)]);
}

/// A full 5-agent row targets an equal fifth per pane (resizing the first
/// four). At 480 cols: (480-4)/5 = 95 each.
#[test]
fn agent_row_widths_five_agents_equal_fifths() {
    let targets = agent_row_widths(480, 5);
    assert_eq!(targets, vec![(2, 95), (3, 95), (4, 95), (5, 95)]);
}

/// A single agent needs no rebalance; two rows (6 agents = 5 + 1) rebalance
/// only the full first row — the lone second-row pane (:0.7) is omitted.
#[test]
fn agent_row_widths_skips_single_pane_rows() {
    assert!(
        agent_row_widths(480, 1).is_empty(),
        "a lone agent needs no width rebalance"
    );
    let targets = agent_row_widths(480, 6);
    // Row 1 panes :0.2..:0.5 resized (4); row 2's lone pane :0.7 omitted.
    assert_eq!(targets.len(), 4, "only the full first row rebalances");
    assert!(
        targets.iter().all(|(idx, _)| *idx >= 2 && *idx <= 5),
        "no second-row pane resized; got {targets:?}"
    );
}

/// The rebalance never targets the top row (panes 0/1), so the
/// supervisor/dashboard 50/50 split is untouched (spec 4.3 / D4).
#[test]
fn agent_row_widths_never_touch_top_row() {
    for n in 1..=crate::supervisor::layout::SUPERVISOR_MAX_AGENTS {
        for (idx, _) in agent_row_widths(480, n) {
            assert!(idx >= 2, "rebalance must skip panes 0 and 1 for n={n}");
        }
    }
}

/// No agent row exceeds 5 panes, so the smallest equal-width target stays
/// at ~20% of the window (design D5). At a 480-col window every per-pane
/// target is >= floor(480/5)-ish; assert none drops below 20% of width.
#[test]
fn agent_row_widths_minimum_is_one_fifth() {
    let window = 480usize;
    let floor = window / 5; // 96 cols = 20%
    for n in 1..=crate::supervisor::layout::SUPERVISOR_MAX_AGENTS {
        for (idx, cols) in agent_row_widths(window, n) {
            assert!(
                cols + 1 >= floor,
                "pane {idx} width {cols} below the ~20% floor ({floor}) for n={n}"
            );
        }
    }
}

/// A zero-width window (no live geometry) yields no resize targets.
#[test]
fn agent_row_widths_zero_window_is_empty() {
    assert!(agent_row_widths(0, 5).is_empty());
}

// -----------------------------------------------------------------------
// 6.5 — JSON↔tmux reconciliation (design D3, G2b)
// -----------------------------------------------------------------------

#[test]
fn reconcile_reports_agent_with_no_live_pane() {
    let agents = vec![
        ("feat/a".to_string(), PathBuf::from("/tmp/wt-a")),
        ("feat/b".to_string(), PathBuf::from("/tmp/wt-b")),
        ("feat/c".to_string(), PathBuf::from("/tmp/wt-c")),
    ];
    // Live panes map only to a and c; b's pane was dropped.
    let live = vec![PathBuf::from("/tmp/wt-a"), PathBuf::from("/tmp/wt-c")];
    let missing = agents_without_live_pane(&agents, &live);
    assert_eq!(
        missing,
        vec!["feat/b".to_string()],
        "only b has no live pane"
    );
}

#[test]
fn reconcile_passes_when_every_agent_maps() {
    let agents = vec![
        ("feat/a".to_string(), PathBuf::from("/tmp/wt-a")),
        ("feat/b".to_string(), PathBuf::from("/tmp/wt-b")),
    ];
    let live = vec![
        PathBuf::from("/tmp/wt-b"),
        PathBuf::from("/tmp/wt-a"),
        PathBuf::from("/tmp/wt-supervisor"),
    ];
    assert!(
        agents_without_live_pane(&agents, &live).is_empty(),
        "all agents map to a live pane → no divergence"
    );
}

// -----------------------------------------------------------------------
// 6.6 — launch-readiness gate (design D1, G1)
// -----------------------------------------------------------------------

fn test_budget() -> ReadinessBudget {
    // Tiny budget so the IO-free generic core runs instantly: timeout =
    // 2 poll intervals, one relaunch attempt.
    ReadinessBudget {
        poll_interval: Duration::from_millis(1),
        timeout: Duration::from_millis(2),
        relaunch_attempts: 1,
    }
}

#[test]
fn classify_distinguishes_ready_bareshell_indeterminate() {
    assert_eq!(
        classify_pane_readiness("some banner\n? for shortcuts\n"),
        PaneReadiness::Ready
    );
    assert_eq!(
        classify_pane_readiness("user@host ~/repo % "),
        PaneReadiness::BareShell
    );
    assert_eq!(
        classify_pane_readiness("\n\n   \n"),
        PaneReadiness::Indeterminate,
        "a blank/clearing screen is not yet a bare shell"
    );
}

#[test]
fn gate_returns_ready_without_relaunch_when_marker_present() {
    let mut relaunches = 0;
    let outcome = gate_pane_generic(
        test_budget(),
        || Some("welcome\n? for shortcuts".to_string()),
        || relaunches += 1,
        |_| {},
    );
    assert_eq!(outcome, GateOutcome::Ready);
    assert_eq!(relaunches, 0, "a ready pane is never relaunched");
}

#[test]
fn gate_relaunches_a_persistent_bare_shell_then_falls_back() {
    let mut relaunches = 0;
    let outcome = gate_pane_generic(
        test_budget(),
        || Some("user@host:~$ ".to_string()),
        || relaunches += 1,
        |_| {},
    );
    assert_eq!(
        outcome,
        GateOutcome::FellBack,
        "a never-ready bare shell falls back after the relaunch budget"
    );
    assert_eq!(
        relaunches, 1,
        "exactly one relaunch fires (relaunch_attempts = 1)"
    );
}

#[test]
fn gate_does_not_relaunch_an_unrecognised_cli() {
    let mut relaunches = 0;
    // Content that is neither a known marker nor an obvious shell prompt.
    let outcome = gate_pane_generic(
        test_budget(),
        || Some("custom-cli interactive session — type a command".to_string()),
        || relaunches += 1,
        |_| {},
    );
    assert_eq!(outcome, GateOutcome::FellBack);
    assert_eq!(
        relaunches, 0,
        "an unrecognised (indeterminate) CLI falls back without relaunching"
    );
}

#[test]
fn gate_becomes_ready_after_a_relaunch() {
    let mut captures = 0;
    let mut relaunches = 0;
    let outcome = gate_pane_generic(
        test_budget(),
        || {
            captures += 1;
            // Bare shell until the relaunch fires (~4 captures for the
            // first attempt's poll loop + final classification), then the
            // CLI marker appears.
            if captures > 4 {
                Some("? for shortcuts".to_string())
            } else {
                Some("user@host:~$ ".to_string())
            }
        },
        || relaunches += 1,
        |_| {},
    );
    assert_eq!(outcome, GateOutcome::Ready);
    assert_eq!(
        relaunches, 1,
        "the bare shell was relaunched once before going ready"
    );
}

// ---------------------------------------------------------------------------
// Runtime path behind the `CommandRunner` seam (code-analysis-refactor R3)
//
// The session/readiness/layout operations shell out to a live tmux server, so
// before the seam their argv and their success/failure branches were only
// reachable end-to-end. These assert the exact argv git-paw emits plus each
// documented reaction to tmux's output — no live server involved.
// ---------------------------------------------------------------------------

use crate::command_runner::CommandOutput;
use crate::command_runner::test_support::FakeCommandRunner;

/// A scripted tmux result: exit 0 with the given stdout.
fn ok_out(stdout: &str) -> CommandOutput {
    CommandOutput {
        success: true,
        code: Some(0),
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
    }
}

/// A scripted tmux result: exit 1 with the given stderr.
fn err_out(stderr: &str) -> CommandOutput {
    CommandOutput {
        success: false,
        code: Some(1),
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

/// The argv of the single call the fake recorded.
fn only_call(fake: &FakeCommandRunner) -> (String, Vec<String>) {
    let calls = fake.calls();
    assert_eq!(calls.len(), 1, "expected exactly one tmux invocation");
    calls.into_iter().next().unwrap()
}

#[test]
fn is_session_alive_probes_has_session_and_maps_exit_status() {
    let alive = FakeCommandRunner::succeeding("");
    assert!(is_session_alive_with(&alive, "paw-proj").unwrap());
    assert_eq!(
        only_call(&alive),
        (
            "tmux".to_string(),
            vec![
                "has-session".to_string(),
                "-t".to_string(),
                "paw-proj".to_string()
            ]
        )
    );

    let gone = FakeCommandRunner::failing("can't find session");
    assert!(
        !is_session_alive_with(&gone, "paw-proj").unwrap(),
        "a non-zero has-session exit means the session is not alive"
    );
}

#[test]
fn is_session_alive_surfaces_a_spawn_failure_as_an_error() {
    let broken = FakeCommandRunner::scripted(|_, _| {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "tmux missing",
        ))
    });
    let err = is_session_alive_with(&broken, "paw-proj").unwrap_err();
    assert!(
        matches!(&err, PawError::TmuxError(m) if m.contains("failed to run tmux")),
        "unexpected error: {err:?}"
    );
}

#[test]
fn session_liveness_distinguishes_alive_stale_and_indeterminate() {
    let alive = FakeCommandRunner::succeeding("");
    assert_eq!(
        session_liveness_with(&alive, "paw-proj"),
        SessionLiveness::Alive
    );

    let stale = FakeCommandRunner::failing("can't find session");
    assert_eq!(
        session_liveness_with(&stale, "paw-proj"),
        SessionLiveness::Stale
    );

    // A probe that could not run at all is NOT evidence the session died.
    let unspawnable = FakeCommandRunner::scripted(|_, _| {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "tmux missing",
        ))
    });
    assert_eq!(
        session_liveness_with(&unspawnable, "paw-proj"),
        SessionLiveness::Indeterminate
    );
}

#[test]
fn attach_sends_attach_session_and_names_the_session_on_failure() {
    let ok = FakeCommandRunner::succeeding("");
    assert!(attach_with(&ok, "paw-proj").is_ok());
    assert_eq!(
        only_call(&ok),
        (
            "tmux".to_string(),
            vec![
                "attach-session".to_string(),
                "-t".to_string(),
                "paw-proj".to_string()
            ]
        )
    );

    let gone = FakeCommandRunner::failing("no such session");
    let err = attach_with(&gone, "paw-proj").unwrap_err();
    assert!(
        matches!(&err, PawError::TmuxError(m) if m.contains("paw-proj")),
        "the failure must name the session: {err:?}"
    );
}

#[test]
fn detach_client_targets_the_session_and_treats_no_clients_as_a_no_op() {
    let ok = FakeCommandRunner::succeeding("");
    assert!(detach_client_with(&ok, "paw-proj").is_ok());
    assert_eq!(
        only_call(&ok),
        (
            "tmux".to_string(),
            vec![
                "detach-client".to_string(),
                "-s".to_string(),
                "paw-proj".to_string()
            ]
        )
    );

    for benign in ["no clients attached", "no current client"] {
        let idempotent = FakeCommandRunner::failing(benign);
        assert!(
            detach_client_with(&idempotent, "paw-proj").is_ok(),
            "'{benign}' is the already-detached no-op case"
        );
    }

    let real = FakeCommandRunner::failing("  server exited unexpectedly  ");
    let err = detach_client_with(&real, "paw-proj").unwrap_err();
    assert!(
        matches!(&err, PawError::TmuxError(m) if m == "server exited unexpectedly"),
        "a genuine failure surfaces trimmed stderr: {err:?}"
    );
}

#[test]
fn kill_pane_addresses_session_window_pane_and_tolerates_a_missing_pane() {
    let ok = FakeCommandRunner::succeeding("");
    assert!(kill_pane_with(&ok, "paw-proj", 3).is_ok());
    assert_eq!(
        only_call(&ok),
        (
            "tmux".to_string(),
            vec![
                "kill-pane".to_string(),
                "-t".to_string(),
                "paw-proj:0.3".to_string()
            ]
        )
    );

    for benign in ["can't find pane", "no such pane", "pane not found"] {
        let idempotent = FakeCommandRunner::failing(benign);
        assert!(
            kill_pane_with(&idempotent, "paw-proj", 3).is_ok(),
            "'{benign}' is the already-gone no-op case"
        );
    }

    let real = FakeCommandRunner::failing("permission denied");
    assert!(kill_pane_with(&real, "paw-proj", 3).is_err());
}

#[test]
fn kill_pane_by_id_targets_the_pane_id_and_tolerates_a_missing_pane() {
    let ok = FakeCommandRunner::succeeding("");
    assert!(kill_pane_by_id_with(&ok, "%7").is_ok());
    assert_eq!(
        only_call(&ok),
        (
            "tmux".to_string(),
            vec!["kill-pane".to_string(), "-t".to_string(), "%7".to_string()]
        )
    );

    let gone = FakeCommandRunner::failing("can't find pane %7");
    assert!(
        kill_pane_by_id_with(&gone, "%7").is_ok(),
        "a pane that has already gone is an idempotent no-op"
    );
}

#[test]
fn kill_session_targets_the_session_and_surfaces_trimmed_stderr() {
    let ok = FakeCommandRunner::succeeding("");
    assert!(kill_session_with(&ok, "paw-proj").is_ok());
    assert_eq!(
        only_call(&ok),
        (
            "tmux".to_string(),
            vec![
                "kill-session".to_string(),
                "-t".to_string(),
                "paw-proj".to_string()
            ]
        )
    );

    let gone = FakeCommandRunner::failing("  can't find session: paw-proj\n");
    let err = kill_session_with(&gone, "paw-proj").unwrap_err();
    assert!(
        matches!(&err, PawError::TmuxError(m) if m == "can't find session: paw-proj"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn list_panes_requests_the_id_path_format_and_parses_the_pairs() {
    let fake = FakeCommandRunner::scripted(|_, _| Ok(ok_out("%0 /repo\n%1 /repo/wt-a\n")));
    let panes = list_panes_with_paths_with(&fake, "paw-proj").unwrap();
    assert_eq!(
        panes,
        vec![
            ("%0".to_string(), "/repo".to_string()),
            ("%1".to_string(), "/repo/wt-a".to_string()),
        ]
    );
    assert_eq!(
        only_call(&fake),
        (
            "tmux".to_string(),
            vec![
                "list-panes".to_string(),
                "-t".to_string(),
                "paw-proj".to_string(),
                "-F".to_string(),
                "#{pane_id} #{pane_current_path}".to_string(),
            ]
        )
    );
}

#[test]
fn list_panes_degrades_to_empty_when_the_session_or_server_is_gone() {
    for benign in ["can't find session", "no such window", "no server running"] {
        let fake = FakeCommandRunner::failing(benign);
        assert_eq!(
            list_panes_with_paths_with(&fake, "paw-proj").unwrap(),
            Vec::new(),
            "'{benign}' degrades to no live panes rather than failing"
        );
    }

    let real = FakeCommandRunner::failing("  bad -F format  ");
    let err = list_panes_with_paths_with(&real, "paw-proj").unwrap_err();
    assert!(
        matches!(&err, PawError::TmuxError(m) if m == "bad -F format"),
        "a genuine tmux failure still surfaces: {err:?}"
    );
}

#[test]
fn list_panes_skips_lines_without_a_space_separator() {
    let fake = FakeCommandRunner::scripted(|_, _| Ok(ok_out("%0 /repo\ngarbage\n\n%1 /repo/wt\n")));
    assert_eq!(
        list_panes_with_paths_with(&fake, "paw-proj").unwrap(),
        vec![
            ("%0".to_string(), "/repo".to_string()),
            ("%1".to_string(), "/repo/wt".to_string()),
        ]
    );
}

#[test]
fn resolve_session_name_returns_the_base_when_it_is_free() {
    // has-session fails => nothing occupies the name.
    let free = FakeCommandRunner::failing("can't find session");
    assert_eq!(
        resolve_session_name_with(&free, "proj").unwrap(),
        "paw-proj"
    );
}

#[test]
fn resolve_session_name_walks_past_occupied_names() {
    // `paw-proj` and `paw-proj-2` are taken; `paw-proj-3` is free.
    let fake = FakeCommandRunner::scripted(|_, args| {
        let target = args.last().copied().unwrap_or_default();
        if target == "paw-proj" || target == "paw-proj-2" {
            Ok(ok_out(""))
        } else {
            Ok(err_out("can't find session"))
        }
    });
    assert_eq!(
        resolve_session_name_with(&fake, "proj").unwrap(),
        "paw-proj-3"
    );
}

#[test]
fn resolve_session_name_gives_up_after_too_many_collisions() {
    // Every candidate is occupied.
    let all_taken = FakeCommandRunner::succeeding("");
    let err = resolve_session_name_with(&all_taken, "proj").unwrap_err();
    assert!(
        matches!(&err, PawError::TmuxError(m) if m.contains("too many session name collisions")),
        "unexpected error: {err:?}"
    );
}

#[test]
fn resolve_pane_id_matches_the_worktree_path_and_reports_none_otherwise() {
    let tmp = tempfile::tempdir().unwrap();
    let wt = tmp.path().join("wt-a");
    std::fs::create_dir(&wt).unwrap();
    let listing = format!("%0 /elsewhere\n%4 {}\n", wt.display());

    let fake = FakeCommandRunner::scripted(move |_, _| Ok(ok_out(&listing)));
    assert_eq!(
        resolve_pane_id_for_worktree_with(&fake, "paw-proj", &wt).unwrap(),
        Some("%4".to_string())
    );

    let none = FakeCommandRunner::scripted(|_, _| Ok(ok_out("%0 /elsewhere\n")));
    assert_eq!(
        resolve_pane_id_for_worktree_with(&none, "paw-proj", &wt).unwrap(),
        None,
        "no live pane for the worktree makes removal an idempotent no-op"
    );
}

#[test]
fn reconcile_reports_only_agents_whose_worktree_has_no_live_pane() {
    let tmp = tempfile::tempdir().unwrap();
    let live = tmp.path().join("wt-live");
    let dead = tmp.path().join("wt-dead");
    std::fs::create_dir(&live).unwrap();
    std::fs::create_dir(&dead).unwrap();

    let listing = format!("%1 {}\n", live.display());
    let fake = FakeCommandRunner::scripted(move |_, _| Ok(ok_out(&listing)));
    let agents = vec![
        ("feat/live".to_string(), live.clone()),
        ("feat/dead".to_string(), dead.clone()),
    ];
    assert_eq!(
        reconcile_agents_to_panes_with(&fake, "paw-proj", &agents).unwrap(),
        vec!["feat/dead".to_string()]
    );
}

#[test]
fn relaunch_clears_the_input_line_before_sending_the_cli_command() {
    let fake = FakeCommandRunner::succeeding("");
    relaunch_cli_into_pane(
        &fake,
        "paw-proj",
        2,
        "claude --dangerously-skip-permissions",
    );
    assert_eq!(
        fake.calls(),
        vec![
            (
                "tmux".to_string(),
                vec![
                    "send-keys".to_string(),
                    "-t".to_string(),
                    "paw-proj:0.2".to_string(),
                    "C-u".to_string(),
                ]
            ),
            (
                "tmux".to_string(),
                vec![
                    "send-keys".to_string(),
                    "-t".to_string(),
                    "paw-proj:0.2".to_string(),
                    "claude --dangerously-skip-permissions".to_string(),
                    "Enter".to_string(),
                ]
            ),
        ]
    );
}

#[test]
fn relaunch_swallows_tmux_failures_so_the_fallback_injection_proceeds() {
    let failing = FakeCommandRunner::failing("can't find pane");
    // Best-effort: no panic, and both sends are still attempted.
    relaunch_cli_into_pane(&failing, "paw-proj", 9, "claude");
    assert_eq!(failing.calls().len(), 2);
}

#[test]
fn rebalance_queries_the_window_width_then_resizes_all_but_the_last_pane_in_a_row() {
    // 80 columns, 2 agents => one row of 2 panes; only the first is resized,
    // to (80 - 1 separator) / 2 = 39 columns.
    let fake = FakeCommandRunner::scripted(|_, args| {
        if args.first() == Some(&"display-message") {
            Ok(ok_out("80\n"))
        } else {
            Ok(ok_out(""))
        }
    });
    rebalance_agent_rows_with(&fake, "paw-proj", 2).unwrap();

    let calls = fake.calls();
    assert_eq!(
        calls[0],
        (
            "tmux".to_string(),
            vec![
                "display-message".to_string(),
                "-p".to_string(),
                "-t".to_string(),
                "paw-proj:0".to_string(),
                "#{window_width}".to_string(),
            ]
        )
    );
    let resizes: Vec<Vec<String>> = calls[1..].iter().map(|(_, args)| args.clone()).collect();
    assert_eq!(
        resizes,
        vec![vec![
            "resize-pane".to_string(),
            "-t".to_string(),
            "paw-proj:0.2".to_string(),
            "-x".to_string(),
            "39".to_string(),
        ]],
        "exactly the panes agent_row_widths names are resized"
    );
}

#[test]
fn rebalance_is_a_no_op_when_the_window_is_gone() {
    let gone = FakeCommandRunner::failing("can't find window");
    rebalance_agent_rows_with(&gone, "paw-proj", 3).unwrap();
    assert_eq!(
        gone.calls().len(),
        1,
        "only the width query runs; no pane is resized"
    );
}

#[test]
fn rebalance_is_a_no_op_for_a_single_agent() {
    let fake = FakeCommandRunner::scripted(|_, args| {
        if args.first() == Some(&"display-message") {
            Ok(ok_out("80\n"))
        } else {
            Ok(ok_out(""))
        }
    });
    rebalance_agent_rows_with(&fake, "paw-proj", 1).unwrap();
    assert_eq!(
        fake.calls().len(),
        1,
        "a lone agent already spans the row; nothing to rebalance"
    );
}

// -----------------------------------------------------------------------
// Spec: safe-process-invocation — "Session names are sanitized to a
// tmux-safe form". Reproducing tests for the unsanitized-session-name bug:
// a project directory named `My Project` or `my.app` produced a session
// name tmux either refuses or whose `session:0.N` pane targets are
// ambiguous (`.` and `:` are tmux's window/pane separators).
// -----------------------------------------------------------------------

#[test]
fn awkward_project_names_yield_tmux_safe_session_names_and_pane_targets() {
    for (project, expected) in [
        ("My Project", "paw-My-Project"),
        ("my.app", "paw-my-app"),
        // Behavior-preserving: a well-formed name is byte-identical.
        ("git-paw", "paw-git-paw"),
    ] {
        let session = TmuxSessionBuilder::new(project)
            .add_pane(make_pane("main", "/tmp/wt0", "claude"))
            .add_pane(make_pane("feat/api", "/tmp/wt1", "codex"))
            .build()
            .unwrap();

        assert_eq!(session.name, expected, "project: {project}");
        assert!(
            !session
                .name
                .contains(|c: char| c == '.' || c == ':' || c.is_whitespace()),
            "session name '{}' must be a valid tmux target (no . : or whitespace)",
            session.name
        );

        // Every pane target derived from the name is unambiguous.
        let cmds = session.command_strings();
        for pane in 0..2 {
            let target = format!("{expected}:0.{pane}");
            assert!(
                cmds.iter().any(|c| c.contains(&target)),
                "pane target '{target}' missing from commands for project '{project}'"
            );
        }
    }
}

#[test]
#[serial_test::serial]
fn a_session_for_a_dotted_project_name_resolves_its_pane_targets() {
    // A `.` in the session name makes tmux read `paw-my.app` as
    // session `paw-my` + pane `app`, so `split-window`/`select-layout`
    // fail with "can't find pane: app" and the second pane is never
    // created. Sanitizing the name to `paw-my-app` fixes it.
    let session = TmuxSessionBuilder::new("my.app")
        .add_pane(make_pane("main", "/tmp", "echo one"))
        .add_pane(make_pane("feat/api", "/tmp", "echo two"))
        .build()
        .unwrap();
    let name = session.name.clone();
    cleanup_session(&name);

    session
        .execute()
        .expect("a dotted project name must build a live session");
    assert!(is_session_alive(&name).unwrap());

    // Each `session:0.N` target addresses a real pane.
    for pane in 0..2 {
        let target = format!("{name}:0.{pane}");
        let probe = std::process::Command::new("tmux")
            .args(["display-message", "-t", &target, "-p", "#{pane_index}"])
            .output()
            .expect("probe pane target");
        assert!(
            probe.status.success(),
            "pane target '{target}' did not resolve: {}",
            String::from_utf8_lossy(&probe.stderr)
        );
    }

    cleanup_session(&name);
}

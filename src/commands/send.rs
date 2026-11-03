//! `git paw __send` — type a message into a coding agent with verified
//! submission.
//!
//! The backend of the bundled `sweep.sh tell` verb: the orchestrator's one way
//! to type a message into a worker. It reuses the drive loop's
//! submit-and-verify actuator ([`submit_and_verify`]) with the target agent's
//! own CLI profile, and is guarded like an approval (see [`deliver`] and
//! [`run`]). The message arrives on stdin, so it is not in `__send`'s argv.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::command_runner::CommandRunner;
use crate::commands::output::Output;
use crate::config::{CompiledPromptProfile, CustomCli, resolve_submit_delay_ms};
use crate::error::PawError;
use crate::supervisor::approval_gate::live_prompt_in_tail;
use crate::supervisor::claim::PaneClaim;
use crate::supervisor::drive::{
    Clock, INJECTED_TEXT_MAX_CHARS, PaneCapture, SubmitPort, SubmitResult,
    input_box_shows_placeholder, input_box_text, pane_is_responsive, submit_and_verify,
};
use crate::supervisor::keys::KeyDispatcher;
use crate::supervisor::permission_prompt::is_option_line;
use crate::terminal::TerminalBackend;

/// Exit status of a delivered message.
pub const EXIT_DELIVERED: i32 = 0;
/// Exit status of a message still unsubmitted after every bounded attempt.
pub const EXIT_WEDGED: i32 = 2;
/// Exit status of a refused send (no keystroke was sent).
pub const EXIT_REFUSED: i32 = 3;

/// The result of one `__send`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendResult {
    /// The message was verified submitted as a user turn.
    Delivered,
    /// Keystrokes went out but the message was not confirmed submitted; the
    /// note says why.
    Wedged(String),
    /// Nothing was sent, for the stated reason.
    Refused(String),
}

impl SendResult {
    /// The one line printed for this result.
    #[must_use]
    pub fn line(&self) -> String {
        match self {
            SendResult::Delivered => "delivered".to_string(),
            SendResult::Wedged(_) => "wedged".to_string(),
            SendResult::Refused(reason) => format!("refused: {reason}"),
        }
    }

    /// The process exit status for this result: `0`, `2` or `3`.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            SendResult::Delivered => EXIT_DELIVERED,
            SendResult::Wedged(_) => EXIT_WEDGED,
            SendResult::Refused(_) => EXIT_REFUSED,
        }
    }
}

/// How many trailing non-blank lines are searched for a selection menu.
const MENU_TAIL_LINES: usize = 15;

/// Whether `capture` shows a selection menu: a cursor (`❯`, `›` or `>`) on a
/// numbered option, or an `Enter to select` footer. An `Enter` typed into one
/// picks an option instead of submitting text.
fn menu_showing(capture: &str, profile: &CompiledPromptProfile) -> bool {
    let lines: Vec<&str> = capture.lines().filter(|l| !l.trim().is_empty()).collect();
    let tail = &lines[lines.len().saturating_sub(MENU_TAIL_LINES)..];
    tail.iter().any(|line| {
        let trimmed = strip_decoration_keep_cursor(line);
        let cursor = trimmed
            .strip_prefix('❯')
            .or_else(|| trimmed.strip_prefix('›'))
            .or_else(|| trimmed.strip_prefix('>'));
        cursor.is_some_and(|rest| is_option_line(rest.trim_start(), profile))
            || line.to_ascii_lowercase().contains("enter to select")
    })
}

/// Trims box-drawing borders and whitespace but keeps a cursor glyph, which
/// is what marks a numbered line as a live menu.
fn strip_decoration_keep_cursor(line: &str) -> &str {
    line.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '│' | '┃' | '|'))
}

/// The reason a fresh capture makes the pane unsafe to press a key on.
fn unsafe_to_press(capture: &str, profile: &CompiledPromptProfile) -> Option<String> {
    if live_prompt_in_tail(capture, profile) {
        Some("permission prompt showing".to_string())
    } else if menu_showing(capture, profile) {
        Some("selection menu showing".to_string())
    } else {
        None
    }
}

/// Whether the first input-box row is drawn dim or grey in `styled` — a
/// CLI's prompt suggestion, not text anyone typed.
fn composer_text_is_dim(styled: &str, first_row: &str) -> bool {
    let Some(line) = styled
        .lines()
        .rev()
        .find(|l| crate::replay::strip_ansi(l).contains(first_row))
    else {
        return false;
    };
    let cells = styled_cells(line);
    let plain: String = cells.iter().map(|(c, _)| *c).collect();
    let Some(byte_at) = plain.find(first_row) else {
        return false;
    };
    let start = plain[..byte_at].chars().count();
    cells
        .iter()
        .skip(start)
        .take(first_row.chars().count())
        .filter(|(c, _)| !c.is_whitespace())
        .all(|(_, dim)| *dim)
}

/// Each visible character of `line` with whether it is drawn dim or grey.
fn styled_cells(line: &str) -> Vec<(char, bool)> {
    let mut cells = Vec::new();
    let mut dim = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            cells.push((c, dim));
            continue;
        }
        if chars.peek() != Some(&'[') {
            chars.next();
            continue;
        }
        chars.next();
        let mut params = String::new();
        let mut final_byte = None;
        for p in chars.by_ref() {
            if ('@'..='~').contains(&p) {
                final_byte = Some(p);
                break;
            }
            params.push(p);
        }
        if final_byte == Some('m') {
            dim = apply_sgr(dim, &params);
        }
    }
    cells
}

/// Applies one SGR parameter list to the dim/grey state.
fn apply_sgr(mut dim: bool, params: &str) -> bool {
    let codes: Vec<u16> = params.split(';').map(|p| p.parse().unwrap_or(0)).collect();
    let mut i = 0;
    while i < codes.len() {
        match codes[i] {
            2 | 90 => dim = true,
            0 | 22 | 39 | 30..=37 | 91..=97 => dim = false,
            38 => match codes.get(i + 1) {
                Some(5) => {
                    let n = codes.get(i + 2).copied().unwrap_or(0);
                    dim = n == 8 || (232..=250).contains(&n);
                    i += 2;
                }
                Some(2) => {
                    let (r, g, b) = (
                        codes.get(i + 2).copied().unwrap_or(0),
                        codes.get(i + 3).copied().unwrap_or(0),
                        codes.get(i + 4).copied().unwrap_or(0),
                    );
                    dim = r == g && g == b && r < 200;
                    i += 4;
                }
                _ => {}
            },
            _ => {}
        }
        i += 1;
    }
    dim
}

/// Evidence of a submitted user turn: how many lines of `capture` show
/// `message` (by its first characters), and how many show a paste
/// placeholder. A new line of either after the send is a new turn.
fn turn_evidence(capture: &str, message: &str, profile: &CompiledPromptProfile) -> (usize, usize) {
    let head: String = message.chars().take(20).collect();
    let lines = || capture.lines();
    (
        lines().filter(|line| line.contains(&head)).count(),
        lines()
            .filter(|line| profile.paste_buffer_regex().is_match(line))
            .count(),
    )
}

/// One coding agent in the session roster, as the launch recorded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterAgent {
    /// The agent's broker id (the slugified branch).
    pub agent_id: String,
    /// The worktree root its surface was started in.
    pub worktree: PathBuf,
    /// The CLI the launch recorded for it.
    pub cli: String,
}

/// What a send targets once the session is resolved.
pub struct SendTarget<'a> {
    /// The repository's main working tree (the claim lives under it).
    pub repo_root: &'a Path,
    /// The terminal session name.
    pub session: &'a str,
    /// The session's coding agents.
    pub roster: &'a [RosterAgent],
    /// The `[clis.*]` table the agents' profiles resolve against.
    pub clis: &'a HashMap<String, CustomCli>,
}

/// Flattens `message` to one line, so an embedded newline cannot submit part
/// of it early.
fn one_line(message: &str) -> String {
    message.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A control character typed literally could act as a key.
fn check_message(message: &str) -> Option<SendResult> {
    let count = message.chars().count();
    if count == 0 {
        return Some(SendResult::Refused("empty message".to_string()));
    }
    if message.chars().any(char::is_control) {
        return Some(SendResult::Refused(
            "message contains control characters".to_string(),
        ));
    }
    if count > INJECTED_TEXT_MAX_CHARS {
        return Some(SendResult::Refused(format!(
            "message is {count} characters, over the {INJECTED_TEXT_MAX_CHARS}-character \
             injected-text limit; deliver it with the broker feedback delivery mode instead"
        )));
    }
    None
}

/// Types `message` into `agent_id`'s surface and verifies it submitted, or
/// refuses without sending a keystroke.
///
/// The pane's [`PaneClaim`] is held for the whole sequence.
///
/// # Errors
/// Returns an error when the agent's prompt profile does not compile or a
/// keystroke cannot be dispatched.
pub fn deliver<B: TerminalBackend>(
    target: &SendTarget<'_>,
    backend: &B,
    clock: &dyn Clock,
    agent_id: &str,
    message: &str,
) -> Result<SendResult, PawError> {
    let message = one_line(message);
    if let Some(refused) = check_message(&message) {
        return Ok(refused);
    }
    let Some(agent) = target.roster.iter().find(|a| a.agent_id == agent_id) else {
        return Ok(SendResult::Refused(format!(
            "`{agent_id}` is not a coding agent in session {}",
            target.session
        )));
    };
    let Some(pane_index) = surface_of(backend, target.session, agent) else {
        return Ok(SendResult::Refused(format!(
            "no live surface for agent `{agent_id}`"
        )));
    };
    let profile =
        crate::agent_cli::CliProfile::resolve(&agent.cli, target.clis).compiled_prompt_profile()?;
    let settle_delay = Duration::from_millis(resolve_submit_delay_ms(&agent.cli, target.clis));

    let Some(_claim) = PaneClaim::try_acquire(target.repo_root, pane_index) else {
        return Ok(SendResult::Refused("pane busy".to_string()));
    };
    let surface = backend.surface_at(target.session, pane_index);
    let Some(styled) = backend.capture_styled(&surface) else {
        return Ok(SendResult::Refused(
            "the pane could not be read".to_string(),
        ));
    };
    let capture = crate::replay::strip_ansi(&styled);
    if let Some(reason) = unsafe_to_press(&capture, &profile) {
        return Ok(SendResult::Refused(reason));
    }
    if !pane_is_responsive(&capture, &profile) {
        return Ok(SendResult::Refused("agent is mid-response".to_string()));
    }
    // Never type over text someone else left in the input line: appending
    // would submit a line neither party wrote, and recovery would clear it.
    // A dim prompt suggestion is not typed text.
    if let Some(text) = input_box_text(&capture, &profile)
        && !input_box_shows_placeholder(&capture, &profile)
        && !composer_text_is_dim(&styled, &text)
    {
        return Ok(SendResult::Refused(
            "input line holds unsubmitted text".to_string(),
        ));
    }
    let evidence_before = turn_evidence(&capture, &message, &profile);

    let guard = |capture: &str| unsafe_to_press(capture, &profile);
    let mut dispatcher = BackendDispatcher(backend);
    let mut port = SubmitPort {
        capturer: backend,
        dispatcher: &mut dispatcher,
        clock,
        guard: Some(&guard),
    };
    let submitted = submit_and_verify(
        &mut port,
        target.session,
        pane_index,
        &message,
        settle_delay,
        &profile,
    )?;
    Ok(match submitted {
        // An empty or unrecognised input box is not proof on its own: the
        // message must show up as a new user turn.
        SubmitResult::Delivered => {
            let after = backend.capture(target.session, pane_index);
            let (text_after, placeholders_after) = turn_evidence(&after, &message, &profile);
            if text_after > evidence_before.0 || placeholders_after > evidence_before.1 {
                SendResult::Delivered
            } else {
                SendResult::Wedged(
                    "the input emptied but the message does not show as a submitted turn"
                        .to_string(),
                )
            }
        }
        SubmitResult::Wedged => SendResult::Wedged(
            "the message is still on the input line after every retry".to_string(),
        ),
        SubmitResult::Aborted(reason) => SendResult::Wedged(format!(
            "stopped before the next key: {reason}; the message may sit unsubmitted on the input line"
        )),
    })
}

/// The position of the live surface `agent`'s worktree owns, matched by the
/// core owner rule.
fn surface_of<B: TerminalBackend>(
    backend: &B,
    session: &str,
    agent: &RosterAgent,
) -> Option<usize> {
    let roster = [(agent.agent_id.as_str(), agent.worktree.as_path())];
    backend
        .list_surfaces(session)
        .ok()?
        .into_iter()
        .find(|s| {
            s.owner
                .as_deref()
                .is_some_and(|owner| crate::surface::resolve_agent(owner, roster).is_some())
        })
        .map(|s| s.position)
}

/// Lends a shared backend as the actuator's keystroke dispatcher; the port's
/// operations take `&self`, so the same backend is also the capturer.
struct BackendDispatcher<'a, B: TerminalBackend>(&'a B);

impl<B: TerminalBackend> KeyDispatcher for BackendDispatcher<'_, B> {
    fn send_key(&mut self, session: &str, pane_index: usize, key: &str) -> std::io::Result<()> {
        crate::supervisor::keys::dispatch_key(self.0, session, pane_index, key)
    }

    fn send_text(&mut self, session: &str, pane_index: usize, text: &str) -> std::io::Result<()> {
        crate::supervisor::keys::dispatch_text(self.0, session, pane_index, text)
    }
}

/// Runs `git paw __send` from `cwd`: refuses from a linked worktree, resolves
/// the repository's session and roster, then [`deliver`]s.
///
/// # Errors
/// Returns an error when `cwd` is not in a repository, the config or session
/// file cannot be read, or [`deliver`] fails.
pub fn run<B: TerminalBackend>(
    runner: &dyn CommandRunner,
    cwd: &Path,
    backend: &B,
    clock: &dyn Clock,
    agent_id: &str,
    message: &str,
) -> Result<SendResult, PawError> {
    let layout = crate::cli_setup::shell::resolve_layout(runner, cwd)?;
    if layout.is_linked_worktree() {
        return Ok(SendResult::Refused(
            "run sweep.sh tell from the repository's main working tree, not from a linked worktree"
                .to_string(),
        ));
    }
    let repo_root = crate::git::validate_repo(cwd)?;
    let Some(session) = crate::session::find_session_for_repo(&repo_root)? else {
        return Ok(SendResult::Refused(
            "no git-paw session for this repository".to_string(),
        ));
    };
    let config = crate::config::load_config(&repo_root, None)?;
    let roster: Vec<RosterAgent> = session
        .worktrees
        .iter()
        .map(|w| RosterAgent {
            agent_id: crate::broker::messages::slugify_branch(&w.branch),
            worktree: w.worktree_path.clone(),
            cli: w.cli.clone(),
        })
        .collect();
    let target = SendTarget {
        repo_root: &repo_root,
        session: &session.session_name,
        roster: &roster,
        clis: &config.clis,
    };
    deliver(&target, backend, clock, agent_id, message)
}

/// `git paw __send --agent <id>`: sends `message` from `cwd` through tmux,
/// writes the one-line result to `out`, and returns the exit status (`0`,
/// `2` or `3`). The entry layer reads `cwd` and the message from stdin.
///
/// # Errors
/// Returns an error when [`run`] fails.
pub fn send(
    cwd: &Path,
    agent_id: &str,
    message: &str,
    out: &mut dyn Output,
) -> Result<i32, PawError> {
    let result = run(
        &crate::command_runner::RealCommandRunner,
        cwd,
        &crate::terminal::tmux::TmuxBackend::new(),
        &crate::supervisor::drive::SystemClock,
        agent_id,
        message,
    )?;
    out.line(&result.line());
    for note in notes(&result, message) {
        out.warn(&note);
    }
    Ok(result.exit_code())
}

/// The stderr notes that accompany `result`: a flattened multi-line message,
/// and why a send is wedged.
fn notes(result: &SendResult, message: &str) -> Vec<String> {
    let mut notes = Vec::new();
    if message.trim().contains('\n') && result.exit_code() != EXIT_REFUSED {
        notes.push("note: the multi-line message was typed as one line".to_string());
    }
    if let SendResult::Wedged(why) = result {
        notes.push(format!("note: {why}"));
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::fake::{FakeBackend, Operation};
    use std::time::Instant;

    struct NoSleep;
    impl Clock for NoSleep {
        fn now(&self) -> Instant {
            Instant::now()
        }
        fn sleep(&self, _dur: Duration) {}
    }

    const SESSION: &str = "paw-x";

    fn idle_box(text: &str) -> String {
        format!(
            "╭────────────────────────────────╮\n│ > {text}                       │\n╰────────────────────────────────╯\n  ? for shortcuts"
        )
    }

    fn empty_box() -> String {
        idle_box("")
    }

    fn live_prompt() -> String {
        "Bash command\n\n  rm -rf target\n\nDo you want to proceed?\n❯ 1. Yes\n  2. No, and tell Claude what to do differently (esc)".to_string()
    }

    /// A repository root, a worktree for `feat-x`, and a fake session whose
    /// surface 1 is the dashboard (no owner) and 2 is `feat-x`.
    struct Fixture {
        _dir: tempfile::TempDir,
        repo: PathBuf,
        roster: Vec<RosterAgent>,
        clis: HashMap<String, CustomCli>,
        backend: FakeBackend,
    }

    fn fixture(screens: &[Option<&str>]) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let worktree = dir.path().join("wt-feat-x");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&worktree).unwrap();
        let backend = FakeBackend::new();
        backend.add_surface(SESSION, 1, None);
        let surface = backend.add_surface(SESSION, 2, Some(&worktree));
        backend.script_screens(&surface, screens);
        Fixture {
            _dir: dir,
            repo,
            roster: vec![RosterAgent {
                agent_id: "feat-x".to_string(),
                worktree,
                cli: "claude".to_string(),
            }],
            clis: HashMap::new(),
            backend,
        }
    }

    fn send(fx: &Fixture, agent: &str, message: &str) -> SendResult {
        let target = SendTarget {
            repo_root: &fx.repo,
            session: SESSION,
            roster: &fx.roster,
            clis: &fx.clis,
        };
        deliver(&target, &fx.backend, &NoSleep, agent, message).unwrap()
    }

    fn keystrokes(fx: &Fixture) -> Vec<Operation> {
        fx.backend
            .operations()
            .into_iter()
            .filter(Operation::is_keystroke)
            .collect()
    }

    fn long_message() -> String {
        "please re-read the verification feedback and fix the failing gate ".repeat(7)[..400]
            .to_string()
    }

    /// The pane after `message` was submitted: a user turn above an empty
    /// input box.
    fn submitted(message: &str) -> String {
        format!("> {message}\n\n{}", empty_box())
    }

    /// Spec scenario "A paragraph-length message is submitted".
    #[test]
    fn a_paragraph_length_message_is_delivered() {
        let placeholder = idle_box("[Pasted text #1 +3 lines]");
        let done = submitted(&long_message());
        let fx = fixture(&[
            Some(&empty_box()), // pre-send gate
            Some(&placeholder), // guard before the first Enter
            Some(&placeholder), // verify: still in the paste buffer
            Some(&placeholder), // guard before C-u
            Some(&placeholder), // guard before the retry Enter
            Some(&done),        // verify
            Some(&done),        // submitted-turn check
        ]);
        let result = send(&fx, "feat-x", &long_message());
        assert_eq!(result, SendResult::Delivered);
        assert_eq!(result.line(), "delivered");
        assert_eq!(result.exit_code(), 0);
        assert!(
            keystrokes(&fx).iter().any(|op| matches!(
                op,
                Operation::SendText { text, .. } if *text == long_message()
            )),
            "the message itself was typed"
        );
    }

    /// Spec scenario "A message that never submits is reported wedged".
    #[test]
    fn a_message_that_never_submits_is_wedged() {
        let fx = fixture(&[
            Some(&empty_box()),
            Some(&idle_box("[Pasted text #1 +3 lines]")),
        ]);
        let result = send(&fx, "feat-x", &long_message());
        assert!(matches!(result, SendResult::Wedged(_)), "{result:?}");
        assert_eq!(result.line(), "wedged");
        assert_eq!(result.exit_code(), 2);
    }

    /// Spec scenario "A selection menu blocks the send": an `Enter` would
    /// pick an option, so nothing is typed.
    #[test]
    fn a_selection_menu_blocks_the_send() {
        for menu in [
            "Cache is cold. Send anyway?\n❯ 1. Send\n  2. Wait\nEnter to select",
            "Cache is cold.\n  1. Send\n  2. Wait\nEnter to select",
        ] {
            let fx = fixture(&[Some(menu)]);
            let result = send(&fx, "feat-x", "please continue");
            assert_eq!(result.line(), "refused: selection menu showing", "{menu}");
            assert!(keystrokes(&fx).is_empty());
        }
    }

    #[test]
    fn a_menu_with_an_esc_footer_is_refused_too() {
        let menu =
            "Cache is cold. Send anyway?\n❯ 1. Send\n  2. Wait\nEnter to select · Esc to cancel";
        let fx = fixture(&[Some(menu)]);
        let result = send(&fx, "feat-x", "please continue");
        assert!(matches!(result, SendResult::Refused(_)), "{result:?}");
        assert!(keystrokes(&fx).is_empty());
    }

    /// A numbered list in the agent's own output, with no cursor on it, is
    /// not a menu.
    #[test]
    fn a_numbered_list_in_the_transcript_is_not_a_menu() {
        let screen = format!(
            "Plan:\n1. read the spec\n2. write the test\n\n{}",
            empty_box()
        );
        let done = submitted("please continue");
        let fx = fixture(&[Some(&screen), Some(&screen), Some(&done), Some(&done)]);
        assert_eq!(
            send(&fx, "feat-x", "please continue"),
            SendResult::Delivered
        );
    }

    /// Spec scenario "A menu that appears mid-send stops the next key": the
    /// guard re-reads the pane before the `Enter`, and the result is wedged.
    #[test]
    fn a_menu_that_appears_before_the_enter_stops_the_send() {
        let menu = "Cache is cold. Send anyway?\n❯ 1. Send\n  2. Wait";
        let fx = fixture(&[Some(&empty_box()), Some(menu)]);
        let result = send(&fx, "feat-x", "please continue");
        assert!(matches!(result, SendResult::Wedged(_)), "{result:?}");
        let enters = keystrokes(&fx)
            .iter()
            .filter(|op| matches!(op, Operation::SendKey { .. }))
            .count();
        assert_eq!(enters, 0, "no Enter or C-u landed on the menu");
    }

    /// Spec scenario "An emptied input without a submitted turn is not
    /// delivered": the composer cleared, but the message never showed up.
    #[test]
    fn an_emptied_input_without_a_new_turn_is_wedged() {
        let fx = fixture(&[Some(&empty_box()), Some(&empty_box())]);
        let result = send(&fx, "feat-x", "please continue");
        assert!(matches!(result, SendResult::Wedged(_)), "{result:?}");
    }

    /// Spec scenario "A dim prompt suggestion does not block the send": the
    /// composer of an idle pane shows a grey suggestion, captured with its
    /// style escapes (a constructed capture in the shape `tmux capture-pane
    /// -e` returns for a dim run).
    #[test]
    fn a_dim_prompt_suggestion_does_not_block_the_send() {
        let suggestion = idle_box("\x1b[2mcommit when the suite is green\x1b[22m");
        let plain = idle_box("commit when the suite is green");
        let done = submitted("please continue");
        let fx = fixture(&[Some(&suggestion), Some(&plain), Some(&done), Some(&done)]);
        assert_eq!(
            send(&fx, "feat-x", "please continue"),
            SendResult::Delivered
        );
    }

    /// The same composer text in the normal colour is typed text, refused.
    #[test]
    fn typed_text_in_the_normal_colour_is_still_refused() {
        let typed = idle_box("\x1b[39mcommit when the suite is green\x1b[0m");
        let fx = fixture(&[Some(&typed)]);
        let result = send(&fx, "feat-x", "please continue");
        assert_eq!(result.line(), "refused: input line holds unsubmitted text");
    }

    /// Spec scenario "A multi-line message is typed as one line": the result
    /// carries a note that it was flattened, and a wedged result says why.
    #[test]
    fn notes_flag_a_flattened_message_and_a_wedged_reason() {
        let flattened = notes(&SendResult::Delivered, "first\nsecond");
        assert_eq!(
            flattened,
            ["note: the multi-line message was typed as one line"]
        );
        assert!(notes(&SendResult::Delivered, "one line").is_empty());
        assert!(notes(&SendResult::Refused("x".into()), "a\nb").is_empty());
        let wedged = notes(&SendResult::Wedged("still on the input line".into()), "m");
        assert_eq!(wedged, ["note: still on the input line"]);
    }

    #[test]
    fn grey_sgr_forms_read_as_dim() {
        for sgr in ["2", "90", "38;5;244", "38;2;128;128;128", "1;2"] {
            let line = format!("> \x1b[{sgr}mhint\x1b[0m");
            assert!(composer_text_is_dim(&line, "hint"), "{sgr}");
        }
        for sgr in ["0", "97", "38;5;15", "38;2;200;100;100"] {
            let line = format!("> \x1b[{sgr}mtext\x1b[0m");
            assert!(!composer_text_is_dim(&line, "text"), "{sgr}");
        }
    }

    /// Spec scenario "A live permission prompt blocks the send".
    #[test]
    fn a_live_permission_prompt_blocks_the_send() {
        let fx = fixture(&[Some(&live_prompt())]);
        let result = send(&fx, "feat-x", "please continue");
        assert_eq!(result.line(), "refused: permission prompt showing");
        assert_eq!(result.exit_code(), 3);
        assert!(keystrokes(&fx).is_empty(), "no keystroke on a live prompt");
    }

    /// Spec scenario "A non-agent surface is refused": the dashboard and an
    /// id no roster agent carries.
    #[test]
    fn a_non_agent_target_is_refused() {
        for target in ["dashboard", "supervisor", "feat-unknown"] {
            let fx = fixture(&[Some(&empty_box())]);
            let result = send(&fx, target, "please continue");
            assert!(
                matches!(result, SendResult::Refused(_)),
                "{target}: {result:?}"
            );
            assert_eq!(result.exit_code(), 3);
            assert!(keystrokes(&fx).is_empty(), "{target}: no keystroke");
        }
    }

    /// A roster agent whose surface is gone is refused, not typed at a
    /// guessed position.
    #[test]
    fn an_agent_with_no_live_surface_is_refused() {
        let mut fx = fixture(&[Some(&empty_box())]);
        fx.roster[0].worktree = fx.repo.join("elsewhere");
        let result = send(&fx, "feat-x", "please continue");
        assert!(matches!(result, SendResult::Refused(_)), "{result:?}");
        assert!(keystrokes(&fx).is_empty());
    }

    /// Spec scenario "A held claim is refused, not raced".
    #[test]
    fn a_held_claim_is_refused() {
        let fx = fixture(&[Some(&empty_box())]);
        let _held = PaneClaim::try_acquire(&fx.repo, 2).unwrap();
        let result = send(&fx, "feat-x", "please continue");
        assert_eq!(result.line(), "refused: pane busy");
        assert_eq!(result.exit_code(), 3);
        assert!(keystrokes(&fx).is_empty(), "no keystroke on a busy pane");
    }

    /// Spec scenario "An over-long message is refused".
    #[test]
    fn an_over_long_message_is_refused_with_a_pointer_to_feedback_mode() {
        let fx = fixture(&[Some(&empty_box())]);
        let message = "x".repeat(INJECTED_TEXT_MAX_CHARS + 1);
        let result = send(&fx, "feat-x", &message);
        assert_eq!(result.exit_code(), 3);
        assert!(
            result.line().contains("broker feedback delivery mode"),
            "{}",
            result.line()
        );
        assert!(keystrokes(&fx).is_empty());
    }

    /// Spec scenario "A mid-response agent is refused".
    #[test]
    fn a_mid_response_agent_is_refused() {
        let busy = format!("{}\n  ✻ Working… (esc to interrupt)", empty_box());
        let fx = fixture(&[Some(&busy)]);
        let result = send(&fx, "feat-x", "please continue");
        assert_eq!(result.line(), "refused: agent is mid-response");
        assert!(keystrokes(&fx).is_empty());
    }

    /// Spec scenario "Text already in the input line is refused".
    #[test]
    fn text_already_in_the_input_line_is_refused() {
        let fx = fixture(&[Some(&idle_box("half-typed by a human"))]);
        let result = send(&fx, "feat-x", "please continue");
        assert_eq!(result.line(), "refused: input line holds unsubmitted text");
        assert!(keystrokes(&fx).is_empty());
    }

    /// A stuck paste placeholder may be re-sent over: it is buffered content
    /// waiting for a submit, not someone's half-typed line.
    #[test]
    fn a_stuck_paste_placeholder_does_not_block_a_resend() {
        let placeholder = idle_box("[Pasted text #1 +3 lines]");
        let done = submitted(&long_message());
        let fx = fixture(&[
            Some(&placeholder),
            Some(&placeholder),
            Some(&done),
            Some(&done),
        ]);
        let result = send(&fx, "feat-x", &long_message());
        assert_eq!(result, SendResult::Delivered);
    }

    /// Spec scenario "An empty or control-character message is refused".
    #[test]
    fn a_message_with_a_control_character_is_refused() {
        let fx = fixture(&[Some(&empty_box())]);
        let result = send(&fx, "feat-x", "please\u{1b}continue");
        assert_eq!(
            result.line(),
            "refused: message contains control characters"
        );
        assert!(keystrokes(&fx).is_empty());
    }

    #[test]
    fn an_empty_message_is_refused() {
        let fx = fixture(&[Some(&empty_box())]);
        let result = send(&fx, "feat-x", "  \n ");
        assert_eq!(result.exit_code(), 3);
        assert!(keystrokes(&fx).is_empty());
    }

    /// A multi-line message is typed as one line, so a newline cannot submit
    /// part of it early.
    #[test]
    fn a_multi_line_message_is_typed_as_one_line() {
        let done = submitted("first line second line");
        let fx = fixture(&[
            Some(&empty_box()),
            Some(&empty_box()),
            Some(&done),
            Some(&done),
        ]);
        let result = send(&fx, "feat-x", "first line\nsecond line");
        assert_eq!(result, SendResult::Delivered);
        assert!(keystrokes(&fx).iter().any(|op| matches!(
            op,
            Operation::SendText { text, .. } if text == "first line second line"
        )));
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir)
            .env_remove("GIT_DIR")
            .env_remove("GIT_COMMON_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    /// Spec scenario "A send from a linked worktree is refused".
    #[test]
    fn a_send_from_a_linked_worktree_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q"]);
        git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);
        let linked = dir.path().join("linked");
        git(
            &repo,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "feat-y",
                linked.to_str().unwrap(),
            ],
        );
        let backend = FakeBackend::new();
        let result = run(
            &crate::command_runner::RealCommandRunner,
            &linked,
            &backend,
            &NoSleep,
            "feat-y",
            "please continue",
        )
        .unwrap();
        assert_eq!(result.exit_code(), 3);
        assert!(
            result.line().starts_with("refused:") && result.line().contains("main working tree"),
            "{}",
            result.line()
        );
        assert!(
            backend.operations().iter().all(|op| !op.is_keystroke()),
            "no keystroke from a linked worktree"
        );
    }
}

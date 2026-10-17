//! Permission-prompt detection.
//!
//! The one module that answers "is a permission prompt on screen, and which
//! command is it about?" for every feature that needs it: capture pane via
//! `tmux capture-pane -p -t <session>:<pane>`, scan for the resolved CLI
//! profile's approval markers, decide whether the prompt is LIVE (the
//! live-prompt gate), read the CLI's idle input box, and extract the prompted
//! command slice or file-operation target path.
//!
//! Detection is intentionally only a thin wrapper around `tmux capture-pane`
//! and string scanning — no parsing of agent CLI internals. Every marker and
//! pattern is sourced from the resolved `cli-prompt-profiles` profile, except
//! the security-relevant window sizes and structural rules, which stay
//! compiled.
//!
//! Detection makes no safety decision about the prompted command. What the
//! prompt is asking for is decided by the agent CLI's own permission system or
//! by a human — never here.

use std::process::Command;
use std::sync::OnceLock;

use regex::Regex;

use crate::config::{CliPromptProfile, claude_prompt_profile};
use crate::static_regex::static_regex;

/// Captures the content of pane `pane_index` in `session` via
/// `tmux capture-pane -p -t <session>:<pane>`.
///
/// Returns `None` when tmux is unavailable, the target does not exist, or
/// the pane has no content. The function never panics on tmux exit codes —
/// any error is treated as "no content available".
#[must_use]
pub fn capture_pane(session: &str, pane_index: usize) -> Option<String> {
    let target = format!("{session}:0.{pane_index}");
    let output = Command::new("tmux")
        .args(["capture-pane", "-p", "-t", &target])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?;
    Some(s)
}

/// Returns whether `captured` carries one of `profile`'s approval markers —
/// that is, whether the pane is showing a permission prompt at all.
///
/// This reports PRESENCE and nothing else. It deliberately does not say what
/// the prompt is asking for: git-paw makes no safety decision about an
/// agent's command, so there is no command class to report and no caller that
/// could act on one. Whether the command runs is the agent CLI's decision or
/// the human's.
///
/// Markers come from the resolved `cli-prompt-profiles` profile, so a pane
/// running a CLI other than Claude Code is recognised by its own wording.
/// Pure function — exposed separately from any capture so unit tests can
/// drive it without touching tmux.
#[must_use]
pub fn has_approval_marker(captured: &str, profile: &CliPromptProfile) -> bool {
    profile
        .approval_markers
        .iter()
        .any(|m| captured.contains(m.as_str()))
}

/// Compiled regex matching the lead-in of a Claude filesystem-operation
/// permission prompt and capturing the target path.
///
/// Covers the documented write / edit / create prompt shapes, e.g.
/// `"Do you want to allow this write to <path>?"` and
/// `"Do you want to make this edit to <path>?"`. The capture group runs to
/// the end of the line (minus a trailing `?`), so the path may contain
/// spaces. Matching is case-insensitive.
///
/// Sourced from the embedded Claude Code profile's `file_prompt_pattern`
/// (`cli-prompt-profiles` capability) rather than a compiled literal.
fn file_prompt_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| static_regex(claude_prompt_profile().file_prompt_pattern.as_str()))
}

/// Extracts the target path from a Claude filesystem-operation permission
/// prompt, or `None` when the prompt is not a recognised file-op prompt.
///
/// The prompt text is scanned line by line; the first line whose lead-in
/// matches a documented write / edit / create shape yields its captured
/// path. Surrounding quotes and backticks are stripped. The returned string
/// is the raw path as it appeared in the prompt — resolving it against a
/// worktree root is the caller's job.
#[must_use]
pub fn extract_path_from_file_prompt(prompt: &str) -> Option<String> {
    let re = file_prompt_regex();
    for line in prompt.lines() {
        if let Some(caps) = re.captures(line) {
            let raw = caps.get(1)?.as_str().trim();
            let cleaned = raw
                .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                .trim();
            if !cleaned.is_empty() {
                return Some(cleaned.to_string());
            }
        }
    }
    None
}

/// Strips leading TUI decoration (box-drawing glyphs, bullets, the selection
/// caret, and surrounding whitespace) from a captured line so command
/// extraction and boundary detection are not confused by pane chrome.
///
/// A bare `>` is deliberately NOT stripped: it is significant to the
/// `> /dev/` danger pattern when it appears mid-line as a redirect.
fn strip_decoration(line: &str) -> &str {
    line.trim_start_matches(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '│' | '─'
                    | '╭'
                    | '╮'
                    | '╰'
                    | '╯'
                    | '├'
                    | '┤'
                    | '┐'
                    | '└'
                    | '┘'
                    | '┌'
                    | '⎿'
                    | '❯'
                    | '●'
                    | '•'
                    | '*'
                    | '·'
            )
    })
    .trim_end()
}

/// Compiled regex for the embedded profile's `option_line_pattern`
/// (`cli-prompt-profiles` capability), matching a numbered option line
/// (`1. …`, `2) …`) once leading whitespace is trimmed.
fn option_line_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| static_regex(claude_prompt_profile().option_line_pattern.as_str()))
}

/// Returns `true` when a cleaned line is a numbered option (`1. …`, `2) …`).
fn is_option_line(line: &str) -> bool {
    option_line_regex().is_match(line.trim_start())
}

/// Returns `true` when a cleaned line marks the end of the command block —
/// the confirmation question, an approval marker, or the option list.
///
/// The confirmation-question and approval-marker checks source their
/// literals from the embedded Claude Code profile's `live_prompt_markers`
/// and `approval_markers` (`cli-prompt-profiles` capability).
fn is_confirmation_boundary(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let profile = claude_prompt_profile();
    profile
        .live_prompt_markers
        .iter()
        .any(|m| lower.starts_with(m.as_str()))
        || profile
            .approval_markers
            .iter()
            .any(|m| lower.contains(&m.to_ascii_lowercase()))
        || is_option_line(line)
}

/// Returns `true` when a cleaned line is a `Bash command` prompt header.
///
/// Sources its literal from the embedded profile's `command_header_markers`
/// (`cli-prompt-profiles` capability) — a header-style marker is one that
/// does NOT end in `(` (an inline-embedding marker like `Bash(`, handled
/// separately by [`extract_command_slice`]'s inline form).
fn is_bash_command_header(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    claude_prompt_profile()
        .command_header_markers
        .iter()
        .filter(|m| !m.ends_with('('))
        .any(|m| lower.starts_with(&m.to_ascii_lowercase()))
}

/// Extracts the prompted command slice from a pane capture: the command text
/// between the `Bash command` / `Bash(` header and the confirmation question,
/// ignoring surrounding narration.
///
/// The LAST header in the capture wins — when a pane shows earlier narration
/// followed by the live prompt, the live command is the most recent one. The
/// `Bash(<cmd>)` inline form is read from inside the parentheses; the
/// `Bash command` header form takes the first non-empty line after the header
/// (the command itself), stopping before any description line or the
/// confirmation question. Returns `None` when no recognised header is present.
#[must_use]
pub fn extract_command_slice(capture: &str) -> Option<String> {
    let inline_markers: Vec<&str> = claude_prompt_profile()
        .command_header_markers
        .iter()
        .filter(|m| m.ends_with('('))
        .map(String::as_str)
        .collect();
    let lines: Vec<&str> = capture.lines().collect();
    for (idx, raw) in lines.iter().enumerate().rev() {
        let line = strip_decoration(raw);

        // Inline `Bash(<cmd>)` form.
        if let Some((start, marker)) = inline_markers
            .iter()
            .find_map(|m| line.find(m).map(|start| (start, *m)))
        {
            let after = &line[start + marker.len()..];
            if let Some(end) = after.rfind(')') {
                let cmd = after[..end].trim();
                if !cmd.is_empty() {
                    return Some(cmd.to_string());
                }
            }
        }

        // `Bash command` header form: the command is the first non-empty
        // cleaned line that follows, up to the confirmation question.
        if is_bash_command_header(line) {
            for next in &lines[idx + 1..] {
                let cleaned = strip_decoration(next);
                if cleaned.is_empty() {
                    continue;
                }
                if is_confirmation_boundary(cleaned) {
                    break;
                }
                return Some(cleaned.to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Live-prompt gate (Section 6)
// ---------------------------------------------------------------------------

/// Trailing non-blank lines that must ANCHOR the prompt: a textual marker or
/// a numbered option line must sit within this window for the capture to be
/// live. Output printed after an answered prompt evicts it from the anchor,
/// so a prompt scrolled into history never counts.
pub const LIVE_PROMPT_TAIL: usize = 4;

/// Widest window of trailing non-blank lines inspected for the prompt's
/// textual markers — wide enough to span a full multi-option prompt block
/// (`Do you want to proceed?` + numbered options, possibly wrapped, + the
/// footer), so a prompt offering a "don't ask again" option is detected
/// rather than missed by a narrow tail.
pub const LIVE_PROMPT_BLOCK: usize = 15;

/// Widest number of EXTRA offsets searched above the capture's true end — not
/// directly the number of trailing lines tolerated. At each searched offset
/// the probe's own [`LIVE_PROMPT_TAIL`]-wide window can still catch a footer
/// up to `LIVE_PROMPT_TAIL - 1` lines further back than the offset itself, so
/// the real number of non-blank trailing lines a footer can have below it and
/// still be found LIVE is `LIVE_PROMPT_TRAILING_BOUND + LIVE_PROMPT_TAIL - 1`
/// = 13 (a footer with exactly 13 trailing lines is still live; 14 is not —
/// see `live_prompt_beyond_trailing_bound_does_not_fire` and the parity
/// test's boundary fixtures). Current Claude Code's persistent task list,
/// observed live during the v0.17.0 dogfood, renders 6 non-blank lines below
/// the input area; 13 adds headroom above that measured value rather than
/// tuning to it exactly. The tolerance is deliberately **bounded** — an
/// unbounded search would re-admit a prompt that has genuinely scrolled into
/// history, which is exactly the phantom-approval case
/// [`live_prompt_markers_at_tail`] exists to refuse.
pub(crate) const LIVE_PROMPT_TRAILING_BOUND: usize = 10;

/// Markers drawn only while a modal is actively live — never left behind by
/// an already-answered prompt — and so the only markers allowed to anchor a
/// match at a bounded offset above the capture's true end. A bare
/// confirmation question (e.g. `do you want to`) does NOT qualify: an
/// answered prompt can leave its question text sitting in the pane followed
/// by ordinary command output, and admitting that at a shifted anchor would
/// re-introduce exactly the phantom-approval risk the trailing bound exists
/// to prevent (spec scenario "An answered prompt followed by command output
/// is not live"). At the capture's true end (offset 0, nothing trails the
/// match) this restriction does not apply: a prompt with nothing printed
/// after it cannot be a stale, already-answered one.
const LIVE_PROMPT_FOOTER_MARKERS: &[&str] = &["esc to cancel"];

/// Returns `true` when `line`, after [`strip_decoration`] and an optional
/// leading `(`, STARTS WITH one of [`LIVE_PROMPT_FOOTER_MARKERS`] — exactly
/// how a live modal renders its footer (`Esc to cancel` or `(esc to
/// cancel)`), never buried mid-sentence.
///
/// This is deliberately stricter than a bare substring check. Once the
/// anchor has shifted past the capture's true end, admitting any line that
/// merely *contains* the footer text would also fire on narration (`the
/// footer reads "Esc to cancel" at the bottom`) or on grep-style output that
/// happens to echo the same text — neither of which is a live modal. Pairing
/// this with an [`is_option_line`] requirement elsewhere
/// ([`live_prompt_markers_at_tail`]) closes that gap: a shifted-anchor match
/// now requires the modal's actual shape, not a lone string.
fn is_footer_line(line: &str) -> bool {
    let stripped = strip_decoration(line);
    let stripped = stripped.strip_prefix('(').unwrap_or(stripped);
    let lower = stripped.to_ascii_lowercase();
    LIVE_PROMPT_FOOTER_MARKERS
        .iter()
        .any(|m| lower.starts_with(*m))
}

/// Returns `true` when `line`, once trimmed, consists ENTIRELY of horizontal
/// box-drawing rule glyphs. Current Claude Code's idle input box has NO side
/// borders at all — ground-truthed against a live `tmux capture-pane -p`, it
/// renders as a rule line, the sigil line, another rule line, then the mode
/// strip — so the rule line immediately above (or below) the sigil line is
/// what frames the box, not a `│` border. `pub(crate)` so `input_box_rows` in
/// `src/supervisor/drive.rs` can find the borderless box's closing rule when
/// reading wrapped continuation rows.
pub(crate) fn is_rule_line(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && trimmed.chars().all(|c| matches!(c, '─' | '━' | '═' | '-'))
}

/// Returns the text after `line`'s composer sigil (one of the embedded
/// profile's `input_sigils`), once optional `│` box-border characters are
/// trimmed from both ends — or `None` if `line` does not start with a sigil
/// either way (bordered or borderless). `pub(crate)` so `input_box_text` in
/// `src/supervisor/drive.rs` can extract the typed text once
/// [`is_idle_input_box_line`] has confirmed the shape.
pub(crate) fn strip_input_sigil(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let inner = trimmed
        .strip_prefix('│')
        .map_or(trimmed, |s| s.trim_end_matches('│').trim());
    claude_prompt_profile()
        .input_sigils
        .iter()
        .find_map(|sigil| inner.strip_prefix(sigil.as_str()))
}

/// Returns `true` when `line` is the CLI's idle input-box line, recognising
/// BOTH shapes a CLI may render: a `│`-bordered composer line (`│ > … │`),
/// or a borderless composer line framed by a horizontal rule immediately
/// above it (`preceding_non_blank_line`) — current Claude Code's idle input
/// box is the latter (see [`is_rule_line`]). Either way, a numbered option
/// line is never read as the input box, since the same selection-caret sigil
/// also prefixes option lines.
///
/// A real live modal REPLACES the input box while it is up — the two never
/// render at once. So an idle input-box line appearing between a candidate
/// footer match and the capture's true end is evidence the footer is
/// quoted/historical content (e.g. the supervisor's own pane running
/// `sweep.sh capture 2` and echoing another pane's modal as tool output,
/// with the supervisor's own idle input box below it), not a currently
/// active modal — [`live_prompt_markers_at_tail`]'s shifted-anchor path
/// rejects a match when this is found below the footer.
///
/// Shared with `input_box_text` in `src/supervisor/drive.rs` (`pub(crate)`
/// rather than duplicated), so idle-pane buffered-input detection and the
/// live-prompt gate's input-box rejection cannot drift apart on what counts
/// as "the input box".
pub(crate) fn is_idle_input_box_line(line: &str, preceding_non_blank_line: Option<&str>) -> bool {
    let Some(after_sigil) = strip_input_sigil(line) else {
        return false;
    };
    if is_option_line(after_sigil.trim_start()) {
        return false;
    }
    let bordered = line.trim().starts_with('│');
    bordered || preceding_non_blank_line.is_some_and(is_rule_line)
}

/// Tries `probe` against `nonblank` truncated at increasing offsets from the
/// end, so a caller's own tail/block windowing logic effectively anchors on
/// the **modal block** wherever it sits, rather than strictly on the
/// capture's last line. `probe` runs first against the untruncated slice
/// (offset 0 — today's behaviour, unchanged for a capture with no trailing
/// content) and then against each shorter truncation up to
/// [`LIVE_PROMPT_TRAILING_BOUND`] lines removed, returning `true` as soon as
/// `probe` does. The offset is passed to `probe` so a caller can restrict
/// what counts as a match once the anchor has shifted away from the true end
/// (see [`live_prompt_markers_at_tail`]'s footer-only restriction).
///
/// Generic over `probe` rather than inlined into
/// [`live_prompt_markers_at_tail`] so a follow-up scan over a different
/// marker set (e.g. a CLI-unavailable banner search) can reuse the same
/// bounded-trailing-region discipline without re-deriving it.
pub(crate) fn anchor_on_trailing_region<'a>(
    nonblank: &[&'a str],
    mut probe: impl FnMut(&[&'a str], usize) -> bool,
) -> bool {
    let max_offset = LIVE_PROMPT_TRAILING_BOUND.min(nonblank.len());
    (0..=max_offset).any(|offset| probe(&nonblank[..nonblank.len() - offset], offset))
}

/// Shared structural live-prompt check: `true` when, anchored at the end of
/// the capture or at some bounded offset above it
/// ([`anchor_on_trailing_region`]), a textual marker sits in the last
/// [`LIVE_PROMPT_TAIL`] non-blank lines, OR a numbered option line anchors
/// that tail while a textual marker sits within the last [`LIVE_PROMPT_BLOCK`]
/// non-blank lines (a multi-option prompt bottoms out in its option list,
/// with the question above it).
///
/// Searching upward over a bounded trailing region means a CLI that renders a
/// task list, status strip, or progress footer below its input area does not
/// hide a live modal underneath that chrome — while a prompt separated from
/// the end of the capture by more than the effective trailing tolerance (see
/// [`LIVE_PROMPT_TRAILING_BOUND`]) still fails the gate, preserving the
/// original phantom-approval guard. Critically, once the anchor has shifted
/// away from the true end, detection narrows to the modal's actual SHAPE
/// rather than any textual marker: an [`is_footer_line`] match (the footer
/// line itself, not a bare substring anywhere) AND an [`is_option_line`]
/// match within the wider block above it. Both together only render while a
/// modal is genuinely live — narration or grep-style output that merely
/// *mentions* the footer text has neither on its own, so a bare confirmation
/// question (or a footer string in prose) followed by ordinary output still
/// fails the gate no matter how small the shift.
///
/// Textual markers are matched case-insensitively as substrings. This is the
/// one structural algorithm behind [`is_live_prompt`] and the
/// `broker-mediated-approvals` re-confirm
/// ([`crate::supervisor::approval_gate::live_prompt_in_tail`]), so detection
/// and send-time re-confirm agree on what "live" means.
#[must_use]
pub fn live_prompt_markers_at_tail(capture: &str, textual_markers: &[&str]) -> bool {
    let nonblank: Vec<&str> = capture.lines().filter(|l| !l.trim().is_empty()).collect();
    let has_textual = |lines: &[&str], markers: &[&str]| {
        lines.iter().any(|line| {
            let lower = line.to_ascii_lowercase();
            markers
                .iter()
                .any(|m| lower.contains(&m.to_ascii_lowercase()))
        })
    };
    anchor_on_trailing_region(&nonblank, |lines, offset| {
        let tail = &lines[lines.len().saturating_sub(LIVE_PROMPT_TAIL)..];
        if offset == 0 {
            if has_textual(tail, textual_markers) {
                return true;
            }
            let block = &lines[lines.len().saturating_sub(LIVE_PROMPT_BLOCK)..];
            return tail.iter().any(|l| is_option_line(strip_decoration(l)))
                && has_textual(block, textual_markers);
        }
        // Beyond the true end: a bare substring match would also fire on
        // narration or tool output that happens to mention the footer text.
        // Require the modal's actual shape instead — a footer-shaped line in
        // the tail AND an option line within the wider block above it — AND
        // reject it if an idle input-box line sits anywhere between that
        // footer and the capture's true end: a real live modal replaces the
        // input box, so seeing one below the footer means the footer is a
        // quoted copy (e.g. `sweep.sh capture` output), not an active modal.
        let block = &lines[lines.len().saturating_sub(LIVE_PROMPT_BLOCK)..];
        let tail_start = lines.len() - tail.len();
        tail.iter().enumerate().any(|(i, l)| {
            is_footer_line(l)
                && block.iter().any(|l2| is_option_line(strip_decoration(l2)))
                && !(tail_start + i + 1..nonblank.len())
                    .any(|k| is_idle_input_box_line(nonblank[k], Some(nonblank[k - 1])))
        })
    })
}

/// Returns `true` when the capture shows a LIVE prompt: its structural
/// markers — the numbered option glyphs and/or the `Do you want to …`
/// question, together with the `Esc to cancel` footer — are present at the
/// capture's tail, over a window wide enough to span a full multi-option
/// prompt block (see [`live_prompt_markers_at_tail`]).
///
/// This is the precondition for any keystroke dispatch — a supervisor merely
/// narrating about a pane, or an earlier prompt that has scrolled away, will
/// not have the markers in the live window and so cannot trip a phantom
/// approval.
///
/// Sources its markers from the embedded Claude Code profile's
/// `live_prompt_markers` (`cli-prompt-profiles` capability).
#[must_use]
pub fn is_live_prompt(capture: &str) -> bool {
    let markers: Vec<&str> = claude_prompt_profile()
        .live_prompt_markers
        .iter()
        .map(String::as_str)
        .collect();
    live_prompt_markers_at_tail(capture, &markers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    /// `core-project-conventions`' "Every static pattern compiles under test":
    /// forces every static regex in this module to compile.
    #[test]
    fn static_regexes_compile() {
        assert!(file_prompt_regex().is_match("Do you want to allow this write to src/lib.rs?"));
        assert!(option_line_regex().is_match("1. Yes"));
    }

    /// Spec scenario "Approval prompt detected in working agent"
    /// (`approval-command-safety`): detection reports whether an approval
    /// marker is present, and nothing more. Every capture carrying one of the
    /// profile's markers is detected; one carrying none is not.
    #[test]
    fn approval_marker_presence_is_reported() {
        let profile = claude_prompt_profile();
        for (captured, expected) in [
            ("lorem ipsum dolor sit amet\n$ ls\n", false),
            (
                "curl http://127.0.0.1:9119/publish\nrequires approval\n",
                true,
            ),
            ("git commit -m hi\nrequires approval", true),
            (
                "do you want to proceed\nRunning cargo test --workspace",
                true,
            ),
            ("[y/N] cargo fmt --all", true),
            ("requires approval", true),
            // A dangerous-looking command with NO marker is not a prompt.
            ("rm -rf /tmp/foo\n$ ls\n", false),
        ] {
            assert_eq!(
                has_approval_marker(captured, profile),
                expected,
                "captured: {captured:?}"
            );
        }
    }

    /// Spec scenario "Detection carries no command class"
    /// (`approval-command-safety`): a `curl` prompt and an `rm -rf` prompt
    /// produce the IDENTICAL result. There is nothing in the return value a
    /// caller could branch on to treat one command differently from another —
    /// which is the whole point of retiring the classifier. If this ever
    /// needs a richer return type, that is a safety decision git-paw does not
    /// make.
    #[test]
    fn detection_carries_no_command_class() {
        let profile = claude_prompt_profile();
        let curl = "Bash command:\ncurl http://127.0.0.1:9119/publish\nrequires approval [y/N]";
        let destructive = "Bash command:\nrm -rf /\nrequires approval [y/N]";
        let secret_read = "Bash command:\ncat ~/.ssh/id_rsa\nrequires approval [y/N]";
        assert_eq!(
            has_approval_marker(curl, profile),
            has_approval_marker(destructive, profile),
            "the command text must not change the detection result"
        );
        assert_eq!(
            has_approval_marker(curl, profile),
            has_approval_marker(secret_read, profile),
            "the command text must not change the detection result"
        );
        assert!(has_approval_marker(curl, profile));
    }

    /// Detection is non-invasive: capturing a session that does not exist
    /// yields nothing rather than erroring or touching the pane.
    #[test]
    fn capture_pane_returns_none_for_nonexistent_session() {
        let out = capture_pane("paw-nonexistent-session-aabbccdd-zz", 0);
        assert!(
            out.is_none(),
            "nonexistent session should not capture, got {out:?}"
        );
    }

    #[test]
    fn extracts_path_from_write_prompt() {
        assert_eq!(
            extract_path_from_file_prompt("Do you want to allow this write to Containerfile?"),
            Some("Containerfile".to_string())
        );
    }

    #[test]
    fn extracts_path_from_edit_prompt() {
        assert_eq!(
            extract_path_from_file_prompt("Do you want to make this edit to src/main.rs?"),
            Some("src/main.rs".to_string())
        );
    }

    #[test]
    fn extracts_absolute_path_from_prompt() {
        assert_eq!(
            extract_path_from_file_prompt("Do you want to allow this write to /etc/hosts?"),
            Some("/etc/hosts".to_string())
        );
    }

    #[test]
    fn shell_prompt_yields_no_file_path() {
        // A shell-command prompt must not be mistaken for a file op.
        assert_eq!(
            extract_path_from_file_prompt("Do you want to proceed?\ncargo build --release"),
            None
        );
    }

    // --- Section 1: command-slice extraction ---

    /// Prose mentioning `rm -rf /` elsewhere in the capture is ignored: the
    /// extracted slice is the live `cargo test`, read from between the command
    /// header and the confirmation question rather than from the whole pane.
    #[test]
    fn command_slice_ignores_narration_prose() {
        let capture = "\
I will avoid running rm -rf / because it is destructive.
Let me run the tests instead.

  Bash command
    cargo test --workspace
    Run the test suite

  Do you want to proceed?
  ❯ 1. Yes
    2. No";
        let slice = extract_command_slice(capture).expect("a command slice");
        assert_eq!(slice, "cargo test --workspace");
    }

    #[test]
    fn command_slice_reads_bash_paren_form() {
        let capture = "Bash(git push origin main)\n  Push the branch\nDo you want to proceed?";
        assert_eq!(
            extract_command_slice(capture),
            Some("git push origin main".to_string())
        );
    }

    #[test]
    fn command_slice_none_without_header() {
        assert_eq!(extract_command_slice("just some prose\n$ ls\n"), None);
    }

    #[test]
    fn command_slice_prefers_last_header() {
        let capture = "\
Bash command
  git status

Bash command
  git push origin main
Do you want to proceed?";
        assert_eq!(
            extract_command_slice(capture),
            Some("git push origin main".to_string())
        );
    }

    // --- Section 6: live-prompt gate ---

    /// Spec scenario "Live prompt fires": the footer is within the last lines.
    #[test]
    fn live_prompt_with_footer_fires() {
        let capture = "\
Bash command
  cargo test
Do you want to proceed?
❯ 1. Yes
  2. No
  (esc to cancel)";
        assert!(is_live_prompt(capture));
    }

    /// Spec scenario "Footer absent does not fire".
    #[test]
    fn live_prompt_without_footer_does_not_fire() {
        let capture = "\
I might run cargo test soon.
Here is some narration about the plan.
$ ls -la
done.";
        assert!(!is_live_prompt(capture));
    }

    /// Spec scenario "Scrolled-away prompt still fails the gate" (original
    /// pinned regression fixture — restored to its original size after a
    /// gate-review finding: stretching it to 20 lines would have hidden a
    /// bug rather than exercised it). `Esc to cancel` appears, but with no
    /// option list anywhere the shape-gated extension can never succeed
    /// regardless of how close the footer sits to the end — a bare footer
    /// string with no modal around it is not a live prompt.
    #[test]
    fn live_prompt_scrolled_out_does_not_fire() {
        let capture = "\
Do you want to proceed?
  Esc to cancel
output line 1
output line 2
output line 3
output line 4
output line 5";
        assert!(
            !is_live_prompt(capture),
            "a footer with no option list anywhere is not a live modal"
        );
    }

    /// Spec scenario "Trailing task region does not hide a live modal" (task
    /// 1.1): a live multi-option prompt with a six-line trailing task-list
    /// region appended below `Esc to cancel` — within the bounded trailing
    /// region — must still be detected as live.
    #[test]
    fn live_prompt_with_trailing_task_region_still_fires() {
        let markers = ["do you want to", "esc to cancel"];
        let capture = "\
Bash command
  rm -rf /tmp/paw-build-9
Do you want to proceed?
❯ 1. Yes
  2. No
  Esc to cancel
Tasks
☐ Implement feature
☐ Write tests
☐ Update docs
main* ⇡2
Sonnet 5.1 · 42% context used";
        assert!(
            live_prompt_markers_at_tail(capture, &markers),
            "a trailing task-list region within the bound must not hide a live modal"
        );
    }

    /// Mirror-image of the fixture above with the trailing task region
    /// removed (task 1.2) — guards against over-correcting: the identical
    /// prompt with nothing below its footer must still be detected as live.
    #[test]
    fn live_prompt_without_trailing_region_still_fires() {
        let markers = ["do you want to", "esc to cancel"];
        let capture = "\
Bash command
  rm -rf /tmp/paw-build-9
Do you want to proceed?
❯ 1. Yes
  2. No
  Esc to cancel";
        assert!(live_prompt_markers_at_tail(capture, &markers));
    }

    /// Builds a live multi-option prompt (with a footer AND an option list,
    /// so the shape-gated extension can in principle succeed) followed by
    /// `n` ordinary trailing lines.
    fn multi_option_prompt_with_trailing_lines(n: usize) -> String {
        let mut capture =
            String::from("Do you want to proceed?\n❯ 1. Yes\n  2. No\n  Esc to cancel\n");
        for i in 1..=n {
            let _ = writeln!(capture, "output line {i}");
        }
        capture
    }

    /// Boundary fixture: exactly [`LIVE_PROMPT_TRAILING_BOUND`] +
    /// `LIVE_PROMPT_TAIL` - 1 = 13 trailing lines is still within the
    /// effective tolerance (see the doc comment on
    /// [`LIVE_PROMPT_TRAILING_BOUND`]) — the modal is detected as live.
    #[test]
    fn live_prompt_at_exact_trailing_bound_still_fires() {
        let markers = ["do you want to", "esc to cancel"];
        let capture = multi_option_prompt_with_trailing_lines(13);
        assert!(
            live_prompt_markers_at_tail(&capture, &markers),
            "exactly 13 trailing lines is within the effective tolerance"
        );
    }

    /// Spec scenario "Scrolled-away prompt still fails the gate" (task 1.3):
    /// one line past the boundary (14 trailing lines, `LIVE_PROMPT_TAIL` +
    /// `LIVE_PROMPT_TRAILING_BOUND`) the modal is separated from the end of
    /// the capture by more than the effective tolerance, so it SHALL NOT be
    /// detected as live — the safety property the bound exists to preserve.
    #[test]
    fn live_prompt_one_past_trailing_bound_does_not_fire() {
        let markers = ["do you want to", "esc to cancel"];
        let capture = multi_option_prompt_with_trailing_lines(14);
        assert!(
            !live_prompt_markers_at_tail(&capture, &markers),
            "14 trailing lines is one past the effective tolerance and must stay scrolled-away"
        );
    }

    /// Spec scenario "An answered prompt followed by command output is not
    /// live": the question line has no option list and no `Esc to cancel`
    /// footer after it, so the bounded trailing region must NOT admit it even
    /// though the question sits well within the bound — only the modal's
    /// structural footer can anchor a match once the anchor has shifted.
    #[test]
    fn answered_question_followed_by_output_is_not_live() {
        let capture = "\
Do you want to proceed?
ran it
line b
line c
line d
$ ";
        assert!(
            !is_live_prompt(capture),
            "a bare answered question followed by ordinary output must not read as live"
        );
    }

    /// Gate-review safety finding: narration that merely *mentions* the
    /// footer text, more than [`LIVE_PROMPT_TAIL`] lines from the end (so
    /// only the offset>0 shape-gated path can reach it) and with no option
    /// list anywhere, must NOT read as live. Before the fix a bare substring
    /// match on the shifted tail would have fired here.
    #[test]
    fn narration_mentioning_footer_text_is_not_live() {
        let capture = "\
I checked the pane and its footer currently reads \"Esc to cancel\" below it.
Nothing else is happening right now.
Just idle, waiting for the next task.
Still idle.
Still nothing.
$ ";
        assert!(
            !is_live_prompt(capture),
            "narration that merely mentions the footer text is not a live modal"
        );
    }

    /// Gate-review safety finding: grep-style output that echoes a line
    /// containing the footer text (prefixed by file/line context, so it does
    /// not START the line) must NOT read as live either — same shape-gated
    /// reasoning as the narration case above, with no option list present.
    #[test]
    fn grep_style_output_containing_footer_text_is_not_live() {
        let capture = "\
grep -rn \"esc to cancel\" src/
src/supervisor/permission_prompt.rs:1378:const LIVE_PROMPT_FOOTER_MARKERS: &[&str] = &[\"esc to cancel\"];
done searching
no further prompts found
agent waiting for next instruction
$ ";
        assert!(
            !is_live_prompt(capture),
            "grep output echoing the footer text is not a live modal"
        );
    }

    /// Gate-review safety finding: a FULL modal copied verbatim into tool
    /// output (e.g. the supervisor's own pane running `sweep.sh capture 2`,
    /// echoing another pane's footer + option lines quoted behind the tool-
    /// output corner glyph `⎿`) must NOT read as live when the supervisor's
    /// own idle input box sits below it. A real live modal replaces the
    /// input box while it is up, so seeing one below a footer match is
    /// evidence the footer is quoted/historical content.
    #[test]
    fn captured_modal_in_tool_output_with_idle_input_box_is_not_live() {
        let capture = "\
$ sweep.sh capture 2
⎿ Bash command
⎿   rm -rf /tmp/foo
⎿ Do you want to proceed?
⎿ ❯ 1. Yes
⎿   2. No
⎿ Esc to cancel
Done.
╭──────────────────────────────────────────────╮
│ >                                              │
╰──────────────────────────────────────────────╯";
        assert!(
            !is_live_prompt(capture),
            "a quoted modal above the supervisor's own idle input box is not a live prompt"
        );
    }

    /// Same scenario as above, but with current Claude Code's REAL idle
    /// input-box shape — ground-truthed against a live `tmux capture-pane
    /// -p`: NO side borders at all, just a rule line, the bare sigil line,
    /// another rule line, then the mode strip. The bordered-box test above
    /// alone would have missed this: `is_idle_input_box_line` originally
    /// required a `│` border and never matched this real shape.
    #[test]
    fn captured_modal_in_tool_output_with_borderless_idle_input_box_is_not_live() {
        let capture = "\
$ sweep.sh capture 2
⎿ Bash command
⎿   rm -rf /tmp/foo
⎿ Do you want to proceed?
⎿ ❯ 1. Yes
⎿   2. No
⎿ Esc to cancel
Done.
────────────────────────────────────────────────
❯
────────────────────────────────────────────────
  ⏵⏵ accept edits on (shift+tab to cycle) · esc to interrupt";
        assert!(
            !is_live_prompt(capture),
            "a quoted modal above the real borderless idle input box is not a live prompt"
        );
    }

    /// Spec scenario "Live multi-option prompt is detected": the proceed
    /// question sits above a 3-option list (with a "don't ask again" option)
    /// whose last line carries only an inline `(esc)` — no separate footer.
    /// The option glyphs anchor the tail; the question is within the block.
    #[test]
    fn live_prompt_multi_option_block_is_detected() {
        let capture = "\
╭──────────────────────────────────────────────────────────────╮
│ Bash command                                                 │
│   cargo test --workspace                                     │
│   Run the full test suite                                    │
│ Do you want to proceed?                                      │
│ ❯ 1. Yes                                                     │
│   2. Yes, and don't ask again for cargo test in this project │
│   3. No, and tell Claude what to do differently (esc)        │
╰──────────────────────────────────────────────────────────────╯";
        assert!(
            is_live_prompt(capture),
            "a multi-option prompt block must be detected as live"
        );
    }

    /// Spec scenario "Live multi-option prompt is detected" (footer variant):
    /// the same block with an explicit `Esc to cancel` footer line, the
    /// proceed question more than 4 non-blank lines above the bottom.
    #[test]
    fn live_prompt_multi_option_with_footer_is_detected() {
        let capture = "\
Do you want to proceed?
❯ 1. Yes
  2. Yes, and don't ask again for: git status
  3. No
  Esc to cancel";
        assert!(is_live_prompt(capture));
    }

    /// A numbered list at the tail of plain narration (no proceed question or
    /// footer anywhere in the block window) is NOT a live prompt — option
    /// glyphs alone cannot bridge to liveness.
    #[test]
    fn numbered_list_without_prompt_markers_is_not_live() {
        let capture = "\
Here is my plan:
1. run the tests
2. commit the work
3. stand by for verification";
        assert!(
            !is_live_prompt(capture),
            "a prose numbered list must not be detected as live"
        );
    }
}

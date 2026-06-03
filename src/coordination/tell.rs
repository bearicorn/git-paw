//! Pure logic for the supervisor `/tell` routing command: argument parsing
//! and delivery-mode selection (design D3).
//!
//! The keystroke send and broker publish themselves are driven by the
//! supervisor skill (shell + `tmux send-keys` + the broker publish helper);
//! the decisions that govern them are factored here so they are unit-testable
//! and reusable by future consumers.

use crate::config::TellMode;

use super::inventory::Mode;

/// A parsed `/tell <agent_id> <prompt>` directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TellCommand {
    /// The target agent identifier (first whitespace-delimited token).
    pub target_id: String,
    /// The remainder of the directive — the prompt to route, verbatim
    /// (multi-line content preserved).
    pub prompt: String,
}

/// Parses a `/tell` directive typed in the supervisor pane.
///
/// The agent identifier is the first whitespace-delimited token after
/// `/tell`; the prompt is the rest of the input (which may span multiple
/// lines). Returns `None` when the input is not a `/tell` directive, names no
/// target, or carries no prompt body.
#[must_use]
pub fn parse_tell(input: &str) -> Option<TellCommand> {
    let trimmed = input.trim_start();
    let rest = trimmed.strip_prefix("/tell")?;
    // Require a separator between `/tell` and the target so `/tellfoo` does
    // not parse.
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim_start();
    // Split the target off the first whitespace run; the prompt is everything
    // after, with only the leading separator trimmed (trailing newlines in a
    // multi-line prompt are preserved up to a final trim).
    let mut chars = rest.char_indices();
    let target_end = chars
        .find(|(_, c)| c.is_whitespace())
        .map_or(rest.len(), |(i, _)| i);
    let target_id = rest[..target_end].to_string();
    if target_id.is_empty() {
        return None;
    }
    let prompt = rest[target_end..].trim().to_string();
    if prompt.is_empty() {
        return None;
    }
    Some(TellCommand { target_id, prompt })
}

/// The delivery channel `/tell` resolves to for a given config + target mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryDecision {
    /// Publish an `agent.feedback` broker message (the safe default).
    Feedback,
    /// Inject the prompt into the target pane via `tmux send-keys`.
    SendKeys,
    /// Configured `send-keys` but the target is not in accept-edits mode, so
    /// fall back to `agent.feedback` and emit a stderr note.
    FeedbackFallback,
}

impl DeliveryDecision {
    /// Whether this decision delivers via `agent.feedback` (either the plain
    /// default or the fallback path).
    #[must_use]
    pub fn uses_feedback(self) -> bool {
        matches!(self, Self::Feedback | Self::FeedbackFallback)
    }

    /// Whether this decision is the send-keys → feedback fallback (design
    /// D3.3), which the caller pairs with a stderr note.
    #[must_use]
    pub fn is_fallback(self) -> bool {
        matches!(self, Self::FeedbackFallback)
    }

    /// The label recorded for this delivery in the learnings routing log.
    #[must_use]
    pub fn learnings_label(self) -> &'static str {
        match self {
            Self::Feedback | Self::FeedbackFallback => "feedback",
            Self::SendKeys => "send-keys",
        }
    }
}

/// Selects the `/tell` delivery mode per design D3's precedence:
///
/// 1. configured `send-keys` AND target detected `accept-edits` → `send-keys`;
/// 2. configured `feedback` (default) → `agent.feedback`;
/// 3. configured `send-keys` but target `interactive`/`unknown` → fall back
///    to `agent.feedback` (caller emits a stderr note).
#[must_use]
pub fn select_delivery_mode(configured: TellMode, detected: Mode) -> DeliveryDecision {
    match configured {
        TellMode::SendKeys => match detected {
            Mode::AcceptEdits => DeliveryDecision::SendKeys,
            Mode::Interactive | Mode::Unknown => DeliveryDecision::FeedbackFallback,
        },
        TellMode::Feedback => DeliveryDecision::Feedback,
    }
}

/// The stderr-side note emitted when `send-keys` falls back to feedback
/// because the target's mode is not `accept-edits` (design D3.3 / task 6.4).
#[must_use]
pub fn fallback_note(target_id: &str, detected: Mode) -> String {
    format!(
        "note: [supervisor.tell] mode = \"send-keys\" but target `{target_id}` detected mode is \
         `{detected}`; falling back to agent.feedback delivery. Check the agent's mode if you \
         expected direct keystroke injection."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tell_basic() {
        let cmd = parse_tell("/tell feat/auth rebase onto main").unwrap();
        assert_eq!(cmd.target_id, "feat/auth");
        assert_eq!(cmd.prompt, "rebase onto main");
    }

    #[test]
    fn parse_tell_multiline_prompt_preserved() {
        let cmd = parse_tell("/tell feat-api\nrun the migration\nthen restart").unwrap();
        assert_eq!(cmd.target_id, "feat-api");
        assert_eq!(cmd.prompt, "run the migration\nthen restart");
    }

    #[test]
    fn parse_tell_rejects_non_directive() {
        assert!(parse_tell("hello there").is_none());
        assert!(parse_tell("/tellfoo bar").is_none());
    }

    #[test]
    fn parse_tell_rejects_missing_prompt_or_target() {
        assert!(parse_tell("/tell feat-auth").is_none());
        assert!(parse_tell("/tell    ").is_none());
        assert!(parse_tell("/tell").is_none());
    }

    #[test]
    fn default_feedback_mode_uses_feedback() {
        for detected in [Mode::AcceptEdits, Mode::Interactive, Mode::Unknown] {
            assert_eq!(
                select_delivery_mode(TellMode::Feedback, detected),
                DeliveryDecision::Feedback
            );
        }
    }

    #[test]
    fn send_keys_mode_targets_accept_edits() {
        assert_eq!(
            select_delivery_mode(TellMode::SendKeys, Mode::AcceptEdits),
            DeliveryDecision::SendKeys
        );
    }

    #[test]
    fn send_keys_falls_back_for_non_accept_edits() {
        assert_eq!(
            select_delivery_mode(TellMode::SendKeys, Mode::Interactive),
            DeliveryDecision::FeedbackFallback
        );
        assert_eq!(
            select_delivery_mode(TellMode::SendKeys, Mode::Unknown),
            DeliveryDecision::FeedbackFallback
        );
    }

    #[test]
    fn decision_helpers() {
        assert!(DeliveryDecision::Feedback.uses_feedback());
        assert!(DeliveryDecision::FeedbackFallback.uses_feedback());
        assert!(!DeliveryDecision::SendKeys.uses_feedback());
        assert!(DeliveryDecision::FeedbackFallback.is_fallback());
        assert!(!DeliveryDecision::Feedback.is_fallback());
        assert_eq!(DeliveryDecision::SendKeys.learnings_label(), "send-keys");
        assert_eq!(
            DeliveryDecision::FeedbackFallback.learnings_label(),
            "feedback"
        );
    }

    #[test]
    fn fallback_note_names_target_and_mode() {
        let note = fallback_note("feat-auth", Mode::Unknown);
        assert!(note.contains("feat-auth"));
        assert!(note.contains("unknown"));
        assert!(note.contains("agent.feedback"));
    }
}

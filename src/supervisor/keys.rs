//! Keystroke dispatch into a tmux pane.
//!
//! The one seam every git-paw path that types into a pane goes through: the
//! drive loop's nudge, its stale-input and buffered-input recovery, and the
//! orchestrator hand-off.
//!
//! Nothing here answers a permission prompt. git-paw approves no prompt on
//! its own initiative; the only approval path is an explicit
//! `sweep.sh approve`, run by a human or on a human's instruction, which
//! types through tmux itself.

use std::process::Command;

/// Abstraction over `tmux send-keys` so every path that types into a pane
/// can be tested without spawning tmux.
///
/// The two methods are NOT interchangeable, and every implementation that
/// actually shells out to tmux must keep them distinct:
///
/// - [`Self::send_key`] sends a key **name** — `1`, `Enter`, `C-u` — which tmux
///   resolves through its key table. The nudge path depends on this: the
///   submitting `Enter` must be interpreted, not typed as five characters.
/// - [`Self::send_text`] sends **literal characters** (`send-keys -l`), so no
///   token inside free text can be resolved as a key name. Any path that types
///   text git-paw did not choose word-by-word — feedback quoted back to a
///   worker, a routed user prompt — must use this one.
pub trait KeyDispatcher {
    /// Sends a single key (in tmux key-name notation) to the given pane.
    ///
    /// Returns the dispatch result; failures are surfaced to the caller so
    /// it can decide whether to log or abort.
    fn send_key(&mut self, session: &str, pane_index: usize, key: &str) -> std::io::Result<()>;

    /// Sends `text` to the given pane as literal characters, never resolving
    /// any part of it through tmux's key table.
    ///
    /// Use this for free text; use [`Self::send_key`] for the key names that
    /// drive a prompt. A single-token payload is exactly where the two diverge:
    /// sent as a key, a bare `C-c` would fire an interrupt and `Escape` would
    /// cancel the pane's prompt instead of typing those characters.
    fn send_text(&mut self, session: &str, pane_index: usize, text: &str) -> std::io::Result<()>;
}

/// Production [`KeyDispatcher`] that shells out to `tmux send-keys`.
pub struct TmuxKeyDispatcher;

impl KeyDispatcher for TmuxKeyDispatcher {
    fn send_key(&mut self, session: &str, pane_index: usize, key: &str) -> std::io::Result<()> {
        let target = format!("{session}:0.{pane_index}");
        let status = Command::new("tmux")
            .args(["send-keys", "-t", &target, key])
            .status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "tmux send-keys exited with {status}"
            )));
        }
        Ok(())
    }

    fn send_text(&mut self, session: &str, pane_index: usize, text: &str) -> std::io::Result<()> {
        let target = format!("{session}:0.{pane_index}");
        // `-l` immediately before the payload, matching the boot-prompt
        // injection convention (`cmd_start`'s prompt send).
        let status = Command::new("tmux")
            .args(["send-keys", "-t", &target, "-l", text])
            .status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "tmux send-keys -l exited with {status}"
            )));
        }
        Ok(())
    }
}

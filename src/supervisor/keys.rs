//! Keystroke dispatch into an agent's surface.
//!
//! The one seam every git-paw path that types into a pane goes through: the
//! drive loop's nudge, its stale-input and buffered-input recovery, and the
//! orchestrator hand-off. Every [`TerminalBackend`] implements it, so the
//! keystrokes reach the surface through the terminal port.
//!
//! Nothing here answers a permission prompt. git-paw approves no prompt on
//! its own initiative; the only approval path is an explicit
//! `sweep.sh approve`, run by a human or on a human's instruction, which
//! types through tmux itself.

use crate::terminal::{Key, TerminalBackend};

/// Abstraction over keystroke delivery so every path that types into a pane
/// can be tested without a terminal.
///
/// The two methods are NOT interchangeable, and every implementation must
/// keep them distinct:
///
/// - [`Self::send_key`] sends a key **name** — `Enter`, `C-u` — which the
///   backend resolves to a named key. The nudge path depends on this: the
///   submitting `Enter` must be interpreted, not typed as five characters.
/// - [`Self::send_text`] sends **literal characters**, so no token inside free
///   text can be resolved as a key name. Any path that types text git-paw did
///   not choose word-by-word — feedback quoted back to a worker, a routed user
///   prompt — must use this one.
pub trait KeyDispatcher {
    /// Sends a single key (`Enter` or `C-u`) to the given pane.
    ///
    /// Returns the dispatch result; failures are surfaced to the caller so
    /// it can decide whether to log or abort.
    fn send_key(&mut self, session: &str, pane_index: usize, key: &str) -> std::io::Result<()>;

    /// Sends `text` to the given pane as literal characters, never resolving
    /// any part of it as a key name.
    ///
    /// Use this for free text; use [`Self::send_key`] for the key names that
    /// drive a prompt. A single-token payload is exactly where the two diverge:
    /// sent as a key, a bare `C-c` would fire an interrupt and `Escape` would
    /// cancel the pane's prompt instead of typing those characters.
    fn send_text(&mut self, session: &str, pane_index: usize, text: &str) -> std::io::Result<()>;
}

/// Every terminal backend dispatches keystrokes: the key names the loop sends
/// (`Enter`, `C-u`) map to the port's named [`Key`]s, and free text goes out
/// through [`TerminalBackend::send_text`], never through a key table. A key
/// name the port does not know is refused rather than typed as text.
impl<T: TerminalBackend + ?Sized> KeyDispatcher for T {
    fn send_key(&mut self, session: &str, pane_index: usize, key: &str) -> std::io::Result<()> {
        dispatch_key(self, session, pane_index, key)
    }

    fn send_text(&mut self, session: &str, pane_index: usize, text: &str) -> std::io::Result<()> {
        dispatch_text(self, session, pane_index, text)
    }
}

/// Sends key name `key` to the surface at `pane_index` as the port's named
/// [`Key`]; an unknown name is refused, never typed. Takes `&T` because the
/// port's operations all take `&self`, so a test can share one backend as the
/// loop's enumerator, capturer and dispatcher.
pub(crate) fn dispatch_key<T: TerminalBackend + ?Sized>(
    backend: &T,
    session: &str,
    pane_index: usize,
    key: &str,
) -> std::io::Result<()> {
    let named = match key {
        "Enter" => Key::Enter,
        "C-u" => Key::ClearLine,
        other => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("no terminal key named `{other}`"),
            ));
        }
    };
    backend
        .send_key(&backend.surface_at(session, pane_index), named)
        .map_err(|e| std::io::Error::other(e.to_string()))
}

/// Types `text` into the surface at `pane_index` literally.
pub(crate) fn dispatch_text<T: TerminalBackend + ?Sized>(
    backend: &T,
    session: &str,
    pane_index: usize,
    text: &str,
) -> std::io::Result<()> {
    backend
        .send_text(&backend.surface_at(session, pane_index), text)
        .map_err(|e| std::io::Error::other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::fake::{FakeBackend, Operation};

    #[test]
    fn key_names_map_to_the_ports_named_keys_and_text_stays_literal() {
        let mut fake = FakeBackend::new();
        let surface = fake.surface_at("paw-x", 2);
        KeyDispatcher::send_key(&mut fake, "paw-x", 2, "C-u").unwrap();
        KeyDispatcher::send_text(&mut fake, "paw-x", 2, "C-c Escape").unwrap();
        KeyDispatcher::send_key(&mut fake, "paw-x", 2, "Enter").unwrap();
        assert_eq!(
            fake.operations(),
            vec![
                Operation::SendKey {
                    surface: surface.clone(),
                    key: Key::ClearLine
                },
                Operation::SendText {
                    surface: surface.clone(),
                    text: "C-c Escape".into()
                },
                Operation::SendKey {
                    surface,
                    key: Key::Enter
                },
            ]
        );
    }

    #[test]
    fn an_unknown_key_name_is_refused_not_typed() {
        let mut fake = FakeBackend::new();
        let err = KeyDispatcher::send_key(&mut fake, "paw-x", 2, "C-c").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(fake.operations().is_empty());
    }
}

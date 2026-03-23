//! Session state persistence.
//!
//! Saves and loads session data to disk for recovery after crashes, reboots,
//! or `stop`. One session per repository, stored as JSON under the XDG data
//! directory (`~/.local/share/git-paw/sessions/`).

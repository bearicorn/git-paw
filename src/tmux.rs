//! Tmux session and pane orchestration.
//!
//! Checks tmux availability, creates sessions, splits panes, sends commands,
//! applies layouts, and manages attach/reattach. Uses a builder pattern for
//! testability and dry-run support.

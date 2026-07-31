//! Layout configuration for git-paw-managed tmux sessions.

use serde::{Deserialize, Serialize};

/// Layout configuration for git-paw-managed tmux sessions.
///
/// Controls the optional pane "affordances" — heavy borders, per-pane title
/// labels, and active-pane highlighting — applied to `paw-*` sessions.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LayoutConfig {
    /// Whether to apply the border affordances (heavy borders, dim/active
    /// border styling, per-pane label strip, and per-pane titles) to
    /// git-paw-managed sessions.
    ///
    /// `None` (the default, including when the `[layout]` section is absent)
    /// resolves to `true` via [`LayoutConfig::border_affordances_enabled`].
    /// Set to `false` to opt out and inherit the user's default tmux styling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_affordances: Option<bool>,
}

impl LayoutConfig {
    /// Resolve the border-affordances setting, defaulting to `true` when unset.
    #[must_use]
    pub fn border_affordances_enabled(&self) -> bool {
        self.border_affordances.unwrap_or(true)
    }
}

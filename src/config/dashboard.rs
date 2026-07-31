//! Dashboard configuration, including the broker-log panel.

use serde::{Deserialize, Serialize};

/// Dashboard configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DashboardConfig {
    /// Whether to show the legacy broker messages panel in the dashboard.
    ///
    /// Superseded by the type-filterable "Broker log" panel
    /// ([`DashboardConfig::broker_log`]); retained for source compatibility
    /// with v0.5.0 configs.
    #[serde(default)]
    pub show_message_log: bool,
    /// Configuration for the v0.6.0 "Broker log" panel — its ring-buffer cap
    /// and default visibility. An absent `[dashboard.broker_log]` section
    /// loads [`BrokerLogConfig::default`] so v0.5.0 configs parse unchanged.
    #[serde(default)]
    pub broker_log: BrokerLogConfig,
}

/// Configuration for the dashboard's "Broker log" panel.
///
/// All fields carry `#[serde(default)]` so a v0.5.0 `[dashboard]` section
/// with no `broker_log` table — or a `[dashboard.broker_log]` table that
/// sets only some fields — loads with the documented defaults for the rest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrokerLogConfig {
    /// Maximum number of messages retained in the panel's in-memory ring
    /// buffer. Older messages drop off the top as new ones arrive. Default:
    /// `500`.
    #[serde(default = "BrokerLogConfig::default_max_messages")]
    pub max_messages: usize,
    /// Whether the panel is visible when the dashboard first launches. The
    /// `l` hotkey toggles visibility at runtime regardless of this value.
    /// Default: `true`.
    #[serde(default = "BrokerLogConfig::default_visible")]
    pub default_visible: bool,
    /// Number of terminal rows the panel occupies when visible. Raised from
    /// the v0.6.0 fixed `12` so more broker messages are visible without
    /// scrolling; the agent table keeps a positive minimum and yields slack
    /// to the panel only on tall terminals. Default: `20`.
    #[serde(default = "BrokerLogConfig::default_height_lines")]
    pub height_lines: u16,
}

impl Default for BrokerLogConfig {
    fn default() -> Self {
        Self {
            max_messages: Self::default_max_messages(),
            default_visible: Self::default_visible(),
            height_lines: Self::default_height_lines(),
        }
    }
}

impl BrokerLogConfig {
    fn default_max_messages() -> usize {
        500
    }

    fn default_visible() -> bool {
        true
    }

    /// Default panel height in terminal rows. Strictly greater than the
    /// v0.6.0 fixed `12` so the panel shows materially more messages.
    pub(crate) fn default_height_lines() -> u16 {
        20
    }
}

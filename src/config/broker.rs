//! HTTP broker configuration and its filesystem-watcher tuning.

use serde::{Deserialize, Serialize};

/// Configuration for the broker filesystem watcher.
///
/// The watcher publishes `agent.status: working` from git-status changes.
/// Bug 8 (`auto-approve-scope-v0-6-x`) adds a post-commit re-entry: after an
/// `agent.artifact status: "committed"` event, a subsequent file modification
/// observed within [`Self::republish_working_ttl_seconds`] re-publishes
/// `working` so the dashboard reflects the agent's continued activity.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WatcherConfig {
    /// TTL (seconds) after a `committed` event during which a file write
    /// re-publishes `working`.
    ///
    /// `None` resolves to [`Self::DEFAULT_REPUBLISH_TTL_SECONDS`] (60) via
    /// [`Self::republish_working_ttl_seconds`]. A value of `0` disables the
    /// auto-republish entirely (restoring the v0.5.0 "committed is terminal
    /// until explicit republish" model). Non-zero values below
    /// [`Self::MIN_REPUBLISH_TTL_SECONDS`] (5) are clamped to that floor with
    /// a stderr warning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub republish_working_ttl_seconds: Option<u64>,
}

impl WatcherConfig {
    /// Default post-commit re-entry TTL in seconds.
    pub const DEFAULT_REPUBLISH_TTL_SECONDS: u64 = 60;
    /// Minimum non-zero TTL; smaller positive values clamp up to this floor.
    pub const MIN_REPUBLISH_TTL_SECONDS: u64 = 5;

    /// Returns the effective post-commit re-entry TTL in seconds.
    ///
    /// - `None` → [`Self::DEFAULT_REPUBLISH_TTL_SECONDS`].
    /// - `Some(0)` → `0` (auto-republish disabled).
    /// - `Some(n)` with `0 < n < 5` → clamped to
    ///   [`Self::MIN_REPUBLISH_TTL_SECONDS`] with a stderr warning.
    /// - `Some(n)` with `n >= 5` → `n`.
    #[must_use]
    pub fn republish_working_ttl_seconds(&self) -> u64 {
        match self.republish_working_ttl_seconds {
            None => Self::DEFAULT_REPUBLISH_TTL_SECONDS,
            Some(0) => 0,
            Some(n) if n < Self::MIN_REPUBLISH_TTL_SECONDS => {
                eprintln!(
                    "warning: [broker.watcher] republish_working_ttl_seconds = {n} clamped to {}s minimum",
                    Self::MIN_REPUBLISH_TTL_SECONDS
                );
                Self::MIN_REPUBLISH_TTL_SECONDS
            }
            Some(n) => n,
        }
    }
}

/// HTTP broker configuration for agent coordination.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrokerConfig {
    /// Whether the broker is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// TCP port the broker listens on.
    #[serde(default = "BrokerConfig::default_port")]
    pub port: u16,
    /// Bind address for the broker.
    #[serde(default = "BrokerConfig::default_bind")]
    pub bind: String,
    /// Filesystem watcher tuning.
    #[serde(default)]
    pub watcher: WatcherConfig,
}

impl Default for BrokerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 9119,
            bind: "127.0.0.1".to_string(),
            watcher: WatcherConfig::default(),
        }
    }
}

impl BrokerConfig {
    /// Returns the full URL for the broker endpoint.
    pub fn url(&self) -> String {
        format!("http://{}:{}", self.bind, self.port)
    }

    fn default_port() -> u16 {
        9119
    }

    fn default_bind() -> String {
        "127.0.0.1".to_string()
    }
}

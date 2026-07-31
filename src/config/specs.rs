//! Spec scanning configuration.

use serde::{Deserialize, Serialize};

/// Spec scanning configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpecsConfig {
    /// Directory containing spec files (relative to repo root).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    /// Spec format type: `"openspec"` or `"markdown"`.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub spec_type: Option<String>,
}

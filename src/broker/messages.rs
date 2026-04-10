//! Broker message types, validation, and branch slug conversion.
//!
//! Defines [`BrokerMessage`] -- the envelope type for all inter-agent
//! communication -- along with its payload structs and helper methods.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Validation errors for broker messages.
#[derive(Debug, thiserror::Error)]
pub enum MessageError {
    /// The `agent_id` field is empty or whitespace-only.
    #[error("agent_id must not be empty")]
    EmptyAgentId,

    /// The `agent_id` field contains characters outside `[a-z0-9-_]`.
    #[error("agent_id contains invalid characters — only [a-z0-9-_] allowed")]
    InvalidAgentIdChars,

    /// The `status` field is empty or whitespace-only.
    #[error("status field must not be empty")]
    EmptyStatusField,

    /// The `needs` field is empty or whitespace-only.
    #[error("needs field must not be empty")]
    EmptyNeedsField,

    /// The `from` field is empty or whitespace-only.
    #[error("from field must not be empty")]
    EmptyFromField,

    /// JSON deserialization failed.
    #[error("invalid message JSON: {0}")]
    Deserialize(#[from] serde_json::Error),
}

/// Payload for `agent.status` messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusPayload {
    /// Current status label (e.g. `"working"`, `"idle"`).
    pub status: String,
    /// List of files modified by the agent.
    pub modified_files: Vec<String>,
    /// Optional human-readable message.
    pub message: Option<String>,
}

/// Payload for `agent.artifact` messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactPayload {
    /// Current status label (e.g. `"done"`).
    pub status: String,
    /// List of exported symbols or public API items.
    pub exports: Vec<String>,
    /// List of files modified by the agent.
    pub modified_files: Vec<String>,
}

/// Payload for `agent.blocked` messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedPayload {
    /// What the agent needs to proceed.
    pub needs: String,
    /// Agent ID of the agent that can unblock the sender.
    pub from: String,
}

/// Envelope for all inter-agent messages.
///
/// The wire format uses JSON with an internally tagged `"type"` discriminator
/// whose values are `"agent.status"`, `"agent.artifact"`, and `"agent.blocked"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BrokerMessage {
    /// Status heartbeat -- not routed to inboxes.
    #[serde(rename = "agent.status")]
    Status {
        /// Sender agent ID (slugified branch name).
        agent_id: String,
        /// Status payload.
        payload: StatusPayload,
    },
    /// Artifact announcement -- broadcast to all peers.
    #[serde(rename = "agent.artifact")]
    Artifact {
        /// Sender agent ID.
        agent_id: String,
        /// Artifact payload.
        payload: ArtifactPayload,
    },
    /// Blocked notification -- sent to the target agent.
    #[serde(rename = "agent.blocked")]
    Blocked {
        /// Sender agent ID.
        agent_id: String,
        /// Blocked payload (contains `from` -- the unblocking agent).
        payload: BlockedPayload,
    },
}

impl BrokerMessage {
    /// Deserializes and validates a broker message from a JSON string.
    ///
    /// Returns [`MessageError`] if the JSON is malformed or the `agent_id` is
    /// invalid.
    pub fn from_json(input: &str) -> Result<Self, MessageError> {
        let msg: Self = serde_json::from_str(input)?;
        msg.validate()?;
        Ok(msg)
    }

    /// Returns the `agent_id` field from whichever variant.
    pub fn agent_id(&self) -> &str {
        match self {
            Self::Status { agent_id, .. }
            | Self::Artifact { agent_id, .. }
            | Self::Blocked { agent_id, .. } => agent_id,
        }
    }

    /// Returns a short status label for the message.
    ///
    /// - `Status` returns `payload.status` (e.g. `"working"`)
    /// - `Artifact` returns `payload.status` (e.g. `"done"`)
    /// - `Blocked` returns `"blocked"`
    pub fn status_label(&self) -> &str {
        match self {
            Self::Status { payload, .. } => &payload.status,
            Self::Artifact { payload, .. } => &payload.status,
            Self::Blocked { .. } => "blocked",
        }
    }

    /// Validates all fields according to the broker message spec.
    fn validate(&self) -> Result<(), MessageError> {
        let id = self.agent_id();
        if id.trim().is_empty() {
            return Err(MessageError::EmptyAgentId);
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(MessageError::InvalidAgentIdChars);
        }
        match self {
            Self::Status { payload, .. } => {
                if payload.status.trim().is_empty() {
                    return Err(MessageError::EmptyStatusField);
                }
            }
            Self::Artifact { payload, .. } => {
                if payload.status.trim().is_empty() {
                    return Err(MessageError::EmptyStatusField);
                }
            }
            Self::Blocked { payload, .. } => {
                if payload.needs.trim().is_empty() {
                    return Err(MessageError::EmptyNeedsField);
                }
                if payload.from.trim().is_empty() {
                    return Err(MessageError::EmptyFromField);
                }
            }
        }
        Ok(())
    }
}

impl fmt::Display for BrokerMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Status { agent_id, payload } => {
                write!(
                    f,
                    "[{agent_id}] status: {} ({} files modified)",
                    payload.status,
                    payload.modified_files.len()
                )
            }
            Self::Artifact {
                agent_id, payload, ..
            } => {
                if payload.exports.is_empty() {
                    write!(f, "[{agent_id}] artifact: {}", payload.status)
                } else {
                    write!(
                        f,
                        "[{agent_id}] artifact: {} \u{2014} exports: {}",
                        payload.status,
                        payload.exports.join(", ")
                    )
                }
            }
            Self::Blocked {
                agent_id, payload, ..
            } => {
                write!(
                    f,
                    "[{agent_id}] blocked: needs {} from {}",
                    payload.needs, payload.from
                )
            }
        }
    }
}

/// Converts a git branch name into a stable broker `agent_id` slug.
///
/// Applies a 5-step normalization algorithm:
///
/// 1. Convert to ASCII lowercase
/// 2. Replace any character not in `[a-z0-9_]` with `-`
/// 3. Collapse consecutive `-` into a single `-`
/// 4. Trim leading and trailing `-`
/// 5. If the result is empty, return `"agent"`
///
/// # Examples
///
/// - `"feat/http-broker"` → `"feat-http-broker"`
/// - `"a/b/c"` → `"a-b-c"`
/// - `"FEAT/X"` → `"feat-x"`
/// - `""` → `"agent"`
/// - `"---"` → `"agent"`
pub fn slugify_branch(name: &str) -> String {
    // Step 1: to ASCII lowercase
    let lowered = name.to_ascii_lowercase();

    // Step 2: replace non-[a-z0-9_] with -
    let replaced: String = lowered
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();

    // Step 3: collapse consecutive - to single -
    let mut collapsed = String::with_capacity(replaced.len());
    let mut prev_dash = false;
    for c in replaced.chars() {
        if c == '-' {
            if !prev_dash {
                collapsed.push('-');
            }
            prev_dash = true;
        } else {
            collapsed.push(c);
            prev_dash = false;
        }
    }

    // Step 4: trim leading/trailing -
    let trimmed = collapsed.trim_matches('-');

    // Step 5: if empty, return "agent"
    if trimmed.is_empty() {
        "agent".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_status(agent_id: &str, status: &str) -> BrokerMessage {
        BrokerMessage::Status {
            agent_id: agent_id.to_string(),
            payload: StatusPayload {
                status: status.to_string(),
                modified_files: vec![],
                message: None,
            },
        }
    }

    fn make_artifact(agent_id: &str, status: &str, exports: &[&str]) -> BrokerMessage {
        BrokerMessage::Artifact {
            agent_id: agent_id.to_string(),
            payload: ArtifactPayload {
                status: status.to_string(),
                exports: exports.iter().map(|s| (*s).to_string()).collect(),
                modified_files: vec!["src/main.rs".to_string()],
            },
        }
    }

    fn make_blocked(agent_id: &str, needs: &str, from: &str) -> BrokerMessage {
        BrokerMessage::Blocked {
            agent_id: agent_id.to_string(),
            payload: BlockedPayload {
                needs: needs.to_string(),
                from: from.to_string(),
            },
        }
    }

    #[test]
    fn slugify_branch_replaces_slashes() {
        assert_eq!(slugify_branch("feat/errors"), "feat-errors");
        assert_eq!(slugify_branch("main"), "main");
        assert_eq!(slugify_branch("a/b/c"), "a-b-c");
    }

    #[test]
    fn slugify_branch_lowercases() {
        assert_eq!(slugify_branch("FEAT/X"), "feat-x");
    }

    #[test]
    fn slugify_branch_empty_returns_agent() {
        assert_eq!(slugify_branch(""), "agent");
    }

    #[test]
    fn slugify_branch_only_dashes_returns_agent() {
        assert_eq!(slugify_branch("---"), "agent");
    }

    #[test]
    fn slugify_branch_collapses_consecutive_dashes() {
        assert_eq!(slugify_branch("feat//x"), "feat-x");
    }

    #[test]
    fn slugify_branch_trims_leading_trailing_dashes() {
        assert_eq!(slugify_branch("/feat/x/"), "feat-x");
    }

    #[test]
    fn agent_id_status() {
        let msg = make_status("feat-x", "working");
        assert_eq!(msg.agent_id(), "feat-x");
    }

    #[test]
    fn agent_id_artifact() {
        let msg = make_artifact("feat-y", "done", &["auth"]);
        assert_eq!(msg.agent_id(), "feat-y");
    }

    #[test]
    fn agent_id_blocked() {
        let msg = make_blocked("feat-config", "error types", "feat-errors");
        assert_eq!(msg.agent_id(), "feat-config");
    }

    #[test]
    fn status_label_status_variant() {
        let msg = make_status("feat-x", "working");
        assert_eq!(msg.status_label(), "working");
    }

    #[test]
    fn status_label_artifact_variant() {
        let msg = make_artifact("feat-x", "done", &[]);
        assert_eq!(msg.status_label(), "done");
    }

    #[test]
    fn status_label_blocked_variant() {
        let msg = make_blocked("feat-config", "error types", "feat-errors");
        assert_eq!(msg.status_label(), "blocked");
    }

    #[test]
    fn display_status() {
        let msg = make_status("feat-x", "working");
        assert_eq!(
            msg.to_string(),
            "[feat-x] status: working (0 files modified)"
        );
    }

    #[test]
    fn display_status_with_files() {
        let msg = BrokerMessage::Status {
            agent_id: "feat-x".to_string(),
            payload: StatusPayload {
                status: "working".to_string(),
                modified_files: vec!["a.rs".to_string(), "b.rs".to_string()],
                message: None,
            },
        };
        assert_eq!(
            msg.to_string(),
            "[feat-x] status: working (2 files modified)"
        );
    }

    #[test]
    fn display_artifact_no_exports() {
        let msg = make_artifact("feat-x", "done", &[]);
        assert_eq!(msg.to_string(), "[feat-x] artifact: done");
    }

    #[test]
    fn display_artifact_with_exports() {
        let msg = make_artifact("feat-x", "done", &["PawError", "Config"]);
        assert_eq!(
            msg.to_string(),
            "[feat-x] artifact: done \u{2014} exports: PawError, Config"
        );
    }

    #[test]
    fn display_blocked() {
        let msg = make_blocked("feat-config", "error types", "feat-errors");
        assert_eq!(
            msg.to_string(),
            "[feat-config] blocked: needs error types from feat-errors"
        );
    }

    #[test]
    fn from_json_valid_status() {
        let json = r#"{"type":"agent.status","agent_id":"feat-x","payload":{"status":"working","modified_files":[],"message":null}}"#;
        let msg = BrokerMessage::from_json(json).unwrap();
        assert_eq!(msg.agent_id(), "feat-x");
        assert_eq!(msg.status_label(), "working");
    }

    #[test]
    fn from_json_empty_agent_id_rejected() {
        let json = r#"{"type":"agent.status","agent_id":"","payload":{"status":"working","modified_files":[]}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyAgentId));
    }

    #[test]
    fn from_json_invalid_agent_id_chars_rejected() {
        let json = r#"{"type":"agent.status","agent_id":"feat/x","payload":{"status":"working","modified_files":[]}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::InvalidAgentIdChars));
    }

    #[test]
    fn from_json_empty_status_rejected() {
        let json = r#"{"type":"agent.status","agent_id":"feat-x","payload":{"status":"","modified_files":[]}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyStatusField));
    }

    #[test]
    fn from_json_empty_artifact_status_rejected() {
        let json = r#"{"type":"agent.artifact","agent_id":"feat-x","payload":{"status":"","exports":[],"modified_files":[]}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyStatusField));
    }

    #[test]
    fn from_json_empty_needs_rejected() {
        let json = r#"{"type":"agent.blocked","agent_id":"feat-x","payload":{"needs":"","from":"feat-y"}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyNeedsField));
    }

    #[test]
    fn from_json_empty_from_rejected() {
        let json =
            r#"{"type":"agent.blocked","agent_id":"feat-x","payload":{"needs":"types","from":""}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyFromField));
    }

    #[test]
    fn from_json_invalid_json_rejected() {
        let err = BrokerMessage::from_json("not json").unwrap_err();
        assert!(matches!(err, MessageError::Deserialize(_)));
    }

    #[test]
    fn serde_roundtrip_status() {
        let msg = make_status("feat-x", "working");
        let json = serde_json::to_string(&msg).unwrap();
        let back: BrokerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.agent_id(), "feat-x");
        assert_eq!(back.status_label(), "working");
    }

    #[test]
    fn serde_roundtrip_artifact() {
        let msg = make_artifact("feat-x", "done", &["PawError"]);
        let json = serde_json::to_string(&msg).unwrap();
        let back: BrokerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.agent_id(), "feat-x");
        assert_eq!(back.status_label(), "done");
    }

    #[test]
    fn serde_roundtrip_blocked() {
        let msg = make_blocked("a", "types", "b");
        let json = serde_json::to_string(&msg).unwrap();
        let back: BrokerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.agent_id(), "a");
        assert_eq!(back.status_label(), "blocked");
    }

    #[test]
    fn from_json_whitespace_agent_id_rejected() {
        let json = r#"{"type":"agent.status","agent_id":"   ","payload":{"status":"working","modified_files":[],"message":null}}"#;
        assert!(BrokerMessage::from_json(json).is_err());
    }

    #[test]
    fn slugify_branch_preserves_underscores() {
        assert_eq!(slugify_branch("feat/my_feature"), "feat-my_feature");
    }

    #[test]
    fn slugify_branch_replaces_non_ascii() {
        let result = slugify_branch("feat/日本語");
        assert!(result.is_ascii());
        assert_eq!(result, "feat");
    }

    #[test]
    fn slugify_branch_deterministic() {
        let a = slugify_branch("feat/http-broker");
        let b = slugify_branch("feat/http-broker");
        assert_eq!(a, b);
    }
}

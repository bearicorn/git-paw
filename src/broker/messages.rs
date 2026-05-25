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

    /// The `status` field is empty or whitespace-only.
    #[error("status field must not be empty")]
    EmptyStatusField,

    /// The `needs` field is empty or whitespace-only.
    #[error("needs field must not be empty")]
    EmptyNeedsField,

    /// The `from` field is empty or whitespace-only.
    #[error("from field must not be empty")]
    EmptyFromField,

    /// The `verified_by` field is empty or whitespace-only.
    #[error("verified_by field must not be empty")]
    EmptyVerifiedBy,

    /// The `errors` list is empty.
    #[error("errors list must not be empty")]
    EmptyErrors,

    /// The `question` field is empty or whitespace-only.
    #[error("question field must not be empty")]
    EmptyQuestionField,

    /// The intent `files` array is empty.
    #[error("intent files list must not be empty")]
    EmptyIntentFiles,

    /// An entry in the intent `files` array is empty or whitespace-only.
    #[error("intent files entry must not be empty or whitespace-only")]
    EmptyIntentFileEntry,

    /// The intent `summary` field is empty or whitespace-only.
    #[error("intent summary field must not be empty")]
    EmptyIntentSummary,

    /// The intent `valid_for_seconds` field is zero.
    #[error("intent valid_for_seconds must be > 0")]
    ZeroValidForSeconds,

    /// JSON deserialization failed.
    #[error("invalid message JSON: {0}")]
    Deserialize(#[from] serde_json::Error),
}

/// Payload for `agent.status` messages.
///
/// `cli` and `phase` are optional and serialise with `skip_serializing_if =
/// "Option::is_none"`, so legacy payloads without these fields deserialise as
/// `None` and new payloads with `None` omit the field from the wire bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusPayload {
    /// Current status label (e.g. `"working"`, `"idle"`).
    pub status: String,
    /// List of files modified by the agent.
    pub modified_files: Vec<String>,
    /// Optional human-readable message.
    pub message: Option<String>,
    /// Optional CLI name (e.g. `"claude"`) identifying the CLI running in the
    /// publishing agent's pane. The supervisor pane resolves this from
    /// `[supervisor].cli` configuration; coding-agent panes typically omit
    /// the field and rely on the broker's watch-target map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli: Option<String>,
    /// Optional free-form phase label (e.g. `"watching"`, `"merging"`) for
    /// the publishing agent's current lifecycle phase. The dashboard prefers
    /// this label over the message-type-derived `status_label()` when
    /// rendering the agent's row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
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

/// Payload for `agent.verified` messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedPayload {
    /// Agent ID of the verifier (typically `"supervisor"`).
    pub verified_by: String,
    /// Optional human-readable summary of the verification result.
    pub message: Option<String>,
}

/// Payload for `agent.question` messages.
///
/// Wire format: `{"type": "agent.question", "agent_id": "<slug>", "payload": {"question": "<text>"}}`.
/// The `question` field MUST NOT be empty. Question messages are routed to the
/// `"supervisor"` inbox by the broker delivery layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionPayload {
    /// The question text the agent is asking.
    pub question: String,
}

/// Payload for `agent.intent` messages.
///
/// Wire format: `{"type": "agent.intent", "agent_id": "<slug>", "payload": {...}}`.
/// `files` declares paths the agent plans to modify (relative to the repository
/// root; globs are permitted but discouraged). `summary` is a one-line human
/// description. `valid_for_seconds` is a relative TTL after which a consumer
/// MAY treat the intent as stale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentPayload {
    /// File paths the agent intends to modify.
    pub files: Vec<String>,
    /// One-line human description of the planned change.
    pub summary: String,
    /// Relative TTL in seconds (strictly positive).
    pub valid_for_seconds: u64,
}

/// Payload for `agent.feedback` messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackPayload {
    /// Agent ID of the sender (typically `"supervisor"`).
    pub from: String,
    /// List of error messages the target agent should address.
    pub errors: Vec<String>,
}

/// Envelope for all inter-agent messages.
///
/// The wire format uses JSON with an internally tagged `"type"` discriminator
/// whose values are `"agent.status"`, `"agent.artifact"`, `"agent.blocked"`,
/// `"agent.verified"`, `"agent.feedback"`, `"agent.question"`, and
/// `"agent.intent"`.
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
    /// Verification acknowledgement -- broadcast to all peers.
    #[serde(rename = "agent.verified")]
    Verified {
        /// Target agent ID (the agent whose work was verified).
        agent_id: String,
        /// Verified payload (contains `verified_by` -- the sender).
        payload: VerifiedPayload,
    },
    /// Feedback from a verifier -- delivered to the target agent only.
    #[serde(rename = "agent.feedback")]
    Feedback {
        /// Target agent ID (the agent receiving feedback).
        agent_id: String,
        /// Feedback payload (contains `from` -- the sender).
        payload: FeedbackPayload,
    },
    /// Agent question -- delivered to the `"supervisor"` inbox for human reply.
    #[serde(rename = "agent.question")]
    Question {
        /// Sender agent ID (the agent asking the question).
        agent_id: String,
        /// Question payload.
        payload: QuestionPayload,
    },
    /// Intent announcement -- broadcast to every other registered agent's inbox.
    ///
    /// Lets peers (and the broker conflict detector) see which files an
    /// agent is about to modify before any commit lands.
    #[serde(rename = "agent.intent")]
    Intent {
        /// Sender agent ID (the agent declaring the intent).
        agent_id: String,
        /// Intent payload.
        payload: IntentPayload,
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
            | Self::Blocked { agent_id, .. }
            | Self::Verified { agent_id, .. }
            | Self::Feedback { agent_id, .. }
            | Self::Question { agent_id, .. }
            | Self::Intent { agent_id, .. } => agent_id,
        }
    }

    /// Returns a short status label for the message.
    ///
    /// - `Status` returns `payload.status` (e.g. `"working"`)
    /// - `Artifact` returns `payload.status` (e.g. `"done"`)
    /// - `Blocked` returns `"blocked"`
    /// - `Verified` returns `"verified"`
    /// - `Feedback` returns `"feedback"`
    /// - `Question` returns `"question"`
    /// - `Intent` returns `"intent"`
    pub fn status_label(&self) -> &str {
        match self {
            Self::Status { payload, .. } => &payload.status,
            Self::Artifact { payload, .. } => &payload.status,
            Self::Blocked { .. } => "blocked",
            Self::Verified { .. } => "verified",
            Self::Feedback { .. } => "feedback",
            Self::Question { .. } => "question",
            Self::Intent { .. } => "intent",
        }
    }

    /// Validates all fields according to the broker message spec.
    ///
    /// The `agent_id` *shape* is enforced at the HTTP boundary by
    /// `src/broker/server.rs::publish` against the canonical regex
    /// `^(supervisor|feat/[a-z0-9][a-z0-9-]+|feat-[a-z0-9][a-z0-9-]+)$`
    /// — this validator only catches the empty-or-whitespace case so
    /// non-HTTP callers still trip a clear error on garbage input
    /// before the typed value flows further.
    fn validate(&self) -> Result<(), MessageError> {
        let id = self.agent_id();
        if id.trim().is_empty() {
            return Err(MessageError::EmptyAgentId);
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
            Self::Verified { payload, .. } => {
                if payload.verified_by.trim().is_empty() {
                    return Err(MessageError::EmptyVerifiedBy);
                }
            }
            Self::Feedback { payload, .. } => {
                if payload.from.trim().is_empty() {
                    return Err(MessageError::EmptyFromField);
                }
                if payload.errors.is_empty() {
                    return Err(MessageError::EmptyErrors);
                }
            }
            Self::Question { payload, .. } => {
                if payload.question.trim().is_empty() {
                    return Err(MessageError::EmptyQuestionField);
                }
            }
            Self::Intent { payload, .. } => {
                if payload.files.is_empty() {
                    return Err(MessageError::EmptyIntentFiles);
                }
                if payload.files.iter().any(|f| f.trim().is_empty()) {
                    return Err(MessageError::EmptyIntentFileEntry);
                }
                if payload.summary.trim().is_empty() {
                    return Err(MessageError::EmptyIntentSummary);
                }
                if payload.valid_for_seconds == 0 {
                    return Err(MessageError::ZeroValidForSeconds);
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
            Self::Verified {
                agent_id, payload, ..
            } => {
                if let Some(message) = &payload.message {
                    write!(
                        f,
                        "[{agent_id}] verified by {} \u{2014} {message}",
                        payload.verified_by
                    )
                } else {
                    write!(f, "[{agent_id}] verified by {}", payload.verified_by)
                }
            }
            Self::Feedback {
                agent_id, payload, ..
            } => {
                write!(
                    f,
                    "[{agent_id}] feedback from {}: {} errors",
                    payload.from,
                    payload.errors.len()
                )
            }
            Self::Question {
                agent_id, payload, ..
            } => {
                write!(f, "[{agent_id}] question: {}", payload.question)
            }
            Self::Intent {
                agent_id, payload, ..
            } => {
                write!(
                    f,
                    "[{agent_id}] intent: {} files for {}s \u{2014} {}",
                    payload.files.len(),
                    payload.valid_for_seconds,
                    payload.summary,
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
                ..Default::default()
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
                ..Default::default()
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
    fn from_json_accepts_slash_in_agent_id() {
        // `feat/<name>` is valid per the agent_id regex enforced at the HTTP
        // boundary; the deserialisation-layer validator no longer rejects it
        // on character grounds. The shape check happens in
        // `src/broker/server.rs::publish` against the canonical regex.
        let json = r#"{"type":"agent.status","agent_id":"feat/x","payload":{"status":"working","modified_files":[]}}"#;
        BrokerMessage::from_json(json).expect("feat/x deserialises cleanly");
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

    // --- StatusPayload cli/phase fields (tasks 1.3-1.6) ---

    #[test]
    fn status_payload_roundtrip_with_cli_and_phase() {
        let payload = StatusPayload {
            status: "working".to_string(),
            modified_files: vec!["src/a.rs".to_string()],
            message: Some("refactoring".to_string()),
            cli: Some("claude".to_string()),
            phase: Some("watching".to_string()),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"cli\":\"claude\""));
        assert!(json.contains("\"phase\":\"watching\""));
        let back: StatusPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back, payload);
    }

    #[test]
    fn status_payload_deserialises_legacy_json_without_cli_or_phase() {
        let json = r#"{"status":"working","modified_files":[],"message":"Supervisor booting"}"#;
        let payload: StatusPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.cli, None);
        assert_eq!(payload.phase, None);
        assert_eq!(payload.status, "working");
        assert_eq!(payload.message.as_deref(), Some("Supervisor booting"));
    }

    #[test]
    fn status_payload_serialises_none_cli_and_phase_with_no_keys() {
        let payload = StatusPayload {
            status: "idle".to_string(),
            modified_files: vec![],
            message: None,
            cli: None,
            phase: None,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(
            !json.contains("\"cli\""),
            "cli key must be omitted when None; got {json}"
        );
        assert!(
            !json.contains("\"phase\""),
            "phase key must be omitted when None; got {json}"
        );
    }

    #[test]
    fn status_payload_deserialises_with_only_cli_populated() {
        let json = r#"{"status":"working","modified_files":[],"message":null,"cli":"claude"}"#;
        let payload: StatusPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.cli.as_deref(), Some("claude"));
        assert_eq!(payload.phase, None);
    }

    #[test]
    fn status_payload_deserialises_with_only_phase_populated() {
        let json = r#"{"status":"feedback","modified_files":[],"message":null,"phase":"merging"}"#;
        let payload: StatusPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.phase.as_deref(), Some("merging"));
        assert_eq!(payload.cli, None);
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

    fn make_verified(agent_id: &str, verified_by: &str, message: Option<&str>) -> BrokerMessage {
        BrokerMessage::Verified {
            agent_id: agent_id.to_string(),
            payload: VerifiedPayload {
                verified_by: verified_by.to_string(),
                message: message.map(str::to_string),
            },
        }
    }

    fn make_feedback(agent_id: &str, from: &str, errors: &[&str]) -> BrokerMessage {
        BrokerMessage::Feedback {
            agent_id: agent_id.to_string(),
            payload: FeedbackPayload {
                from: from.to_string(),
                errors: errors.iter().map(|s| (*s).to_string()).collect(),
            },
        }
    }

    #[test]
    fn serde_roundtrip_verified_with_message() {
        let msg = make_verified("feat-errors", "supervisor", Some("all 12 tests pass"));
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"agent.verified\""));
        assert!(json.contains("all 12 tests pass"));
        let back: BrokerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn serde_roundtrip_verified_without_message() {
        let msg = make_verified("feat-errors", "supervisor", None);
        let json = serde_json::to_string(&msg).unwrap();
        let back: BrokerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn serde_roundtrip_feedback() {
        let msg = make_feedback(
            "feat-errors",
            "supervisor",
            &["test failed", "missing doc comment"],
        );
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"agent.feedback\""));
        let back: BrokerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn from_json_empty_verified_by_rejected() {
        let json = r#"{"type":"agent.verified","agent_id":"feat-errors","payload":{"verified_by":"","message":null}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyVerifiedBy));
    }

    #[test]
    fn from_json_empty_feedback_from_rejected() {
        let json = r#"{"type":"agent.feedback","agent_id":"feat-errors","payload":{"from":"","errors":["e1"]}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyFromField));
    }

    #[test]
    fn from_json_empty_feedback_errors_rejected() {
        let json = r#"{"type":"agent.feedback","agent_id":"feat-errors","payload":{"from":"supervisor","errors":[]}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyErrors));
    }

    #[test]
    fn display_verified_without_message() {
        let msg = make_verified("feat-errors", "supervisor", None);
        assert_eq!(msg.to_string(), "[feat-errors] verified by supervisor");
    }

    #[test]
    fn display_verified_with_message() {
        let msg = make_verified("feat-errors", "supervisor", Some("all tests pass"));
        assert_eq!(
            msg.to_string(),
            "[feat-errors] verified by supervisor \u{2014} all tests pass"
        );
    }

    #[test]
    fn display_feedback_with_three_errors() {
        let msg = make_feedback("feat-errors", "supervisor", &["e1", "e2", "e3"]);
        assert_eq!(
            msg.to_string(),
            "[feat-errors] feedback from supervisor: 3 errors"
        );
    }

    #[test]
    fn status_label_verified() {
        let msg = make_verified("feat-x", "supervisor", None);
        assert_eq!(msg.status_label(), "verified");
    }

    #[test]
    fn status_label_feedback() {
        let msg = make_feedback("feat-x", "supervisor", &["e"]);
        assert_eq!(msg.status_label(), "feedback");
    }

    #[test]
    fn agent_id_verified() {
        let msg = make_verified("feat-x", "supervisor", None);
        assert_eq!(msg.agent_id(), "feat-x");
    }

    #[test]
    fn agent_id_feedback() {
        let msg = make_feedback("feat-x", "supervisor", &["e"]);
        assert_eq!(msg.agent_id(), "feat-x");
    }

    fn make_question(agent_id: &str, question: &str) -> BrokerMessage {
        BrokerMessage::Question {
            agent_id: agent_id.to_string(),
            payload: QuestionPayload {
                question: question.to_string(),
            },
        }
    }

    #[test]
    fn question_empty_field_rejected() {
        let json =
            r#"{"type":"agent.question","agent_id":"feat-config","payload":{"question":""}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyQuestionField));
    }

    #[test]
    fn serde_roundtrip_question() {
        let msg = make_question("feat-config", "Should I skip tests?");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"agent.question\""));
        assert!(json.contains("\"agent_id\":\"feat-config\""));
        let back: BrokerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn display_question() {
        let msg = make_question("feat-config", "Should I add a config field?");
        let s = msg.to_string();
        assert_eq!(s, "[feat-config] question: Should I add a config field?");
        assert!(!s.contains('\n'));
    }

    #[test]
    fn status_label_question() {
        let msg = make_question("feat-config", "anything?");
        assert_eq!(msg.status_label(), "question");
    }

    #[test]
    fn agent_id_question() {
        let msg = make_question("feat-config", "anything?");
        assert_eq!(msg.agent_id(), "feat-config");
    }

    #[test]
    fn question_whitespace_field_rejected() {
        let json =
            r#"{"type":"agent.question","agent_id":"feat-x","payload":{"question":"   \n\t  "}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyQuestionField));
    }

    #[test]
    fn question_empty_agent_id_rejected() {
        let json = r#"{"type":"agent.question","agent_id":"","payload":{"question":"why?"}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyAgentId));
    }

    #[test]
    fn from_json_valid_question() {
        let json = r#"{"type":"agent.question","agent_id":"feat-x","payload":{"question":"Should I merge feat-a before feat-b?"}}"#;
        let msg = BrokerMessage::from_json(json).unwrap();
        assert_eq!(msg.agent_id(), "feat-x");
        assert_eq!(msg.status_label(), "question");
        match &msg {
            BrokerMessage::Question { payload, .. } => {
                assert_eq!(payload.question, "Should I merge feat-a before feat-b?");
            }
            other => panic!("expected Question variant, got {other:?}"),
        }
    }

    #[test]
    fn serde_roundtrip_question_feat_x() {
        let msg = make_question("feat-x", "Should I rebase?");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"agent.question\""));
        assert!(json.contains("\"agent_id\":\"feat-x\""));
        assert!(json.contains("\"payload\""));
        assert!(json.contains("\"question\":\"Should I rebase?\""));
        let back: BrokerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn display_question_matches_spec_format() {
        let msg = make_question("supervisor", "Should I merge feat-a before feat-b?");
        let s = msg.to_string();
        assert_eq!(
            s,
            "[supervisor] question: Should I merge feat-a before feat-b?"
        );
        assert!(!s.contains('\n'), "display output must be a single line");
        // ANSI escape sequences start with the ESC character (0x1B).
        assert!(
            !s.contains('\u{1b}'),
            "display output must not contain ANSI escape sequences"
        );
    }

    #[test]
    fn from_json_unknown_type_rejected() {
        let json = r#"{"type":"agent.unknown","agent_id":"x","payload":{}}"#;
        assert!(BrokerMessage::from_json(json).is_err());
    }

    #[test]
    fn slugify_branch_deterministic() {
        let a = slugify_branch("feat/http-broker");
        let b = slugify_branch("feat/http-broker");
        assert_eq!(a, b);
    }

    // --- Intent variant ---

    fn make_intent(agent_id: &str, files: &[&str], summary: &str, ttl: u64) -> BrokerMessage {
        BrokerMessage::Intent {
            agent_id: agent_id.to_string(),
            payload: IntentPayload {
                files: files.iter().map(|s| (*s).to_string()).collect(),
                summary: summary.to_string(),
                valid_for_seconds: ttl,
            },
        }
    }

    #[test]
    fn intent_message_round_trips_through_serde() {
        let msg = make_intent("feat-auth", &["src/auth.rs"], "wire AuthClient", 900);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"agent.intent\""));
        assert!(json.contains("\"agent_id\":\"feat-auth\""));
        assert!(json.contains("\"payload\""));
        let back: BrokerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn intent_payload_with_multiple_files_round_trips() {
        let msg = make_intent(
            "feat-auth",
            &["src/auth.rs", "src/auth/client.rs"],
            "wire AuthClient",
            900,
        );
        let json = serde_json::to_string(&msg).unwrap();
        let back: BrokerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, msg);
        // files preserved in order
        if let BrokerMessage::Intent { payload, .. } = back {
            assert_eq!(payload.files, vec!["src/auth.rs", "src/auth/client.rs"]);
        } else {
            panic!("expected Intent");
        }
    }

    #[test]
    fn intent_payload_with_single_file_round_trips() {
        let msg = make_intent("feat-x", &["README.md"], "doc fix", 300);
        let json = serde_json::to_string(&msg).unwrap();
        let back: BrokerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn intent_empty_files_array_rejected() {
        let json = r#"{"type":"agent.intent","agent_id":"feat-x","payload":{"files":[],"summary":"x","valid_for_seconds":60}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyIntentFiles));
    }

    #[test]
    fn intent_whitespace_file_path_rejected() {
        let json = r#"{"type":"agent.intent","agent_id":"feat-x","payload":{"files":["   "],"summary":"x","valid_for_seconds":60}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyIntentFileEntry));
    }

    #[test]
    fn intent_empty_summary_rejected() {
        let json = r#"{"type":"agent.intent","agent_id":"feat-x","payload":{"files":["a"],"summary":"","valid_for_seconds":60}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyIntentSummary));
    }

    #[test]
    fn intent_zero_valid_for_seconds_rejected() {
        let json = r#"{"type":"agent.intent","agent_id":"feat-x","payload":{"files":["a"],"summary":"s","valid_for_seconds":0}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::ZeroValidForSeconds));
    }

    #[test]
    fn intent_valid_message_produces_broker_message() {
        let json = r#"{"type":"agent.intent","agent_id":"feat-auth","payload":{"files":["src/auth.rs"],"summary":"wire AuthClient","valid_for_seconds":900}}"#;
        let msg = BrokerMessage::from_json(json).unwrap();
        if let BrokerMessage::Intent { agent_id, payload } = msg {
            assert_eq!(agent_id, "feat-auth");
            assert_eq!(payload.files, vec!["src/auth.rs"]);
            assert_eq!(payload.summary, "wire AuthClient");
            assert_eq!(payload.valid_for_seconds, 900);
        } else {
            panic!("expected Intent variant");
        }
    }

    #[test]
    fn intent_display_output() {
        let msg = make_intent(
            "feat-auth",
            &["src/a.rs", "src/b.rs", "src/c.rs"],
            "wire AuthClient",
            900,
        );
        let s = msg.to_string();
        assert_eq!(
            s,
            "[feat-auth] intent: 3 files for 900s \u{2014} wire AuthClient"
        );
        assert!(!s.contains('\n'));
        assert!(!s.contains('\x1b'));
    }

    #[test]
    fn intent_display_with_one_file() {
        let msg = make_intent("feat-x", &["README.md"], "doc fix", 300);
        assert_eq!(
            msg.to_string(),
            "[feat-x] intent: 1 files for 300s \u{2014} doc fix"
        );
    }

    #[test]
    fn status_label_intent() {
        let msg = make_intent("feat-x", &["a"], "s", 60);
        assert_eq!(msg.status_label(), "intent");
    }

    #[test]
    fn agent_id_intent() {
        let msg = make_intent("feat-auth", &["a"], "s", 60);
        assert_eq!(msg.agent_id(), "feat-auth");
    }

    // Maps to scenario `Intent Display empty path edge` from
    // forward-coordination. Bypasses `from_json` (which would reject
    // `summary == ""` via MessageError::EmptyIntentSummary) and constructs
    // the variant directly so Display can be exercised on the empty case.
    // (test-coverage-v0-5-0 task 4.2)
    #[test]
    fn intent_display_with_empty_summary_renders_dash() {
        let msg = BrokerMessage::Intent {
            agent_id: "feat-x".to_string(),
            payload: IntentPayload {
                files: vec!["src/a.rs".to_string()],
                summary: String::new(),
                valid_for_seconds: 60,
            },
        };
        let rendered = format!("{msg}");
        assert!(
            rendered.ends_with("\u{2014} "),
            "Display should end with em-dash + space when summary is empty; got: {rendered:?}"
        );
        assert!(
            rendered.starts_with("[feat-x] intent: 1 files for 60s "),
            "Display prefix should reflect file count and TTL; got: {rendered:?}"
        );
    }

    // spec-corrections-v0-5-0 envelope + question coverage. The v0.5.0
    // wire format ships seven `BrokerMessage` variants, each tagged via
    // `#[serde(rename = "agent.<lowercase>")]`. The two tests below lock
    // the discriminator strings and the question-payload field shape so a
    // future serde rename can't drift the wire format.

    #[test]
    fn envelope_serde_rename_covers_seven_variants() {
        let variants = [
            (
                BrokerMessage::Status {
                    agent_id: "feat-a".to_string(),
                    payload: StatusPayload {
                        status: "working".to_string(),
                        modified_files: vec![],
                        message: None,
                        cli: None,
                        phase: None,
                    },
                },
                "agent.status",
            ),
            (
                BrokerMessage::Artifact {
                    agent_id: "feat-a".to_string(),
                    payload: ArtifactPayload {
                        status: "committed".to_string(),
                        exports: vec![],
                        modified_files: vec![],
                    },
                },
                "agent.artifact",
            ),
            (
                BrokerMessage::Blocked {
                    agent_id: "feat-a".to_string(),
                    payload: BlockedPayload {
                        needs: "auth token".to_string(),
                        from: "feat-b".to_string(),
                    },
                },
                "agent.blocked",
            ),
            (
                BrokerMessage::Verified {
                    agent_id: "feat-a".to_string(),
                    payload: VerifiedPayload {
                        verified_by: "supervisor".to_string(),
                        message: None,
                    },
                },
                "agent.verified",
            ),
            (
                BrokerMessage::Feedback {
                    agent_id: "feat-a".to_string(),
                    payload: FeedbackPayload {
                        from: "supervisor".to_string(),
                        errors: vec![],
                    },
                },
                "agent.feedback",
            ),
            (
                BrokerMessage::Question {
                    agent_id: "feat-a".to_string(),
                    payload: QuestionPayload {
                        question: "rs256 or hs256?".to_string(),
                    },
                },
                "agent.question",
            ),
            (
                BrokerMessage::Intent {
                    agent_id: "feat-a".to_string(),
                    payload: IntentPayload {
                        files: vec!["src/a.rs".to_string()],
                        summary: "wire AuthClient".to_string(),
                        valid_for_seconds: 900,
                    },
                },
                "agent.intent",
            ),
        ];

        // Sanity: assert we constructed seven distinct variants, matching
        // the spec'd count.
        assert_eq!(
            variants.len(),
            7,
            "expected exactly seven BrokerMessage variants"
        );

        for (msg, expected_tag) in &variants {
            let value = serde_json::to_value(msg).expect("serialise BrokerMessage");
            let obj = value.as_object().unwrap_or_else(|| {
                panic!("BrokerMessage must serialise as JSON object; got {value:?}")
            });
            let tag = obj
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("missing 'type' on {expected_tag} envelope"));
            assert_eq!(
                tag, *expected_tag,
                "wire discriminator drift: expected {expected_tag}, got {tag}",
            );
        }
    }

    #[test]
    fn question_payload_omits_from_field() {
        let payload = QuestionPayload {
            question: "what?".to_string(),
        };
        let value = serde_json::to_value(&payload).expect("serialise QuestionPayload");
        let obj = value
            .as_object()
            .expect("QuestionPayload must serialise as JSON object");
        assert!(
            !obj.contains_key("from"),
            "QuestionPayload must not have a 'from' field; got keys {:?}",
            obj.keys().collect::<Vec<_>>(),
        );
        // Sanity: the only documented field is `question`.
        assert!(
            obj.contains_key("question"),
            "QuestionPayload must serialise the 'question' field; got keys {:?}",
            obj.keys().collect::<Vec<_>>(),
        );
    }
}

//! Broker message types, validation, and branch slug conversion.
//!
//! Defines [`BrokerMessage`] -- the envelope type for all inter-agent
//! communication -- along with its payload structs and helper methods.

use std::fmt;

use chrono::{DateTime, Utc};
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

    /// The `answer` field is empty or whitespace-only.
    #[error("answer field must not be empty")]
    EmptyAnswerField,

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

    /// The advanced-main `merged_branch` field is empty or whitespace-only.
    #[error("merged_branch field must not be empty")]
    EmptyMergedBranch,

    /// The advanced-main `new_main_sha` field is empty or whitespace-only.
    #[error("new_main_sha field must not be empty")]
    EmptyNewMainSha,

    /// The advanced-main `base` field is empty or whitespace-only.
    #[error("base field must not be empty")]
    EmptyBase,
    /// The learning `category` field is empty or whitespace-only.
    #[error("learning category field must not be empty")]
    EmptyCategory,

    /// The learning `title` field is empty or whitespace-only.
    #[error("learning title field must not be empty")]
    EmptyTitle,

    /// The learning `timestamp` field is empty or whitespace-only.
    #[error("learning timestamp field must not be empty")]
    EmptyTimestamp,

    /// JSON deserialization failed.
    #[error("invalid message JSON: {0}")]
    Deserialize(#[from] serde_json::Error),
}

/// Payload for `agent.status` messages.
///
/// `cli`, `phase`, and `detail` are optional and serialise with
/// `skip_serializing_if = "Option::is_none"`, so legacy payloads without these
/// fields deserialise as `None` and new payloads with `None` omit the field
/// from the wire bytes — preserving v0.5.0 wire compatibility byte-for-byte.
///
/// `Eq` is intentionally not derived: `detail` carries a
/// [`serde_json::Value`], which is `PartialEq` but not `Eq`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
    /// the publishing agent's current lifecycle phase. An open string — the
    /// broker does not validate the set of values, so the supervisor's phase
    /// taxonomy (`sweep`, `audit`, `merge`, `feedback`, `intent_watch`,
    /// `learnings`, `idle`, `checkpoint`) can grow without a wire change. The
    /// dashboard prefers this label over the message-type-derived
    /// `status_label()` when rendering the supervisor's row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    /// Optional phase-specific structured detail body. Free-form JSON; the
    /// broker does not validate its shape. Populated by the supervisor's
    /// introspection emissions (e.g. `{ "branch": "feat/x", "audit_step":
    /// "tests" }` for `phase = "audit"`) and surfaced by the MCP
    /// `get_session_status` tool. Consumers treat an unrecognised shape
    /// gracefully — extracting documented fields and ignoring the rest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
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

/// A declared region within a file an agent intends to touch.
///
/// Serialised as a `tag = "kind"` enum so each region carries an explicit
/// `kind` discriminator on the wire (`{"kind": "function", "name": "..."}`).
/// Tagged (not untagged) serialisation is deliberate: an unknown `kind` fails
/// to deserialise loudly rather than silently falling through to whichever
/// variant serde tries first — a dropped region would weaken the detector's
/// signal. The set is closed at four kinds in v0.6.0; adding a kind is an
/// additive wire change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Region {
    /// A named function / method symbol.
    Function {
        /// The symbol name, compared as an opaque string (no source parsing).
        name: String,
    },
    /// A named class / struct / type symbol.
    Class {
        /// The symbol name, compared as an opaque string.
        name: String,
    },
    /// A prose / config landmark (Markdown heading, config block, etc.) for
    /// files without code symbols.
    Block {
        /// The free-form landmark text.
        anchor: String,
    },
    /// A line-range hint, used only when symbolic names don't fit.
    Range {
        /// First line of the range (1-based, inclusive).
        start_line: u32,
        /// Last line of the range (inclusive).
        end_line: u32,
    },
}

impl fmt::Display for Region {
    /// Renders a region as `kind name` (or `range start-end`) for warning
    /// text and dashboard summaries.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Function { name } => write!(f, "function {name}"),
            Self::Class { name } => write!(f, "class {name}"),
            Self::Block { anchor } => write!(f, "block {anchor}"),
            Self::Range {
                start_line,
                end_line,
            } => write!(f, "range {start_line}-{end_line}"),
        }
    }
}

/// One entry in an intent's `files` array.
///
/// Accepts EITHER the v0.5.0 plain-string shape (`"src/main.rs"` → file-level
/// intent) OR the v0.6.0 object shape (`{ "path": "...", "regions": [...] }`).
/// Both forms may appear in the same array. Serialised via an `untagged` enum
/// so a [`FileIntent::Path`] round-trips to a bare JSON string — preserving
/// v0.5.0 wire bytes for string-only publishers — while
/// [`FileIntent::Detailed`] round-trips to an object. An empty `regions` vec
/// is omitted from the wire bytes (a detailed entry with no regions is
/// equivalent to the plain string form).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FileIntent {
    /// File-level intent: just a path, no declared regions (v0.5.0 shape).
    Path(String),
    /// File intent with optional declared regions (v0.6.0 shape).
    Detailed {
        /// The file path.
        path: String,
        /// Declared regions within the file; empty / omitted means
        /// file-level intent.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        regions: Vec<Region>,
    },
}

impl FileIntent {
    /// Returns the file path regardless of which shape this entry uses.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::Path(p) | Self::Detailed { path: p, .. } => p,
        }
    }

    /// Returns the declared regions, or `None` for a file-level intent.
    ///
    /// A plain-string entry and an object entry with an empty `regions` vec
    /// both report `None` — both mean "file-level intent, no regions".
    #[must_use]
    pub fn regions(&self) -> Option<&[Region]> {
        match self {
            Self::Path(_) => None,
            Self::Detailed { regions, .. } if regions.is_empty() => None,
            Self::Detailed { regions, .. } => Some(regions),
        }
    }
}

impl From<&str> for FileIntent {
    fn from(s: &str) -> Self {
        Self::Path(s.to_string())
    }
}

/// Payload for `agent.intent` messages.
///
/// Wire format: `{"type": "agent.intent", "agent_id": "<slug>", "payload": {...}}`.
/// `files` declares paths the agent plans to modify (relative to the repository
/// root; globs are permitted but discouraged). Each entry is a [`FileIntent`] —
/// either a plain path string (v0.5.0 file-level intent) or an object carrying
/// optional [`Region`] hints. `summary` is a one-line human description.
/// `valid_for_seconds` is a relative TTL after which a consumer MAY treat the
/// intent as stale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentPayload {
    /// File intents the agent intends to modify — plain paths or
    /// `{ path, regions }` objects.
    pub files: Vec<FileIntent>,
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

/// Payload for `agent.answer` messages.
///
/// Carries a non-error supervisor→agent reply to an `agent.question`. The
/// envelope's `agent_id` names the TARGET agent (the one being answered);
/// the sender lives in the payload's `from` field, mirroring
/// [`FeedbackPayload`]. Unlike feedback, an answer is authoritative guidance
/// to act on — not a corrective error list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnswerPayload {
    /// Agent ID of the sender (typically `"supervisor"`).
    pub from: String,
    /// The reply text.
    pub answer: String,
    /// Optional short free-text reference to the question being answered;
    /// omitted from the wire bytes when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub re: Option<String>,
}

/// Payload for `agent.advanced-main` messages.
///
/// Published by the supervisor after a successful merge to the repository's
/// default branch so downstream agents learn the base moved without polling
/// git directly. The wire shape is flat — the payload fields sit at the top
/// level of the envelope alongside the `"type"` discriminator (see
/// [`BrokerMessage::AdvancedMain`]'s `#[serde(flatten)]`), matching the curl
/// example the supervisor skill teaches.
///
/// All fields are required except `summary`, which the publishing supervisor
/// LLM populates with a one-line human-readable description and which
/// serialises with `skip_serializing_if = "Option::is_none"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvancedMainPayload {
    /// Who advanced main — typically `"supervisor"`.
    pub from: String,
    /// The branch that was just merged.
    pub merged_branch: String,
    /// The new abbreviated SHA of the default branch (12+ chars by
    /// convention; the broker does not validate length or existence).
    pub new_main_sha: String,
    /// The base branch that advanced — the resolved default-branch name
    /// (typically `"main"`), carried explicitly so consumers need not look
    /// up the session's default branch.
    pub base: String,
    /// When the merge landed, as a UTC timestamp.
    pub merged_at: DateTime<Utc>,
    /// Optional one-line human-readable summary of what merged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

impl AdvancedMainPayload {
    /// Returns the deterministic dedup id for this advance event.
    ///
    /// Convenience wrapper over [`advanced_main_id`] using this payload's
    /// fields. See that function for the canonical input and hashing
    /// contract.
    #[must_use]
    pub fn deterministic_id(&self) -> String {
        advanced_main_id(
            &self.merged_branch,
            &self.new_main_sha,
            &self.base,
            self.merged_at,
        )
    }
}

/// Computes the deterministic dedup `id` for an `agent.advanced-main` event.
///
/// Reuses the `agent.learning` id-hashing pattern from the
/// `agent-learning-variant` capability — the std-hash shape (no third-party
/// crates): a `std::collections::hash_map::DefaultHasher` over a canonical,
/// newline-delimited serialisation of
/// `merged_branch | new_main_sha | base | hour_bucket`, rendered as a
/// zero-padded 16-hex-char (64-bit) string. `hour_bucket` is the UTC
/// `YYYY-MM-DDTHH` truncation of `merged_at`.
///
/// The hour bucket is a deduplication safety net: re-emitting the same merge
/// within the same UTC hour yields an identical id, while the same merge
/// across an hour boundary yields a different id (matching the learning
/// variant's recurrence-detection contract). Because `new_main_sha` is unique
/// per merge, distinct merges effectively never collide regardless of bucket.
///
/// `DefaultHasher` is seeded with fixed keys, so the id is stable within a
/// session and across processes built from the same toolchain — sufficient
/// for in-session / in-log dedup, which is all this id is used for.
#[must_use]
pub fn advanced_main_id(
    merged_branch: &str,
    new_main_sha: &str,
    base: &str,
    merged_at: DateTime<Utc>,
) -> String {
    use std::hash::{Hash as _, Hasher as _};

    let hour_bucket = merged_at.format("%Y-%m-%dT%H").to_string();
    let canonical = format!("{merged_branch}\n{new_main_sha}\n{base}\n{hour_bucket}");
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    canonical.hash(&mut hasher);
    // u64 -> zero-padded 16 hex chars, matching agent.learning's std-hash id.
    format!("{:016x}", hasher.finish())
}

/// Payload for `agent.learning` messages.
///
/// Carries one structured learning record produced by the broker's learnings
/// aggregator (see [`crate::broker::learnings`]). The shape is fixed across
/// all categories: `category` is an *open* string tag (consumers filter on
/// it; descendant changes may add values without a broker change), and `body`
/// is a category-specific structured object typed as [`serde_json::Value`].
///
/// `branch_id` is optional and omitted from the wire bytes when `None`
/// (cross-cutting records such as permission patterns and conflict pairs are
/// not scoped to a single branch). The `id` is the deterministic dedup hash
/// produced by [`crate::broker::learnings::LearningRecord::deterministic_id`].
///
/// Note: unlike the other `agent.*` variants, the sender `agent_id` lives in
/// the payload rather than the envelope (this variant has no separate
/// envelope `agent_id`). [`BrokerMessage::agent_id`] resolves it from here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearningPayload {
    /// Deterministic dedup id — a stable 16-hex-char (64-bit) hash over the
    /// record's canonical serialisation. Stable for the same logical record
    /// within a UTC hour.
    pub id: String,
    /// The publishing agent id (typically `"supervisor"`, since the
    /// aggregator runs in the broker/supervisor context).
    pub agent_id: String,
    /// Branch the learning is scoped to; `None` (and omitted on the wire) for
    /// cross-cutting records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<String>,
    /// Open category tag — one of `conflict_event`, `stuck_duration`,
    /// `recovery_cycles`, `permission_pattern`, or any value added by a
    /// descendant change.
    pub category: String,
    /// Short human-readable summary.
    pub title: String,
    /// Category-specific structured body.
    pub body: serde_json::Value,
    /// ISO 8601 UTC timestamp.
    pub timestamp: String,
}

/// Envelope for all inter-agent messages.
///
/// The wire format uses JSON with an internally tagged `"type"` discriminator
/// whose values are `"agent.status"`, `"agent.artifact"`, `"agent.blocked"`,
/// `"agent.verified"`, `"agent.feedback"`, `"agent.answer"`,
/// `"agent.question"`, `"agent.intent"`, `"agent.advanced-main"`,
/// `"agent.learning"`, and `"supervisor.verify-now"`.
/// The last is broker-emitted rather than agent-published; see
/// [`BrokerMessage::VerifyNow`].
///
/// `Eq` is intentionally not derived: the `agent.learning` payload carries a
/// [`serde_json::Value`] body, which is `PartialEq` but not `Eq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Non-error supervisor reply -- delivered to the target agent only.
    ///
    /// Answers an `agent.question` with authoritative guidance; corrective
    /// errors stay on `agent.feedback`. Routed like feedback: the envelope's
    /// `agent_id` names the TARGET agent and the sender is `payload.from`.
    #[serde(rename = "agent.answer")]
    Answer {
        /// Target agent ID (the agent being answered).
        agent_id: String,
        /// Answer payload (contains `from` -- the sender).
        payload: AnswerPayload,
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
    /// Main-advanced notification -- published by the supervisor after a
    /// successful merge to the default branch, broadcast to every registered
    /// agent's inbox so dependents learn the base moved.
    ///
    /// The payload is flattened into the envelope (its fields sit at the top
    /// level alongside `"type"`, not nested under a `payload` key) so the
    /// wire shape matches the curl example the supervisor skill teaches. The
    /// sender identity is the payload's `from` field (typically
    /// `"supervisor"`), surfaced through [`BrokerMessage::agent_id`].
    #[serde(rename = "agent.advanced-main")]
    AdvancedMain {
        /// Flattened advanced-main payload (`from`, `merged_branch`,
        /// `new_main_sha`, `base`, `merged_at`, optional `summary`).
        #[serde(flatten)]
        payload: AdvancedMainPayload,
    },
    /// Structured learning record -- published by the broker's learnings
    /// aggregator when `[supervisor] learnings = true` and broker publish is
    /// active (see [`crate::broker::learnings`]). Carries a deterministic
    /// dedup `id` so consumers can collapse re-emissions. Routed to the
    /// scoped `branch_id` inbox when present, otherwise the supervisor inbox;
    /// always retained in the broker message log. The variant is additive --
    /// existing consumers ignore unknown types per the broker-messages
    /// contract.
    #[serde(rename = "agent.learning")]
    Learning {
        /// Learning payload (carries its own `agent_id`).
        payload: LearningPayload,
    },
    /// Supervisor verification nudge -- emitted by the broker (not by an
    /// agent) when an `agent.artifact { status: "committed" }` arrives and
    /// `[supervisor].verify_on_commit_nudge` is enabled. Delivered to the
    /// `"supervisor"` inbox so per-commit verification is triggered by an
    /// explicit event rather than relying on the supervisor's sweep cadence
    /// to notice the commit.
    ///
    /// Unlike the `agent.*` variants this carries a `branch_id` directly
    /// (the committing branch) rather than a sender `agent_id` plus a payload
    /// -- the message originates from the broker itself, so there is no
    /// publishing agent.
    #[serde(rename = "supervisor.verify-now")]
    VerifyNow {
        /// The committing branch whose commit should be verified now. Copied
        /// verbatim from the triggering artifact's `agent_id`.
        branch_id: String,
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
            | Self::Answer { agent_id, .. }
            | Self::Question { agent_id, .. }
            | Self::Intent { agent_id, .. } => agent_id,
            // `AdvancedMain` has no top-level `agent_id`; the sender identity
            // is the payload's `from` field (typically `"supervisor"`).
            Self::AdvancedMain { payload } => &payload.from,
            // `Learning` carries its sender in the payload (no envelope id).
            Self::Learning { payload } => &payload.agent_id,
            // `VerifyNow` has no publishing agent; the closest identity is the
            // committing branch it nudges verification for.
            Self::VerifyNow { branch_id } => branch_id,
        }
    }

    /// Returns a short status label for the message.
    ///
    /// - `Status` returns `payload.status` (e.g. `"working"`)
    /// - `Artifact` returns `payload.status` (e.g. `"done"`)
    /// - `Blocked` returns `"blocked"`
    /// - `Verified` returns `"verified"`
    /// - `Feedback` returns `"feedback"`
    /// - `Answer` returns `"answer"`
    /// - `Question` returns `"question"`
    /// - `Intent` returns `"intent"`
    /// - `AdvancedMain` returns `"advanced-main"`
    /// - `VerifyNow` returns `"verify-now"`
    pub fn status_label(&self) -> &str {
        match self {
            Self::Status { payload, .. } => &payload.status,
            Self::Artifact { payload, .. } => &payload.status,
            Self::Blocked { .. } => "blocked",
            Self::Verified { .. } => "verified",
            Self::Feedback { .. } => "feedback",
            Self::Answer { .. } => "answer",
            Self::Question { .. } => "question",
            Self::Intent { .. } => "intent",
            Self::AdvancedMain { .. } => "advanced-main",
            Self::Learning { .. } => "learning",
            Self::VerifyNow { .. } => "verify-now",
        }
    }

    /// Validates all fields according to the broker message spec.
    ///
    /// The `agent_id` *shape* is enforced at the HTTP boundary by
    /// `src/broker/server.rs::publish` against the canonical regex
    /// `^(supervisor|[a-z0-9][a-z0-9-]*[/-][a-z0-9][a-z0-9-]*)$` (any
    /// `{prefix}/{name}` or `{prefix}-{name}` slug, not just `feat`)
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
            Self::Answer { payload, .. } => {
                if payload.from.trim().is_empty() {
                    return Err(MessageError::EmptyFromField);
                }
                if payload.answer.trim().is_empty() {
                    return Err(MessageError::EmptyAnswerField);
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
                if payload.files.iter().any(|f| f.path().trim().is_empty()) {
                    return Err(MessageError::EmptyIntentFileEntry);
                }
                if payload.summary.trim().is_empty() {
                    return Err(MessageError::EmptyIntentSummary);
                }
                if payload.valid_for_seconds == 0 {
                    return Err(MessageError::ZeroValidForSeconds);
                }
            }
            Self::AdvancedMain { payload } => {
                // `from` is this message's `agent_id()`, so the empty-id guard
                // at the top of this method already rejects a blank `from`.
                // The remaining required string fields are checked here so a
                // present-but-blank value trips a clear, field-named error.
                // `merged_at` is typed as `DateTime<Utc>`, so serde rejects an
                // absent or malformed timestamp before this validator runs.
                if payload.merged_branch.trim().is_empty() {
                    return Err(MessageError::EmptyMergedBranch);
                }
                if payload.new_main_sha.trim().is_empty() {
                    return Err(MessageError::EmptyNewMainSha);
                }
                if payload.base.trim().is_empty() {
                    return Err(MessageError::EmptyBase);
                }
            }
            Self::Learning { payload } => {
                // The empty-agent-id guard at the top already covers a blank
                // `payload.agent_id`. `body` presence is guaranteed by serde
                // (a required field); absence surfaces as a deserialize error
                // before we get here. We only reject present-but-empty
                // required string fields.
                if payload.category.trim().is_empty() {
                    return Err(MessageError::EmptyCategory);
                }
                if payload.title.trim().is_empty() {
                    return Err(MessageError::EmptyTitle);
                }
                if payload.timestamp.trim().is_empty() {
                    return Err(MessageError::EmptyTimestamp);
                }
            }
            // `branch_id` is the message's `agent_id()`, so the empty-id guard
            // at the top of this method already rejects a blank branch.
            Self::VerifyNow { .. } => {}
        }
        Ok(())
    }
}

impl fmt::Display for BrokerMessage {
    #[allow(clippy::too_many_lines)] // one display arm per message variant
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
            Self::Answer {
                agent_id, payload, ..
            } => {
                if let Some(re) = &payload.re {
                    write!(
                        f,
                        "[{agent_id}] answer from {} (re: {re}): {}",
                        payload.from, payload.answer
                    )
                } else {
                    write!(
                        f,
                        "[{agent_id}] answer from {}: {}",
                        payload.from, payload.answer
                    )
                }
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
            Self::AdvancedMain { payload } => {
                write!(
                    f,
                    "[{}] advanced-main: {} \u{2192} {} ({})",
                    payload.from, payload.merged_branch, payload.base, payload.new_main_sha
                )
            }
            Self::Learning { payload } => {
                let scope = payload.branch_id.as_deref().unwrap_or("*");
                write!(
                    f,
                    "[{}] learning ({}/{}): {}",
                    payload.agent_id, payload.category, scope, payload.title
                )
            }
            Self::VerifyNow { branch_id } => {
                write!(f, "[{branch_id}] verify-now")
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
            detail: None,
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
            detail: None,
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

    // --- supervisor-introspection: phase + detail fields (tasks 1.2-1.4) ---

    #[test]
    fn status_payload_v050_shape_round_trips_byte_equivalent() {
        // GIVEN a v0.5.0-shape status with no phase/detail (and no cli).
        // WHEN it is deserialised and re-serialised THEN the JSON must be
        // byte-equivalent — no `phase`/`detail`/`cli` null keys appear.
        let json = r#"{"status":"working","modified_files":["src/a.rs"],"message":"booting"}"#;
        let payload: StatusPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.phase, None);
        assert_eq!(payload.detail, None);
        let round_tripped = serde_json::to_string(&payload).unwrap();
        assert_eq!(
            round_tripped, json,
            "v0.5.0 payload must round-trip byte-equivalently; got {round_tripped}"
        );
    }

    #[test]
    fn status_payload_round_trips_with_phase_and_detail() {
        // Status with phase = "audit" and a structured detail body
        // round-trips losslessly through serde.
        let payload = StatusPayload {
            status: "working".to_string(),
            modified_files: vec![],
            message: None,
            cli: None,
            phase: Some("audit".to_string()),
            detail: Some(serde_json::json!({
                "branch": "feat/x",
                "audit_step": "tests",
            })),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"phase\":\"audit\""));
        assert!(json.contains("\"audit_step\":\"tests\""));
        let back: StatusPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back, payload);
        assert_eq!(
            back.detail.as_ref().unwrap()["branch"],
            serde_json::json!("feat/x")
        );
    }

    #[test]
    fn status_payload_accepts_unknown_phase_value() {
        // An unknown phase string (not in the v0.6.0 taxonomy) is accepted —
        // the broker does not validate the set of phase values.
        let json = r#"{"type":"agent.status","agent_id":"supervisor","payload":{"status":"working","modified_files":[],"phase":"future_value_not_in_v0_6_0_taxonomy","detail":{"k":"v"}}}"#;
        let msg = BrokerMessage::from_json(json).expect("unknown phase accepted");
        match &msg {
            BrokerMessage::Status { payload, .. } => {
                assert_eq!(
                    payload.phase.as_deref(),
                    Some("future_value_not_in_v0_6_0_taxonomy")
                );
                assert!(payload.detail.is_some());
            }
            other => panic!("expected Status variant, got {other:?}"),
        }
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

    fn make_answer(agent_id: &str, from: &str, answer: &str, re: Option<&str>) -> BrokerMessage {
        BrokerMessage::Answer {
            agent_id: agent_id.to_string(),
            payload: AnswerPayload {
                from: from.to_string(),
                answer: answer.to_string(),
                re: re.map(str::to_string),
            },
        }
    }

    #[test]
    fn serde_roundtrip_answer_with_re() {
        // Spec scenario: valid answer round-trips through serde.
        let json = r#"{"type":"agent.answer","agent_id":"feat-x","payload":{"from":"supervisor","answer":"Use the existing helper; do not add a dependency","re":"add crate X?"}}"#;
        let msg = BrokerMessage::from_json(json).unwrap();
        assert_eq!(
            msg,
            make_answer(
                "feat-x",
                "supervisor",
                "Use the existing helper; do not add a dependency",
                Some("add crate X?"),
            )
        );
        let back = serde_json::to_string(&msg).unwrap();
        assert!(back.contains("\"type\":\"agent.answer\""));
        assert!(back.contains("\"re\":\"add crate X?\""));
        let reparsed: BrokerMessage = serde_json::from_str(&back).unwrap();
        assert_eq!(reparsed, msg);
    }

    #[test]
    fn serde_roundtrip_answer_without_re_omits_field() {
        // Spec scenario: `re` is optional and omitted from serialization.
        let msg = make_answer("feat-x", "supervisor", "yes, proceed", None);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"agent.answer\""));
        assert!(
            !json.contains("\"re\""),
            "absent re must be omitted: {json}"
        );
        let back: BrokerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn from_json_answer_without_re_validates() {
        let json = r#"{"type":"agent.answer","agent_id":"feat-x","payload":{"from":"supervisor","answer":"yes"}}"#;
        let msg = BrokerMessage::from_json(json).unwrap();
        match &msg {
            BrokerMessage::Answer { payload, .. } => assert_eq!(payload.re, None),
            other => panic!("expected Answer variant, got {other:?}"),
        }
    }

    #[test]
    fn from_json_empty_answer_rejected() {
        // Spec scenario: empty answer is rejected with a named error.
        let json = r#"{"type":"agent.answer","agent_id":"feat-x","payload":{"from":"supervisor","answer":""}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyAnswerField));
    }

    #[test]
    fn from_json_empty_answer_from_rejected() {
        // Spec scenario: empty from is rejected with a named error.
        let json =
            r#"{"type":"agent.answer","agent_id":"feat-x","payload":{"from":"","answer":"yes"}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyFromField));
    }

    #[test]
    fn display_answer_with_re() {
        let msg = make_answer("feat-x", "supervisor", "use the helper", Some("crate X?"));
        assert_eq!(
            msg.to_string(),
            "[feat-x] answer from supervisor (re: crate X?): use the helper"
        );
    }

    #[test]
    fn display_answer_without_re() {
        let msg = make_answer("feat-x", "supervisor", "use the helper", None);
        assert_eq!(
            msg.to_string(),
            "[feat-x] answer from supervisor: use the helper"
        );
    }

    #[test]
    fn status_label_answer() {
        let msg = make_answer("feat-x", "supervisor", "yes", None);
        assert_eq!(msg.status_label(), "answer");
    }

    #[test]
    fn agent_id_answer_is_the_target() {
        let msg = make_answer("feat-x", "supervisor", "yes", None);
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
                files: files.iter().map(|s| FileIntent::from(*s)).collect(),
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
            let paths: Vec<&str> = payload.files.iter().map(FileIntent::path).collect();
            assert_eq!(paths, vec!["src/auth.rs", "src/auth/client.rs"]);
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
            assert_eq!(payload.files, vec![FileIntent::from("src/auth.rs")]);
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
                files: vec![FileIntent::from("src/a.rs")],
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

    // === FileIntent / Region wire shape (conflict-detector-fn-granularity
    //     tasks 1.4) ===

    #[test]
    fn file_intent_string_entry_round_trips_to_bare_string() {
        // The v0.5.0 plain-string shape parses to a file-level intent and
        // serialises back to a bare JSON string (no object wrapper).
        let parsed: FileIntent = serde_json::from_str(r#""src/main.rs""#).unwrap();
        assert_eq!(parsed, FileIntent::Path("src/main.rs".to_string()));
        assert!(parsed.regions().is_none(), "string entry has no regions");
        assert_eq!(serde_json::to_string(&parsed).unwrap(), r#""src/main.rs""#);
    }

    #[test]
    fn file_intent_object_entry_with_regions_round_trips() {
        let json =
            r#"{"path":"src/auth.rs","regions":[{"kind":"function","name":"validate_token"}]}"#;
        let parsed: FileIntent = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.path(), "src/auth.rs");
        assert_eq!(
            parsed.regions(),
            Some(
                &vec![Region::Function {
                    name: "validate_token".to_string()
                }][..]
            )
        );
        // Re-serialises to the same object shape.
        assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
    }

    #[test]
    fn file_intent_object_entry_without_regions_omits_field() {
        // An object entry whose regions vec is empty serialises without the
        // `regions` key — equivalent on the wire to the plain string form.
        let entry = FileIntent::Detailed {
            path: "src/main.rs".to_string(),
            regions: vec![],
        };
        assert_eq!(
            serde_json::to_string(&entry).unwrap(),
            r#"{"path":"src/main.rs"}"#
        );
        // And `{"path": "..."}` parses back as a Detailed with no regions.
        let parsed: FileIntent = serde_json::from_str(r#"{"path":"src/main.rs"}"#).unwrap();
        assert_eq!(parsed.path(), "src/main.rs");
        assert!(parsed.regions().is_none());
    }

    #[test]
    fn intent_mixed_string_and_object_files_round_trip() {
        let json = r#"{"type":"agent.intent","agent_id":"feat-x","payload":{"files":["src/main.rs",{"path":"src/auth.rs","regions":[{"kind":"function","name":"validate_token"}]}],"summary":"s","valid_for_seconds":60}}"#;
        let msg = BrokerMessage::from_json(json).unwrap();
        let BrokerMessage::Intent { payload, .. } = &msg else {
            panic!("expected Intent");
        };
        assert_eq!(payload.files.len(), 2);
        assert_eq!(
            payload.files[0],
            FileIntent::Path("src/main.rs".to_string())
        );
        assert_eq!(payload.files[1].path(), "src/auth.rs");
        assert_eq!(payload.files[1].regions().unwrap().len(), 1);
        // Round-trips byte-equivalently.
        assert_eq!(serde_json::to_string(&msg).unwrap(), json);
    }

    #[test]
    fn region_each_kind_round_trips() {
        let cases = [
            (
                Region::Function {
                    name: "f".to_string(),
                },
                r#"{"kind":"function","name":"f"}"#,
            ),
            (
                Region::Class {
                    name: "C".to_string(),
                },
                r#"{"kind":"class","name":"C"}"#,
            ),
            (
                Region::Block {
                    anchor: "Setup".to_string(),
                },
                r#"{"kind":"block","anchor":"Setup"}"#,
            ),
            (
                Region::Range {
                    start_line: 10,
                    end_line: 50,
                },
                r#"{"kind":"range","start_line":10,"end_line":50}"#,
            ),
        ];
        for (region, expected_json) in cases {
            let json = serde_json::to_string(&region).unwrap();
            assert_eq!(json, expected_json);
            let back: Region = serde_json::from_str(&json).unwrap();
            assert_eq!(back, region);
        }
    }

    #[test]
    fn region_unknown_kind_rejected_with_clear_error() {
        let json = r#"{"kind":"macro","name":"vec"}"#;
        let err = serde_json::from_str::<Region>(json).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("macro"),
            "error should identify the offending kind `macro`; got: {msg}"
        );
    }

    #[test]
    fn intent_with_unknown_region_kind_rejected() {
        let json = r#"{"type":"agent.intent","agent_id":"feat-x","payload":{"files":[{"path":"src/a.rs","regions":[{"kind":"macro","name":"vec"}]}],"summary":"s","valid_for_seconds":60}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::Deserialize(_)));
    }

    #[test]
    fn v050_string_only_intent_round_trips_byte_equivalent() {
        // Backwards compatibility: an intent published with the v0.5.0
        // string-only `files` shape must re-serialise byte-for-byte — no
        // object wrappers, no `regions` keys leak in.
        let json = r#"{"type":"agent.intent","agent_id":"feat-x","payload":{"files":["src/foo.rs","src/bar.rs"],"summary":"s","valid_for_seconds":900}}"#;
        let msg = BrokerMessage::from_json(json).unwrap();
        assert_eq!(
            serde_json::to_string(&msg).unwrap(),
            json,
            "v0.5.0 string-only intent must round-trip byte-equivalently"
        );
    }

    #[test]
    fn region_display_renders_kind_and_name() {
        assert_eq!(
            Region::Function {
                name: "validate_token".to_string()
            }
            .to_string(),
            "function validate_token"
        );
        assert_eq!(
            Region::Class {
                name: "Auth".to_string()
            }
            .to_string(),
            "class Auth"
        );
        assert_eq!(
            Region::Block {
                anchor: "Setup".to_string()
            }
            .to_string(),
            "block Setup"
        );
        assert_eq!(
            Region::Range {
                start_line: 10,
                end_line: 30
            }
            .to_string(),
            "range 10-30"
        );
    }

    // spec-corrections-v0-5-0 envelope + question coverage. The v0.5.0
    // wire format ships seven `BrokerMessage` variants, each tagged via
    // `#[serde(rename = "agent.<lowercase>")]`. The two tests below lock
    // the discriminator strings and the question-payload field shape so a
    // future serde rename can't drift the wire format.

    #[test]
    #[allow(clippy::too_many_lines)] // exhaustive variant data table
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
                        detail: None,
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
                BrokerMessage::Answer {
                    agent_id: "feat-a".to_string(),
                    payload: AnswerPayload {
                        from: "supervisor".to_string(),
                        answer: "use rs256".to_string(),
                        re: Some("rs256 or hs256?".to_string()),
                    },
                },
                "agent.answer",
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
                        files: vec![FileIntent::from("src/a.rs")],
                        summary: "wire AuthClient".to_string(),
                        valid_for_seconds: 900,
                    },
                },
                "agent.intent",
            ),
            (
                BrokerMessage::AdvancedMain {
                    payload: AdvancedMainPayload {
                        from: "supervisor".to_string(),
                        merged_branch: "feat/auth".to_string(),
                        new_main_sha: "a1b2c3d4e5f6".to_string(),
                        base: "main".to_string(),
                        merged_at: DateTime::parse_from_rfc3339("2026-06-04T13:30:00Z")
                            .unwrap()
                            .with_timezone(&Utc),
                        summary: None,
                    },
                },
                "agent.advanced-main",
            ),
            (
                BrokerMessage::Learning {
                    payload: LearningPayload {
                        id: "deadbeefdeadbeef".to_string(),
                        agent_id: "supervisor".to_string(),
                        branch_id: Some("feat/x".to_string()),
                        category: "conflict_event".to_string(),
                        title: "forward conflict: feat-x and feat-y".to_string(),
                        body: serde_json::json!({"shape": "forward"}),
                        timestamp: "2026-05-28T12:01:01Z".to_string(),
                    },
                },
                "agent.learning",
            ),
            (
                BrokerMessage::VerifyNow {
                    branch_id: "feat/foo".to_string(),
                },
                "supervisor.verify-now",
            ),
        ];

        // Sanity: assert we constructed eleven distinct variants, matching the
        // spec'd count (ten `agent.*` — now including `agent.answer` alongside
        // `agent.advanced-main` and `agent.learning` — plus the broker-emitted
        // `supervisor.verify-now` nudge).
        assert_eq!(
            variants.len(),
            11,
            "expected exactly eleven BrokerMessage variants"
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

    // === supervisor.verify-now nudge (per-commit-verification-v0-6-x) ===

    #[test]
    fn verify_now_round_trips_with_branch_id() {
        let json = r#"{"type":"supervisor.verify-now","branch_id":"feat/foo"}"#;
        let msg = BrokerMessage::from_json(json).expect("verify-now must parse");
        let BrokerMessage::VerifyNow { branch_id } = &msg else {
            panic!("expected VerifyNow, got {msg:?}");
        };
        assert_eq!(branch_id, "feat/foo");

        // Re-serialise and confirm the discriminator + field survive.
        let value = serde_json::to_value(&msg).expect("serialise VerifyNow");
        assert_eq!(
            value.get("type").and_then(|v| v.as_str()),
            Some("supervisor.verify-now")
        );
        assert_eq!(
            value.get("branch_id").and_then(|v| v.as_str()),
            Some("feat/foo")
        );
    }

    #[test]
    fn verify_now_exposes_branch_as_agent_id_and_label() {
        let msg = BrokerMessage::VerifyNow {
            branch_id: "feat-bar".to_string(),
        };
        assert_eq!(msg.agent_id(), "feat-bar");
        assert_eq!(msg.status_label(), "verify-now");
    }

    #[test]
    fn verify_now_rejects_blank_branch_id() {
        let json = r#"{"type":"supervisor.verify-now","branch_id":"   "}"#;
        assert!(
            matches!(
                BrokerMessage::from_json(json),
                Err(MessageError::EmptyAgentId)
            ),
            "blank branch_id must be rejected as an empty agent id"
        );
    }

    // === agent.advanced-main variant (advanced-main-event) ===

    fn sample_advanced_main_json() -> &'static str {
        r#"{"type":"agent.advanced-main","from":"supervisor","merged_branch":"feat/auth","new_main_sha":"a1b2c3d4e5f6","base":"main","merged_at":"2026-06-04T13:30:00Z","summary":"landed auth client"}"#
    }

    #[test]
    fn advanced_main_round_trips_with_all_fields() {
        let msg = BrokerMessage::from_json(sample_advanced_main_json())
            .expect("well-formed advanced-main parses");
        let BrokerMessage::AdvancedMain { payload } = &msg else {
            panic!("expected AdvancedMain, got {msg:?}");
        };
        assert_eq!(payload.from, "supervisor");
        assert_eq!(payload.merged_branch, "feat/auth");
        assert_eq!(payload.new_main_sha, "a1b2c3d4e5f6");
        assert_eq!(payload.base, "main");
        assert_eq!(payload.summary.as_deref(), Some("landed auth client"));

        // Re-serialise: the payload fields must be flat (top-level), not
        // nested under a `payload` key, and the discriminator must survive.
        let value = serde_json::to_value(&msg).expect("serialise AdvancedMain");
        assert_eq!(
            value.get("type").and_then(|v| v.as_str()),
            Some("agent.advanced-main")
        );
        assert_eq!(
            value.get("merged_branch").and_then(|v| v.as_str()),
            Some("feat/auth"),
            "merged_branch must be flattened to the envelope top level; got {value:?}"
        );
        assert!(
            value.get("payload").is_none(),
            "advanced-main fields must not nest under a `payload` key; got {value:?}"
        );

        // Full round-trip equality.
        let back: BrokerMessage =
            serde_json::from_value(value).expect("deserialise re-serialised value");
        assert_eq!(back, msg);
    }

    #[test]
    fn advanced_main_summary_omitted_when_absent() {
        let json = r#"{"type":"agent.advanced-main","from":"supervisor","merged_branch":"feat/x","new_main_sha":"deadbeefcafe","base":"main","merged_at":"2026-06-04T13:30:00Z"}"#;
        let msg = BrokerMessage::from_json(json).expect("parses without summary");
        let BrokerMessage::AdvancedMain { payload } = &msg else {
            panic!("expected AdvancedMain");
        };
        assert_eq!(payload.summary, None);
        // `summary` must be skipped on the wire when None.
        let serialised = serde_json::to_string(&msg).unwrap();
        assert!(
            !serialised.contains("summary"),
            "summary key must be omitted when None; got {serialised}"
        );
    }

    #[test]
    fn advanced_main_preserves_summary_verbatim() {
        let msg = BrokerMessage::from_json(sample_advanced_main_json()).unwrap();
        if let BrokerMessage::AdvancedMain { payload } = &msg {
            assert_eq!(payload.summary.as_deref(), Some("landed auth client"));
        }
    }

    // === agent.learning variant (agent-learning-variant) ===

    fn make_learning(
        id: &str,
        agent_id: &str,
        branch_id: Option<&str>,
        category: &str,
        title: &str,
        body: serde_json::Value,
    ) -> BrokerMessage {
        BrokerMessage::Learning {
            payload: LearningPayload {
                id: id.to_string(),
                agent_id: agent_id.to_string(),
                branch_id: branch_id.map(str::to_string),
                category: category.to_string(),
                title: title.to_string(),
                body,
                timestamp: "2026-05-28T12:01:01Z".to_string(),
            },
        }
    }

    #[test]
    fn advanced_main_missing_merged_branch_rejected() {
        // serde reports the absent required field by name -> 400-class.
        let json = r#"{"type":"agent.advanced-main","from":"supervisor","new_main_sha":"abc123abc123","base":"main","merged_at":"2026-06-04T13:30:00Z"}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        let text = err.to_string();
        assert!(
            matches!(err, MessageError::Deserialize(_)) && text.contains("merged_branch"),
            "missing merged_branch must be rejected and named; got {text}"
        );
    }

    #[test]
    fn advanced_main_missing_new_main_sha_rejected() {
        let json = r#"{"type":"agent.advanced-main","from":"supervisor","merged_branch":"feat/x","base":"main","merged_at":"2026-06-04T13:30:00Z"}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(err.to_string().contains("new_main_sha"));
    }

    #[test]
    fn advanced_main_missing_base_rejected() {
        let json = r#"{"type":"agent.advanced-main","from":"supervisor","merged_branch":"feat/x","new_main_sha":"abc123abc123","merged_at":"2026-06-04T13:30:00Z"}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(err.to_string().contains("base"));
    }

    #[test]
    fn advanced_main_missing_merged_at_rejected() {
        let json = r#"{"type":"agent.advanced-main","from":"supervisor","merged_branch":"feat/x","new_main_sha":"abc123abc123","base":"main"}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(err.to_string().contains("merged_at"));
    }

    #[test]
    fn advanced_main_blank_merged_branch_rejected() {
        let json = r#"{"type":"agent.advanced-main","from":"supervisor","merged_branch":"   ","new_main_sha":"abc123abc123","base":"main","merged_at":"2026-06-04T13:30:00Z"}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyMergedBranch));
    }

    #[test]
    fn advanced_main_blank_new_main_sha_rejected() {
        let json = r#"{"type":"agent.advanced-main","from":"supervisor","merged_branch":"feat/x","new_main_sha":"","base":"main","merged_at":"2026-06-04T13:30:00Z"}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyNewMainSha));
    }

    #[test]
    fn advanced_main_blank_base_rejected() {
        let json = r#"{"type":"agent.advanced-main","from":"supervisor","merged_branch":"feat/x","new_main_sha":"abc123abc123","base":"  ","merged_at":"2026-06-04T13:30:00Z"}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyBase));
    }

    #[test]
    fn advanced_main_blank_from_rejected() {
        // `from` is the message's agent_id() -> caught by the empty-id guard.
        let json = r#"{"type":"agent.advanced-main","from":"   ","merged_branch":"feat/x","new_main_sha":"abc123abc123","base":"main","merged_at":"2026-06-04T13:30:00Z"}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyAgentId));
    }

    #[test]
    fn learning_round_trips_through_serde() {
        let msg = make_learning(
            "abc123def456abcd",
            "supervisor",
            Some("feat/x"),
            "stuck_duration",
            "feat-x blocked 11m12s waiting on feat-y",
            serde_json::json!({
                "agent_id": "feat-x",
                "blocked_on": "feat-y",
                "duration_seconds": 672,
                "resolved": true
            }),
        );
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"agent.learning\""));
        let back: BrokerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, msg);
        assert_eq!(back.agent_id(), "supervisor");
        assert_eq!(back.status_label(), "learning");
    }

    #[test]
    fn learning_omits_branch_id_when_none() {
        let msg = make_learning(
            "abc123def456abcd",
            "supervisor",
            None,
            "permission_pattern",
            "`cargo check` auto-approved 23 times",
            serde_json::json!({"command_class": "cargo check", "count": 23}),
        );
        let json = serde_json::to_string(&msg).unwrap();
        assert!(
            !json.contains("branch_id"),
            "branch_id must be omitted when None; got {json}"
        );
        let back: BrokerMessage = serde_json::from_str(&json).unwrap();
        if let BrokerMessage::Learning { payload } = back {
            assert_eq!(payload.branch_id, None);
        } else {
            panic!("expected Learning");
        }
    }

    #[test]
    fn learning_missing_category_rejected_as_deserialize_error() {
        // `category` is a required serde field — its absence is a 400-class
        // deserialize error that names the missing field.
        let json = r#"{"type":"agent.learning","payload":{"id":"x","agent_id":"supervisor","title":"t","body":{},"timestamp":"2026-05-28T12:01:01Z"}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::Deserialize(_)), "got {err:?}");
        assert!(err.to_string().contains("category"));
    }

    #[test]
    fn learning_missing_body_rejected_as_deserialize_error() {
        let json = r#"{"type":"agent.learning","payload":{"id":"x","agent_id":"supervisor","category":"stuck_duration","title":"t","timestamp":"2026-05-28T12:01:01Z"}}"#;
        let err = BrokerMessage::from_json(json).unwrap_err();
        assert!(matches!(err, MessageError::Deserialize(_)), "got {err:?}");
        assert!(err.to_string().contains("body"));
    }

    #[test]
    fn learning_empty_category_rejected() {
        let msg = make_learning("x", "supervisor", None, "  ", "t", serde_json::json!({}));
        let json = serde_json::to_string(&msg).unwrap();
        let err = BrokerMessage::from_json(&json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyCategory));
    }

    #[test]
    fn learning_empty_title_rejected() {
        let msg = make_learning(
            "x",
            "supervisor",
            None,
            "stuck_duration",
            "",
            serde_json::json!({}),
        );
        let json = serde_json::to_string(&msg).unwrap();
        let err = BrokerMessage::from_json(&json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyTitle));
    }

    #[test]
    fn learning_empty_timestamp_rejected() {
        let msg = BrokerMessage::Learning {
            payload: LearningPayload {
                id: "x".to_string(),
                agent_id: "supervisor".to_string(),
                branch_id: None,
                category: "stuck_duration".to_string(),
                title: "t".to_string(),
                body: serde_json::json!({}),
                timestamp: "   ".to_string(),
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let err = BrokerMessage::from_json(&json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyTimestamp));
    }

    #[test]
    fn learning_empty_agent_id_rejected() {
        let msg = make_learning(
            "x",
            "",
            Some("feat/x"),
            "stuck_duration",
            "t",
            serde_json::json!({}),
        );
        let json = serde_json::to_string(&msg).unwrap();
        let err = BrokerMessage::from_json(&json).unwrap_err();
        assert!(matches!(err, MessageError::EmptyAgentId));
    }

    #[test]
    fn advanced_main_agent_id_is_from_field() {
        let msg = BrokerMessage::from_json(sample_advanced_main_json()).unwrap();
        assert_eq!(msg.agent_id(), "supervisor");
        assert_eq!(msg.status_label(), "advanced-main");
    }

    #[test]
    fn advanced_main_display_is_single_line() {
        let msg = BrokerMessage::from_json(sample_advanced_main_json()).unwrap();
        let s = msg.to_string();
        assert_eq!(
            s,
            "[supervisor] advanced-main: feat/auth \u{2192} main (a1b2c3d4e5f6)"
        );
        assert!(!s.contains('\n'));
        assert!(!s.contains('\u{1b}'));
    }

    // --- Deterministic id (advanced-main-event §2) ---

    fn ts(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .expect("valid rfc3339")
            .with_timezone(&Utc)
    }

    #[test]
    fn advanced_main_id_is_16_hex_chars() {
        let id = advanced_main_id("feat/x", "abc123abc123", "main", ts("2026-06-04T13:30:00Z"));
        assert_eq!(id.len(), 16, "id must be a 16-hex-char (64-bit) prefix");
        assert!(
            id.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "id must be lowercase hex; got {id}"
        );
    }

    #[test]
    fn advanced_main_id_same_input_same_hour_is_identical() {
        let a = advanced_main_id("feat/x", "abc123abc123", "main", ts("2026-06-04T13:00:00Z"));
        let b = advanced_main_id("feat/x", "abc123abc123", "main", ts("2026-06-04T13:59:59Z"));
        assert_eq!(
            a, b,
            "same merge within the same UTC hour must dedup to one id"
        );
    }

    #[test]
    fn advanced_main_id_differs_across_hour_boundary() {
        let a = advanced_main_id("feat/x", "abc123abc123", "main", ts("2026-06-04T13:59:59Z"));
        let b = advanced_main_id("feat/x", "abc123abc123", "main", ts("2026-06-04T14:00:00Z"));
        assert_ne!(
            a, b,
            "the same merge across an hour boundary must produce different ids"
        );
    }

    #[test]
    fn advanced_main_id_differs_for_different_shas() {
        let a = advanced_main_id("feat/x", "aaaaaaaaaaaa", "main", ts("2026-06-04T13:30:00Z"));
        let b = advanced_main_id("feat/x", "bbbbbbbbbbbb", "main", ts("2026-06-04T13:30:00Z"));
        assert_ne!(a, b, "different SHAs must produce different ids");
    }

    #[test]
    fn advanced_main_id_differs_for_different_base() {
        let a = advanced_main_id("feat/x", "abc123abc123", "main", ts("2026-06-04T13:30:00Z"));
        let b = advanced_main_id(
            "feat/x",
            "abc123abc123",
            "release",
            ts("2026-06-04T13:30:00Z"),
        );
        assert_ne!(a, b, "different base branches must produce different ids");
    }

    #[test]
    fn advanced_main_payload_deterministic_id_matches_free_fn() {
        let payload = AdvancedMainPayload {
            from: "supervisor".to_string(),
            merged_branch: "feat/x".to_string(),
            new_main_sha: "abc123abc123".to_string(),
            base: "main".to_string(),
            merged_at: ts("2026-06-04T13:30:00Z"),
            summary: None,
        };
        assert_eq!(
            payload.deterministic_id(),
            advanced_main_id("feat/x", "abc123abc123", "main", ts("2026-06-04T13:30:00Z")),
        );
    }

    #[test]
    fn learning_accepts_unknown_category_open_enum() {
        // A descendant change ([[qualitative-learnings]]) adds new category
        // values; the broker must accept them without an enum check.
        let msg = make_learning(
            "x",
            "supervisor",
            Some("feat/x"),
            "qualitative_insight",
            "agent kept re-reading the same file",
            serde_json::json!({"note": "thrash"}),
        );
        let json = serde_json::to_string(&msg).unwrap();
        let back = BrokerMessage::from_json(&json).expect("unknown category must be accepted");
        assert_eq!(back.agent_id(), "supervisor");
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

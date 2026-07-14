//! Axum HTTP server for the broker.
//!
//! Defines the router and endpoint handlers for `/publish`, `/messages/:agent_id`,
//! and `/status`. All handlers follow the lock discipline documented in
//! [`super`] — no `RwLock` guard is held across an `.await` boundary.

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::BrokerState;
use super::delivery;
use super::messages::BrokerMessage;
use super::{WatchTarget, watcher};

/// Compiled-once regex matching the only `agent_id` shapes the broker accepts:
/// `"supervisor"`, or a `{prefix}/{name}` / `{prefix}-{name}` slug whose two
/// segments each begin with `[a-z0-9]` and consist of `[a-z0-9-]`. See
/// `supervisor-bugfixes-v0-5-x` §4 + `broker-messages` spec.
///
/// The prefix is deliberately *not* restricted to `feat`. git-paw creates
/// worktrees on whatever branch it is given — `branch_prefix` is user-
/// configurable (documented default `"spec/"`), and the shipped config example
/// advertises `branches = ["feat/api", "fix/db"]` — so pinning the broker to
/// `feat` left every `fix/`, `spec/`, `chore/`… agent unable to register,
/// report, ask, or block, with no way to reach the supervisor to say so.
///
/// The two-segment shape is still enforced: it is what keeps a bare word (a
/// typo, or an unsubstituted `<agent-id>` label) from being accepted as an id.
fn agent_id_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(supervisor|[a-z0-9][a-z0-9-]*[/-][a-z0-9][a-z0-9-]*)$")
            .expect("AGENT_ID_RE compiles")
    })
}

/// Compiled-once regex matching unfilled placeholder strings — exact match
/// `<anything>` from start to end.
fn placeholder_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^<.*>$").expect("PLACEHOLDER_RE compiles"))
}

/// Build the HTTP 400 response for an `agent_id` that did not match
/// [`agent_id_regex`].
fn agent_id_rejection(value: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        axum::Json(serde_json::json!({
            "error": "invalid agent_id",
            "value": value,
            "detail": "agent_id must be 'supervisor' or a {prefix}/{name} / {prefix}-{name} slug (lowercase, e.g. feat/add-auth, fix-db-timeout)",
        })),
    )
        .into_response()
}

/// Build the HTTP 400 response for a payload string that looks like an
/// unfilled placeholder (`<…>`).
fn placeholder_rejection(field: &str, value: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        axum::Json(serde_json::json!({
            "error": "field looks like an unfilled placeholder",
            "field": field,
            "value": value,
            "detail": "substitute the real value before publishing",
        })),
    )
        .into_response()
}

/// Returns `Some(response)` if any tracked payload string field of `msg`
/// matches [`placeholder_regex`]; otherwise `None`.
///
/// Per `supervisor-bugfixes-v0-5-x` design D5, the placeholder check covers
/// only the fields the supervisor skill's example curls populate:
/// `payload.question`, `payload.needs`, and each string element of
/// `payload.errors[]`. Other free-form string fields (`StatusPayload.message`,
/// `VerifiedPayload.message`) are left alone — real human content sometimes
/// uses angle brackets inline.
fn check_placeholder_fields(msg: &BrokerMessage) -> Option<Response> {
    let re = placeholder_regex();
    match msg {
        BrokerMessage::Question { payload, .. } => {
            if re.is_match(&payload.question) {
                return Some(placeholder_rejection("question", &payload.question));
            }
        }
        BrokerMessage::Blocked { payload, .. } => {
            if re.is_match(&payload.needs) {
                return Some(placeholder_rejection("needs", &payload.needs));
            }
        }
        BrokerMessage::Feedback { payload, .. } => {
            for err in &payload.errors {
                if re.is_match(err) {
                    return Some(placeholder_rejection("errors", err));
                }
            }
        }
        BrokerMessage::Status { .. }
        | BrokerMessage::Artifact { .. }
        | BrokerMessage::Verified { .. }
        | BrokerMessage::Intent { .. }
        | BrokerMessage::AdvancedMain { .. }
        | BrokerMessage::Learning { .. }
        | BrokerMessage::VerifyNow { .. } => {}
    }
    None
}

/// Request body for the `POST /watch` endpoint — a live filesystem watch
/// target to register on the running broker.
#[derive(Deserialize)]
struct WatchRequest {
    /// Agent identifier (slugified branch name) that owns the worktree.
    agent_id: String,
    /// Absolute path to the worktree root to begin watching.
    worktree_path: String,
    /// CLI label running in the agent's pane (e.g. `"claude"`). Optional —
    /// an absent or empty value leaves the roster's CLI column unseeded,
    /// matching the start-time behaviour for a blank CLI.
    #[serde(default)]
    cli: String,
}

/// Query parameters for the `GET /messages/:agent_id` endpoint.
#[derive(Deserialize)]
struct PollQuery {
    /// Return only messages with sequence number > `since`. Defaults to 0.
    since: Option<String>,
}

/// Response body for the `GET /messages/:agent_id` endpoint.
#[derive(Serialize)]
struct PollResponse {
    /// Messages newer than the requested cursor.
    messages: Vec<BrokerMessage>,
    /// Highest sequence number in the result (0 if empty).
    last_seq: u64,
}

/// Response body for the `GET /log` endpoint.
#[derive(Serialize)]
struct LogResponse {
    /// All messages with `seq > since`, in chronological order.
    /// Each entry is `[seq, timestamp_unix_secs, message]`.
    entries: Vec<LogEntry>,
    /// Highest sequence number in the result (0 if empty).
    last_seq: u64,
}

/// One entry in `GET /log`.
#[derive(Serialize)]
struct LogEntry {
    /// Sequence number assigned at publish time.
    seq: u64,
    /// Wall-clock seconds since the Unix epoch when the message was published.
    timestamp_unix_secs: u64,
    /// The original broker message.
    message: BrokerMessage,
}

/// Builds the axum [`Router`] with all broker endpoints.
pub fn router(state: Arc<BrokerState>) -> Router {
    Router::new()
        .route("/publish", post(publish))
        .route("/watch", post(watch))
        .route("/messages/{agent_id}", get(messages))
        .route("/status", get(status))
        .route("/log", get(log))
        .with_state(state)
}

/// `POST /publish` — accepts a JSON [`BrokerMessage`] and queues it for delivery.
///
/// - 415 if `Content-Type` is missing or not `application/json`
/// - 400 if body is empty or fails validation
/// - 202 on success
async fn publish(
    State(state): State<Arc<BrokerState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    // Check Content-Type
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !content_type.starts_with("application/json") {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            axum::Json(serde_json::json!({"error": "Content-Type must be application/json"})),
        )
            .into_response();
    }

    // Check for empty body
    if body.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({"error": "request body must not be empty"})),
        )
            .into_response();
    }

    // Parse and validate
    match BrokerMessage::from_json(&body) {
        Ok(msg) => {
            // Validate the top-level agent_id against the broker regex.
            // Phantom debris (`"a"`, `"<agent-id>"`, empty strings) is
            // rejected at the API boundary so it cannot leak into
            // `/status`.
            if !agent_id_regex().is_match(msg.agent_id()) {
                return agent_id_rejection(msg.agent_id());
            }
            // Reject obviously-unfilled placeholder strings in the few
            // payload fields the supervisor skill's examples touch.
            if let Some(rejection) = check_placeholder_fields(&msg) {
                return rejection;
            }
            delivery::publish_message(&state, &msg);
            StatusCode::ACCEPTED.into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// `POST /watch` — registers a live filesystem watch target on the running
/// broker so the watcher begins surfacing the worktree's activity without a
/// restart.
///
/// Bound to loopback only, on the same listener as `/publish` and `/status`.
///
/// - 415 if `Content-Type` is missing or not `application/json`
/// - 400 if the body is empty, malformed, has an invalid `agent_id`, or an
///   empty / placeholder `worktree_path`
/// - 202 on success (including an idempotent re-registration, which records
///   nothing new and spawns no watcher)
async fn watch(
    State(state): State<Arc<BrokerState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !content_type.starts_with("application/json") {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            axum::Json(serde_json::json!({"error": "Content-Type must be application/json"})),
        )
            .into_response();
    }
    if body.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({"error": "request body must not be empty"})),
        )
            .into_response();
    }

    let req: WatchRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                axum::Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    // Validate agent_id against the same regex `/publish` enforces, so
    // phantom debris cannot mint a watch target.
    if !agent_id_regex().is_match(&req.agent_id) {
        return agent_id_rejection(&req.agent_id);
    }
    // The worktree path must be present and not an unfilled placeholder.
    if req.worktree_path.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({"error": "worktree_path must not be empty"})),
        )
            .into_response();
    }
    if placeholder_regex().is_match(&req.worktree_path) {
        return placeholder_rejection("worktree_path", &req.worktree_path);
    }

    let target = WatchTarget {
        agent_id: req.agent_id,
        cli: req.cli,
        worktree_path: std::path::PathBuf::from(req.worktree_path),
    };

    // Record the target (idempotent). Only spawn a watcher for a freshly
    // registered path, and only when the broker has a live shutdown signal to
    // enroll it in (absent in router-only unit tests).
    if state.register_watch_target(&target)
        && let Some(rx) = state.watcher_shutdown_rx()
    {
        tokio::spawn(watcher::watch_worktree(Arc::clone(&state), target, rx));
    }

    StatusCode::ACCEPTED.into_response()
}

/// `GET /messages/:agent_id?since=N` — polls for messages destined to the given agent.
///
/// - 400 if `agent_id` contains invalid characters
/// - 400 if `since` is present but not a valid `u64`
/// - 200 with `{"messages": [...], "last_seq": N}` on success
async fn messages(
    State(state): State<Arc<BrokerState>>,
    Path(agent_id): Path<String>,
    Query(params): Query<PollQuery>,
) -> Response {
    // Validate agent_id: only lowercase alphanumeric, hyphens, underscores
    if agent_id.is_empty()
        || !agent_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({"error": "agent_id must match [a-z0-9-_]+"})),
        )
            .into_response();
    }

    let since = match params.since {
        Some(s) => match s.parse::<u64>() {
            Ok(n) => n,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    axum::Json(serde_json::json!({"error": "since must be a valid u64"})),
                )
                    .into_response();
            }
        },
        None => 0,
    };

    let (msgs, last_seq) = delivery::poll_messages(&state, &agent_id, since);
    (
        StatusCode::OK,
        axum::Json(PollResponse {
            messages: msgs,
            last_seq,
        }),
    )
        .into_response()
}

/// `GET /log?since=N` — returns the broker's full message log filtered to
/// `seq > since`.
///
/// Used by `cmd_supervisor` to reconstruct broker state from outside the
/// dashboard process so it can build the dependency graph for merge ordering
/// and write a real session summary instead of an empty one.
async fn log(State(state): State<Arc<BrokerState>>, Query(params): Query<PollQuery>) -> Response {
    let since = match params.since {
        Some(s) => match s.parse::<u64>() {
            Ok(n) => n,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    axum::Json(serde_json::json!({"error": "since must be a valid u64"})),
                )
                    .into_response();
            }
        },
        None => 0,
    };

    let raw = delivery::full_log(&state, since);
    let last_seq = raw.iter().map(|(s, _, _)| *s).max().unwrap_or(0);
    let entries: Vec<LogEntry> = raw
        .into_iter()
        .map(|(seq, ts, message)| LogEntry {
            seq,
            timestamp_unix_secs: ts
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            message,
        })
        .collect();

    (
        StatusCode::OK,
        axum::Json(LogResponse { entries, last_seq }),
    )
        .into_response()
}

/// `GET /status` — returns broker health and agent summary.
async fn status(State(state): State<Arc<BrokerState>>) -> Response {
    let uptime = state.uptime_seconds();
    let agents = delivery::agent_status_snapshot(&state);
    (
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "git_paw": true,
            "version": env!("CARGO_PKG_VERSION"),
            "uptime_seconds": uptime,
            "agents": agents,
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_router() -> Router {
        router(Arc::new(BrokerState::new(None)))
    }

    #[tokio::test]
    async fn publish_valid_message_returns_202() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/publish")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"type":"agent.status","agent_id":"feat-xx","payload":{"status":"idle","modified_files":[]}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn publish_invalid_json_returns_400() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/publish")
                    .header("content-type", "application/json")
                    .body(Body::from("not json"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn publish_empty_body_returns_400() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/publish")
                    .header("content-type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn publish_wrong_content_type_returns_415() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/publish")
                    .header("content-type", "text/plain")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn publish_missing_content_type_returns_415() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/publish")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn publish_empty_agent_id_returns_400() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/publish")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"type":"agent.status","agent_id":"","payload":{"status":"idle","modified_files":[]}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // -----------------------------------------------------------------------
    // supervisor-bugfixes-v0-5-x §4 — broker validates agent_id + payload
    // placeholder syntax. The unit-test matrix below covers the spec scenarios
    // for invalid + valid agent_ids and the placeholder rejection rules.
    // -----------------------------------------------------------------------

    /// Helper: POST a body to `/publish` and return (status, body-bytes).
    async fn post_publish(body: &'static str) -> (StatusCode, axum::body::Bytes) {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/publish")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, bytes)
    }

    #[tokio::test]
    async fn agent_id_rejects_single_letter() {
        let (status, body) = post_publish(
            r#"{"type":"agent.status","agent_id":"a","payload":{"status":"working","modified_files":[]}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let text = String::from_utf8_lossy(&body);
        assert!(
            text.contains("invalid agent_id"),
            "body should mention 'invalid agent_id'; got: {text}"
        );
    }

    #[tokio::test]
    async fn agent_id_rejects_placeholder() {
        let (status, body) = post_publish(
            r#"{"type":"agent.status","agent_id":"<agent-id>","payload":{"status":"working","modified_files":[]}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("invalid agent_id"), "body: {text}");
    }

    #[tokio::test]
    async fn agent_id_rejects_empty() {
        let (status, body) = post_publish(
            r#"{"type":"agent.status","agent_id":"","payload":{"status":"working","modified_files":[]}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // The empty-string case is caught either by from_json's
        // EmptyAgentId validation or by the regex — both surface as 400.
        let _ = body;
    }

    #[tokio::test]
    async fn agent_id_accepts_supervisor() {
        let (status, _) = post_publish(
            r#"{"type":"agent.status","agent_id":"supervisor","payload":{"status":"working","modified_files":[]}}"#,
        )
        .await;
        assert!(
            status == StatusCode::ACCEPTED || status == StatusCode::OK,
            "supervisor should be accepted; got: {status}"
        );
    }

    #[tokio::test]
    async fn agent_id_accepts_feat_dash() {
        let (status, _) = post_publish(
            r#"{"type":"agent.status","agent_id":"feat-test-branch","payload":{"status":"working","modified_files":[]}}"#,
        )
        .await;
        assert!(
            status == StatusCode::ACCEPTED || status == StatusCode::OK,
            "feat-test-branch should be accepted; got: {status}"
        );
    }

    #[tokio::test]
    async fn agent_id_accepts_feat_slash() {
        let (status, _) = post_publish(
            r#"{"type":"agent.status","agent_id":"feat/test-branch","payload":{"status":"working","modified_files":[]}}"#,
        )
        .await;
        assert!(
            status == StatusCode::ACCEPTED || status == StatusCode::OK,
            "feat/test-branch should be accepted; got: {status}"
        );
    }

    /// A `fix/` agent must be able to publish. git-paw itself creates worktrees
    /// on any branch — its own config documents `branches = ["feat/api", "fix/db"]`
    /// as a valid preset — so a broker that only accepts `feat` leaves every
    /// non-feat agent unable to register, report, ask, or block.
    #[tokio::test]
    async fn agent_id_accepts_fix_slash() {
        let (status, _) = post_publish(
            r#"{"type":"agent.status","agent_id":"fix/olx-auth-error-mapping","payload":{"status":"working","modified_files":[]}}"#,
        )
        .await;
        assert!(
            status == StatusCode::ACCEPTED || status == StatusCode::OK,
            "fix/olx-auth-error-mapping should be accepted; got: {status}"
        );
    }

    #[tokio::test]
    async fn agent_id_accepts_fix_dash() {
        let (status, _) = post_publish(
            r#"{"type":"agent.status","agent_id":"fix-olx-auth-error-mapping","payload":{"status":"working","modified_files":[]}}"#,
        )
        .await;
        assert!(
            status == StatusCode::ACCEPTED || status == StatusCode::OK,
            "fix-olx-auth-error-mapping should be accepted; got: {status}"
        );
    }

    /// `branch_prefix` is user-configurable and documented with a default of
    /// `"spec/"`, so spec-derived branches must be accepted too — otherwise the
    /// broker rejects the very branches git-paw creates by default.
    #[tokio::test]
    async fn agent_id_accepts_configured_branch_prefix() {
        for id in ["spec/add-auth", "chore-bump-deps", "hotfix/prod-outage"] {
            let body = format!(
                r#"{{"type":"agent.status","agent_id":"{id}","payload":{{"status":"working","modified_files":[]}}}}"#
            );
            let app = test_router();
            let resp = app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/publish")
                        .header("content-type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert!(
                resp.status() == StatusCode::ACCEPTED || resp.status() == StatusCode::OK,
                "{id} should be accepted; got: {}",
                resp.status()
            );
        }
    }

    /// Widening the prefix must not degrade into "accept anything": the id still
    /// has to be a `{prefix}{/ or -}{name}` slug, so a bare word with no
    /// separator (a typo, or an unsubstituted label) is still rejected.
    #[tokio::test]
    async fn agent_id_rejects_prefixless_word() {
        let (status, body) = post_publish(
            r#"{"type":"agent.status","agent_id":"agent","payload":{"status":"working","modified_files":[]}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("invalid agent_id"), "body: {text}");
    }

    #[tokio::test]
    async fn payload_question_rejects_placeholder() {
        let (status, body) = post_publish(
            r#"{"type":"agent.question","agent_id":"feat-test-branch","payload":{"question":"<your specific question>"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let text = String::from_utf8_lossy(&body);
        assert!(
            text.contains("placeholder") && text.contains("question"),
            "body should mention both 'placeholder' and 'question'; got: {text}"
        );
    }

    #[tokio::test]
    async fn payload_question_accepts_real_content() {
        let (status, _) = post_publish(
            r#"{"type":"agent.question","agent_id":"feat-test-branch","payload":{"question":"Should we use bcrypt or argon2?"}}"#,
        )
        .await;
        assert!(
            status == StatusCode::ACCEPTED || status == StatusCode::OK,
            "real human content should be accepted; got: {status}"
        );
    }

    #[tokio::test]
    async fn payload_blocked_rejects_placeholder_needs() {
        let (status, body) = post_publish(
            r#"{"type":"agent.blocked","agent_id":"feat-test-branch","payload":{"needs":"<what>","from":"feat-other"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let text = String::from_utf8_lossy(&body);
        assert!(
            text.contains("placeholder") && text.contains("needs"),
            "body: {text}"
        );
    }

    #[tokio::test]
    async fn payload_feedback_rejects_placeholder_error_entry() {
        let (status, body) = post_publish(
            r#"{"type":"agent.feedback","agent_id":"feat-test-branch","payload":{"from":"supervisor","errors":["<error 1>"]}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let text = String::from_utf8_lossy(&body);
        assert!(
            text.contains("placeholder") && text.contains("errors"),
            "body: {text}"
        );
    }

    // === agent.advanced-main routing + validation (advanced-main-event §3) ===

    #[tokio::test]
    async fn advanced_main_accepted_through_publish_endpoint() {
        // No new endpoint: the variant flows through the existing /publish.
        let (status, _) = post_publish(
            r#"{"type":"agent.advanced-main","from":"supervisor","merged_branch":"feat/auth","new_main_sha":"a1b2c3d4e5f6","base":"main","merged_at":"2026-06-04T13:30:00Z","summary":"landed auth"}"#,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::ACCEPTED,
            "a well-formed advanced-main must be accepted (202)"
        );
    }

    #[tokio::test]
    async fn advanced_main_missing_field_returns_400_naming_field() {
        let (status, body) = post_publish(
            r#"{"type":"agent.advanced-main","from":"supervisor","new_main_sha":"a1b2c3d4e5f6","base":"main","merged_at":"2026-06-04T13:30:00Z"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let text = String::from_utf8_lossy(&body);
        assert!(
            text.contains("merged_branch"),
            "the 400 must name the missing field; got: {text}"
        );
    }

    #[tokio::test]
    async fn advanced_main_routes_to_every_registered_agent() {
        // After two agents register, a supervisor-published advance lands in
        // both their inboxes within one poll.
        let state = Arc::new(BrokerState::new(None));
        publish_json(
            &state,
            r#"{"type":"agent.status","agent_id":"feat-alpha","payload":{"status":"working","modified_files":[]}}"#,
        )
        .await;
        publish_json(
            &state,
            r#"{"type":"agent.status","agent_id":"feat-beta","payload":{"status":"working","modified_files":[]}}"#,
        )
        .await;
        publish_json(
            &state,
            r#"{"type":"agent.advanced-main","from":"supervisor","merged_branch":"feat/alpha","new_main_sha":"a1b2c3d4e5f6","base":"main","merged_at":"2026-06-04T13:30:00Z"}"#,
        )
        .await;

        for agent in ["feat-alpha", "feat-beta"] {
            let (msgs, _) = delivery::poll_messages(&state, agent, 0);
            assert!(
                msgs.iter()
                    .any(|m| matches!(m, BrokerMessage::AdvancedMain { .. })),
                "{agent} inbox must surface the advanced-main event"
            );
        }
    }

    #[tokio::test]
    async fn messages_valid_agent_returns_200_with_last_seq() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/messages/feat-x")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["messages"], serde_json::json!([]));
        assert_eq!(json["last_seq"], serde_json::json!(0));
    }

    #[tokio::test]
    async fn messages_invalid_agent_returns_400() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/messages/INVALID!")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn messages_invalid_since_returns_400() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/messages/feat-x?since=abc")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn status_returns_marker_and_version() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["git_paw"], true);
        assert!(json["version"].is_string());
        assert!(json["uptime_seconds"].is_number());
        assert_eq!(json["agents"], serde_json::json!([]));
    }

    /// POSTs a JSON body to `/publish` against a router built from `state`.
    async fn publish_json(state: &Arc<BrokerState>, body: &'static str) {
        let resp = router(Arc::clone(state))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/publish")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
    }

    /// GETs `/status` against a router built from `state` and returns the
    /// parsed JSON body.
    async fn get_status(state: &Arc<BrokerState>) -> serde_json::Value {
        let resp = router(Arc::clone(state))
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn e2e_feedback_from_human_creates_no_phantom_roster_row() {
        // W15-16 end-to-end: two real agents register via `agent.status`,
        // then a `agent.feedback` with `from:"human"` is published. The
        // `/status` roster must hold exactly the two real agents — no
        // phantom `"human"` row.
        let state = Arc::new(BrokerState::new(None));
        publish_json(
            &state,
            r#"{"type":"agent.status","agent_id":"supervisor","payload":{"status":"working","modified_files":[],"cli":"claude-oss"}}"#,
        )
        .await;
        publish_json(
            &state,
            r#"{"type":"agent.status","agent_id":"feat-roster","payload":{"status":"working","modified_files":[],"cli":"claude-oss"}}"#,
        )
        .await;
        publish_json(
            &state,
            r#"{"type":"agent.feedback","agent_id":"feat-roster","payload":{"from":"human","errors":["fix the flaky test"]}}"#,
        )
        .await;

        let json = get_status(&state).await;
        let agents = json["agents"].as_array().expect("agents array");
        let ids: Vec<&str> = agents
            .iter()
            .map(|a| a["agent_id"].as_str().unwrap())
            .collect();
        assert!(
            !ids.contains(&"human"),
            "a feedback `from:human` must not mint a phantom roster row; got {ids:?}",
        );
        assert_eq!(ids.len(), 2, "roster holds exactly the two real agents");
    }

    #[tokio::test]
    async fn e2e_status_shows_cli_for_every_agent() {
        // W15-15 end-to-end: every agent that publishes `agent.status` with a
        // `cli` shows that CLI in the `/status` roster — not just the
        // supervisor.
        let state = Arc::new(BrokerState::new(None));
        publish_json(
            &state,
            r#"{"type":"agent.status","agent_id":"supervisor","payload":{"status":"working","modified_files":[],"cli":"claude-oss"}}"#,
        )
        .await;
        publish_json(
            &state,
            r#"{"type":"agent.status","agent_id":"feat-build","payload":{"status":"working","modified_files":[],"cli":"claude-oss"}}"#,
        )
        .await;

        let json = get_status(&state).await;
        let agents = json["agents"].as_array().expect("agents array");
        assert_eq!(agents.len(), 2);
        for a in agents {
            assert_eq!(
                a["cli"].as_str(),
                Some("claude-oss"),
                "every agent row must carry its cli: {a}",
            );
        }
    }

    // -----------------------------------------------------------------------
    // broker-live-watch-registration — POST /watch
    // -----------------------------------------------------------------------

    /// Minimal git repo for the watch integration test.
    fn init_test_repo_server(dir: &std::path::Path) {
        use std::process::Command;
        let run = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(dir)
                .output()
                .expect("git command failed");
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "test"]);
        run(&["commit", "--allow-empty", "-m", "root", "-q"]);
    }

    /// POSTs a body to `/watch` against a router built from `state`.
    async fn post_watch(state: &Arc<BrokerState>, body: String) -> StatusCode {
        router(Arc::clone(state))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/watch")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }

    /// Spec scenario: registering a target surfaces the worktree via the
    /// watcher without a broker restart (tasks 2.3 + 3.3 at the broker layer).
    #[tokio::test]
    async fn watch_registers_target_and_surfaces_worktree_in_status() {
        use super::super::watcher::POLL_INTERVAL;
        let tmp = tempfile::tempdir().unwrap();
        init_test_repo_server(tmp.path());

        let state = Arc::new(BrokerState::new(None));
        // Wire the shared watcher shutdown signal the way start_broker_with
        // does, so the handler enrolls the spawned watcher in it.
        let (tx, rx) = tokio::sync::watch::channel(false);
        state.set_watcher_shutdown_rx(rx);

        let body = format!(
            r#"{{"agent_id":"feat-hot","worktree_path":"{}","cli":"claude"}}"#,
            tmp.path().display()
        );
        assert_eq!(post_watch(&state, body).await, StatusCode::ACCEPTED);

        // Dirty the worktree so the watcher has activity to surface.
        std::fs::write(tmp.path().join("hot.rs"), "fn hot() {}").unwrap();

        let mut found = false;
        for _ in 0..20 {
            tokio::time::sleep(POLL_INTERVAL / 2).await;
            let json = get_status(&state).await;
            if let Some(agents) = json["agents"].as_array()
                && agents.iter().any(|a| a["agent_id"] == "feat-hot")
            {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "a registered worktree must surface its agent in /status from activity"
        );

        let _ = tx.send(true);
    }

    /// Spec scenario: registration is idempotent — a duplicate POST still
    /// succeeds and records no second target.
    #[tokio::test]
    async fn watch_duplicate_registration_is_a_noop_success() {
        let state = Arc::new(BrokerState::new(None));
        let body = r#"{"agent_id":"feat-hot","worktree_path":"/tmp/feat-hot","cli":"claude"}"#;
        assert_eq!(
            post_watch(&state, body.to_string()).await,
            StatusCode::ACCEPTED
        );
        assert_eq!(
            post_watch(&state, body.to_string()).await,
            StatusCode::ACCEPTED
        );
        assert_eq!(
            state.read().watched_paths.len(),
            1,
            "duplicate registration must not record a second target"
        );
    }

    #[tokio::test]
    async fn watch_rejects_invalid_agent_id() {
        let state = Arc::new(BrokerState::new(None));
        let body = r#"{"agent_id":"a","worktree_path":"/tmp/x","cli":"claude"}"#;
        assert_eq!(
            post_watch(&state, body.to_string()).await,
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn watch_rejects_placeholder_worktree_path() {
        let state = Arc::new(BrokerState::new(None));
        let body = r#"{"agent_id":"feat-hot","worktree_path":"<path>","cli":"claude"}"#;
        assert_eq!(
            post_watch(&state, body.to_string()).await,
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn watch_rejects_empty_worktree_path() {
        let state = Arc::new(BrokerState::new(None));
        let body = r#"{"agent_id":"feat-hot","worktree_path":"","cli":"claude"}"#;
        assert_eq!(
            post_watch(&state, body.to_string()).await,
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn watch_wrong_content_type_returns_415() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/watch")
                    .header("content-type", "text/plain")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn unknown_route_returns_404() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/unknown/route")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn wrong_method_returns_405() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/publish")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn panic_in_handler_is_isolated() {
        // Verify that a panicking handler does not take down the server.
        let app = Router::new()
            .route(
                "/panic",
                get(|| async {
                    panic!("deliberate test panic");
                    #[allow(unreachable_code)]
                    StatusCode::OK.into_response()
                }),
            )
            .route("/status", get(status))
            .with_state(Arc::new(BrokerState::new(None)));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        let client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build_http();

        // Request to /panic — should not crash the server.
        let _panic_resp = client
            .request(
                Request::builder()
                    .method("GET")
                    .uri(format!("http://{addr}/panic"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await;

        // The panicking connection may return an error or a 500.
        // Either is acceptable — the key test is that /status still works after.

        let status_resp = client
            .request(
                Request::builder()
                    .method("GET")
                    .uri(format!("http://{addr}/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .expect("server should still be alive after a panic in another handler");

        assert_eq!(status_resp.status(), StatusCode::OK);

        server.abort();
    }

    #[tokio::test]
    async fn log_returns_full_message_log_in_chronological_order() {
        let state = Arc::new(BrokerState::new(None));
        // Seed the broker with three published messages.
        for (agent, status_label) in [
            ("feat-a", "working"),
            ("feat-b", "blocked"),
            ("feat-c", "done"),
        ] {
            let msg = BrokerMessage::Status {
                agent_id: agent.to_string(),
                payload: super::super::messages::StatusPayload {
                    status: status_label.to_string(),
                    modified_files: vec![],
                    message: None,
                    ..Default::default()
                },
            };
            delivery::publish_message(&state, &msg);
        }

        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/log")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let entries = parsed["entries"].as_array().expect("entries array");
        assert_eq!(entries.len(), 3, "all three messages must appear in /log");
        // Chronological order: feat-a first, feat-c last.
        assert_eq!(entries[0]["message"]["agent_id"], "feat-a");
        assert_eq!(entries[2]["message"]["agent_id"], "feat-c");
        assert_eq!(parsed["last_seq"], 3);
    }

    #[tokio::test]
    async fn log_with_since_filters_older_entries() {
        let state = Arc::new(BrokerState::new(None));
        for agent in ["feat-a", "feat-b", "feat-c"] {
            let msg = BrokerMessage::Status {
                agent_id: agent.to_string(),
                payload: super::super::messages::StatusPayload {
                    status: "working".to_string(),
                    modified_files: vec![],
                    message: None,
                    ..Default::default()
                },
            };
            delivery::publish_message(&state, &msg);
        }

        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/log?since=2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let entries = parsed["entries"].as_array().unwrap();
        assert_eq!(
            entries.len(),
            1,
            "since=2 must yield only the message at seq=3"
        );
        assert_eq!(entries[0]["seq"], 3);
    }

    #[tokio::test]
    async fn log_invalid_since_returns_400() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/log?since=notanumber")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}

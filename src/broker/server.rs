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

/// Compiled-once regex matching the only `agent_id` shapes the broker accepts:
/// `"supervisor"`, or a `feat-{name}` / `feat/{name}` slug whose `{name}`
/// begins with `[a-z0-9]` and consists of `[a-z0-9-]+`. See
/// `supervisor-bugfixes-v0-5-x` §4 + `broker-messages` spec.
fn agent_id_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(supervisor|feat/[a-z0-9][a-z0-9-]+|feat-[a-z0-9][a-z0-9-]+)$")
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
            "detail": "agent_id must be 'supervisor' or match feat-{name} / feat/{name}",
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
        | BrokerMessage::Intent { .. } => {}
    }
    None
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

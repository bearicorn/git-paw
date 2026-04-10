//! Axum HTTP server for the broker.
//!
//! Defines the router and endpoint handlers for `/publish`, `/messages/:agent_id`,
//! and `/status`. All handlers follow the lock discipline documented in
//! [`super`] — no `RwLock` guard is held across an `.await` boundary.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};

use super::BrokerState;
use super::delivery;
use super::messages::BrokerMessage;

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

/// Builds the axum [`Router`] with all broker endpoints.
pub fn router(state: Arc<BrokerState>) -> Router {
    Router::new()
        .route("/publish", post(publish))
        .route("/messages/{agent_id}", get(messages))
        .route("/status", get(status))
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
                        r#"{"type":"agent.status","agent_id":"feat-x","payload":{"status":"idle","modified_files":[]}}"#,
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
}

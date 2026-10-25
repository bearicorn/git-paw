//! Axum HTTP server for the broker.
//!
//! Defines the router and endpoint handlers for `/publish`, `/messages/:agent_id`,
//! and `/status`. All handlers follow the lock discipline documented in
//! [`super`] — no `RwLock` guard is held across an `.await` boundary.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

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
use super::origin;
use super::{WatchTarget, watcher};
use crate::static_regex::static_regex;

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
    RE.get_or_init(|| static_regex(r"^(supervisor|[a-z0-9][a-z0-9-]*[/-][a-z0-9][a-z0-9-]*)$"))
}

/// Returns whether `value` is an `agent_id` the `/publish` handler accepts.
///
/// The handler's own predicate, exposed so that callers deriving an id —
/// notably [`crate::broker::messages::slugify_branch`] and its tests — are
/// checked against the very regex that would reject them on the wire, rather
/// than against a second copy of the pattern that can drift from it. That
/// drift is exactly what let an `_` in a branch name become an id every
/// publish answered with HTTP 400 (PP-26).
#[must_use]
pub fn is_valid_agent_id(value: &str) -> bool {
    agent_id_regex().is_match(value)
}

/// Compiled-once regex matching unfilled placeholder strings — exact match
/// `<anything>` from start to end.
fn placeholder_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| static_regex(r"^<.*>$"))
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
/// `payload.question`, `payload.needs`, each string element of
/// `payload.errors[]`, and `payload.answer` / `payload.re`. Other free-form
/// string fields (`StatusPayload.message`, `VerifiedPayload.message`) are
/// left alone — real human content sometimes uses angle brackets inline.
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
        BrokerMessage::Answer { payload, .. } => {
            if re.is_match(&payload.answer) {
                return Some(placeholder_rejection("answer", &payload.answer));
            }
            if let Some(re_field) = &payload.re
                && re.is_match(re_field)
            {
                return Some(placeholder_rejection("re", re_field));
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
    /// The spec the agent was launched from, when it was. Optional — a body
    /// without it registers the target exactly as before, and the agent stays
    /// unmapped for the learnings aggregator's intra-/cross-spec split.
    #[serde(default)]
    spec_id: Option<String>,
}

/// Longest spec id `POST /watch` accepts.
const MAX_SPEC_ID_LEN: usize = 128;

/// Whether `spec_id` has the shape of a spec id every supported format uses:
/// at most [`MAX_SPEC_ID_LEN`] characters of ASCII letters, digits, `.`, `_`,
/// `/` and `-`.
fn spec_id_is_valid(spec_id: &str) -> bool {
    spec_id.len() <= MAX_SPEC_ID_LEN
        && spec_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-'))
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

/// Query parameters for the `GET /log` endpoint.
#[derive(Deserialize)]
struct LogQuery {
    /// Return only entries with sequence number > `since`. Defaults to 0.
    since: Option<String>,
    /// Hold the request for up to this many whole seconds when no entry is
    /// newer than `since`. Absent means "answer immediately".
    wait: Option<String>,
}

/// Upper bound on a `GET /log` long-poll, in seconds. A client asking for
/// longer is served at the cap rather than refused, so no caller can pin a
/// handler task indefinitely.
const MAX_LOG_WAIT_SECONDS: u64 = 60;

/// Response body for the `GET /log` endpoint.
#[derive(Serialize)]
struct LogResponse {
    /// All messages with `seq > since`, in chronological order.
    /// Each entry is `[seq, timestamp_unix_secs, message]`.
    entries: Vec<LogEntry>,
    /// Highest sequence number in the result, or the request's `since` when
    /// the result is empty — a reader's cursor must never rewind.
    last_seq: u64,
    /// The running broker instance's epoch (its start time, Unix seconds).
    /// Sequence numbers restart at zero on every broker start, so a reader
    /// whose recorded epoch differs from this one must reset its cursor to
    /// `0` or it will silently skip the new broker's entries.
    epoch: u64,
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

/// Builds the axum [`Router`] with all broker endpoints, wrapped in the
/// browser-exposure guard.
///
/// `bind_is_loopback` comes from [`origin::is_loopback_bind`] applied to
/// `[broker] bind`; it decides whether the `Host` check runs (see
/// [`origin::decide_request_origin`]). The guard is a
/// [`axum::middleware::from_fn`] layer over the whole router rather than a
/// per-handler check, so it runs before routing — covering every route and the
/// `404`/`405` fallbacks alike, including any route added later.
pub fn router(state: Arc<BrokerState>, bind_is_loopback: bool) -> Router {
    Router::new()
        .route("/publish", post(publish))
        .route("/watch", post(watch))
        .route("/messages/{agent_id}", get(messages))
        .route("/status", get(status))
        .route("/log", get(log))
        .with_state(state)
        .layer(axum::middleware::from_fn(
            move |request: axum::extract::Request, next: axum::middleware::Next| async move {
                origin_guard(bind_is_loopback, request, next).await
            },
        ))
}

/// Reads one header as the guard sees it: absent, a single readable value, or
/// unusable because it was sent more than once or its bytes are not valid
/// UTF-8. Both unusable shapes are request-smuggling signals, so neither is
/// collapsed into "absent" or silently resolved to the first value.
fn header_view<'a>(
    headers: &'a HeaderMap,
    name: &axum::http::HeaderName,
) -> origin::HeaderView<'a> {
    let mut values = headers.get_all(name).iter();
    let Some(first) = values.next() else {
        return origin::HeaderView::Absent;
    };
    if values.next().is_some() {
        return origin::HeaderView::Unusable;
    }
    first
        .to_str()
        .map_or(origin::HeaderView::Unusable, origin::HeaderView::Value)
}

/// Middleware that refuses browser-shaped and foreign-`Host` requests with
/// `403` before they reach any route.
async fn origin_guard(
    bind_is_loopback: bool,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let headers = request.headers();
    let host = header_view(headers, &axum::http::header::HOST);
    let origin = header_view(headers, &axum::http::header::ORIGIN);
    // `Uri::authority` is populated only for an absolute-form request target
    // (`GET http://host/path`); an ordinary origin-form target leaves it None.
    let target_authority = request
        .uri()
        .authority()
        .map(axum::http::uri::Authority::as_str);

    if let Err(rejection) =
        origin::decide_request_origin(host, origin, target_authority, bind_is_loopback)
    {
        return (
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({ "error": rejection.error_message() })),
        )
            .into_response();
    }
    next.run(request).await
}

/// `POST /publish` — accepts a JSON [`BrokerMessage`] and queues it for delivery.
///
/// Delivery is synchronous and can block (the opsx role-gating guard shells out
/// to `git` for a `committed` artifact), so it runs on tokio's blocking pool
/// rather than on an async worker thread.
///
/// - 415 if `Content-Type` is missing or not `application/json`
/// - 400 if body is empty or fails validation
/// - 500 if the offloaded delivery task fails
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
            if !is_valid_agent_id(msg.agent_id()) {
                return agent_id_rejection(msg.agent_id());
            }
            // Reject obviously-unfilled placeholder strings in the few
            // payload fields the supervisor skill's examples touch.
            if let Some(rejection) = check_placeholder_fields(&msg) {
                return rejection;
            }
            // `publish_message` is synchronous and, on the role-gating path,
            // spawns a blocking `git` subprocess. Run it on tokio's blocking
            // pool so a burst of `committed` artifacts cannot occupy every
            // async worker thread and stall the rest of the HTTP surface. The
            // delivery path itself — lock scope, routing, and the guard it
            // invokes — is unchanged; only where it executes moves.
            if tokio::task::spawn_blocking(move || delivery::publish_message(&state, &msg))
                .await
                .is_err()
            {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(serde_json::json!({"error": "publish task failed"})),
                )
                    .into_response();
            }
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

    // The spec id ends up in the learnings file the orchestrator reads, so it
    // is held to the shape real spec ids have: short, one line, no markup.
    let spec_id = req.spec_id.as_deref().filter(|s| !s.is_empty());
    if let Some(spec_id) = spec_id
        && !spec_id_is_valid(spec_id)
    {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({
                "error": format!(
                    "spec_id must be at most {MAX_SPEC_ID_LEN} characters of [A-Za-z0-9._/-]"
                )
            })),
        )
            .into_response();
    }

    // Record the agent -> spec mapping before the target moves, so a forward
    // conflict involving this agent is classified against its spec.
    if let (Some(spec_id), Some(aggregator)) = (spec_id, state.learnings.as_ref())
        && let Ok(mut agg) = aggregator.lock()
    {
        agg.set_spec_id(&req.agent_id, spec_id);
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

/// Collects log entries newer than `since`, holding the request for up to
/// `wait` when there are none.
///
/// Registers interest in the publish notification *before* each re-check, so
/// an entry appended between the check and the await still wakes the waiter.
/// [`delivery::full_log`] takes and releases the state read lock inside a
/// single statement, so no guard is ever live across the `.await`
/// (`clippy::await_holding_lock`).
async fn await_log_entries(
    state: &Arc<BrokerState>,
    since: u64,
    wait: Duration,
) -> Vec<(u64, std::time::SystemTime, BrokerMessage)> {
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        let notified = state.publish_notified();
        tokio::pin!(notified);
        notified.as_mut().enable();

        let raw = delivery::full_log(state, since);
        if !raw.is_empty() {
            return raw;
        }
        if tokio::time::timeout_at(deadline, notified).await.is_err() {
            // The wait elapsed. One last read so an entry published in the
            // same instant as the timeout is still returned.
            return delivery::full_log(state, since);
        }
    }
}

/// `GET /log?since=N&wait=S` — returns the broker's message log filtered to
/// `seq > since`, optionally long-polling for up to `wait` seconds.
///
/// Used by `cmd_supervisor` to reconstruct broker state from outside the
/// dashboard process so it can build the dependency graph for merge ordering
/// and write a real session summary instead of an empty one, and by the
/// orchestrator read surface (`wait_for_events`, `sweep.sh events`, the drive
/// loop's message observer) to advance a cursor event-by-event.
///
/// With `wait` present and nothing newer than `since`, the request is held
/// until an entry is appended or the wait elapses, capped at
/// [`MAX_LOG_WAIT_SECONDS`]. Without `wait` the handler answers immediately,
/// exactly as before. In both cases an empty answer reports
/// `last_seq = since`, so a reader's cursor never rewinds.
async fn log(State(state): State<Arc<BrokerState>>, Query(params): Query<LogQuery>) -> Response {
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
    let wait = match params.wait {
        Some(w) => match w.parse::<u64>() {
            Ok(n) => Some(Duration::from_secs(n.min(MAX_LOG_WAIT_SECONDS))),
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    axum::Json(serde_json::json!({"error": "wait must be a valid u64"})),
                )
                    .into_response();
            }
        },
        None => None,
    };

    let raw = match wait {
        Some(w) => await_log_entries(&state, since, w).await,
        None => delivery::full_log(&state, since),
    };
    // An empty answer holds the caller's cursor at `since` rather than
    // rewinding it to 0.
    let last_seq = raw.iter().map(|(s, _, _)| *s).max().unwrap_or(since);
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
        axum::Json(LogResponse {
            entries,
            last_seq,
            epoch: state.epoch(),
        }),
    )
        .into_response()
}

/// `GET /status` — returns broker health and agent summary.
async fn status(State(state): State<Arc<BrokerState>>) -> Response {
    let uptime = state.uptime_seconds();
    let agents = delivery::agent_status_snapshot(&state);
    // `repo_root` is what the stale-broker probe compares against its own
    // repository before it will reattach, so a broker that was started
    // without a declared repository reports an empty string rather than
    // omitting the field — an absent owner is treated as unknown, never as
    // the caller's.
    let repo_root = state
        .repo_root
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    (
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "git_paw": true,
            "version": env!("CARGO_PKG_VERSION"),
            "uptime_seconds": uptime,
            "agents": agents,
            "repo_root": repo_root,
            // Wave-level progress, alongside the per-agent rows: `agents[]`
            // answers "is anyone alive", `wave` answers "has anything
            // moved" — the question a roster of repeated `working`
            // heartbeats cannot.
            "wave": state.wave_status(),
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
        router(Arc::new(BrokerState::new(None)), true)
    }

    /// `core-project-conventions`' "Every static pattern compiles under test":
    /// forces every static regex in this module to compile.
    #[test]
    fn static_regexes_compile() {
        assert!(agent_id_regex().is_match("feat/add-auth"));
        assert!(placeholder_regex().is_match("<agent-id>"));
    }

    /// A request builder pre-seeded with a loopback `Host`, as every real
    /// client sends. Tests that exercise the guard itself build their own
    /// requests with [`Request::builder`] instead.
    fn loopback_request() -> axum::http::request::Builder {
        Request::builder().header("host", "127.0.0.1:9119")
    }

    #[tokio::test]
    async fn publish_valid_message_returns_202() {
        let app = test_router();
        let resp = app
            .oneshot(
                loopback_request()
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
                loopback_request()
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
                loopback_request()
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
                loopback_request()
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
                loopback_request()
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
                loopback_request()
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
                loopback_request()
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
                    loopback_request()
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

    #[tokio::test]
    async fn payload_answer_rejects_placeholder() {
        let (status, body) = post_publish(
            r#"{"type":"agent.answer","agent_id":"feat-test-branch","payload":{"from":"supervisor","answer":"<your answer>"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let text = String::from_utf8_lossy(&body);
        assert!(
            text.contains("placeholder") && text.contains("answer"),
            "body should mention both 'placeholder' and 'answer'; got: {text}"
        );
    }

    #[tokio::test]
    async fn payload_answer_rejects_placeholder_re() {
        let (status, body) = post_publish(
            r#"{"type":"agent.answer","agent_id":"feat-test-branch","payload":{"from":"supervisor","answer":"use the helper","re":"<the question>"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let text = String::from_utf8_lossy(&body);
        assert!(
            text.contains("placeholder") && text.contains("re"),
            "body: {text}"
        );
    }

    #[tokio::test]
    async fn payload_answer_accepts_real_content() {
        let (status, _) = post_publish(
            r#"{"type":"agent.answer","agent_id":"feat-test-branch","payload":{"from":"supervisor","answer":"Use the existing helper; do not add a dependency","re":"add crate X?"}}"#,
        )
        .await;
        assert!(
            status == StatusCode::ACCEPTED || status == StatusCode::OK,
            "real human content should be accepted; got: {status}"
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
                loopback_request()
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
                loopback_request()
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
                loopback_request()
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
                loopback_request()
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
    async fn status_reports_the_repository_it_serves() {
        let repo = tempfile::tempdir().expect("tempdir");
        let state = Arc::new(BrokerState::new(None).with_repo_root(repo.path()));
        let resp = router(state, true)
            .oneshot(
                loopback_request()
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
        let canonical = repo.path().canonicalize().expect("canonicalize repo");
        assert_eq!(
            json["repo_root"],
            serde_json::json!(canonical.display().to_string()),
            "repo_root must be the broker's canonical repository path"
        );
    }

    /// POSTs a JSON body to `/publish` against a router built from `state`.
    async fn publish_json(state: &Arc<BrokerState>, body: &'static str) {
        let resp = router(Arc::clone(state), true)
            .oneshot(
                loopback_request()
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
        let resp = router(Arc::clone(state), true)
            .oneshot(
                loopback_request()
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
        router(Arc::clone(state), true)
            .oneshot(
                loopback_request()
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

    /// A broker with a learnings aggregator attached, writing under `dir`.
    fn state_with_learnings(dir: &std::path::Path) -> Arc<BrokerState> {
        let mut state = BrokerState::new(None);
        state.attach_learnings(Arc::new(std::sync::Mutex::new(
            crate::broker::learnings::LearningsAggregator::new(dir.join("session-learnings.md")),
        )));
        Arc::new(state)
    }

    /// Feeds the detector's forward conflict between `a` and `b` (both
    /// directions) to the aggregator, flushes, and returns the learnings file.
    fn classify_forward_conflict(state: &BrokerState, a: &str, b: &str) -> String {
        let aggregator = state.learnings.as_ref().unwrap();
        let mut agg = aggregator.lock().unwrap();
        for (from, other) in [(a, b), (b, a)] {
            agg.record_detector_message(&crate::broker::messages::BrokerMessage::Feedback {
                agent_id: from.to_string(),
                payload: crate::broker::messages::FeedbackPayload {
                    from: "conflict-detector".to_string(),
                    errors: vec![format!(
                        "[conflict-detector] forward conflict with {other} on src/main.rs"
                    )],
                },
            });
        }
        agg.flush().unwrap();
        std::fs::read_to_string(agg.file_path()).unwrap()
    }

    /// `broker-runtime` "A registration with a spec id maps the agent" and
    /// `supervisor-learnings` "Two agents launched from the same spec are
    /// classified intra-spec", driven through `POST /watch` — the path a
    /// fresh launch and `git paw add --from-spec` register agents by.
    #[tokio::test]
    async fn a_watch_registration_with_a_spec_id_maps_the_agent_intra_spec() {
        let tmp = tempfile::tempdir().unwrap();
        let state = state_with_learnings(tmp.path());
        for agent in ["feat-x", "feat-y"] {
            let body = format!(
                r#"{{"agent_id":"{agent}","worktree_path":"/tmp/{agent}","cli":"claude","spec_id":"003-user-list"}}"#
            );
            assert_eq!(post_watch(&state, body).await, StatusCode::ACCEPTED);
        }
        let md = classify_forward_conflict(&state, "feat-x", "feat-y");
        assert!(md.contains("forward-conflict-intra-spec"), "got:\n{md}");
        assert!(md.contains("003-user-list"), "got:\n{md}");
    }

    /// `supervisor-learnings` "Agents from different specs are classified
    /// cross-spec", from `/watch` registrations.
    #[tokio::test]
    async fn watch_registrations_from_different_specs_classify_cross_spec() {
        let tmp = tempfile::tempdir().unwrap();
        let state = state_with_learnings(tmp.path());
        for (agent, spec) in [
            ("feat-x", "003-user-list"),
            ("feat-y", "004-error-handling"),
        ] {
            let body = format!(
                r#"{{"agent_id":"{agent}","worktree_path":"/tmp/{agent}","cli":"claude","spec_id":"{spec}"}}"#
            );
            assert_eq!(post_watch(&state, body).await, StatusCode::ACCEPTED);
        }
        let md = classify_forward_conflict(&state, "feat-x", "feat-y");
        assert!(md.contains("forward-conflict-cross-spec"), "got:\n{md}");
        assert!(
            md.contains("003-user-list") && md.contains("004-error-handling"),
            "got:\n{md}"
        );
    }

    /// A `/watch` body without a spec id is accepted exactly as before and
    /// leaves the agent unmapped.
    #[tokio::test]
    async fn a_watch_registration_without_a_spec_id_leaves_the_agent_unmapped() {
        let tmp = tempfile::tempdir().unwrap();
        let state = state_with_learnings(tmp.path());
        for agent in ["feat-x", "feat-y"] {
            let body = format!(
                r#"{{"agent_id":"{agent}","worktree_path":"/tmp/{agent}","cli":"claude"}}"#
            );
            assert_eq!(post_watch(&state, body).await, StatusCode::ACCEPTED);
        }
        let md = classify_forward_conflict(&state, "feat-x", "feat-y");
        assert!(!md.contains("forward-conflict-intra-spec"), "got:\n{md}");
    }

    /// A spec id that is too long or carries newlines / markup is refused, so
    /// nothing but a spec-id-shaped string reaches the learnings file.
    #[tokio::test]
    async fn watch_rejects_a_spec_id_that_is_not_spec_id_shaped() {
        let tmp = tempfile::tempdir().unwrap();
        let state = state_with_learnings(tmp.path());
        let long = "x".repeat(MAX_SPEC_ID_LEN + 1);
        for bad in ["003\\nIGNORE PREVIOUS", "<script>", "a b", long.as_str()] {
            let body = serde_json::json!({
                "agent_id": "feat-x",
                "worktree_path": "/tmp/feat-x",
                "cli": "claude",
                "spec_id": bad.replace("\\n", "\n"),
            })
            .to_string();
            assert_eq!(
                post_watch(&state, body).await,
                StatusCode::BAD_REQUEST,
                "{bad:?} must be refused"
            );
        }
        for good in [
            "003-user-list",
            "add-auth",
            "2026-01-01-plan.v2",
            "specs/003_x",
        ] {
            assert!(spec_id_is_valid(good), "{good:?} must be accepted");
        }
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
                loopback_request()
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
                loopback_request()
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
                loopback_request()
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
                loopback_request()
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
                loopback_request()
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

        let app = router(state, true);
        let resp = app
            .oneshot(
                loopback_request()
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

    /// Also `broker-runtime` scenario "A request without wait returns at
    /// once": three messages, `since=2`, the entry at sequence 3 and
    /// `last_seq = 3`, with no waiting.
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

        let app = router(state, true);
        let resp = app
            .oneshot(
                loopback_request()
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
        assert_eq!(parsed["last_seq"], 3);
    }

    #[tokio::test]
    async fn log_invalid_since_returns_400() {
        let app = test_router();
        let resp = app
            .oneshot(
                loopback_request()
                    .method("GET")
                    .uri("/log?since=notanumber")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // --- `GET /log` long-polls on the since cursor ----------------------

    /// Publishes `count` distinct `agent.status` messages so the log has a
    /// known last sequence number.
    fn seed_log(state: &Arc<BrokerState>, count: u64) {
        for n in 1..=count {
            let msg = BrokerMessage::Status {
                agent_id: format!("feat-seed{n}"),
                payload: super::super::messages::StatusPayload {
                    status: "working".to_string(),
                    ..Default::default()
                },
            };
            delivery::publish_message(state, &msg);
        }
    }

    /// GETs `uri` against a router over `state` and returns the parsed body.
    async fn get_log(state: Arc<BrokerState>, uri: &str) -> serde_json::Value {
        let resp = router(state, true)
            .oneshot(
                loopback_request()
                    .method("GET")
                    .uri(uri)
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

    /// `broker-runtime` scenario "A waiting request returns as soon as an
    /// entry is published". Virtual time (`start_paused`) keeps the test
    /// instant while still proving the handler returned on the *publish*, at
    /// the 2-second mark, rather than sitting out its 30-second wait.
    #[tokio::test(start_paused = true)]
    async fn log_wait_returns_as_soon_as_an_entry_is_published() {
        let state = Arc::new(BrokerState::new(None));
        seed_log(&state, 7);

        let publisher = Arc::clone(&state);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let msg = BrokerMessage::Status {
                agent_id: "feat-late".to_string(),
                payload: super::super::messages::StatusPayload {
                    status: "working".to_string(),
                    ..Default::default()
                },
            };
            delivery::publish_message(&publisher, &msg);
        });

        let started = tokio::time::Instant::now();
        let parsed = get_log(state, "/log?since=7&wait=30").await;
        let waited = started.elapsed();

        let entries = parsed["entries"].as_array().expect("entries array");
        assert_eq!(entries.len(), 1, "exactly the newly published entry");
        assert_eq!(entries[0]["message"]["agent_id"], "feat-late");
        assert_eq!(parsed["last_seq"], 8);
        assert!(
            waited < Duration::from_secs(10),
            "the wait must end on the publish (~2s), not run to 30s; waited {waited:?}"
        );
    }

    /// `broker-runtime` scenario "A waiting request times out empty" — and
    /// the cursor-holding clause: an empty answer reports `last_seq = since`,
    /// never `0`.
    #[tokio::test(start_paused = true)]
    async fn log_wait_times_out_empty_and_holds_the_cursor() {
        let state = Arc::new(BrokerState::new(None));
        seed_log(&state, 7);

        let started = tokio::time::Instant::now();
        let parsed = get_log(state, "/log?since=7&wait=2").await;
        let waited = started.elapsed();

        assert!(
            parsed["entries"].as_array().unwrap().is_empty(),
            "nothing was published, so the answer must be empty"
        );
        assert_eq!(parsed["last_seq"], 7, "an empty answer holds the cursor");
        assert!(
            waited >= Duration::from_secs(2),
            "the handler must hold the request for the full wait; waited {waited:?}"
        );
    }

    /// `broker-runtime` scenario "The wait is capped": a client asking for
    /// 600 seconds is served at the 60-second cap, so no caller can pin a
    /// handler task indefinitely.
    #[tokio::test(start_paused = true)]
    async fn log_wait_is_capped_at_sixty_seconds() {
        let state = Arc::new(BrokerState::new(None));

        let started = tokio::time::Instant::now();
        let parsed = get_log(state, "/log?since=0&wait=600").await;
        let waited = started.elapsed();

        assert!(parsed["entries"].as_array().unwrap().is_empty());
        assert!(
            waited >= Duration::from_secs(60) && waited < Duration::from_secs(120),
            "wait=600 must be served at the 60s cap; waited {waited:?}"
        );
    }

    /// `broker-runtime` scenario "A non-numeric wait is rejected".
    #[tokio::test]
    async fn log_non_numeric_wait_returns_400() {
        let resp = test_router()
            .oneshot(
                loopback_request()
                    .method("GET")
                    .uri("/log?wait=soon")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    /// `broker-runtime` scenario "An empty response without wait holds the
    /// cursor": the v0.17 behaviour reported `last_seq = 0` here, rewinding
    /// every reader that asked for nothing new.
    #[tokio::test]
    async fn log_empty_response_without_wait_holds_the_cursor() {
        let state = Arc::new(BrokerState::new(None));
        seed_log(&state, 7);

        let parsed = get_log(state, "/log?since=7").await;

        assert!(parsed["entries"].as_array().unwrap().is_empty());
        assert_eq!(parsed["last_seq"], 7);
    }

    /// `broker-runtime` scenario "A restarted broker reports a new epoch":
    /// every `/log` answer names the serving broker instance, it is stable
    /// across that broker's answers, and a replacement broker reports a
    /// different one — which is the only way a reader can learn its cursor
    /// has become meaningless.
    #[tokio::test]
    async fn log_reports_a_stable_epoch_that_changes_when_the_broker_does() {
        let first = Arc::new(BrokerState::new(None));
        seed_log(&first, 3);

        let epoch = get_log(Arc::clone(&first), "/log?since=0").await["epoch"]
            .as_u64()
            .expect("every /log answer names its broker");
        assert!(epoch > 0);
        assert_eq!(
            get_log(first, "/log?since=3").await["epoch"],
            epoch,
            "one broker's epoch must not drift between answers, or a reader \
             would reset its cursor on every poll"
        );

        // The replacement a pause/start cycle creates. Its sequence numbers
        // begin again at 1, so its epoch has to differ.
        tokio::time::sleep(Duration::from_millis(5)).await;
        let replacement = Arc::new(BrokerState::new(None));
        let new_epoch = get_log(replacement, "/log?since=0").await["epoch"]
            .as_u64()
            .expect("epoch");

        assert_ne!(
            new_epoch, epoch,
            "a replacement broker must be distinguishable from the one whose \
             cursor the reader still holds"
        );
    }

    // --- `GET /status` reports wave staleness ---------------------------

    /// Publishes `agent.status` for `agent` with the given label and phase.
    fn publish_status(state: &Arc<BrokerState>, agent: &str, status: &str, phase: Option<&str>) {
        let msg = BrokerMessage::Status {
            agent_id: agent.to_string(),
            payload: super::super::messages::StatusPayload {
                status: status.to_string(),
                phase: phase.map(str::to_string),
                ..Default::default()
            },
        };
        delivery::publish_message(state, &msg);
    }

    /// Returns the current Unix second, for ageing a broker's last-progress
    /// stamp without sleeping.
    fn now_unix() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Builds a broker whose agent `feat-a` is on record as `working` and
    /// whose wave last progressed `idle` seconds ago, under a `threshold`
    /// second stall threshold.
    fn aged_broker(threshold: u64, idle: u64) -> Arc<BrokerState> {
        let state = Arc::new(BrokerState::new(None));
        state.set_wave_stall_seconds(threshold);
        // The agent's first status is genuinely new, so it is progress; age
        // the wave afterwards to set up the "and then nothing moved" case.
        publish_status(&state, "feat-a", "working", None);
        state.set_last_progress_unix_secs(now_unix() - idle);
        state
    }

    /// Reads the `wave` object from `/status`.
    async fn get_wave(state: &Arc<BrokerState>) -> serde_json::Value {
        get_status(state).await["wave"].clone()
    }

    /// `broker-runtime` scenario "Repeated identical heartbeats are not
    /// progress": the stall that hid behind a roster of live `working` rows.
    #[tokio::test]
    async fn status_repeated_identical_heartbeats_are_not_progress() {
        let state = aged_broker(60, 120);
        // Two minutes of 10-second heartbeats, all identical.
        for _ in 0..12 {
            publish_status(&state, "feat-a", "working", None);
        }

        let wave = get_wave(&state).await;
        assert_eq!(wave["stalled"], true);
        assert!(
            wave["idle_seconds"].as_u64().unwrap() >= 120,
            "repeated heartbeats must not reset the wave clock; got {wave}"
        );
    }

    /// `broker-runtime` scenario "A real event resets staleness".
    #[tokio::test]
    async fn status_an_artifact_resets_staleness() {
        let state = aged_broker(60, 120);
        assert_eq!(get_wave(&state).await["stalled"], true);

        delivery::publish_message(
            &state,
            &BrokerMessage::Artifact {
                agent_id: "feat-a".to_string(),
                payload: super::super::messages::ArtifactPayload {
                    status: "committed".to_string(),
                    exports: vec![],
                    modified_files: vec!["src/lib.rs".to_string()],
                },
            },
        );

        let wave = get_wave(&state).await;
        assert_eq!(wave["stalled"], false);
        assert!(
            wave["idle_seconds"].as_u64().unwrap() <= 2,
            "an artifact is progress, so the wave clock restarts; got {wave}"
        );
    }

    /// `broker-runtime` scenario "Uncommitted file churn is not progress": an
    /// agent editing for two minutes without committing moves no phase.
    #[tokio::test]
    async fn status_uncommitted_file_churn_is_not_progress() {
        let state = aged_broker(60, 120);
        for files in [
            vec!["a.rs"],
            vec!["a.rs", "b.rs"],
            vec!["a.rs", "b.rs", "c.rs"],
            vec!["b.rs"],
        ] {
            delivery::publish_message(
                &state,
                &BrokerMessage::Status {
                    agent_id: "feat-a".to_string(),
                    payload: super::super::messages::StatusPayload {
                        status: "working".to_string(),
                        modified_files: files.iter().map(ToString::to_string).collect(),
                        ..Default::default()
                    },
                },
            );
        }

        let wave = get_wave(&state).await;
        assert_eq!(
            wave["stalled"], true,
            "a changing modified-file set is not a phase advance; got {wave}"
        );
    }

    /// `broker-runtime` scenario "The drive loop's own escalations and
    /// learnings are not progress": under `--unattended` the drive loop
    /// escalates a live permission prompt, re-escalates it after its dedup
    /// window, and records learnings — all while the wave sits on the
    /// unanswered prompt.
    #[tokio::test]
    async fn status_drive_loop_escalations_and_learnings_are_not_progress() {
        let state = aged_broker(60, 120);
        for _ in 0..2 {
            delivery::publish_message(
                &state,
                &BrokerMessage::Question {
                    agent_id: "feat-a".to_string(),
                    payload: super::super::messages::QuestionPayload {
                        question: "permission prompt: allow `cargo test`?".to_string(),
                    },
                },
            );
            delivery::publish_message(
                &state,
                &BrokerMessage::Learning {
                    payload: super::super::messages::LearningPayload {
                        id: "0123456789abcdef".to_string(),
                        agent_id: "supervisor".to_string(),
                        branch_id: Some("feat-a".to_string()),
                        category: "permission_pattern".to_string(),
                        title: "escalated a permission prompt".to_string(),
                        body: serde_json::json!({}),
                        timestamp: "2026-10-26T00:00:00Z".to_string(),
                    },
                },
            );
        }

        let wave = get_wave(&state).await;
        assert_eq!(wave["stalled"], true);
        assert!(
            wave["idle_seconds"].as_u64().unwrap() >= 120,
            "escalations and learnings must not mask the stall; got {wave}"
        );
    }

    /// `broker-runtime` scenario "A stuck-detection alert is not progress",
    /// end to end through `/status`: two minutes of the helper's alerts
    /// about a stalled agent leave the wave reported as stalled.
    #[tokio::test]
    async fn status_stuck_detection_alerts_are_not_progress() {
        let state = aged_broker(60, 120);
        for phase in [
            "stuck-on-prompt",
            "stuck-stream-timeout",
            "context-bloat",
            "no-progress",
            "blocked-on-supervisor",
        ] {
            publish_status(&state, "feat-a", "working", Some(phase));
        }

        let wave = get_wave(&state).await;
        assert_eq!(wave["stalled"], true, "got {wave}");
        assert!(
            wave["idle_seconds"].as_u64().unwrap() >= 120,
            "detecting the stall must not reset the clock; got {wave}"
        );
    }

    /// `broker-runtime` scenario "A zero threshold disables the flag".
    #[tokio::test]
    async fn status_zero_threshold_disables_the_flag_but_still_reports_idleness() {
        let state = aged_broker(0, 3600);

        let wave = get_wave(&state).await;
        assert_eq!(wave["stalled"], false);
        assert_eq!(wave["stall_threshold_seconds"], 0);
        assert!(wave["idle_seconds"].as_u64().unwrap() >= 3600);
    }

    /// The `wave` object carries every documented field, and a broker that
    /// has accepted no message reports its start time as the last progress.
    #[tokio::test]
    async fn status_wave_reports_start_time_before_any_message() {
        let state = Arc::new(BrokerState::new(None));
        let wave = get_wave(&state).await;

        assert!(wave["last_progress_unix_secs"].as_u64().unwrap() > 0);
        assert!(wave["idle_seconds"].as_u64().unwrap() <= 2);
        assert_eq!(wave["stall_threshold_seconds"], 1800);
        assert_eq!(wave["stalled"], false);
    }

    // --- Browser-exposure guard (Host / Origin) -------------------------

    /// Reads a response body into a string.
    async fn body_text(resp: Response) -> String {
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("read body");
        String::from_utf8(bytes.to_vec()).expect("body is utf-8")
    }

    const ANSWER_BODY: &str = r#"{"type":"agent.answer","agent_id":"supervisor","payload":{"target":"feat-x","from":"supervisor","answer":"ship it"}}"#;

    #[tokio::test]
    async fn loopback_host_is_served() {
        let resp = test_router()
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .header("host", "127.0.0.1:9119")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn tunnelled_loopback_host_on_another_port_is_served() {
        let resp = test_router()
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .header("host", "localhost:19119")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rebinding_host_is_refused_without_state_change() {
        let state = Arc::new(BrokerState::new(None));
        let resp = router(Arc::clone(&state), true)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/publish")
                    .header("content-type", "application/json")
                    .header("host", "attacker.example:9119")
                    .body(Body::from(ANSWER_BODY))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert_eq!(body_text(resp).await, r#"{"error":"forbidden host"}"#);

        // The refused publish must not have reached delivery.
        let resp = router(Arc::clone(&state), true)
            .oneshot(
                loopback_request()
                    .uri("/messages/feat-x")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_text(resp).await;
        assert!(
            !body.contains("agent.answer"),
            "inbox must stay empty, got {body}"
        );
    }

    #[tokio::test]
    async fn foreign_origin_is_refused_even_with_a_loopback_host() {
        let state = Arc::new(BrokerState::new(None));
        let resp = router(Arc::clone(&state), true)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/publish")
                    .header("content-type", "application/json")
                    .header("host", "127.0.0.1:9119")
                    .header("origin", "http://attacker.example")
                    .body(Body::from(ANSWER_BODY))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert_eq!(body_text(resp).await, r#"{"error":"forbidden origin"}"#);
    }

    #[tokio::test]
    async fn null_origin_is_refused() {
        let resp = test_router()
            .oneshot(
                Request::builder()
                    .uri("/log")
                    .header("host", "127.0.0.1:9119")
                    .header("origin", "null")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert_eq!(body_text(resp).await, r#"{"error":"forbidden origin"}"#);
    }

    #[tokio::test]
    async fn loopback_origin_is_served() {
        let resp = test_router()
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .header("host", "127.0.0.1:9119")
                    .header("origin", "http://localhost:3000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn unknown_route_with_a_foreign_host_is_refused_before_routing() {
        let resp = test_router()
            .oneshot(
                Request::builder()
                    .uri("/no-such-route")
                    .header("host", "attacker.example:9119")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert_eq!(body_text(resp).await, r#"{"error":"forbidden host"}"#);
    }

    #[tokio::test]
    async fn unknown_route_with_a_loopback_host_still_404s() {
        let resp = test_router()
            .oneshot(
                loopback_request()
                    .uri("/no-such-route")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn non_loopback_bind_serves_a_foreign_host_but_still_refuses_an_origin() {
        let state = Arc::new(BrokerState::new(None));
        let resp = router(Arc::clone(&state), false)
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .header("host", "192.168.1.5:9119")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = router(state, false)
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .header("host", "192.168.1.5:9119")
                    .header("origin", "http://attacker.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_non_text_origin_is_refused_not_treated_as_absent() {
        let mut request = Request::builder()
            .uri("/status")
            .header("host", "127.0.0.1:9119")
            .body(Body::empty())
            .unwrap();
        request.headers_mut().insert(
            axum::http::header::ORIGIN,
            axum::http::HeaderValue::from_bytes(b"http://\xffevil.com").unwrap(),
        );
        let resp = test_router().oneshot(request).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert_eq!(body_text(resp).await, r#"{"error":"forbidden origin"}"#);
    }

    #[tokio::test]
    async fn a_non_text_host_is_refused() {
        let mut request = Request::builder()
            .uri("/status")
            .body(Body::empty())
            .unwrap();
        request.headers_mut().insert(
            axum::http::header::HOST,
            axum::http::HeaderValue::from_bytes(b"127.0.0.1\xff").unwrap(),
        );
        let resp = test_router().oneshot(request).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert_eq!(body_text(resp).await, r#"{"error":"forbidden host"}"#);
    }

    #[tokio::test]
    async fn a_duplicate_host_header_is_refused_not_resolved_to_the_first() {
        let mut request = Request::builder()
            .uri("/status")
            .header("host", "127.0.0.1:9119")
            .body(Body::empty())
            .unwrap();
        request.headers_mut().append(
            axum::http::header::HOST,
            axum::http::HeaderValue::from_static("attacker.example:9119"),
        );
        let resp = test_router().oneshot(request).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert_eq!(body_text(resp).await, r#"{"error":"forbidden host"}"#);
    }

    #[tokio::test]
    async fn a_duplicate_origin_header_is_refused_not_resolved_to_the_first() {
        let mut request = Request::builder()
            .uri("/status")
            .header("host", "127.0.0.1:9119")
            .header("origin", "http://localhost:3000")
            .body(Body::empty())
            .unwrap();
        request.headers_mut().append(
            axum::http::header::ORIGIN,
            axum::http::HeaderValue::from_static("http://attacker.example"),
        );
        let resp = test_router().oneshot(request).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert_eq!(body_text(resp).await, r#"{"error":"forbidden origin"}"#);
    }

    #[tokio::test]
    async fn a_foreign_absolute_form_target_is_refused_despite_a_loopback_host() {
        let resp = test_router()
            .oneshot(
                Request::builder()
                    .uri("http://evil.com/status")
                    .header("host", "127.0.0.1:9119")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert_eq!(body_text(resp).await, r#"{"error":"forbidden host"}"#);
    }

    #[tokio::test]
    async fn a_loopback_absolute_form_target_is_served() {
        let resp = test_router()
            .oneshot(
                Request::builder()
                    .uri("http://127.0.0.1:9119/status")
                    .header("host", "127.0.0.1:9119")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn a_host_with_junk_after_the_port_is_refused() {
        for host in ["localhost:evil.com", "127.0.0.1:80:evil", "[::1]evil.com"] {
            let resp = test_router()
                .oneshot(
                    Request::builder()
                        .uri("/status")
                        .header("host", host)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::FORBIDDEN, "host {host}");
            assert_eq!(body_text(resp).await, r#"{"error":"forbidden host"}"#);
        }
    }
}

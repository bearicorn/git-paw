//! Broker-round-trip integration tests for the conflict detector.
//!
//! Spins up a real broker (`start_broker_with`) with the detector wired
//! in, publishes messages over HTTP just like agents would, and verifies
//! the auto-emitted feedback / question messages reach the right inboxes.
//!
//! Covers `openspec/changes/conflict-detection/tasks.md` items 5.1-5.3.

use std::fmt::Write as _;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

use git_paw::broker::{self, BrokerState};
use git_paw::config::{BrokerConfig, ConflictConfig};
use serial_test::serial;

/// Atomic counter to give each test a unique port — avoids cross-test
/// races on the same port.
static PORT_COUNTER: AtomicU16 = AtomicU16::new(0);

fn spawn_test_broker(conflict: Option<&ConflictConfig>) -> (broker::BrokerHandle, String) {
    #[allow(clippy::cast_possible_truncation)]
    let base = 26_000 + (std::process::id() as u16 % 3000);
    let offset = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut port = base + offset;
    let mut attempts = 0;
    loop {
        let config = BrokerConfig {
            enabled: true,
            port,
            bind: "127.0.0.1".to_string(),
        };
        match broker::start_broker_with(
            &config,
            BrokerState::new(None),
            Vec::new(),
            conflict.cloned(),
            60,
        ) {
            Ok(handle) => {
                let url = config.url();
                return (handle, url);
            }
            Err(_) if attempts < 10 => {
                port += 100;
                attempts += 1;
            }
            Err(e) => panic!("failed to start test broker after retries: {e}"),
        }
    }
}

fn http_request(url: &str, method: &str, path: &str, body: &str) -> (u16, String) {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let addr = url.strip_prefix("http://").unwrap_or(url);
    let mut stream = TcpStream::connect(addr).expect("connect to broker");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    if !body.is_empty() {
        let _ = write!(
            request,
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
    }
    request.push_str("\r\n");
    request.push_str(body);

    stream.write_all(request.as_bytes()).expect("write request");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).ok();

    let mut parts = raw.splitn(2, "\r\n\r\n");
    let header = parts.next().unwrap_or("");
    let raw_body = parts.next().unwrap_or("").to_string();
    let status_line = header.lines().next().unwrap_or("");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    // Decode chunked transfer encoding if present.
    let body_text = if header.to_lowercase().contains("transfer-encoding: chunked") {
        decode_chunked(&raw_body)
    } else {
        raw_body
    };
    (status, body_text)
}

fn decode_chunked(body: &str) -> String {
    let mut result = String::new();
    let mut remaining = body;
    loop {
        let line_end = remaining.find("\r\n").unwrap_or(remaining.len());
        let size_str = &remaining[..line_end];
        let size = usize::from_str_radix(size_str.trim(), 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        remaining = &remaining[line_end + 2..];
        if remaining.len() >= size {
            result.push_str(&remaining[..size]);
            remaining = &remaining[size..];
            if remaining.starts_with("\r\n") {
                remaining = &remaining[2..];
            }
        } else {
            break;
        }
    }
    result
}

fn publish(url: &str, payload: &str) {
    let (status, _) = http_request(url, "POST", "/publish", payload);
    assert_eq!(status, 202, "publish should return 202");
}

fn poll(url: &str, agent: &str) -> serde_json::Value {
    let (status, body) = http_request(url, "GET", &format!("/messages/{agent}"), "");
    assert_eq!(status, 200, "poll should return 200, body: {body}");
    serde_json::from_str(&body).expect("messages JSON")
}

/// Polls `agent` for up to ~2 seconds waiting for `predicate` to return
/// `Some(value)`. The detector loop runs on its own tick interval; this
/// gives it time to observe and react.
fn poll_until<F, T>(url: &str, agent: &str, mut predicate: F) -> Option<T>
where
    F: FnMut(&serde_json::Value) -> Option<T>,
{
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        let response = poll(url, agent);
        if let Some(v) = predicate(&response) {
            return Some(v);
        }
        std::thread::sleep(Duration::from_millis(80));
    }
    None
}

/// Ensure an inbox exists at the broker for the given agent so subsequent
/// detector-emitted Feedback messages don't get silently dropped.
fn register_agent(url: &str, agent_id: &str) {
    let body = format!(
        r#"{{"type":"agent.status","agent_id":"{agent_id}","payload":{{"status":"idle","modified_files":[]}}}}"#
    );
    publish(url, &body);
}

fn drain_inbox(url: &str, agent: &str) {
    let _ = poll(url, agent);
}

// =========================================================================
// 5.1: Forward conflict over HTTP — both publishers receive
//      [conflict-detector] feedback from supervisor.
// =========================================================================

#[test]
#[serial]
fn forward_conflict_round_trips_over_http() {
    let cfg = ConflictConfig::default();
    let (_handle, url) = spawn_test_broker(Some(&cfg));
    register_agent(&url, "feat-xx");
    register_agent(&url, "feat-yy");
    // Drain the initial agent.status echoes so the predicate below sees
    // only what comes after the intents.
    drain_inbox(&url, "feat-xx");
    drain_inbox(&url, "feat-yy");

    publish(
        &url,
        r#"{"type":"agent.intent","agent_id":"feat-xx","payload":{"files":["src/a.rs","src/b.rs"],"summary":"x","valid_for_seconds":600}}"#,
    );
    publish(
        &url,
        r#"{"type":"agent.intent","agent_id":"feat-yy","payload":{"files":["src/b.rs","src/c.rs"],"summary":"y","valid_for_seconds":600}}"#,
    );

    let predicate = |resp: &serde_json::Value| {
        let msgs = resp["messages"].as_array()?;
        let fb = msgs.iter().find(|m| {
            m["type"] == "agent.feedback"
                && m["payload"]["from"] == "supervisor"
                && m["payload"]["errors"]
                    .as_array()
                    .and_then(|errs| errs.first()?.as_str())
                    .is_some_and(|s| {
                        s.starts_with("[conflict-detector]") && s.contains("forward conflict")
                    })
        })?;
        Some(fb.clone())
    };

    let x_fb = poll_until(&url, "feat-xx", predicate)
        .expect("feat-xx should receive forward-conflict feedback over HTTP");
    let y_fb = poll_until(&url, "feat-yy", predicate)
        .expect("feat-yy should receive forward-conflict feedback over HTTP");

    let x_err = x_fb["payload"]["errors"][0].as_str().unwrap();
    assert!(x_err.contains("feat-yy"));
    assert!(x_err.contains("src/b.rs"));
    let y_err = y_fb["payload"]["errors"][0].as_str().unwrap();
    assert!(y_err.contains("feat-xx"));
    assert!(y_err.contains("src/b.rs"));
}

// =========================================================================
// 5.2: In-flight conflict — both branches warned. Escalation hits the
//      supervisor inbox once the window elapses.
//
// We use a 1-second window to keep the test fast (the detector polls
// every 500ms, so two ticks comfortably cover the elapsed window).
// =========================================================================

#[test]
#[serial]
fn in_flight_conflict_round_trips_and_escalates() {
    let cfg = ConflictConfig {
        window_seconds: 1,
        ..ConflictConfig::default()
    };
    let (_handle, url) = spawn_test_broker(Some(&cfg));
    register_agent(&url, "feat-xx");
    register_agent(&url, "feat-yy");
    drain_inbox(&url, "feat-xx");
    drain_inbox(&url, "feat-yy");
    drain_inbox(&url, "supervisor");

    publish(
        &url,
        r#"{"type":"agent.status","agent_id":"feat-xx","payload":{"status":"working","modified_files":["src/a.rs"]}}"#,
    );
    publish(
        &url,
        r#"{"type":"agent.status","agent_id":"feat-yy","payload":{"status":"working","modified_files":["src/a.rs"]}}"#,
    );

    let fb_pred = |resp: &serde_json::Value| {
        let msgs = resp["messages"].as_array()?;
        msgs.iter()
            .find(|m| {
                m["type"] == "agent.feedback"
                    && m["payload"]["errors"]
                        .as_array()
                        .and_then(|errs| errs.first()?.as_str())
                        .is_some_and(|s| {
                            s.starts_with("[conflict-detector]") && s.contains("in-flight conflict")
                        })
            })
            .cloned()
    };
    assert!(
        poll_until(&url, "feat-xx", fb_pred).is_some(),
        "feat-xx in-flight feedback"
    );
    assert!(
        poll_until(&url, "feat-yy", fb_pred).is_some(),
        "feat-yy in-flight feedback"
    );

    // Wait long enough for the window to elapse and a detector tick to fire.
    std::thread::sleep(Duration::from_secs(2));

    let question_pred = |resp: &serde_json::Value| {
        let msgs = resp["messages"].as_array()?;
        msgs.iter()
            .find(|m| {
                m["type"] == "agent.question"
                    && m["agent_id"] == "supervisor"
                    && m["payload"]["question"]
                        .as_str()
                        .is_some_and(|s| s.contains("[conflict-detector]"))
            })
            .cloned()
    };
    let q = poll_until(&url, "supervisor", question_pred)
        .expect("supervisor inbox should receive an escalation question");
    let text = q["payload"]["question"].as_str().unwrap();
    assert!(text.contains("src/a.rs"));
    assert!(text.contains("feat-xx"));
    assert!(text.contains("feat-yy"));
}

// =========================================================================
// 5.3: Ownership-violation round trip + escalate_on_violation = false
//      suppresses the supervisor question.
// =========================================================================

#[test]
#[serial]
fn ownership_violation_round_trips_with_optional_escalation() {
    let cfg = ConflictConfig {
        // Disable forward warnings so we only observe ownership signals.
        warn_on_intent_overlap: false,
        escalate_on_violation: false,
        ..ConflictConfig::default()
    };
    let (_handle, url) = spawn_test_broker(Some(&cfg));
    register_agent(&url, "feat-xx");
    register_agent(&url, "feat-yy");
    drain_inbox(&url, "feat-xx");
    drain_inbox(&url, "feat-yy");
    drain_inbox(&url, "supervisor");

    publish(
        &url,
        r#"{"type":"agent.intent","agent_id":"feat-xx","payload":{"files":["src/a.rs"],"summary":"x","valid_for_seconds":600}}"#,
    );
    publish(
        &url,
        r#"{"type":"agent.intent","agent_id":"feat-yy","payload":{"files":["src/b.rs"],"summary":"y","valid_for_seconds":600}}"#,
    );
    publish(
        &url,
        r#"{"type":"agent.status","agent_id":"feat-yy","payload":{"status":"working","modified_files":["src/a.rs"]}}"#,
    );

    let fb_pred = |resp: &serde_json::Value| {
        let msgs = resp["messages"].as_array()?;
        msgs.iter()
            .find(|m| {
                m["type"] == "agent.feedback"
                    && m["payload"]["errors"]
                        .as_array()
                        .and_then(|errs| errs.first()?.as_str())
                        .is_some_and(|s| {
                            s.starts_with("[conflict-detector]")
                                && s.contains("ownership violation")
                        })
            })
            .cloned()
    };
    let fb = poll_until(&url, "feat-yy", fb_pred).expect("feat-yy should get ownership feedback");
    let err = fb["payload"]["errors"][0].as_str().unwrap();
    assert!(err.contains("src/a.rs"));
    assert!(err.contains("feat-xx"));

    // With escalate_on_violation = false, no question should appear in
    // the supervisor inbox. Give the detector a couple of ticks first so
    // the absence is meaningful.
    std::thread::sleep(Duration::from_millis(1500));
    let supervisor_inbox = poll(&url, "supervisor");
    let questions: Vec<&serde_json::Value> = supervisor_inbox["messages"]
        .as_array()
        .map(|m| m.iter().filter(|m| m["type"] == "agent.question").collect())
        .unwrap_or_default();
    assert!(
        questions.is_empty(),
        "no question expected when escalate_on_violation = false, got: {questions:?}"
    );
}

// =========================================================================
// Detector inactive when supervisor mode is off (task 4.16 + lifecycle
// requirement). Conflict-detection module is simply not started, so
// overlapping intents produce no auto-emitted messages.
// =========================================================================

#[test]
#[serial]
fn detector_not_started_when_supervisor_disabled() {
    // `None` here means "do not pass a ConflictConfig", i.e. supervisor
    // disabled. Intents still broadcast normally per forward-coordination,
    // but no [conflict-detector] feedback fires.
    let (_handle, url) = spawn_test_broker(None);
    register_agent(&url, "feat-xx");
    register_agent(&url, "feat-yy");
    drain_inbox(&url, "feat-xx");
    drain_inbox(&url, "feat-yy");

    publish(
        &url,
        r#"{"type":"agent.intent","agent_id":"feat-xx","payload":{"files":["src/a.rs"],"summary":"x","valid_for_seconds":600}}"#,
    );
    publish(
        &url,
        r#"{"type":"agent.intent","agent_id":"feat-yy","payload":{"files":["src/a.rs"],"summary":"y","valid_for_seconds":600}}"#,
    );

    std::thread::sleep(Duration::from_millis(1200));

    for who in &["feat-xx", "feat-yy"] {
        let resp = poll(&url, who);
        let detector_feedback_count = resp["messages"].as_array().map_or(0, |msgs| {
            msgs.iter()
                .filter(|m| {
                    m["type"] == "agent.feedback"
                        && m["payload"]["errors"]
                            .as_array()
                            .and_then(|errs| errs.first()?.as_str())
                            .is_some_and(|s| s.starts_with("[conflict-detector]"))
                })
                .count()
        });
        assert_eq!(
            detector_feedback_count, 0,
            "{who} should not receive detector-emitted feedback when supervisor is off"
        );
    }
}

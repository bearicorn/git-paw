//! Integration tests for the HTTP broker.

use std::fmt::Write as _;
use std::sync::atomic::{AtomicU16, Ordering};

use git_paw::broker::{self, BrokerState};
use git_paw::config::BrokerConfig;
use serial_test::serial;

/// Atomic counter to ensure each test gets a unique port.
static PORT_COUNTER: AtomicU16 = AtomicU16::new(0);

/// Starts a broker on a unique free port and returns the handle + URL.
fn spawn_test_broker() -> (broker::BrokerHandle, String) {
    #[allow(clippy::cast_possible_truncation)]
    let base = 20_000 + (std::process::id() as u16 % 5000);
    let offset = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut port = base + offset;
    let mut attempts = 0;
    loop {
        let config = BrokerConfig {
            enabled: true,
            port,
            bind: "127.0.0.1".to_string(),
        };
        match broker::start_broker(&config, BrokerState::new(None), Vec::new()) {
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

/// Helper to make HTTP requests to the broker using raw TCP.
fn http_request(
    url: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> HttpResponse {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let addr = url.strip_prefix("http://").unwrap_or(url);
    let mut stream = TcpStream::connect(addr).expect("failed to connect to broker");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    for (key, value) in headers {
        let _ = write!(request, "{key}: {value}\r\n");
    }
    if !body.is_empty() {
        let _ = write!(request, "Content-Length: {}\r\n", body.len());
    }
    request.push_str("\r\n");
    request.push_str(body);

    stream
        .write_all(request.as_bytes())
        .expect("failed to write request");

    let mut response = String::new();
    stream.read_to_string(&mut response).ok();
    HttpResponse::parse(&response)
}

struct HttpResponse {
    status: u16,
    body: String,
}

impl HttpResponse {
    fn parse(raw: &str) -> Self {
        let mut parts = raw.splitn(2, "\r\n\r\n");
        let header_section = parts.next().unwrap_or("");
        let body = parts.next().unwrap_or("").to_string();

        let status_line = header_section.lines().next().unwrap_or("");
        let status = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Handle chunked transfer encoding
        let body = if header_section
            .to_lowercase()
            .contains("transfer-encoding: chunked")
        {
            decode_chunked(&body)
        } else {
            body
        };

        Self { status, body }
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| {
            panic!(
                "failed to parse response body as JSON: {e}\nbody: {:?}",
                self.body
            )
        })
    }
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

// --- Integration tests ---

#[test]
#[serial]
fn publish_valid_agent_status_returns_202() {
    let (_handle, url) = spawn_test_broker();
    let resp = http_request(
        &url,
        "POST",
        "/publish",
        &[("Content-Type", "application/json")],
        r#"{"type":"agent.status","agent_id":"feat-x","payload":{"status":"idle","modified_files":[]}}"#,
    );
    assert_eq!(resp.status, 202);
}

#[test]
#[serial]
fn publish_invalid_json_returns_400() {
    let (_handle, url) = spawn_test_broker();
    let resp = http_request(
        &url,
        "POST",
        "/publish",
        &[("Content-Type", "application/json")],
        "not json",
    );
    assert_eq!(resp.status, 400);
    let json = resp.json();
    assert!(json["error"].is_string());
}

#[test]
#[serial]
fn publish_empty_agent_id_returns_400() {
    let (_handle, url) = spawn_test_broker();
    let resp = http_request(
        &url,
        "POST",
        "/publish",
        &[("Content-Type", "application/json")],
        r#"{"type":"agent.status","agent_id":"","payload":{"status":"idle","modified_files":[]}}"#,
    );
    assert_eq!(resp.status, 400);
}

#[test]
#[serial]
fn publish_wrong_content_type_returns_415() {
    let (_handle, url) = spawn_test_broker();
    let resp = http_request(
        &url,
        "POST",
        "/publish",
        &[("Content-Type", "text/plain")],
        "{}",
    );
    assert_eq!(resp.status, 415);
}

#[test]
#[serial]
fn publish_empty_body_returns_400() {
    let (_handle, url) = spawn_test_broker();
    let resp = http_request(
        &url,
        "POST",
        "/publish",
        &[("Content-Type", "application/json")],
        "",
    );
    assert_eq!(resp.status, 400);
}

#[test]
#[serial]
fn messages_returns_empty_list_with_last_seq() {
    let (_handle, url) = spawn_test_broker();
    let resp = http_request(&url, "GET", "/messages/feat-x", &[], "");
    assert_eq!(resp.status, 200);
    let json = resp.json();
    assert_eq!(json["messages"], serde_json::json!([]));
    assert_eq!(json["last_seq"], serde_json::json!(0));
}

#[test]
#[serial]
fn messages_invalid_agent_id_returns_400() {
    let (_handle, url) = spawn_test_broker();
    // URL-encoded slash: feat%2Fx decodes to feat/x which has uppercase or slash
    let resp = http_request(&url, "GET", "/messages/feat%2Fx", &[], "");
    assert_eq!(resp.status, 400);
}

#[test]
#[serial]
fn messages_invalid_since_returns_400() {
    let (_handle, url) = spawn_test_broker();
    let resp = http_request(&url, "GET", "/messages/feat-x?since=abc", &[], "");
    assert_eq!(resp.status, 400);
}

#[test]
#[serial]
fn status_returns_marker_and_fields() {
    let (_handle, url) = spawn_test_broker();
    let resp = http_request(&url, "GET", "/status", &[], "");
    assert_eq!(resp.status, 200);
    let json = resp.json();
    assert_eq!(json["git_paw"], true);
    assert!(json["version"].is_string());
    assert!(json["uptime_seconds"].is_number());
    assert_eq!(json["agents"], serde_json::json!([]));
}

#[test]
#[serial]
fn concurrent_status_requests() {
    let (_handle, url) = spawn_test_broker();
    let mut handles = Vec::new();
    for _ in 0..10 {
        let url = url.clone();
        handles.push(std::thread::spawn(move || {
            let resp = http_request(&url, "GET", "/status", &[], "");
            assert_eq!(resp.status, 200);
            let json = resp.json();
            assert_eq!(json["git_paw"], true);
        }));
    }
    for h in handles {
        h.join().expect("thread panicked");
    }
}

#[test]
#[serial]
fn unknown_route_returns_404() {
    let (_handle, url) = spawn_test_broker();
    let resp = http_request(&url, "GET", "/unknown/route", &[], "");
    assert_eq!(resp.status, 404);
}

#[test]
#[serial]
fn wrong_method_returns_405() {
    let (_handle, url) = spawn_test_broker();
    // GET /publish should be 405
    let resp1 = http_request(&url, "GET", "/publish", &[], "");
    assert_eq!(resp1.status, 405);

    // POST /status should be 405
    let resp2 = http_request(&url, "POST", "/status", &[], "");
    assert_eq!(resp2.status, 405);

    // POST /messages/feat-x should be 405
    let resp3 = http_request(
        &url,
        "POST",
        "/messages/feat-x",
        &[("Content-Type", "application/json")],
        "{}",
    );
    assert_eq!(resp3.status, 405);
}

#[test]
#[serial]
fn dropping_handle_shuts_down_broker() {
    let (handle, url) = spawn_test_broker();

    // Verify it's running.
    let resp = http_request(&url, "GET", "/status", &[], "");
    assert_eq!(resp.status, 200);

    // Drop the handle.
    drop(handle);

    // Give the runtime a moment to shut down.
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Subsequent connect should fail.
    let addr = url.strip_prefix("http://").unwrap();
    let result = std::net::TcpStream::connect_timeout(
        &addr.parse().unwrap(),
        std::time::Duration::from_millis(500),
    );
    assert!(
        result.is_err(),
        "connection should fail after handle dropped"
    );
}

#[test]
#[serial]
fn second_broker_on_same_port_reattaches() {
    let (handle1, url) = spawn_test_broker();

    // Extract the port from the URL.
    let port: u16 = url.rsplit(':').next().unwrap().parse().unwrap();

    let config = BrokerConfig {
        enabled: true,
        port,
        bind: "127.0.0.1".to_string(),
    };

    // Starting a second broker on the same port should reattach.
    let handle2 = broker::start_broker(&config, BrokerState::new(None), Vec::new())
        .expect("second broker should reattach");

    // The reattached handle should have the same URL.
    assert_eq!(handle2.url, url);

    drop(handle2);
    drop(handle1);
}

#[test]
#[serial]
fn publish_and_poll_roundtrip() {
    let (_handle, url) = spawn_test_broker();

    // Register two agents
    for payload in [
        r#"{"type":"agent.status","agent_id":"a","payload":{"status":"working","modified_files":[]}}"#,
        r#"{"type":"agent.status","agent_id":"b","payload":{"status":"working","modified_files":[]}}"#,
    ] {
        let resp = http_request(
            &url,
            "POST",
            "/publish",
            &[("Content-Type", "application/json")],
            payload,
        );
        assert_eq!(resp.status, 202);
    }

    // Publish artifact from a -> broadcasts to b
    let resp = http_request(
        &url,
        "POST",
        "/publish",
        &[("Content-Type", "application/json")],
        r#"{"type":"agent.artifact","agent_id":"a","payload":{"status":"done","exports":[],"modified_files":["src/lib.rs"]}}"#,
    );
    assert_eq!(resp.status, 202);

    // Poll b's inbox
    let resp = http_request(&url, "GET", "/messages/b", &[], "");
    assert_eq!(resp.status, 200);
    let json = resp.json();
    let msgs = json["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1);
    assert!(json["last_seq"].as_u64().unwrap() > 0);
}

#[test]
#[serial]
fn poll_with_since_returns_only_newer() {
    let (_handle, url) = spawn_test_broker();

    // Register agents
    for payload in [
        r#"{"type":"agent.status","agent_id":"a","payload":{"status":"working","modified_files":[]}}"#,
        r#"{"type":"agent.status","agent_id":"b","payload":{"status":"working","modified_files":[]}}"#,
    ] {
        http_request(
            &url,
            "POST",
            "/publish",
            &[("Content-Type", "application/json")],
            payload,
        );
    }

    // Publish 3 artifacts from a -> b
    for _ in 0..3 {
        http_request(
            &url,
            "POST",
            "/publish",
            &[("Content-Type", "application/json")],
            r#"{"type":"agent.artifact","agent_id":"a","payload":{"status":"done","exports":[],"modified_files":[]}}"#,
        );
    }

    // Poll all first
    let resp = http_request(&url, "GET", "/messages/b", &[], "");
    let json = resp.json();
    let first_seq = json["last_seq"].as_u64().unwrap();
    assert_eq!(json["messages"].as_array().unwrap().len(), 3);

    // Publish 1 more
    http_request(
        &url,
        "POST",
        "/publish",
        &[("Content-Type", "application/json")],
        r#"{"type":"agent.artifact","agent_id":"a","payload":{"status":"done","exports":[],"modified_files":[]}}"#,
    );

    // Poll with since=first_seq
    let path = format!("/messages/b?since={first_seq}");
    let resp = http_request(&url, "GET", &path, &[], "");
    let json = resp.json();
    assert_eq!(json["messages"].as_array().unwrap().len(), 1);
}

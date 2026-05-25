//! Asserts that the live broker rejects phantom-shaped `agent_id` values and
//! does NOT expose them via `/status`. Maps to scenarios under
//! `supervisor-bugfixes-v0-5-x` / `broker-messages` (tasks 4.12 +
//! "Single-letter `agent_id` is rejected by the running broker").

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use git_paw::broker::{self, BrokerState};
use git_paw::config::BrokerConfig;
use serial_test::serial;

/// Atomic counter to ensure each test gets a unique port.
static PORT_COUNTER: AtomicU16 = AtomicU16::new(0);

fn spawn_test_broker() -> (broker::BrokerHandle, String) {
    #[allow(clippy::cast_possible_truncation)]
    let base = 25_000 + (std::process::id() as u16 % 5000);
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

struct HttpResponse {
    status: u16,
    body: String,
}

fn http_request(url: &str, method: &str, path: &str, body: &str) -> HttpResponse {
    let addr = url.strip_prefix("http://").unwrap_or(url);
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok();
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    if !body.is_empty() {
        let _ = write!(
            req,
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
    }
    req.push_str("\r\n");
    req.push_str(body);
    stream.write_all(req.as_bytes()).expect("write");
    let mut response = String::new();
    stream.read_to_string(&mut response).ok();
    let mut parts = response.splitn(2, "\r\n\r\n");
    let header_section = parts.next().unwrap_or("");
    let body = parts.next().unwrap_or("").to_string();
    let status_line = header_section.lines().next().unwrap_or("");
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let body = if header_section
        .to_lowercase()
        .contains("transfer-encoding: chunked")
    {
        decode_chunked(&body)
    } else {
        body
    };
    HttpResponse { status, body }
}

fn decode_chunked(body: &str) -> String {
    let mut result = String::new();
    let mut remaining = body;
    loop {
        let line_end = remaining.find("\r\n").unwrap_or(remaining.len());
        let size = usize::from_str_radix(remaining[..line_end].trim(), 16).unwrap_or(0);
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

#[test]
#[serial]
fn phantom_agents_cannot_appear_in_status() {
    let (_handle, url) = spawn_test_broker();

    let resp = http_request(
        &url,
        "POST",
        "/publish",
        r#"{"type":"agent.status","agent_id":"a","payload":{"status":"working","modified_files":[]}}"#,
    );
    assert_eq!(
        resp.status, 400,
        "publish should reject `a`; body: {}",
        resp.body
    );
    assert!(
        resp.body.contains("invalid agent_id"),
        "publish body should mention 'invalid agent_id'; got: {}",
        resp.body
    );

    let status_resp = http_request(&url, "GET", "/status", "");
    assert_eq!(
        status_resp.status, 200,
        "status should be 200; body: {}",
        status_resp.body
    );
    let json: serde_json::Value =
        serde_json::from_str(&status_resp.body).expect("status JSON parses");
    if let Some(agents) = json.get("agents").and_then(|v| v.as_array()) {
        for entry in agents {
            let id = entry.get("agent_id").and_then(|v| v.as_str()).unwrap_or("");
            assert_ne!(
                id, "a",
                "phantom `a` must NOT appear in /status; got: {}",
                status_resp.body
            );
        }
    }
}

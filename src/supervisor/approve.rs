//! Auto-approval keystroke dispatch.
//!
//! Implements the `automatic-approval` capability of the
//! `auto-approve-patterns` change: when the supervisor poll loop classifies
//! a stalled agent's prompt as safe, this module sends the
//! `BTab Down Enter` sequence to the pane via three separate
//! `tmux send-keys` invocations and publishes an `agent.status` audit log
//! entry to the broker before the keystrokes go out.
//!
//! The send-keys invoker is abstracted through [`KeyDispatcher`] so unit
//! tests can record argument vectors without spawning tmux.

use std::process::Command;

use crate::broker::publish::{build_status_message, publish_to_broker_http};
use crate::error::PawError;
use crate::supervisor::permission_prompt::PermissionType;

/// Keys (in tmux notation) sent to a pane to approve and remember a
/// permission prompt.
///
/// `BTab` focuses the prompt's "Yes, don't ask again" choice,
/// `Down` selects it, and `Enter` submits.
pub const APPROVAL_KEYS: &[&str] = &["BTab", "Down", "Enter"];

/// Abstraction over `tmux send-keys` so [`auto_approve_pane`] can be tested
/// without spawning tmux.
pub trait KeyDispatcher {
    /// Sends a single key (in tmux key-name notation) to the given pane.
    ///
    /// Returns the dispatch result; failures are surfaced to the caller so
    /// it can decide whether to log or abort.
    fn send_key(&mut self, session: &str, pane_index: usize, key: &str) -> std::io::Result<()>;
}

/// Production [`KeyDispatcher`] that shells out to `tmux send-keys`.
pub struct TmuxKeyDispatcher;

impl KeyDispatcher for TmuxKeyDispatcher {
    fn send_key(&mut self, session: &str, pane_index: usize, key: &str) -> std::io::Result<()> {
        let target = format!("{session}:0.{pane_index}");
        let status = Command::new("tmux")
            .args(["send-keys", "-t", &target, key])
            .status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "tmux send-keys exited with {status}"
            )));
        }
        Ok(())
    }
}

/// Inputs for [`auto_approve_pane`].
///
/// Bundled into a struct so the API has a single grow point as the spec
/// adds new fields (e.g. an optional reason string).
#[derive(Debug, Clone, Copy)]
pub struct ApprovalRequest<'a> {
    /// Whether `[supervisor.auto_approve] enabled` is `true`.
    pub enabled: bool,
    /// tmux session name (e.g. `"paw-myproject"`).
    pub session: &'a str,
    /// Pane index inside `session:0.<idx>` to receive the keystrokes.
    pub pane_index: usize,
    /// Agent ID for the audit log entry (slugified branch name).
    pub agent_id: &'a str,
    /// Classification of the detected prompt.
    pub kind: PermissionType,
    /// Whitelist entry that matched the captured command, included in the
    /// audit log when `Some`.
    pub matched_entry: Option<&'a str>,
    /// Broker URL for the audit log message; `None` skips logging.
    pub broker_url: Option<&'a str>,
}

/// Dispatches the auto-approval sequence and publishes an audit log entry.
///
/// Returns `Ok(true)` when the keystrokes were dispatched, `Ok(false)`
/// when the no-op rules apply (auto-approval disabled or class is
/// `Unknown`).
///
/// Order of operations is fixed:
///
/// 1. If `req.enabled == false` or `req.kind == Unknown`, return `Ok(false)`.
/// 2. Publish an `agent.status` message tagged `auto_approved` to the
///    broker. Failures are non-fatal — see the spec rationale.
/// 3. Send `BTab`, `Down`, `Enter` as three separate `send-keys` calls.
///
/// The `matched_entry` field is included in the audit log so the human
/// can see which whitelist entry triggered the approval.
pub fn auto_approve_pane<D: KeyDispatcher>(
    dispatcher: &mut D,
    req: ApprovalRequest<'_>,
) -> Result<bool, PawError> {
    if !req.enabled || req.kind == PermissionType::Unknown {
        return Ok(false);
    }

    // Publish the audit log BEFORE sending keystrokes so a crash mid-action
    // still leaves a trail.
    if let Some(url) = req.broker_url {
        let summary = req.matched_entry.map_or_else(
            || "auto_approved".to_string(),
            |e| format!("auto_approved: matched {e}"),
        );
        let msg = build_status_message(req.agent_id, "auto_approved", Some(summary));
        if let Err(e) = publish_to_broker_http(url, &msg) {
            eprintln!(
                "warning: failed to publish auto-approve status for {}: {e}",
                req.agent_id
            );
        }
    }

    for key in APPROVAL_KEYS {
        dispatcher
            .send_key(req.session, req.pane_index, key)
            .map_err(|e| PawError::TmuxError(format!("send-keys {key} failed: {e}")))?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recording dispatcher that captures every (session, pane, key) tuple
    /// instead of touching tmux.
    struct Recorder {
        events: Vec<(String, usize, String)>,
    }

    impl Recorder {
        fn new() -> Self {
            Self { events: Vec::new() }
        }
    }

    impl KeyDispatcher for Recorder {
        fn send_key(&mut self, session: &str, pane_index: usize, key: &str) -> std::io::Result<()> {
            self.events
                .push((session.to_string(), pane_index, key.to_string()));
            Ok(())
        }
    }

    fn req(
        enabled: bool,
        kind: PermissionType,
        matched_entry: Option<&str>,
    ) -> ApprovalRequest<'_> {
        ApprovalRequest {
            enabled,
            session: "paw-test",
            pane_index: 2,
            agent_id: "feat-foo",
            kind,
            matched_entry,
            broker_url: None,
        }
    }

    #[test]
    fn safe_prompt_dispatches_btab_down_enter_in_order() {
        let mut rec = Recorder::new();
        let fired = auto_approve_pane(
            &mut rec,
            req(true, PermissionType::Cargo, Some("cargo test")),
        )
        .unwrap();
        assert!(fired, "should fire when enabled and class is safe");
        let keys: Vec<&str> = rec.events.iter().map(|(_, _, k)| k.as_str()).collect();
        assert_eq!(keys, vec!["BTab", "Down", "Enter"]);
        for (s, p, _) in &rec.events {
            assert_eq!(s, "paw-test");
            assert_eq!(*p, 2);
        }
    }

    #[test]
    fn each_key_dispatched_separately() {
        let mut rec = Recorder::new();
        auto_approve_pane(&mut rec, req(true, PermissionType::Curl, None)).unwrap();
        // Three distinct invocations (no concatenated string).
        assert_eq!(rec.events.len(), 3);
    }

    #[test]
    fn disabled_config_is_noop() {
        let mut rec = Recorder::new();
        let fired = auto_approve_pane(
            &mut rec,
            req(false, PermissionType::Cargo, Some("cargo test")),
        )
        .unwrap();
        assert!(!fired);
        assert!(rec.events.is_empty(), "disabled => no keystrokes");
    }

    #[test]
    fn unknown_class_is_noop() {
        let mut rec = Recorder::new();
        let fired = auto_approve_pane(&mut rec, req(true, PermissionType::Unknown, None)).unwrap();
        assert!(!fired);
        assert!(rec.events.is_empty(), "Unknown => no keystrokes");
    }

    #[test]
    fn approval_keys_constant_matches_spec() {
        // Spec scenario: "Default Claude approval sequence" requires
        // BTab Down Enter in order, sent via tmux send-keys.
        assert_eq!(APPROVAL_KEYS, &["BTab", "Down", "Enter"]);
    }

    /// Spec scenario `auto-approve-patterns/automatic-approval` —
    /// "Approval emits broker message": when `auto_approve_pane` fires for
    /// a safe-class prompt, an `agent.status` message tagged
    /// `auto_approved` MUST be published to the broker BEFORE any
    /// `send-keys` keystrokes are dispatched. This test stands up a
    /// localhost TCP listener that accepts the broker's `/publish` request,
    /// records the receive time relative to the recorded keystroke times,
    /// and asserts the audit log entry preceded every key.
    #[test]
    #[allow(clippy::items_after_statements)]
    fn broker_audit_message_published_before_keystrokes() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::{Arc, Mutex};
        use std::time::Instant;

        // Shared timeline: each event records a label and the moment it
        // was observed. The test asserts the broker event comes first.
        #[derive(Debug)]
        #[allow(dead_code)] // Fields are read via Debug formatting in failure messages.
        enum Event {
            Published(String),
            Key(String),
        }

        // Recording dispatcher that timestamps each key as it arrives.
        struct TimedRecorder {
            timeline: Arc<Mutex<Vec<(Instant, Event)>>>,
        }
        impl KeyDispatcher for TimedRecorder {
            fn send_key(
                &mut self,
                _session: &str,
                _pane_index: usize,
                key: &str,
            ) -> std::io::Result<()> {
                self.timeline
                    .lock()
                    .unwrap()
                    .push((Instant::now(), Event::Key(key.to_string())));
                Ok(())
            }
        }

        // Bind a real listener on an ephemeral port so we can drive
        // publish_to_broker_http end-to-end without depending on the
        // production broker.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("local_addr");

        let timeline: Arc<Mutex<Vec<(Instant, Event)>>> = Arc::new(Mutex::new(Vec::new()));

        // Spawn a thread that accepts a single connection, reads the HTTP
        // request, replies 202, and pushes a Published event onto the
        // timeline keyed on the moment the body was received.
        let server_timeline = Arc::clone(&timeline);
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 4096];
            // We only need enough bytes to confirm the body arrived.
            let n = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            server_timeline
                .lock()
                .unwrap()
                .push((Instant::now(), Event::Published(request.clone())));
            let _ = stream.write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n");
            let _ = stream.flush();
            request
        });

        let mut dispatcher = TimedRecorder {
            timeline: Arc::clone(&timeline),
        };
        let broker_url = format!("http://{addr}");
        let req = ApprovalRequest {
            enabled: true,
            session: "paw-test",
            pane_index: 0,
            agent_id: "feat-foo",
            kind: PermissionType::Cargo,
            matched_entry: Some("cargo test"),
            broker_url: Some(&broker_url),
        };
        let fired = auto_approve_pane(&mut dispatcher, req).expect("auto_approve_pane");
        assert!(fired, "safe class with enabled=true must fire");

        // Wait for the server thread so the Published event is recorded.
        let request = server.join().expect("server thread");

        // The broker received an HTTP POST whose body identifies an
        // auto_approved status for the agent.
        assert!(
            request.contains("POST /publish"),
            "expected a /publish request, got: {request}"
        );
        assert!(
            request.contains("auto_approved"),
            "expected auto_approved tag in body, got: {request}"
        );
        assert!(
            request.contains("feat-foo"),
            "expected agent_id in body, got: {request}"
        );

        // The Published event must precede every Key event in the timeline.
        let events = timeline.lock().unwrap();
        let publish_idx = events
            .iter()
            .position(|(_, e)| matches!(e, Event::Published(_)))
            .expect("publish event recorded");
        let first_key_idx = events
            .iter()
            .position(|(_, e)| matches!(e, Event::Key(_)))
            .expect("key event recorded");
        assert!(
            publish_idx < first_key_idx,
            "audit message must be published BEFORE keystrokes; timeline: {events:?}"
        );

        // Sanity: all three keys were dispatched in order.
        let keys: Vec<String> = events
            .iter()
            .filter_map(|(_, e)| match e {
                Event::Key(k) => Some(k.clone()),
                Event::Published(_) => None,
            })
            .collect();
        assert_eq!(keys, vec!["BTab", "Down", "Enter"]);
    }
}

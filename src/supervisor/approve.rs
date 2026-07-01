//! Auto-approval keystroke dispatch.
//!
//! Implements the `automatic-approval` capability: when the supervisor poll
//! loop classifies a stalled agent's prompt as safe and LIVE, this module
//! selects the prompt's option index, types the option digit followed by
//! `Enter` via two separate `tmux send-keys` invocations, and publishes an
//! `agent.status` audit log entry to the broker before the keystrokes go out.
//!
//! Digit selection is deliberate: a blind `Down`+`Enter` selects the wrong
//! row on a 2-option `Yes`/`No` prompt (it lands on `No`). Typing the option
//! number is unambiguous across 2- and 3-option shapes.
//!
//! The send-keys invoker is abstracted through [`KeyDispatcher`] so unit
//! tests can record argument vectors without spawning tmux.

use std::process::Command;

use crate::broker::publish::{build_status_message, publish_to_broker_http};
use crate::error::PawError;
use crate::supervisor::approval_gate::{
    GateOutcome, PaneCapturer, SUPERVISOR_PANE_INDEX, approval_send_gate,
};
use crate::supervisor::permission_prompt::PermissionType;

/// Keystrokes (in tmux notation) that select the 1-based `option_index` at a
/// permission prompt: type the option digit, then submit with `Enter`.
#[must_use]
pub fn approval_keystrokes(option_index: u8) -> [String; 2] {
    [option_index.to_string(), "Enter".to_string()]
}

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
    /// Whether the prompt is LIVE (footer `Esc to cancel` in the last ~4
    /// non-blank lines). A non-live prompt SHALL NOT receive keystrokes.
    pub live_prompt: bool,
    /// 1-based option index to select (per the prompt shape and broad-grant
    /// rule). Typed as the leading keystroke before `Enter`.
    pub option_index: u8,
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
/// 1. If `req.enabled == false`, `req.kind == Unknown`, or
///    `req.live_prompt == false`, return `Ok(false)` (no keystrokes). The
///    live-prompt check is a hard precondition — a non-live *detection*
///    capture is never acted on.
/// 2. If `req.pane_index` is 0 (the supervisor's own pane), return `Ok(false)`
///    with no keystrokes — pane 0 is excluded from the blind send-keys path
///    (`broker-mediated-approvals`). This is checked before the audit so a
///    refused send is never logged as approved.
/// 3. Publish an `agent.status` message tagged `auto_approved` to the
///    broker. Failures are non-fatal — see the spec rationale. Kept BEFORE
///    the re-confirm capture so no broker round-trip sits between the
///    re-confirm and the send.
/// 4. Pass the keystrokes through [`approval_send_gate`], which re-captures the
///    pane immediately before the send and confirms a live prompt is still in
///    the tail; only then are the option digit and `Enter` dispatched as two
///    separate `send-keys` calls (see [`approval_keystrokes`]). When the prompt
///    cleared between detection and send, the gate dispatches nothing and this
///    returns `Ok(false)`.
///
/// The `matched_entry` field is included in the audit log so the human
/// can see which whitelist entry triggered the approval.
pub fn auto_approve_pane<C: PaneCapturer, D: KeyDispatcher>(
    capturer: &C,
    dispatcher: &mut D,
    req: ApprovalRequest<'_>,
) -> Result<bool, PawError> {
    if !req.enabled || req.kind == PermissionType::Unknown || !req.live_prompt {
        return Ok(false);
    }

    // Pane 0 (the supervisor's own pane) is excluded from the blind send-keys
    // path. Refuse before the audit so a send we will not make is never logged
    // as approved; the prompt is left for the non-blind supervisor-pane path.
    if req.pane_index == SUPERVISOR_PANE_INDEX {
        return Ok(false);
    }

    // Publish the audit log BEFORE the re-confirm+send so a crash mid-action
    // still leaves a trail. Kept before the gate's re-confirm capture so no
    // broker round-trip sits between the re-confirm and the send.
    if let Some(url) = req.broker_url {
        let summary = req.matched_entry.map_or_else(
            || "auto_approved".to_string(),
            |e| format!("auto_approved: matched {e}"),
        );
        let msg = build_status_message(req.agent_id, "auto_approved", Some(summary), None);
        if let Err(e) = publish_to_broker_http(url, &msg) {
            eprintln!(
                "warning: failed to publish auto-approve status for {}: {e}",
                req.agent_id
            );
        }
    }

    // Re-confirm a live prompt with a fresh capture immediately before the
    // send. A cleared prompt dispatches nothing — no stray input.
    let keys = approval_keystrokes(req.option_index);
    match approval_send_gate(capturer, dispatcher, req.session, req.pane_index, &keys)? {
        GateOutcome::Sent => Ok(true),
        GateOutcome::PromptCleared | GateOutcome::Pane0Excluded => Ok(false),
    }
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

    /// Capturer returning a fixed capture, so the approval-send gate's
    /// re-confirm step sees a deterministic pane state without tmux.
    struct StubCapturer(String);
    impl StubCapturer {
        /// A capture whose tail carries a live permission-prompt marker.
        fn live() -> Self {
            Self("Bash command\n  cargo test\nDo you want to proceed?".to_string())
        }
        /// A capture with no permission-prompt marker (the prompt cleared).
        fn cleared() -> Self {
            Self("$ \nagent moved on\nall done\n".to_string())
        }
    }
    impl PaneCapturer for StubCapturer {
        fn capture(&self, _session: &str, _pane_index: usize) -> String {
            self.0.clone()
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
            live_prompt: true,
            option_index: 1,
            broker_url: None,
        }
    }

    #[test]
    fn safe_prompt_types_option_digit_then_enter() {
        let mut rec = Recorder::new();
        let fired = auto_approve_pane(
            &StubCapturer::live(),
            &mut rec,
            req(true, PermissionType::Cargo, Some("cargo test")),
        )
        .unwrap();
        assert!(fired, "should fire when enabled, safe, and live");
        let keys: Vec<&str> = rec.events.iter().map(|(_, _, k)| k.as_str()).collect();
        assert_eq!(keys, vec!["1", "Enter"], "option 1 => type `1` then Enter");
        for (s, p, _) in &rec.events {
            assert_eq!(s, "paw-test");
            assert_eq!(*p, 2);
        }
    }

    #[test]
    fn broad_grant_selects_option_two() {
        let mut rec = Recorder::new();
        let request = ApprovalRequest {
            option_index: 2,
            ..req(true, PermissionType::SafeCommand, Some("git"))
        };
        auto_approve_pane(&StubCapturer::live(), &mut rec, request).unwrap();
        let keys: Vec<&str> = rec.events.iter().map(|(_, _, k)| k.as_str()).collect();
        assert_eq!(keys, vec!["2", "Enter"], "option 2 => type `2` then Enter");
    }

    #[test]
    fn each_key_dispatched_separately() {
        let mut rec = Recorder::new();
        auto_approve_pane(
            &StubCapturer::live(),
            &mut rec,
            req(true, PermissionType::Curl, None),
        )
        .unwrap();
        // Two distinct invocations (digit + Enter), no concatenated string.
        assert_eq!(rec.events.len(), 2);
    }

    #[test]
    fn disabled_config_is_noop() {
        let mut rec = Recorder::new();
        let fired = auto_approve_pane(
            &StubCapturer::live(),
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
        let fired = auto_approve_pane(
            &StubCapturer::live(),
            &mut rec,
            req(true, PermissionType::Unknown, None),
        )
        .unwrap();
        assert!(!fired);
        assert!(rec.events.is_empty(), "Unknown => no keystrokes");
    }

    /// Spec scenario "Footer absent does not fire" / live-prompt precondition:
    /// a non-live prompt receives no keystrokes even when the class is safe.
    #[test]
    fn non_live_prompt_is_noop() {
        let mut rec = Recorder::new();
        let request = ApprovalRequest {
            live_prompt: false,
            ..req(true, PermissionType::SafeCommand, Some("cargo test"))
        };
        let fired = auto_approve_pane(&StubCapturer::live(), &mut rec, request).unwrap();
        assert!(!fired, "non-live prompt must not fire");
        assert!(rec.events.is_empty(), "non-live => no keystrokes");
    }

    /// Spec scenario `automatic-approval` "Cleared prompt suppresses the
    /// keystroke sequence": a prompt that classified safe and live at detection
    /// time but has cleared by the re-confirm capture receives NO keystrokes.
    #[test]
    fn cleared_prompt_suppresses_keystrokes_via_auto_approve() {
        let mut rec = Recorder::new();
        let fired = auto_approve_pane(
            &StubCapturer::cleared(),
            &mut rec,
            req(true, PermissionType::Cargo, Some("cargo test")),
        )
        .unwrap();
        assert!(!fired, "cleared prompt must not fire");
        assert!(
            rec.events.is_empty(),
            "cleared prompt => no stray keystrokes into the CLI"
        );
    }

    /// Spec scenario `automatic-approval` "Auto-approval never types into
    /// pane 0": a safe, live, enabled request whose resolved target pane index
    /// is 0 receives NO keystrokes via the blind send-keys path.
    #[test]
    fn pane_zero_is_never_auto_approved() {
        let mut rec = Recorder::new();
        let request = ApprovalRequest {
            pane_index: 0,
            ..req(true, PermissionType::SafeCommand, Some("cargo test"))
        };
        let fired = auto_approve_pane(&StubCapturer::live(), &mut rec, request).unwrap();
        assert!(!fired, "pane 0 must never be blind-approved");
        assert!(rec.events.is_empty(), "pane 0 => no keystrokes");
    }

    #[test]
    fn approval_keystrokes_are_digit_then_enter() {
        assert_eq!(
            approval_keystrokes(1),
            ["1".to_string(), "Enter".to_string()]
        );
        assert_eq!(
            approval_keystrokes(2),
            ["2".to_string(), "Enter".to_string()]
        );
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
            pane_index: 2,
            agent_id: "feat-foo",
            kind: PermissionType::Cargo,
            matched_entry: Some("cargo test"),
            live_prompt: true,
            option_index: 1,
            broker_url: Some(&broker_url),
        };
        let fired = auto_approve_pane(&StubCapturer::live(), &mut dispatcher, req)
            .expect("auto_approve_pane");
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

        // Sanity: the option digit then Enter were dispatched in order.
        let keys: Vec<String> = events
            .iter()
            .filter_map(|(_, e)| match e {
                Event::Key(k) => Some(k.clone()),
                Event::Published(_) => None,
            })
            .collect();
        assert_eq!(keys, vec!["1", "Enter"]);
    }
}

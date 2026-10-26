//! `broker-agent-helper`'s *Helper publish subcommands* requirement, for the
//! two defects this change fixes:
//!
//! 1. `broker.sh intent <summary> <files>` with no TTL omitted
//!    `valid_for_seconds` — a REQUIRED field of `agent.intent` — so `/publish`
//!    answered `400`. The helper now sends the documented default of `900`.
//! 2. Both helpers' `publish()` used `curl -s` without checking the status, so
//!    a rejected publish was silent and the helper still exited `0`. A publish
//!    the broker does not accept now prints the broker's error to stderr and
//!    exits non-zero.
//!
//! Both are driven against a REAL broker (`git_paw::broker::start_broker`), so
//! the assertions are about what the broker actually accepted, not about a
//! stub's idea of the wire shape.
//!
//! `supervisor-learnings`' *sweep.sh learn ids follow the aggregator's
//! deterministic-id scheme* is covered here too, as a parity test against
//! `LearningRecord::deterministic_id`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Output};
use std::time::{Duration, SystemTime};

use serial_test::serial;
use tempfile::TempDir;

use git_paw::broker::learnings::LearningRecord;

fn init_git_repo(dir: &Path) {
    let run = |args: &[&str]| {
        StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git command");
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "t@e.st"]);
    run(&["config", "user.name", "Test"]);
    fs::write(dir.join("README.md"), "x").expect("readme");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);
}

/// Copies the bundled helpers (and the preamble they source) into
/// `<repo>/.git-paw/scripts/`, mirroring `git paw init`'s co-deployment.
fn install_helpers(repo: &Path) -> (PathBuf, PathBuf) {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts");
    let dst = repo.join(".git-paw/scripts");
    fs::create_dir_all(&dst).expect("mk scripts dir");
    for name in ["_paw_common.sh", "broker.sh", "sweep.sh"] {
        fs::copy(assets.join(name), dst.join(name)).unwrap_or_else(|e| panic!("copy {name}: {e}"));
    }
    (dst.join("broker.sh"), dst.join("sweep.sh"))
}

fn write_broker_config(repo: &Path, port: u16) {
    fs::create_dir_all(repo.join(".git-paw")).expect("mk .git-paw");
    fs::write(
        repo.join(".git-paw/config.toml"),
        format!("[broker]\nenabled = true\nport = {port}\n"),
    )
    .expect("write config.toml");
}

fn pick_broker_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read local addr")
        .port()
}

/// Starts a real broker on a free port, retrying on a port race.
fn spawn_broker() -> (git_paw::broker::BrokerHandle, u16) {
    let mut attempts = 0;
    loop {
        let port = pick_broker_port();
        let config = git_paw::config::BrokerConfig {
            enabled: true,
            port,
            bind: "127.0.0.1".to_string(),
            ..Default::default()
        };
        let state = git_paw::broker::BrokerState::new(None)
            .with_authority(Some(TEST_AUTHORITY.to_string()));
        match git_paw::broker::start_broker(&config, state, Vec::new()) {
            Ok(handle) => return (handle, port),
            Err(_) if attempts < 10 => attempts += 1,
            Err(e) => panic!("failed to start test broker after retries: {e}"),
        }
    }
}

fn run(script: &Path, repo: &Path, args: &[&str]) -> Output {
    let mut cmd = StdCommand::new("bash");
    cmd.arg(script)
        .args(args)
        .current_dir(repo)
        .env_remove("TMUX");
    let _home = with_root_session(&mut cmd, repo);
    cmd.output().expect("run helper")
}

/// The authority credential the test session holds. `sweep.sh`'s verdict
/// verbs (`verified`, `feedback-gate`, `conflict-ack`) fetch it through
/// `git paw __authority` and send it as `X-Paw-Authority`.
const TEST_AUTHORITY: &str = "test-authority";

/// Points `cmd` at a temp `HOME` holding a session for `repo` with
/// [`TEST_AUTHORITY`], with the built binary first on `PATH` so `git paw`
/// resolves to it. The returned dir must outlive the command.
fn with_root_session(cmd: &mut StdCommand, repo: &Path) -> TempDir {
    let home = TempDir::new().expect("home");
    let sessions = if cfg!(target_os = "macos") {
        home.path()
            .join("Library/Application Support/git-paw/sessions")
    } else {
        home.path().join(".local/share/git-paw/sessions")
    };
    let session = git_paw::session::Session {
        session_name: "paw-helper-test".to_string(),
        repo_path: git_paw::git::validate_repo(repo).expect("validate repo"),
        project_name: "helper-test".to_string(),
        created_at: std::time::SystemTime::UNIX_EPOCH,
        status: git_paw::session::SessionStatus::Active,
        worktrees: Vec::new(),
        broker_port: None,
        broker_bind: None,
        broker_log_path: None,
        mode: git_paw::session::SessionMode::Bare,
        dashboard_pane: None,
        layout: None,
        broker_authority: Some(TEST_AUTHORITY.to_string()),
    };
    git_paw::session::save_session_in(&session, &sessions).expect("save session");
    let bin_dir = Path::new(env!("CARGO_BIN_EXE_git-paw"))
        .parent()
        .expect("binary dir")
        .to_path_buf();
    cmd.env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin_dir.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        );
    home
}

/// Fetches `GET /log?since=0` from the broker over raw TCP and returns the
/// response body.
fn fetch_log(port: u16) -> String {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let addr = format!("127.0.0.1:{port}");
    let mut stream = TcpStream::connect(&addr).expect("connect to broker");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let request = format!("GET /log?since=0 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).expect("write request");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("read response");
    raw.split_once("\r\n\r\n")
        .map_or_else(String::new, |(_, body)| body.to_string())
}

/// Spec scenario "intent without a TTL is accepted by the broker": the
/// published `agent.intent` carries `valid_for_seconds = 900` and the broker
/// accepts it (the helper exits 0 only when it did).
#[test]
#[serial]
fn intent_without_a_ttl_sends_the_default_and_is_accepted() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let (broker_sh, _) = install_helpers(repo.path());
    let (handle, port) = spawn_broker();
    write_broker_config(repo.path(), port);

    let out = run(
        &broker_sh,
        repo.path(),
        &["--agent", "feat-x", "intent", "wire auth", "src/auth.rs"],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "a TTL-less intent must be accepted by the broker; stderr:\n{stderr}"
    );

    let log = fetch_log(port);
    let parsed: serde_json::Value = serde_json::from_str(&log).expect("parse /log body");
    let entries = parsed["entries"]
        .as_array()
        .expect("/log carries an entries array");
    let intent = entries
        .iter()
        .find(|e| e["message"]["type"] == "agent.intent")
        .unwrap_or_else(|| panic!("no agent.intent reached the broker; log:\n{log}"));
    assert_eq!(
        intent["message"]["payload"]["valid_for_seconds"], 900,
        "a TTL-less intent must carry the default valid_for_seconds; log:\n{log}"
    );

    drop(handle);
}

/// An explicit TTL still wins over the default.
#[test]
#[serial]
fn intent_with_an_explicit_ttl_sends_that_ttl() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let (broker_sh, _) = install_helpers(repo.path());
    let (handle, port) = spawn_broker();
    write_broker_config(repo.path(), port);

    let out = run(
        &broker_sh,
        repo.path(),
        &[
            "--agent",
            "feat-x",
            "intent",
            "wire auth",
            "src/auth.rs",
            "60",
        ],
    );
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let log = fetch_log(port);
    let parsed: serde_json::Value = serde_json::from_str(&log).expect("parse /log body");
    let intent = parsed["entries"]
        .as_array()
        .expect("entries array")
        .iter()
        .find(|e| e["message"]["type"] == "agent.intent")
        .unwrap_or_else(|| panic!("no agent.intent reached the broker; log:\n{log}"));
    assert_eq!(intent["message"]["payload"]["valid_for_seconds"], 60);

    drop(handle);
}

/// Spec scenario "A rejected publish is not silent", for `broker.sh`: the
/// broker rejects an invalid `agent_id` with `400`, so the helper must exit
/// non-zero and surface the broker's reason.
#[test]
#[serial]
fn broker_sh_publish_rejected_by_the_broker_exits_non_zero() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let (broker_sh, _) = install_helpers(repo.path());
    let (handle, port) = spawn_broker();
    write_broker_config(repo.path(), port);

    // `a` is not a valid broker agent id (`supervisor`, or a
    // `<prefix>-<name>` / `<prefix>/<name>` slug).
    let out = run(
        &broker_sh,
        repo.path(),
        &["--agent", "a", "status", "booting"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a publish the broker rejected must not exit 0; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("agent_id"),
        "the broker's error must reach stderr; stderr:\n{stderr}"
    );

    drop(handle);
}

/// Spec scenario "A rejected publish is not silent", for `sweep.sh`: the
/// `feedback-gate` verb addresses the message at the target agent, so an
/// invalid target id is rejected by the broker and must not be reported as
/// delivered.
#[test]
#[serial]
fn sweep_sh_publish_rejected_by_the_broker_exits_non_zero() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let (_, sweep_sh) = install_helpers(repo.path());
    let (handle, port) = spawn_broker();
    write_broker_config(repo.path(), port);

    let out = run(
        &sweep_sh,
        repo.path(),
        &["feedback-gate", "a", "testing", "a test failed"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a publish the broker rejected must not exit 0; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stdout.contains("feedback (testing)"),
        "a rejected publish must not print the delivered line; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("agent_id"),
        "the broker's error must reach stderr; stderr:\n{stderr}"
    );

    drop(handle);
}

/// A publish with NO broker listening at all is equally loud: the helper
/// reports the failure and exits non-zero rather than claiming success.
#[test]
#[serial]
fn publish_with_no_broker_listening_exits_non_zero() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let (broker_sh, _) = install_helpers(repo.path());
    // A port nothing is listening on: bind it, read the number, drop it.
    write_broker_config(repo.path(), pick_broker_port());

    let out = run(
        &broker_sh,
        repo.path(),
        &["--agent", "feat-x", "status", "booting"],
    );
    assert!(
        !out.status.success(),
        "a publish with no broker must not exit 0; stdout:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("broker.sh"),
        "the failure must be reported by the helper on stderr; stderr:\n{stderr}"
    );
}

/// Runs `sweep.sh learn` and returns the `id` of the published
/// `agent.learning`, read back from the broker's log.
fn learn_id(
    repo: &Path,
    sweep_sh: &Path,
    port: u16,
    category: &str,
    title: &str,
    body: &str,
) -> String {
    let out = run(sweep_sh, repo, &["learn", category, title, body]);
    assert!(
        out.status.success(),
        "learn should publish; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let log = fetch_log(port);
    let parsed: serde_json::Value = serde_json::from_str(&log).expect("parse /log body");
    parsed["entries"]
        .as_array()
        .expect("entries array")
        .iter()
        .rfind(|e| e["message"]["type"] == "agent.learning")
        .unwrap_or_else(|| panic!("no agent.learning reached the broker; log:\n{log}"))["message"]
        ["payload"]["id"]
        .as_str()
        .expect("learning id is a string")
        .to_string()
}

/// Spec scenario "learn's id matches the aggregator's id for the same
/// record": the helper and `LearningRecord::deterministic_id` assign the same
/// id to the same logical record.
///
/// The hour bucket is wall-clock, so the Rust side is computed at instants
/// bracketing the helper's own call: the ids must agree for one of them (they
/// differ only if the UTC hour rolled over mid-test, in which case either
/// bucket is a correct answer for the helper).
#[test]
#[serial]
fn learn_id_matches_the_aggregator_deterministic_id() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let (_, sweep_sh) = install_helpers(repo.path());
    let (handle, port) = spawn_broker();
    write_broker_config(repo.path(), port);

    // Nested objects, an array, unicode, and integers — the canonical-form
    // cases most likely to drift between the Rust and Python serialisers.
    let body_json = r#"{"friction":"git commit re-prompts — déjà vu","occurrences":3,"panes":[2,1],"detail":{"z":true,"a":null}}"#;
    let body: serde_json::Value = serde_json::from_str(body_json).expect("valid body json");

    let before = SystemTime::now();
    let helper_id = learn_id(
        repo.path(),
        &sweep_sh,
        port,
        "tooling_friction",
        "git commit re-prompts",
        body_json,
    );
    let after = SystemTime::now();

    let rust_id = |at: SystemTime| {
        LearningRecord {
            category: "tooling_friction".to_string(),
            agent_id: "supervisor".to_string(),
            branch_id: None,
            title: "git commit re-prompts".to_string(),
            body: body.clone(),
            timestamp: at,
        }
        .deterministic_id()
    };

    assert!(
        helper_id == rust_id(before) || helper_id == rust_id(after),
        "sweep.sh learn id {helper_id} must equal the aggregator's id \
         ({} / {})",
        rust_id(before),
        rust_id(after)
    );

    drop(handle);
}

/// Spec scenario "The title does not change the id": the title is not part of
/// the canonical form, so two phrasings of the same record dedupe to one id.
///
/// The id's canonical form carries the UTC HOUR bucket, so two calls that
/// straddle an hour boundary legitimately differ. The pair is therefore
/// retried while the hour observed before and after it differ, pinning the
/// comparison to a single bucket instead of racing the clock.
#[test]
#[serial]
fn learn_id_is_independent_of_the_title() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let (_, sweep_sh) = install_helpers(repo.path());
    let (handle, port) = spawn_broker();
    write_broker_config(repo.path(), port);

    let body = r#"{"friction":"approval prompts","occurrences":2}"#;
    let hour = || chrono::Utc::now().format("%Y-%m-%dT%H").to_string();

    let mut pair = None;
    for _ in 0..3 {
        let before = hour();
        let first = learn_id(
            repo.path(),
            &sweep_sh,
            port,
            "tooling_friction",
            "approval prompts",
            body,
        );
        let second = learn_id(
            repo.path(),
            &sweep_sh,
            port,
            "tooling_friction",
            "a completely different title",
            body,
        );
        if hour() == before {
            pair = Some((first, second));
            break;
        }
    }
    let (first, second) =
        pair.expect("three attempts should land both publishes inside one UTC hour");

    assert_eq!(
        first, second,
        "the title must not enter the id's canonical form"
    );

    drop(handle);
}

/// The `learn` id must hash a canonical form that renders byte-identically on
/// both sides. Python and the aggregator disagree on exponent-form floats
/// (`1e+16` / `1e-05` against `1e16` / `1e-5`), and an integer beyond the
/// 64-bit range is a float on the aggregator side and an exact int here, so
/// such a body is REJECTED with a clear message rather than published under an
/// id the aggregator would not reproduce.
#[test]
#[serial]
fn learn_rejects_numbers_whose_canonical_form_would_not_match() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let (_, sweep_sh) = install_helpers(repo.path());
    let (handle, port) = spawn_broker();
    write_broker_config(repo.path(), port);

    for (label, body) in [
        ("a plain float", r#"{"ratio":0.5}"#),
        ("an exponent-form float", r#"{"ratio":1e16}"#),
        ("a small exponent-form float", r#"{"ratio":1e-5}"#),
        ("a nested float", r#"{"detail":{"ratio":2.5}}"#),
        ("a float inside an array", r#"{"samples":[1,2.5]}"#),
        (
            "an integer beyond 64 bits",
            r#"{"occurrences":99999999999999999999999}"#,
        ),
    ] {
        let out = run(
            &sweep_sh,
            repo.path(),
            &["learn", "tooling_friction", "t", body],
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success(),
            "{label} must be rejected; stdout:\n{stdout}\nstderr:\n{stderr}"
        );
        assert!(
            stderr.contains("64-bit integer") || stderr.contains("non-integer number"),
            "{label} must be refused with a clear message; stderr:\n{stderr}"
        );
        assert!(
            !stdout.contains("learning published"),
            "{label} must not report a publish; stdout:\n{stdout}"
        );
    }

    // The log must carry nothing at all: every body above was refused.
    let log = fetch_log(port);
    let parsed: serde_json::Value = serde_json::from_str(&log).expect("parse /log body");
    assert!(
        !parsed["entries"]
            .as_array()
            .expect("entries array")
            .iter()
            .any(|e| e["message"]["type"] == "agent.learning"),
        "a rejected learn must publish nothing; log:\n{log}"
    );

    // 64-bit integers, booleans, nulls, strings and nested containers still
    // publish — the restriction is numeric, not structural.
    let ok = run(
        &sweep_sh,
        repo.path(),
        &[
            "learn",
            "tooling_friction",
            "t",
            r#"{"occurrences":9223372036854775807,"ok":true,"none":null,"nested":{"xs":[1,2]}}"#,
        ],
    );
    assert!(
        ok.status.success(),
        "an all-integer body must still publish; stderr:\n{}",
        String::from_utf8_lossy(&ok.stderr)
    );

    drop(handle);
}

/// `conflict-ack` publishes TWICE — one `agent.feedback` per agent in the
/// pair — inside a loop. An unguarded publish there printed the broker's
/// error twice, then the `conflict-ack:` success line, and exited 0: the
/// supervisor would record the pair as acknowledged while neither agent had
/// been told, so the conflict keeps re-escalating. Both publishes are
/// guarded, so a rejecting broker is reported and nothing claims success.
#[test]
#[serial]
fn conflict_ack_rejected_by_the_broker_exits_non_zero() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let (_, sweep_sh) = install_helpers(repo.path());
    let (handle, port) = spawn_broker();
    write_broker_config(repo.path(), port);

    // `a` is not a valid broker agent id, so the broker answers 400.
    let out = run(
        &sweep_sh,
        repo.path(),
        &["conflict-ack", "a", "feat-y", "src/a.rs"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a rejected conflict-ack must not exit 0; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("agent_id"),
        "the broker's error must reach stderr; stderr:\n{stderr}"
    );
    assert!(
        !stdout.contains("conflict-ack:"),
        "a rejected conflict-ack must not print its success line; stdout:\n{stdout}"
    );

    drop(handle);
}

/// The same, with no broker listening at all: `conflict-ack` must stop at the
/// FIRST failed publish rather than reporting the pair as acknowledged.
#[test]
#[serial]
fn conflict_ack_with_no_broker_listening_exits_non_zero() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let (_, sweep_sh) = install_helpers(repo.path());
    write_broker_config(repo.path(), pick_broker_port());

    let out = run(
        &sweep_sh,
        repo.path(),
        &["conflict-ack", "feat-x", "feat-y", "src/a.rs"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "conflict-ack with no broker must not exit 0; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("sweep.sh"),
        "the helper must report the failure on stderr; stderr:\n{stderr}"
    );
    assert!(
        !stdout.contains("conflict-ack:"),
        "no success line may be printed; stdout:\n{stdout}"
    );
    assert_eq!(
        stderr.matches("no response from the broker").count(),
        1,
        "it must stop at the first failed publish, not attempt both; stderr:\n{stderr}"
    );
}

/// A negative zero is the subtle member of the rejected-number family: JSON's
/// `-0` parses to Python's int `0` and renders `0`, while the aggregator
/// renders it `-0.0`. The sign survives only in the RAW literal, so the helper
/// inspects that. This test pins BOTH halves — serde's rendering (the premise)
/// and the helper's refusal (the behaviour).
#[test]
#[serial]
fn learn_rejects_a_negative_zero() {
    // The premise: the aggregator's canonical form renders `-0` with a sign
    // and a fractional part, which Python's int cannot reproduce.
    let minus_zero: serde_json::Value = serde_json::from_str("-0").expect("-0 is valid JSON");
    assert_eq!(
        minus_zero.to_string(),
        "-0.0",
        "premise: the aggregator renders -0 distinctly from 0"
    );

    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let (_, sweep_sh) = install_helpers(repo.path());
    let (handle, port) = spawn_broker();
    write_broker_config(repo.path(), port);

    for (label, body) in [
        ("a top-level negative zero", r#"{"delta":-0}"#),
        ("a nested negative zero", r#"{"detail":{"delta":-0}}"#),
        ("a negative zero in an array", r#"{"deltas":[1,-0]}"#),
    ] {
        let out = run(
            &sweep_sh,
            repo.path(),
            &["learn", "tooling_friction", "t", body],
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success(),
            "{label} must be rejected; stdout:\n{stdout}\nstderr:\n{stderr}"
        );
        assert!(
            stderr.contains("negative zero"),
            "{label} must be refused by name; stderr:\n{stderr}"
        );
        assert!(
            !stdout.contains("learning published"),
            "{label} must not report a publish; stdout:\n{stdout}"
        );
    }

    // A plain zero is fine — only the signed spelling is ambiguous.
    let ok = run(
        &sweep_sh,
        repo.path(),
        &["learn", "tooling_friction", "t", r#"{"delta":0}"#],
    );
    assert!(
        ok.status.success(),
        "an unsigned zero must still publish; stderr:\n{}",
        String::from_utf8_lossy(&ok.stderr)
    );

    drop(handle);
}

/// A `\udXXX` escape parses to a lone surrogate, which is not encodable as
/// UTF-8: hashing the canonical form used to die with a raw Python
/// `UnicodeEncodeError` traceback. It must be a clean diagnostic and a
/// non-zero exit instead — and nothing may be published, since the aggregator
/// (whose strings are always valid UTF-8) could never hold such a record.
#[test]
#[serial]
fn learn_rejects_a_lone_surrogate_with_a_clean_diagnostic() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let (_, sweep_sh) = install_helpers(repo.path());
    let (handle, port) = spawn_broker();
    write_broker_config(repo.path(), port);

    for (label, body) in [
        (
            "a lone high surrogate in a value",
            r#"{"friction":"\ud800"}"#,
        ),
        (
            "a lone low surrogate in a value",
            r#"{"friction":"\udfff"}"#,
        ),
        ("a lone surrogate in a key", r#"{"\ud800":"x"}"#),
        ("a lone surrogate nested", r#"{"detail":{"f":"\ud800"}}"#),
        ("a lone surrogate in an array", r#"{"xs":["ok","\ud800"]}"#),
    ] {
        let out = run(
            &sweep_sh,
            repo.path(),
            &["learn", "tooling_friction", "t", body],
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success(),
            "{label} must be rejected; stdout:\n{stdout}\nstderr:\n{stderr}"
        );
        assert!(
            stderr.contains("not valid UTF-8"),
            "{label} must be refused by name; stderr:\n{stderr}"
        );
        assert!(
            !stderr.contains("Traceback"),
            "{label} must not surface a Python traceback; stderr:\n{stderr}"
        );
        assert!(
            !stdout.contains("learning published"),
            "{label} must not report a publish; stdout:\n{stdout}"
        );
    }

    // A well-formed surrogate PAIR is ordinary text and still publishes.
    let ok = run(
        &sweep_sh,
        repo.path(),
        &[
            "learn",
            "tooling_friction",
            "t",
            r#"{"friction":"😀 déjà vu"}"#,
        ],
    );
    assert!(
        ok.status.success(),
        "a valid surrogate pair must still publish; stderr:\n{}",
        String::from_utf8_lossy(&ok.stderr)
    );

    drop(handle);
}

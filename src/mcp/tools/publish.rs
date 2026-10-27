//! Publish tools: `publish_status`, `publish_artifact`, `publish_blocked`,
//! `publish_question` for every session, and `publish_verified`,
//! `publish_feedback` for a root session only.
//!
//! Capability `mcp-agent-publish`: the four agent boot events the bundled
//! `broker.sh` helper covers, plus the two verdict events `sweep.sh verified`
//! / `feedback-gate` cover, nothing else. Each tool derives the publishing
//! identity from the session (design D3): a worktree session publishes as its
//! branch slug, a root session as `supervisor`. No tool schema here accepts
//! an `agent_id` or publisher parameter, so a session cannot publish as a
//! peer. The verdict tools are registered only for a root session (they are
//! absent from a worktree server, so a coding agent cannot self-verify), and
//! they attach the session's authority credential, which the broker requires
//! for a verdict.
//!
//! Every tool publishes the same `BrokerMessage` shape the shell helper does
//! (design D5) via the shared builders in [`crate::broker::publish`], and
//! degrades to an `ErrorData` (never a panic or a stdout write) when the
//! branch cannot be resolved or the broker is unreachable/rejects the
//! message — the server keeps running either way.

use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use crate::broker::client::BrokerClient;
use crate::broker::messages::{BrokerMessage, FeedbackPayload, VerifiedPayload, slugify_branch};
use crate::broker::publish::{
    build_artifact_message, build_blocked_message, build_question_message, build_status_message,
    publish_to_broker_http,
};
use crate::error::PawError;
use crate::mcp::server::GitPawMcpServer;
use crate::mcp::{RepoContext, SessionAuthority};
use crate::surface::SUPERVISOR_AGENT_ID;

/// Maps an internal error to an MCP protocol error. Takes the error by value
/// so it composes directly with `Result::map_err`.
#[allow(clippy::needless_pass_by_value)]
fn to_err(e: PawError) -> ErrorData {
    ErrorData::internal_error(e.to_string(), None)
}

/// Resolves the publishing identity (design D3) — the caller never supplies
/// one.
///
/// A root session (the repository's main working tree) publishes as the root
/// identity `supervisor`: the main working tree is the orchestrator's, and
/// deriving from its branch would name an agent after the default branch
/// (`main` → `agent-main`). A worktree session publishes as its slugified
/// branch.
///
/// Root detection is decided once, when the server starts
/// ([`RepoContext::for_root`]), through the same
/// [`RepoLayout::is_linked_worktree`] check `git paw setup-cli` makes.
///
/// [`RepoLayout::is_linked_worktree`]: crate::cli_setup::shell::RepoLayout::is_linked_worktree
fn resolve_agent_id(ctx: &RepoContext) -> Result<String, PawError> {
    if ctx.is_main_worktree {
        return Ok(SUPERVISOR_AGENT_ID.to_string());
    }
    let branch = crate::git::current_branch(&ctx.root)?;
    Ok(slugify_branch(&branch))
}

/// Returns the active broker URL, or an error when no broker is reachable
/// (no active session, or the session has no broker configured).
fn broker_url(ctx: &RepoContext) -> Result<&str, PawError> {
    ctx.broker_url.as_deref().ok_or_else(|| {
        PawError::McpError(
            "no active broker for this repository — start a git-paw session with the broker \
             enabled before publishing"
                .to_string(),
        )
    })
}

/// Where a verdict goes and the credential it carries: the root session's
/// broker URL and authority credential.
///
/// Read from the session receipt at call time, because a root session may
/// have started (or restarted) the session after this server did — through
/// `start_session` itself — and the receipt is what names the live broker and
/// its credential. Falls back to what the server resolved at startup. The
/// verdict tools exist only in a root session, so this never runs for a
/// worktree. The errors never quote the credential.
fn verdict_endpoint(ctx: &RepoContext) -> Result<(String, SessionAuthority), PawError> {
    if let Some(session) = crate::session::find_session_for_repo(&ctx.root)?
        && let Some(url) = crate::mcp::broker_url_from_session(&session)
        && let Some(credential) = session.broker_authority.filter(|c| !c.is_empty())
    {
        return Ok((url, SessionAuthority::new(credential)));
    }
    let url = broker_url(ctx)?.to_string();
    let authority = ctx.authority.clone().ok_or_else(|| {
        PawError::McpError(
            "no session authority credential for this repository — start a git-paw session \
             with the broker enabled from the main working tree before publishing a verdict"
                .to_string(),
        )
    })?;
    Ok((url, authority))
}

/// Parameters for [`GitPawMcpServer::publish_status`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PublishStatusParams {
    /// Human-readable status message (e.g. `"booting"`).
    pub message: String,
}

/// Parameters for [`GitPawMcpServer::publish_artifact`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PublishArtifactParams {
    /// Public API items exposed by this artifact.
    #[serde(default)]
    pub exports: Vec<String>,
    /// Files touched by this artifact.
    #[serde(default)]
    pub modified_files: Vec<String>,
}

/// Parameters for [`GitPawMcpServer::publish_blocked`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PublishBlockedParams {
    /// What the agent needs to proceed.
    pub needs: String,
    /// Agent ID (or resource) that can unblock the sender.
    pub from: String,
}

/// Parameters for [`GitPawMcpServer::publish_question`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PublishQuestionParams {
    /// The question text.
    pub question: String,
}

/// Response shared by every publish tool.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct PublishResponse {
    /// Always `true` on success (a failure returns an `ErrorData` instead).
    pub published: bool,
}

#[tool_router(router = publish_router, vis = "pub(crate)")]
impl GitPawMcpServer {
    /// `publish_status` — REGISTER / working-status boot event.
    #[tool(
        description = "Publish an agent.status event (status=\"working\") to the broker, with the \
                       given human-readable message. Use this as your very first action on boot \
                       (message: \"booting\") to become visible in the dashboard immediately. The \
                       publishing agent_id is resolved automatically from this session's worktree \
                       branch — you cannot publish as a different agent."
    )]
    pub(crate) fn publish_status(
        &self,
        Parameters(p): Parameters<PublishStatusParams>,
    ) -> Result<Json<PublishResponse>, ErrorData> {
        let agent_id = resolve_agent_id(&self.ctx).map_err(to_err)?;
        let url = broker_url(&self.ctx).map_err(to_err)?;
        let msg = build_status_message(&agent_id, "working", Some(p.message), None);
        publish_to_broker_http(url, &msg).map_err(to_err)?;
        Ok(Json(PublishResponse { published: true }))
    }

    /// `publish_artifact` — DONE / code-less task-completion boot event.
    #[tool(
        description = "Publish an agent.artifact event (status=\"done\") to the broker. This is the \
                       code-less DONE fallback — only for tasks that produce no commit (docs-only \
                       updates outside this worktree, planning notes, exploration tasks). Tasks that \
                       produce code changes SHALL NOT call this tool: commit via `git commit` and let \
                       the post-commit hook publish it with the authoritative modified_files list. \
                       exports lists public API items exposed by this artifact; modified_files \
                       lists files touched. The publishing agent_id is resolved automatically from \
                       this session's worktree branch."
    )]
    pub(crate) fn publish_artifact(
        &self,
        Parameters(p): Parameters<PublishArtifactParams>,
    ) -> Result<Json<PublishResponse>, ErrorData> {
        let agent_id = resolve_agent_id(&self.ctx).map_err(to_err)?;
        let url = broker_url(&self.ctx).map_err(to_err)?;
        let msg = build_artifact_message(&agent_id, p.exports, p.modified_files);
        publish_to_broker_http(url, &msg).map_err(to_err)?;
        Ok(Json(PublishResponse { published: true }))
    }

    /// `publish_blocked` — BLOCKED / dependency-waiting boot event.
    #[tool(
        description = "Publish an agent.blocked event to the broker: needs describes what the agent \
                       requires to proceed, from names the agent id or resource that can unblock it. \
                       Call this as soon as you realize you are waiting on another agent or external \
                       state. The publishing agent_id is resolved automatically from this session's \
                       worktree branch."
    )]
    pub(crate) fn publish_blocked(
        &self,
        Parameters(p): Parameters<PublishBlockedParams>,
    ) -> Result<Json<PublishResponse>, ErrorData> {
        let agent_id = resolve_agent_id(&self.ctx).map_err(to_err)?;
        let url = broker_url(&self.ctx).map_err(to_err)?;
        let msg = build_blocked_message(&agent_id, &p.needs, &p.from);
        publish_to_broker_http(url, &msg).map_err(to_err)?;
        Ok(Json(PublishResponse { published: true }))
    }

    /// `publish_question` — QUESTION / uncertainty-escalation boot event.
    #[tool(
        description = "Publish an agent.question event to the broker, routed to the supervisor inbox \
                       for a human/supervisor reply. Call this and WAIT for an answer whenever you are \
                       uncertain what is wanted — do not guess. The publishing agent_id is resolved \
                       automatically from this session's worktree branch."
    )]
    pub(crate) fn publish_question(
        &self,
        Parameters(p): Parameters<PublishQuestionParams>,
    ) -> Result<Json<PublishResponse>, ErrorData> {
        let agent_id = resolve_agent_id(&self.ctx).map_err(to_err)?;
        let url = broker_url(&self.ctx).map_err(to_err)?;
        let msg = build_question_message(&agent_id, &p.question);
        publish_to_broker_http(url, &msg).map_err(to_err)?;
        Ok(Json(PublishResponse { published: true }))
    }
}

/// Parameters for [`GitPawMcpServer::publish_verified`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PublishVerifiedParams {
    /// The agent whose work passed verification (its `agent_id`, e.g.
    /// `feat-x`).
    pub agent: String,
    /// Human-readable summary of the verification result.
    pub message: String,
}

/// Parameters for [`GitPawMcpServer::publish_feedback`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PublishFeedbackParams {
    /// The agent the feedback is for (its `agent_id`, e.g. `feat-x`).
    pub agent: String,
    /// The gate that failed (e.g. `testing`, `spec audit`). When given, each
    /// error is prefixed `[<gate>] `, as `sweep.sh feedback-gate` does.
    #[serde(default)]
    pub gate: Option<String>,
    /// The errors the agent should address; at least one.
    pub errors: Vec<String>,
}

/// A required text parameter, trimmed; an error naming `field` when blank.
fn required(field: &str, value: &str) -> Result<String, PawError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(PawError::McpError(format!("`{field}` must not be empty")));
    }
    Ok(trimmed.to_string())
}

/// The verdict tools, registered for a root session only (see
/// [`GitPawMcpServer::new`]). They publish as `supervisor` and attach the
/// session's authority credential, as `sweep.sh verified` / `feedback-gate`
/// do.
#[tool_router(router = verdict_router, vis = "pub(crate)")]
impl GitPawMcpServer {
    /// `publish_verified` — the root's verdict that an agent passed
    /// verification.
    #[tool(
        description = "Publish an agent.verified verdict for the named agent, as the supervisor (root \
                       session only). Use it after the agent's work has passed every verification \
                       gate. The verifier identity and the session credential are attached \
                       automatically; the agent parameter names the agent whose work was verified."
    )]
    pub(crate) fn publish_verified(
        &self,
        Parameters(p): Parameters<PublishVerifiedParams>,
    ) -> Result<Json<PublishResponse>, ErrorData> {
        let agent = required("agent", &p.agent).map_err(to_err)?;
        let message = required("message", &p.message).map_err(to_err)?;
        let (url, authority) = verdict_endpoint(&self.ctx).map_err(to_err)?;
        let msg = BrokerMessage::Verified {
            agent_id: agent,
            payload: VerifiedPayload {
                verified_by: SUPERVISOR_AGENT_ID.to_string(),
                message: Some(message),
            },
        };
        BrokerClient::new(url)
            .publish_with_authority(&msg, Some(authority.expose()))
            .map_err(to_err)?;
        Ok(Json(PublishResponse { published: true }))
    }

    /// `publish_feedback` — the root's corrective feedback to an agent.
    #[tool(
        description = "Publish an agent.feedback message to the named agent, from the supervisor \
                       (root session only): the errors it must address before it can be verified, \
                       optionally tagged with the gate that failed. The sender identity and the \
                       session credential are attached automatically."
    )]
    pub(crate) fn publish_feedback(
        &self,
        Parameters(p): Parameters<PublishFeedbackParams>,
    ) -> Result<Json<PublishResponse>, ErrorData> {
        let agent = required("agent", &p.agent).map_err(to_err)?;
        let gate = match p.gate.as_deref() {
            Some(gate) => Some(required("gate", gate).map_err(to_err)?),
            None => None,
        };
        let errors = p
            .errors
            .iter()
            .map(|e| required("errors", e))
            .collect::<Result<Vec<_>, _>>()
            .map_err(to_err)?;
        if errors.is_empty() {
            return Err(to_err(PawError::McpError(
                "`errors` must name at least one error".to_string(),
            )));
        }
        let errors = match gate {
            Some(gate) => errors
                .into_iter()
                .map(|e| format!("[{gate}] {e}"))
                .collect(),
            None => errors,
        };
        let (url, authority) = verdict_endpoint(&self.ctx).map_err(to_err)?;
        let msg = BrokerMessage::Feedback {
            agent_id: agent,
            payload: FeedbackPayload {
                from: SUPERVISOR_AGENT_ID.to_string(),
                errors,
            },
        };
        BrokerClient::new(url)
            .publish_with_authority(&msg, Some(authority.expose()))
            .map_err(to_err)?;
        Ok(Json(PublishResponse { published: true }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::messages::BrokerMessage;
    use crate::broker::publish::fetch_log_over_http;
    use crate::broker::{self, BrokerState};
    use crate::config::BrokerConfig;
    use std::path::Path;
    use std::process::Command;

    fn git_run(dir: &Path, args: &[&str]) {
        assert!(
            Command::new("git")
                .current_dir(dir)
                .args(args)
                .status()
                .unwrap()
                .success(),
            "git {args:?} failed"
        );
    }

    /// A throwaway git repo checked out on `branch`, for resolving the
    /// publishing `agent_id`.
    /// A repository and a LINKED WORKTREE checked out on `branch` — the shape
    /// an agent actually runs in, and the only one from which a publish tool
    /// derives an id (it refuses in the main working tree).
    ///
    /// [`Self::path`] is the worktree, not the repository, because that is
    /// what an agent's MCP server resolves to.
    struct AgentWorktree {
        _sandbox: tempfile::TempDir,
        worktree: std::path::PathBuf,
    }

    impl AgentWorktree {
        fn path(&self) -> &std::path::Path {
            &self.worktree
        }
    }

    fn repo_on_branch(branch: &str) -> AgentWorktree {
        let sandbox = tempfile::tempdir().unwrap();
        let repo = sandbox.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.email", "t@example.com"],
            vec!["config", "user.name", "Test"],
        ] {
            git_run(&repo, &args);
        }
        git_run(&repo, &["commit", "--allow-empty", "-q", "-m", "root"]);

        let worktree = sandbox.path().join("worktree");
        git_run(
            &repo,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                branch,
                worktree.to_str().unwrap(),
            ],
        );
        AgentWorktree {
            _sandbox: sandbox,
            worktree,
        }
    }

    /// Just the repository's main working tree, on `main` — where a publish
    /// tool has no agent to be.
    fn main_working_tree() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.email", "t@example.com"],
            vec!["config", "user.name", "Test"],
        ] {
            git_run(dir, &args);
        }
        git_run(dir, &["commit", "--allow-empty", "-q", "-m", "root"]);
        tmp
    }

    fn pick_broker_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .expect("bind ephemeral port")
            .local_addr()
            .expect("read local addr")
            .port()
    }

    fn spawn_test_broker() -> (broker::BrokerHandle, String) {
        let mut port = pick_broker_port();
        let mut attempts = 0;
        loop {
            let config = BrokerConfig {
                enabled: true,
                port,
                bind: "127.0.0.1".to_string(),
                ..Default::default()
            };
            match broker::start_broker(&config, BrokerState::new(None), Vec::new()) {
                Ok(handle) => return (handle, config.url()),
                Err(_) if attempts < 10 => {
                    port = pick_broker_port();
                    attempts += 1;
                }
                Err(e) => panic!("failed to start test broker after retries: {e}"),
            }
        }
    }

    fn server_for(root: std::path::PathBuf, broker_url: Option<String>) -> GitPawMcpServer {
        GitPawMcpServer::new(RepoContext {
            root,
            git_paw_dir: None,
            broker_url,
            server_name: "git-paw".to_string(),
            is_main_worktree: false,
            authority: None,
        })
    }

    #[test]
    fn publish_status_sends_wire_identical_message_to_broker_sh() {
        let (handle, url) = spawn_test_broker();
        let repo = repo_on_branch("feat/x");
        let server = server_for(repo.path().canonicalize().unwrap(), Some(url.clone()));

        let resp = server
            .publish_status(Parameters(PublishStatusParams {
                message: "booting".to_string(),
            }))
            .expect("publish_status succeeds against a live broker");
        assert!(resp.0.published);

        let log = fetch_log_over_http(&url).expect("fetch broker log");
        let BrokerMessage::Status { agent_id, payload } = log
            .into_iter()
            .find(|m| matches!(m, BrokerMessage::Status { .. }))
            .expect("a status message was published")
        else {
            unreachable!()
        };
        // Wire-identical to `broker.sh status booting`: status="working",
        // message carried verbatim, modified_files empty, no cli/phase keys.
        assert_eq!(agent_id, "feat-x");
        assert_eq!(payload.status, "working");
        assert_eq!(payload.message.as_deref(), Some("booting"));
        assert!(payload.modified_files.is_empty());
        assert_eq!(payload.cli, None);
        drop(handle);
    }

    /// `mcp-agent-publish` scenario "Impersonation is not possible": a
    /// session resolved to one worktree branch can only ever publish under
    /// its own derived `agent_id`, no matter what — there is no parameter that
    /// could attribute a message to a peer.
    #[test]
    fn publish_tools_cannot_impersonate_another_agent() {
        let (handle, url) = spawn_test_broker();
        let repo_a = repo_on_branch("feat/a");
        let repo_b = repo_on_branch("feat/b");
        let server_a = server_for(repo_a.path().canonicalize().unwrap(), Some(url.clone()));
        let server_b = server_for(repo_b.path().canonicalize().unwrap(), Some(url.clone()));

        server_a
            .publish_status(Parameters(PublishStatusParams {
                message: "from a".to_string(),
            }))
            .expect("session a publishes");
        server_b
            .publish_status(Parameters(PublishStatusParams {
                message: "from b".to_string(),
            }))
            .expect("session b publishes");

        let log = fetch_log_over_http(&url).expect("fetch broker log");
        let statuses: Vec<(String, String)> = log
            .into_iter()
            .filter_map(|m| match m {
                BrokerMessage::Status { agent_id, payload } => {
                    Some((agent_id, payload.message.unwrap_or_default()))
                }
                _ => None,
            })
            .collect();
        assert!(
            statuses
                .iter()
                .any(|(id, msg)| id == "feat-a" && msg == "from a"),
            "session a's message must be attributed to feat-a; got {statuses:?}"
        );
        assert!(
            statuses
                .iter()
                .any(|(id, msg)| id == "feat-b" && msg == "from b"),
            "session b's message must be attributed to feat-b; got {statuses:?}"
        );
        assert!(
            !statuses
                .iter()
                .any(|(id, msg)| id == "feat-a" && msg == "from b"),
            "session b must never publish attributed to feat-a; got {statuses:?}"
        );
        assert!(
            !statuses
                .iter()
                .any(|(id, msg)| id == "feat-b" && msg == "from a"),
            "session a must never publish attributed to feat-b; got {statuses:?}"
        );
        drop(handle);
    }

    #[test]
    fn publish_artifact_sends_wire_identical_message_to_broker_sh() {
        let (handle, url) = spawn_test_broker();
        let repo = repo_on_branch("feat/y");
        let server = server_for(repo.path().canonicalize().unwrap(), Some(url.clone()));

        server
            .publish_artifact(Parameters(PublishArtifactParams {
                exports: vec!["foo".to_string()],
                modified_files: vec!["src/lib.rs".to_string()],
            }))
            .expect("publish_artifact succeeds against a live broker");

        let log = fetch_log_over_http(&url).expect("fetch broker log");
        let BrokerMessage::Artifact { agent_id, payload } = log
            .into_iter()
            .find(|m| matches!(m, BrokerMessage::Artifact { .. }))
            .expect("an artifact message was published")
        else {
            unreachable!()
        };
        assert_eq!(agent_id, "feat-y");
        assert_eq!(payload.status, "done");
        assert_eq!(payload.exports, vec!["foo".to_string()]);
        assert_eq!(payload.modified_files, vec!["src/lib.rs".to_string()]);
        drop(handle);
    }

    #[test]
    fn publish_blocked_sends_wire_identical_message_to_broker_sh() {
        let (handle, url) = spawn_test_broker();
        let repo = repo_on_branch("feat/z");
        let server = server_for(repo.path().canonicalize().unwrap(), Some(url.clone()));

        server
            .publish_blocked(Parameters(PublishBlockedParams {
                needs: "peer review".to_string(),
                from: "feat-w".to_string(),
            }))
            .expect("publish_blocked succeeds against a live broker");

        let log = fetch_log_over_http(&url).expect("fetch broker log");
        let BrokerMessage::Blocked { agent_id, payload } = log
            .into_iter()
            .find(|m| matches!(m, BrokerMessage::Blocked { .. }))
            .expect("a blocked message was published")
        else {
            unreachable!()
        };
        assert_eq!(agent_id, "feat-z");
        assert_eq!(payload.needs, "peer review");
        assert_eq!(payload.from, "feat-w");
        drop(handle);
    }

    #[test]
    fn publish_question_sends_wire_identical_message_to_broker_sh() {
        let (handle, url) = spawn_test_broker();
        let repo = repo_on_branch("feat/q");
        let server = server_for(repo.path().canonicalize().unwrap(), Some(url.clone()));

        server
            .publish_question(Parameters(PublishQuestionParams {
                question: "should I proceed?".to_string(),
            }))
            .expect("publish_question succeeds against a live broker");

        let log = fetch_log_over_http(&url).expect("fetch broker log");
        let BrokerMessage::Question { agent_id, payload } = log
            .into_iter()
            .find(|m| matches!(m, BrokerMessage::Question { .. }))
            .expect("a question message was published")
        else {
            unreachable!()
        };
        assert_eq!(agent_id, "feat-q");
        assert_eq!(payload.question, "should I proceed?");
        drop(handle);
    }

    /// Raw `GET <broker_url><path>`, returning the response body. Mirrors
    /// [`crate::broker::publish::fetch_log_entries_over_http`]'s manual TCP
    /// style for an endpoint this module has no typed client for.
    fn http_get(broker_url: &str, path: &str) -> String {
        use std::io::{Read as _, Write as _};
        use std::net::TcpStream;

        let addr = broker_url.strip_prefix("http://").unwrap_or(broker_url);
        let mut stream = TcpStream::connect(addr).expect("connect to test broker");
        let request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
        stream
            .write_all(request.as_bytes())
            .expect("write GET request");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("read GET response");
        response
            .split_once("\r\n\r\n")
            .map(|(_, body)| body.to_string())
            .expect("response has a body")
    }

    /// Cross-module E2E (tasks.md 5.1): MCP tool call → broker publish →
    /// delivery → peer poll returns the message. `publish_question` routes
    /// to the `"supervisor"` inbox (creating it if absent); a peer `GET
    /// /messages/supervisor` — the same endpoint `broker.sh poll` and the
    /// supervisor pane use — must see it.
    #[test]
    fn e2e_mcp_publish_question_delivers_to_peer_poll() {
        let (handle, url) = spawn_test_broker();
        let repo = repo_on_branch("feat/asker");
        let server = server_for(repo.path().canonicalize().unwrap(), Some(url.clone()));

        server
            .publish_question(Parameters(PublishQuestionParams {
                question: "should I proceed?".to_string(),
            }))
            .expect("publish_question succeeds against a live broker");

        let body = http_get(&url, "/messages/supervisor?since=0");
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("valid JSON body");
        let messages = parsed["messages"].as_array().expect("messages array");
        assert!(
            messages.iter().any(|m| {
                m.get("type").and_then(serde_json::Value::as_str) == Some("agent.question")
                    && m.get("agent_id").and_then(serde_json::Value::as_str) == Some("feat-asker")
                    && m.get("payload")
                        .and_then(|p| p.get("question"))
                        .and_then(serde_json::Value::as_str)
                        == Some("should I proceed?")
            }),
            "supervisor's peer poll must see the published question; got {messages:?}"
        );
        drop(handle);
    }

    /// E2E (tasks.md 5.2): an agent booting via the MCP form registers with
    /// the broker without any shell permission grant — this test spawns no
    /// `Command`/subprocess at all; the REGISTER step is a single in-process
    /// tool call, and the agent still surfaces in the broker's live roster
    /// (`GET /status`), the same aggregate a peer or the dashboard reads.
    #[test]
    fn e2e_mcp_form_boot_registers_without_any_shell_command() {
        let (handle, url) = spawn_test_broker();
        let repo = repo_on_branch("feat/mcp-booter");
        let server = server_for(repo.path().canonicalize().unwrap(), Some(url.clone()));

        server
            .publish_status(Parameters(PublishStatusParams {
                message: "booting".to_string(),
            }))
            .expect("MCP REGISTER step succeeds with no shell command");

        let agents = crate::coordination::inventory::fetch_status_agents_over_http(&url)
            .expect("fetch broker roster");
        assert!(
            agents
                .iter()
                .any(|a| a.agent_id == "feat-mcp-booter" && a.status == "working"),
            "agent must be visible in the broker roster after the MCP REGISTER step; got {agents:?}"
        );
        drop(handle);
    }

    #[test]
    fn publish_status_without_active_broker_returns_mcp_error_not_panic() {
        let repo = repo_on_branch("feat/no-broker");
        let server = server_for(repo.path().canonicalize().unwrap(), None);

        let err = server
            .publish_status(Parameters(PublishStatusParams {
                message: "booting".to_string(),
            }))
            .err()
            .expect("no broker configured must error, not panic");
        assert!(err.message.contains("no active broker"), "got: {err:?}");
    }

    #[test]
    fn publish_status_with_unreachable_broker_returns_mcp_error_not_panic() {
        let repo = repo_on_branch("feat/unreachable");
        // A closed local port: broker_url is Some but nothing is listening.
        let dead_url = "http://127.0.0.1:1".to_string();
        let server = server_for(repo.path().canonicalize().unwrap(), Some(dead_url));

        let err = server
            .publish_status(Parameters(PublishStatusParams {
                message: "booting".to_string(),
            }))
            .err()
            .expect("an unreachable broker must error, not panic");
        assert!(!err.message.is_empty());
    }

    /// A server resolved the way `git paw mcp` resolves one, for `root`.
    fn resolved_server(root: &Path, broker_url: Option<String>) -> GitPawMcpServer {
        let mut ctx = RepoContext::for_root(root.canonicalize().unwrap());
        ctx.broker_url = broker_url;
        GitPawMcpServer::new(ctx)
    }

    /// A root-session server presenting `authority`.
    fn root_server(broker_url: &str, authority: Option<&str>) -> GitPawMcpServer {
        GitPawMcpServer::new(RepoContext {
            root: std::env::temp_dir(),
            git_paw_dir: None,
            broker_url: Some(broker_url.to_string()),
            server_name: "git-paw".to_string(),
            is_main_worktree: true,
            authority: authority.map(|a| crate::mcp::SessionAuthority::new(a.to_string())),
        })
    }

    /// A test broker that enforces `credential` for verdicts.
    fn spawn_authority_broker(credential: &str) -> (broker::BrokerHandle, String) {
        for _ in 0..10 {
            let config = BrokerConfig {
                enabled: true,
                port: pick_broker_port(),
                bind: "127.0.0.1".to_string(),
                ..Default::default()
            };
            let state = BrokerState::new(None).with_authority(Some(credential.to_string()));
            if let Ok(handle) = broker::start_broker(&config, state, Vec::new()) {
                return (handle, config.url());
            }
        }
        panic!("failed to start an authority test broker");
    }

    const CREDENTIAL: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// `mcp-agent-publish` "Publish tools act as the calling agent only": a
    /// root session publishes the worker events as the root identity
    /// `supervisor`, never as an agent named after the default branch.
    #[test]
    fn a_root_session_publishes_worker_events_as_supervisor() {
        let (handle, url) = spawn_test_broker();
        let repo = main_working_tree();
        let server = resolved_server(repo.path(), Some(url.clone()));

        server
            .publish_status(Parameters(PublishStatusParams {
                message: "orchestrating".to_string(),
            }))
            .expect("a root session publishes");

        let log = fetch_log_over_http(&url).expect("fetch broker log");
        assert!(
            log.iter().any(|m| matches!(
                m,
                BrokerMessage::Status { agent_id, payload }
                    if agent_id == "supervisor" && payload.message.as_deref() == Some("orchestrating")
            )),
            "the root session must publish as supervisor; log: {log:?}"
        );
        assert!(
            !log.iter()
                .any(|m| m.agent_id() == "agent-main" || m.agent_id() == "main"),
            "no agent may be derived from the default branch; log: {log:?}"
        );
        drop(handle);
    }

    /// `mcp-agent-publish` "Root verdict carries supervisor as the verifier"
    /// and "Root session verdict is accepted by the broker": wire-identical
    /// to `sweep.sh verified <agent> <message>`.
    #[test]
    fn publish_verified_publishes_the_helper_shape_with_the_credential() {
        let (handle, url) = spawn_authority_broker(CREDENTIAL);
        let server = root_server(&url, Some(CREDENTIAL));

        server
            .publish_verified(Parameters(PublishVerifiedParams {
                agent: "feat-x".to_string(),
                message: "gates green".to_string(),
            }))
            .expect("the broker accepts a credentialed verdict");

        let log = fetch_log_over_http(&url).expect("fetch broker log");
        let verified: Vec<_> = log
            .iter()
            .filter_map(|m| match m {
                BrokerMessage::Verified { agent_id, payload } => Some((agent_id, payload)),
                _ => None,
            })
            .collect();
        assert_eq!(verified.len(), 1, "one verdict; log: {log:?}");
        let (agent_id, payload) = verified[0];
        assert_eq!(agent_id, "feat-x");
        assert_eq!(payload.verified_by, "supervisor");
        assert_eq!(payload.message.as_deref(), Some("gates green"));
        drop(handle);
    }

    /// Wire-identical to `sweep.sh feedback-gate <agent> <gate> <message>`:
    /// each error is tagged `[<gate>] `, from `supervisor`.
    #[test]
    fn publish_feedback_tags_errors_with_the_gate_as_the_helper_does() {
        let (handle, url) = spawn_authority_broker(CREDENTIAL);
        let server = root_server(&url, Some(CREDENTIAL));

        server
            .publish_feedback(Parameters(PublishFeedbackParams {
                agent: "feat-x".to_string(),
                gate: Some("testing".to_string()),
                errors: vec!["two tests fail".to_string()],
            }))
            .expect("the broker accepts credentialed feedback");
        server
            .publish_feedback(Parameters(PublishFeedbackParams {
                agent: "feat-x".to_string(),
                gate: None,
                errors: vec!["rename the flag".to_string()],
            }))
            .expect("feedback without a gate is accepted");

        let log = fetch_log_over_http(&url).expect("fetch broker log");
        let feedback: Vec<_> = log
            .iter()
            .filter_map(|m| match m {
                BrokerMessage::Feedback { agent_id, payload } => Some((
                    agent_id.clone(),
                    payload.from.clone(),
                    payload.errors.clone(),
                )),
                _ => None,
            })
            .collect();
        assert_eq!(
            feedback,
            vec![
                (
                    "feat-x".to_string(),
                    "supervisor".to_string(),
                    vec!["[testing] two tests fail".to_string()]
                ),
                (
                    "feat-x".to_string(),
                    "supervisor".to_string(),
                    vec!["rename the flag".to_string()]
                ),
            ]
        );
        drop(handle);
    }

    /// A root session without a credential publishes no verdict, and says
    /// why without quoting any credential.
    #[test]
    fn a_verdict_without_a_session_credential_is_refused_and_publishes_nothing() {
        let (handle, url) = spawn_authority_broker(CREDENTIAL);
        let server = root_server(&url, None);

        let err = server
            .publish_verified(Parameters(PublishVerifiedParams {
                agent: "feat-x".to_string(),
                message: "gates green".to_string(),
            }))
            .err()
            .expect("no credential, no verdict");
        assert!(err.message.contains("credential"), "got: {err:?}");

        let log = fetch_log_over_http(&url).expect("fetch broker log");
        assert!(
            !log.iter()
                .any(|m| matches!(m, BrokerMessage::Verified { .. })),
            "nothing may be published; log: {log:?}"
        );
        drop(handle);
    }

    /// A credential the broker does not hold is refused (403), and the error
    /// never echoes the credential that was presented.
    #[test]
    fn a_wrong_credential_is_refused_without_echoing_it() {
        let (handle, url) = spawn_authority_broker(CREDENTIAL);
        let presented = "f".repeat(64);
        let server = root_server(&url, Some(&presented));

        let err = server
            .publish_verified(Parameters(PublishVerifiedParams {
                agent: "feat-x".to_string(),
                message: "gates green".to_string(),
            }))
            .err()
            .expect("the broker refuses a credential it does not hold");
        assert!(err.message.contains("403"), "got: {err:?}");
        assert!(!err.message.contains(&presented), "got: {err:?}");

        let log = fetch_log_over_http(&url).expect("fetch broker log");
        assert!(
            !log.iter()
                .any(|m| matches!(m, BrokerMessage::Verified { .. })),
            "nothing may be published; log: {log:?}"
        );
        drop(handle);
    }

    #[test]
    fn publish_feedback_needs_at_least_one_error() {
        let (handle, url) = spawn_authority_broker(CREDENTIAL);
        let server = root_server(&url, Some(CREDENTIAL));
        let err = server
            .publish_feedback(Parameters(PublishFeedbackParams {
                agent: "feat-x".to_string(),
                gate: Some("testing".to_string()),
                errors: Vec::new(),
            }))
            .err()
            .expect("an empty error list is refused");
        assert!(err.message.contains("errors"), "got: {err:?}");
        drop(handle);
    }

    /// `mcp-agent-publish` "Credential is never a tool parameter" and
    /// "Publisher id is derived, not supplied": the verdict schemas take the
    /// target and the text only.
    #[test]
    fn verdict_schemas_take_no_credential_or_publisher() {
        let server = root_server("http://127.0.0.1:1", None);
        let tools = server.tool_router.list_all();
        for (name, allowed) in [
            ("publish_verified", &["agent", "message"][..]),
            ("publish_feedback", &["agent", "gate", "errors"][..]),
        ] {
            let tool = tools
                .iter()
                .find(|t| t.name == name)
                .unwrap_or_else(|| panic!("{name} is registered for a root session"));
            let props: Vec<&String> = tool
                .input_schema
                .get("properties")
                .and_then(serde_json::Value::as_object)
                .map(|p| p.keys().collect())
                .unwrap_or_default();
            assert!(
                props.iter().all(|p| allowed.contains(&p.as_str())),
                "{name} must take only {allowed:?}; got {props:?}"
            );
            assert!(
                !props.is_empty(),
                "{name} schema must declare its parameters"
            );
        }
    }

    #[test]
    fn publish_tool_schemas_carry_no_agent_id_parameter() {
        let server = server_for(std::env::temp_dir(), None);
        for tool in server.tool_router.list_all() {
            if !tool.name.starts_with("publish_") {
                continue;
            }
            let schema = serde_json::to_string(&tool.input_schema).unwrap();
            assert!(
                !schema.contains("\"agent_id\""),
                "{} schema must not accept an agent_id parameter; got {schema}",
                tool.name
            );
        }
    }
}

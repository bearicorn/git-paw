//! Publish tools: `publish_status`, `publish_artifact`, `publish_blocked`,
//! `publish_question`.
//!
//! The MCP server's only write surface (capability `mcp-agent-publish`):
//! exactly the four agent boot events the bundled `broker.sh` helper covers,
//! nothing else. Each tool derives the publishing `agent_id` from this
//! session's resolved worktree branch (design D3) — no tool schema here
//! accepts an `agent_id` parameter, so an agent cannot publish as a peer.
//! The supervisor authority verbs (`agent.verified`, `agent.feedback`) are
//! deliberately absent: exposing them would let a coding agent self-verify
//! and bypass the five-gate framework (design D1).
//!
//! Every tool publishes the same `BrokerMessage` shape the shell helper does
//! (design D5) via the shared builders in [`crate::broker::publish`], and
//! degrades to an `ErrorData` (never a panic or a stdout write) when the
//! branch cannot be resolved or the broker is unreachable/rejects the
//! message — the server keeps running either way.

use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use crate::broker::messages::slugify_branch;
use crate::broker::publish::{
    build_artifact_message, build_blocked_message, build_question_message, build_status_message,
    publish_to_broker_http,
};
use crate::error::PawError;
use crate::mcp::RepoContext;
use crate::mcp::server::GitPawMcpServer;

/// Maps an internal error to an MCP protocol error. Takes the error by value
/// so it composes directly with `Result::map_err`.
#[allow(clippy::needless_pass_by_value)]
fn to_err(e: PawError) -> ErrorData {
    ErrorData::internal_error(e.to_string(), None)
}

/// Resolves the publishing `agent_id` from the session's worktree branch
/// (design D3) — the caller never supplies one.
fn resolve_agent_id(ctx: &RepoContext) -> Result<String, PawError> {
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

/// Parameters for [`GitPawMcpServer::publish_status`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PublishStatusParams {
    /// Human-readable status message (e.g. `"booting"`).
    pub message: String,
}

/// Parameters for [`GitPawMcpServer::publish_artifact`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PublishArtifactParams {
    /// Public API items exported for peers to cherry-pick.
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
                       exports announces public API items for peers to cherry-pick; modified_files \
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
    fn repo_on_branch(branch: &str) -> tempfile::TempDir {
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
        if branch != "main" {
            git_run(dir, &["checkout", "-q", "-b", branch]);
        }
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

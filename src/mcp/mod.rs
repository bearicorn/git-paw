//! Read-only Model Context Protocol (MCP) server for `git paw mcp`.
//!
//! Exposes this repository's deterministic read-only state — coordination
//! intents/conflicts, governance docs, specs and tasks, session status and
//! learnings, agent skills, and git context — over the standard MCP protocol
//! on stdio, so any MCP-aware client (Claude Desktop, Cursor, `ChatGPT`
//! Desktop, Windsurf, VS Code MCP) can query it.
//!
//! # Module layout & dependency rule (design D2)
//!
//! ```text
//! mcp/
//! ├── mod.rs     entry: cmd_mcp(), RepoContext + repo resolution
//! ├── server.rs  stdio transport setup, tool registry wiring, lifecycle
//! ├── tools/     MCP tool definitions (one file per category)
//! └── query/     data-layer reads (no MCP types here)
//! ```
//!
//! The dependency direction is strict and one-way:
//!
//! - `query` knows **nothing** about MCP — it returns plain Rust / serde
//!   types built from broker HTTP state, files on disk, and git output.
//! - `tools` knows about MCP **and** `query`, but not about `server`.
//! - `server` only wires `tools` onto a transport.
//!
//! This keeps the future v2.0.0 HTTP transport additive: drop in a new
//! `server.rs` variant and reuse `tools` + `query` unchanged.
//!
//! # Guardrails
//!
//! - **No agent CLI is ever spawned** as an inference backend. Every tool
//!   result is derived from deterministic data sources only.
//! - **stdout is reserved** for JSON-RPC frames. All logging goes to stderr
//!   (see [`crate::mcp::server`]); there are no `print!`/`println!` calls in
//!   this module tree (enforced by a unit test in this file).

pub mod logging;
pub mod query;
pub mod server;
pub mod tools;

use std::path::{Path, PathBuf};

use crate::error::PawError;
use crate::session::{self, Session, SessionStatus};

/// Resolved context for a single `git paw mcp` invocation, constructed once
/// at startup and shared (read-only) by every tool.
#[derive(Debug, Clone)]
pub struct RepoContext {
    /// Absolute repository root (a worktree root resolves to its own root,
    /// not the main repository — see [`resolve_repo`]).
    pub root: PathBuf,
    /// `<root>/.git-paw/` when it exists, else `None` (cold / pure-manual
    /// repo).
    pub git_paw_dir: Option<PathBuf>,
    /// Broker base URL (`http://host:port`) derived from the active session
    /// receipt, or `None` when no session is active. Liveness is **not**
    /// probed here — query helpers attempt the HTTP call and degrade to
    /// empty results on failure.
    pub broker_url: Option<String>,
    /// Effective server identity advertised in the `initialize` handshake's
    /// `serverInfo.name`. Resolved once at construction from the loaded
    /// config's `[mcp].name` (defaulting to `"git-paw"`) so the server handler
    /// never re-loads config per `get_info()` call.
    pub server_name: String,
}

impl RepoContext {
    /// Builds a [`RepoContext`] from a resolved repository root.
    ///
    /// Reads the active session receipt (if any) to populate
    /// [`RepoContext::broker_url`]; a missing or stopped session simply
    /// leaves it `None`. Resolves the advertised server identity from the
    /// merged config's `[mcp].name`, defaulting to `"git-paw"` when unset (or
    /// when the config cannot be loaded).
    #[must_use]
    pub fn for_root(root: PathBuf) -> Self {
        let git_paw_dir = {
            let candidate = root.join(".git-paw");
            candidate.is_dir().then_some(candidate)
        };
        let broker_url = session::find_session_for_repo(&root)
            .ok()
            .flatten()
            .as_ref()
            .and_then(broker_url_from_session);
        let server_name = crate::config::load_config(&root, None)
            .map_or_else(|_| "git-paw".to_string(), |cfg| cfg.mcp_server_name());
        Self {
            root,
            git_paw_dir,
            broker_url,
            server_name,
        }
    }
}

/// Derives the broker base URL from a session receipt, or `None` when the
/// session carries no broker port (broker disabled, or session not active).
fn broker_url_from_session(session: &Session) -> Option<String> {
    // A stopped session's broker is gone; a paused session has stopped its
    // broker too. Only an active session can have a reachable broker.
    if session.status != SessionStatus::Active {
        return None;
    }
    let port = session.broker_port?;
    let bind = session.broker_bind.as_deref().unwrap_or("127.0.0.1");
    // 0.0.0.0 (listen-on-all) is not a connectable address from a client.
    let host = if bind == "0.0.0.0" || bind.is_empty() {
        "127.0.0.1"
    } else {
        bind
    };
    Some(format!("http://{host}:{port}"))
}

/// Resolves the target repository root per design D3.
///
/// Precedence:
/// 1. `--repo <path>` if provided — canonicalized, then required to be a git
///    repository (errors clearly with the path otherwise).
/// 2. Otherwise the nearest ancestor of the current directory containing a
///    `.git` entry. Worktrees resolve to their **own** root (git
///    `rev-parse --show-toplevel` returns the worktree root).
///
/// Returns a clear, human-readable [`PawError::McpError`] when no repository
/// can be resolved — the server never silently serves nothing.
pub fn resolve_repo(repo_flag: Option<&Path>) -> Result<PathBuf, PawError> {
    if let Some(flag) = repo_flag {
        let canonical = flag.canonicalize().map_err(|e| {
            PawError::McpError(format!(
                "--repo path {} could not be opened: {e}. Pass an existing repository path.",
                flag.display()
            ))
        })?;
        crate::git::validate_repo(&canonical).map_err(|_| {
            PawError::McpError(format!(
                "--repo path {} is not a git repository. Point --repo at a directory inside a git repo.",
                canonical.display()
            ))
        })
    } else {
        let cwd = std::env::current_dir()
            .map_err(|e| PawError::McpError(format!("cannot read current directory: {e}")))?;
        crate::git::validate_repo(&cwd).map_err(|_| {
            PawError::McpError(
                "no git repository found in the current directory or any parent. \
                 Run `git paw mcp` from inside a git repository, or pass \
                 `--repo <path>` (required for clients like Claude Desktop that \
                 spawn from a fixed directory)."
                    .to_string(),
            )
        })
    }
}

/// Entry point for the `git paw mcp` subcommand.
///
/// Resolves the repository, builds the [`RepoContext`], initializes stderr
/// logging, and runs the stdio MCP server until the client closes stdin.
pub fn cmd_mcp(repo_flag: Option<&Path>, log_file: Option<&Path>) -> Result<(), PawError> {
    let root = resolve_repo(repo_flag)?;
    let context = RepoContext::for_root(root);
    server::run(context, log_file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// Initializes a throwaway git repo at `dir`.
    fn git_init(dir: &Path) {
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.email", "t@example.com"],
            vec!["config", "user.name", "Test"],
        ] {
            let ok = Command::new("git")
                .current_dir(dir)
                .args(&args)
                .status()
                .expect("git runs")
                .success();
            assert!(ok, "git {args:?} failed");
        }
    }

    #[test]
    fn resolve_repo_with_valid_repo_path_returns_root() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("proj");
        std::fs::create_dir(&repo).unwrap();
        git_init(&repo);

        let resolved = resolve_repo(Some(&repo)).expect("valid repo resolves");
        assert_eq!(
            resolved.canonicalize().unwrap(),
            repo.canonicalize().unwrap()
        );
    }

    #[test]
    fn resolve_repo_with_non_git_path_errors_with_path() {
        let tmp = tempfile::tempdir().unwrap();
        let not_repo = tmp.path().join("plain");
        std::fs::create_dir(&not_repo).unwrap();

        let err = resolve_repo(Some(&not_repo)).expect_err("non-git path must error");
        let msg = err.to_string();
        assert!(msg.contains("not a git repository"), "got: {msg}");
    }

    #[test]
    fn resolve_repo_with_nonexistent_path_errors() {
        let err = resolve_repo(Some(Path::new("/no/such/path/at/all")))
            .expect_err("nonexistent path must error");
        assert!(
            err.to_string().contains("could not be opened"),
            "got: {err}"
        );
    }

    #[test]
    fn resolve_repo_from_subdir_finds_enclosing_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("proj");
        std::fs::create_dir(&repo).unwrap();
        git_init(&repo);
        let sub = repo.join("a").join("b");
        std::fs::create_dir_all(&sub).unwrap();

        // resolve_repo(Some(subdir)) exercises the same validate_repo path the
        // CWD branch uses, without mutating the process-wide current dir.
        let resolved = resolve_repo(Some(&sub)).expect("subdir resolves to enclosing repo");
        assert_eq!(
            resolved.canonicalize().unwrap(),
            repo.canonicalize().unwrap()
        );
    }

    #[test]
    fn resolve_repo_worktree_resolves_to_worktree_root() {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("main");
        std::fs::create_dir(&main).unwrap();
        git_init(&main);
        // Need a commit before adding a worktree.
        std::fs::write(main.join("README.md"), "hi").unwrap();
        for args in [vec!["add", "."], vec!["commit", "-q", "-m", "init"]] {
            assert!(
                Command::new("git")
                    .current_dir(&main)
                    .args(&args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let wt = tmp.path().join("wt");
        assert!(
            Command::new("git")
                .current_dir(&main)
                .args([
                    "worktree",
                    "add",
                    "-q",
                    wt.to_str().unwrap(),
                    "-b",
                    "feat/x"
                ])
                .status()
                .unwrap()
                .success(),
            "worktree add failed"
        );

        let resolved = resolve_repo(Some(&wt)).expect("worktree resolves");
        assert_eq!(
            resolved.canonicalize().unwrap(),
            wt.canonicalize().unwrap(),
            "worktree must resolve to its own root, not the main repo"
        );
    }

    #[test]
    fn for_root_without_git_paw_dir_yields_none() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("proj");
        std::fs::create_dir(&repo).unwrap();
        git_init(&repo);

        let ctx = RepoContext::for_root(repo.canonicalize().unwrap());
        assert!(ctx.git_paw_dir.is_none());
        assert!(ctx.broker_url.is_none());
    }

    #[test]
    fn for_root_with_git_paw_dir_is_some() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("proj");
        std::fs::create_dir(&repo).unwrap();
        git_init(&repo);
        std::fs::create_dir(repo.join(".git-paw")).unwrap();

        let ctx = RepoContext::for_root(repo.canonicalize().unwrap());
        assert!(ctx.git_paw_dir.is_some());
    }

    /// Lint (design risk: stdout pollution kills the MCP protocol): assert no
    /// `print!`/`println!` invocations exist anywhere under `src/mcp/`. Only
    /// `eprint!`/`eprintln!`/`tracing` (all stderr) are permitted.
    #[test]
    fn no_stdout_macros_under_src_mcp() {
        let mcp_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("mcp");
        let mut offenders = Vec::new();
        visit_rs_files(&mcp_dir, &mut |path, contents| {
            for (lineno, line) in contents.lines().enumerate() {
                // Skip comment lines (the doc-comment naming the macros).
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") || trimmed.starts_with('*') {
                    continue;
                }
                // Flag genuine macro *calls* (`println!(`), not the literal text
                // appearing inside a string (e.g. this detector itself, where it
                // is preceded by a `"`).
                if is_macro_call(line, "println!(") || is_macro_call(line, "print!(") {
                    offenders.push(format!("{}:{}", path.display(), lineno + 1));
                }
            }
        });
        assert!(
            offenders.is_empty(),
            "stdout macros found under src/mcp/ (stdout is reserved for JSON-RPC): {offenders:?}"
        );
    }

    /// Returns true if `needle` (a macro-call prefix like `println!(`) appears
    /// in `line` as a real invocation — i.e. not immediately preceded by a `"`
    /// (which would mean it is inside a string literal, like in this detector).
    fn is_macro_call(line: &str, needle: &str) -> bool {
        let mut from = 0;
        while let Some(rel) = line[from..].find(needle) {
            let idx = from + rel;
            let prev = line[..idx].chars().next_back();
            if prev != Some('"') {
                return true;
            }
            from = idx + needle.len();
        }
        false
    }

    fn visit_rs_files(dir: &Path, f: &mut impl FnMut(&Path, &str)) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit_rs_files(&path, f);
            } else if path.extension().is_some_and(|e| e == "rs")
                && let Ok(contents) = std::fs::read_to_string(&path)
            {
                f(&path, &contents);
            }
        }
    }
}

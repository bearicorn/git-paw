//! CLI argument parsing.
//!
//! Defines the command-line interface using `clap` v4 with derive macros.
//! All subcommands, flags, and options are declared here.

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Spec format selector for the `--specs-format` flag.
///
/// Four formats are supported:
/// - `openspec` — `openspec/changes/<name>/` directory layout.
/// - `markdown` — single-file Markdown specs with YAML frontmatter.
/// - `speckit` — GitHub Spec Kit `.specify/specs/<feature>/` layout.
/// - `superpowers` — obra/superpowers `docs/superpowers/plans/*.md` documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "lowercase")]
pub enum SpecsFormat {
    /// `OpenSpec` format (directory of `<change>/tasks.md`).
    Openspec,
    /// Markdown format (one `.md` file per spec with frontmatter).
    Markdown,
    /// Spec Kit format (`.specify/specs/<feature>/`).
    Speckit,
    /// Superpowers format (`docs/superpowers/plans/*.md` writing-plans docs).
    Superpowers,
}

impl SpecsFormat {
    /// Returns the backend-dispatch string for this format.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Openspec => "openspec",
            Self::Markdown => "markdown",
            Self::Speckit => "speckit",
            Self::Superpowers => "superpowers",
        }
    }
}

/// Parallel AI Worktrees — orchestrate multiple AI coding CLI sessions
/// across git worktrees from a single terminal using tmux.
#[derive(Debug, Parser)]
#[command(
    name = "git-paw",
    version,
    about = "Parallel AI Worktrees — orchestrate multiple AI coding CLI sessions across git worktrees",
    long_about = "git-paw orchestrates multiple AI coding CLI sessions (Claude, Codex, Gemini, etc.) \
                  across git worktrees from a single terminal using tmux. Each branch gets its own \
                  worktree and AI session, running in parallel.",
    after_help = "\x1b[1mQuick Start:\x1b[0m\n\n  \
                  # Launch interactive session (picks CLI and branches)\n  \
                  git paw\n\n  \
                  # Use Claude on specific branches\n  \
                  git paw start --cli claude --branches feat/auth,feat/api\n\n  \
                  # Check session status\n  \
                  git paw status\n\n  \
                  # Diagnose the environment before launching\n  \
                  git paw doctor\n\n  \
                  # Pause session (detaches client, stops broker, keeps CLIs alive)\n  \
                  git paw pause\n\n  \
                  # Stop session (kills CLIs, preserves worktrees for later)\n  \
                  git paw stop\n\n  \
                  # Remove everything\n  \
                  git paw purge"
)]
pub struct Cli {
    /// Subcommand to run. Defaults to `start` if omitted.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Launch a new session or reattach to an existing one
    #[command(
        about = "Launch a new session or reattach to an existing one",
        long_about = "Smart start: reattaches if a session is active, recovers if stopped/crashed, \
                      or launches a new interactive session.\n\n\
                      By default, every existing agent branch is rebased onto the repository's \
                      default branch (whatever `origin/HEAD` tracks — typically `main`) before \
                      its worktree is opened, so agents always start from current main. Pass \
                      `--no-rebase` to skip this step and reproduce the pre-v0.6 behaviour \
                      (useful when you have local pinned SHAs or are deliberately working off a \
                      stale baseline). If the rebase hits a conflict, the affected branch is \
                      left at its pre-rebase HEAD and `git paw start` exits with an error \
                      listing the conflicting files.\n\n\
                      Examples:\n  \
                      git paw start\n  \
                      git paw start --cli claude\n  \
                      git paw start --cli claude --branches feat/auth,feat/api\n  \
                      git paw start --from-all-specs\n  \
                      git paw start --from-all-specs --cli claude\n  \
                      git paw start --specs add-auth,fix-session\n  \
                      git paw start --specs   # opens spec picker (TTY required)\n  \
                      git paw start --dry-run\n  \
                      git paw start --preset backend\n  \
                      git paw start --supervisor   # auto-approve safe prompts via [supervisor.auto_approve]\n  \
                      git paw start --no-supervisor  # disable supervisor for this session (overrides config)\n  \
                      git paw start --no-rebase   # skip rebasing agent branches onto the default branch"
    )]
    Start {
        /// AI CLI to use (e.g., claude, codex, agy). Skips CLI picker if provided.
        #[arg(long, help = "AI CLI to use (skips CLI picker)")]
        cli: Option<String>,

        /// Comma-separated branch names. Skips branch picker if provided.
        #[arg(
            long,
            value_delimiter = ',',
            help = "Comma-separated branches (skips branch picker)"
        )]
        branches: Option<Vec<String>>,

        /// Launch worktrees for every discovered spec across all configured formats.
        #[arg(
            long,
            alias = "from-specs",
            help = "Launch from every discovered spec across all configured formats"
        )]
        from_all_specs: bool,

        /// Narrow the session to named specs, or open the multi-select picker
        /// when given without values.
        ///
        /// `--specs add-auth,fix-session` runs only those specs. Bare `--specs`
        /// opens a multi-select picker; an interactive terminal is required
        /// (otherwise the command exits with an actionable error pointing at
        /// `--specs NAME[,NAME...]` and `--from-all-specs`).
        ///
        /// Mutually exclusive with `--from-all-specs`.
        #[arg(
            long,
            value_delimiter = ',',
            num_args = 0..,
            conflicts_with = "from_all_specs",
            help = "Comma-separated spec names; bare flag opens picker (TTY required)"
        )]
        specs: Option<Vec<String>>,

        /// Override the spec format used for `--from-all-specs` / `--specs` scanning.
        ///
        /// Accepted values: `openspec`, `markdown`, `speckit`, `superpowers`.
        /// Overrides the `[specs] type` setting in `.git-paw/config.toml`. There
        /// is no filesystem auto-detection — the format is resolved only from
        /// this flag or the `[specs]` config.
        #[arg(
            long,
            value_enum,
            help = "Override spec format (openspec, markdown, speckit, superpowers)"
        )]
        specs_format: Option<SpecsFormat>,

        /// Preview the session plan without executing.
        #[arg(long, help = "Preview the session plan without executing")]
        dry_run: bool,

        /// Use a named preset from config.
        #[arg(long, help = "Use a named preset from config")]
        preset: Option<String>,

        /// Enable supervisor mode for this session.
        #[arg(
            long,
            default_value_t = false,
            help = "Enable supervisor mode for this session"
        )]
        supervisor: bool,

        /// Disable supervisor mode for this session, overriding any config setting.
        #[arg(
            long,
            conflicts_with = "supervisor",
            default_value_t = false,
            help = "Disable supervisor for this session, overriding any [supervisor] enabled = true in config"
        )]
        no_supervisor: bool,

        /// Bypass uncommitted-spec validation warning.
        #[arg(long, help = "Bypass uncommitted-spec validation warning")]
        force: bool,

        /// Skip rebasing existing agent branches onto the default branch
        /// before opening their worktrees.
        ///
        /// By default, `git paw start` rebases every existing agent branch
        /// onto the repository's default branch (whatever `origin/HEAD`
        /// tracks, typically `main`) before opening or reopening its
        /// worktree, so agents always start from current `main`. Pass
        /// `--no-rebase` to skip the rebase step entirely and reproduce the
        /// pre-v0.6 behaviour. Newly created branches (no prior commits) are
        /// not rebased regardless of this flag.
        #[arg(
            long,
            default_value_t = false,
            help = "Skip rebasing existing agent branches onto the default branch before opening worktrees"
        )]
        no_rebase: bool,

        /// Run the supervisor wave to completion with no human babysitting.
        ///
        /// Engages supervisor mode (equivalent to `--supervisor`) and then
        /// drives an in-process loop that sweeps the supervisor pane and every
        /// coding-agent pane on a ~15s cadence: it auto-approves
        /// classifier-safe permission prompts, escalates risky/unknown prompts
        /// for later human review WITHOUT blocking the rest of the wave,
        /// detects completion, and exits with a summary. Designed for detached
        /// operation — it does not require an attached interactive terminal.
        ///
        /// Mutually exclusive with `--no-supervisor` (they express opposing
        /// intents). May be combined with `--supervisor`, `--from-all-specs`,
        /// `--specs`, `--cli`, and `--branches`.
        #[arg(
            long,
            default_value_t = false,
            conflicts_with = "no_supervisor",
            help = "Run the supervisor wave to completion with no human babysitting (auto-approve safe prompts, escalate the rest, detect completion, exit with a summary)"
        )]
        unattended: bool,
    },

    /// Attach a new worktree + agent pane to a running session
    #[command(
        about = "Attach a new worktree + agent pane to a running session",
        long_about = "Hot-attaches a worktree and agent pane to an already-running session — \
                      no stop/purge/restart, the other agents keep working undisturbed. The \
                      agent grid re-tiles to the layout a start of that many agents would \
                      produce, the new branch is registered in the session, and the agent boots \
                      with the same broker boot block + initial prompt a start-time agent gets.\n\n\
                      Provide a branch name, or use --from-spec to derive the branch (and CLI) \
                      from a discovered spec. Adding past the 25-agent cap is rejected. When the \
                      session is paused, the new pane starts paused too and begins on the next \
                      `git paw start`. The supervisor (if any) discovers the new agent on its \
                      next broker poll — no restart.\n\n\
                      Examples:\n  \
                      git paw add feat/new-thing\n  \
                      git paw add feat/api --cli codex\n  \
                      git paw add --from-spec add-export"
    )]
    Add {
        /// Branch to attach. Omit when using --from-spec (the branch is
        /// derived from the spec).
        #[arg(
            required_unless_present = "from_spec",
            help = "Branch to attach (omit when using --from-spec)"
        )]
        branch: Option<String>,

        /// AI CLI to launch in the new pane (defaults to the session's CLI).
        #[arg(
            long,
            help = "AI CLI for the new pane (defaults to the session's default CLI)"
        )]
        cli: Option<String>,

        /// Resolve the branch name and CLI from a discovered spec instead of a
        /// positional branch argument.
        #[arg(
            long,
            conflicts_with = "branch",
            help = "Derive branch + CLI from a spec (OpenSpec change, Markdown spec, or Spec Kit feature)"
        )]
        from_spec: Option<String>,
    },

    /// Detach a single agent from a running session
    #[command(
        about = "Detach a single agent from a running session",
        long_about = "Removes one agent from an active session: closes its tmux pane, re-tiles \
                      the grid for the smaller agent count, removes its worktree (reusing \
                      `git paw purge`'s per-worktree teardown), and drops it from the session. \
                      The other agents are left untouched.\n\n\
                      Safe by default: `remove` refuses to delete a worktree with uncommitted \
                      changes (it lists what would be lost) unless you pass --force. Pass \
                      --keep-worktree to detach the pane + session entry but leave the worktree \
                      and branch on disk (this skips the uncommitted-work check, since nothing \
                      is deleted). `remove supervisor` is refused — use `git paw stop` to end \
                      the whole session. The supervisor notices the departure on its next broker \
                      poll (the agent stops heartbeating) — no restart.\n\n\
                      Examples:\n  \
                      git paw remove feat/done-thing\n  \
                      git paw remove feat/wip --force\n  \
                      git paw remove feat/keep --keep-worktree"
    )]
    Remove {
        /// Branch of the agent to remove.
        #[arg(help = "Branch of the agent to remove")]
        branch: String,

        /// Detach the pane + session entry but leave the worktree and branch
        /// on disk (skips the uncommitted-work safety check).
        #[arg(
            long,
            help = "Leave the worktree + branch on disk; only detach the pane and session entry"
        )]
        keep_worktree: bool,

        /// Remove the worktree even when it has uncommitted changes.
        #[arg(
            long,
            help = "Remove even with uncommitted changes (bypass the safety check)"
        )]
        force: bool,
    },

    /// Pause the session (detaches client, stops broker, leaves CLIs running)
    #[command(
        about = "Pause the session (detaches client, stops broker, leaves CLIs running)",
        long_about = "Detaches the tmux client and stops the broker, but leaves all CLI \
                      processes running in the background. This preserves agent conversation \
                      state for instant resume via `git paw start`. RAM stays allocated \
                      (~300 MB per Claude pane).\n\n\
                      Use pause for short breaks (lunch, meetings, end-of-day). For longer \
                      breaks, use `git paw stop` to kill the CLIs and release RAM (worktrees \
                      preserved). A future `git paw hibernate` (v1.0.0) will snapshot state \
                      to disk.\n\n\
                      Example:\n  git paw pause"
    )]
    Pause,

    /// Stop the session (kills tmux, keeps worktrees and state)
    #[command(
        about = "Stop the session (kills tmux, keeps worktrees and state)",
        long_about = "Kills the tmux session and every CLI pane process, but preserves \
                      worktrees and session state on disk. CLI conversation context is lost. \
                      Run `git paw start` later to recover the session with fresh CLI \
                      processes.\n\n\
                      Three teardown verbs:\n  \
                      pause — soft stop (detach + broker stop; CLIs keep running, RAM held)\n  \
                      stop  — kills CLI processes; preserves worktrees on disk (this command)\n  \
                      purge — full reset; removes worktrees, branches, and state\n\n\
                      `stop` prompts for confirmation in interactive terminals. Use \
                      `--force` to skip the prompt (scripts) or pipe stdin from \
                      `/dev/null` for non-interactive contexts.\n\n\
                      Examples:\n  git paw stop\n  git paw stop --force"
    )]
    Stop {
        /// Skip confirmation prompt.
        #[arg(long, default_value_t = false, help = "Skip confirmation prompt")]
        force: bool,
    },

    /// Remove everything (tmux session, worktrees, and state)
    #[command(
        about = "Remove everything (tmux session, worktrees, and state)",
        long_about = "Nuclear option: kills the tmux session, removes all worktrees, and deletes \
                      session state. Requires confirmation unless --force is used.\n\n\
                      Use --stale to purge only sessions whose tmux session is gone (a stale \
                      receipt). Live sessions are left untouched, so --stale is safe in cleanup \
                      scripts. Pairing --stale with --force is a no-op (--force is redundant on \
                      a stale entry).\n\n\
                      Examples:\n  git paw purge\n  git paw purge --force\n  git paw purge --stale"
    )]
    Purge {
        /// Skip confirmation prompt.
        #[arg(long, help = "Skip confirmation prompt")]
        force: bool,
        /// Purge only stale sessions (receipt claims active but tmux is gone).
        #[arg(
            long,
            help = "Purge only stale sessions (receipt claims active but tmux is gone); \
                    live sessions untouched"
        )]
        stale: bool,
    },

    /// Show session state for the current repo
    #[command(
        about = "Show session state for the current repo",
        long_about = "Displays the current session status, branches, CLIs, and worktree paths \
                      for the repository in the current directory.\n\n\
                      Status is one of 🟢 active (tmux running), 🔵 paused, 🟡 stopped, or \
                      🔴 stale (the receipt claims active but the tmux session no longer \
                      exists — a crash or release-boundary carry-over). Run `git paw start` to \
                      self-heal a stale receipt, or `git paw purge --stale` to clear it.\n\n\
                      Pass --json for machine-readable output (the `status` field is one of \
                      active/paused/stopped/stale).\n\n\
                      Examples:\n  git paw status\n  git paw status --json"
    )]
    Status {
        /// Emit machine-readable JSON instead of the human-readable display.
        #[arg(long, help = "Emit machine-readable JSON")]
        json: bool,
    },

    /// List detected and custom AI CLIs
    #[command(
        about = "List detected and custom AI CLIs",
        long_about = "Shows all AI CLIs found on PATH (auto-detected) and any custom CLIs \
                      registered in your config.\n\n\
                      Example:\n  git paw list-clis"
    )]
    ListClis,

    /// Register a custom AI CLI
    #[command(
        about = "Register a custom AI CLI",
        long_about = "Adds a custom CLI to your global config (~/.config/git-paw/config.toml). \
                      The command can be an absolute path or a binary name on PATH.\n\n\
                      Examples:\n  \
                      git paw add-cli my-agent /usr/local/bin/my-agent\n  \
                      git paw add-cli my-agent my-agent --display-name \"My Agent\""
    )]
    AddCli {
        /// Name to register the CLI as.
        #[arg(help = "Name to register the CLI as")]
        name: String,

        /// Command or path to the CLI binary.
        #[arg(help = "Command or path to the CLI binary")]
        command: String,

        /// Optional display name for the CLI.
        #[arg(long, help = "Display name shown in prompts")]
        display_name: Option<String>,
    },

    /// Unregister a custom AI CLI
    #[command(
        about = "Unregister a custom AI CLI",
        long_about = "Removes a custom CLI from your global config. Only custom CLIs can be \
                      removed — auto-detected CLIs cannot.\n\n\
                      Example:\n  git paw remove-cli my-agent"
    )]
    RemoveCli {
        /// Name of the custom CLI to remove.
        #[arg(help = "Name of the custom CLI to remove")]
        name: String,
    },

    /// Initialize .git-paw/ directory and configuration
    #[command(
        about = "Initialize .git-paw/ directory and configuration",
        long_about = "Creates the .git-paw/ directory with a default config and sets up \
                      .gitignore for logs.\n\n\
                      Examples:\n  git paw init"
    )]
    Init,

    /// Internal: run the broker and dashboard in pane 0
    #[command(
        hide = true,
        name = "__dashboard",
        about = "Internal: run the broker and dashboard in pane 0",
        long_about = "Internal subcommand used by git-paw to run the broker and dashboard TUI \
                      in pane 0 of a tmux session. Not intended for direct invocation."
    )]
    Dashboard,

    /// View captured session logs
    #[command(
        about = "View captured session logs",
        long_about = "Reads session logs captured by pipe-pane. By default, strips ANSI codes \
                      for clean output. Use --color to view with colors via less -R.\n\n\
                      Examples:\n  \
                      git paw replay --list\n  \
                      git paw replay feat/add-auth\n  \
                      git paw replay feat/add-auth --color\n  \
                      git paw replay feat/add-auth --session paw-myproject"
    )]
    Replay {
        /// Branch name to replay (fuzzy-matched against log filenames).
        #[arg(required_unless_present = "list", help = "Branch to replay")]
        branch: Option<String>,

        /// List available log sessions and branches.
        #[arg(long, help = "List available log sessions and branches")]
        list: bool,

        /// Display with ANSI colors via less -R.
        #[arg(long, help = "Display with colors via less -R")]
        color: bool,

        /// Session name to replay from (defaults to most recent).
        #[arg(long, help = "Session to replay from (defaults to most recent)")]
        session: Option<String>,
    },

    /// Report manually-approved command patterns for a session
    #[command(
        about = "Report manually-approved command patterns for a session",
        long_about = "Lists the command patterns you manually approved during a session — the \
                      prompts the auto-approve preset did NOT match — sorted by how often each \
                      was approved. Each row carries a SUGGEST hint for where the pattern might \
                      be promoted: the project-local allowlist (project-specific scripts/paths) \
                      or the bundled dev-allowlist preset (general commands like `make <target>`). \
                      The suggestion is a hint, not a rule.\n\n\
                      Reads `.git-paw/sessions/<session>.manual-approvals.jsonl`. Defaults to the \
                      active session; pass --session to target another. Recording is controlled by \
                      `[supervisor] manual_approvals_log` (default on).\n\n\
                      Examples:\n  \
                      git paw approvals\n  \
                      git paw approvals --json\n  \
                      git paw approvals --session paw-myproject\n  \
                      git paw approvals --limit 5"
    )]
    Approvals {
        /// Session to read approvals from (defaults to the active session).
        #[arg(long, help = "Session to read from (defaults to the active session)")]
        session: Option<String>,

        /// Cap the output to the top N patterns by count.
        #[arg(long, help = "Show at most N patterns (top N by count)")]
        limit: Option<usize>,

        /// Emit machine-readable JSON instead of the text table.
        #[arg(long, help = "Emit machine-readable JSON")]
        json: bool,
    },

    /// Run a read-only Model Context Protocol (MCP) server over stdio
    #[command(
        about = "Run a read-only Model Context Protocol (MCP) server over stdio",
        long_about = "Starts a Model Context Protocol (MCP) server on stdin/stdout so any \
                      MCP-aware client (Claude Desktop, Cursor, ChatGPT Desktop, Windsurf, \
                      VS Code MCP) can query this repository's read-only state: agent \
                      coordination intents/conflicts, governance docs, specs and tasks, \
                      session status and learnings, agent skills, git context, source \
                      browsing (list_files, read_file, search_code over the local working \
                      tree), and the repository's own README and documentation.\n\n\
                      The server is client-spawned and one-shot: the MCP client owns the \
                      process lifecycle and the server exits when stdin is closed. It runs \
                      standalone — no tmux session, broker, or supervisor is required. When a \
                      data source is unavailable (no broker, no session, no governance config) \
                      tools return well-formed empty/null results rather than errors.\n\n\
                      Repository resolution: --repo wins; otherwise the nearest ancestor of the \
                      current directory containing .git is used (worktrees resolve to their own \
                      root). Claude Desktop spawns servers from its app-support directory, so it \
                      MUST pass --repo with an absolute path.\n\n\
                      Claude Desktop config (claude_desktop_config.json):\n\n  \
                      {\n    \
                        \"mcpServers\": {\n      \
                          \"git-paw\": {\n        \
                            \"command\": \"git\",\n        \
                            \"args\": [\"paw\", \"mcp\", \"--repo\", \"/absolute/path/to/your/repo\"]\n      \
                          }\n    \
                        }\n  \
                      }\n\n\
                      Examples:\n  \
                      git paw mcp\n  \
                      git paw mcp --repo /path/to/repo\n  \
                      git paw mcp --repo /path/to/repo --log-file /tmp/git-paw-mcp.log"
    )]
    Mcp {
        /// Repository to operate against, overriding current-directory
        /// discovery. Required for clients that spawn from a fixed directory
        /// (notably Claude Desktop).
        #[arg(
            long,
            value_name = "PATH",
            help = "Repository to serve (overrides current-directory discovery; required for Claude Desktop)"
        )]
        repo: Option<PathBuf>,

        /// Write tracing output to this file in addition to stderr (off by
        /// default). Stdout always stays reserved for the JSON-RPC stream.
        #[arg(
            long,
            value_name = "PATH",
            help = "Also write tracing output to this file (stderr is always used)"
        )]
        log_file: Option<PathBuf>,
    },

    /// Diagnose the environment, configuration, and repository state
    #[command(
        about = "Diagnose the environment, configuration, and repository state",
        long_about = "Runs read-only preflight checks and prints a grouped report answering \
                      \"why won't it launch?\". Checks are grouped under Environment, CLIs, \
                      Config, Spec system, Bundled scripts, Broker, Supervisor, and Hygiene; \
                      each one reports \u{2713} (pass), \u{26a0} (warning), or \u{2717} \
                      (failure), and every non-\u{2713} check prints an actionable remedy.\n\n\
                      Doctor diagnoses but never repairs: no file, config, session, or other \
                      persistent state is created, modified, or deleted. Apply the remedies \
                      yourself (most are `git paw init`).\n\n\
                      Exits non-zero when any check is \u{2717}, and zero when the worst status \
                      is \u{26a0} or better \u{2014} so it works as a pre-launch or CI gate that \
                      blocks only on true blockers. `--json` emits the same checks as one \
                      machine-readable document with an identical exit-code contract.\n\n\
                      Companion command: the hidden `git paw selftest` runs a live \
                      session-lifecycle smoke check, where doctor inspects statically.\n\n\
                      Examples:\n  \
                      git paw doctor\n  \
                      git paw doctor --json"
    )]
    Doctor {
        /// Emit machine-readable JSON instead of the grouped report.
        #[arg(long, help = "Emit machine-readable JSON")]
        json: bool,
    },

    /// Run an isolated end-to-end session-lifecycle smoke check (internal; the live arm of `doctor`)
    #[command(
        hide = true,
        about = "Run an isolated end-to-end session-lifecycle smoke check",
        long_about = "Exercises the full session lifecycle (start \u{2192} add \u{2192} remove \u{2192} \
                      stop) against a throwaway repository and a dummy CLI, then reports a single \
                      pass/fail verdict. No real AI CLI backend (LLM) and no interactive terminal \
                      are required.\n\n\
                      The harness isolates every external resource so it never disturbs your live \
                      work: a private tmux socket (it does not touch your default tmux socket), an \
                      OS-assigned ephemeral broker port, an isolated HOME (your real sessions \
                      directory is untouched), and a throwaway git repository under .git-paw/tmp/. \
                      The session boots in detached mode with a dummy CLI (`cat`) in place of a \
                      real agent CLI, so no LLM process is spawned. All artifacts are cleaned up \
                      on both the success and failure paths.\n\n\
                      Exits 0 when the lifecycle completes (printing `selftest passed`), or skips \
                      with a message and exits 0 when tmux is unavailable. Exits non-zero and \
                      names the failing step only on an actual lifecycle failure.\n\n\
                      This is an internal diagnostic (hidden from the command list) \u{2014} the live \
                      smoke-test arm of `git paw doctor`, whose static preflight checks it \
                      complements.\n\n\
                      Example:\n  git paw selftest"
    )]
    Selftest,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// Helper: parse args as if running `git-paw <args>`.
    fn parse(args: &[&str]) -> Cli {
        let mut full = vec!["git-paw"];
        full.extend(args);
        Cli::try_parse_from(full).expect("failed to parse")
    }

    // -- Default subcommand --

    #[test]
    fn no_args_defaults_to_none_command() {
        let cli = parse(&[]);
        assert!(
            cli.command.is_none(),
            "no args should yield None (handled as Start in main)"
        );
    }

    // -- Start subcommand --

    /// Parsed `Command::Start` fields, mirrored so a single table can assert
    /// the full flag surface per row. `Default` is the no-flag baseline; each
    /// row overrides only the fields its args set, so the full-struct compare
    /// also pins every unset flag at its default (the "flag X defaults to
    /// false/None" cases).
    #[derive(Debug, Default, PartialEq)]
    #[allow(clippy::struct_excessive_bools)]
    struct StartFields {
        cli: Option<String>,
        branches: Option<Vec<String>>,
        from_all_specs: bool,
        specs: Option<Vec<String>>,
        specs_format: Option<SpecsFormat>,
        dry_run: bool,
        preset: Option<String>,
        supervisor: bool,
        no_supervisor: bool,
        force: bool,
        no_rebase: bool,
        unattended: bool,
    }

    fn start_fields(args: &[&str]) -> StartFields {
        match parse(args).command.unwrap() {
            Command::Start {
                cli,
                branches,
                from_all_specs,
                specs,
                specs_format,
                dry_run,
                preset,
                supervisor,
                no_supervisor,
                force,
                no_rebase,
                unattended,
            } => StartFields {
                cli,
                branches,
                from_all_specs,
                specs,
                specs_format,
                dry_run,
                preset,
                supervisor,
                no_supervisor,
                force,
                no_rebase,
                unattended,
            },
            other => panic!("expected Start, got {other:?}"),
        }
    }

    fn strs(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    /// One row per `git paw start` flag / flag-combo -> the fully-parsed
    /// `Command::Start` field set. Because the compare is full-struct, an unset
    /// flag is asserted at its default on every row that omits it.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn start_flag_combinations_parse_to_expected_fields() {
        for (args, expected) in [
            (vec!["start"], StartFields::default()),
            (
                vec!["start", "--cli", "claude"],
                StartFields {
                    cli: Some("claude".to_string()),
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--unattended"],
                StartFields {
                    unattended: true,
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--unattended", "--from-specs", "--cli", "claude"],
                StartFields {
                    unattended: true,
                    from_all_specs: true,
                    cli: Some("claude".to_string()),
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--from-all-specs"],
                StartFields {
                    from_all_specs: true,
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--specs"],
                StartFields {
                    specs: Some(vec![]),
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--specs", "add-auth"],
                StartFields {
                    specs: Some(strs(&["add-auth"])),
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--specs", "add-auth,fix-session"],
                StartFields {
                    specs: Some(strs(&["add-auth", "fix-session"])),
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--specs", "add-auth,fix-session,add-logging"],
                StartFields {
                    specs: Some(strs(&["add-auth", "fix-session", "add-logging"])),
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--from-all-specs", "--supervisor"],
                StartFields {
                    from_all_specs: true,
                    supervisor: true,
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--supervisor"],
                StartFields {
                    supervisor: true,
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--branches", "feat/a,feat/b,fix/c"],
                StartFields {
                    branches: Some(strs(&["feat/a", "feat/b", "fix/c"])),
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--dry-run"],
                StartFields {
                    dry_run: true,
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--preset", "backend"],
                StartFields {
                    preset: Some("backend".to_string()),
                    ..Default::default()
                },
            ),
            (
                vec![
                    "start",
                    "--supervisor",
                    "--cli",
                    "claude",
                    "--branches",
                    "feat/a,feat/b",
                ],
                StartFields {
                    supervisor: true,
                    cli: Some("claude".to_string()),
                    branches: Some(strs(&["feat/a", "feat/b"])),
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--from-specs", "--specs-format", "speckit"],
                StartFields {
                    from_all_specs: true,
                    specs_format: Some(SpecsFormat::Speckit),
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--from-specs", "--specs-format", "superpowers"],
                StartFields {
                    from_all_specs: true,
                    specs_format: Some(SpecsFormat::Superpowers),
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--from-specs", "--specs-format", "openspec"],
                StartFields {
                    from_all_specs: true,
                    specs_format: Some(SpecsFormat::Openspec),
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--from-specs", "--specs-format", "markdown"],
                StartFields {
                    from_all_specs: true,
                    specs_format: Some(SpecsFormat::Markdown),
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--no-supervisor"],
                StartFields {
                    no_supervisor: true,
                    ..Default::default()
                },
            ),
            (
                vec![
                    "start",
                    "--no-supervisor",
                    "--cli",
                    "claude",
                    "--branches",
                    "feat/a,feat/b",
                ],
                StartFields {
                    no_supervisor: true,
                    cli: Some("claude".to_string()),
                    branches: Some(strs(&["feat/a", "feat/b"])),
                    ..Default::default()
                },
            ),
            (
                vec![
                    "start",
                    "--cli",
                    "gemini",
                    "--branches",
                    "a,b",
                    "--dry-run",
                    "--preset",
                    "dev",
                ],
                StartFields {
                    cli: Some("gemini".to_string()),
                    branches: Some(strs(&["a", "b"])),
                    dry_run: true,
                    preset: Some("dev".to_string()),
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--no-rebase"],
                StartFields {
                    no_rebase: true,
                    ..Default::default()
                },
            ),
            (
                vec!["start", "--no-rebase", "--supervisor"],
                StartFields {
                    no_rebase: true,
                    supervisor: true,
                    ..Default::default()
                },
            ),
        ] {
            assert_eq!(start_fields(&args), expected, "args: {args:?}");
        }
    }

    // -- Task 1.3 / cli-parsing: --unattended flag --

    #[test]
    fn start_unattended_with_no_supervisor_is_rejected() {
        let result = Cli::try_parse_from(["git-paw", "start", "--unattended", "--no-supervisor"]);
        assert!(
            result.is_err(),
            "--unattended and --no-supervisor express opposing intents and must conflict"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("--unattended") && msg.contains("--no-supervisor"),
            "the parse error should name both conflicting flags; got: {msg}"
        );
    }

    #[test]
    fn start_with_from_specs_alias_parses_identically_to_from_all_specs() {
        let alias_args = parse(&["start", "--from-specs"]);
        let canonical_args = parse(&["start", "--from-all-specs"]);
        match (alias_args.command.unwrap(), canonical_args.command.unwrap()) {
            (
                Command::Start {
                    from_all_specs: a_all,
                    specs: a_specs,
                    supervisor: a_sup,
                    ..
                },
                Command::Start {
                    from_all_specs: c_all,
                    specs: c_specs,
                    supervisor: c_sup,
                    ..
                },
            ) => {
                assert_eq!(a_all, c_all);
                assert_eq!(a_specs, c_specs);
                assert_eq!(a_sup, c_sup);
                assert!(a_all);
            }
            other => panic!("expected two Start variants, got {other:?}"),
        }
    }

    #[test]
    fn start_with_from_all_specs_and_specs_is_rejected() {
        let result = Cli::try_parse_from([
            "git-paw",
            "start",
            "--from-all-specs",
            "--specs",
            "add-auth",
        ]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("--from-all-specs"), "got: {err}");
        assert!(err.contains("--specs"), "got: {err}");
    }

    #[test]
    fn start_with_from_specs_alias_and_specs_is_rejected() {
        let result =
            Cli::try_parse_from(["git-paw", "start", "--from-specs", "--specs", "add-auth"]);
        assert!(result.is_err());
    }

    #[test]
    fn start_help_contains_from_all_specs_and_specs_but_not_alias() {
        let result = Cli::try_parse_from(["git-paw", "start", "--help"]);
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
        let help = err.to_string();
        assert!(
            help.contains("--from-all-specs"),
            "start --help should contain --from-all-specs; got: {help}"
        );
        assert!(
            help.contains("--specs"),
            "start --help should contain --specs; got: {help}"
        );
        assert!(
            !help.contains("--from-specs"),
            "start --help should NOT contain hidden alias --from-specs; got: {help}"
        );
    }

    // -- --specs-format flag --

    #[test]
    fn specs_format_superpowers_maps_to_backend_string() {
        assert_eq!(SpecsFormat::Superpowers.as_str(), "superpowers");
    }

    /// Standing guard (spec-traceability-audit C7): every `SpecsFormat` variant's
    /// wire string appears in the `--specs-format` `--help`, so a new backend
    /// can't ship without being surfaced (caught `superpowers` missing).
    #[test]
    fn specs_format_help_lists_every_variant() {
        use clap::CommandFactory;
        let cmd = Cli::command();
        let start = cmd
            .find_subcommand("start")
            .expect("start subcommand exists");
        let arg = start
            .get_arguments()
            .find(|a| a.get_id().as_str() == "specs_format")
            .expect("start has a --specs-format argument");
        let help = arg
            .get_help()
            .map(std::string::ToString::to_string)
            .unwrap_or_default();
        for variant in [
            SpecsFormat::Openspec,
            SpecsFormat::Markdown,
            SpecsFormat::Speckit,
            SpecsFormat::Superpowers,
        ] {
            assert!(
                help.contains(variant.as_str()),
                "--specs-format help must list `{}`; got: {help}",
                variant.as_str()
            );
        }
    }

    #[test]
    fn start_rejects_unknown_specs_format() {
        let result = Cli::try_parse_from([
            "git-paw",
            "start",
            "--from-specs",
            "--specs-format",
            "unknown-value",
        ]);
        assert!(result.is_err(), "unknown value should be rejected");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("openspec") && err.contains("markdown") && err.contains("speckit"),
            "error should list all three valid values, got: {err}"
        );
    }

    #[test]
    fn specs_format_as_str_matches_backend_names() {
        assert_eq!(SpecsFormat::Openspec.as_str(), "openspec");
        assert_eq!(SpecsFormat::Markdown.as_str(), "markdown");
        assert_eq!(SpecsFormat::Speckit.as_str(), "speckit");
    }

    // -- --no-supervisor flag --

    #[test]
    fn start_with_supervisor_and_no_supervisor_is_rejected() {
        let result = Cli::try_parse_from(["git-paw", "start", "--supervisor", "--no-supervisor"]);
        assert!(
            result.is_err(),
            "--supervisor + --no-supervisor must be rejected by clap"
        );
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("--supervisor") && msg.contains("--no-supervisor"),
            "error should mention both flags, got: {msg}"
        );
    }

    #[test]
    fn start_with_no_supervisor_and_supervisor_reversed_is_also_rejected() {
        // clap's conflicts_with is bidirectional; order of flags shouldn't matter.
        let result = Cli::try_parse_from(["git-paw", "start", "--no-supervisor", "--supervisor"]);
        assert!(result.is_err());
    }

    // -- Add subcommand --

    #[test]
    fn add_with_branch_only() {
        let cli = parse(&["add", "feat/new"]);
        match cli.command.unwrap() {
            Command::Add {
                branch,
                cli,
                from_spec,
            } => {
                assert_eq!(branch.as_deref(), Some("feat/new"));
                assert!(cli.is_none());
                assert!(from_spec.is_none());
            }
            other => panic!("expected Add, got {other:?}"),
        }
    }

    #[test]
    fn add_with_branch_and_cli() {
        let cli = parse(&["add", "feat/x", "--cli", "codex"]);
        match cli.command.unwrap() {
            Command::Add { branch, cli, .. } => {
                assert_eq!(branch.as_deref(), Some("feat/x"));
                assert_eq!(cli.as_deref(), Some("codex"));
            }
            other => panic!("expected Add, got {other:?}"),
        }
    }

    #[test]
    fn add_with_from_spec_only_needs_no_branch() {
        let cli = parse(&["add", "--from-spec", "add-export"]);
        match cli.command.unwrap() {
            Command::Add {
                branch, from_spec, ..
            } => {
                assert!(branch.is_none());
                assert_eq!(from_spec.as_deref(), Some("add-export"));
            }
            other => panic!("expected Add, got {other:?}"),
        }
    }

    #[test]
    fn add_with_no_branch_and_no_from_spec_is_rejected() {
        let result = Cli::try_parse_from(["git-paw", "add"]);
        assert!(
            result.is_err(),
            "add requires either a branch or --from-spec"
        );
    }

    #[test]
    fn add_with_branch_and_from_spec_is_rejected() {
        let result = Cli::try_parse_from(["git-paw", "add", "feat/x", "--from-spec", "change"]);
        assert!(
            result.is_err(),
            "branch and --from-spec are mutually exclusive"
        );
    }

    // -- Remove subcommand --

    #[test]
    fn remove_with_branch_only() {
        let cli = parse(&["remove", "feat/done"]);
        match cli.command.unwrap() {
            Command::Remove {
                branch,
                keep_worktree,
                force,
            } => {
                assert_eq!(branch, "feat/done");
                assert!(!keep_worktree);
                assert!(!force);
            }
            other => panic!("expected Remove, got {other:?}"),
        }
    }

    #[test]
    fn remove_with_keep_worktree_and_force() {
        let cli = parse(&["remove", "feat/x", "--keep-worktree", "--force"]);
        match cli.command.unwrap() {
            Command::Remove {
                keep_worktree,
                force,
                ..
            } => {
                assert!(keep_worktree);
                assert!(force);
            }
            other => panic!("expected Remove, got {other:?}"),
        }
    }

    #[test]
    fn remove_without_branch_is_rejected() {
        let result = Cli::try_parse_from(["git-paw", "remove"]);
        assert!(result.is_err(), "remove requires a branch");
    }

    // -- Pause subcommand --

    #[test]
    fn pause_parses() {
        let cli = parse(&["pause"]);
        assert!(matches!(cli.command.unwrap(), Command::Pause));
    }

    #[test]
    fn pause_help_mentions_ram_tradeoff() {
        let result = Cli::try_parse_from(["git-paw", "pause", "--help"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
        let help = err.to_string();
        assert!(
            help.to_lowercase().contains("ram"),
            "pause --help should mention RAM tradeoff, got: {help}"
        );
        assert!(
            help.contains("stop"),
            "pause --help should cross-reference stop, got: {help}"
        );
    }

    #[test]
    fn pause_rejects_unknown_flags() {
        let result = Cli::try_parse_from(["git-paw", "pause", "--anything"]);
        assert!(result.is_err(), "pause should reject unknown flags");
    }

    // -- Stop subcommand --

    /// One row per `git paw stop` invocation -> the parsed `force` flag.
    #[test]
    fn stop_force_flag_parses() {
        for (args, expected_force) in [(vec!["stop"], false), (vec!["stop", "--force"], true)] {
            match parse(&args).command.unwrap() {
                Command::Stop { force } => assert_eq!(force, expected_force, "args: {args:?}"),
                other => panic!("expected Stop, got {other:?}"),
            }
        }
    }

    // -- Purge subcommand --

    /// One row per `git paw purge` flag-combo -> the parsed `force` / `stale`
    /// flags (each of the four combinations).
    #[test]
    fn purge_flag_combinations_parse() {
        for (args, expected_force, expected_stale) in [
            (vec!["purge"], false, false),
            (vec!["purge", "--force"], true, false),
            (vec!["purge", "--stale"], false, true),
            (vec!["purge", "--stale", "--force"], true, true),
        ] {
            match parse(&args).command.unwrap() {
                Command::Purge { force, stale } => {
                    assert_eq!(force, expected_force, "force for {args:?}");
                    assert_eq!(stale, expected_stale, "stale for {args:?}");
                }
                other => panic!("expected Purge, got {other:?}"),
            }
        }
    }

    // -- Status subcommand --

    /// One row per `git paw status` invocation -> the parsed `json` flag.
    #[test]
    fn status_json_flag_parses() {
        for (args, expected_json) in [(vec!["status"], false), (vec!["status", "--json"], true)] {
            match parse(&args).command.unwrap() {
                Command::Status { json } => assert_eq!(json, expected_json, "args: {args:?}"),
                other => panic!("expected Status, got {other:?}"),
            }
        }
    }

    // -- List-CLIs subcommand --

    #[test]
    fn list_clis_parses() {
        let cli = parse(&["list-clis"]);
        assert!(matches!(cli.command.unwrap(), Command::ListClis));
    }

    // -- Add-CLI subcommand --

    #[test]
    fn add_cli_with_required_args() {
        let cli = parse(&["add-cli", "my-agent", "/usr/local/bin/my-agent"]);
        match cli.command.unwrap() {
            Command::AddCli {
                name,
                command,
                display_name,
            } => {
                assert_eq!(name, "my-agent");
                assert_eq!(command, "/usr/local/bin/my-agent");
                assert!(display_name.is_none());
            }
            other => panic!("expected AddCli, got {other:?}"),
        }
    }

    #[test]
    fn add_cli_with_display_name() {
        let cli = parse(&[
            "add-cli",
            "my-agent",
            "my-agent",
            "--display-name",
            "My Agent",
        ]);
        match cli.command.unwrap() {
            Command::AddCli {
                name,
                command,
                display_name,
            } => {
                assert_eq!(name, "my-agent");
                assert_eq!(command, "my-agent");
                assert_eq!(display_name.as_deref(), Some("My Agent"));
            }
            other => panic!("expected AddCli, got {other:?}"),
        }
    }

    // -- Remove-CLI subcommand --

    #[test]
    fn remove_cli_parses() {
        let cli = parse(&["remove-cli", "my-agent"]);
        match cli.command.unwrap() {
            Command::RemoveCli { name } => assert_eq!(name, "my-agent"),
            other => panic!("expected RemoveCli, got {other:?}"),
        }
    }

    // -- Help text quality --

    /// One row per subcommand: `<sub> --help` renders help (`DisplayHelp`) and
    /// surfaces every listed flag / copy-pasteable example anchor. Merges the
    /// per-flag `*_help_shows_*` and `*_help_lists_flags_and_examples` greps
    /// into a single table.
    #[test]
    fn subcommand_help_surfaces_expected_anchors() {
        let cases: &[(&[&str], &[&str])] = &[
            (
                &["start", "--help"],
                &[
                    "--specs-format",
                    "--supervisor",
                    "--no-supervisor",
                    "--no-rebase",
                ],
            ),
            (
                &["add", "--help"],
                &["--cli", "--from-spec", "git paw add feat/api --cli codex"],
            ),
            (
                &["remove", "--help"],
                &[
                    "--keep-worktree",
                    "--force",
                    "git paw remove feat/wip --force",
                ],
            ),
            (&["stop", "--help"], &["pause", "purge", "--force"]),
            (
                &["approvals", "--help"],
                &["--session", "--limit", "--json", "git paw approvals --json"],
            ),
            (&["replay", "--help"], &["--list", "--color", "--session"]),
        ];
        for &(args, anchors) in cases {
            let mut full = vec!["git-paw"];
            full.extend_from_slice(args);
            let err = Cli::try_parse_from(full).expect_err("--help should display help");
            assert_eq!(
                err.kind(),
                clap::error::ErrorKind::DisplayHelp,
                "kind for {args:?}"
            );
            let help = err.to_string();
            for &anchor in anchors {
                assert!(
                    help.contains(anchor),
                    "{args:?} --help should surface {anchor:?}; got: {help}"
                );
            }
        }
    }

    /// The root `--help` lists every public subcommand plus the quick-start
    /// reference. Merges the individual `help_shows_*` / `root_help_lists_*`
    /// greps.
    #[test]
    fn root_help_lists_public_subcommands() {
        let err = Cli::try_parse_from(["git-paw", "--help"]).expect_err("--help displays help");
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
        let help = err.to_string();
        for anchor in [
            "add",
            "remove",
            "pause",
            "git paw pause",
            "replay",
            "approvals",
            "mcp",
        ] {
            assert!(
                help.contains(anchor),
                "root --help should surface {anchor:?}; got: {help}"
            );
        }
    }

    /// Hidden / internal subcommands never appear in root `--help`. Merges the
    /// `dashboard_does_not_appear_in_help` and `selftest_is_hidden_from_help`
    /// guards.
    #[test]
    fn root_help_hides_internal_subcommands() {
        let help = Cli::try_parse_from(["git-paw", "--help"])
            .unwrap_err()
            .to_string();
        for hidden in ["__dashboard", "selftest"] {
            assert!(
                !help.contains(hidden),
                "root --help must not surface internal subcommand {hidden:?}; got: {help}"
            );
        }
    }

    #[test]
    fn version_flag_is_accepted() {
        let result = Cli::try_parse_from(["git-paw", "--version"]);
        // clap returns Err(DisplayVersion) for --version, which is expected
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn help_flag_is_accepted() {
        let result = Cli::try_parse_from(["git-paw", "--help"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    // --- Gap #6: init_parses ---

    #[test]
    fn init_parses() {
        let cli = parse(&["init"]);
        assert!(matches!(cli.command.unwrap(), Command::Init));
    }

    // --- Gap #7: init_help_text ---

    #[test]
    fn init_help_text() {
        let result = Cli::try_parse_from(["git-paw", "init", "--help"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn unknown_subcommand_is_rejected() {
        let result = Cli::try_parse_from(["git-paw", "unknown-command"]);
        assert!(result.is_err());
    }

    #[test]
    fn add_cli_missing_required_args_is_rejected() {
        let result = Cli::try_parse_from(["git-paw", "add-cli"]);
        assert!(result.is_err());
    }

    // -- Replay subcommand --

    #[test]
    fn replay_with_branch() {
        let cli = parse(&["replay", "feat/add-auth"]);
        match cli.command.unwrap() {
            Command::Replay {
                branch,
                list,
                color,
                session,
            } => {
                assert_eq!(branch.as_deref(), Some("feat/add-auth"));
                assert!(!list);
                assert!(!color);
                assert!(session.is_none());
            }
            other => panic!("expected Replay, got {other:?}"),
        }
    }

    #[test]
    fn replay_with_list() {
        let cli = parse(&["replay", "--list"]);
        match cli.command.unwrap() {
            Command::Replay { branch, list, .. } => {
                assert!(list);
                assert!(branch.is_none());
            }
            other => panic!("expected Replay, got {other:?}"),
        }
    }

    #[test]
    fn replay_with_color() {
        let cli = parse(&["replay", "feat/add-auth", "--color"]);
        match cli.command.unwrap() {
            Command::Replay { color, .. } => assert!(color),
            other => panic!("expected Replay, got {other:?}"),
        }
    }

    #[test]
    fn replay_with_session() {
        let cli = parse(&["replay", "feat/add-auth", "--session", "paw-myproject"]);
        match cli.command.unwrap() {
            Command::Replay { session, .. } => {
                assert_eq!(session.as_deref(), Some("paw-myproject"));
            }
            other => panic!("expected Replay, got {other:?}"),
        }
    }

    #[test]
    fn replay_no_args_fails() {
        let result = Cli::try_parse_from(["git-paw", "replay"]);
        assert!(result.is_err());
    }

    // -- __dashboard subcommand --

    #[test]
    fn dashboard_parses() {
        let cli = parse(&["__dashboard"]);
        assert!(matches!(cli.command.unwrap(), Command::Dashboard));
    }

    // -- Approvals subcommand --

    #[test]
    fn approvals_parses_with_no_flags() {
        let cli = parse(&["approvals"]);
        match cli.command.unwrap() {
            Command::Approvals {
                session,
                limit,
                json,
            } => {
                assert!(session.is_none());
                assert!(limit.is_none());
                assert!(!json);
            }
            other => panic!("expected Approvals, got {other:?}"),
        }
    }

    #[test]
    fn approvals_with_session_limit_and_json() {
        let cli = parse(&[
            "approvals",
            "--session",
            "paw-other",
            "--limit",
            "5",
            "--json",
        ]);
        match cli.command.unwrap() {
            Command::Approvals {
                session,
                limit,
                json,
            } => {
                assert_eq!(session.as_deref(), Some("paw-other"));
                assert_eq!(limit, Some(5));
                assert!(json);
            }
            other => panic!("expected Approvals, got {other:?}"),
        }
    }

    #[test]
    fn approvals_rejects_non_numeric_limit() {
        let result = Cli::try_parse_from(["git-paw", "approvals", "--limit", "lots"]);
        assert!(result.is_err());
    }

    // -- Mcp subcommand --

    #[test]
    fn mcp_parses_with_no_flags() {
        let cli = parse(&["mcp"]);
        match cli.command.unwrap() {
            Command::Mcp { repo, log_file } => {
                assert!(repo.is_none());
                assert!(log_file.is_none());
            }
            other => panic!("expected Mcp, got {other:?}"),
        }
    }

    #[test]
    fn mcp_parses_with_repo() {
        let cli = parse(&["mcp", "--repo", "/path/to/repo"]);
        match cli.command.unwrap() {
            Command::Mcp { repo, .. } => {
                assert_eq!(repo.as_deref(), Some(std::path::Path::new("/path/to/repo")));
            }
            other => panic!("expected Mcp, got {other:?}"),
        }
    }

    #[test]
    fn mcp_parses_with_repo_and_log_file() {
        let cli = parse(&["mcp", "--repo", "/r", "--log-file", "/tmp/mcp.log"]);
        match cli.command.unwrap() {
            Command::Mcp { repo, log_file } => {
                assert_eq!(repo.as_deref(), Some(std::path::Path::new("/r")));
                assert_eq!(
                    log_file.as_deref(),
                    Some(std::path::Path::new("/tmp/mcp.log"))
                );
            }
            other => panic!("expected Mcp, got {other:?}"),
        }
    }

    #[test]
    fn mcp_rejects_daemon_and_http_flags() {
        // v0.7.0 ships stdio only — no --port / --host / --daemon / start / stop / status.
        for bad in [
            vec!["mcp", "--port", "9119"],
            vec!["mcp", "--host", "127.0.0.1"],
            vec!["mcp", "--daemon"],
            vec!["mcp", "start"],
            vec!["mcp", "stop"],
            vec!["mcp", "status"],
        ] {
            let mut full = vec!["git-paw"];
            full.extend(bad.iter().copied());
            assert!(
                Cli::try_parse_from(&full).is_err(),
                "mcp should reject {bad:?} in v0.7.0"
            );
        }
    }

    #[test]
    fn mcp_help_describes_supported_flags_and_config_snippet() {
        let result = Cli::try_parse_from(["git-paw", "mcp", "--help"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
        let help = err.to_string();
        assert!(
            help.contains("--repo"),
            "mcp --help should list --repo; got: {help}"
        );
        assert!(
            help.contains("--log-file"),
            "mcp --help should list --log-file; got: {help}"
        );
        assert!(
            help.contains("mcpServers"),
            "mcp --help should include a copy-pasteable Claude Desktop config snippet; got: {help}"
        );
        for forbidden in ["--port", "--host", "--daemon"] {
            assert!(
                !help.contains(forbidden),
                "mcp --help must not advertise {forbidden}; got: {help}"
            );
        }
    }

    // -- Doctor subcommand --

    #[test]
    fn doctor_parses_with_and_without_json() {
        for (args, expected_json) in [(vec!["doctor"], false), (vec!["doctor", "--json"], true)] {
            match parse(&args).command.unwrap() {
                Command::Doctor { json } => {
                    assert_eq!(json, expected_json, "args: {args:?}");
                }
                other => panic!("expected Doctor for {args:?}, got: {other:?}"),
            }
        }
    }

    #[test]
    fn doctor_help_describes_the_groups_and_does_not_advertise_a_repair_flag() {
        let result = Cli::try_parse_from(["git-paw", "doctor", "--help"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
        let help = err.to_string();

        for group in ["Environment", "Config", "Broker", "Supervisor", "Hygiene"] {
            assert!(
                help.contains(group),
                "doctor --help should name the '{group}' group; got: {help}"
            );
        }
        assert!(
            help.contains("--json"),
            "doctor --help should list --json; got: {help}"
        );
        assert!(
            help.contains("git paw doctor"),
            "doctor --help should include a usage example; got: {help}"
        );
        // Diagnose-only in v0.13.0: no repair mode is offered anywhere in help.
        for forbidden in ["--fix", "--repair"] {
            assert!(
                !help.contains(forbidden),
                "doctor is diagnose-only and must not advertise {forbidden}; got: {help}"
            );
        }
    }

    #[test]
    fn doctor_rejects_unknown_flags() {
        for flag in ["--fix", "--repair", "--anything"] {
            assert!(
                Cli::try_parse_from(["git-paw", "doctor", flag]).is_err(),
                "doctor should reject {flag}"
            );
        }
    }

    // -- Selftest subcommand --

    #[test]
    fn selftest_parses_with_no_args() {
        let cli = parse(&["selftest"]);
        assert!(matches!(cli.command.unwrap(), Command::Selftest));
    }

    #[test]
    fn selftest_rejects_unknown_flags() {
        let result = Cli::try_parse_from(["git-paw", "selftest", "--anything"]);
        assert!(result.is_err(), "selftest takes no flags");
    }

    #[test]
    fn selftest_help_describes_isolated_lifecycle_with_example() {
        let result = Cli::try_parse_from(["git-paw", "selftest", "--help"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
        let help = err.to_string();
        assert!(
            help.contains("dummy CLI"),
            "selftest --help should describe the dummy CLI; got: {help}"
        );
        assert!(
            help.to_lowercase().contains("no real ai cli")
                || help.to_lowercase().contains("no llm")
                || help.to_lowercase().contains("real ai cli backend"),
            "selftest --help should say no real LLM backend is required; got: {help}"
        );
        assert!(
            help.contains("git paw selftest"),
            "selftest --help should include a usage example; got: {help}"
        );
    }
}

## Why

Today git-paw only delivers value once a multi-pane session is running. A
developer who wants to inspect specs, intents, learnings, or governance
docs from a chat client (Claude Desktop, Cursor, any MCP-aware client)
has no programmatic surface — they have to read the repo by hand. v0.6.0
positions git-paw as the universal "developer-MCP-for-this-repo" by
exposing read-only state via the standard Model Context Protocol.
Standalone operation matters: the MCP server must run without requiring
an active tmux session, broker, or supervisor so it can answer "what's
in this repo?" before any agents launch.

## What Changes

- New `git paw mcp` subcommand that starts an MCP server on **stdio**
  (per the MCP spec), one-shot and client-spawned. The MCP client owns
  the process lifecycle.
- `--repo <path>` flag overrides the default CWD-based repo discovery
  so non-workspace-aware clients (notably Claude Desktop) can target
  a specific repo via the spawn args.
- New `src/mcp/` module implementing the MCP protocol over stdio,
  tool registration, and a degradation layer that returns empty
  results / null states (not errors) when no session is active.
- Read-only tool surface across five categories:
  - **Coordination**: `get_intents()`, `get_intent(branch_id)`,
    `get_conflicts()` — index active `agent.intent` messages and
    conflict events from the broker (empty when no broker is running).
  - **Governance**: `get_adrs()`, `get_adr(query)`,
    `get_test_strategy()`, `get_security_checklist()`, `get_dod()`,
    `check_dod(branch)`, `get_constitution()` — serve docs from
    `[governance]` configured paths.
  - **Project knowledge**: `get_specs()`, `get_spec(id)`,
    `get_tasks()`, `get_task(id)`, `get_dependency_graph()` —
    index OpenSpec / Markdown / Spec Kit specs and tasks.
  - **Session state**: `get_session_status()`, `get_session_summary()`,
    `get_learnings()` — read session JSON + learnings file (empty
    when no session is active).
  - **Git context**: `get_branches()`, `get_recent_commits(branch)`,
    `get_diff(branch)` — wrap `std::process::Command` git invocations.
- Graceful degradation: every tool returns empty / null results
  rather than erroring when its data source is unavailable (no
  broker, no session, no governance docs configured).
- Detailed setup documentation in the mdBook user guide covering
  Claude Desktop, ChatGPT Desktop, Cursor, VS Code MCP, and
  Windsurf — including the per-repo config patterns and known
  client-specific quirks.
- Write tools (create/modify specs, control sessions, deliver
  agent feedback) are deliberately out of scope for v0.6.0;
  deferred to v1.0.0 where a backend-agnostic spec-authoring
  surface across OpenSpec / Spec Kit / Markdown can be designed
  alongside the Per-CLI Hook Providers work.

## Non-goals

- **No agent CLI is invoked as an LLM backend.** Every tool in this
  change reads deterministic data (broker state, files on disk, git
  process output). No tool pipes prompts into `claude`, `gemini`, or
  any other agent CLI and returns the LLM's response as a tool result.
  The MCP client brings its own LLM; the MCP server provides
  deterministic tools. This guardrail also applies to descendants
  (the v1.0.0 MCP write tools and any future MCP work).
- **No HTTP/SSE daemon transport in v0.6.0.** `git paw mcp start` /
  `mcp stop` / `mcp status` were considered and deferred. The stdio
  client-spawned model gives every supported MCP client a working
  path today; a long-lived HTTP daemon adds lifecycle management,
  port allocation, and a second transport surface for benefit that
  doesn't materialise until A2A (v2.0.0) reshapes the broker's HTTP
  layer. Revisit consolidation with the broker's HTTP server in v2.0.0
  alongside the A2A migration.
- **No multi-repo registry in v0.6.0.** A single MCP server that
  exposes `list_repos()` + a `repo` parameter on every tool, backed
  by a registry written by `git paw init`, was considered. Deferred
  to v1.0.0 alongside the per-CLI specialisation work — that's the
  natural home for "one config, many repos, many CLIs" UX. v0.6.0
  users with N repos add N entries to their client config.

## Capabilities

### New Capabilities
- `mcp-server`: MCP server lifecycle, `git paw mcp` subcommand,
  stdio transport, `--repo` override, tool registry, graceful
  no-session degradation.
- `mcp-read-tools`: The five read-only tool categories
  (coordination, governance, project knowledge, session state,
  git context) — each tool's input schema, output shape, and
  data-source contract.

### Modified Capabilities
<!-- None. MCP is entirely new surface. The `cli-parsing` spec gains a
     new subcommand but that's a spec-level extension, not a behavior
     change to existing requirements; handled within the mcp-server
     capability. -->

## Known limitations

- **ChatGPT Web (chatgpt.com in a browser) does not work** with
  stdio-only MCP — the browser cannot spawn local processes. ChatGPT
  Desktop on macOS works (with Developer Mode); ChatGPT Web waits
  for the v2.0.0 HTTP transport.
- **Per-repo config burden.** Each repo requires its own entry in
  the MCP client config. For users with 5+ active repos this becomes
  painful; deferred fix is the v1.0.0 repo registry.
- **Claude Desktop spawns from app-support directory.** CWD-based
  repo discovery doesn't work there; users must use `--repo <path>`.
  Workspace-aware clients (Cursor, VS Code MCP, Windsurf) work with
  bare `git paw mcp` because they spawn from the workspace root.

## Impact

- **New code**: `src/mcp/` module (server, transport, tool registry,
  tool implementations); new `cmd_mcp` entry in `src/main.rs`; new
  `Command::Mcp { repo: Option<PathBuf> }` variant in `src/cli.rs`.
- **New dependencies**: an MCP SDK (or hand-rolled JSON-RPC over
  stdio if no FOSS-compatible crate exists at planning time).
  Any new crate added to the approved-set in AGENTS.md with
  license review.
- **Existing modules touched** (read-only borrows, no behaviour change):
  - `broker::*` — query intents and conflicts (degrades when broker
    not running)
  - `specs::*` — read OpenSpec / Markdown / Spec Kit specs
  - `session::*` — read session JSON and learnings file
  - `governance` — read configured doc paths
- **Documentation** (treated as a first-class deliverable, not an
  afterthought):
  - New mdBook chapter `docs/src/user-guide/mcp.md` with full
    setup walkthroughs for Claude Desktop, ChatGPT Desktop, Cursor,
    VS Code MCP, and Windsurf. Each walkthrough includes the
    exact config-file path, JSON snippet, restart steps, and how
    to verify the server is connected.
  - Known limitations + client-specific quirks (Claude Desktop
    needing `--repo`, ChatGPT Web unsupported, per-repo entry
    pattern) called out prominently.
  - README quick-start MCP section pointing at the chapter.
  - `--help` text for `git paw mcp` with the most common config
    snippet inline.
- **Cross-references**: v1.0.0 MCP write tools will add the
  write surface on top of this read-only foundation;
  [[agent-learning-variant]] and [[qualitative-learnings]] feed
  `get_learnings()`; the v1.0.0 repo registry (Pattern 3) is the
  follow-up for multi-repo UX.

## Context

git-paw is a Rust CLI orchestrating multi-pane agent CLI sessions over
git worktrees. Its state today lives in three places: the in-process
HTTP broker (`src/broker/`), files on disk under `.git-paw/` and the
project's spec directories (`openspec/`, `.specify/`, plain Markdown),
and git itself. None of that is reachable from a chat client — a user
who wants to ask "what specs are pending?" or "what are agents about
to touch?" has to read the repo by hand. v0.6.0's MCP server closes
that gap by exposing read-only state over the standard Model Context
Protocol (MCP) so any MCP-aware client (Claude Desktop, Cursor,
ChatGPT Desktop, Windsurf, VS Code MCP extensions) can query it.

The proposal locks the v0.6.0 surface: stdio one-shot transport,
client-spawned, with five tool categories totalling ~18 read-only
tools. This design document covers HOW to build that — the module
layout, the dependency choice, the repo-resolution algorithm, the
degradation contract, and the testing approach — without re-litigating
the WHY (which lives in the proposal) or the WHAT-EACH-TOOL-DOES
(which lives in the spec files).

## Goals / Non-Goals

**Goals:**
- A working `git paw mcp` subcommand on stdio that any MCP client can
  spawn via standard config.
- A `--repo <path>` flag that overrides CWD-based repo discovery for
  clients that don't pass workspace context (Claude Desktop, ChatGPT
  Desktop).
- A degradation contract: every tool returns a well-formed empty /
  null result when its data source is unavailable, never an error
  that the LLM has to interpret.
- A clean separation between transport (stdio JSON-RPC framing), tool
  registry (dispatch + schemas), and data-layer queries (the actual
  reads against broker / specs / session / governance / git). Future
  transports (HTTP/SSE in v2.0.0) reuse the registry + data layer
  unchanged.
- Comprehensive setup docs covering each major MCP client.

**Non-Goals:**
- Long-lived daemon mode (`mcp start` / `mcp stop` / `mcp status`).
  Deferred to v2.0.0 alongside the A2A transport reshape.
- HTTP / SSE / streamable-HTTP transport.
- Repo registry (single-server, multi-repo). Deferred to v1.0.0 with
  the per-CLI specialisation work as `mcp-repo-registry`.
- Write tools. Deferred to v1.0.0 pending a backend-agnostic
  spec-authoring design across OpenSpec / Spec Kit / Markdown.
- Wrapping agent CLIs as LLM backends. Hard guardrail — no tool in
  this change or any future MCP change pipes prompts into `claude`,
  `gemini`, or similar and returns the LLM's output as a tool result.
- ChatGPT Web support. Browser cannot spawn local processes; users
  wait for v2.0.0's HTTP transport.

## Decisions

### D1. MCP SDK: official `rmcp` crate over hand-rolling JSON-RPC

**Decision:** Use the official Anthropic-published `rmcp` Rust SDK
for protocol framing, message dispatch, and tool registration. Pin
to the latest stable release at planning time; track upstream.

**Alternatives considered:**
- Hand-rolled JSON-RPC over stdio (~300 LOC of framing + dispatch).
  Rejected: protocol churn risk, more surface area to maintain, no
  benefit over the SDK once it exists.
- Generic JSON-RPC crate (e.g. `jsonrpc-core`). Rejected: doesn't
  cover MCP-specific concepts (capabilities, tool schemas,
  notifications), so we'd still need a thin MCP layer on top.

**Constraints to verify before locking:**
- License: `rmcp` must be MIT or Apache-2.0 (or dual). If it lands
  under any other license, we hand-roll. (`dirs` is the precedent
  for license-driven dependency rejection in this project.)
- Async runtime fit: must work with `tokio` (already a transitive
  dep via axum).
- Stability: if `rmcp` is still in heavy churn at implementation
  time, pin a release and document the upgrade strategy.

### D2. Module layout: `src/mcp/{server, tools, query}`

**Decision:** Three sub-modules with a strict dependency direction:

```
src/mcp/
├── mod.rs              // entry: cmd_mcp(), wires server with tools
├── server.rs           // stdio transport setup + lifecycle
├── tools/              // tool definitions (one file per category)
│   ├── mod.rs          // registers all tools with the server
│   ├── coordination.rs // get_intents, get_intent, get_conflicts
│   ├── governance.rs   // get_adrs, get_dod, get_constitution, ...
│   ├── project.rs      // get_specs, get_tasks, get_dependency_graph
│   ├── session.rs      // get_session_status, get_learnings, ...
│   └── git.rs          // get_branches, get_recent_commits, get_diff
└── query/              // data-layer reads (no MCP types here)
    ├── mod.rs
    ├── intents.rs      // wraps broker::intents::active()
    ├── specs.rs        // wraps specs::scan() etc.
    ├── session.rs      // wraps session::status()
    ├── governance.rs   // reads files at [governance] paths
    └── git.rs          // wraps std::process::Command git invocations
```

**Dependency rule:** `query` knows nothing about MCP. `tools` knows
about MCP and `query` but not about `server`. `server` only wires
things up. This makes the v2.0.0 HTTP transport additive — drop in a
new `server.rs` variant, reuse `tools` + `query` unchanged.

**Alternatives considered:**
- Flat module with one big file per tool category. Rejected: makes
  testing per-tool slower (compile time) and conflates data access
  with MCP schemas.
- Single trait `McpTool` with one impl per tool, registered in a
  factory. Considered but deferred — `rmcp` likely provides its
  own registration macro; align with that rather than reinvent.

### D3. Repo resolution: CWD walk, then `--repo` override

**Decision:** On `cmd_mcp` entry, resolve the active repo path with
this algorithm:

```
1. If `--repo <path>` was passed:
   - canonicalize the path
   - error out if it's not a git repository (no .git/ at any ancestor)
   - return that path

2. Otherwise:
   - Start at std::env::current_dir()
   - Walk parents looking for .git/ (handle worktrees: a worktree's
     .git is a file pointing at the main repo; resolve that)
   - If found, return the worktree root
   - If not found, log to stderr and exit with a clear "not a git
     repository" message — DO NOT silently serve nothing
```

Once resolved, store as `RepoContext { root: PathBuf,
git_paw_dir: Option<PathBuf>, broker_url: Option<String> }` and
pass to every tool. `git_paw_dir` resolves to `<root>/.git-paw/` if
it exists (None otherwise — pure-manual / cold-repo case).
`broker_url` is read from `<root>/.git-paw/sessions/*.json` if a
session is active, None otherwise.

**Alternatives considered:**
- Environment variables (`$GIT_PAW_REPO`). Rejected as primary: MCP
  clients don't reliably pass env. Acceptable as a future addition
  if `--repo` proves awkward.
- Repo registry lookup. Deferred to v1.0.0 (see Non-Goals).

### D4. Degradation contract: empty results, never errors

**Decision:** Every tool that depends on data the user might not
have set up returns a well-formed empty / null result rather than
erroring. The LLM gets unambiguous "there is nothing here" signal,
not a stack trace.

| Tool category | Empty shape |
|---|---|
| Coordination (broker off / no session) | `{ "intents": [], "conflicts": [] }` |
| Governance (no `[governance]` config) | `{ "adrs": [], "dod": null, ... }` |
| Project (no specs anywhere) | `{ "specs": [] }` |
| Session (no session active) | `{ "session": null, "summary": null }` |
| Git (works always — repo always exists) | always populated |

**Hard failures only for:**
- Path-resolution errors (no git repo found — see D3).
- Malformed user config (e.g. `[governance]` path points at a file
  that exists but isn't readable). Returns an MCP-protocol-level
  error so the LLM can surface "your governance.dod path is
  unreadable, fix `.git-paw/config.toml`".

**Alternatives considered:**
- Throw structured errors for missing data, let the LLM interpret.
  Rejected: LLM behaviour on errors is non-deterministic; returning
  empty arrays is unambiguous.
- Add a `data_available: bool` field to every response. Rejected:
  empty arrays already carry that signal; extra field adds noise.

### D5. Logging: stderr only, never stdout

**Decision:** stdio MCP servers MUST keep stdout reserved for
JSON-RPC frames. All logging goes to stderr. Use the same `tracing`
infrastructure git-paw already uses, configured at server startup
with a stderr writer.

Configurable via `RUST_LOG` env var (standard `tracing-subscriber`
convention). Default level: `warn` (quiet by default — MCP servers
that spam logs annoy clients).

**Alternatives considered:**
- Log to a file under `.git-paw/logs/`. Considered for debugging;
  added as an optional fallback (`--log-file <path>` flag) but not
  the default.

### D6. Testing strategy: protocol-level integration tests via in-process client

**Decision:** Three layers:

1. **Unit tests per query function** — `src/mcp/query/*` tests assert
   correct read behaviour against fixture repos with `tempfile`.
2. **Tool integration tests** — `src/mcp/tools/*` tests dispatch a
   simulated MCP request through the tool, verify the response JSON
   matches the schema. No real transport.
3. **End-to-end protocol tests** — `tests/mcp_e2e.rs` spawns
   `git paw mcp` as a subprocess, writes JSON-RPC frames to its
   stdin, reads responses from stdout, validates lifecycle
   (initialize → list_tools → call_tool → shutdown).

The E2E layer covers (a) stdio framing correctness, (b) cold-start
(no session) degradation, (c) `--repo` flag behaviour. Tmux is
NOT required for MCP tests since the server doesn't touch tmux.

### D7. Subcommand surface: `git paw mcp` with `--repo` only

**Decision:** v0.6.0 ships exactly one subcommand:

```
git paw mcp [--repo <path>] [--log-file <path>]
```

No `start` / `stop` / `status` (no daemon — see Non-Goals).
No `--port` (no HTTP transport).
No `--config` (uses `<repo>/.git-paw/config.toml` like the rest
of git-paw).

`--help` text includes a copy-pasteable Claude Desktop config
snippet to lower the activation cost.

## Risks / Trade-offs

- **MCP SDK churn** → pin to a release at implementation start,
  document the version in `Cargo.toml` comments, and add an
  upgrade-cadence note to AGENTS.md so future maintainers know
  this dep needs active tracking.
- **Tool surface is wide (~18 tools)** → keeping each tool's
  schema in a per-category file (D2) limits blast radius when
  one tool's shape changes; the spec files (per capability)
  define the contract independent of the implementation.
- **License risk on `rmcp`** → verify before merging the design
  phase; if it's non-FOSS, switch to hand-rolled JSON-RPC and
  budget an extra week (~300 LOC + tests).
- **Repo-resolution edge cases** (worktrees, submodules, bare
  repos) → D3's algorithm handles worktrees; submodules
  intentionally resolve to the outer repo (the inner-submodule
  case is too rare to design for in v0.6.0); bare repos are
  rejected with a clear error.
- **stdout pollution killing the protocol** → linted via a
  unit test that asserts no `println!` exists in `src/mcp/`
  (only `eprintln!` allowed); CI enforced.
- **Per-repo config burden across many repos** → known
  limitation, deferred fix is the v1.0.0 repo registry. Document
  prominently in the mdBook chapter so users don't think it's
  a bug.
- **ChatGPT Web users have no path** → known limitation,
  deferred to v2.0.0 HTTP transport. Document prominently.

## Migration Plan

This is net-new functionality with no v0.5.0 surface to migrate
from. Rollout:

1. Implementation lands behind no feature flag — the subcommand
   simply appears.
2. mdBook chapter + README MCP section publish with the release.
3. v0.6.0 release notes call out the new `git paw mcp` subcommand
   with copy-pasteable Claude Desktop + ChatGPT Desktop + Cursor
   configs.
4. No rollback needed; if the subcommand is broken, users simply
   don't use it. The rest of git-paw is unaffected.

**Forward-compatibility note** for the v0.6.0-to-v2.0.0
transition: the tool registry (D2) is the stable surface.
v2.0.0 may add a long-lived HTTP transport, but the tool names,
input schemas, and output shapes carry over. Document each
tool's contract in `specs/mcp-read-tools/spec.md` precisely
enough that the v2.0.0 HTTP transport is a transport-only swap.

## Open Questions

- **`rmcp` license + maturity check.** Resolve before specs
  artifact lands (or commit to hand-rolled fallback).
- **Single binary vs separate `git-paw-mcp` binary?** Some MCP
  clients prefer a top-level binary in their config. Pro of
  separate binary: shorter client config (`"command": "git-paw-mcp"`
  vs `"command": "git", "args": ["paw", "mcp", ...]`). Con: extra
  cargo target, extra cargo-dist artifact. Lean: stay with
  `git paw mcp` subcommand; document the activation pattern.
  Revisit if dogfood shows pain.
- **Tool naming for the read-only / write-tool split.** When
  the v1.0.0 MCP write tools land, do we prefix write tools (`write_*`
  / `create_*`) or namespace them by category? Decide in that
  change's design phase.
- **Schema versioning.** MCP supports tool capability discovery;
  do we need a `git_paw_version` field in each response for the
  client to detect older servers? Lean: no for v0.6.0 (MCP
  protocol-level capabilities cover this); add if v2.0.0 introduces
  schema-incompatible changes.

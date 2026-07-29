# Specifications

git-paw uses [OpenSpec](https://github.com/openspec) for formal, testable
specifications: each capability has a dedicated `spec.md` under
[`openspec/specs/`](https://github.com/bearicorn/git-paw/tree/main/openspec/specs)
using RFC 2119 keywords (`SHALL`, `MUST`, `SHOULD`) and GIVEN/WHEN/THEN
scenarios, and every scenario maps to at least one test.

The eight **foundational capabilities** (the original v0.1–v0.2 surface) are
reproduced in full below. The project has since grown to many more capabilities
(broker, supervisor, dashboard, MCP, learnings, conflict detection, unattended
operation, agent-friendly docs, …); the **complete index** at the end of this
page links to every capability spec in the repository.

## Foundational capabilities

| Capability | Description |
|------------|-------------|
| [CLI Parsing](#cli-parsing) | Command-line argument parsing and subcommands |
| [CLI Detection](#cli-detection) | Auto-detect AI CLIs on PATH, load custom CLIs |
| [Git Operations](#git-operations) | Validate repos, list branches, manage worktrees |
| [Tmux Orchestration](#tmux-orchestration) | Create sessions, manage panes, apply layout |
| [Session State](#session-state) | Persist and recover session state |
| [Configuration](#configuration) | Parse and merge TOML config files |
| [Interactive Selection](#interactive-selection) | User prompts for mode, branch, and CLI selection |
| [Error Handling](#error-handling) | Unified error types with exit codes |

---

## CLI Parsing

{{#include ../../../openspec/specs/cli-parsing/spec.md}}

---

## CLI Detection

{{#include ../../../openspec/specs/cli-resolution/spec.md}}

---

## Git Operations

{{#include ../../../openspec/specs/git-operations/spec.md}}

---

## Tmux Orchestration

{{#include ../../../openspec/specs/tmux-orchestration/spec.md}}

---

## Session State

{{#include ../../../openspec/specs/session-state/spec.md}}

---

## Configuration

{{#include ../../../openspec/specs/core-configuration/spec.md}}

---

## Interactive Selection

{{#include ../../../openspec/specs/cli-interactive-selection/spec.md}}

---

## Error Handling

{{#include ../../../openspec/specs/core-error-handling/spec.md}}

---

## Complete capability index

Every capability spec in the repository, grouped by its `<namespace>-` prefix
(links go to the canonical `openspec/specs/` source on GitHub). Each entry's
blurb is condensed from that spec's `## Purpose`.

A few entries are **internal process specs** — they govern the test suite,
CI, or the verification workflow itself (e.g. `core-ci-hygiene`,
`quality-selftest`) rather than a user-facing feature. These intentionally
have no user-guide chapter: the spec itself is their documentation.

### core-

- [`core-configuration`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/core-configuration/spec.md) — parses and merges TOML config from global and per-repo files, with repo config overriding global.
- [`core-init`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/core-init/spec.md) — bootstraps a repo for git-paw (`.git-paw/` scaffold, default config, `.gitignore` update), idempotently.
- [`core-error-handling`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/core-error-handling/spec.md) — the central `PawError` type: every variant carries an actionable message and maps to a process exit code.
- [`core-lang-agnostic`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/core-lang-agnostic/spec.md) — keeps bundled skills project-agnostic via config-sourced placeholder substitutions and a CI no-language-leak audit.
- [`core-ci-hygiene`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/core-ci-hygiene/spec.md) — isolates the test suite from the live environment and closes the local-vs-CI gap via cold-start smoke recipes and convention-enforcement tests.
- [`core-memory-isolation`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/core-memory-isolation/spec.md) — keeps coding agents from writing outside their worktree into the operator's config and memory territory via a config-driven protected-path set.
- [`core-project-conventions`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/core-project-conventions/spec.md) — repo-wide contributor conventions and their doc accuracy: the AGENTS.md dependency table in sync with `Cargo.toml`, the commit-scope list, the mdBook architecture module list and changelog include, and the README's enumeration of the current user-facing surface.
- [`core-governance-config`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/core-governance-config/spec.md) — `PawConfig.governance`: optional root-relative pointers to a project's governance/doc artifacts, paths-only with no gating semantics.
- [`core-opsx-role-gating`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/core-opsx-role-gating/spec.md) — enforces the supervisor-only boundary on `/opsx:verify` and `/opsx:archive`, with a post-commit guard detecting archive activity by coding agents.
- [`core-selftest`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/core-selftest/spec.md) — `git paw selftest` runs an isolated end-to-end session lifecycle against a throwaway repo with a dummy CLI and reports a single pass/fail verdict.

### cli-

- [`cli-parsing`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/cli-parsing/spec.md) — the clap v4 CLI: all subcommands, flags, and argument validation, defaulting to `start` when no subcommand is given.
- [`cli-resolution`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/cli-resolution/spec.md) — detects AI coding CLIs on PATH, merges user-defined custom CLIs, and resolves which CLI each branch uses via a priority chain.
- [`cli-interactive-selection`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/cli-interactive-selection/spec.md) — interactive prompts for choosing branches and CLIs (uniform or per-branch), with logic separated from UI via the `Prompter` trait.

### git-

- [`git-operations`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/git-operations/spec.md) — validates repos, lists branches, creates/removes worktrees, and derives worktree directory names — the git plumbing under parallel sessions.
- [`git-hook-injection`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/git-hook-injection/spec.md) — installs a post-commit dispatcher, pre-push block, and per-worktree branch-guard hooks: committing publishes `agent.artifact`, pushes are blocked, and pre-existing user content is preserved.
- [`git-add-remove-branch`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/git-add-remove-branch/spec.md) — `git paw add`/`remove` attach or detach a single agent worktree+pane on a live session, re-tiling the grid without a restart.

### tmux-

- [`tmux-orchestration`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/tmux-orchestration/spec.md) — orchestrates tmux sessions with per-pane CLIs in worktrees via a testable builder with dry-run and automatic tiled layout.

### session-

- [`session-state`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/session-state/spec.md) — persists one JSON file per session for crash recovery, with atomic writes, tmux-liveness stale detection, and per-repo session receipts.
- [`session-logging`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/session-logging/spec.md) — captures raw per-pane terminal output via tmux pipe-pane and replays it (ANSI strip/preserve, fuzzy branch match, most-recent auto-select).

### boot-

- [`boot-block`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/boot-block/spec.md) — the standardized boot-instruction block injected into each agent: its format/content, `{{VARIABLE}}` substitution, and the shared `build_boot_block` renderer.
- [`boot-agents-md`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/boot-agents-md/spec.md) — injects a marker-delimited git-paw section into AGENTS.md and writes per-worktree AGENTS.md combining root content with worktree assignment sections.

### broker-

- [`broker-protocol`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/broker-protocol/spec.md) — the complete wire protocol: the `BrokerMessage` tagged enum, `agent_id` slugification, publish validation, and the delivery layer's per-variant routing, sequencing, and cursor polling.
- [`broker-runtime`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/broker-runtime/spec.md) — the in-process coordination server: `[broker]` config, the tokio/axum lifecycle, HTTP surface (`/publish`, `/messages`, `/status`, `/log`, `/watch`), and session-lifecycle integration.
- [`broker-watcher-and-state`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/broker-watcher-and-state/spec.md) — keeps the `/status` roster honest as worktrees change: filesystem watcher, roster hygiene, working/committed republishing, terminal-status stickiness, and the introspection surface.
- [`broker-agent-helper`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/broker-agent-helper/spec.md) — bundled shell helpers (`broker.sh` for agents, `sweep.sh` for the supervisor) that wrap every broker interaction and detect stuck-agent shapes.
- [`broker-conflict-detection`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/broker-conflict-detection/spec.md) — a broker-internal, supervisor-mode detector that flags forward/in-flight/ownership conflicts across agent intents and modified-file sets (with sub-file regions).
- [`broker-dashboard`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/broker-dashboard/spec.md) — a ratatui TUI observing broker state: an agent-status table with a pinned supervisor row plus a scrolling, filterable broker-log panel.

### supervisor-

- [`supervisor-launch`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-launch/spec.md) — orchestrates the full supervisor session launch via `cmd_supervisor()`: layout, worktrees, pane structure, boot prompts, and the attach vs `--unattended` drive-loop branch.
- [`supervisor-config`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-config/spec.md) — defines the `[supervisor]` config schema: approval level, nested sub-tables, gate-command templates, and `[supervisor.auto_approve]`.
- [`supervisor-skill-discipline`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-skill-discipline/spec.md) — encodes the disciplines the bundled supervisor skill teaches: sweep-driven pane work, isolated verification worktrees, stream-timeout recovery, per-event verification, and the no-fail-fast testing gate.
- [`supervisor-unattended-operation`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-unattended-operation/spec.md) — the in-process drive loop under `--unattended` that keeps a wave moving with no human: polling, auto-approving safe prompts, detecting completion, and exiting with a summary.
- [`supervisor-learnings`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-learnings/spec.md) — an opt-in, broker-internal aggregator that derives deterministic and qualitative learning signals into `.git-paw/session-learnings.md` (and the `agent.learning` broker variant), performing no telemetry.

### approval-

- [`approval-auto`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/approval-auto/spec.md) — clears an unattended agent's safe permission prompts through three layers: keystroke approval gate, broker-mediated send-gate re-confirm, and worktree-confined file-edit auto-approval.
- [`approval-pattern-surfacing`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/approval-pattern-surfacing/spec.md) — logs every manually-decided prompt to per-session JSONL and provides `git paw approvals` to aggregate recurring patterns with a promotion-target hint.
- [`approval-command-safety`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/approval-command-safety/spec.md) — detects CLI permission prompts via rate-limited pane capture, classifies them into permission types, and marks a command safe or escalate against a configurable whitelist plus a terminal danger-list.

### spec-

- [`spec-providers`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/spec-providers/spec.md) — discovers pending specs and represents them as launchable `SpecEntry` values through a pluggable backend system (OpenSpec, Markdown, Spec Kit, superpowers plans).

### mcp-

- [`mcp-server`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/mcp-server/spec.md) — `git paw mcp` runs a stdio JSON-RPC MCP server exposing read-only, deterministically-sourced tools over a resolved repo/worktree root.
- [`mcp-agent-docs`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/mcp-agent-docs/spec.md) — makes the docs site machine-consumable (`llms.txt`, sitemap, robots, per-page metadata) and bundles a path-allowlisted `docs-fetch` helper and skill.

### skill-

- [`skill-agent`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/skill-agent/spec.md) — defines the embedded coordination and supervisor skill templates git-paw injects, their resolution order (user override then embedded default), and the placeholder-rendering contract.
- [`skill-standardized`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/skill-standardized/spec.md) — supports the agentskills.io standardized skill format (a `SKILL.md` directory): auto-detecting, loading, and schema-validating it with actionable errors.

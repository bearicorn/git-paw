<div align="center">

<img src="https://raw.githubusercontent.com/bearicorn/git-paw/main/.github/assets/logo.jpg" alt="git-paw logo" width="152">

# git-paw

**Parallel AI Worktrees** — orchestrate multiple AI coding CLI sessions across git worktrees from a single terminal using tmux.

[![CI](https://github.com/bearicorn/git-paw/actions/workflows/ci.yml/badge.svg)](https://github.com/bearicorn/git-paw/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/git-paw.svg)](https://crates.io/crates/git-paw)
[![Downloads](https://img.shields.io/crates/d/git-paw.svg)](https://crates.io/crates/git-paw)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![MSRV: 1.96](https://img.shields.io/badge/MSRV-1.96-brightgreen.svg)](Cargo.toml)

</div>

## Demo

```
$ git paw

  Select branches (type to filter, ctrl-u to clear, space to toggle, enter to confirm)
  filter:
  > [x] feat/api
    [x] feat/auth
    [ ] fix/typo
    [ ] main

  CLI assignment mode:
  > Same CLI for all branches
    Different CLI per branch

  Select AI CLI for all branches:
    Antigravity (agy)
  > Claude (claude)
    Codex (codex)

  # git-paw creates a worktree per branch, opens the tmux session
  # paw-myproject with one pane per branch running claude, and attaches you
```

## What It Does

git-paw lets you run multiple AI coding assistants in parallel, each in its own git worktree and tmux pane. Pick your branches, pick your AI CLI(s), and git-paw handles the rest — creating worktrees, launching a tmux session, and wiring everything together. Stop and resume sessions at will; your worktrees and uncommitted work are preserved.

## Features

- **Contained worktree layout (v0.8.0)** — new repos place agent worktrees *inside* the project at `.git-paw/worktrees/<branch-slug>/` instead of scattering them as siblings of the repo, enabling a single project-scoped permission grant for every agent. Configurable via `worktree_placement` (`"child"` | `"sibling"`); existing repos default to the v0.7.0 sibling layout and stay there until they opt in. See [Worktree Placement](docs/src/user-guide/worktree-placement.md)
- **Parallel AI sessions** — run Claude, Codex, Antigravity, or any AI CLI across multiple branches simultaneously
- **Git worktree isolation** — each branch gets its own working directory, no stashing or switching needed
- **Runtime isolation** — opt into `[worktree.env]` / `[worktree.ports]` and each worktree gets its own copy of the env files you name plus its own block of ports in a generated `.env.local`, so several agents can actually *run* their dev servers side by side instead of colliding on one `.env` and one port. Add `[worktree.hooks]` and your own `on_create` / `on_remove` commands run per worktree — a script that prints `DATABASE_URL=...` gives every agent its own database branch, and its teardown drops it again. Absent config changes nothing. See [Runtime Isolation](docs/src/user-guide/runtime-isolation.md)
- **Smart session management** — reattach to active sessions, auto-recover after crashes or reboots, and rebase existing agent branches onto the repository's default branch on every `git paw start` so agents never drift behind supervisor commits on `main` (pass `--no-rebase` to opt out)
- **Interactive or scripted** — fuzzy branch picker and CLI selector, or pass `--cli` and `--branches` flags
- **Per-branch CLI assignment** — use Claude on one branch and Codex on another in the same session
- **Presets** — save branch + CLI combos in config for one-command launches
- **Custom CLI support** — register any AI CLI with `git paw add-cli`
- **Session persistence** — state saved to disk, survives tmux crashes and system reboots
- **Dry run** — preview the session plan before executing with `--dry-run`
- **Mouse-friendly tmux** — click to switch panes, drag borders to resize, scroll with mouse wheel
- **Spec-driven launch** — auto-discover specs and launch sessions with `--from-all-specs` (or narrow to a subset via `--specs NAME[,NAME...]`)
- **AGENTS.md integration** — auto-inject session context into worktree AGENTS.md files
- **Session logging** — capture raw terminal output per pane for later review
- **Replay** — view session logs with ANSI stripping or colored output via `less -R`
- **Project init** — `git paw init` bootstraps `.git-paw/`, config, and gitignore
- **Standards-based** — uses `AGENTS.md` following the Linux Foundation standard for AI agent instructions
- **Agent coordination** — built-in HTTP broker lets agents share status, artifacts, and blocked requests
- **Dashboard TUI** — live status table in pane 0 shows agent progress at a glance
- **Broker messages panel** — optional dashboard section showing real-time agent communication (configurable via `[dashboard] show_message_log = true`)
- **Skill templates** — coordination instructions auto-injected into each agent's AGENTS.md
- **Boot-prompt injection** — standardized boot instructions automatically prepended to all agent prompts, ensuring reliable self-reporting (register, done, blocked, question operations) — always enabled for broker sessions
- **Cursor-based messaging** — lossless message polling with sequence tracking
- **Spec Kit backend** — first-class support for [GitHub Spec Kit](https://github.com/github/spec-kit) projects via `[specs] type = "speckit"` with `dir = ".specify/specs"` (what `git paw init` writes when you pick Spec Kit); the `[P]`/non-`[P]` task split decomposes into per-task and consolidated worktrees
- **`--specs-format` override** — force-select the spec backend (`openspec`, `markdown`, `speckit`, `superpowers`) on the command line, overriding `[specs] type` in config (there is no filesystem auto-detection)
- **`--no-supervisor`** — single-session override of `[supervisor] enabled = true` for plain (non-supervisor) operation without editing config
- **`start --unattended`** — run a supervisor wave to completion with no human in the seat: engages supervisor mode and drives an in-process loop that escalates every live permission prompt for a human decision without blocking the wave, detects completion, and exits with a summary (designed for detached operation; mutually exclusive with `--no-supervisor`). It approves nothing — a genuinely prompt-free run needs your agent CLI's own allow rules, or `full-auto` inside the documented sandbox. See [Supervisor → Unattended mode](docs/src/user-guide/supervisor.md#unattended-mode---unattended)
- **Orchestrator outside the session** — a supervised session holds only the dashboard (pane 0, full-width) and the coding agents (pane 1 onwards); the orchestrator is your own agent session, run at the repository root outside tmux and pointed at the context the launch writes to `.git-paw/AGENTS.local.md`. It drives the run through the broker, the bundled helpers, or the MCP server, so no LLM process runs at the repo root inside the session. The unattended loop records every judgment call — a live permission prompt, a worker's `agent.question`, a `committed`/`done` artifact's merge decision, a non-converging branch — as a broker review item and types into coding-agent panes only. A permission prompt is **relayed to you**, not decided: the orchestrator runs `sweep.sh approve` only when you tell it to, and the helper refuses any pane its discovery roster does not list as a coding agent. See [the loop is a pump, the supervisor is the brain](docs/src/user-guide/supervisor.md#the-loop-is-a-pump-the-supervisor-is-the-brain)
- **Headless orchestrator hand-off** — under `--unattended`, a judgment call the loop cannot make (an ambiguous question, a merge decision, a correction-exhausted branch) is also handed to your orchestrator CLI as one non-interactive, one-shot turn at the repository root (`claude -p`, `codex exec`, or any CLI's `[clis.<name>].headless_args`), with the context on stdin and its approval level from `[supervisor].approval`. One runs at a time, deduplicated, killed after `[supervisor.escalation] timeout_seconds`, logged, and reported on the broker and in the exit summary; permission prompts are never handed off and stay with you. Disable with `[supervisor.escalation] enabled = false`. See [Headless hand-off](docs/src/user-guide/supervisor.md#headless-hand-off-of-judgment-calls)
- **`start --force`** — bypass the uncommitted-spec validation warning when launching with `--from-all-specs` or `--specs`
- **Forward coordination** — agents publish `agent.intent` before they begin editing so peers (and the broker conflict detector) see the planned file set ahead of the first commit
- **Automatic conflict detection** — the broker auto-emits `[conflict-detector]`-tagged `agent.feedback` for forward (overlapping intents), in-flight (overlapping `modified_files`), and ownership-violation conflicts; an unresolved in-flight overlap is classified from the agents' declared regions — a true collision escalates to the supervisor inbox via `agent.question`, while an additive overlap (disjoint declared regions) is downgraded to an informational `agent.feedback` and never blocks on human input
- **Learnings mode** — opt-in `[supervisor] learnings = true` collects deterministic friction signals (stuck duration, recovery cycles, forward conflicts, in-flight conflicts, ownership violations) into `.git-paw/session-learnings.md` for post-session review. **No telemetry** — the file is purely local and nothing is sent anywhere; you can optionally share it via a [GitHub issue](https://github.com/bearicorn/git-paw/issues) (after reviewing/anonymising it) to help improve the tool. See [Learnings → Privacy & Sharing](docs/src/user-guide/learnings.md#privacy--sharing)
- **Governance pointers** — point the supervisor at your existing ADRs, test strategy, security checklist, DoD, and constitution via the `[governance]` config table; Spec Kit projects auto-wire `.specify/memory/constitution.md` when present
- **Conflict-detector tuning** — `[supervisor.conflict]` exposes the in-flight escalation window (`window_seconds`), the intent-overlap warning toggle (`warn_on_intent_overlap`), and the ownership-violation escalation toggle (`escalate_on_violation`)
- **Learnings flush cadence** — `[supervisor.learnings_config] flush_interval_seconds` (default 60) controls how often learnings entries are flushed from memory to `.git-paw/session-learnings.md`
- **Routing through the supervisor** — type `/agents` to the orchestrator to see the live agent inventory (status, mode, pane) and `/tell <agent> <prompt>` to route a prompt to one agent without tab-switching into its pane; `[supervisor.tell] mode` picks the delivery channel (`feedback` queue by default, `send-keys` for accept-edits agents) and every route is recorded in the session learnings
- **MCP server (v0.7.0+)** — `git paw mcp` exposes this repo's read-only state (coordination intents/conflicts, governance docs, specs/tasks, session status/learnings, agent skills, git context) over the [Model Context Protocol](https://modelcontextprotocol.io) so any MCP-aware client can query it, plus a bounded write surface — agent publish tools, and for a server in the main working tree the session lifecycle (start/pause/stop/purge) and verdict tools, plus spec authoring (`create_spec`/`write_spec_artifact`) once the repository opts in with `[mcp] allow_repo_writes`; runs standalone over stdio, degrades gracefully when no session is active, and never invokes an agent CLI as a backend

> **Tip:** git-paw uses `AGENTS.md` as the standard agent instruction file. Point your CLI at it without duplicating content:
> - **Claude Code** reads only `CLAUDE.md` and supports imports, so create a `CLAUDE.md` whose first line imports `AGENTS.md`, then add any machine-local notes below it:
>   ```markdown
>   @AGENTS.md
>
>   # personal, machine-local notes (optional)
>   ```
>   The import stays in sync with `AGENTS.md` and survives tools that rewrite `CLAUDE.md` (e.g. `rtk init`) — a symlink gets clobbered by those and can't carry CLI-specific additions.
> - **Other CLIs** that read a fixed file and have no import syntax (e.g. Gemini → `GEMINI.md`) can symlink instead: `ln -s AGENTS.md GEMINI.md`.
>
> Add the local `CLAUDE.md` / symlinks to `.gitignore` so they stay per-developer.

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| macOS (ARM) | Supported | Primary development platform |
| macOS (x86) | Supported | |
| Linux (x86_64) | Supported | |
| Linux (ARM64) | Supported | |
| Windows | WSL only | tmux is a Unix tool — use Windows Subsystem for Linux |

> **Why no native Windows?** git-paw relies on tmux for terminal multiplexing, which is not available natively on Windows. WSL provides a full Linux environment where git-paw works perfectly.

## Quick Start: Same CLI

Use the same AI CLI across all branches:

```bash
# Interactive — pick branches and CLI from prompts
git paw

# Non-interactive — specify everything upfront
git paw start --cli claude --branches feat/auth,feat/api
```

This creates:
- A worktree for each branch — `.git-paw/worktrees/feat-auth/`, `.git-paw/worktrees/feat-api/` in a repo set up with `git paw init`, or `../yourproject-feat-auth/`, `../yourproject-feat-api/` when no `worktree_placement` is configured (see [Worktree Placement](docs/src/user-guide/worktree-placement.md))
- A tmux session with one pane per branch, each running `claude`

Every launch path — including this bare one, not just supervisor mode —
builds its pane command from the resolved approval flags: with no
`[supervisor]` config at all, `agent_approval` defaults to `auto`, so Claude
panes launch with `--permission-mode acceptEdits` automatically. See
[Approval flag resolution](docs/src/configuration/README.md#supervisor) to
change the level or set per-CLI overrides.

## Quick Start: Per-Branch CLI

Use different AI CLIs on different branches:

```bash
# Interactive mode — select "Different CLI per branch"
git paw
# → Pick branches: feat/auth, feat/api
# → Pick CLI for feat/auth: claude
# → Pick CLI for feat/api: codex
```

Result: a tmux session where `feat/auth` runs Claude and `feat/api` runs Codex, side by side.

## Quick Start: Orchestrator Mode

Launch the worker agents under a dashboard and orchestrate them from your own agent session:

```bash
git paw start --supervisor

# Skip supervisor for a single run even when [supervisor] enabled = true is set
git paw start --no-supervisor

# Bypass the uncommitted-spec validation warning when launching from specs
# (non-supervisor only — see the `--force` note below)
git paw start --from-all-specs --force

# Drive the whole wave to completion with no human in the seat (detached)
git paw start --unattended --from-all-specs
git paw start --unattended --branches feat/auth,feat/api
```

The session holds the dashboard at pane 0 (a full-width top row) and one coding agent per pane from pane 1; no orchestrator runs inside it. `git paw init` exports the orchestrator skill, rendered with your gate commands, to `.git-paw/skills/orchestrator/SKILL.md` and adds a pointer section to your root `AGENTS.md`; each launch refreshes the skill, writes the session context (session name, broker URL, governance documents) to `.git-paw/AGENTS.local.md`, and prints both paths with the attach hint. Load the skill in your own agent session, at the repository root and outside tmux: the orchestrator follows each worker's progress and artifacts through the broker, runs the configured test command between merges, and publishes a final summary status when work completes. Use this mode when you want to leave a multi-branch session running without continually steering each agent yourself. See [Quick Start: Orchestrator Mode](docs/src/quick-start-orchestrator.md) for the full walkthrough and [Remote control](docs/src/user-guide/remote-control.md) for driving it from a phone.

By default `git paw start --supervisor` builds the session and returns. Add **`--unattended`** to fold the *driving* half of the operator loop into the tool: it engages supervisor mode and drives an in-process loop that sweeps every coding-agent pane on a ~15-second cadence, escalates every live permission prompt for a human decision without blocking the wave, and exits with a summary on completion or a heartbeat after ~25 minutes without progress. It needs no attached terminal, and once the loop has exited, re-running the same command binds a new loop to the still-live session — reviving or recovering it first if needed — rather than forking a parallel one (only one drive loop may drive a session at a time).

**`--unattended` approves nothing.** Deciding whether a command may run is the agent CLI's job, not git-paw's. For a run that genuinely does not stop for prompts, pre-approve the commands in your agent CLI's own allow rules, or launch the agents with `[supervisor] agent_approval = "full-auto"` inside the documented [sandbox](docs/src/user-guide/sandbox.md). Otherwise expect the loop to escalate — and to keep the rest of the wave moving while it does. See [Supervisor → Unattended mode](docs/src/user-guide/supervisor.md#unattended-mode---unattended).

The loop is the pump; the orchestrator is the brain. Each judgment call the loop cannot make mechanically — a live permission prompt, a worker's `agent.question`, the merge decision a `committed`/`done` artifact opens, a branch that will not converge — is a **broker review item** the orchestrator reads; nothing is typed into any pane for it, and the loop never blocks waiting for a reply. A permission prompt is the one call the orchestrator must not decide itself: it relays the prompt to you, and runs `sweep.sh approve <pane>` only on your explicit instruction. Nudges aimed at a *worker* pane stop once that worker has finished (`done`/`verified`), judged from its broker status rather than a pane diff — a finished pane and a stuck pane look identical. See [the loop is a pump, the supervisor is the brain](docs/src/user-guide/supervisor.md#the-loop-is-a-pump-the-supervisor-is-the-brain).

For the end-to-end recipe — planning as a pre-step, writing task-level-concrete specs, and defining what "verified" means in your `AGENTS.md` so a fleet of cheaper workers succeeds under one smart orchestrator — see the [Tiered-Model Workflow](docs/src/user-guide/tiered-model-workflow.md) guide.

To stop routine dev-loop commands (`git commit`, `cargo test`, your build) prompting at all, add them to **your agent CLI's own allow rules** — that is the only thing that decides. `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` are deprecated: they still parse so old configs load, but they have no effect and print one warning per config file.

`--no-supervisor` is the highest-precedence step in the supervisor-mode resolution chain — it wins over both `[supervisor] enabled = true` in config and any interactive prompt. It is mutually exclusive with `--supervisor`; passing both fails at parse time. `--force` only matters for a spec-mode launch **without** supervisor mode (`--from-all-specs` / `--specs`, no `--supervisor`) and bypasses the warning when uncommitted spec changes are detected on disk; the check itself does not run under supervisor mode, so `--force` has no effect there.

## Installation

### From crates.io

```bash
cargo install git-paw
```

### Homebrew

```bash
brew install bearicorn/tap/git-paw
```

### Shell installer

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/bearicorn/git-paw/releases/latest/download/git-paw-installer.sh | sh
```

### Windows (WSL)

Install [WSL](https://learn.microsoft.com/en-us/windows/wsl/install), then use any of the Linux installation methods above inside your WSL environment:

```bash
# Inside WSL
sudo apt install tmux
cargo install git-paw
```

### Prerequisites

- [tmux](https://github.com/tmux/tmux) — terminal multiplexer
- [Git](https://git-scm.com/) — with worktree support (2.5+)

### `git paw` vs `git-paw`

Once installed to your PATH, git-paw works as a git subcommand:

```bash
git paw start    # git finds `git-paw` on PATH automatically
```

You can also call the binary directly — useful during development or if it's not on PATH:

```bash
git-paw start    # equivalent
cargo run -- start  # during development
```

All examples below use `git paw`, but `git-paw` works identically.

## Usage

`git paw <command>` (equivalently `git-paw <command>`). One-line summary below — the **full reference, with every flag and example, lives in the [CLI Reference](https://bearicorn.github.io/git-paw/cli-reference.html)**.

| Command | What it does |
|---------|--------------|
| `init` | Scaffold `.git-paw/` (default config + `.gitignore`). |
| `start` | Launch or reattach a session — interactive, `--cli`/`--branches`, or spec-driven (`--from-all-specs`, `--specs`, `--preset`, `--dry-run`, `--unattended`). Auto-recovers a stopped session. |
| `add` | Attach a branch + agent pane mid-session, on any live session (v0.6.0+). |
| `remove` | Detach one agent — pane + worktree — with `--force` / `--keep-worktree` (v0.6.0+). |
| `pause` | Soft-stop: detach and stop the broker, keep CLI panes for instant resume (v0.5.0+). |
| `stop` | Kill the tmux session and CLIs; keep worktrees + state. |
| `purge` | Remove the session, all worktrees, and state (`--force` to skip the prompt, `--stale` to sweep only dead receipts). Deletes only the branches git-paw created that are fully merged — a branch with unmerged commits is kept and printed with its tip SHA. |
| `status` | Show session name, branches, CLIs, and state. |
| `attach` | Reattach the terminal to the current repo's running session (v0.16.0+). |
| `list-clis` / `add-cli` / `remove-cli` | Manage auto-detected and custom AI CLIs. |
| `setup-cli` | Pre-approve git-paw's helper-script calls in one AI CLI's own settings. Consent-only: you run it, it prints its plan, and `--undo` takes back exactly what it added. |
| `replay` | View session logs (`--list`, `--color`, `--session`). |
| `approvals` | Report the command patterns you approved by hand during a session, with a hint on where to promote each (`--session`, `--limit`, `--json`). |
| `mcp` | [MCP](https://modelcontextprotocol.io) server over stdio — read-only repository state plus bounded publish and root-session lifecycle tools; no session or broker required (v0.7.0+). |
| `doctor` | Read-only preflight diagnostics — grouped ✓/⚠/✗ checks with a remedy on each finding, `--json`, `--live` (v0.13.0+). |
| `completions` | Print a bash/zsh/fish completion script to stdout (v0.16.0+). |

Full flags, examples, and per-client MCP setup: **[CLI Reference](https://bearicorn.github.io/git-paw/cli-reference.html)**.

## Configuration

git-paw reads a per-repo `.git-paw/config.toml` and a global `~/.config/git-paw/config.toml` (per-repo overrides global for overlapping fields). The common fields:

```toml
default_cli = "claude"        # pre-select in the interactive picker
default_spec_cli = "claude"   # skip the picker for spec-mode launches
branch_prefix = "spec/"       # prefix for spec-derived branches

[specs]
type = "openspec"             # "openspec" | "markdown" | "speckit" | "superpowers"

[presets.backend]
branches = ["feat/api", "fix/db"]
cli = "claude"
```

Custom CLIs, dashboard, logging, broker, supervisor, and worktree placement each have their own sections — see the **[Configuration reference](https://bearicorn.github.io/git-paw/configuration/index.html)** for the full schema and defaults.

## Supported AI CLIs

git-paw auto-detects the following AI coding CLIs when they are on `PATH`. The list reflects `src/detect.rs::KNOWN_CLIS` at the time of this release; the table grows as binaries land in upstream releases.

| CLI | Binary |
|-----|--------|
| Claude Code | `claude` |
| OpenAI Codex | `codex` |
| Antigravity CLI | `agy` |
| Aider | `aider` |
| Vibe | `vibe` |
| Qwen | `qwen` |
| Amp | `amp` |
| opencode | `opencode` |
| Cline | `cline` |
| Droid | `droid` |
| Pi | `pi` |
| Junie | `junie` |
| Cursor Agent | `cursor-agent` |
| GitHub Copilot CLI | `copilot` |
| cn | `cn` |
| Kilo Code | `kilo` |
| Kimi | `kimi` |

Don't see your CLI? Register it:

```bash
git paw add-cli my-cli /path/to/my-cli --display-name "My CLI"
```

## How It Works

```
git paw start
    │
    ├─ 1. Validate git repo
    ├─ 2. Load config (global + repo)
    ├─ 3. Detect AI CLIs on PATH + custom
    ├─ 4. Interactive selection (or use flags)
    ├─ 5. Create git worktrees
    │      ../project-feat-auth/
    │      ../project-feat-api/
    ├─ 6. Create tmux session (paw-project)
    │      ┌─────────────────┬─────────────────┐
    │      │ feat/auth        │ feat/api         │
    │      │ → claude         │ → claude         │
    │      │                  │                  │
    │      └─────────────────┴─────────────────┘
    ├─ 7. Save session state to disk
    └─ 8. Attach to tmux session

git paw pause  → soft stop (detach + broker stop; CLIs keep running)
git paw stop   → kills CLIs, keeps worktrees + state
git paw start  → auto-recovers (or restarts a paused session)
git paw purge  → removes worktrees + state; deletes only merged git-paw branches
```

`purge` deletes a branch only when git-paw created it **and** it is fully
merged into the default branch — or into the branch it was created from, which
matters when you launch a wave from an integration branch — printing
`Deleted branch <name> (was <sha>)`. A branch with commits in neither, or one
whose merge state cannot be determined, is kept even under `--force`, and is
reported with its tip SHA plus how to resume (`git worktree add '<path>'
<branch>`) or discard (`git branch -d`/`-D <branch>`) it.
`git paw purge --stale` sweeps only sessions
whose tmux session is already gone, scoped to the current repository unless you
add `--all-repos`; live sessions are never touched.

## Security

git-paw runs AI agents that **execute arbitrary code with your user's
privileges** — often in full-auto with skipped permissions. The blast radius is
your user account: an agent (malicious, prompt-injected, or mistaken) can by
default read and write anything you can, including SSH keys, cloud credentials,
and other repositories.

git-paw itself makes **no** judgement about whether an agent's command is safe,
and approves **no** permission prompt on its own. Whether a command runs without
asking is decided by the agent CLI's own permission system — the permission mode
git-paw launches it with, plus the allow and deny rules you configure there. An
allow rule is only as narrow as you write it, and a `full-auto` level skips the
prompts entirely; neither is containment. For kernel-enforced confinement,
git-paw documents an optional, opt-in [FS-scoped sandbox][sandbox]
(`sandbox-exec` / `bwrap`) that scopes an agent's writes to its worktree.

If you ran v0.16.0 or earlier, note that the retired classifier may have
accepted durable "don't ask again" grants on your behalf, which persist in your
agent CLI's own settings file. git-paw creates no new ones and removes none —
review that file and delete what you did not intend.

Read the [Security Posture][posture] before trusting git-paw with autonomous
execution — it states plainly what each control does and does **not** guarantee.

[posture]: https://bearicorn.github.io/git-paw/user-guide/security-posture.html
[sandbox]: https://bearicorn.github.io/git-paw/user-guide/sandbox.html

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Releases

OpenSpec changes are archived **per-change on the feature branch** as they land
(syncing their delta specs into `openspec/specs/`); the archive directories are
gitignored, so the canonical post-archive state lives in `openspec/specs/`. The
`chore: prepare vX.Y.Z release` commit therefore touches **only** the version
bump in `Cargo.toml` and the `CHANGELOG.md` regenerated via `git cliff` — no
archive moves. Pushing the `vX.Y.Z` tag triggers
[`cargo-dist`](https://github.com/axodotdev/cargo-dist) on GitHub Actions to
build cross-platform binaries and update the Homebrew tap.

After the tag, the maintainer publishes to crates.io **manually** (it is not
wired into cargo-dist):

```bash
cargo publish --dry-run   # verify
cargo publish             # upload vX.Y.Z
```

The full procedure (archive ordering, changelog regeneration, tag rules,
crates.io publish step, recovery from a botched prep commit) lives in
[`AGENTS.md` § Release & Distribution](AGENTS.md#release--distribution).

## License

[MIT](LICENSE) — Copyright 2026 bearicorn

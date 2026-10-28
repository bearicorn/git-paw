# Quick Start: Orchestrator Mode

This walkthrough shows how to launch git-paw in **supervisor mode** and drive it from **your own agent session**, the orchestrator. Several coding agents work in parallel; the orchestrator verifies them, runs tests between merges, and publishes a final summary.

## What Orchestrator Mode Does

Supervisor mode with a spec flag (`git paw start --supervisor --from-all-specs`) launches every pending spec as its own coding agent in a tmux pane, with the dashboard above them. (Bare `git paw start --supervisor`, with no spec flag, opens the interactive branch picker instead.)

**The orchestrator runs outside the tmux session.** No orchestrator runs inside the session. The orchestrator is your own agent session (any agent CLI you like), run at the repository root and outside tmux. It loads the exported orchestrator skill and the per-session context, then drives the run through the bundled helpers (`.git-paw/scripts/sweep.sh`, `broker.sh`) or git-paw's MCP server. It runs the configured `test_command` between merges, walks branches in topological order based on declared dependencies, and publishes a final summary status when the run is complete.

It does **not** approve permission prompts. Whether a command runs without asking is your agent CLI's decision, and anything it prompts for is escalated to you. For a run that proceeds without stopping, pre-approve the commands in that CLI's own allow rules, or use `agent_approval = "full-auto"` inside a [sandbox](user-guide/sandbox.md).

## Prerequisites

- The standard git-paw [prerequisites](quick-start-same-cli.md#prerequisites): git-paw installed, tmux, an AI CLI on `PATH`, and a git repo.
- `[broker] enabled = true` in your config. Supervisor mode requires the coordination broker.
- A `[supervisor]` section is recommended but optional. If it is missing, `--supervisor` runs on built-in defaults, and a plain `git paw start` asks `Start in supervisor mode?`. No orchestrator CLI is needed to launch: the coding agents' CLI resolves from `--cli`, then `default_cli`, then `[supervisor].cli`, and the launch fails only when none of them is set.
- A specs directory configured via `[specs]` (default: `specs/`) containing at least one pending spec.

## Smallest Runnable Example

### 1. Initialize the repository

```bash
git paw init
```

`git paw init` writes `.git-paw/config.toml` (asking whether to enable supervisor mode by default and, if you do, for the test command) and installs the bundled helpers under `.git-paw/scripts/`. It also exports two skills and adds one pointer section:

- `.git-paw/skills/orchestrator/SKILL.md` is the orchestrator skill, rendered with your gate commands. It is the procedure your agent follows.
- `.git-paw/skills/cli-setup/SKILL.md` holds the consent rules for `git paw setup-cli`.
- A section between `<!-- git-paw-orchestrator:start -->` and `<!-- git-paw-orchestrator:end -->` is added to your root `AGENTS.md`. It tells any agent reading the file that, when a git-paw session is active, it is that session's orchestrator and should read the two files above.

Both skills follow the standard skill-directory layout (a `SKILL.md` with `name` and `description` frontmatter), so a skill-aware agent CLI can load them directly. Copy or symlink them into your CLI's own skill directory if it needs them there. `.git-paw/skills/` is gitignored, and `git paw start` re-renders both files, so they never lag your config or the binary. Re-running `git paw init` reports `Updated` only for a file whose content changed.

### 2. Create one OpenSpec-format spec

```bash
mkdir -p specs/add-greeting
cat > specs/add-greeting/tasks.md <<'MD'
## Add a greeting helper

- [ ] Add `pub fn greet(name: &str) -> String` to `src/lib.rs`.
- [ ] Add a unit test asserting `greet("paw") == "hello, paw"`.
- [ ] Run `cargo test` and make sure it passes.
MD
```

### 3. Configure broker + supervisor

`.git-paw/config.toml`:

```toml
[broker]
enabled = true
port = 9119

[specs]
dir = "specs"
type = "openspec"

[supervisor]
enabled = true
cli = "claude"
test_command = "cargo test"
agent_approval = "auto"
```

### 4. Launch

```bash
git paw start --supervisor --from-all-specs
```

git-paw creates a worktree for each spec and opens a tmux session. The dashboard is in pane 0 (a full-width top row). There is one coding agent per spec in the panes below it, from pane 1 onwards. The launch then prints the attach command and the two orchestrator paths:

```text
Supervisor session 'paw-myproject' launched with 1 coding agent(s).
Orchestrator context: .git-paw/AGENTS.local.md (point your orchestrating agent session at it)
Orchestrator skill:   .git-paw/skills/orchestrator/SKILL.md
Attach with:  tmux attach -t paw-myproject
```

`.git-paw/AGENTS.local.md` is the short **session context**. It holds the session name, the broker URL, the helper paths, any `[governance]` documents and, under `--unattended`, the drive-loop directive, and it points at the skill. It is the only file that carries this session's values. The exported skill carries none, so the same `SKILL.md` serves every session.

### 5. Load the skill in your own agent

Open your own agent session at the repository root, outside tmux. Have it load `.git-paw/skills/orchestrator/SKILL.md` and read `.git-paw/AGENTS.local.md`. An agent that reads the root `AGENTS.md` finds both through the pointer section. Over MCP, `get_skill` with the name `orchestrator` serves the same skill (`supervisor` is accepted as an alias).

From there the orchestrator:

1. **Follows events.** It blocks on the broker's event cursor (the MCP `wait_for_events` tool, or `.git-paw/scripts/sweep.sh events`), carrying the returned cursor forward. It inspects the session when an event or a stalled wave says to look, so it never polls pane captures on a timer. `get_session_status` and `sweep.sh status` give the roster at any time.
2. **Verifies.** It runs the five gates against each branch that reports done. It publishes a verdict only through the credential-carrying helpers: `sweep.sh verified <agent> <message>`, `sweep.sh feedback-gate <agent> <gate> <message>` or `sweep.sh conflict-ack <agent-a> <agent-b> <file>`. In the root session, the MCP `publish_verified` and `publish_feedback` tools do the same.
3. **Answers.** It replies to a worker's `agent.question` with `sweep.sh answer <agent> <text> [re]`.
4. **Merges.** Once every expected agent is verified, it fast-forwards each branch in topological order, runs the test command after each merge, and announces each advance with `sweep.sh advanced-main <merged-branch> <summary>`. In an attended run it may confirm the merge order with you first.

When every branch has merged (or been skipped), it publishes a final `agent.status` summarising which branches merged and which were skipped, and why.

### 6. Pair it with `--unattended`

```bash
git paw start --unattended --from-all-specs
```

`--unattended` adds an in-process drive loop: it keeps the coding-agent panes moving and escalates every live permission prompt to the broker, approving none. Your orchestrator reads those escalations through the event surface and relays each prompt to you. It runs `sweep.sh approve <pane>` only when you say so. In an unattended run the orchestrator merges as soon as every expected agent is verified, without asking first. It escalates only on a conflict (a non-fast-forward branch or a dependency cycle), an exhausted correction cycle, or a regression. If the loop exits on its heartbeat, re-run `git paw start --unattended` to re-attach a drive loop to the still-live session.

Your driving session can itself be remote-controlled from a phone through its own agent CLI. git-paw needs no extra topology for that. See [Remote control](user-guide/remote-control.md).

## Authoring a Change from Your Own Agent

An orchestrator connected over MCP can write the OpenSpec change it then launches, even without file tools of its own. Repository writes are off by default; you opt in per repository:

1. **Enable the switch** in the repository's own `.git-paw/config.toml` (the user-level config cannot enable it):

   ```toml
   [mcp]
   allow_repo_writes = true
   ```

2. **Restart the MCP server** at the repository root. The switch is read once at startup, and the write tools exist only for a server in the main working tree.
3. **Scaffold the change** with `create_spec` and a kebab-case name, such as `add-greeting`.
4. **Write each artifact** with `write_spec_artifact`: `proposal.md`, one `specs/<capability>/spec.md` per capability, `tasks.md`, and anything else your workflow uses.
5. **Check the structure** with `validate_spec`, and fix what it reports.
6. **Run your project's own spec validator.** `validate_spec` checks only a subset of the rules.
7. **Review `git diff` and commit yourself**, following your project's conventions. The tools never stage or commit.

See [Spec authoring](user-guide/mcp.md#spec-authoring-repository-writes-root-session-only) for the tools' parameters and the confinement rules.

## Skipping Supervisor for One Session

If your project sets `[supervisor] enabled = true` (the recommended setup for active development), every `git paw start` launches in supervisor mode. To skip it for a single session, pass `--no-supervisor`. This suits a quick debug-only run or a one-off branch flip, and you don't need to edit the config:

```bash
# Project config says [supervisor] enabled = true, but we want a plain session
git paw start --no-supervisor --cli claude --branches feat/quick-fix
```

`--no-supervisor` is the highest-precedence step in the [supervisor mode resolution chain](configuration/README.md#supervisor). It wins over both `[supervisor] enabled = true` in config and any prompt. It is mutually exclusive with `--supervisor`; passing both fails at parse time.

The flag only affects the current session. Your config is untouched, so the next `git paw start` returns to supervisor mode as configured.

## Key Config Knobs

The full reference lives in [Configuration → Supervisor](configuration/README.md#supervisor). The fields you will reach for most often:

| Field | What it controls |
|-------|------------------|
| `[supervisor].enabled` | Default to supervisor mode without passing `--supervisor`. |
| `[supervisor].cli` | The orchestrator's CLI, named in its context (falls back to `default_cli`); also the coding agents' last CLI fallback. No pane runs it. |
| `[supervisor].test_command` | Command run after each agent reports `done` and again after every merge. Rendered into the exported skill. |
| `[supervisor].agent_approval` | `"manual"`, `"auto"`, or `"full-auto"`. Translates into CLI permission flags for the coding agents. |
| `[supervisor].approval` | The orchestrator's **own** level, reserved for git-paw-initiated orchestrator invocations. Still parsed; it changes no pane command. |
| `[supervisor].manual_approvals_log` | Records every prompt an `--unattended` run escalates, for `git paw approvals` to aggregate (default on). |

> `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` are **deprecated and have no effect**. They still parse so old configs load, but each file declaring them warns once. Pre-approve routine commands in your agent CLI's own allow rules instead. See [Deprecated auto-approval tables](configuration/README.md#deprecated-auto-approval-tables).

See [Configuration → Broker](configuration/README.md#broker) for `[broker]` settings and [Configuration → Dashboard](configuration/README.md#dashboard) if you want the live broker-message panel turned on.

## Broker Wire Format

Every coordination message the orchestrator and its agents exchange goes through the broker as a JSON envelope. The canonical example below comes from `src/broker/messages.rs`. The broker accepts an `agent_id` of `supervisor` or a lowercase two-part slug: letters, digits and `-`, with the parts joined by `-` or `/`. git-paw itself publishes the slug form, so branch `feat/auth` becomes `feat-auth`.

```json
{"type": "agent.status", "agent_id": "feat-auth", "payload": {"status": "working", "modified_files": ["src/auth.rs"], "message": "wiring JWT verifier"}}
```

Nobody hand-writes these envelopes:

- The coding agents publish theirs through `.git-paw/scripts/broker.sh` (`status`, `artifact`, `blocked`, `question`, `intent`) or the MCP publish tools.
- The orchestrator publishes through `.git-paw/scripts/sweep.sh` (`status-publish`, `verified`, `feedback-gate`, `conflict-ack`, `learn`, `answer`, `advanced-main`) or `broker.sh --agent supervisor` (`question`, `intent`).
- The filesystem watcher and the post-commit hook publish `agent.status` and `agent.artifact` automatically.

The rest of the message set is `agent.blocked`, `agent.intent`, `agent.question`, `agent.answer`, `agent.feedback`, `agent.verified`, `agent.advanced-main` and `agent.learning`, plus the broker-emitted `supervisor.verify-now`. See [Agent Coordination](user-guide/coordination.md) for the full reference with payload-field semantics.

## Where to Look When Things Go Wrong

When a run misbehaves, these places have everything you need:

1. **Dashboard pane (pane 0).** With `[broker] enabled = true` the dashboard runs in pane 0 of the session and updates every second. Use it to spot which agent is `blocked`, which is `working`, and how long it has been since each agent's last status update. See the [Dashboard chapter](user-guide/dashboard.md) for status-symbol meanings and controls.
2. **Your orchestrator session.** The orchestrator runs in your own agent session outside tmux and reports there as it walks agents through the merge plan. Its broker messages also appear in the dashboard's broker log.
3. **The orchestrator's final status.** When its merge loop completes, the orchestrator publishes a final `agent.status` (agent `supervisor`). It lists which branches merged cleanly, which were skipped and why, and any regressions it hit. It shows up in the dashboard's broker log panel.
4. **`<session_name>.broker.log`.** This plain-text audit trail has one line per message the broker handled (`[<seq>] <unix-seconds> [<agent_id>] <message summary>`).
   - It lives in git-paw's sessions directory, not in `.git-paw/`. On Linux that is `~/.local/share/git-paw/sessions/<session_name>.broker.log` (or under `$XDG_DATA_HOME`). On macOS it is `~/Library/Application Support/git-paw/sessions/<session_name>.broker.log`.
   - The name carries the session, so two repositories each keep their own log.
   - Tail it live with `tail -f` to watch coordination as it happens. The file is appended every five seconds; see [Agent Coordination](user-guide/coordination.md) for the message types.

To inspect an individual agent's pane history, run `.git-paw/scripts/sweep.sh capture <pane>`, or `tmux capture-pane -p -t <session>:0.<pane>` against the session named in `.git-paw/AGENTS.local.md`. The agent panes are in window 0; the session's second window, `broker`, hosts the broker.

## Governance

If your project has existing governance docs (ADRs, DoD, security checklist, test strategy, constitution), point the orchestrator at them via the `[governance]` table in `.git-paw/config.toml`. When set, those paths are written into the session context (`.git-paw/AGENTS.local.md`), and the orchestrator consults each doc as a sub-step of its spec-audit step. Findings flow through the existing `agent.feedback` errors path, with no new wire format. See the [Governance chapter](user-guide/governance.md) for the full config, what the orchestrator checks per doc, and illustrative starting-point examples for each doc type.

## What's Next

- [User Guide → Supervisor](user-guide/supervisor.md#driving-the-session-from-your-own-agent): driving the session from your own agent, in depth.
- [User Guide → Remote control](user-guide/remote-control.md): driving the session from a phone.
- [User Guide → MCP Server](user-guide/mcp.md): the read and write tools the orchestrator can use.
- [User Guide → Agent Coordination](user-guide/coordination.md): broker message types and how agents talk to each other.
- [User Guide → Dashboard](user-guide/dashboard.md): the dashboard pane's status table (pane 0 in every mode with a broker), controls, and configuration.
- [User Guide → Governance](user-guide/governance.md): pointing the orchestrator at your project's existing governance docs.
- [Configuration → Supervisor](configuration/README.md#supervisor): every supervisor field with defaults.

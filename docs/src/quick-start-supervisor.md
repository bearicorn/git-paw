# Quick Start: Supervisor Mode

This walkthrough shows how to launch git-paw in **supervisor mode** — git-paw's hands-off, spec-driven workflow where a supervisor agent orchestrates several coding agents in parallel, runs tests between merges, and publishes a final summary.

## What Supervisor Mode Does

Supervisor mode with a spec flag (`git paw start --supervisor --from-all-specs`) launches every pending spec as its own coding agent in a background tmux pane and gives you an interactive supervisor agent in the foreground pane. (Bare `git paw start --supervisor`, with no spec flag, opens the interactive branch picker instead.) The supervisor automatically launches all agents per spec, runs the configured `test_command` between merges, walks branches in topological order based on declared dependencies, publishes a final summary status when the run is complete, and auto-approves common safe permission prompts (e.g. `git commit`, broker `curl` calls, and your declared stack's toolchain verbs such as `cargo test` for `stacks = ["rust"]`) so the run can proceed unattended.

## Prerequisites

- The standard git-paw [prerequisites](quick-start-same-cli.md#prerequisites) (git-paw installed, tmux, an AI CLI on `PATH`, a git repo).
- `[broker] enabled = true` in your config — supervisor mode requires the coordination broker.
- A `[supervisor]` section is recommended but optional. If it is missing, `--supervisor` runs on built-in defaults and takes the supervisor CLI from `default_cli` (one of `[supervisor].cli` or `default_cli` must be set), and a plain `git paw start` asks `Start in supervisor mode?`. `git paw init` writes the section for you, asking whether to enable supervisor mode by default and, if you do, for the test command.
- A specs directory configured via `[specs]` (default: `specs/`) containing at least one pending spec.

## Smallest Runnable Example

### 1. Create one OpenSpec-format spec

```bash
mkdir -p specs/add-greeting
cat > specs/add-greeting/tasks.md <<'MD'
## Add a greeting helper

- [ ] Add `pub fn greet(name: &str) -> String` to `src/lib.rs`.
- [ ] Add a unit test asserting `greet("paw") == "hello, paw"`.
- [ ] Run `cargo test` and make sure it passes.
MD
```

### 2. Configure broker + supervisor

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

### 3. Launch

```bash
git paw start --supervisor --from-all-specs
```

git-paw creates a worktree per spec, opens a tmux session with the supervisor in pane 0, the dashboard in pane 1, and one coding agent per spec in the remaining panes (pane 2 onwards). The supervisor injects its boot prompt automatically; you only need to chat with it for high-level approvals.

When every agent has reported `verified` and the supervisor has merged each branch in topological order, the supervisor publishes a final `agent.status` summarising which branches merged and which were skipped (and why).

## Skipping Supervisor for One Session

If your project sets `[supervisor] enabled = true` (the recommended setup for active development), the supervisor runs by default for every `git paw start`. To skip the supervisor for a single session — e.g. for a quick debug-only run or a one-off branch flip — without editing the config, pass `--no-supervisor`:

```bash
# Project config says [supervisor] enabled = true, but we want a plain session
git paw start --no-supervisor --cli claude --branches feat/quick-fix
```

`--no-supervisor` is the highest-precedence step in the [supervisor mode resolution chain](configuration/README.md#supervisor) — it wins over both `[supervisor] enabled = true` in config and any prompt. It is mutually exclusive with `--supervisor`; passing both fails at parse time.

The flag only affects the current session. Your config is untouched, so the next `git paw start` returns to supervisor mode as configured.

## Key Config Knobs

The full reference lives in [Configuration → Supervisor](configuration/README.md#supervisor). The fields you will reach for most often:

| Field | What it controls |
|-------|------------------|
| `[supervisor].enabled` | Default to supervisor mode without passing `--supervisor`. |
| `[supervisor].cli` | Which AI CLI runs as the supervisor (falls back to `default_cli`). |
| `[supervisor].test_command` | Command run after each agent reports `done` and again after every merge. |
| `[supervisor].agent_approval` | `"manual"`, `"auto"`, or `"full-auto"` — translates into CLI permission flags for the coding agents. |
| `[supervisor].approval` | The supervisor pane's **own** level, decoupled from `agent_approval` (unset → inherits it). Set `"full-auto"` for [unattended runs](user-guide/supervisor.md#run-the-supervisor-pane-at-native-full-auto) without dropping agent guardrails. |
| `[supervisor.auto_approve].enabled` | Master switch for git-paw's safe-prompt auto-dismisser. |
| `[supervisor.auto_approve].safe_commands` | Project-specific command prefixes appended to the composed safe list. |
| `[supervisor.auto_approve].approval_level` | `"off"`, `"conservative"`, or `"safe"` preset for the auto-approve whitelist. |
| `[supervisor.common_dev_allowlist].stacks` | Named stack presets (`rust` / `node` / `python` / `go`) whose toolchain verbs are folded into the auto-approve whitelist and the rendered skill guidance (e.g. `["rust"]` makes `cargo test` auto-approve). |
| `[supervisor.common_dev_allowlist].extra` | Project-specific prefix patterns appended to the built-in preset (e.g. `["pnpm test", "deno fmt"]`). Also folded into the auto-approve whitelist. |

See [Configuration → Broker](configuration/README.md#broker) for `[broker]` settings and [Configuration → Dashboard](configuration/README.md#dashboard) if you want the live broker-message panel turned on.

## Broker Wire Format

Every coordination message the supervisor and its agents exchange goes through
the broker as a JSON envelope of the form
`{"type": "agent.<variant>", "agent_id": "<slug>", "payload": {...}}`. The
canonical examples below come from `src/broker/messages.rs`. The broker accepts
an `agent_id` of `supervisor` or a lowercase two-part slug — letters, digits and
`-`, with the parts joined by `-` or `/` (git-paw itself publishes the slug
form: branch `feat/auth` becomes `feat-auth`).

`agent.status` (auto-published by the filesystem watcher whenever an agent's
worktree changes; the manual form below is an escape hatch):

```bash
curl -s -X POST "$GIT_PAW_BROKER_URL/publish" \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.status","agent_id":"feat-auth","payload":{"status":"working","modified_files":["src/auth.rs"],"message":"wiring JWT verifier"}}'
```

`agent.artifact` (auto-published by the post-commit hook with the committed
files; manual form is the escape hatch for code-less tasks):

```bash
curl -s -X POST "$GIT_PAW_BROKER_URL/publish" \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.artifact","agent_id":"feat-auth","payload":{"status":"done","exports":["AuthClient"],"modified_files":[]}}'
```

`agent.blocked`, `agent.intent`, `agent.question`, `agent.answer`,
`agent.feedback`, `agent.verified`, `agent.advanced-main`, and `agent.learning`
— plus the broker-emitted `supervisor.verify-now` — round out the message set;
see the full reference with payload-field semantics in
[Agent Coordination](user-guide/coordination.md).

## Where to Look When Things Go Wrong

When a supervisor run misbehaves, these places have everything you need:

1. **Dashboard pane (pane 1).** With `[broker] enabled = true` the dashboard runs in pane 1 of the supervisor session and updates every second. Use it to spot which agent is `blocked`, which is `working`, and how long it has been since each agent's last status update. See the [Dashboard chapter](user-guide/dashboard.md) for status-symbol meanings and controls.
2. **Supervisor pane (pane 0).** The supervisor CLI runs here and prints feedback inline as it walks agents through the merge plan. Scroll back with tmux's copy mode if you need to inspect earlier reasoning.
3. **The supervisor's final status.** When its merge loop completes, the supervisor publishes a final `agent.status` (agent `supervisor`) listing which branches merged cleanly, which were skipped and why, and any regressions it hit. It shows up in the dashboard's broker log panel.
4. **`<session_name>.broker.log`.** A plain-text audit trail with one line per message the broker handled (`[<seq>] <unix-seconds> [<agent_id>] <message summary>`), written to git-paw's sessions directory — not to `.git-paw/` — at `~/.local/share/git-paw/sessions/<session_name>.broker.log` on Linux (or under `$XDG_DATA_HOME`) and `~/Library/Application Support/git-paw/sessions/<session_name>.broker.log` on macOS. The name carries the session, so two repositories each keep their own log. Tail it live with `tail -f` to watch coordination as it happens. The file is appended every five seconds; see [Agent Coordination](user-guide/coordination.md) for the message types.

If you need to inspect an individual agent's pane history, run `tmux capture-pane -p -t <session>:<window>.<pane>` against the session printed at launch.

## Governance

If your project has existing governance docs (ADRs, DoD, security checklist,
test strategy, constitution), point the supervisor at them via the
`[governance]` table in `.git-paw/config.toml`. When set, those paths are
injected into the supervisor's boot prompt and the supervisor consults each doc
as a sub-step of its spec-audit step. Findings flow through the existing
`agent.feedback` errors path — no new wire format. See the [Governance
chapter](user-guide/governance.md) for the full config, what the supervisor
checks per doc, and illustrative starting-point examples for each doc type.

## What's Next

- [User Guide → Agent Coordination](user-guide/coordination.md) — broker message types and how agents talk to each other.
- [User Guide → Dashboard](user-guide/dashboard.md) — dashboard pane status table (pane 1 in supervisor mode, pane 0 otherwise), controls, and configuration.
- [User Guide → Governance](user-guide/governance.md) — pointing the supervisor at your project's existing governance docs.
- [Configuration → Supervisor](configuration/README.md#supervisor) — every supervisor and `auto_approve` field with defaults.

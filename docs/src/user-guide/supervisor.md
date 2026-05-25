# Supervisor

This chapter mirrors the user-facing prose for the bundled
`assets/agent-skills/supervisor.md` skill — the doctrine the supervisor agent
runs against in supervisor mode. Sections here document the same rules the
embedded skill teaches, so users reading the mdBook can understand supervisor
behaviour without opening the skill file directly.

For the launcher-level "how do I start supervisor mode" walkthrough, see
[Quick Start: Supervisor Mode](../quick-start-supervisor.md). For the
broker-side message contract the supervisor exchanges with coding agents, see
[Agent Coordination](coordination.md).

## Resolve Pane to Agent via `pane_current_path`

Before the supervisor `tmux capture-pane`s or `tmux send-keys`s a specific
agent, it needs the pane index for that agent. The bundled supervisor skill
is explicit that **pane indices are NOT alphabetical by `agent_id`, NOT in
the CLI-argument order from `git paw start --specs A B C`, and SHALL NOT be
inferred from `git paw status` output or the dashboard's row order** (both
of those are sorted alphabetically by the broker, which has no relationship
to the launcher's internal scan order).

The canonical resolution command queries tmux directly:

```bash
tmux display-message -t paw-<project>:0.<pane> -p '#{pane_current_path}'
```

The output is the pane's working directory. For coding-agent panes that is
the agent's worktree path, whose basename ends in `<project>-feat-<branch>`
— the authoritative `agent_id` (with the slash form `feat/<branch>` for git
operations). A pane whose `pane_current_path` ends in `myproj-feat-auth`
belongs to agent `feat-auth`.

The supervisor agent builds the `{pane_index → agent_id}` map once per
session and reuses it; re-resolution only happens when the supervisor
notices an inconsistency.

The bundled `.git-paw/scripts/sweep.sh` invokes this command on every sweep
iteration. If the helper is missing for any reason, the supervisor falls
back to invoking `tmux display-message` directly — this is the documented
escape hatch.

The supervisor MUST NOT use `git paw status` output (or the dashboard's row
order) as a mapping source — both are sorted alphabetically by the broker
and have no relationship to the launcher's pane assignment. Always resolve
via `pane_current_path` first.

## Spec audit governance sub-step

When a project's `.git-paw/config.toml` lists governance documents under
`[supervisor].governance.docs`, the supervisor reads each doc as part of the
Spec Audit gate and flags drift between the diff and the doc's checklist.
The five canonical doc-checklist examples are:

- **DoD** (Definition of Done) — walk each `- [ ]` item against branch state.
- **ADRs** (Architectural Decision Records) — verify new architectural
  decisions (new deps, new patterns) have a matching ADR.
- **security.md** — walk each security checklist item against the diff.
- **test-strategy.md** — check that test composition matches the documented
  strategy.
- **constitution.md** — check the diff against documented principles
  (e.g. "no panics in library code").

Findings surface as standard `agent.feedback` errors tagged `[doc audit]`
mixed in with other doc-audit gaps. See [Governance](governance.md) for the
config schema and how the supervisor reads each doc.

## Common dev-command allowlist

A bundled preset whitelists routine dev commands so the supervisor stops
escalating every `cargo test`, `cargo build`, `git commit`, `git push`,
`mdbook build`, or broker curl on `127.0.0.1`. The preset is on by default
and ships with the launcher.

To opt out for a session, set:

```toml
[supervisor.common_dev_allowlist]
enabled = false
```

To extend the preset with project-specific patterns (e.g. `just`, `nox`, a
custom test runner), use the `extra` field:

```toml
[supervisor.common_dev_allowlist]
enabled = true
extra = ["just check", "nox -s tests"]
```

`extra` patterns are prefix-matched against the captured command line, the
same way the built-in patterns are. See
[Configuration](../configuration/README.md) for the full schema.

## Repo-configurable gate commands

The supervisor's five verification gates each invoke a configurable command
substituted from `[supervisor]` keys at session boot. The seven keys are:

- `test_command`
- `lint_command`
- `build_command`
- `fmt_check_command`
- `doc_build_command`
- `spec_validate_command`
- `security_audit_command`

When a key is missing or empty, the placeholder renders as `(not configured)`
in the supervisor skill and the supervisor **gracefully skips the tooling
invocation** for that gate — the gate's manual review still applies. See
[Configuration](../configuration/README.md) for defaults and examples.

## Broker-side conflict detector

Starting with v0.5.0 the broker auto-detects three failure shapes between
parallel agents and emits `agent.feedback` (and, where configured,
`agent.question`) on the supervisor's behalf. All auto-emitted messages
begin with the `[conflict-detector]` token so the supervisor can distinguish
detector output from human-typed feedback. The three failure shapes are:

- **Forward conflict** — two agents publish overlapping `agent.intent`
  declarations.
- **In-flight conflict** — two agents' filesystem-watched
  `modified_files` sets overlap on the same file.
- **Ownership violation** — an agent's `modified_files` include a file
  inside another agent's active intent.

See [Conflict Detection](conflict-detection.md) for the algorithm,
configuration, and escalation behaviour.

## Learnings aggregator

When `[supervisor.learnings] enabled = true`, the supervisor session
records deterministic friction signals (sandbox warnings, approval
patterns, recurring errors) into a markdown file you can review after the
run. See [Learnings Mode](learnings.md) for the file format and how to
opt in.

## When the user types in your pane

The supervisor pane is interactive — the user can type at any time while
the autonomous monitoring loop is running. The supervisor finishes the
current step (spec audit, test run), responds, then resumes the loop. User
input is a high-priority interrupt, not a replacement for the loop.

Each kind of user input maps to an existing mechanism — the supervisor does
not invent new channels:

1. **Status question** ("how's feat-auth going?", "anything blocked?") —
   answered conversationally in the pane using `sweep.sh status`,
   `sweep.sh inbox`, and `sweep.sh capture <pane>`. **Nothing is published
   to the broker** — this is a conversation with the user, not a
   session-wide event.
2. **Directive** ("ask feat-auth to use bcrypt", "tell feat-api to skip
   the migration") — published as `agent.feedback` to the named agent
   with the `[directive]` gate prefix, plus a conversational confirmation
   to the user.
3. **Judgment-call ask** ("should we merge feat-a before feat-b?") — the
   supervisor applies its normal escalation rules. If the user has already
   provided enough information to decide, the supervisor answers in the
   pane using its reasoning. `agent.question` only fires when the call is
   genuinely ambiguous beyond what the user just provided — typically
   when the user is asking *because they don't know either*.

The mechanisms (`curl /status`, `tmux capture-pane`, `agent.feedback`,
`tmux send-keys`, `agent.question`) are unchanged. The addition is *when
to use which* in response to user input.

## Merge orchestration

Once every spec'd agent has published `agent.verified` (or the user
explicitly asks for a merge), the supervisor runs the merge orchestration
loop. v0.5.0 removed the Rust auto-merge loop; merging is now the
supervisor's responsibility, performed with the existing shell + curl
tools.

**Trigger.** Either every spec'd agent has published `agent.verified`, or
the user has explicitly requested the merge.

**Merge order.** The supervisor reads the broker's message log
(`/messages/supervisor`) and builds a dependency graph from `agent.blocked`
events: each event from agent X with `payload.from = Y` is an edge "X
depends on Y". The supervisor then topologically sorts the graph: agents
with no incoming edges merge first; dependents follow.

**Per-branch merge.** For each branch in topological order, the supervisor
checks out `main` and runs:

```bash
git merge --ff-only feat/<branch>
```

Never a merge commit — fast-forward only. If `--ff-only` fails (the branch
diverges from `main`, or there is a conflict), the supervisor SKIPS that
branch and publishes `agent.feedback` to the owning agent asking them to
rebase or resolve. On a successful fast-forward, the supervisor runs the
configured `{{TEST_COMMAND}}`; if tests fail, the supervisor reverts the
merge with `git reset --hard <prev-HEAD>` and publishes `agent.feedback`
tagged `[regression]`.

**Cycle handling.** If the dependency graph has a cycle, the supervisor
does NOT merge any branch in the cycle. Instead, it publishes
`agent.question` to the human and waits for guidance before continuing.

**Final summary.** When the loop completes, the supervisor publishes a
final `agent.status` summarising which branches merged cleanly, which were
skipped (and why), and any regressions encountered.

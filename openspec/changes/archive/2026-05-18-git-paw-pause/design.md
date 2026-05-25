## Context

`git paw stop` today calls `tmux::kill_session`, which terminates every
pane process — including the dashboard pane that hosts the broker AND
every coding-agent CLI. The CLI processes lose their in-memory state
on `SIGKILL`, so the next `git paw start` invocation spawns fresh CLIs
with no conversation history.

tmux natively supports a softer alternative: `tmux detach-client`
detaches the user's client from the tmux server but leaves the server,
the session, and every pane process running. Re-attaching via `tmux
attach -t <session>` resumes instantly — every pane is in the exact
state the user left it.

git-paw can compose `detach-client` with a broker-shutdown step (which
the user explicitly requested in drift 60: "leave CLIs + tmux session
alive AND stop the broker") to produce a new `pause` verb. The
broker matters because it owns a TCP port and a background tokio
runtime — leaving it running across a long break is wasteful and
collides with subsequent unrelated sessions binding the same port.

The relevant existing code paths:

- `src/main.rs::cmd_stop` — current destructive stop.
- `src/main.rs::cmd_start` — detects `existing_session.alive` and
  re-attaches if alive (line 326-332). This becomes the natural
  restart-from-pause hook.
- `src/broker/mod.rs::BrokerHandle::Drop` — already handles graceful
  shutdown when the dashboard subprocess exits. The pause flow
  triggers this by killing the dashboard pane only.
- `src/tmux.rs::kill_session` — what stop calls today. Pause needs a
  new sibling (`detach_client`) plus optionally `kill_pane` for the
  dashboard pane.
- `src/session.rs::SessionStatus` — two-variant enum
  (`Active` / `Stopped`); needs a third (`Paused`).

The open question from drift 60 — "should `stop` become an alias for
`pause`, grow a confirmation prompt, or remain as-is?" — is resolved
in D5 below. Short answer: **option (b), keep `stop` destructive but
add a TTY confirmation prompt + `--force` bypass + clearer help text**.

## Goals / Non-Goals

**Goals:**

- A new `git paw pause` subcommand that performs the soft-stop the
  user described in drift 60: detach client, stop broker, leave
  tmux session + CLI panes alive.
- A new `SessionStatus::Paused` state that `git paw status` displays
  distinctly and that `git paw start` interprets as "restart broker +
  re-attach, do not spawn new CLIs."
- Symmetric `git paw start` flow against a paused session that is
  cheap (no worktree creation, no CLI spawn, no boot prompts).
- Predictable `git paw stop` after `git paw pause`: detect paused
  state and proceed with the destructive kill (idempotent semantics).
- Resolve drift 60's "stop is too destructive UX-wise" open question
  with a back-compat-preserving fix (D5).

**Non-Goals:**

- A "true" hibernate that swaps RAM to disk and survives reboots —
  v1.0.0 material; out of scope.
- Per-CLI `--continue` / `--resume` on cold-start recovery — drift 61;
  out of scope, but cross-referenced in D8.
- Automatic pause on inactivity / scheduled pause — out of scope.
- A `paws` / `pause-all` aggregate that pauses every git-paw session
  on the machine — out of scope.
- Making the broker survive pause (keeping it running). The user
  explicitly asked for the broker to stop on pause; D2 below covers
  the mechanism.

## Decisions

### D1. Detach mechanism: `tmux detach-client -s <session>`

The pause flow detaches all clients attached to the session by running
`tmux detach-client -s <session-name>`. This is the server-side form
that does not require a particular client to be the caller. It works
in three calling contexts:

- **From inside the tmux session itself (the user runs `git paw pause`
  in one of the panes).** `detach-client -s` detaches the current
  client along with any other clients attached to the same session.
  The user's terminal returns to the parent shell.
- **From outside the session (the user runs `git paw pause` in a
  different terminal that is not attached to the paw tmux).** The
  command issues to the tmux server, the attached client (if any) is
  detached, and `git paw pause` returns.
- **With no client attached (already pause-like state).** `tmux
  detach-client -s <session>` is a no-op against an unattached
  session and exits 0. Pause uses this as part of its idempotent
  semantics — running pause twice is safe.

**Rejected alternative:** `tmux detach-client -t <client-tty>`. This
form requires identifying *which* client to detach. From inside the
session we'd use `$TMUX_CLIENT` or `tmux list-clients` — adds parsing
and edge-cases. `-s <session>` is simpler and matches the user
intent: "no one should be attached to this session after pause."

### D2. Broker stop without killing tmux: kill the dashboard pane only

The broker runs inside the `__dashboard` subprocess hosted by the
dashboard pane (pane 0 in bare-start mode, pane 1 in supervisor mode
per `supervisor-as-pane`). `BrokerHandle::Drop` already implements
graceful shutdown — flush log, drain in-flight HTTP requests,
shutdown the tokio runtime within 2s. That drop fires when the
`__dashboard` subprocess exits.

Pause triggers the drop by killing the dashboard pane *only* (via
`tmux kill-pane -t <session>:0.<dashboard-pane-index>`), leaving all
agent panes alive. The dashboard pane's subprocess receives `SIGHUP`,
exits, the drop runs, the broker shuts down cleanly, the log flushes.

The dashboard pane index is read from the session state — bare-start
mode = 0, supervisor mode = 1. The session state's `broker_port` and
`broker_log_path` are *not* cleared on pause; they describe the
broker that *was* running and that will be re-created on restart.

**Rejected alternative 1:** `SIGINT` (Ctrl-C) sent via `tmux send-keys
-t <dashboard-pane> C-c`. The dashboard TUI handles `q` to quit but
the broker process doesn't directly listen for SIGINT — quitting the
TUI is what drops the `BrokerHandle`. We *could* send `q Enter`, but
`kill-pane` is more direct, more reliable (no race against TUI input
handling), and works regardless of which CLI happens to be running
inside the dashboard pane in some future refactor.

**Rejected alternative 2:** Add an explicit `POST /shutdown` endpoint
to the broker. Cleaner contract on paper, but introduces a new attack
surface (any localhost client could shut down the broker) and
requires a new endpoint + auth scaffold. Killing the pane reuses the
existing drop logic with zero new code paths inside the broker.

**Rejected alternative 3:** Leave the broker running across pause.
The user explicitly asked for the broker to stop. Reasonable rationale:
the broker holds a TCP port and a background tokio runtime; a paused
session that may sit for hours should not block port 9119 from being
re-used by an unrelated session.

**Gotcha noted during code review:** `BrokerHandle::reattached` mode
(when the broker subprocess attached to an *existing* live broker)
has a no-op drop. This does not affect pause — pane 0 *creates* the
broker, it does not reattach to one. But the restart-from-pause flow
(D6) must use the same `start_broker` path the first launch did, so
the new dashboard subprocess does not skip its own owned-runtime.
Already a property of `start_broker`'s probe logic — call out in
tasks for the implementer.

### D3. Session state: third variant `SessionStatus::Paused`

`SessionStatus` becomes a three-variant enum:

```rust
pub enum SessionStatus {
    Active,
    Paused,
    Stopped,
}
```

Serde lowercase rename gives `"active" | "paused" | "stopped"` on
disk. v0.4 sessions (only `active` / `stopped`) load fine because the
new variant doesn't appear in any v0.4-saved file.

`effective_status(is_tmux_alive)` is updated to:

```
if self.status == SessionStatus::Active && !is_tmux_alive(name) => Stopped
if self.status == SessionStatus::Paused && !is_tmux_alive(name) => Stopped  // tmux died despite pause
if self.status == SessionStatus::Paused && is_tmux_alive(name) => Paused    // intended state
otherwise => self.status
```

The "tmux died despite pause" case downgrades to `Stopped` because
the CLI processes are gone — recovery from `Paused` requires the
tmux session and panes to still be alive. If something killed the
tmux server (drift 35), the session is effectively stopped and the
user has to take the cold-recovery path on the next start.

`git paw status` displays:

| Recorded | Tmux alive | Effective | Emoji | Restart action          |
|----------|------------|-----------|-------|-------------------------|
| Active   | yes        | Active    | green | re-attach               |
| Active   | no         | Stopped   | yellow| recover (fresh CLIs)    |
| Paused   | yes        | Paused    | blue  | re-attach + restart broker |
| Paused   | no         | Stopped   | yellow| recover (fresh CLIs)    |
| Stopped  | any        | Stopped   | yellow| recover (fresh CLIs)    |

### D4. Cost / trade-off framing in help text

Pause keeps the CLI processes alive, which keeps their RAM allocation
alive. With ~10 Claude panes at ~300 MB each, a paused session holds
roughly 3-5 GB of RAM. The `git paw pause --help` long-about
explicitly states this:

> Pause detaches the tmux client and stops the broker, but leaves all
> CLI processes running in the background. This preserves agent
> conversation state for instant resume via `git paw start`. RAM stays
> allocated (~300 MB per Claude pane). Use pause for short breaks
> (lunch, meetings, end-of-day). For a longer break, use `git paw
> stop` to kill the CLIs while keeping worktrees, then restart later
> with a cold recovery. A future `git paw hibernate` (v1.0.0) will
> snapshot state to disk.

This frames the trade-off honestly: pause is the fast / state-preserving
option but holds RAM; stop is the slow / state-losing option but
releases RAM; future hibernate will be the best-of-both.

### D5. `git paw stop` open question — resolution: option (b)

Drift 60's open question gave three options:

- **(a) `stop` becomes an alias for `pause`.** Cleanest from a
  "least-destructive default" angle, but a breaking change for any
  script or muscle-memory user who relies on `stop` actually killing
  processes. Pause and stop also have observably different effects on
  RAM and the broker port; aliasing them muddies the distinction we
  just spent design effort drawing in D1-D4.
- **(b) `stop` remains as-is but grows a confirmation prompt + clearer
  help text + a `--force` bypass.** Back-compat-preserving (scripts
  using `git paw stop --force` work unchanged after a one-line
  addition; scripts not setting `--force` only see the prompt in TTY
  contexts — non-TTY contexts skip the prompt). Adds the UX guardrail
  drift 60 asked for without breaking the verb's meaning.
- **(c) Keep `stop` as-is, document the trade-off in `--help` only.**
  Minimum change, but the dogfood evidence (drift 60) shows
  `--help`-only is insufficient: users hit `stop` reflexively and the
  destructive consequence is invisible until they try to restart.

**Decision: (b).** Specifically:

1. `cmd_stop` reads `force: bool` from CLI args (new field on `Stop`).
2. When `force` is false AND stdin is a TTY, render a confirmation
   prompt (using the existing `dialoguer::Confirm` already in the
   dep tree, same pattern as `purge`):
   > `git paw stop` will kill all CLI processes in panes
   > (`<branch1>`, `<branch2>`, …), losing each agent's conversation
   > context. Worktrees and branches are preserved on disk.
   >
   > For a soft stop that preserves state, use `git paw pause`.
   > For a full reset (removes worktrees too), use `git paw purge`.
   >
   > Continue stopping? [y/N]
3. When `force` is true OR stdin is not a TTY, skip the prompt and
   proceed with the kill (today's behaviour).
4. `Command::Stop`'s `long_about` is rewritten to name all three
   verbs (pause / stop / purge) with a one-line summary of each.

The non-TTY skip preserves CI / automation back-compat without
requiring a `--force` flag for those callers. Scripts that *are*
attached to a TTY but want no prompt set `--force`.

**Rejected alternative within (b):** "Always prompt regardless of TTY"
breaks `assert_cmd` tests and CI flows that don't expect interactive
prompts. The TTY check is standard for confirmation prompts (purge
uses the same pattern).

### D6. Symmetric `git paw start` flow for paused sessions

`cmd_start`'s existing reattach branch (line 326-332 of `main.rs`)
matches on "tmux alive." We extend it to inspect
`existing.effective_status(is_tmux_alive)`:

```
Active + alive    → re-attach (today's path)
Stopped + alive   → impossible (effective_status downgrades to Stopped)
Stopped + dead    → recover (today's path — fresh CLI spawn)
Paused  + alive   → restart-from-pause (new path)
Paused  + dead    → recover (downgraded by effective_status)
```

The restart-from-pause path:

1. Recreate the dashboard pane in its original index (pane 0 for
   bare-start, pane 1 for supervisor mode — read from session state).
2. Send the `git paw __dashboard` command via `tmux send-keys`.
3. The dashboard subprocess starts, `start_broker` runs, broker
   listens on the configured port, log file reopens (appending —
   `broker_log_path` is reused).
4. Update session state: `status: Active` (no longer paused).
5. `tmux attach` to bring the user back into the session.

Cost: one tmux pane spawn + one process exec + one broker boot. No
worktree creation, no per-pane CLI process spawn, no boot prompts.
The agent panes are exactly as the user left them.

**Edge case: the dashboard pane index has shifted.** Supervisor mode
moved the dashboard from pane 0 to pane 1 (`supervisor-as-pane`).
The restart flow reads `session.broker_port` to detect whether the
session had a broker at all, but the pane *index* needs to be stored
somewhere. Two options:

- **(D6.a)** Store the dashboard pane index in `Session`
  (`dashboard_pane: Option<u32>`). Explicit, survives any future
  index shifts. Adds a new optional serde field (`#[serde(default,
  skip_serializing_if = "Option::is_none")]`).
- **(D6.b)** Derive it at restart time from the saved supervisor
  state. Today: bare-start = 0, supervisor = 1. Brittle if any
  future change re-arranges panes.

**Decision: D6.a.** Explicit storage is one new optional `u32`; the
serde-default keeps v0.4 sessions loading cleanly. The restart flow
reads `session.dashboard_pane.unwrap_or(0)` for v0.4-saved sessions
(which never had a supervisor anyway — so 0 is correct).

### D7. `git paw stop` after `git paw pause`

If the user runs `pause` and later runs `stop` (without restarting
in between), `cmd_stop` detects the paused state via the same
`effective_status` lookup. The kill-session path runs unchanged:
`tmux::kill_session` kills the tmux session, taking down every pane
including the still-running CLI panes. Session state updates to
`Stopped`.

The confirmation prompt (D5) fires in this path too — the prompt's
message text adapts to mention "this session is currently paused;
continuing will kill the running CLIs." Implementation detail:
`cmd_stop` reads the current effective status before prompting and
includes that in the prompt body.

Idempotent property preserved: `stop` (forced) → `stop` (forced)
runs through both invocations cleanly even though the session is
gone after the first.

### D8. Hibernate / per-CLI resume — out of scope

Two related v1.0.0 items live in the same neighbourhood:

- **Drift 61 — per-CLI session resumption on cold start.** When
  `git paw start` recovers from a *stopped* (not paused) state, it
  spawns fresh CLI processes. For Claude Code, claude-oss, and any
  other CLI that supports `--continue` / `--resume`, the recovery
  flow could detect prior sessions and pass the resume flag
  automatically. This is the same problem space as pause-vs-stop
  but solved at the cold-process level instead of the live-process
  level. v1.0.0 candidate under the Per-CLI Hook Providers theme.
- **Hibernate.** A future verb that snapshots the tmux session +
  every CLI's persistable state to disk, kills the processes
  (releasing RAM), and re-hydrates on next start. Distinct from
  pause (state in disk, not RAM) and distinct from stop (preserves
  conversation context, not just worktrees). v1.0.0 material.

Pause is the v0.5.0 minimum-viable freeze that solves the
short-break case end-to-end with the smallest possible change.
v1.0.0 hibernate + drift-61 cold-resume extend the design to the
long-break and process-death cases without rewriting pause.

## Risks / Trade-offs

- **[Risk] User pauses a long-running session, forgets, comes back a
  week later with 3-5 GB of RAM gone.** → **Mitigation:** `git paw
  status` shows the paused state distinctly with a "since" timestamp
  and a hint reminding the user the CLIs are still running. The
  `--help` long-about names this explicitly as the trade-off.
- **[Risk] tmux server crashes during pause.** The session goes from
  `Paused + alive` to `Paused + dead`, which `effective_status`
  downgrades to `Stopped`. Recovery on next start spawns fresh CLIs
  — same outcome as if the user had run `stop` initially. Acceptable
  fallback; the user pays the cold-restart cost for the rare crash
  case.
- **[Risk] Killing the dashboard pane only (D2) leaves a "ghost" pane
  in the layout that just shows the shell prompt after the dashboard
  process exits.** Mitigation: the `kill-pane` removes the pane
  entirely from the layout; tmux re-flows the remaining panes. On
  restart, the dashboard pane is re-created in its original index
  (D6) and the layout reflows again. The user briefly sees an
  N-1-pane layout if they re-attach during the pause — acceptable
  since the dashboard pane is decorative during the agent loop and
  the user has no reason to re-attach mid-pause.
- **[Trade-off] Pause leaves CLI processes vulnerable to OS-level
  signals the user didn't ask for (OOM killer, reboot, etc.).** Not
  a regression — `stop` already exposes the same vulnerability for
  worktrees, branches, and session state. Documented in the
  --help long-about: "pause is for short breaks; for longer breaks
  use stop and recover, or wait for hibernate (v1.0.0)."

## Migration Plan

Additive. Three back-compat surface notes:

1. **`git paw stop` (no flags) from a TTY now prompts.** Users in a
   TTY context who run `stop` without `--force` see the new
   confirmation. Non-TTY contexts (CI, scripts) skip the prompt.
   Release notes call out the change with a one-line "use `--force`
   to skip the prompt" hint.
2. **`SessionStatus::Paused` is a new serde variant.** Saved sessions
   from v0.4 (only `active` / `stopped`) load unchanged. Sessions
   saved by v0.5+ may include `paused`, which v0.4 binaries cannot
   parse — but downgrading the binary mid-session is not a supported
   scenario.
3. **`Session.dashboard_pane: Option<u32>` is a new optional field.**
   Defaults to `None` via serde, which the restart flow interprets
   as "v0.4-saved session, dashboard was at pane 0."

Rollback: revert the change. Pause-saved sessions become
unparseable on the v0.4 binary; users who pause + downgrade need to
manually delete the session file under
`~/.local/share/git-paw/sessions/`. Same risk profile as any other
new serde variant.

## Open Questions

- **Should `git paw pause` show a one-line "broker stopped, X CLI
  panes still running, run `git paw start` to resume" hint after
  successful pause?** Decision: yes. Mirrors the existing `stop`
  flow's print-on-success hint. Spec'd in the broker-lifecycle
  delta.
- **Should `git paw status` show RAM estimates for paused sessions?**
  Decision: not in scope. Estimating per-pane RAM requires
  per-process inspection (`/proc` on Linux, `ps` on macOS, neither
  portable). The trade-off framing in `--help` (D4) tells the user
  the rough order of magnitude; precise numbers are deferred to a
  future `git paw status --verbose` exercise.
- **Should pause auto-trigger after N minutes of no broker activity?**
  Decision: not in scope. Auto-pause is interesting future polish
  but invites questions about "what counts as activity" and "what
  if the user is actively using the panes but the broker is quiet."
  v0.6.0+ candidate.

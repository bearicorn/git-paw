---
name: concept-decoupling
description: Apply git-paw's target architecture — ports and adapters at the edges, a functional core (pure decisions) inside, Strategy for per-CLI and per-spec-format behavior, DTOs at serialized contracts — when writing new code or untangling a concern that has leaked across the codebase (tmux, headless mode, the broker host vs the dashboard view, agent CLIs, spec backends, MCP). Covers the layers and the inward dependency rule, finding the leaked model, domain-term ports, pulling decisions out of I/O, compile-time in-tree adapters (enum vs trait), thin clients, wire DTOs, seam-first refactor-then-delete, and scoping the ripple.
license: MIT
compatibility: git-paw
---

# Concept Decoupling

git-paw's code is moving toward one target structure. The canonical description is
`docs/src/architecture-direction.md` ("Target Architecture"); this skill is how to *apply* it —
to new code, and to a concern that has leaked across the codebase (tmux, headless mode, the
broker host vs the dashboard view, agent CLIs, spec backends, MCP).

Two references define the safety contract and the Rust-level mechanics; stay inside them:
- the `code-architecture` capability (`openspec/specs/code-architecture/spec.md`) — refactors are
  behavior-preserving: byte-identical CLI / config / wire surface, frozen serde / wire / lock /
  SIGHUP surfaces, test-gated waves.
- `code-standards` (`.agents/skills/code-standards/SKILL.md`) — the decided Rust patterns
  (`CommandRunner`, domain newtypes, module layout) and the do-not-touch list.

## The target in one screen

```text
 entry      who calls git-paw      CLI (clap), MCP tools, dashboard, broker HTTP server
   │          thin: parse → call a use case → render the result
   ▼
 app        use cases              start/stop/purge/add/remove … + the drive-loop shell
   │          observes and acts through ports, asks core what to do
   ▼
 core       the domain             agents, waves, surfaces, phases, escalations + pure decisions
              no I/O · no tmux · no axum / rmcp · no wire formats

 ports      traits the app needs   TerminalBackend, Vcs, BrokerClient, ProcessRunner, Clock
 adapters   implementations        tmux, git CLI, broker over HTTP, in-memory fakes
 wire       serialized formats     session file, config.toml, broker messages, MCP payloads
```

**The one rule: dependencies point inward** — `entry → app → core`. Adapters depend on the ports
they implement; only startup wiring depends on an adapter; `core` depends on nothing else in
git-paw. These are layers, not directories that exist today — modules move into them when next
touched.

Patterns, by slot:
- **Ports and adapters (hexagonal)** — the structure. The *driven* side (terminal, git, broker,
  process, clock) is the bulk of the work. The *driving* side (several callers sharing one use
  case) matters once the CLI and the MCP write tools both call the same commands.
- **Functional core, imperative shell (sans-IO)** — inside `app`/`core`: `observe → decide →
  apply`, with only `decide` pure.
- **Strategy + Null Object** — per agent CLI and per spec format, with a `generic` fallback.
- **DTO + mapper** — only at serialized contracts.

Rejected as ceremony or risk for this codebase: Clean/Onion layering beyond the above, runtime
plugins (an ABI promise before the v1.0 freeze), CQRS or event-sourcing as broker storage.

## 1. Find the leaked concern — map the model, not the call count

A concept is coupled when its *vocabulary* has leaked into the domain, not merely when many
files call its API.
- **Shallow:** callers invoke the subsystem's API. Usually already behind a seam.
- **Deep:** the tool's model is baked into unrelated modules and into persisted state. This is
  what blocks a swap.

Grep the model-leaking symbols across `src/` *and* `assets/scripts/` (the bundled shell helpers
read the same state). For tmux: `pane_current_path`, `pane_index`, hand-built
`"{session}:0.{n}"` targets, `send-keys` / `capture-pane` / `pipe-pane` spawned outside
`src/tmux/`, the persisted `pane_index` in the session file, pane-keyed claim files, and
"supervisor = pane 0" checks. Map every site and list it in the change's Impact section, so the
work is *scoped*, not *discovered*.

## 2. Ports speak git-paw's language, never the tool's

- ✗ `pane_current_path(pane)` — a tmux-shaped hole only tmux fits.
  ✓ *"which agent owns this surface?"* — tmux answers with `pane_current_path`, a PTY backend
  knows by construction, headless never asks.
- ✗ `pipe_pane(target)` → ✓ *"stream this agent's output to a log"*.
- Identify a surface by an opaque `SurfaceId`, never a pane index.

If a method only makes sense for one implementation, it is still leaking — rephrase it as the
need behind it.

## 3. Pull decisions out of the I/O — the functional core

Most of git-paw's hard logic (approve? nudge? escalate? merged? exited cleanly?) is currently
interleaved with the calls that capture panes and send keys. Separate them:

```rust
// core: pure
pub fn decide_approval(tail: &str, profile: &CliProfile, policy: &Policy) -> ApprovalDecision;

// app: shell
let tail = terminal.capture_tail(surface)?;
match core::decide_approval(&tail, profile_for(agent), &policy) {
    ApprovalDecision::Approve { option } => terminal.send_key(surface, option)?,
    ApprovalDecision::Escalate(reason)   => broker.publish(escalation(agent, reason))?,
    ApprovalDecision::CliUnavailable(l)  => return Ok(Outcome::CliUnavailable(l)),
    ApprovalDecision::NotLive            => {}
}
```

- Inputs are *observed facts* passed in (the captured tail, the refs, the roster) — never read
  inside the decision. Example: an exit summary must take "is this branch an ancestor of the
  default branch" as an input, not infer "merged" from loop state.
- Test the decision with data: fixture in, decision out, no fakes.
- Effects can be returned as data (`Vec<Effect>`) when a step has several; that also gives a
  dry-run and an audit trail for free.
- Migrate one decision at a time, starting with the nearly pure ones. Precedents: the
  live-prompt gate in `supervisor::permission_prompt`, and the dashboard's MVU `update`.

## 4. Adapters — compile-time, in-tree, zero new dependencies

- Adapters are trait implementations compiled into the binary and selected at startup; each
  shells out the way git-paw already does. No runtime plugin loading.
- The port's real I/O goes through `CommandRunner` (`src/command_runner.rs`); provide one
  in-memory fake per port, so logic is tested once and each real adapter is only smoke-tested.
- **Enum or trait?** A small, closed set where callers must handle each case → `enum` (the
  compiler checks exhaustiveness; e.g. `BrokerMessage`). An open set where callers must not care
  which one they hold → trait (e.g. terminal backends). An enum for an open set re-creates the
  coupling (`match backend { Tmux | Pty | Headless }` in every caller).

## 5. Thin clients — split a host from its view

When one process both hosts state and renders a view, separate them. Precedent: the MCP query
layer (`src/mcp/query/`) holds no `BrokerState` — it resolves `broker_url` from the session file
and talks HTTP. Counter-example: the dashboard command hosts the broker, so killing the view
kills the broker. The target is a broker in its own host process with the dashboard as one more
HTTP client. Name the ownership question every split surfaces (here: which process owns the
broker, and does it also host the drive loop when unattended?).

## 6. Per-CLI and per-format behavior — Strategy, resolved once

- One implementation per agent CLI or spec format behind one trait, plus a `generic` fallback
  (Null Object) for anything unrecognized.
- Resolve the profile **once per agent** and pass it to the decisions that need it. Never call a
  specific CLI's profile directly from a decision — that silently ignores per-CLI config for every
  other CLI.
- Keep one representation of each concept (one spec-format type, not a CLI enum plus a string
  plus a kind enum); dispatch on the type, not on raw strings — a string match escapes the
  compiler's exhaustiveness check.

## 7. Wire edges — DTOs only where a format is a contract

A format is a contract when another process, a bundled script, or a future release reads it:
the session files, `config.toml` (scripts parse its keys), broker messages (scripts hand-build
the JSON), MCP payloads, logs that are parsed back.
- Give each contract its own wire type plus a mapper to and from `core`, so the internal model can
  change without changing the format. Add a version field. Keep it additive:
  `#[serde(default)]` + `skip_serializing_if`, old files load unchanged.
- Share one definition between the server and its clients instead of each client re-declaring the
  response shape.
- Do not DTO-ify structs that never leave the process — that is ceremony.

## 8. Seam-first — refactor, then delete

1. Introduce the port / type / role and route the existing code through it — byte-identical,
   whole suite green.
2. The compiler now lists every consumer; fix each.
3. Only then add the second implementation, flip the default, or delete the old path.

Precedents: `WorkerPhase`, the dashboard MVU restructure, the `CommandRunner` seam. For example, a
role that is half-typed (an enum decided by "pane 0", then flattened to a string and re-compared)
must become a type end to end *before* the thing it describes can be removed.

## 9. Scope the ripple

- **State:** a leaked persisted field (e.g. `pane_index` in the session file) is a back-compat
  obligation — keep it additive and loadable.
- **Scripts:** `assets/scripts/` read the same files and HTTP shapes; change them together and
  keep the parity tests.
- **Tests:** a fake per port replaces per-call fakes.
- **Exhaustive matches and struct literals:** follow the AGENTS.md enum-variant ripple checklist —
  and treat *struct-field* additions the same way (a new required field breaks every literal on a
  concurrent branch; prefer `..Default::default()` in test literals).

## Guardrails

- **Behavior-preserving, or it is not this skill.** A decoupling that changes behavior is a
  feature and needs its own spec.
- **No new leaks.** New code does not add tmux calls outside the terminal adapter, pane-index or
  "pane 0" logic, direct calls to one CLI's profile, or raw-string dispatch on a closed set.
- **Domain-term ports only**; reject interfaces named after tmux / axum / rmcp / a CLI's verbs.
- **Compile-time, in-tree, zero-new-dependency adapters.**
- **Spec it and wave it.** Scope the ripple in the proposal's Impact, gate each wave on the full
  suite, and keep `docs/src/architecture-direction.md` in step when the target itself changes.

## Checklist for a change

- [ ] Decisions are pure functions over observed inputs; I/O stays in the shell.
- [ ] New external access goes through a port named in git-paw's terms.
- [ ] No new tmux / pane-index / "pane 0" / single-CLI coupling.
- [ ] Per-CLI or per-format behavior is a Strategy resolved once, with a generic fallback.
- [ ] Serialized contracts stay additive; new contracts get a wire type and a version.
- [ ] Ripple (state, scripts, matches, struct literals) is listed in Impact.
- [ ] Behavior is unchanged where the change claims to be a refactor.

<!-- summary: The target structure git-paw's code is moving toward — ports and adapters at the edges, pure decisions in the middle — and the rules new code follows. -->
# Target Architecture

The [Architecture](architecture.md) chapter describes the code as it is today. This page
describes the structure the code is **moving toward**, and the rules new code follows so that it
moves in that direction rather than away from it.

The move is incremental. Modules are restructured when they are next changed, not in one large
rewrite, and every step keeps git-paw's behavior exactly the same.

## Why

git-paw coordinates several outside systems: a terminal multiplexer, git, agent CLIs, an HTTP
broker, and MCP clients. Today some of them leak into code that should not need to know about
them — tmux pane numbers appear in session state and in the supervisor's decision logic, and the
drive loop assumes one particular CLI's screen layout in places. That makes each of them hard to
replace (for example, tmux by another terminal backend or a headless mode), hard to test without
faking processes, and risky to freeze as a stable contract.

## The pattern

Two well-known patterns, used together:

- **Ports and adapters** (also called *hexagonal architecture*) for the overall structure.
  Everything git-paw talks to sits behind a *port* — a Rust trait named in git-paw's own terms —
  and is reached only through an *adapter* that implements that trait.
- **Functional core, imperative shell** (in Rust also called *sans-IO*) for the logic inside.
  Code that *decides* — whether to approve a prompt, whether to nudge an agent, whether a branch
  was merged — is a pure function: data in, data out. Separate code observes the world and
  carries out the decisions.

Two smaller patterns fill specific slots: **Strategy** for behavior that differs per agent CLI or
per spec format, and **data-transfer objects (DTOs)** wherever data is written to a file or sent
over the network.

## Layers

```text
 entry      who calls git-paw      CLI (clap), MCP tools, dashboard, broker HTTP server
   │          thin: parse input → call a use case → show the result
   ▼
 app        use cases              start, stop, purge, add, remove, … and the drive-loop shell
   │          orchestrates: observes and acts through ports, asks core what to do
   ▼
 core       the domain             agents, waves, surfaces, phases, escalations + pure decisions
              no I/O · no tmux · no HTTP or MCP libraries · no file or wire formats

 ports      traits the app needs   terminal backend, version control, broker client, clock
 adapters   implementations        tmux, the git command line, the broker over HTTP, test fakes
 wire       serialized formats     session file, config file, broker messages, MCP payloads
```

These are layers, not directories that exist today.

### The dependency rule

Dependencies point inward: **entry → app → core**. Adapters depend on the ports they implement,
and nothing depends on an adapter except the startup code that wires the program together.
`core` depends on nothing else in git-paw. If code in `core` would need tmux, an HTTP framework,
the MCP library, or a file format, it belongs in another layer.

## Ports

| Port | What it means in git-paw's terms | Adapters |
|---|---|---|
| Terminal backend | Create and remove an agent's *surface*; send it text or a key; read its recent output; list the surfaces and the worktree each one is in; stream its output to a log; check it is alive; let a person attach | tmux today; a native terminal backend or a headless mode could be added later |
| Version control | Create and remove worktrees, read branches and refs, ask whether a branch has been merged | the `git` command line |
| Broker client | Publish a message; read agent status and the message log | HTTP to the broker |
| Process runner | Run an external command | `CommandRunner` (already exists) |
| Clock | The current time | the system clock; a fixed clock in tests |

Ports use git-paw's vocabulary, never a tool's. A terminal method named after tmux's
`pane_current_path` only fits tmux. The question git-paw actually needs answered is *"which agent
owns this surface?"* — tmux answers it with `pane_current_path`, a native terminal backend knows
it because it created the surface, and a headless mode never needs to ask. Surfaces are identified
by an opaque id rather than a tmux pane number.

Adapters are compiled into the binary and chosen at startup, not loaded as runtime plugins. Each
one runs its tool the way git-paw already does, so no new dependencies are needed.

## Example: approving a permission prompt

The names below are illustrative.

```rust
// core: pure — plain data in, plain data out
pub enum ApprovalDecision { NotLive, Approve { option: u8 }, Escalate(Reason), CliUnavailable(String) }

pub fn decide_approval(tail: &str, profile: &CliProfile, policy: &Policy) -> ApprovalDecision { … }

// app: the shell — the only part that does I/O, and only through ports
let tail = terminal.capture_tail(surface)?;
match core::decide_approval(&tail, profile_for(agent), &policy) {
    Approve { option } => terminal.send_key(surface, option)?,
    Escalate(reason)   => broker.publish(escalation(agent, reason))?,
    CliUnavailable(l)  => return Ok(Outcome::CliUnavailable(l)),
    NotLive            => {}
}
```

The decision is tested by passing in a captured screen and checking the result — no fake
terminal and no fake clock. Supporting a different terminal means writing a new adapter; `core`
and `app` do not change.

## Behavior that differs per CLI or per spec format

Agent CLIs differ in how their screens look and which flags they accept; spec formats differ in
where specs live and how tasks are tracked. Each is a **Strategy**: one implementation per CLI or
format behind a single trait, with a generic implementation as the fallback for anything
unrecognized. The profile for an agent's CLI is worked out once for that agent and passed to the
decisions that need it, instead of each decision assuming one particular CLI.

Use a Rust `enum` when the set is small and fixed and callers really do need to handle each case
differently — the compiler then checks that every case is handled. Use a trait when the set can
grow and callers should not need to know which implementation they hold; terminal backends are
the clearest example.

## Serialized formats

The session file, the config file, broker messages and MCP responses are read by other processes
and by the bundled helper scripts, so they are contracts that must stay stable. The target keeps
each one as a separate **wire type** (a DTO) with a conversion to and from the `core` types, so
the internal model can change without changing the format. Each format carries a version field
and stays backward-compatible: new fields are optional, and older files keep loading unchanged.

This split applies only where a format is a contract, not to every struct in the program.

## Rules for new code

- Put each decision in a pure function that takes data and returns data. Keep I/O in the shell.
- Reach tmux, git, the broker and the filesystem only through their port. Do not add direct
  `tmux` calls outside the terminal adapter.
- Name ports and core types in git-paw's terms (agent, surface, wave), never a tool's.
- Identify an agent's terminal by its surface, not by a pane number. Do not add logic that
  depends on "pane 0".
- Do not hardcode one agent CLI's behavior where a per-CLI profile exists.
- Keep serialized formats additive and backward-compatible.
- Restructure in small, safe steps: first add the port or type and route the existing code
  through it with no change in behavior, then let the compiler find every call site, and only
  then add a second implementation or remove the old code.

## Where the code is today

| Area | Status |
|---|---|
| Live-prompt gate (`supervisor::permission_prompt`) | Already a pure decision |
| Dashboard | Already Model-View-Update — a functional core for a terminal UI |
| Process execution (`CommandRunner`) | Already a port with a test fake; not yet used everywhere |
| MCP server | Already layered one way (`query` → `tools` → `server`) and talks to the broker over HTTP |
| Terminal | tmux calls are spread across several modules; the terminal port is the main open item. Reserved pane indices no longer appear in decision code: they live in `surface::SessionLayout`, and each surface's role is the closed `surface::SurfaceRole` enum a single resolver produces. Pane numbers still appear in session state and at the adapter edge |
| Broker | Runs inside the dashboard process; moving it into its own process so the dashboard becomes a thin client is planned |
| Drive loop | Its side effects are already behind traits; its decisions are not yet separated from them |
| Commands (`start`, `stop`, `purge`, …) | Not yet callable as a library: they read the current directory and print directly |
| Serialized formats | The session file, config file and broker messages use the same types as the internal model |

## Further reading

- *Hexagonal Architecture* (ports and adapters), Alistair Cockburn.
- *Functional Core, Imperative Shell*, Gary Bernhardt.

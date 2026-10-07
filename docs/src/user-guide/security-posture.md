# Security Posture

This is the single, authoritative statement of git-paw's trust model. Read it
before trusting git-paw with autonomous execution. It is deliberately blunt about
what the tool does and does **not** protect.

## What git-paw runs

git-paw launches AI coding agents — arbitrary programs — in tmux panes on your
machine, each in its own git worktree. Under supervisor/unattended mode (and
whenever you point an agent CLI at a skip-permissions or full-auto flag), those
agents **execute arbitrary code with your user's privileges**: they run your
build, your tests, shell commands, and anything else the model decides to run,
often without pausing for per-command approval.

**The blast radius is your user account.** An agent — whether malicious, prompt-
injected, or simply mistaken — can by default read and write anything your user
can: your whole home directory, your SSH keys, your cloud credentials, your other
repositories, and your login keychain. git-paw does not change your OS's
permission model; it automates running code within it.

## What the controls do — and do not — guarantee

git-paw ships several controls. None is a complete sandbox on its own; each
narrows one specific risk. Do not read any of them as "the agent is now safe."

| Control | What it does | What it does **not** do |
|---|---|---|
| **Auto-approve classifier** | Auto-approves command shapes it recognizes as safe (worktree-confined writes, known read-only verbs); escalates the rest to a human. | It is **heuristic pattern-matching**, not a security boundary. It can misclassify, and it is trivially bypassed by a command shape it hasn't seen. It reduces prompt fatigue; it does not confine anything. |
| **Command allowlists** | Grant specific command prefixes by path/verb so routine dev commands don't prompt. | A grant is a convenience, not containment — an allowed prefix can still do anything that prefix permits. Path-scope your grants; never grant `curl *` / `cd *`. |
| **Protected-paths rule** (agent-memory isolation) | *Flags* writes that resolve outside the agent's worktree (e.g. into another agent's memory). | It only **warns/gates** at the classifier layer; it is not kernel-enforced and does not physically prevent the write. |
| **`.git/`-write rule** | *Flags* any write/edit/create targeting a path inside a repository's `.git/` directory (config, hooks, `info/exclude`) as a danger-class escalation, never auto-approved — even when the verb is otherwise whitelisted. | Same classifier-layer caveat as above: heuristic, not kernel-enforced. See [Configuration reference](../configuration/README.md#auto-approve-safe-permission-prompts). |
| **FS-scoped sandbox** (optional, you configure it) | When you wrap an agent CLI in `sandbox-exec`/`bwrap`, the **kernel** confines the agent's *writes* to its worktree — `rm -rf ~`, dotfile corruption, and planted git hooks are blocked at the syscall level. | It protects the machine **around** the work area, not the work area (the worktree, including uncommitted work, stays destroyable by design). The default profile does **not** restrict *reads*, so host secrets remain readable unless you add read-hardening. It does not cover network, resource-exhaustion, or exposed-socket vectors unless you add rules. See [Sandbox](./sandbox.md). |
| **Broker Host/Origin guard** | Refuses browser-shaped requests (a non-loopback or `null` `Origin`) and, on a loopback bind, requests whose `Host` is not loopback — closing the DNS-rebinding path into the unauthenticated coordination bus. See [Browser exposure](#browser-exposure--the-brokers-host-and-origin-guard). | It is **not authentication**: any local process can still reach the broker, read the whole log, and publish any message. A non-loopback `[broker] bind` disables the `Host` half entirely. |
| **Remote control** (documented workflow) | Lets you drive a session from elsewhere via each tool's own remote-control. | Exposing a session to remote control extends the blast radius to whoever reaches that channel; secure the channel yourself. |

## Browser exposure — the broker's Host and Origin guard

The coordination broker is an HTTP server on your loopback interface
(`127.0.0.1:9119` by default) and it **authenticates nothing**: any client that
reaches the port can read `GET /log` — the whole session's coordination traffic
— and `POST /publish` any message, including an `agent.answer` or
`agent.feedback` that coding agents read as authoritative guidance. Because
those agents run with auto-approved execution, a request that reaches the
broker is a prompt-injection channel into a shell.

A web page you open in a browser can reach a loopback port through
**DNS rebinding**: the attacker's hostname re-resolves to `127.0.0.1`, so the page
becomes "same-origin" with `http://<attacker-host>:9119` and its requests go to
your broker. To block that, the broker checks two headers on **every** request,
before routing — so the check covers every endpoint, including the `404` and
`405` fallbacks and any endpoint added in a later release:

| Check | When it applies | What it accepts |
|---|---|---|
| **`Host`** | Only when `[broker] bind` is a loopback address (the default). | `localhost` (case-insensitive), any IPv4 literal in `127.0.0.0/8`, or `[::1]`. The **port is not checked**, so `ssh -L`-style tunnels and local port-forwards keep working. A missing `Host` is refused. |
| **`Origin`** | Whenever the header is present, on **any** bind. | An origin whose host passes the `Host` rule. The serialized opaque origin `null` (sandboxed iframes, `file://` pages) is refused. A request with **no** `Origin`, which is what every non-browser client sends, is unaffected. |

A refused request gets `403` with `{"error":"forbidden host"}` or
`{"error":"forbidden origin"}`, is not routed, and changes no broker state.

**No git-paw client is affected.** The bundled `broker.sh` and `sweep.sh`
helpers, the post-commit git hook, the Rust HTTP clients used by the CLI, the
MCP server and the dashboard all address the broker by its configured
`http://<bind>:<port>` URL and send no `Origin`.

### The non-loopback exception

If you set `[broker] bind` to `0.0.0.0` or a LAN address, git-paw cannot
enumerate the names your network uses for the machine, so **the `Host` check is
disabled**. Two things follow, and the second is easy to get wrong.

First, the broker is **reachable from your network by any host, with no
authentication at all** — a far larger exposure than the one this guard
addresses.

Second, **a non-loopback bind does not keep browsers out.** The `Origin` check
still blocks browser *writes* and *cross-origin reads*: browsers attach
`Origin` to every `POST`, and to any cross-origin `GET` made in CORS mode
(`fetch` defaults to it, as does `XMLHttpRequest`). A cross-origin `GET` made
in `no-cors` mode sends no `Origin`, but the browser hands the page an opaque
response it cannot read, so that is not a way out either.

A DNS-rebinding page, though, is **same-origin** with the broker once the
attacker's hostname re-resolves to your machine. A same-origin `GET` carries
**no `Origin` header at all**, and its response is fully readable — so with the
`Host` check off, `fetch('/log')` from that page is served and the page reads
it. The same goes for `/status` and `/messages/*`. **Only a loopback bind stops
a rebinding page from reading the coordination log.**

Every command that brings the broker up prints a warning on stderr naming the
bind address: `git paw start` (including `--from-specs`), the supervisor
launch, a recovery of a stopped session, and a resume from pause. The broker
process prints it too, but into the dashboard pane, which switches to the
alternate screen moments later. Do not expose the broker this way on a network
you do not control.

### This is not authentication

The guard distinguishes *browsers and foreign host names* from *local
processes*. It does not authenticate anybody. **Any process running on your
machine can still reach the broker**, publish any message, and read the whole
log — including an agent that has gone off the rails, and any other software
running as your user.

That includes pages: **any page served from a loopback origin is accepted** —
`http://localhost:3000`, or any other port — so a local development server you
are running, or a cross-site-scripting flaw in a page it serves, can reach the
broker from the browser on both a loopback and a non-loopback bind. The guard
excludes *foreign* origins, not *all* browser code.

If you need the broker to be unreachable by local processes, that is a
containment problem; see the [FS-scoped sandbox](./sandbox.md), and do not rely
on this guard for it.

### Migration: reverse proxies

A reverse proxy that forwards the original external `Host` to the broker gets
`403`. Configure it to rewrite `Host` to a loopback name (for nginx,
`proxy_set_header Host 127.0.0.1:9119;`), or have clients address the broker
directly. Tunnels and port-forwards need no change — they already present a
loopback `Host`.

## The bottom line

- Treat every agent as capable of running arbitrary code as you.
- The classifier, allowlists, and protected-paths reduce *friction and mistakes*;
  they are not containment.
- For containment, use the optional [FS-scoped sandbox](./sandbox.md) — and
  understand precisely what it does (writes) and does not (reads, by default)
  protect.
- Run untrusted or aggressive automation on hardware, accounts, and repositories
  whose compromise you could tolerate.

`git paw doctor` prints a pointer to this page and reports whether your OS's
sandbox backend is available.

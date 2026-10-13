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

## git-paw decides nothing about your agent's commands

**git-paw makes no safety judgement about a command an agent asks to run, and it
approves no permission prompt on its own.** It has no safe-command list, no
danger list, and no path-based safety rule. There is no command classifier.

Whether a command runs without a prompt is decided entirely by **the agent CLI's
own permission system** — the permission mode git-paw launches it with, plus
whatever allow and deny rules you have configured in that CLI. Anything the CLI
decides to ask about is a prompt, and every prompt is answered by a human.

What git-paw does is narrower, and it is the part it can do honestly: it
*detects* that a pane is sitting on a live permission prompt and gets that prompt
in front of you — on the broker, in the orchestrator pane, in the exit summary
and in the manual-decision log — without stalling the rest of the wave. Under
`git paw start --unattended` every live prompt is escalated this way. Approval
keystrokes reach a pane only when a human explicitly runs
`sweep.sh approve <pane>`, which answers once with option 1 and refuses an
option 1 that would grant a durable "don't ask again" permission.

> **Earlier versions did decide.** Up to and including v0.16.0, git-paw ran a
> command classifier that answered prompts it judged safe, often choosing the
> "Yes, and don't ask again" option. Your agent CLI persisted those choices as
> **durable permission grants in its own settings file** — for Claude Code that
> is the repository-root `.claude/settings.local.json`, which every worktree and
> every one of your own sessions shares. Retiring the classifier stops new grants
> being created; it removes none of the existing ones. **Review that file (and
> any user-level equivalent) and delete grants you did not intend.** git-paw
> never edits or deletes it.

## What the controls do — and do not — guarantee

git-paw's controls are the ones below. None is a complete sandbox on its own;
each narrows one specific risk. Do not read any of them as "the agent is now
safe."

| Control | What it does | What it does **not** do |
|---|---|---|
| **The agent CLI's permission system** | The `[supervisor] approval` / `agent_approval` level maps to that CLI's own launch flags (`manual`, `auto`, `full-auto`), and the CLI then applies its own allow and deny rules to decide what runs without asking. This is the only thing deciding whether a command needs approval. | It is **the CLI's** guarantee, not git-paw's, and it is only as narrow as the rules you wrote: an allow rule for a command prefix permits everything that prefix permits. A `full-auto` level **skips the prompts entirely**, so no rule is consulted and nothing is escalated. Path-scope your rules; never grant `curl *` / `cd *`. |
| **FS-scoped sandbox** (optional, you configure it) | When you wrap an agent CLI in `sandbox-exec`/`bwrap`, the **kernel** confines the agent's *writes* to its worktree — `rm -rf ~`, dotfile corruption, and planted git hooks are blocked at the syscall level. This is the only control here that is kernel-enforced. | It protects the machine **around** the work area, not the work area (the worktree, including uncommitted work, stays destroyable by design). The default profile does **not** restrict *reads*, so it protects write-integrity and **not the confidentiality** of host secrets unless you add read-hardening. It does not cover network, resource-exhaustion, or exposed-socket vectors unless you add rules. See [Sandbox](./sandbox.md). |
| **Broker Host/Origin guard** | Refuses browser-shaped requests (a non-loopback or `null` `Origin`) and, on a loopback bind, requests whose `Host` is not loopback — closing the DNS-rebinding path into the unauthenticated coordination bus. See [Browser exposure](#browser-exposure--the-brokers-host-and-origin-guard). | It is **not authentication**: any local process can still reach the broker, read the whole log, and publish any message. A non-loopback `[broker] bind` disables the `Host` half entirely. |
| **Remote control** (documented workflow) | Lets you drive a session from elsewhere via each tool's own remote-control. | Exposing a session to remote control extends the blast radius to whoever reaches that channel; secure the channel yourself. |

Two things that used to appear in this table are gone, and should not be
relied on: git-paw's **auto-approve classifier** and its **command allowlist**
(`[supervisor.auto_approve]`, `[supervisor.common_dev_allowlist]`) are retired —
both tables still parse so old configs load, but they have no effect and warn.
The **protected-paths** and **`.git/`-write** rules existed only to stop that
classifier approving such a write; with nothing approving automatically, those
prompts reach you like any other. To enforce those boundaries mechanically, use
deny rules in the agent CLI's own settings, or the sandbox.

## Browser exposure — the broker's Host and Origin guard

The coordination broker is an HTTP server on your loopback interface
(`127.0.0.1:9119` by default) and it **authenticates nothing**: any client that
reaches the port can read `GET /log` — the whole session's coordination traffic
— and `POST /publish` any message, including an `agent.answer` or
`agent.feedback` that coding agents read as authoritative guidance. Whatever
those agents are allowed to run without asking — under a permissive CLI
permission mode, or `full-auto` — a request that reaches the broker is a
prompt-injection channel into a shell.

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
- git-paw decides nothing about an agent's commands and approves no prompt. What
  runs without asking is your agent CLI's decision, from the rules you gave it.
- A CLI allow rule reduces *friction*; it is not containment. A `full-auto`
  level removes the prompts altogether.
- For containment, use the optional [FS-scoped sandbox](./sandbox.md) — and
  understand precisely what it does (writes) and does not (reads, by default)
  protect.
- If you ran v0.16.0 or earlier, audit your agent CLI's settings file for
  durable "don't ask again" grants the retired classifier accepted on your
  behalf.
- Run untrusted or aggressive automation on hardware, accounts, and repositories
  whose compromise you could tolerate.

`git paw doctor` prints a pointer to this page and reports whether your OS's
sandbox backend is available.

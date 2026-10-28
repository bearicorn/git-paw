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
in front of you — on the broker, in the exit summary and in the manual-decision
log — without stalling the rest of the wave. Under
`git paw start --unattended` every live prompt is escalated this way. Approval
keystrokes reach a pane only when a human explicitly runs
`sweep.sh approve <pane>`, which answers once with option 1 and refuses an
option 1 that would grant a durable "don't ask again" permission.

**No LLM process runs at the repository root inside a session.** A supervised
session holds only the dashboard and the coding agents, each confined to its own
worktree; the orchestrator runs in your own agent session, under the permission
mode you chose for it, and git-paw launches it with no flags. Every git-paw
keystroke sender — `sweep.sh approve`, and the drive loop's nudges, stale-input
recovery and buffered-input submits — follows one role-based rule: keystrokes
reach only a pane resolved as a coding agent (by the session's discovery roster
for the helper, by the recorded layout and the pane's owning worktree for the
loop). The dashboard and any pane no agent worktree owns are refused, whatever
their index.

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
| **The agent CLI's permission system** | The `[supervisor] agent_approval` level maps to each coding agent's own launch flags (`manual`, `auto`, `full-auto`), and the CLI then applies its own allow and deny rules to decide what runs without asking. This is the only thing deciding whether a command needs approval. | It is **the CLI's** guarantee, not git-paw's, and it is only as narrow as the rules you wrote: an allow rule for a command prefix permits everything that prefix permits. A `full-auto` level **skips the prompts entirely**, so no rule is consulted and nothing is escalated. Path-scope your rules; never grant `curl *` / `cd *`. |
| **FS-scoped sandbox** (optional, you configure it) | When you wrap an agent CLI in `sandbox-exec`/`bwrap`, the **kernel** confines the agent's *writes* to its worktree — `rm -rf ~`, dotfile corruption, and planted git hooks are blocked at the syscall level. This is the only control here that is kernel-enforced. | It protects the machine **around** the work area, not the work area (the worktree, including uncommitted work, stays destroyable by design). The default profile does **not** restrict *reads*, so it protects write-integrity and **not the confidentiality** of host secrets unless you add read-hardening. It does not cover network, resource-exhaustion, or exposed-socket vectors unless you add rules. See [Sandbox](./sandbox.md). |
| **Broker Host/Origin guard** | Refuses browser-shaped requests (a non-loopback or `null` `Origin`) and, on a loopback bind, requests whose `Host` is not loopback — closing the DNS-rebinding path into the unauthenticated coordination bus. See [Browser exposure](#browser-exposure--the-brokers-host-and-origin-guard). | It is **not authentication**: any local process can still reach the broker, read the whole log, and publish any message. A non-loopback `[broker] bind` disables the `Host` half entirely. |
| **Remote control** ([documented workflow](./remote-control.md)) | Lets you drive a session from elsewhere by remote-controlling your own driving agent session through its CLI's native remote control. | Exposing a session to remote control extends the blast radius to whoever reaches that channel; secure the channel yourself. |

Two things that used to appear in this table are gone, and should not be
relied on: git-paw's **auto-approve classifier** and its **command allowlist**
(`[supervisor.auto_approve]`, `[supervisor.common_dev_allowlist]`) are retired —
both tables still parse so old configs load, but they have no effect and warn.
The **protected-paths** and **`.git/`-write** rules existed only to stop that
classifier approving such a write; with nothing approving automatically, those
prompts reach you like any other. To enforce those boundaries mechanically, use
deny rules in the agent CLI's own settings, or the sandbox.

## Writing into an agent CLI's settings

git-paw writes a permission rule into an agent CLI's own settings in exactly
one circumstance: you ran [`git paw setup-cli`](cli-setup.md). Running it is
the consent. No other command, process or interface does it — not `start`,
`add`, `init`, session recovery, the drive loop, the dashboard, or any tool
exposed over MCP.

**What it may write.** For `claude`, one file:
`<main checkout>/.claude/settings.local.json`, plus one anchored line in the
repository's shared `info/exclude` when the target is not already ignored.
Never a file inside a worktree, never a user-level file, never the configured
`[clis.<name>].settings_path`.

**The grant.** Twelve allow entries naming git-paw's own helper scripts by
relative path and fixed subcommand, from a compiled table. Nothing is derived
from a command argument, your configuration or your repository content. No
entry names `curl`, an interpreter, an absolute path or a directory wildcard.
The keystroke-sending and verdict-publishing subcommands are deliberately
excluded, and `sweep.sh approve` is pinned to an **ask** rule so it always
reaches a human. The twelfth entry, `sweep.sh events`, is a pure read of the
broker's log and status — no keystroke, no verdict. See the full table in
[Per-CLI Setup](cli-setup.md).

**An applied grant is never rewritten for you.** When a release adds an entry
to the table, a file applied under an earlier version keeps the entries that
version wrote; git-paw changes nothing behind your back. `git paw doctor`
reports the applied setup as out of date, and the remedy is explicit and
yours: `git paw setup-cli claude --undo`, then
`git paw setup-cli claude --allowlist`. Until then the new subcommand prompts
like any ungranted command.

**The tamper threat, and the mitigation.** A granted helper runs without a
prompt, and it lives inside the agent's own worktree. In an edit-accepting
mode the CLI would apply an agent's edit to that script without asking, after
which the grant runs whatever the agent wrote — arbitrary code, unprompted. A
quieter variant: the helpers read `.git-paw/config.toml`, so an edited broker
address turns a granted publish into exfiltration.

So the allowlist item never writes the grants alone. In the same file and the
same replacement it writes `Edit(**/.git-paw/scripts/**)` to `deny` and
`Edit(**/.git-paw/config.toml)` to `ask`. Both were confirmed against the
installed CLI before the item shipped — including that the deny rule also
blocks the filesystem commands the edit-accepting mode auto-approves
(`sed -i`, `cp`, `mv`, `rm`, `touch`, and `cd`-prefixed forms), and that it
covers both the Edit and the Write tool, at any depth, from a worker's
directory and from a root session reaching into a worker's copy.

**The gate.** An item is written only when its facts have been **verified**
against an installed copy of that CLI. An unverified item can be planned with
`--dry-run` but never written. The `claude` allowlist item ships verified
against Claude Code 2.1.220 (2026-10-15); `git paw doctor` reports the
current status.

**Residual risks.** These remain even with the item applied:

- An agent CLI other than the one with a fact row has no row, so nothing is
  written for it — and nothing is pre-approved.
- A separately installed MCP server or other tool that writes files outside
  the CLI's permission system is not governed by these rules.
- In `bypassPermissions` / `full-auto`, the grant adds no privilege because
  nothing prompts — but the protection rules buy nothing either. Do not pair
  the allowlist with those modes outside the [sandbox](sandbox.md).
- In the CLI's own `auto` mode, its classifier decides edits.
- `broker.sh --agent <other>` can publish under another agent's id. That is
  pre-existing; broker-side identity is a later release. Verdict publishing
  is deliberately not granted.
- A write by the CLI itself (a "don't ask again" approval) landing between
  git-paw's re-read and its rename would be lost. git-paw detects the change
  and stops rather than clobbering it, but prefer to run setup when no
  session is waiting on an approval.

**A grant is not a safety judgement.** git-paw decides nothing about your
prompts. Whether a call runs without one is decided entirely by the agent
CLI's own permission system and the rules in its settings. `git paw doctor`
additionally lists the broad allow rules already in that file, so you can see
what earlier "don't ask again" answers left behind — it reports them and never
edits them.

### Self-approval residue

One shape of leftover grant deserves naming on its own, because it undoes the
single most important distinction the grant table draws.

The allowlist item grants `sweep.sh` **per subcommand**, and withholds
`approve`, `verified`, `feedback-gate` and `conflict-ack` precisely because
the target settings file reaches every session in the repository — a worker's
as much as the orchestrator's. A durable allow rule naming one of those
subcommands, or a whole-script `Bash(.git-paw/scripts/sweep.sh *)` wildcard
that covers them all, hands every agent session exactly what was withheld: it
can press the approval keystroke in another pane, or publish a supervisor
verdict, **without a prompt**. An agent can then approve its own permission
prompts and sign off its own work.

Such a rule is not something git-paw writes. It arrives from a "don't ask
again" answer — including ones the retired classifier accepted on your behalf
up to v0.16.0 — or from a broad rule added by hand. `git paw doctor` now
reports it, in the `project CLI grant residue` check, alongside a blanket
`curl` grant and the inert `allowed_bash_prefixes` seed earlier versions
wrote. See [Doctor](doctor.md).

Removing it is **manual by design**. git-paw never rewrites or deletes a grant
it did not write: it cannot prove the rule is not one you meant to add, and a
tool that edits your permission rules on its own is the thing this whole
section exists to rule out. Doctor names the file and the entry; the deletion
is yours.

### Hook bypass and tmux driving

Two things an agent session can do with ordinary commands step around
git-paw's own safeguards, and both have been seen in practice:

- **Bypassing git-paw's git hooks.** git-paw's hooks guard each agent's branch
  and report its commits. A per-invocation override such as
  `git -c core.hooksPath=…`, or `git commit --no-verify` (`-n`), silently
  skips them — the branch guard and the commit report go with it.
- **Driving tmux.** `tmux send-keys`, `tmux run-shell` and `tmux pipe-pane`
  reach into the session's panes from outside the agent's CLI. Keystrokes sent
  that way can answer a permission prompt — the agent's own included — and so
  bypass the human and the orchestrator entirely.

The bundled coordination skill tells every agent plainly not to do either. An
instruction is not a control, though, and git-paw adds no permission rule for
these commands: it pre-approves only its own helper workflow (`setup-cli`
writes rules naming git-paw's helper scripts and nothing else), and every other
permission decision belongs to your agent CLI and to you. That is the same
line the v0.17.0 classifier retirement drew.

If you want these forms to prompt — or to be refused outright — add your own
ask or deny rules to your agent CLI's settings, for example ask rules for
`git -c …`, `… --no-verify` and the tmux verbs above. git-paw never writes
them, never edits them, and never removes them. Like any rule matched on a
command's text, they are a speed bump against the natural forms rather than a
boundary; the [sandbox](./sandbox.md) is the boundary.

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

## Broker verdict authority

Two broker messages carry the root session's authority: `agent.verified`, which
authorises a branch to merge, and `agent.feedback` whose `from` is the root
identity `supervisor`, which the drive loop's correction loop treats as a gate
verdict. The broker accepts these **only** with the per-session authority
credential that only the root session holds, sent in the `X-Paw-Authority`
request header. Without it — or with a wrong value — `POST /publish` answers
`403 {"error":"authority required"}` and changes no broker state. An
`agent.status` or `agent.artifact` that claims the `verified` status for a worker
is treated as a verdict too, since the roster would otherwise mark the branch
verified from it. The comparison
is constant-time, and the message body (the wire envelope) is unchanged: the
credential travels only in the header.

**Where the credential lives.** A broker-enabled `git paw start` mints 32 bytes
of OS randomness (64 hex characters) per session and stores them in the
user-data session file — under the platform data directory, **outside the
repository** and so outside every worktree — which git-paw writes owner-only
(`0600`). The launcher hands the same value to the broker host through a
one-shot `0600` file next to it, which the host removes once read. It is never
on a command line, in the environment, in a pane, or in a log, and it never
appears in the per-repo discovery file, `GET /status`, `GET /log`,
`git paw status --json`, MCP `get_session_status`, or the broker log. A session
saved by an earlier git-paw gains a credential the first time recovery or a
restart from pause starts its broker host.

**Who carries it.** Only root-side publishers:

- the unattended drive loop, for its own branch-refresh notification;
- the `sweep.sh` verdict verbs `verified`, `feedback-gate` and `conflict-ack`,
  which fetch it through the hidden `git paw __authority` subcommand — which
  refuses to run from a linked worktree.

A root session's MCP server (one started in the main working tree) offers
`publish_verified` and `publish_feedback`, which carry the root credential read
from the same session file. A worktree session's MCP server has no verdict tool.

The worker-side `broker.sh` has no credential-bearing verb.

**Threat model.** The rule stops a mistaken or skill-misled worker — one
following a stale example, or an LLM that decides its own work is verified —
and a rogue worker sandboxed with home-directory **reads** denied: the
[sandbox](./sandbox.md)'s Tier 2 (deny `$HOME`), or a Tier 1 blocklist that
covers git-paw's data directory (`~/Library/Application Support/git-paw` on
macOS, `$XDG_DATA_HOME/git-paw` — by default `~/.local/share/git-paw` — on
Linux). The default sandbox profile confines writes only, so it does not hide
the credential. A worker with home-directory read access is out of this rule's
threat model, whether it runs unsandboxed or under the default profile: it can
read the credential, and keeping it out is the job of the sandbox's
read-confinement tiers.

**What stays ungated.** Every non-verdict message is accepted without the
credential, as before — including **`agent.status`, even one that claims the
`supervisor` identity**, which can wind down the drive loop's wave early.
Feedback from any other sender (the post-commit branch-guard hook, a person) is
not a verdict and stays ungated, and so does one agent publishing a non-verdict
message as another. Messages the broker's own subsystems publish in-process
(the conflict detector, the role guard) never cross HTTP and are not gated.

## MCP repository writes

The MCP server's spec-authoring tools (`create_spec`, `write_spec_artifact`) are its only tools that write file content into the repository. MCP input is untrusted, because the orchestrator calling them may have been prompt-injected by content it read. These are the guarantees:

- **Off by default, and only the repository can consent.** The tools exist only when the repository's own `.git-paw/config.toml` sets `[mcp] allow_repo_writes = true`. The user-level config, environment variables and tool arguments cannot enable them, and no tool has a parameter that grants consent. The server reads the switch once at startup and logs whether writes are enabled.
- **Root session only.** A server in an agent's worktree never lists or runs them, whatever that worktree's config says.
- **Confined to the change directory.** Lexical checks refuse an absolute `path`, a `..` or `.git` component, and a `change` that is not one kebab-case segment. Canonical checks refuse a spec directory outside the repository, inside `.git`, at the repository root or inside `.git-paw`, a symlinked directory leading out of the change directory, and a symlinked target file. The spec directory comes from the repository's own `[specs]` section only, never the user-level config. So writes land only inside change folders under the repository-configured spec directory, and never in `.git` or `.git-paw`. Where that directory points is the repository's choice: set to `.github` or `src`, its "change folders" are CI or source directories. Content is written to a temporary file and renamed into place.
- **No process, no commit.** No tool spawns a process (`validate_spec` checks in-process), and nothing is staged or committed. `git status` / `git diff` and one server-log line per write record what was authored.

**The limit.** The switch is a file in the repository, so anything that can edit the root's `.git-paw/config.toml` can turn it on. That includes an unsandboxed worker writing outside its worktree, or a merged branch that carries the key. The change shows up in the diff a human reviews and in the server's startup log line, and the [sandbox](./sandbox.md) confines a worker's writes to its worktree. Against an unsandboxed rogue worker, though, the switch is no defence, as with [broker verdict authority](#broker-verdict-authority). Likewise, a local process that can already write inside the change directory could swap a directory that was just checked for a symbolic link before the write lands. That is out of scope for the same reason: it already has write access.

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
  behalf. `git paw doctor` names the known-dangerous ones — above all a
  `sweep.sh` rule that lets an agent approve its own prompts — but removing
  them is yours to do.
- Run untrusted or aggressive automation on hardware, accounts, and repositories
  whose compromise you could tolerate.

`git paw doctor` prints a pointer to this page and reports whether your OS's
sandbox backend is available.

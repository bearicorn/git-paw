# FS-Scoped Sandbox

An optional, opt-in way to confine an agent to its worktree using your OS's
kernel sandbox — `sandbox-exec` (Seatbelt) on macOS, `bwrap` (bubblewrap) on
Linux/WSL. It bounds the blast radius of a full-auto agent: writes are allowed
inside the worktree and denied outside, enforced by the kernel.

**The sandbox is your own local configuration.** git-paw does **not** invoke or
generate it, and does not ship a maintained profile. It launches whatever
`[clis.<name>].command` names; you point that at a small wrapper that execs the
real CLI under the sandbox. Everything below is a worked example to **copy and
adapt** to your CLI, OS, and stack — not a drop-in git-paw ships.

> The sandbox confines writes. By default it does **not** restrict reads — see
> [Confidentiality hardening](#confidentiality-hardening) — and it does not cover
> network or resource-exhaustion. Read the [Security Posture](./security-posture.md)
> first.

## How it hooks in

git-paw already gives a wrapper everything it needs at launch: the pane's working
directory *is* the worktree, and `GIT_PAW_BROKER_URL` is in the environment. So a
wrapper derives its scope from `$PWD` with **zero git-paw changes**.

**Point `command` at the wrapper.** git-paw launches the executable `command`
resolves to, so `command` may be the wrapper's path, and the wrapper need not be
on `PATH` under the entry's name. Declare the family so the sandboxed CLI keeps
the real CLI's native flags and boot form:

```toml
[clis.claude-sandboxed]
command = "/home/me/bin/claude-sandboxed"   # the wrapper script itself
family = "claude"
```

If you also keep a wrapper on `PATH` under the entry's name
(`claude-sandboxed`), it must be the same file `command` resolves to. When the
two differ, git-paw **refuses to launch** that CLI and names both paths, rather
than guess which one you meant — so a sandbox wrapper is never silently
bypassed. Rename the entry's key, or set `command` to the full path of the
executable you mean, and start again.

`git paw doctor` reports whether `sandbox-exec`/`bwrap` is installed, and
flags an ambiguous `[clis.*]` entry before a launch would refuse it.

## Sandboxed full-auto workers

Tell git-paw how a CLI's workers are confined with `[clis.<name>].sandbox`.
The declaration is yours: git-paw does **not** read, run or verify the
sandbox, and `git paw doctor` reports the mode as *declared*, never as
verified.

**`sandbox = "wrapper"`** — `command` is your FS-sandbox wrapper (the scripts
below). git-paw adds nothing to the launch command; the declaration only tells
`doctor` the CLI is meant to be confined.

```toml
[clis.claude-sandboxed]
command = "/home/me/bin/claude-sandboxed"
family = "claude"
sandbox = "wrapper"
```

**`sandbox = "native"`** — git-paw passes the CLI's own sandbox arguments at
launch, after `args` and before the approval flags. They come from
`[clis.<name>].sandbox_args`, else the provider family's built-in row. Only
Codex has a built-in row, `--sandbox workspace-write`; for any other CLI set
`sandbox_args` yourself, or the config fails to load naming `sandbox` and
`sandbox_args`.

```toml
[clis.codex]
command = "codex"
sandbox = "native"
# sandbox_args = "--sandbox workspace-write"   # the built-in row; override here
```

When the approval flags already contain the sandbox arguments verbatim (Codex
at `auto` passes `--sandbox workspace-write` as its approval row), they are
not repeated.

**Nothing switches a native sandbox off.** For a CLI declared `native`,
git-paw inspects the approval flags a launch would pass — the built-in row or
your `approval_args` override alike — and refuses the launch, naming the CLI,
the level, the flag and `[clis.<name>].approval_args`, when they:

- contain a flag the CLI family turns its sandbox off with — for Codex,
  `--dangerously-bypass-approvals-and-sandbox`, which is its built-in
  `full-auto` row; or
- set the sandbox's own flag (the first word of the sandbox arguments, such as
  `--sandbox`) to a different value — for example `sandbox_args = "--sandbox
  read-only"` at `auto`, where Codex's approval row already passes
  `--sandbox workspace-write`. Two values would reach the CLI and it would
  pick one.

No pane runs the refused CLI. To run it prompt-free, give that level flags that
keep the sandbox on:

```toml
[clis.codex]
command = "codex"
sandbox = "native"
approval_args = { "full-auto" = "<the CLI's prompt-free flags>" }
```

The headless orchestrator hand-off follows the same rule: it passes the
native sandbox arguments, and is unavailable (judgment calls stay on the
broker) where a launch at the orchestrator's `approval` level would be refused.

A `wrapper` declaration never restricts full-auto: the wrapper confines the CLI
from outside, whatever its flags are.

**Declaring a sandbox does not change the approval level.** The recipe for a
prompt-free worker is explicit: declare the sandbox **and** set the level.

```toml
[supervisor]
agent_approval = "full-auto"

[clis.claude-sandboxed]
command = "/home/me/bin/claude-sandboxed"
family = "claude"
sandbox = "wrapper"
```

Every launch (`start`, `add`, recovery) prints a warning when a pane runs at
`full-auto` on a CLI that declares no sandbox, whichever way the CLI was chosen.
`git paw doctor` lists each configured worker CLI's declared sandbox mode. At
`full-auto` it warns (⚠) for every one of them — a CLI with no declaration, and
one with a declaration, shown as `declared, not verified`, because git-paw
cannot see whether the sandbox confines anything. It also warns when a
`wrapper` entry's `command` is the agent CLI binary itself rather than a
wrapper, and fails (✗) where a launch would be refused. Read
what a sandboxed full-auto worker can still do in the
[Security Posture](./security-posture.md#sandboxed-full-auto-workers).

## macOS (`sandbox-exec`)

A wrapper (the script `command` points at):

```sh
#!/bin/sh
set -eu
WT="$PWD"
GITDIR="$(git -C "$WT" rev-parse --git-common-dir)"; GITDIR="$(cd "$GITDIR" && pwd)"
exec sandbox-exec \
  -D WORKTREE="$WT" \
  -D GITDIR="$GITDIR" \
  -D GITHOOKS="$GITDIR/hooks" \
  -D GITCONFIG="$GITDIR/config" \
  -D TMPDIR="${TMPDIR%/}" \
  -D CLIHOME="$HOME/.claude" \
  -D CARGOHOME="${CARGO_HOME:-$HOME/.cargo}" \
  -f "$HOME/.config/git-paw/sandbox.sb" \
  claude "$@"
```

The profile (`sandbox.sb`), a `(allow default)` base that denies writes except a
scoped set:

```scheme
(version 1)
(allow default)                          ;; reads + network stay open
(deny file-write*)
(allow file-write*
  (subpath (param "WORKTREE"))           ;; the worktree
  (subpath (param "GITDIR"))             ;; shared .git — needed to commit
  (subpath (param "TMPDIR"))
  (subpath (param "CLIHOME"))            ;; the CLI's own config/auth/cache
  (subpath (param "CARGOHOME"))          ;; toolchain cache (stack-specific)
  (regex #"^/private/tmp/claude-")       ;; the CLI's per-session temp/cwd
  (subpath "/private/tmp")               ;; shell heredocs (zsh writes /private/tmp/zsh*)
  (literal "/dev/null") (literal "/dev/tty") (regex #"^/dev/ttys[0-9]+$"))
;; Close the git-persistence gap (see below):
(deny file-write*
  (subpath (param "GITHOOKS"))
  (literal (param "GITCONFIG")))
```

`/private/tmp` must be writable, not just `$TMPDIR` and the CLI's per-session
prefix: `zsh` writes heredoc bodies to `/private/tmp/zsh*` while building the
command it's about to run, so the standard multi-line commit idiom —
`git commit -m "$(cat <<'EOF' … EOF)"` — hits `Operation not permitted`
without this grant. `/private/tmp` is already world-writable and holds no
secrets, so this is a write-only convenience grant; it does not change the
read-confidentiality tiers below (see
[Confidentiality hardening](#confidentiality-hardening)).

### What breaks under the sandbox

Some failures aren't a missing write grant — they're the kernel refusing to
`exec` at all, and no profile rule fixes them:

**Setuid/setgid binaries cannot exec under `sandbox-exec`.** `/bin/ps` and
`/usr/bin/top` are setuid root on macOS. Seatbelt refuses to exec a setuid
binary inside *any* profile, even the most permissive one:

```
$ sandbox-exec -p '(version 1)(allow default)' /bin/ps -o pid= -p $$
sandbox-exec: execvp() of '/bin/ps' failed: Operation not permitted
```

An agent running `ps` inside a wrapped CLI typically only sees a bare
`operation not permitted: ps` — the same wording the sandbox uses for a
denied write. Don't chase it with a write grant: it isn't one, and no
`(allow …)` rule fixes it. Use `pgrep` instead — it works under the same
profile because it isn't setuid.

The bundled worker-orientation skill already tells agents this, so both
surfaces agree; an agent that hits it should adapt (use `pgrep`) rather than
probe or widen the profile.

## Linux / WSL (`bwrap`)

```sh
#!/bin/sh
set -eu
command -v bwrap >/dev/null || exec claude "$@"   # detect-and-degrade
WT="$PWD"
GITDIR="$(git -C "$WT" rev-parse --git-common-dir)"; GITDIR="$(cd "$GITDIR" && pwd)"
exec bwrap \
  --ro-bind / / \
  --bind "$WT" "$WT" \
  --bind "$GITDIR" "$GITDIR" \
  --ro-bind "$GITDIR/hooks" "$GITDIR/hooks" \
  --ro-bind "$GITDIR/config" "$GITDIR/config" \
  --bind "${TMPDIR:-/tmp}" "${TMPDIR:-/tmp}" \
  --bind "$HOME/.claude" "$HOME/.claude" \
  --bind "${CARGO_HOME:-$HOME/.cargo}" "${CARGO_HOME:-$HOME/.cargo}" \
  --dev-bind /dev /dev \
  --unshare-user \
  --chdir "$WT" \
  claude "$@"
```

`bwrap` needs unprivileged user namespaces; some hardened kernels disable them —
hence the `command -v bwrap` degrade line. `sandbox-exec` is deprecated but
present on every macOS; both are pragmatic, not stable contracts.

The `--bind "${TMPDIR:-/tmp}" "${TMPDIR:-/tmp}"` line already covers shell
heredocs on Linux — `zsh` writes heredoc bodies under `$TMPDIR` (or `/tmp` when
unset), and both are bound writable, so the `git commit -m "$(cat <<'EOF' …
EOF)"` idiom works here without an extra grant. `ps` is not setuid on Linux
and works fine here — see [What breaks under the
sandbox](#what-breaks-under-the-sandbox) for why it fails on macOS instead.
`bwrap` itself sets `no_new_privs`, so even a setuid binary that did run
inside it would run without its elevated privileges.

## The writable set to get right

The profile is only as good as this list. Beyond the worktree, an agent CLI
legitimately needs to write:

- **The shared `.git`** — a child worktree commits into the main repo's `.git`,
  so it must be writable **except** `.git/hooks` and `.git/config` (below).
- **The CLI's own config/auth/cache** (`~/.claude`, `~/.config/…`) and its
  **per-session temp/cwd**. Claude Code, for example, uses
  `/private/tmp/claude-<uid>/` *and* `/private/tmp/claude-<hash>-cwd` symlinks
  (random names → a **regex**, not a subpath) — omit these and the agent's shell
  tool dies with `operation not permitted`.
- **`TMPDIR`** and device nodes (`/dev/null`, `/dev/tty`, the pane pty).
- **Toolchain caches** the workers write to (`~/.cargo`, `~/.rustup`, …) — these
  are **stack-specific**; source them from your stack, don't assume.
- **Network** stays open by default: agents need the loopback broker and outbound
  model-API egress.

## The git-persistence gap

Keep `.git` writable for commits, but bind `.git/hooks` and `.git/config`
**read-only**. Otherwise a confined agent can plant a `.git/hooks/post-commit`
script or set `core.pager`/`editor`/`fsmonitor` to an arbitrary command — which
fires **later, unsandboxed, with full privileges** the next time you run plain
`git` outside the box. The agent can still commit (index/objects/refs); it just
can't plant persistence.

## Credential refresh (per-OS)

A long-lived agent refreshes its OAuth token mid-session. Where that token lives
differs by OS, and it affects the profile:

- **macOS:** the token is in the **login Keychain**, and refresh **writes** it
  back — an in-process write the `(deny file-write*)` base blocks, so the agent
  logs in fine at first but hits *"Not logged in"* when the token expires. To
  allow refresh, add `~/Library/Keychains` to the writable set. **Tradeoff:** that
  grants write to your whole login keychain (and reads were already open) — so
  the sandbox protects filesystem write-integrity, not the confidentiality of
  keychain secrets. If your CLI can be configured to use **file-based**
  credentials (a file under the already-writable `CLIHOME`), prefer that and deny
  the Keychain entirely.
- **Linux:** credentials are file-based (headless: a file under `~/.claude*/`) or
  written by a separate Secret Service **daemon** (desktop). The file lives in the
  writable `CLIHOME`, and the daemon writes on the agent's behalf outside the
  sandbox — so **neither has the macOS refresh problem**.

## Confidentiality hardening

The base profile confines **writes** only; the agent can still **read** anything
your user can (SSH keys, cloud creds, other repos). If that matters, add
read-denials. Two tiers:

**Tier 1 — blocklist (cheap).** Keep `(allow default)`, deny reads of known
secret stores:

```scheme
(deny file-read*
  (subpath (param "SSH"))       ;; ~/.ssh
  (subpath (param "AWS"))       ;; ~/.aws
  (subpath (param "GNUPG")))    ;; ~/.gnupg
```

**Tier 2 — allowlist (strong).** Deny reading your home, then re-allow only what
the agent needs (everything outside `$HOME` — system libraries — stays readable,
so the app still runs):

```scheme
(deny file-read* (subpath (param "HOME")))
(allow file-read*
  (subpath (param "REPO"))
  (subpath (param "CLIHOME"))
  (subpath (param "CARGOHOME"))
  (subpath (param "KEYCHAINS")))   ;; still needed for macOS auth (see above)
```

**`~/.ssh` is safe to deny** for a git-paw worker: it commits *locally* (no key
needed unless you sign commits with an SSH key), and pushes/fetches are done by
the unsandboxed supervisor or you — not the worker. Deny it unless your workers
sign with an SSH key or push over SSH themselves, in which case grant the
specific key plus `SSH_AUTH_SOCK` rather than the whole folder.

The one residual even under Tier 2 is the login Keychain (allow-listed for auth);
per-item ACLs do **not** gate a confined agent in your unlocked session. Close it
only via file-based credentials as noted above.

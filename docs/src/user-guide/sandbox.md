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

**The wrapper must be a PATH-resolvable executable.** git-paw launches a custom
CLI *by its name* on `PATH` — not by an arbitrary script path. Put your wrapper on
`PATH` (e.g. symlink it next to the real CLI) and set `command` to that name:

```toml
[clis.claude-sandboxed]
command = "claude-sandboxed"   # a wrapper on PATH, NOT a path to a script
```

If you instead point `command` at a script path that isn't resolvable by name,
the pane drops to a raw shell and the boot prompt is typed into it.

`git paw doctor` reports whether `sandbox-exec`/`bwrap` is installed.

## macOS (`sandbox-exec`)

A wrapper (`claude-sandboxed` on your `PATH`):

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
  (literal "/dev/null") (literal "/dev/tty") (regex #"^/dev/ttys[0-9]+$"))
;; Close the git-persistence gap (see below):
(deny file-write*
  (subpath (param "GITHOOKS"))
  (literal (param "GITCONFIG")))
```

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

## Why

The documented FS-scoped sandbox profile (v0.15) denies the shell's `/private/tmp`
heredoc directory, so the standard `git commit -m "$(cat <<'EOF' … EOF)"` idiom fails
on every commit under the sandbox (peer-4-poker, GP-11). `zsh` writes heredoc bodies to
`/private/tmp/zsh*`, which the profile's `$TMPDIR`-only + `/private/tmp/claude-` grants
do not cover — so the most common multi-line-message commit path is broken for a
sandboxed worker.

## What Changes

- The documented sandbox profile grants write access to `/private/tmp` (macOS) — and
  the bound `TMPDIR`/`/tmp` on Linux — so shell heredocs, and thus the standard
  heredoc commit idiom, work under the sandbox.
- The FS-scoped sandbox chapter notes this grant and why it is required.

Docs-only fix to shipped v0.15 content; git-paw ships no profile (the sandbox is
docs-only, launched through the CLI-command seam). Not breaking.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `core-security-posture`: the documented FS-scoped sandbox profile permits shell
  heredoc temporary files, so the standard heredoc commit idiom works under the sandbox.

## Impact

- **Docs:** `docs/src/user-guide/sandbox.md` (add `(subpath "/private/tmp")` to the
  macOS profile's write allow-list with an explanatory comment + a note in the prose;
  confirm the Linux `bwrap` example binds `/tmp`); the docs-parity guard
  (`tests/security_posture_docs.rs`).
- **Code:** none — the v0.15 sandbox is docs-only.
- **Security:** granting write to `/private/tmp` does not meaningfully expand the
  worker's blast radius — `/private/tmp` is already world-writable and holds no secrets;
  the sandbox's write-confinement value is protecting `$HOME`, the repo root, and
  `.git` internals, which are unchanged. Confirm it does not weaken the documented
  read-confidentiality tiers (it is a *write* grant only). Gate-5 note.

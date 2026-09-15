## Why

The documented FS-scoped sandbox silently breaks setuid binaries, and the guide does not say so
(GP-14). On macOS `/bin/ps` is setuid root (`-rwsr-xr-x root wheel`), and Seatbelt refuses to exec a
setuid binary inside a `sandbox-exec` profile — verified on this machine with the most permissive
profile possible:

```
$ sandbox-exec -p '(version 1)(allow default)' /bin/ps -o pid= -p $$
sandbox-exec: execvp() of '/bin/ps' failed: Operation not permitted   (exit 71)
$ sandbox-exec -p '(version 1)(allow default)' /usr/bin/pgrep -x zsh   # not setuid → works
```

So a confined agent that runs `ps` sees a bare `operation not permitted: ps` — the same wording the
sandbox produces for a denied write — and there is no allow-rule that fixes it, because the refusal
is not a file-write denial. `/usr/bin/top` is setuid too. An agent (or a reader tuning the profile)
who does not know this burns time widening the writable set for nothing. The bundled worker skill
already tells the *agent* this (`assets/agent-skills/coordination.md`, "Some setuid binaries cannot
exec under a sandbox"); the operator-facing sandbox chapter
(`docs/src/user-guide/sandbox.md`) does not, and it is the chapter people read when a tool fails
under the box.

This is a documentation delta only — no code, no profile shipped by git-paw changes.

## What Changes

- `docs/src/user-guide/sandbox.md` gains a short **"What breaks under the sandbox"** list beside the
  existing heredoc note (the `/private/tmp` grant explanation in the macOS section), covering:
  setuid/setgid binaries cannot exec under `sandbox-exec` — `/bin/ps` and `/usr/bin/top` on macOS —
  with the exact symptom (`execvp() … Operation not permitted` / a bare `operation not permitted: ps`),
  that no profile rule fixes it, that it is expected rather than a sign the profile is wrong, the
  non-setuid alternatives that do work (`pgrep`, `lsof`-style tools that are not setuid — the
  chapter recommends `pgrep`, which was verified), and the Linux note that `bwrap` runs setuid
  binaries without their elevated privileges (`no_new_privs`) while `ps` itself is not setuid there
  and works.
- A pointer that the worker-orientation skill already tells agents the same thing, so the two
  surfaces agree.
- `tests/security_posture_docs.rs::sandbox_chapter_covers_the_required_setup` gains the `setuid` and
  `ps` needles so the note cannot silently drop out.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `core-security-posture`: new requirement — the sandbox chapter documents what the FS sandbox
  breaks (setuid exec), next to the heredoc note.

## Impact

- **Docs:** `docs/src/user-guide/sandbox.md` (new subsection in / after the macOS section, adjacent to
  the heredoc grant explanation; a one-line cross-reference from the Linux section). `mdbook build`
  must stay green; no `SUMMARY.md` change.
- **Tests:** `tests/security_posture_docs.rs` needle list (`setuid`, `ps`).
- **Code / assets:** none. `assets/agent-skills/coordination.md` already carries the agent-facing
  note and is not edited (export-agnosticism unaffected).
- **Enum-variant ripple:** none.
- **Behaviour change:** none.
- **Security / safety:** documentation only; it does not weaken the sandbox — it explains a refusal
  the sandbox makes, and steers readers away from widening the profile to "fix" it.
- **Dependencies:** none.

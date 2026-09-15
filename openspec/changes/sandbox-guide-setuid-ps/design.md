## Context

`docs/src/user-guide/sandbox.md` documents the user's own FS-scoped sandbox (`sandbox-exec` on macOS,
`bwrap` on Linux). Its macOS section explains the one non-obvious grant proven in dogfood — writable
`/private/tmp` for zsh heredocs — and "The writable set to get right" lists the paths whose omission
makes an agent's shell die with `operation not permitted`. There is no list of things the sandbox
breaks that **no grant can fix**. Setuid exec is one: Seatbelt refuses to exec a setuid binary inside a
profile regardless of rules (verified locally: `/bin/ps` is `-rwsr-xr-x root`, and
`sandbox-exec -p '(version 1)(allow default)' /bin/ps` fails with `execvp() … Operation not permitted`,
exit 71; non-setuid `/usr/bin/pgrep` succeeds under the same profile). The agent-facing skill
(`assets/agent-skills/coordination.md`) already states this; the operator chapter does not.

## Goals / Non-Goals

**Goals:**
- One short, verified note in the operator chapter, findable from the symptom, with the alternative.
- A doc test that keeps it there.

**Non-Goals:**
- Any code or shipped-asset change; any workaround inside git-paw (there is none to ship — the
  refusal is the kernel's).
- Cataloguing every setuid binary; `ps` and `top` are the ones an agent reaches for.
- Editing the worker skill (already correct).

## Decisions

### D1 — Place it with the heredoc note, framed as "what breaks"

The heredoc paragraph is where a reader lands when something fails under the box; the setuid note
goes right after it under a small `### What breaks under the sandbox` heading in the macOS section
(the refusal is macOS-specific in practice), with a one-line Linux clause so a `bwrap` user does not
go looking for a matching problem. Keeping both notes together turns two anecdotes into the list the
milestone asks for, without a new chapter.

### D2 — Say what fixes it and what does not

The note states explicitly that no `(allow …)` rule fixes a setuid exec refusal, so readers do not
widen the writable set (weakening the sandbox) chasing it, and gives `pgrep` (verified) as the
non-setuid way to find processes. `top` is named as a second setuid example; no alternative is
claimed for it.

### D3 — Guard with the existing doc test

`tests/security_posture_docs.rs::sandbox_chapter_covers_the_required_setup` is a needle list over the
chapter; add `setuid` and `ps` (matched as the token in the note, e.g. `/bin/ps`). Same pattern as the
`heredoc` / `/private/tmp` needles added by `sandbox-guide-heredoc-fix`.

## Risks / Trade-offs

- **Platform drift:** a future macOS could change `/bin/ps`'s bits or Seatbelt's behaviour. The note
  describes the mechanism (setuid exec refused) rather than promising the binary stays setuid, so it
  degrades to "still true for whatever is setuid".
- **Needle `ps` is short**; anchoring on `/bin/ps` keeps it specific.

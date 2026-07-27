---
name: security-review
description: Review a change for external-bad-actor security (the Gate-5 security audit). Use when a change touches command allowlists, spawns processes, handles paths or untrusted input, reads secrets, or adds a dependency. Checks least-privilege path-scoped allowlists, injection-safe construction, no secrets in flags or env, no unsafe shell/path handling, and vetted dependencies. For the harm a rogue agent git-paw runs could cause, use the safety-review skill instead.
license: MIT
compatibility: git-paw
---

# Security Review (external bad actors)

The Gate-5 security lens for threats from **outside** — an attacker against the tool or the
consumer's environment. For the harm a *rogue agent git-paw itself runs* could cause, see the
`safety-review` skill; the two are complementary.

## What to check

- **Least privilege** — allowlist grants are path-scoped and minimal; never `curl *`, `cd *`, or a
  blanket grant. New broker-helper / curl seeding stays path-scoped.
- **Injection-safe construction** — untrusted values (paths, branch/session names, config) are quoted
  or built through validating newtypes, never interpolated raw into a shell / tmux / git command.
- **No secrets in flags or env** — secrets come from files, stdin, or IPC; never a flag (leaks to
  `ps` / shell history) or an env var (leaks to logs / child processes).
- **No unsafe shell/path handling** — validate inputs at the boundary; reject `..` traversal;
  canonicalize where it matters.
- **Supply chain** — new dependencies are in the approved set, permissively licensed, and pass
  `cargo deny` / `audit`.
- **`unsafe`** — justified, documented, and sound.

## Gate-5 review checklist

- [ ] Does this widen an allowlist or add a grant? Path-scoped and least-privilege?
- [ ] Any untrusted value interpolated into a command? Quoted or newtyped?
- [ ] Any secret read from a flag or an env var?
- [ ] New dependency vetted (approved set, license, advisories)?
- [ ] Any new or changed `unsafe`? Justified and sound?

---

Repo-local dev skill.

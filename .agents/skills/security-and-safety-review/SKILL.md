---
name: security-and-safety-review
description: Review a change for harm potential from two angles — SECURITY (external bad actors attacking the tool or the consumer's environment) and SAFETY (the blast radius of a rogue or mistaken agent git-paw itself runs, with real execution power under whatever permission mode its CLI resolves — up to full-auto, where nothing prompts). Use when a change touches allowlists (including any agent-CLI allow rule git-paw seeds), keystroke dispatch, process spawning, paths, untrusted input, secrets, dependencies, prompt detection, worktree confinement, or git/file operations. Covers least-privilege, injection, secrets, and dependencies for security; and out-of-worktree actions, irreversible git, persistence, and exfiltration for safety.
license: MIT
compatibility: git-paw
---

# Security & Safety Review

Two complementary lenses on **how much harm a change could enable** — review whichever the change
touches (often both):

- **Security** — external **bad actors** attacking the tool or the consumer's environment.
- **Safety** — the blast radius of a **rogue or mistaken agent git-paw itself runs**, with real
  execution power: whatever its CLI's resolved permission mode allows without asking, up to
  `full-auto`, where nothing prompts at all.

A tool that launches agents with that much reach needs both, and they trigger on different parts
of a diff.

## Security — external bad actors (Gate 5)

- **Least privilege** — any allow rule git-paw seeds into an agent CLI is path-scoped and
  minimal; never `curl *`, `cd *`, or a blanket grant, and never a consumer toolchain verb. Seed
  only with the consumer's explicit consent. git-paw seeds nothing today, so a change that starts
  seeding is a new surface, not an extension of an existing one.
- **Injection-safe construction** — untrusted values (paths, branch/session names, config) are quoted
  or built through validating newtypes, never interpolated raw into a shell / tmux / git command.
- **No secrets in flags or env** — secrets come from files, stdin, or IPC; never a flag (leaks to
  `ps` / shell history) or an env var (leaks to logs / child processes).
- **No unsafe shell/path handling** — validate inputs at the boundary; reject `..` traversal;
  canonicalize where it matters.
- **Supply chain** — new dependencies are in the approved set, permissively licensed, pass
  `cargo deny` / `audit`; any `unsafe` is justified, documented, and sound.

## Safety — rogue-agent blast radius

git-paw orchestrates AI agents that run real commands. Ask: **if an agent went rogue or made a
catastrophic mistake, how much damage could this change let it do?**

The FS-scoped sandbox is the structural containment, and it is opt-in — so this review remains the
behavioral guard for every session that does not use it.

- **Containment a change MUST NOT weaken** — worktree confinement (an agent writes only inside its
  own worktree; `agent-memory-isolation`), the rule that **git-paw approves no permission prompt**
  (every live prompt reaches a human; nothing in git-paw may start answering one), the
  **live-prompt gate** and the approval **send-gate** (a keystroke goes out only on a re-confirmed
  live prompt, never to pane 0, never as a durable grant), and the per-pane claim.
- **Catastrophes to catch** — deleting/modifying anything **outside the worktree** (OS / system paths,
  `$HOME`, sibling repos); **irreversible git** beyond scope (force-push, history rewrite, deleting a
  branch that is not the agent's own); **persistence / backdoors** that outlive the session (cron,
  shell-profile edits, git hooks, injected startup code); **exfiltration** of repo content or secrets;
  **privilege escalation**.

## Review checklist

**Security**
- [ ] Widens an allowlist or seeds a grant into an agent CLI? Path-scoped, least-privilege, consented?
- [ ] Introduces any path that answers a permission prompt without a human? That is forbidden.
- [ ] Any untrusted value interpolated into a command? Quoted / newtyped?
- [ ] Any secret read from a flag or env var? New dependency vetted? New/changed `unsafe` sound?

**Safety**
- [ ] Could a rogue/mistaken agent use this to act **outside its worktree** or do something
      **irreversible** out of scope?
- [ ] Could it establish **persistence**/a backdoor, or **exfiltrate** repo content or secrets?
- [ ] Does it preserve worktree confinement, the danger-list escalation, and the send-gate?
- [ ] Until the sandbox lands, is the containment for this surface sufficient — or gate it to a human?

---

Repo-local dev skill.

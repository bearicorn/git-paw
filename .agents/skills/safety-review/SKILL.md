---
name: safety-review
description: Review a change for the blast radius of a rogue or mistaken agent that git-paw runs — not external attackers (that is security-review) but the harm an orchestrated agent with real, often auto-approved, execution power could cause. Use when a change touches command classification, allowlists, worktree confinement, file writes, git operations, or anything an agent can trigger. Catches irreversible or out-of-scope actions such as deleting OS folders, writing outside the worktree, destructive git, persistence or backdoors, and exfiltration.
license: MIT
compatibility: git-paw
---

# Safety Review (rogue-agent blast radius)

git-paw orchestrates AI agents that run real commands, often auto-approved. This lens asks: **if an
agent went rogue or made a catastrophic mistake, how much damage could this change let it do?**
External attackers are `security-review`'s job; this is about the agents git-paw *itself* runs.

A FS-scoped sandbox is planned (roadmap v0.15.0) as the structural containment. Until it lands — and
continuously after — this review is the behavioral guard.

## The containment git-paw relies on (a change MUST NOT weaken these)

- **Worktree confinement** — an agent writes only inside its own worktree (`agent-memory-isolation` +
  the write-path checks).
- **The auto-approve danger-list** — the classifier escalates destructive commands (`rm -rf` outside a
  scratch path, etc.) to a human instead of auto-approving.
- **Path-scoped allowlists + the approval send-gate** — no broad grants; approvals re-confirmed live.

## Catastrophes to catch

- Deleting or modifying anything **outside the worktree** — OS / system paths, `$HOME`, sibling repos.
- **Irreversible git** beyond scope — force-push, history rewrite, deleting a branch that is not the
  agent's own.
- **Persistence / backdoors** that outlive the session — cron entries, shell-profile edits, git hooks,
  injected startup code.
- **Exfiltration** — sending repo content or secrets off the machine (ties to the no-telemetry guarantee).
- **Privilege escalation** or touching anything requiring elevated rights.

## Review checklist

- [ ] Could a rogue or mistaken agent use this change to act **outside its worktree**?
- [ ] Does it let an agent do something **irreversible** out of scope (delete, force-push, rewrite)?
- [ ] Could it establish **persistence** or a backdoor beyond the session?
- [ ] Any path for **exfiltration** of repo content or secrets?
- [ ] Does it preserve worktree confinement, the danger-list escalation, and the send-gate?
- [ ] Until the sandbox lands, is the containment for this surface sufficient — or should the action be
      gated to a human?

---

Repo-local dev skill.

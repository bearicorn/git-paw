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

## What the controls do — and do not — guarantee

git-paw ships several controls. None is a complete sandbox on its own; each
narrows one specific risk. Do not read any of them as "the agent is now safe."

| Control | What it does | What it does **not** do |
|---|---|---|
| **Auto-approve classifier** | Auto-approves command shapes it recognizes as safe (worktree-confined writes, known read-only verbs); escalates the rest to a human. | It is **heuristic pattern-matching**, not a security boundary. It can misclassify, and it is trivially bypassed by a command shape it hasn't seen. It reduces prompt fatigue; it does not confine anything. |
| **Command allowlists** | Grant specific command prefixes by path/verb so routine dev commands don't prompt. | A grant is a convenience, not containment — an allowed prefix can still do anything that prefix permits. Path-scope your grants; never grant `curl *` / `cd *`. |
| **Protected-paths rule** (agent-memory isolation) | *Flags* writes that resolve outside the agent's worktree (e.g. into another agent's memory). | It only **warns/gates** at the classifier layer; it is not kernel-enforced and does not physically prevent the write. |
| **`.git/`-write rule** | *Flags* any write/edit/create targeting a path inside a repository's `.git/` directory (config, hooks, `info/exclude`) as a danger-class escalation, never auto-approved — even when the verb is otherwise whitelisted. | Same classifier-layer caveat as above: heuristic, not kernel-enforced. See [Configuration reference](../configuration/README.md#auto-approve-safe-permission-prompts). |
| **FS-scoped sandbox** (optional, you configure it) | When you wrap an agent CLI in `sandbox-exec`/`bwrap`, the **kernel** confines the agent's *writes* to its worktree — `rm -rf ~`, dotfile corruption, and planted git hooks are blocked at the syscall level. | It protects the machine **around** the work area, not the work area (the worktree, including uncommitted work, stays destroyable by design). The default profile does **not** restrict *reads*, so host secrets remain readable unless you add read-hardening. It does not cover network, resource-exhaustion, or exposed-socket vectors unless you add rules. See [Sandbox](./sandbox.md). |
| **Remote control** (documented workflow) | Lets you drive a session from elsewhere via each tool's own remote-control. | Exposing a session to remote control extends the blast radius to whoever reaches that channel; secure the channel yourself. |

## The bottom line

- Treat every agent as capable of running arbitrary code as you.
- The classifier, allowlists, and protected-paths reduce *friction and mistakes*;
  they are not containment.
- For containment, use the optional [FS-scoped sandbox](./sandbox.md) — and
  understand precisely what it does (writes) and does not (reads, by default)
  protect.
- Run untrusted or aggressive automation on hardware, accounts, and repositories
  whose compromise you could tolerate.

`git paw doctor` prints a pointer to this page and reports whether your OS's
sandbox backend is available.

## git-paw orchestrator

This section applies only to the agent session running at the repository
root (the main checkout). If you are a coding agent working in a git-paw
worktree, ignore this section and follow your own session assignment
instead.

When a git-paw session is active in this repository and you are that root
session, you are its orchestrator: you monitor the coding agents, verify
their work and merge their branches from your own session, outside the
session's terminal windows. Before you act on the session, read:

- `.git-paw/skills/orchestrator/SKILL.md` — the orchestration procedure.
- `.git-paw/AGENTS.local.md` — the current session's context: its session
  name, its broker URL and the bundled helper paths. Each session start
  rewrites it.

`git paw init` maintains this section between its markers; edit outside
them.

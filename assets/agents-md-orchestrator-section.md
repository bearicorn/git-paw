## git-paw orchestrator

When a git-paw session is active in this repository, you are its
orchestrator: you monitor the coding agents, verify their work and merge
their branches from your own session, outside the session's terminal
windows. Before you act on the session, read:

- `.git-paw/skills/orchestrator/SKILL.md` — the orchestration procedure.
- `.git-paw/AGENTS.local.md` — the current session's context: its session
  name, its broker URL and the bundled helper paths. Each session start
  rewrites it.

`git paw init` maintains this section between its markers; edit outside
them.

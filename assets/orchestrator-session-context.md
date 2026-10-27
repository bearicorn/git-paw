## git-paw session context

You are the orchestrator of the running git-paw session `{{SESSION_NAME}}`. The
procedure — monitoring, the five verification gates, merge orchestration,
permission relaying — is the orchestrator skill at
`.git-paw/skills/orchestrator/SKILL.md`; read it first. This file carries only
the values of this session, and is rewritten each time the session starts.

- **Session name:** `{{SESSION_NAME}}` — wherever the skill writes `<session>`,
  use this name; every tmux target names window 0 (`{{SESSION_NAME}}:0.<pane>`).
- **Broker URL:** `{{GIT_PAW_BROKER_URL}}` — for reference only; reach the
  broker through the helpers or git-paw's MCP tools, which discover it.
- **Helpers:** `.git-paw/scripts/sweep.sh` (reads, verdicts, answers, approvals
  on the human's instruction) and `.git-paw/scripts/broker.sh --agent supervisor`
  (questions and main-side intents), both run at the repository root.
- **Roster:** `.git-paw/sessions/{{SESSION_NAME}}.json` — each agent's
  `branch_id`, `worktree_path` and `cli`; read it when you need an agent's pane.

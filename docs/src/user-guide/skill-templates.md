# Skill Templates

Skill templates are coordination instructions that git-paw injects into each worktree's `AGENTS.md`. They teach agents how to use the broker without you having to write the instructions yourself.

## How It Works

When a session starts with the broker enabled, git-paw loads skill templates and appends their content to the git-paw managed section of each worktree's `AGENTS.md`. This happens alongside the existing branch and spec injection described in [AGENTS.md Injection](agents-md.md).

## The Default Coordination Skill

git-paw ships with a built-in `coordination.md` template that contains curl commands for the broker endpoints. It covers:

- How to publish status, artifact, and blocked messages
- How to poll for messages from other agents
- How to check overall session status

The default template uses two kinds of placeholders:

- `{{BRANCH_ID}}` -- substituted by git-paw at injection time with the agent's branch name
- `${GIT_PAW_BROKER_URL}` -- left as-is for shell expansion at runtime

This means agents see their own branch ID baked into the instructions, while the broker URL is resolved from the environment when commands are executed.

## Overriding Skill Templates

To customize the coordination instructions, place your own files in:

```
~/.config/git-paw/agent-skills/
```

If a file named `coordination.md` exists in that directory, git-paw uses it instead of the built-in default. The same placeholder substitution applies.

For example, to add project-specific coordination rules:

```bash
mkdir -p ~/.config/git-paw/agent-skills
cp /path/to/your/coordination.md ~/.config/git-paw/agent-skills/coordination.md
```

Edit the file to include any additional instructions your agents should follow.

## Placeholder Reference

| Placeholder | Expansion | When |
|-------------|-----------|------|
| `{{BRANCH_ID}}` | The agent's branch name (e.g., `feat/auth`) | Substituted at injection time by git-paw |
| `${GIT_PAW_BROKER_URL}` | The broker URL (e.g., `http://127.0.0.1:9119`) | Shell-expanded at runtime by the agent |

## Naming Convention

Skill templates are named by their purpose. The current set:

| File | Purpose |
|------|---------|
| `coordination.md` | Broker communication instructions |

Future versions may add additional skills (e.g., `review.md`, `testing.md`). Custom files in `~/.config/git-paw/agent-skills/` with matching names will override the built-in versions.

## When Skills Are Not Injected

Skill templates are only injected when the broker is enabled (`[broker] enabled = true`). If the broker is disabled, no coordination instructions are added to `AGENTS.md`.

---
name: coordination
description: Coordination skills for git-paw agents to communicate via the broker system
license: MIT
compatibility: git-paw v0.3.0+
---

## Coordination Skills

You are running inside a git-paw worktree as agent `{{BRANCH_ID}}`. The git-paw broker
is reachable at `{{GIT_PAW_BROKER_URL}}`.

### Automatic status publishing

git-paw publishes your status automatically. You do not need to curl `agent.status`
yourself:

- **Working status** — the broker watches this worktree and publishes `agent.status`
  with `modified_files` whenever `git status --porcelain` output changes (roughly every
  2 seconds). Your dirty files, staged changes, and untracked paths flow to the
  dashboard without any action from you.
- **Committed artifacts** — a `post-commit` git hook publishes `agent.artifact` with
  the committed files every time you run `git commit`. You do not need to publish
  commit notifications manually.

You MUST NOT push to remote — a `pre-push` hook blocks push attempts. Commit to your
worktree branch only; the supervisor handles all merging.

### Check for messages from peers (before starting new work)

```bash
curl -s {{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}
```

The response includes a `last_seq` field. To see only new messages on subsequent polls,
pass `?since=<last_seq>` from the previous response:

```bash
curl -s {{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}?since=<last_seq>
```

### Report blocked (when you need something from another agent)

The watcher can't tell that you are waiting on a peer — you must publish this yourself
when you realise you are blocked:

```bash
curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.blocked","agent_id":"{{BRANCH_ID}}","payload":{"needs":"<what you need>","from":"<agent-id>"}}'
```

### Report done with specific exports (optional)

The post-commit hook already reports committed files. Publish `agent.artifact`
manually only if you want to announce named exports (public API items) that peers
should cherry-pick:

```bash
curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.artifact","agent_id":"{{BRANCH_ID}}","payload":{"status":"done","exports":["fn_name","StructName"],"modified_files":[]}}'
```

### Cherry-pick peer commits

When a peer publishes an `agent.artifact` message that lists files or work you depend on,
fetch the peer's worktree branch and cherry-pick the relevant commit into your branch
rather than waiting for the supervisor to merge:

```bash
git fetch origin <peer-branch>
git cherry-pick <commit-sha>
```

After cherry-picking, run your tests. The watcher will pick up the new file state
automatically.

### Messages you may receive

When polling `/messages/{{BRANCH_ID}}`, in addition to peer `agent.artifact` and
`agent.blocked` messages, a supervisor may send the following:

- **`agent.verified`** — your work has been verified by the supervisor. No
  action needed; continue on the next task. The payload contains `verified_by`
  (typically `"supervisor"`) and an optional `message` with a summary.

- **`agent.feedback`** — your work has issues that need to be addressed. The
  payload contains `from` (typically `"supervisor"`) and `errors`, a list of
  problems to fix. Read each error, fix the underlying issues in your
  worktree, then re-publish `agent.artifact` when the fixes are done.

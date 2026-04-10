## Coordination Skills

You are running inside a git-paw worktree as agent `{{BRANCH_ID}}`. The git-paw broker
is reachable at `${GIT_PAW_BROKER_URL}`. Use the following `curl` commands to coordinate
with peer agents.

### Report progress (after each commit)

```bash
curl -s -X POST ${GIT_PAW_BROKER_URL}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.status","agent_id":"{{BRANCH_ID}}","payload":{"status":"working","modified_files":[],"message":null}}'
```

### Check for messages from peers (before starting new work)

```bash
curl -s ${GIT_PAW_BROKER_URL}/messages/{{BRANCH_ID}}
```

The response includes a `last_seq` field. To see only new messages on subsequent polls,
pass `?since=<last_seq>` from the previous response:

```bash
curl -s ${GIT_PAW_BROKER_URL}/messages/{{BRANCH_ID}}?since=<last_seq>
```

### Report completion (when done)

```bash
curl -s -X POST ${GIT_PAW_BROKER_URL}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.artifact","agent_id":"{{BRANCH_ID}}","payload":{"status":"done","exports":[],"modified_files":[]}}'
```

### Report blocked (when you need something from another agent)

```bash
curl -s -X POST ${GIT_PAW_BROKER_URL}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.blocked","agent_id":"{{BRANCH_ID}}","payload":{"needs":"<what you need>","from":"<agent-id>"}}'
```

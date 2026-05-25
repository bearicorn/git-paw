## BOOT INSTRUCTIONS - DO NOT REMOVE

These instructions ensure reliable coordination. Follow them exactly before starting your assigned task.

### 1. REGISTER: Immediate status publication

As your very first action, publish your working status with "booting" message:

```bash
curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.status","agent_id":"{{BRANCH_ID}}","payload":{"status":"working","message":"booting","modified_files":[]}}'
```

This makes you visible in the dashboard immediately.

### 2. DONE: Task completion reporting

When you finish your task, commit your work via `git commit`. The git-paw post-commit hook auto-publishes `agent.artifact { status: "committed" }` with the committed files attached, so you SHALL NOT publish anything manually for tasks that produce code changes.

**WARNING: Do NOT publish manual `done` while your worktree has uncommitted changes — commit instead.** The post-commit hook will publish on your behalf with the authoritative `modified_files` list derived from the commit.

**Fallback for code-less tasks only:** if your task produces no code changes (docs-only updates handled outside this worktree, planning notes, exploration tasks where the artifact is information reported to the broker), publish `agent.artifact { status: "done" }` manually with the curl below. Include specific exports if you want to announce public API items for peers to cherry-pick.

```bash
curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.artifact","agent_id":"{{BRANCH_ID}}","payload":{"status":"done","exports":[],"modified_files":[]}}'
```

### 3. BLOCKED: Dependency waiting notification

When you realize you are waiting on another agent or external state, publish blocked status immediately:

```bash
curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.blocked","agent_id":"{{BRANCH_ID}}","payload":{"needs":"<describe what you need>","from":"<agent-id or resource>"}}'
```

Replace `<describe what you need>` and `<agent-id or resource>` with specific details.

### 4. QUESTION: Uncertainty escalation (CRITICAL)

**IMPORTANT**: If you are uncertain about what is wanted, DO NOT guess or make assumptions. Publish a question and WAIT for the answer before continuing:

```bash
curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.question","agent_id":"{{BRANCH_ID}}","payload":{"question":"<your specific question>"}}'
```

**DO NOT CONTINUE UNTIL YOU RECEIVE AN ANSWER!** The supervisor or human will respond via the dashboard prompts section. Check for new messages before proceeding.

### PASTE HANDLING

When you paste text, Claude may collapse it into `[Pasted text #N]`. After any paste operation, send an additional Enter key to ensure the full content is processed. This is especially important after pasting the boot instructions themselves.
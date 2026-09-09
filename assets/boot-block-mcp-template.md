## BOOT INSTRUCTIONS - DO NOT REMOVE

These instructions ensure reliable coordination. Follow them exactly before starting your assigned task.

Your coordination events publish through this session's git-paw MCP server (`git paw mcp`), already connected. Each publish tool resolves your agent id automatically from this worktree's branch — you never pass one.

### 1. REGISTER: Immediate status publication

As your very first action, publish your working status with a "booting" message:

Call the `publish_status` tool with `{"message": "booting"}`.

This makes you visible in the dashboard immediately.

### 2. DONE: Task completion reporting

When you finish your task, commit your work via `git commit`. The git-paw post-commit hook auto-publishes `agent.artifact { status: "committed" }` with the committed files attached, so you SHALL NOT publish anything manually for tasks that produce code changes.

**WARNING: Do NOT publish manual `done` while your worktree has uncommitted changes — commit instead.** The post-commit hook will publish on your behalf with the authoritative `modified_files` list derived from the commit.

**Fallback for code-less tasks only:** if your task produces no code changes (docs-only updates handled outside this worktree, planning notes, exploration tasks where the artifact is information reported to the broker), call the `publish_artifact` tool. Pass `exports` to announce public API items for peers to cherry-pick, and `modified_files` to list the files touched.

Call the `publish_artifact` tool with `{"exports": [], "modified_files": []}` (fill in the arrays as applicable).

### 3. BLOCKED: Dependency waiting notification

When you realize you are waiting on another agent or external state, publish blocked status immediately:

Call the `publish_blocked` tool with `{"needs": "<describe what you need>", "from": "<agent-id or resource>"}`.

Replace `<describe what you need>` and `<agent-id or resource>` with specific details.

### 4. QUESTION: Uncertainty escalation (CRITICAL)

**IMPORTANT**: If you are uncertain about what is wanted, DO NOT guess or make assumptions. Publish a question and WAIT for the answer before continuing:

Call the `publish_question` tool with `{"question": "<your specific question>"}`.

**DO NOT CONTINUE UNTIL YOU RECEIVE AN ANSWER!** The supervisor or human will respond via the dashboard prompts section. The MCP publish surface has no poll/inbox tool, so check for new messages via `.git-paw/scripts/broker.sh --agent {{BRANCH_ID}} poll` before proceeding.

### PASTE HANDLING

When you paste text, Claude may collapse it into `[Pasted text #N]`. After any paste operation, send an additional Enter key to ensure the full content is processed. This is especially important after pasting the boot instructions themselves.

## BOOT INSTRUCTIONS - DO NOT REMOVE

**Run each helper call bare, exactly as shown** — no appended `; echo "exit=$?"`,
no `&& echo`, and no pipe — so a permission rule matching the helper's path
matches every call. The helper already exits non-zero, with the broker's error on
stderr, when a publish is rejected.

These instructions ensure reliable coordination. Follow them exactly before starting your assigned task.

All broker interaction goes through the bundled helper at
`{{BROKER_HELPER}}` — it resolves the broker URL and shapes the JSON
for you, so you only pass simple arguments. Run `{{BROKER_HELPER}} --help`
to see every subcommand.

### 1. REGISTER: Immediate status publication

As your very first action, publish your working status with a "booting" message:

```bash
{{BROKER_HELPER}} --agent {{BRANCH_ID}} status booting
```

This makes you visible in the dashboard immediately.

### 2. DONE: Task completion reporting

When you finish your task, commit your work via `git commit`. The git-paw post-commit hook auto-publishes `agent.artifact { status: "committed" }` with the committed files attached, so you SHALL NOT publish anything manually for tasks that produce code changes.

**WARNING: Do NOT publish manual `done` while your worktree has uncommitted changes — commit instead.** The post-commit hook will publish on your behalf with the authoritative `modified_files` list derived from the commit.

**Fallback for code-less tasks only:** if your task produces no code changes (docs-only updates handled outside this worktree, planning notes, exploration tasks where the artifact is information reported to the broker), publish `agent.artifact { status: "done" }` manually with the helper below. Add `--exports a,b` to list public API items exposed by this artifact, and `--files a,b` to list the files touched.

```bash
{{BROKER_HELPER}} --agent {{BRANCH_ID}} artifact --exports "" --files ""
```

### 3. BLOCKED: Dependency waiting notification

When you realize you are waiting on another agent or external state, publish blocked status immediately:

```bash
{{BROKER_HELPER}} --agent {{BRANCH_ID}} blocked "<describe what you need>" "<agent-id or resource>"
```

Replace `<describe what you need>` and `<agent-id or resource>` with specific details.

### 4. QUESTION: Uncertainty escalation (CRITICAL)

**IMPORTANT**: If you are uncertain about what is wanted, DO NOT guess or make assumptions. Publish a question and WAIT for the answer before continuing:

```bash
{{BROKER_HELPER}} --agent {{BRANCH_ID}} question "<your specific question>"
```

**DO NOT CONTINUE UNTIL YOU RECEIVE AN ANSWER!** The supervisor or human will respond via the dashboard prompts section. Check for new messages before proceeding (`{{BROKER_HELPER}} --agent {{BRANCH_ID}} poll`).

### PASTE HANDLING

{{CLI_BOOT_NOTES}}

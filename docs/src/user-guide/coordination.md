# Agent Coordination

When multiple AI agents work in parallel, they benefit from knowing what the others are doing. The coordination broker is a lightweight HTTP server that lets agents share status updates, publish artifacts, and flag blockers -- all without touching git.

## Enabling the Broker

Add a `[broker]` section to your `.git-paw/config.toml`:

```toml
[broker]
enabled = true
```

When you run `git paw start`, pane 0 becomes a dashboard instead of an agent pane. The dashboard hosts the broker and displays a live status table.

## How Agents Discover the Broker

git-paw sets the `GIT_PAW_BROKER_URL` environment variable in every agent pane. Agents use this URL to send and receive messages. A typical value is `http://127.0.0.1:9119`.

When skill templates are enabled (the default), each agent's `AGENTS.md` also contains curl commands for interacting with the broker, so agents know how to use it without any manual setup.

## Boot-Prompt Injection

To ensure reliable agent self-reporting, git-paw automatically injects a standardized boot instruction block into every agent's initial prompt. This boot block contains pre-expanded curl commands for four essential operations:

### 1. REGISTER - Immediate Status Publication

Agents automatically publish their working status with a "booting" message as their very first action:

```bash
curl -s -X POST http://127.0.0.1:9119/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.status","agent_id":"feat-auth","payload":{"status":"working","message":"booting","modified_files":[]}}'
```

### 2. DONE - Task Completion Reporting

Agents know how to report task completion:

```bash
curl -s -X POST http://127.0.0.1:9119/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.artifact","agent_id":"feat-auth","payload":{"status":"done","exports":[],"modified_files":[]}}'
```

### 3. BLOCKED - Dependency Waiting Notification

Agents can properly declare when they're waiting on dependencies:

```bash
curl -s -X POST http://127.0.0.1:9119/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.blocked","agent_id":"feat-api","payload":{"needs":"auth token format","from":"feat-auth"}}'
```

### 4. QUESTION - Uncertainty Escalation (Critical)

Agents are instructed to publish questions and wait for answers rather than guessing:

```bash
curl -s -X POST http://127.0.0.1:9119/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.question","agent_id":"feat-auth","payload":{"question":"Should the JWT use RS256 or HS256 signing?"}}'
```

**IMPORTANT**: The boot block explicitly instructs agents: "DO NOT CONTINUE UNTIL YOU RECEIVE AN ANSWER!"

### Boot Block Injection Modes

- **Supervisor Mode**: Boot block is prepended to each agent's task prompt before injection
- **Manual Broker Mode**: Boot block is pre-filled into each agent pane's input line (user pastes task after boot instructions)

### Paste Handling

The boot block includes instructions for proper paste handling, particularly the requirement to send an additional Enter key after paste operations to ensure full content processing.

### Benefits

- **Reliable Monitoring**: Agents self-report immediately on boot
- **Consistent Behavior**: All agents follow the same coordination pattern
- **No Permission Prompts**: Pre-expanded curl commands avoid shell variable expansion issues
- **Supervisor Visibility**: Questions and blockers surface to the dashboard promptly
- **Audit Trail**: All boot operations are logged in the broker log

## Message Types

Agents communicate through three message types:

### Status

An agent reports what it is currently doing.

```bash
curl -s -X POST "$GIT_PAW_BROKER_URL/publish" \
  -H "Content-Type: application/json" \
  -d '{"agent_id": "feat/auth", "kind": "status", "body": "implementing login endpoint"}'
```

### Artifact

An agent shares a result that other agents may need -- a file path, an API contract, a type definition.

```bash
curl -s -X POST "$GIT_PAW_BROKER_URL/publish" \
  -H "Content-Type: application/json" \
  -d '{"agent_id": "feat/auth", "kind": "artifact", "body": "auth token format: JWT with sub, exp, iat claims"}'
```

### Blocked

An agent declares that it is waiting on something from another agent.

```bash
curl -s -X POST "$GIT_PAW_BROKER_URL/publish" \
  -H "Content-Type: application/json" \
  -d '{"agent_id": "feat/api", "kind": "blocked", "body": "waiting for auth token format from feat/auth"}'
```

## Polling for Messages

Agents poll for messages from other agents using cursor-based pagination. The `since` parameter is a sequence number -- the broker returns only messages with a sequence greater than the given value.

```bash
# First poll -- get all messages
curl -s "$GIT_PAW_BROKER_URL/messages/feat-auth?since=0"
```

The response includes a `last_seq` field. Pass this value as `since` on the next poll to get only new messages:

```bash
# Subsequent poll -- only new messages since last check
curl -s "$GIT_PAW_BROKER_URL/messages/feat-auth?since=42"
```

This cursor-based approach is lossless -- no messages are missed between polls, regardless of timing.

## Checking Overall Status

The `/status` endpoint returns a summary of all agents and their latest state:

```bash
curl -s "$GIT_PAW_BROKER_URL/status"
```

## Multi-Repo Considerations

Each git-paw session runs its own broker. If you have multiple repos running sessions simultaneously, each needs a unique port:

```toml
# In repo-a/.git-paw/config.toml
[broker]
enabled = true
port = 9119

# In repo-b/.git-paw/config.toml
[broker]
enabled = true
port = 9120
```

The default port is `9119`. The broker always binds to `127.0.0.1` (localhost only) and should never be exposed to the network.

## Audit Trail

The broker writes all messages to `.git-paw/broker.log` as JSONL (one JSON object per line). This file is flushed every 5 seconds and provides a complete audit trail of agent communication.

The log file is automatically cleaned up by `git paw purge`. It is also covered by the `.gitignore` entry that `git paw init` creates.

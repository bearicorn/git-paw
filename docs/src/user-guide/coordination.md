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

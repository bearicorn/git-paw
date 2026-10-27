# Remote Control

You can drive a git-paw session from a phone or another device. The orchestrator is your own agent session, so if that session can be remote-controlled, the whole run can be steered from anywhere. This chapter covers how.

## The topology

```text
phone / browser ──(the agent CLI's own remote control)──▶ your driving agent session
                                                            (local machine, repo root,
                                                             git-paw MCP server attached)
                                                                    │
                                                                    ▼
                                                     git-paw session (tmux: dashboard
                                                     at pane 0, coding agents from pane 1)
```

- **The driving session runs on your local machine**, at the repository root and outside tmux. git-paw's MCP server is attached to it, and it has loaded the orchestrator skill (`.git-paw/skills/orchestrator/SKILL.md`) and the session context (`.git-paw/AGENTS.local.md`).
- **You remote-control that session** through the agent CLI's native remote-control feature. Your phone is only a view into the local session. The session's files, MCP servers and tools stay on the machine.
- **The driving session drives git-paw** the way it does when you sit at the keyboard. It uses the bundled helpers (`.git-paw/scripts/sweep.sh`, `broker.sh`) and the MCP tools: `wait_for_events` and `get_session_status` to follow the run, and, in the root session, `start_session`, `pause_session`, `stop_session`, `purge_session`, `publish_verified` and `publish_feedback`.

**git-paw ships no bridge, tunnel or network transport of its own.** The remote channel belongs to the agent CLI vendor, and git-paw opens no port for it. Pair the driving session with `git paw start --unattended` so the coding agents keep moving while you are away. Each permission prompt the drive loop escalates reaches you through the driving session, and you answer it from the phone.

## Which agent CLIs can be the remote-controlled driving session

**Last checked: 2026-10-29.** These are third-party facts that change quickly; check the vendor's current documentation before relying on a row.

| Agent CLI | Remote-controlled driving session | Notes |
|---|---|---|
| **Claude Code** | ✅ `claude remote-control`, or `/remote-control` in a running session | Continues the local session from the Claude app or the web. Local MCP servers stay available. Requires a paid plan and `/login`. |
| **GitHub Copilot CLI** | ✅ `copilot --remote`, or `/remote on` in a running session | Steered from GitHub Mobile or github.com. Whether local MCP tools are usable remotely is *to verify*; the shell-helper path works regardless. |
| **OpenAI Codex** | ✅ through the **Codex desktop app**, steered from the ChatGPT mobile app (preview) | The `codex` terminal CLI has no remote control of its own; drive git-paw from the desktop app. MCP over the remote channel is *to verify*. |
| **Cursor** | ✅ `/remote-control` in a desktop agent session, steered from the Cursor iOS app (public beta) | Requires a paid plan and a workspace with a Git remote. The agent loop runs in Cursor's cloud and tool calls run on your machine. MCP over the remote channel is *to verify*. |
| **Qwen Code** | ⚠️ *to verify* | Earlier research (2026-07) found text-only messaging integrations, not a native remote control. |
| **Antigravity `agy`** | ❌ *to verify* | No native remote control was found (2026-07). |
| **Amp / Aider** | ❌ *to verify* | No native remote control was found (2026-07). |

A CLI without native remote control can still drive git-paw locally. git-paw does not build a bridge to make it remote-controllable.

## Caveats

- **The local process must stay running.** The remote control is a view into a process on your machine. If the machine sleeps, loses its network or the process exits, the remote session stops with it. An outage of roughly **ten minutes** ends the remote session (Claude Code's documented timeout; other vendors behave similarly), and you have to restart it locally. Keep the machine awake for long runs.
- **Secure the channel yourself.** Remote control extends the blast radius to whoever can reach that channel. Treat your vendor account and its sessions as access to your repository. See [Security posture](security-posture.md).
- **Remote approval is still your approval.** The orchestrator relays every escalated permission prompt to you, and runs `sweep.sh approve` only when you say so, whether you are at the keyboard or on your phone.

## See also

- [Quick Start: Orchestrator Mode](../quick-start-orchestrator.md): init, start, load the skill, follow, verify, merge.
- [MCP Server](mcp.md): the read and write tools the driving session uses.
- [Supervisor → Driving the session from your own agent](supervisor.md#driving-the-session-from-your-own-agent).

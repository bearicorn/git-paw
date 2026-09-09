## 1. Security boundary first

- [x] 1.1 Define the four publish tools as four distinct tool definitions — NOT one generic `publish(type, payload)` (D2). The type must not be expressible as a parameter.
- [x] 1.2 Confirm no tool schema contains an `agent_id` parameter (D3).
- [x] 1.3 Adversarial test: no advertised tool can publish `agent.verified`.
- [x] 1.4 Adversarial test: no advertised tool can publish `agent.feedback`.
- [x] 1.5 Adversarial test: a session resolved to one worktree cannot publish a message attributed to another agent.
- [x] 1.6 Adversarial test: the registry contains no file-write, git-mutation, or config-write tool.

## 2. Publish tools

- [x] 2.1 Implement the status/register publish tool.
- [x] 2.2 Implement the artifact/done publish tool.
- [x] 2.3 Implement the blocked publish tool.
- [x] 2.4 Implement the question publish tool.
- [x] 2.5 Resolve the publishing `agent_id` from the session's worktree branch via the existing `slugify_branch` conversion.
- [x] 2.6 Each tool advertises a JSON Schema for parameters and return shape, per the existing registry requirement.
- [x] 2.7 Test: each tool's published message is wire-identical to the equivalent `broker.sh` invocation (D5).

## 3. Failure handling

- [x] 3.1 Broker unreachable → MCP-level error, server keeps running.
- [x] 3.2 Broker rejects the message → MCP-level error, nothing written to stdout.
- [x] 3.3 Test: stdout carries only protocol frames across both failure paths.

## 4. Boot block dual form

- [x] 4.1 Extend `build_boot_block` to render the MCP tool form for an MCP-capable CLI.
- [x] 4.2 Default to the helper-script form when MCP capability is unknown or absent (D4).
- [x] 4.3 Test: MCP-capable CLI gets the tool form; non-MCP CLI gets the helper form; unknown capability gets the helper form.
- [x] 4.4 Test: both forms cover exactly the four events — neither adds nor omits relative to the other.
- [x] 4.5 Confirm the `broker.sh` path is unchanged and still works end to end.

## 5. E2E

- [x] 5.1 Cross-module E2E: MCP tool call → broker publish → delivery → peer poll returns the message.
- [x] 5.2 E2E: an agent booting via the MCP form registers with the broker without any shell permission grant.

## 6. Docs

- [x] 6.1 MCP tool reference gains the publish category and states the read/write boundary explicitly.
- [x] 6.2 Document why the supervisor authority verbs are absent — so the omission reads as deliberate, not as a gap to fill later.
- [x] 6.3 Boot-block chapter documents both forms and the fallback rule.
- [x] 6.4 `mdbook build docs/` succeeds.

## 7. Quality gates

- [x] 7.1 `cargo fmt` before committing.
- [x] 7.2 `just check` — verify by real exit code, not piped output.
- [x] 7.3 Confirm no `BrokerMessage` variant was added or removed; if one becomes necessary, stop and scope the enum ripple per the AGENTS.md checklist first.
- [x] 7.4 Security review against `.agents/skills/security-and-safety-review/SKILL.md`, focused on the new mutation surface and D1–D3.

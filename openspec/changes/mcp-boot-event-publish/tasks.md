## 1. Security boundary first

- [ ] 1.1 Define the four publish tools as four distinct tool definitions — NOT one generic `publish(type, payload)` (D2). The type must not be expressible as a parameter.
- [ ] 1.2 Confirm no tool schema contains an `agent_id` parameter (D3).
- [ ] 1.3 Adversarial test: no advertised tool can publish `agent.verified`.
- [ ] 1.4 Adversarial test: no advertised tool can publish `agent.feedback`.
- [ ] 1.5 Adversarial test: a session resolved to one worktree cannot publish a message attributed to another agent.
- [ ] 1.6 Adversarial test: the registry contains no file-write, git-mutation, or config-write tool.

## 2. Publish tools

- [ ] 2.1 Implement the status/register publish tool.
- [ ] 2.2 Implement the artifact/done publish tool.
- [ ] 2.3 Implement the blocked publish tool.
- [ ] 2.4 Implement the question publish tool.
- [ ] 2.5 Resolve the publishing `agent_id` from the session's worktree branch via the existing `slugify_branch` conversion.
- [ ] 2.6 Each tool advertises a JSON Schema for parameters and return shape, per the existing registry requirement.
- [ ] 2.7 Test: each tool's published message is wire-identical to the equivalent `broker.sh` invocation (D5).

## 3. Failure handling

- [ ] 3.1 Broker unreachable → MCP-level error, server keeps running.
- [ ] 3.2 Broker rejects the message → MCP-level error, nothing written to stdout.
- [ ] 3.3 Test: stdout carries only protocol frames across both failure paths.

## 4. Boot block dual form

- [ ] 4.1 Extend `build_boot_block` to render the MCP tool form for an MCP-capable CLI.
- [ ] 4.2 Default to the helper-script form when MCP capability is unknown or absent (D4).
- [ ] 4.3 Test: MCP-capable CLI gets the tool form; non-MCP CLI gets the helper form; unknown capability gets the helper form.
- [ ] 4.4 Test: both forms cover exactly the four events — neither adds nor omits relative to the other.
- [ ] 4.5 Confirm the `broker.sh` path is unchanged and still works end to end.

## 5. E2E

- [ ] 5.1 Cross-module E2E: MCP tool call → broker publish → delivery → peer poll returns the message.
- [ ] 5.2 E2E: an agent booting via the MCP form registers with the broker without any shell permission grant.

## 6. Docs

- [ ] 6.1 MCP tool reference gains the publish category and states the read/write boundary explicitly.
- [ ] 6.2 Document why the supervisor authority verbs are absent — so the omission reads as deliberate, not as a gap to fill later.
- [ ] 6.3 Boot-block chapter documents both forms and the fallback rule.
- [ ] 6.4 `mdbook build docs/` succeeds.

## 7. Quality gates

- [ ] 7.1 `cargo fmt` before committing.
- [ ] 7.2 `just check` — verify by real exit code, not piped output.
- [ ] 7.3 Confirm no `BrokerMessage` variant was added or removed; if one becomes necessary, stop and scope the enum ripple per the AGENTS.md checklist first.
- [ ] 7.4 Security review against `.agents/skills/security-and-safety-review/SKILL.md`, focused on the new mutation surface and D1–D3.

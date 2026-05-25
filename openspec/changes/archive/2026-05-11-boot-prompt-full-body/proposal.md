## Why

`cmd_supervisor` constructs each agent's launch prompt as `boot_block + "\n\n" + task_prompt` (`src/main.rs:813-823`). When a spec is associated with the branch, `task_prompt` is set to **only the first line** of the spec's full body via `spec_entry.prompt.lines().next()`. The remainder of the spec body — every task, every acceptance criterion, every design note — is silently dropped from the injected prompt. The full content is still written to the worktree's `AGENTS.md`, but the boot prompt itself never tells the agent that AGENTS.md exists or that the first line is just a heading.

Dogfood evidence (v0.5.0, 11-agent supervisor session): 7 of 11 agents published `agent.question` events whose payloads read variants of "task description appears truncated — it ends at the heading `## 1. Code fix in cmd_supervisor` but no instructions follow". Every agent saw their boot prompt cut off at the first `## 1.` heading and asked the same question. The agents were correct — the prompt WAS truncated by the launcher; the AGENTS.md fallback that *would* have helped them was overridden by the buggy line.

When no spec is present, the code already falls back to a useful default: `"Begin your assigned task as described in AGENTS.md."`. The bug is that this fallback is unreachable whenever a spec is present, even though the same AGENTS.md-pointer guidance is what the agent actually needs.

## What Changes

**Code fix in `cmd_supervisor` task-prompt construction** (`src/main.rs:813-823`):

Today:
```rust
// Initial prompt: spec title/description if present, else default.
let task_prompt = spec_entry
    .map(|s| s.prompt.lines().next().unwrap_or("").trim().to_string())
    .filter(|p| !p.is_empty())
    .unwrap_or_else(|| "Begin your assigned task as described in AGENTS.md.".to_string());
```

After:
```rust
// Initial prompt: always point the agent at AGENTS.md (the full spec body
// is written there by setup_worktree_agents_md). When a spec is associated
// with the branch, include the spec ID so the agent knows which
// openspec/changes/<id>/ directory to consult for additional artifacts
// (proposal, design, specs/, tasks).
let task_prompt = match spec_entry {
    Some(s) => format!(
        "Begin your assigned task. The full spec is in AGENTS.md in this worktree. \
         Additional artifacts (proposal, design, specs, tasks) live under \
         openspec/changes/{id}/ — read them all before starting.",
        id = s.id,
    ),
    None => "Begin your assigned task as described in AGENTS.md.".to_string(),
};
```

The spec body remains the source of truth for AGENTS.md via `WorktreeAssignment.spec_content`; only the injected boot prompt changes. The agent's first action (after publishing `agent.status: booting`) is to read AGENTS.md, which it now knows exists.

**Affected sites NOT changed:**

- `cmd_start_from_specs` (`src/main.rs:1206`) doesn't construct a `task_prompt` at all — non-supervisor mode injects the boot block via `tmux::build_boot_inject_args` (literal-mode, no Enter) and lets the user type. Agents in that flow rely entirely on AGENTS.md and the user's first message.
- `cmd_start` (the non-from-specs path) has no spec entries at all — no task-prompt construction needed.
- `WorktreeAssignment.spec_content` (`src/main.rs:782`) still receives the full spec body. AGENTS.md generation is unchanged.

**Not in scope:**

- Pasting the full spec body into the agent's pane via `tmux send-keys`. The body is often thousands of characters; that paste would land in Claude's paste-buffer (the very trap the supervisor skill recovers from) and cost wall-clock time and complexity to no benefit, since the same content is already in AGENTS.md which Claude auto-reads on startup.
- Per-spec custom boot prompts (e.g. a `paw_boot_prompt` frontmatter field that overrides the default). Belongs in v1.0.0 alongside the per-CLI hook providers.
- Removing the now-unused first-line-truncation path in tests. The behaviour the fix replaces had no tests; nothing to remove.

## Capabilities

### New Capabilities
*(none — fixes an existing capability)*

### Modified Capabilities

- `supervisor-launch`: the existing "Initial prompt injection via tmux send-keys" requirement gains a scenario stating that when a spec is associated with the branch, the injected task prompt SHALL point the agent at AGENTS.md and SHALL include the spec ID for openspec change-directory discovery. The first-line-of-spec-body behaviour is replaced.

## Impact

**Code**:
- `src/main.rs::cmd_supervisor` — replace the `spec_entry.map(|s| s.prompt.lines().next()...)` chain with the spec-ID-pointer construction shown above. ~10 lines changed; net deletion of the `.filter().unwrap_or_else()` chain.

**Tests**:
- Unit test in `src/main.rs::tests`: `task_prompt_with_spec_points_at_agents_md_and_includes_id` — constructs a `SpecEntry` with id `"my-change"` and any prompt body, calls the (extracted) `build_task_prompt` helper, asserts the returned string contains `AGENTS.md`, `openspec/changes/my-change`, and does NOT include the spec's first body line in raw form.
- Unit test: `task_prompt_without_spec_uses_default_agents_md_fallback` — calls `build_task_prompt(None)`, asserts the returned string is the existing default `"Begin your assigned task as described in AGENTS.md."`.
- Behavioural test in `tests/from_specs_launch_fixes_integration.rs` or similar: extract `build_task_prompt` as a pure function so unit tests can assert its output directly without launching tmux.

**Backward compatibility**: from the agent's perspective this is strictly better — the injected prompt now points at content the agent can read instead of being a truncated heading. From the launcher's perspective the change touches only the prompt-string-construction; tmux invocation shape is unchanged.

**Mismatches resolved**:
- MILESTONE drift item 29 (boot-prompt truncation — `task_prompt` keeps only first line of spec content): resolved.
- Dogfood pattern where every agent immediately publishes `agent.question` asking "what should I do?": eliminated. The agent now knows to read AGENTS.md + `openspec/changes/<id>/`.

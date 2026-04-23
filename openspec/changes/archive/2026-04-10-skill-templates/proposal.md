## Why

v0.3.0 agents need to know how to talk to the broker — which endpoints to POST to, which env vars carry the broker URL, which ID to use as their `agent_id`. Hard-coding these instructions into git-paw's source would force a release for every change. This change introduces an editable, templated skill file that git-paw renders per worktree at session launch, embedded as a default but overridable by the user. The skill content is `curl`-based and CLI-agnostic; this change establishes the loading and rendering mechanism that future v0.4+ skills (verification, governance, escalation) will reuse.

## What Changes

- Add a new module `src/skills.rs` exposing a `SkillTemplate` type, a `resolve(skill_name)` function, and a `render(template, branch, broker_url)` function
- Embed the v0.3.0 coordination skill at compile time via `include_str!`. The default file lives at `assets/agent-skills/coordination.md` and contains the broker coordination instructions as `curl` commands
- Implement template substitution: replace `{{BRANCH_ID}}` (using `slugify_branch` from `message-types`) and replace `{{GIT_PAW_BROKER_URL}}` with the actual broker URL at render time so the agent's curl commands contain a literal URL
- Implement user-override resolution. For a given skill name, the loader looks for the skill file in this order and uses the first match:
  1. `~/.config/git-paw/agent-skills/<skill-name>.md` — user override
  2. Embedded `<skill-name>.md` shipped with the binary — always present, never fails
- Use `dirs::config_dir()` (already in the approved dependency set) to locate the user override directory
- Add unit tests for: embedded coordination skill loads, user override is preferred when present, `{{BRANCH_ID}}` substitution, `{{GIT_PAW_BROKER_URL}}` is substituted at render time, missing user dir is not an error, missing skill name returns a clear error
- This change does NOT modify any existing files. It adds a new module, a new asset directory, and a `mod skills;` declaration in `main.rs`.

**Naming convention:** v0.3.0 ships exactly one skill named `coordination`. Future versions add new skills under their own descriptive names (`verification`, `governance`, `escalation`, etc.) alongside `coordination.md`. There is no per-CLI override mechanism in v0.3.0 — all CLIs receive the same skill content. If per-CLI customization becomes necessary in v0.4+ (e.g. tool-specific instructions for Claude vs. Codex), it can be added later without breaking the v0.3.0 API.

## Capabilities

### New Capabilities

- `agent-skills`: Loading, resolution, and rendering of coordination and other instructional skill templates for agents launched in git-paw worktrees. Covers the embedded defaults shipped in `assets/agent-skills/`, the user-override search in `~/.config/git-paw/agent-skills/`, the substitution rules for `{{BRANCH_ID}}`, the literal pass-through of `${GIT_PAW_BROKER_URL}`, and the public API consumed by `skill-injection` in Wave 2.

### Modified Capabilities

- `error-handling`: Add `SkillError` type with `UnknownSkill` and `UserOverrideRead` variants, wrappable inside `PawError`.

## Impact

- **New file (owned by this change):** `src/skills.rs`
- **New asset directory (owned by this change):** `assets/agent-skills/coordination.md`
- **Modified file:** `src/main.rs` (or `src/lib.rs`) — add `mod skills;` declaration
- **Modified file:** `Cargo.toml` — add `assets/` to `include` so `cargo publish` ships the embedded skill files
- **No new runtime dependencies.** Uses existing `dirs` and `std::fs` only. No new approved-set additions needed.
- **Depends on:** `message-types` (for `slugify_branch`) — must merge first.
- **Dependents:** `skill-injection` (Wave 2) — calls `skills::resolve("coordination")` and `skills::render` to produce the text appended to worktree AGENTS.md.
- **No CLI surface changes in this change.** No new commands, no new flags, no new config fields. Skill resolution happens implicitly during session launch via the public API exposed here.
- **User-facing files:** the embedded `coordination.md` is the canonical example; users learn the override path (`~/.config/git-paw/agent-skills/coordination.md`) from the mdBook documentation that `skill-injection` will update.

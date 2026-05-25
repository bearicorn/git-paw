## Context

`cmd_supervisor` constructs each agent's first launch message in two steps: a standardized "boot block" (the four coordination curls: register / done / blocked / question, plus paste-handling guidance) and an appended "task prompt" describing what the agent should actually do. The boot block is well-tested and uniform across agents. The task prompt is per-agent: when a spec is associated with the branch (the `--from-specs` path), the prompt is derived from the spec; when no spec is associated (the bare-`--branches` path), the prompt is the default `"Begin your assigned task as described in AGENTS.md."`.

The derivation when a spec is present is the bug: `spec_entry.prompt.lines().next().unwrap_or("").trim().to_string()`. `SpecEntry::prompt` carries the *full* spec body (the OpenSpec backend concatenates `tasks.md` + `proposal.md` + `specs/**/spec.md` etc.; the Markdown backend uses the file body). `.lines().next()` keeps only the first non-empty line, which for an OpenSpec change is the first heading from `tasks.md` (typically `## 1. <section title>`). Every byte of body context is dropped.

Concretely, during v0.5.0 dogfood:

| Spec | task_prompt the agent received |
|---|---|
| `governance-config` | `## 1. Struct definitions` |
| `forward-coordination` | `## 1. Broker message type` |
| `no-supervisor-flag` | `## 1. CLI flag definition` |
| `supervisor-as-pane` | `## 1. Layout calculation helpers` |
| `prompt-submit-fix` | `## 1. Code fix in cmd_supervisor` |
| `learnings-mode` | `## 1. Configuration` |

Every agent published the same shape of `agent.question`: "the prompt ends with `## 1. <thing>` but no body follows — please paste the full task". The agents were right. The supervisor (acting manually) had to push the workaround "read AGENTS.md and openspec/changes/<id>/" to each one before they could proceed.

The full spec body IS already written to `AGENTS.md` in the agent's worktree via `git_paw::agents::setup_worktree_agents_md` (called at `src/main.rs:793`). Claude Code (and equivalents) auto-load `AGENTS.md` on startup — so the body is in the agent's context. The agent just doesn't know to look there.

## Goals / Non-Goals

**Goals:**
- The boot-time `task_prompt` always points the agent at AGENTS.md as the source of task content (no more silent truncation to a heading).
- When a spec is associated with the branch, the prompt includes the spec ID so the agent knows which `openspec/changes/<id>/` directory holds additional artifacts (proposal, design, specs, tasks).
- The fix is minimal: a single function's worth of code change in `cmd_supervisor`, no impact on AGENTS.md generation or tmux invocation shape.

**Non-Goals:**
- Sending the full spec body via `tmux send-keys`. The body would trigger Claude's paste-buffer (drift item 29 already documents that recovery path), would be subject to tmux argv length limits, and is redundant with AGENTS.md.
- Per-spec custom prompts (e.g. a `paw_boot_prompt` frontmatter override). Out of scope; if dogfood shows users want it, v1.0.0 alongside per-CLI hook providers.
- Pre-validation that AGENTS.md exists before the boot prompt is sent. `setup_worktree_agents_md` already produces an error if writing fails; the prompt path can assume success.
- Changes to `cmd_start_from_specs` (the non-supervisor `--from-specs` path) — it doesn't construct a task prompt at all; the user types their first message after attach.
- Changes to the boot block template itself. The boot block doesn't reference AGENTS.md today (drift candidate for a separate change); this fix puts AGENTS.md guidance in the task-prompt portion instead.

## Decisions

### D1. Always point at AGENTS.md; never inject spec body via send-keys

**Choice:** Replace `spec_entry.map(|s| s.prompt.lines().next()...)` with a fixed message that points the agent at AGENTS.md (and the `openspec/changes/<id>/` directory when a spec is associated). Drop the first-line-of-spec derivation entirely.

**Why:**
- `AGENTS.md` already contains the full spec body. Every supported CLI (Claude Code, Codex, Gemini, etc.) auto-loads `AGENTS.md` at startup. Telling the agent "read AGENTS.md" is sufficient and avoids duplicating content into the boot prompt.
- The first-line derivation had no observable upside: the heading text is already inside the AGENTS.md the agent will read seconds later. Dropping it removes the truncation pattern without losing information.
- Including the spec ID lets the agent locate sibling artifacts (`proposal.md`, `design.md`, `specs/**/spec.md`) in the openspec change directory — useful when the agent's first read of AGENTS.md raises design questions answerable from the proposal.

**Alternatives considered:**
- *Send the full `s.prompt` body as the task prompt* — Rejected. Triggers Claude's paste-buffer (the recovery patterns in the supervisor skill handle this, but it's wasted work to opt into the trap). Costs wall-clock time on each launch. Also doubles up content with AGENTS.md.
- *Send the first line + a "(see AGENTS.md for the rest)" suffix* — Marginally better than today (the suffix at least points somewhere) but still confusing — the first line is a heading without context, and agents may try to act on the heading literally. Cleaner to drop the heading entirely.
- *Auto-generate a one-paragraph summary of the spec body and use that as the prompt* — Requires an LLM call at launch time, or a per-spec frontmatter field with a hand-written summary. Either way it's per-spec authoring effort that the existing `AGENTS.md` already gives us for free.
- *Embed a heuristic title in the boot prompt: parse the change's `proposal.md` "## Why" first paragraph instead of `tasks.md`'s first heading* — Rejected. The "Why" content is supposed to be ≤1000 chars per OpenSpec rules, but it's still long enough to trigger paste-buffer, and it's parser-dependent. Pointing at AGENTS.md is robust against per-format quirks.

### D2. Extract a pure `build_task_prompt` helper for testability

**Choice:** Move the task-prompt construction out of the `cmd_supervisor` body into a pure helper function `build_task_prompt(spec_entry: Option<&SpecEntry>) -> String` at module scope in `src/main.rs`. The helper is `pub(crate)` so the test module can call it without invoking tmux.

**Why:**
- The construction is pure (no I/O, no globals); extracting it makes it unit-testable without needing the full `cmd_supervisor` rigging (tmux session, broker, dashboard).
- Matches the pattern set by `resolve_dispatch_target` and `is_interactive_stdin` from the `from-specs-launch-fixes` change — both are small pure helpers in `main.rs` with focused tests.
- Future per-CLI custom prompt support (v1.0.0) can extend this helper without touching the supervisor launch loop.

**Alternatives considered:**
- *Inline the prompt construction in `cmd_supervisor`* — Works but couples the test surface to tmux. Rejected.
- *Move the helper into `src/specs/mod.rs`* — Tempting because the input is a `SpecEntry`, but the output is "what the supervisor pane gets sent on launch" which is a launcher concern, not a spec-parsing concern. Rejected.

### D3. Default prompt for the no-spec case is unchanged

**Choice:** When `spec_entry` is `None`, return the existing string `"Begin your assigned task as described in AGENTS.md."` verbatim.

**Why:**
- The no-spec branch already had the correct behaviour; only the spec-present branch was buggy. Preserving the no-spec string avoids cosmetic churn and keeps any existing tests on the string content passing.
- The no-spec case is reached when the user invokes `--branches feat/foo` without `--from-specs`. In that case there's no spec ID to mention, and `AGENTS.md` contains the rendered skill (no spec body). The existing pointer is exactly what the agent needs.

## Risks / Trade-offs

- **[Agent ignores the AGENTS.md pointer and asks a question anyway]** → Mitigation: every supported CLI auto-reads AGENTS.md on startup; the prompt's instruction "see AGENTS.md" is reinforcing what the CLI already does. If an agent still asks, the supervisor skill's monitoring loop handles `agent.question` events. Net: strictly better than today's pattern (every agent asks).

- **[AGENTS.md write failure is no longer guarded by the prompt fallback]** → Today, if `setup_worktree_agents_md` succeeds the prompt may still be a heading; if it fails the worktree setup errors before the prompt is built, so the same condition applies. The new prompt assumes AGENTS.md is present, but the assumption was already true. No regression.

- **[Future code paths that *don't* call `setup_worktree_agents_md` but reuse `build_task_prompt`]** → The helper assumes AGENTS.md will exist. If a future caller bypasses AGENTS.md generation, the agent will be told to read a file that doesn't exist. Mitigation: the helper's doc comment SHALL state the AGENTS.md prerequisite explicitly; reviewers will catch a violating caller in code review.

- **[Spec ID may contain characters that read awkwardly in a sentence]** → The spec ID is the OpenSpec change-directory name (e.g. `prompt-submit-fix`) or the markdown spec's filename. Both are user-controlled but tend to be kebab-case. The constructed sentence "Additional artifacts live under openspec/changes/prompt-submit-fix/" reads fine. Pathological IDs (containing spaces, quotes, etc.) would already be problematic for git branch names; not a new failure mode.

## Migration Plan

This is a single-function code change. No data, no config, no schema.

1. **Code change** in `src/main.rs`: extract `build_task_prompt(spec_entry: Option<&SpecEntry>) -> String` as a pure helper; replace the inline construction in `cmd_supervisor` with a call to the helper.
2. **Tests** in `src/main.rs::tests`:
   - `task_prompt_with_spec_points_at_agents_md_and_includes_id` — asserts the returned string contains `AGENTS.md`, `openspec/changes/<id>`, and does NOT include the spec's first body line in raw form.
   - `task_prompt_without_spec_uses_default_agents_md_fallback` — asserts the returned string equals the existing default verbatim.
3. **Rollback** — revert the helper extraction and the call-site change. Behavior reverts to the truncation pattern, which is the documented bug; nothing else regresses.

No flag, no opt-in. The fix is unconditional.

## Open Questions

- *Should the boot-block template itself reference AGENTS.md so the AGENTS.md-pointer is reinforced regardless of the task-prompt content?* Plausible improvement but out of scope. Schedule into v0.5.0 cleanup if a future dogfood shows agents still missing AGENTS.md.

- *Should the prompt include the spec body's first heading as a "task focus" hint, even alongside the AGENTS.md pointer?* The first heading is essentially `## 1. <something>` which is the title of the first task section, not a useful task focus. Rejected for now.

- *Should the prompt's spec-directory path adapt to the configured spec backend? (e.g. `.specify/` for Spec Kit)* Out of scope — Spec Kit support is the `spec-kit-format` v0.5.0 change; this helper will need a small extension when that ships to use the right path per backend. Documented in tasks.md as a v0.5.0 follow-up.

## Context

This is the smallest v0.3.0 change — it adds ~20 lines to `src/agents.rs` and ~10 lines to the launch flow in `src/main.rs`. The hard work (template loading, rendering, substitution) is done by `skill-templates`. This change just calls the API and passes the result into the existing AGENTS.md generation machinery.

The v0.2.0 flow already generates per-worktree `AGENTS.md` files with a marker-delimited section containing assignment + spec + file ownership. This change adds one more subsection (the coordination skill) inside those same markers.

## Goals / Non-Goals

**Goals:**

- Append rendered coordination skill text to the worktree AGENTS.md when broker is enabled
- Preserve the exact v0.2.0 output when broker is disabled (no regression)
- Keep the change minimal — no structural refactoring of `agents.rs`

**Non-Goals:**

- Multiple skills per worktree (v0.4+ may add `verification`, `governance`, etc. — this change handles exactly one skill: `coordination`)
- Per-CLI skill differentiation (all CLIs get the same skill in v0.3.0)
- Skill caching or lazy loading — `resolve` is called once per session, render once per worktree

## Decisions

### Decision 1: skill_content as a pre-rendered String on WorktreeAssignment

```rust
pub struct WorktreeAssignment {
    pub branch: String,
    pub cli: String,
    pub spec_content: Option<String>,
    pub owned_files: Option<Vec<String>>,
    pub skill_content: Option<String>,  // NEW
}
```

The caller (launch flow in `main.rs`) resolves and renders the skill before constructing the assignment. `generate_worktree_section` receives the final text and just appends it.

**Why:**
- `generate_worktree_section` stays a pure formatting function — no I/O, no `skills::resolve` calls
- The launch flow controls whether skills are injected (based on `config.broker.enabled`)
- Testing `generate_worktree_section` with skill content is a simple string-in/string-out test

**Alternatives considered:**
- *`generate_worktree_section` calls `skills::resolve` internally.* Mixes I/O (file reading) into a formatting function. Rejected.
- *Separate injection step after `generate_worktree_section`.* Would require a second marker scheme or string search. Rejected — inserting inside the existing markers is cleaner.

### Decision 2: Skill section appended after file ownership, before end marker

The generated section ordering inside the markers:

```markdown
<!-- git-paw:start — managed by git-paw, do not edit manually -->

## git-paw Session Assignment

- **Branch:** `feat/http-broker`
- **CLI:** claude

### Spec
...

### File Ownership
- `src/broker/server.rs`

## Coordination Skills                ← NEW

You are running inside a git-paw worktree as agent `feat-http-broker`. ...
... curl commands ...

<!-- git-paw:end -->
```

**Why:**
- Assignment → Spec → Ownership → Skills is a natural reading order: "who am I → what should I do → what do I own → how to coordinate"
- The `## Coordination Skills` heading is level 2 (same as `## git-paw Session Assignment`) making it a visible peer section in the AGENTS.md
- The existing `generate_worktree_section` builds a string and pushes `END_MARKER` at the end. Adding skill content is one `if let Some(skill) = ... { section.push_str(skill); }` before the end marker push.

## Risks / Trade-offs

- **Skill resolution failure blocks session launch** → If `skills::resolve("coordination")` returns `Err` (e.g. user override is unreadable), the launch flow fails. **Mitigation:** acceptable — a broken skill file is a real configuration error that should surface immediately, not be silently ignored. The error message from `SkillError` identifies the broken file.

- **AGENTS.md grows when skill is injected** → The coordination skill adds ~20 lines of curl examples. For CLIs with context limits, this is a negligible addition. **Mitigation:** not a concern for Claude/Codex/Aider which all handle AGENTS.md content gracefully.

## Migration Plan

No migration. The new `skill_content` field on `WorktreeAssignment` defaults to `None`. Existing call sites that construct `WorktreeAssignment` without the field continue to work — `None` produces the same output as v0.2.0.

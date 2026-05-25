## Why

The v0.5.0 `boot-prompt-full-body` change (archived as
`2026-05-11-boot-prompt-full-body`) fixed the first-line-truncation bug by
giving every spec-driven agent the same generic AGENTS.md pointer:

```
Begin your assigned task. The full spec is in AGENTS.md in this worktree.
Additional artifacts (proposal, design, specs, tasks) live under
openspec/changes/{id}/ — read them all before starting.
```

That string is correct as a fallback, but it leaves a known optimisation on
the table when the backend is OpenSpec. Every git-paw repo that configures
`specs.type = "openspec"` ships with the `opsx:apply` slash-command skill —
an Anthropic-Claude skill that walks an agent through a change's tasks one
at a time, loading the right artifacts at each step. Invoking
`/opsx:apply <change-id>` as the boot prompt drops the agent straight into
that workflow without a manual "read all of AGENTS.md and decide where to
start" step.

Dogfood evidence from the v0.5.0 session (2026-05-11): four of the agents
running on OpenSpec specs spent their first 30–90 seconds enumerating the
files under `openspec/changes/<id>/` and re-deriving the order in which to
read them, even though `/opsx:apply` already encodes that workflow. The
user's intent (captured in-session): *"When the spec configured in the
config is openspec then the injected initial prompt should be opsx apply
skill instead of the full custom injection."*

The Markdown backend (`MarkdownBackend`) and the upcoming Spec Kit backend
(`SpecKitBackend` in flight under `spec-kit-format`) do not currently have
a comparable slash-command apply workflow. For those backends the v0.5.0
generic AGENTS.md pointer remains the right answer; this change only
specialises the OpenSpec case. A symmetric Spec Kit slash command is
explicitly out of scope and will be handled inside `spec-kit-format` when
the shape of that command solidifies.

## What Changes

**1. Tag every `SpecEntry` with its source backend.**

Extend `SpecEntry` (in `src/specs/mod.rs`) with a new field
`backend: SpecBackendKind`, where `SpecBackendKind` is a new enum:

```rust
pub enum SpecBackendKind {
    OpenSpec,
    Markdown,
    // SpecKit added by the in-flight spec-kit-format change.
}
```

`OpenSpecBackend::scan` SHALL populate every returned entry with
`backend = SpecBackendKind::OpenSpec`. `MarkdownBackend::scan` SHALL
populate every returned entry with `backend = SpecBackendKind::Markdown`.
The field is non-optional — every entry knows which backend produced it.

**2. Branch `build_task_prompt` on the backend.**

`build_task_prompt` (`src/main.rs`) SHALL inspect `spec_entry.backend` and
return:

| `spec_entry`                            | task prompt                                                  |
|-----------------------------------------|--------------------------------------------------------------|
| `Some(s)` with `backend = OpenSpec`     | `format!("/opsx:apply {id}", id = s.id)`                     |
| `Some(s)` with `backend = Markdown`     | the v0.5.0 generic AGENTS.md pointer (current behaviour)     |
| `None`                                  | `"Begin your assigned task as described in AGENTS.md."` (unchanged) |

The Spec Kit branch is intentionally absent — it lands with the
`spec-kit-format` change. Until then, no `SpecEntry` carries
`backend = SpecKit`, so the match is exhaustive over the variants that
exist today.

**3. Tests.**

Three new unit tests in `src/main.rs::tests`:

1. `task_prompt_openspec_backend_invokes_opsx_apply_slash_command` —
   build a `SpecEntry` with `backend = SpecBackendKind::OpenSpec` and
   `id = "my-change"`. Assert
   `build_task_prompt(Some(&entry)) == "/opsx:apply my-change"`.
2. `task_prompt_markdown_backend_uses_generic_agents_md_pointer` —
   build a `SpecEntry` with `backend = SpecBackendKind::Markdown` and
   `id = "my-feature"`. Assert the result contains `AGENTS.md`,
   contains `openspec/changes/my-feature` (the existing pointer shape is
   preserved for Markdown), and does **not** contain `/opsx:apply`.
3. `task_prompt_without_spec_unchanged_after_backend_introduction` —
   regression for the `None` branch. Assert
   `build_task_prompt(None) == "Begin your assigned task as described in AGENTS.md."`
   verbatim (no regression from the v0.5.0 change).

Existing tests on the helper (`task_prompt_with_spec_points_at_agents_md_and_includes_id`,
`task_prompt_does_not_include_spec_body_first_line`) are updated so their
fixture `SpecEntry`s carry `backend = SpecBackendKind::Markdown` — that
keeps them exercising the "non-OpenSpec backend uses generic pointer"
path and preserves the regression coverage for the original v0.5.0 bug.

**Not in scope:**

- A Spec Kit slash-command equivalent (`/speckit:apply` or similar). Spec
  Kit support is the `spec-kit-format` change still in flight; that change
  owns the Spec Kit branch of `build_task_prompt`.
- Per-spec prompt overrides (`paw_boot_prompt` frontmatter, etc.). Belongs
  in v1.0.0 alongside per-CLI hook providers.
- Removing or renaming the v0.5.0 generic pointer. It remains as the
  Markdown branch and the upcoming Spec Kit fallback until those backends
  ship their own slash commands.
- Changes to AGENTS.md generation. `WorktreeAssignment.spec_content` is
  unchanged; agents on the OpenSpec branch still receive the full spec
  body in AGENTS.md, the slash command just orchestrates how they consume
  it.

## Capabilities

### New Capabilities
*(none — extends an existing capability and refines an existing
backend's contract)*

### Modified Capabilities

- `supervisor-launch`: the "Initial prompt injection via tmux send-keys"
  requirement (already updated by `boot-prompt-full-body`) gains
  per-backend dispatch. When the associated `SpecEntry`'s backend is
  `OpenSpec`, the task prompt SHALL be `/opsx:apply <id>`. When the
  backend is `Markdown` (or any non-OpenSpec backend lacking a
  slash-command apply workflow), the task prompt SHALL be the v0.5.0
  generic AGENTS.md pointer. The `None` branch is unchanged.
- `openspec-integration`: the `OpenSpecBackend` SHALL set
  `SpecEntry.backend = SpecBackendKind::OpenSpec` on every entry it
  returns, so downstream consumers (`build_task_prompt` today, future
  governance/dispatch logic later) can specialise behaviour to the
  backend.

## Impact

**Code (informational; this change is spec-only):**
- `src/specs/mod.rs` — add `SpecBackendKind` enum; add
  `backend: SpecBackendKind` field to `SpecEntry`.
- `src/specs/openspec.rs` — set `backend = SpecBackendKind::OpenSpec` on
  every constructed `SpecEntry`.
- `src/specs/markdown.rs` — set `backend = SpecBackendKind::Markdown` on
  every constructed `SpecEntry`.
- `src/main.rs::build_task_prompt` — switch on `spec_entry.backend` to
  pick the prompt shape.
- `src/main.rs::tests` — three new tests + minor fixture updates.

**Backward compatibility:**
- Agents on OpenSpec specs see a shorter, more directive boot prompt
  (`/opsx:apply <id>` is ~25 characters vs the ~200-character generic
  pointer). The paste-buffer trap is correspondingly less likely to
  fire on paste-aware CLIs — a small additional win.
- Agents on Markdown specs see no change. Their boot prompt is
  byte-identical to the v0.5.0 generic pointer.
- The no-spec branch is unchanged.
- `SpecEntry` gains a required field. Internal struct, not serialised;
  no migration concerns. Every constructor (both backends, plus the two
  test fixtures already in `src/main.rs::tests` and
  `src/specs/mod.rs::tests`) is updated as part of the implementation.

**Mismatches resolved:**
- Dogfood pattern where OpenSpec-backed agents enumerate the change
  directory before realising `/opsx:apply` would have done it for them:
  eliminated.
- User's in-session intent ("openspec → /opsx:apply") aligned with the
  shipping boot prompt.

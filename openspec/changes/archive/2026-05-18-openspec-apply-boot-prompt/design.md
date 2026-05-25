## Context

The v0.5.0 boot prompt is constructed by `build_task_prompt(spec_entry: Option<&SpecEntry>) -> String` (`src/main.rs:196`) as a pure helper, and called once by `cmd_supervisor` (`src/main.rs:848`). The output is appended to the standardized boot block (the four-event coordination instructions) and injected into each coding-agent pane via `tmux send-keys`.

`build_task_prompt` today returns one of two strings:

```rust
match spec_entry {
    Some(s) => format!(
        "Begin your assigned task. The full spec is in AGENTS.md in this worktree. \
         Additional artifacts (proposal, design, specs, tasks) live under \
         openspec/changes/{id}/ — read them all before starting.",
        id = s.id,
    ),
    None => "Begin your assigned task as described in AGENTS.md.".to_string(),
}
```

That string was correct for v0.5.0 because v0.5.0 only supported two backends
(OpenSpec and Markdown) and the slash-command skill `/opsx:apply` was not yet
treated as part of the standard agent harness. v0.5.0 dogfood demonstrated
that for OpenSpec specs the slash command is strictly better than the generic
pointer, because the skill internally handles task selection, ordering, and
artifact loading — work the agent would otherwise have to do manually after
reading AGENTS.md.

`SpecEntry` (`src/specs/mod.rs:23`) is the universal spec representation. Each
backend (`OpenSpecBackend`, `MarkdownBackend`) implements `SpecBackend::scan`
and constructs `SpecEntry`s without recording which backend produced them.
The information is recoverable from `config.specs.spec_type` but is not
attached to the entry, so `build_task_prompt` — which receives only a
`&SpecEntry` — has no way to specialise on backend identity today.

## Goals / Non-Goals

**Goals:**
- Make `build_task_prompt` return `/opsx:apply <id>` when the entry's backend
  is `OpenSpec`.
- Preserve the v0.5.0 generic AGENTS.md pointer for Markdown entries and for
  the no-spec case.
- Keep `build_task_prompt` a pure function (no I/O, no globals, callable from
  `cfg(test)` without launching tmux) — preserve the contract established by
  `boot-prompt-full-body`.
- Tag every `SpecEntry` with its source backend at scan time, so future
  per-backend behaviour (governance hooks, AGENTS.md template selection,
  dispatch routing) can specialise on the same field.

**Non-Goals:**
- A Spec Kit slash-command equivalent. The Spec Kit backend is being added
  by the in-flight `spec-kit-format` change; the shape of `/speckit:apply`
  (or whatever the analogous slash command becomes) is not yet settled.
  This change leaves a clear extension point but does not pre-shape the
  Spec Kit branch.
- Reading the configured `specs.type` at prompt-construction time. The
  backend identity belongs on the entry itself (see D1 below); routing it
  through config every time would couple `build_task_prompt` to
  `PawConfig` and break its purity.
- Per-spec `paw_boot_prompt` frontmatter overrides. Belongs in v1.0.0
  alongside per-CLI hook providers.
- Changing the boot block (the four coordination instructions) or
  AGENTS.md generation. Both are upstream of `build_task_prompt`, and
  neither needs to know about the per-backend prompt shape.
- Re-running the dogfood loop. The agents in the original v0.5.0 session
  are already past their boot prompts; the win lands on the next
  supervisor launch.

## Decisions

### D1. Backend identity lives on `SpecEntry`, not on a wrapper

**Choice:** Add `backend: SpecBackendKind` as a required field on
`SpecEntry`. Each backend's `scan` implementation populates the field
when constructing entries.

**Why:**
- Two consumers need the information today: backends (which already know
  their own identity at scan time, so populating the field is trivial)
  and `build_task_prompt` (which receives only a `&SpecEntry`). Putting
  the field on the entry threads through both naturally.
- Future consumers (governance config dispatch, per-backend AGENTS.md
  template selection, conflict-detection routing) all receive a
  `&SpecEntry` or hold one in their state; co-locating backend identity
  avoids a parallel `HashMap<EntryId, BackendKind>` that has to be kept
  in sync.
- The field is non-optional. Every constructor for `SpecEntry`
  (production paths and test fixtures) is updated in the same commit;
  there is no period where the field is missing, no `Option` to handle.
- Centralises the source of truth. Configuration tells the scanner
  which backend to dispatch to; the entry remembers what scanner
  produced it. The two stay in agreement by construction.

**Alternatives considered:**
- *Pass `&SpecsConfig` into `build_task_prompt` and read
  `spec_type`.* — Couples a pure helper to config plumbing; loses the
  per-entry guarantee (a future caller could mismatch config and entry
  source). Rejected.
- *Wrap `SpecEntry` in a `BackendTaggedEntry { entry, backend }` struct
  for the supervisor path only.* — Doubles the type surface and requires
  a translation layer between scanning (returns `SpecEntry`) and launch
  (consumes `BackendTaggedEntry`). The wrapper adds no information the
  entry can't carry itself. Rejected.
- *Encode the backend in the `id` string (`"openspec:my-change"`).* —
  Brittle, conflates two concerns in one string field, breaks every
  existing consumer of `id` (branch derivation, AGENTS.md generation,
  the slash-command path itself). Rejected.

### D2. Slash-command shape is exactly `/opsx:apply <change-id>`

**Choice:** The OpenSpec branch of `build_task_prompt` returns
`format!("/opsx:apply {id}", id = s.id)` — no prefix, no suffix, no
surrounding prose.

**Why:**
- The `opsx:apply` skill (and its plugin-namespaced sibling listed in
  the supervisor's available-skills block as `opsx:apply`) is the
  canonical entry point for "implement tasks from an OpenSpec change".
  Invoking it with the change ID as the sole argument matches the
  skill's documented usage and the slash-command convention used by
  every other skill in the harness.
- Anthropic Claude treats a line starting with `/` as a slash-command
  invocation when present at the start of a turn. Sending exactly
  `/opsx:apply my-change` causes the skill to run immediately, with
  the agent's first action being the skill's task-selection step
  rather than a free-form "what do I do" message.
- Wrapping the slash command in prose (e.g. `"Please run /opsx:apply
  my-change"`) defeats slash-command detection on at least some
  surfaces; keeping the prompt to just the command guarantees it's
  parsed as one.
- The output is short (~20–40 characters depending on `id` length),
  which materially lowers the chance of triggering Claude's
  paste-buffer trap on paste-aware CLIs. The v0.5.0 generic pointer
  is ~200 characters; the slash command is ~10× shorter. This is a
  small win (the supervisor recovers from paste-buffer regardless) but
  it's free.

**Alternatives considered:**
- *`/opsx:apply` without an argument.* — The skill would prompt for a
  change ID and the agent would have to derive it from CWD or the
  worktree path. Wasteful given the launcher already knows the ID.
  Rejected.
- *`/openspec-apply-change <id>` (the long form of the skill).* — Also
  available, but the namespaced `opsx:apply` form is the documented
  entry point and shorter. Pick the shorter equivalent.

### D3. Spec Kit slash command is owned by `spec-kit-format`, not this change

**Implementation deviation (2026-05-14):** the proposal and tasks.md
(task 1.3) were written on the premise that `spec-kit-format` was still
in flight and would land *after* this change, at which point it would
extend `SpecBackendKind` with `SpecKit` and add the matching arm to
`build_task_prompt`. In reality, `spec-kit-format` was archived on
2026-05-13 and `SpecKitBackend` already constructs `SpecEntry` literals
in production. Since `SpecEntry.backend` is non-optional, **this change
also adds the `SpecKit` variant** and populates it inside
`SpecKitBackend::scan`. The `SpecKit` branch of `build_task_prompt`
falls through to the generic AGENTS.md pointer (same shape as
`Markdown`), preserving D3's stance of not pre-empting whatever
slash-command shape Spec Kit eventually adopts. Task 1.3 is therefore
considered superseded by the codebase reality at apply time; the rest
of the spec stands as written.

**Original choice (still applicable to the rest of the change):**
This change adds the `OpenSpec` and `Markdown` variants of
`SpecBackendKind` and the `OpenSpec` branch of `build_task_prompt`.
It does **not** add a `SpecKit` variant or a `SpecKit` branch in
`build_task_prompt`.

**Why:**
- The Spec Kit backend itself is the `spec-kit-format` change, still in
  flight. Until that change merges, no `SpecEntry` exists with
  `backend = SpecKit`, so there is nothing for `build_task_prompt` to
  match.
- The shape of the Spec Kit slash command (does it exist? is it
  `/speckit:apply <feature>` or `/speckit:tasks <feature>` or
  something else?) belongs to whoever designs the Spec Kit workflow.
  Forcing a shape here risks contradicting that change.
- When `spec-kit-format` lands, it SHALL extend `SpecBackendKind` with
  a `SpecKit` variant and extend `build_task_prompt` with the
  corresponding match arm. That is a one-line delta to this change's
  enum plus whichever prompt shape `spec-kit-format` decides on. The
  spec for this change documents the extension point but does not
  pre-empt it.

**Alternatives considered:**
- *Add `SpecKit` variant now with a stubbed prompt.* — Couples the two
  changes' merge timing and forces a temporary prompt that the Spec
  Kit work would have to revise. Rejected.

### D4. Markdown backend keeps the v0.5.0 generic pointer

**Choice:** When `spec_entry.backend == SpecBackendKind::Markdown`,
`build_task_prompt` returns the same v0.5.0 string it returns today —
the AGENTS.md pointer with the spec ID interpolated into the
`openspec/changes/<id>/` path.

**Why:**
- No equivalent slash-command apply workflow exists for plain-Markdown
  specs. The agent does need to read AGENTS.md and start the work
  manually; the v0.5.0 pointer is the correct guidance.
- The Markdown pointer is byte-identical to today's behaviour, so
  Markdown-specs users see zero observable change. This is a
  conservative default and keeps the v0.5.0 regression test
  (`task_prompt_does_not_include_spec_body_first_line`) green by
  shifting its fixture to `backend = Markdown`.
- The path string `openspec/changes/<id>/` is technically OpenSpec
  flavored even for Markdown specs. That's pre-existing v0.5.0
  behaviour (the v0.5.0 pointer was written before Markdown specs were
  a first-class concern) and outside this change's scope. A future
  change can refine the Markdown pointer's path; this change preserves
  what shipped.

### D5. Composition with the boot block: shorter prompt, same shape

**Choice:** The full prompt sent to each agent pane remains
`format!("{boot_block}\n\n{task_prompt}")` (`src/main.rs` around line
848). The boot block (the four coordination instructions) precedes;
the task prompt is appended unchanged in structure — only its content
varies by backend.

**Why:**
- The boot block is uniform across agents and unrelated to spec
  backend. Keeping the composition structure constant means the only
  observable change at the tmux level is the task-prompt portion's
  byte content.
- For OpenSpec agents, the resulting injected string is
  `<boot_block>\n\n/opsx:apply <id>`. The boot block ends with a
  blank line, the slash command sits alone on its own line, and
  Claude parses it as a command at the start of the agent's first
  turn. Slash-command detection is preserved because the boot block
  is itself a sequence of curl commands and instructions; the agent's
  CLI does not interpret the boot block as one command.
- The total injected payload is shorter for OpenSpec entries than it
  was in v0.5.0 by ~150 bytes. Below the paste-buffer threshold most
  paste-aware CLIs use. Net effect: cleaner pane state on launch.

## Risks / Trade-offs

- **[`/opsx:apply` skill not available in the agent's CLI]** → If a
  user configures `specs.type = "openspec"` but their coding agent's
  CLI does not have the OpenSpec slash-command skills installed (e.g.
  a stripped-down Claude Code config, or a non-Anthropic CLI), the
  agent will see `/opsx:apply my-change` as literal user input and
  most likely echo a confused response. **Mitigation:** the supervisor
  skill ships as a built-in git-paw skill, and any project using
  `specs.type = "openspec"` is by convention also using the
  Anthropic-Claude harness with `opsx:apply` available. We treat this
  as a configuration error rather than a supported scenario. If
  dogfood surfaces real cases, a config flag
  (`supervisor.openspec_use_slash_command = false`) can be added later
  to opt back into the v0.5.0 generic pointer.
- **[Exhaustiveness over `SpecBackendKind`]** → Once `spec-kit-format`
  adds a `SpecKit` variant, `build_task_prompt`'s match must handle it
  or fail to compile. **Mitigation:** that's the point of having
  exhaustive matches; the `spec-kit-format` change cannot land without
  also extending `build_task_prompt`. Rust's compiler enforces the
  contract.
- **[Backend identity drift if a future backend forgets to set the
  field]** → If a new backend is added and its `scan` implementation
  forgets to populate `backend`, the entry will not compile (the field
  is required). **Mitigation:** the field's non-optional shape is
  itself the safeguard.
- **[Test fixtures across the codebase]** → Two existing test files
  (`src/main.rs::tests` and `src/specs/mod.rs::tests`) construct
  `SpecEntry` literals. They must be updated to populate the new
  field. **Mitigation:** the implementation tasks call this out
  explicitly; the compiler will catch any missed site.
- **[Path mismatch in the Markdown branch]** → The Markdown pointer
  still references `openspec/changes/<id>/`. For Markdown specs,
  there is no such directory; the agent would discover this on the
  first read attempt. **Mitigation:** this is pre-existing v0.5.0
  behaviour and out of scope; not made worse by this change.

## Migration Plan

This change is internal-shape only. No CLI flag, no config field, no
serialised state changes.

1. **Code change** in `src/specs/mod.rs`: define
   `SpecBackendKind`; add field to `SpecEntry`.
2. **Code change** in `src/specs/openspec.rs` and
   `src/specs/markdown.rs`: populate `backend` on every constructed
   entry.
3. **Code change** in `src/main.rs::build_task_prompt`: switch on
   `spec_entry.backend`; return `/opsx:apply <id>` for OpenSpec, the
   v0.5.0 pointer for Markdown.
4. **Code change** in `src/main.rs::tests::make_spec_entry`: accept
   (or default to) `backend = Markdown` so the existing regression
   tests preserve their semantics.
5. **Test additions** in `src/main.rs::tests`: three new tests
   covering the OpenSpec branch, the Markdown branch, and the no-spec
   branch.
6. **Rollback** — revert the field addition and the `build_task_prompt`
   switch. Behaviour reverts to the v0.5.0 generic pointer for all
   spec-backed agents.

## Open Questions

- *Should the Markdown branch eventually get its own slash command
  (`/markdown:start` or similar)?* Out of scope. The Markdown backend
  serves freeform per-file specs; there isn't an obvious "apply"
  workflow to scaffold around. Revisit if dogfood shows users
  asking for it.
- *Should the boot block itself mention which backend is in play?*
  Plausible but not necessary. The slash command (for OpenSpec) or the
  AGENTS.md pointer (for Markdown) is sufficient signal; the boot
  block stays format-agnostic.
- *Should `/opsx:apply` be configurable (`supervisor.openspec_command
  = "/foo:bar"`)?* Out of scope. Hard-code the canonical command; if
  users want overrides, the v1.0.0 per-CLI hook provider work is the
  right vehicle.
- *Does Spec Kit ship with `/speckit:apply`?* That's a question for the
  `spec-kit-format` change. This change documents the extension point
  and leaves the answer to the appropriate spec.

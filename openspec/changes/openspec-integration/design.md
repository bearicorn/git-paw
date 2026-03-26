## Context

The `spec-scanner` change defines `SpecBackend` and `SpecEntry`. The OpenSpec backend needs to scan the OpenSpec `changes/` directory structure. Each subdirectory under `changes/` represents a pending change with a known layout:

```
openspec/changes/<change-name>/
├── proposal.md
├── design.md
├── specs/
│   └── <capability>/
│       └── spec.md
└── tasks.md
```

The `archive/` directory contains completed changes (ignored by the scanner).

## Goals / Non-Goals

**Goals:**
- Scan `changes/` directory for pending change subdirectories
- Extract prompt content from `tasks.md` (primary) and `specs/` files (supplementary)
- Extract optional `paw_cli` override from frontmatter
- Extract optional file ownership from `tasks.md` content
- Return `SpecEntry` for each pending change

**Non-Goals:**
- Validating OpenSpec schema compliance (that's `openspec validate`)
- Modifying any OpenSpec files
- Handling the `archive/` directory beyond ignoring it
- Supporting nested changes (only top-level `changes/<name>/` directories)

## Decisions

### Decision 1: Module as `src/specs/openspec.rs`

Convert `src/specs.rs` to `src/specs/mod.rs` and add `src/specs/openspec.rs` as a submodule. The `mod.rs` re-exports `OpenSpecBackend` and the stub replacement wires it into `backend_for_type()`.

**Why:** Keeps spec backends organized under `specs/` namespace. The `markdown-integration` change will add `src/specs/markdown.rs` following the same pattern.

### Decision 2: `tasks.md` as primary prompt, specs as supplementary

The prompt content is built by:
1. Read `tasks.md` — this is the implementation checklist, the most actionable content for an agent
2. If `specs/` exists, read each `spec.md` and append under a `## Specs` heading
3. Concatenate into a single `prompt` string

**Why:** `tasks.md` is what the agent should _do_. Specs provide context on _what_ the system should behave like. Together they give the agent both action items and requirements.

### Decision 3: Frontmatter parsing for `paw_cli`

Check the first few lines of `tasks.md` for YAML frontmatter delimited by `---`. If present, look for `paw_cli: <value>`. This is a lightweight parse — no YAML dependency, just line-by-line scanning.

**Why:** Full YAML parsing requires a new dependency (`serde_yaml`). The only frontmatter field we need is `paw_cli`, which can be extracted with a simple string prefix check. This keeps the approved dependency list unchanged.

**Format:**
```markdown
---
paw_cli: gemini
---

## 1. Setup
...
```

### Decision 4: File ownership extraction from tasks.md

Scan `tasks.md` for lines matching the pattern `Files owned:` or `Owned files:` followed by a list. This is a convention, not enforced — if absent, `owned_files` is `None`.

**Why:** File ownership is declared in the change proposal and carried through to tasks. Extracting it lets the worktree AGENTS.md include ownership boundaries.

### Decision 5: Directory entries only, no recursion

`OpenSpecBackend::scan()` reads only immediate subdirectories of the `changes/` directory. It does not recurse into nested directories or follow symlinks.

**Why:** The OpenSpec convention is flat — each change is a top-level directory under `changes/`. Recursion would be confusing and break the 1:1 mapping between directory and change.

## Risks / Trade-offs

**[Missing tasks.md]** → A change directory without `tasks.md` is skipped with a warning to stderr. It's not an error because the change might be in-progress (only proposal written so far). → The scanner only returns actionable changes.

**[Large spec files]** → All spec content is read into memory and concatenated into the prompt. → Acceptable — spec files are typically < 5KB each, and there are rarely more than 10 per change.

**[Frontmatter parsing fragility]** → The simple line-by-line parser could break on edge cases (e.g., `---` inside a code block). → Acceptable for v0.2.0 — the frontmatter convention is controlled by git-paw and is always at the top of the file.

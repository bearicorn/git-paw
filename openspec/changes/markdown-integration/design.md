## Context

The `spec-scanner` change defines `SpecBackend` with a stub `MarkdownBackend`. The `openspec-integration` change implements a lightweight frontmatter parser (`parse_frontmatter`). This change implements the real Markdown backend, reusing that same parser.

Markdown specs are simpler than OpenSpec — each spec is a single `.md` file with frontmatter fields controlling status, branch name, and CLI assignment.

Example spec file:
```markdown
---
paw_status: pending
paw_branch: add-auth
paw_cli: claude
---

## Authentication Module

The system SHALL implement JWT-based authentication...
```

## Goals / Non-Goals

**Goals:**
- Scan a directory for `.md` files with `paw_status: pending` frontmatter
- Derive spec id from `paw_branch` field (fallback: filename stem)
- Extract `paw_cli` as optional CLI override
- Use file body (after frontmatter) as prompt content
- Ignore files with `paw_status` values other than `pending` (e.g., `done`, `in-progress`)
- Ignore files without `paw_status` frontmatter entirely

**Non-Goals:**
- Recursing into subdirectories (flat scan only)
- Supporting non-`.md` file extensions
- Modifying spec files (no status updates)
- Validating spec content beyond frontmatter fields

## Decisions

### Decision 1: Reuse frontmatter parser from `openspec-integration`

The `openspec-integration` change implements `parse_frontmatter(content: &str) -> (Option<HashMap<String, String>>, &str)`. This change reuses it by making it a shared function in `src/specs/mod.rs`.

**Why:** Both backends need the same `---`-delimited frontmatter parsing. Duplicating the parser would create maintenance burden.

**Alternative considered:** Add `serde_yaml` dependency. Rejected — not in the approved dependency list, and the frontmatter is simple enough for line-by-line parsing.

### Decision 2: Flat directory scan, `.md` files only

`MarkdownBackend::scan()` reads only immediate children of the specs directory, filtering for files with `.md` extension. Directories and non-markdown files are skipped.

**Why:** The markdown format is intentionally simple — no directory structure, just files. Recursion would add complexity and ambiguity about which files are specs.

### Decision 3: `paw_branch` with filename fallback

The spec id is derived from:
1. `paw_branch` frontmatter field (if present)
2. Filename stem (e.g., `add-auth.md` → `add-auth`) as fallback

**Why:** Explicit `paw_branch` gives users control over branch naming. The filename fallback keeps things simple for the common case where the filename is already a good branch name.

### Decision 4: Only `paw_status: pending` is actionable

Files with `paw_status: done`, `paw_status: in-progress`, or any other value are ignored. Files without `paw_status` frontmatter are also ignored.

**Why:** The scanner's job is to find _pending_ work. Other statuses are informational. Ignoring files without `paw_status` prevents accidentally picking up unrelated markdown files in the same directory.

## Risks / Trade-offs

**[Mixed file types in specs dir]** → Users might have README.md or other non-spec markdown files in the directory. → Mitigated by requiring `paw_status` frontmatter — files without it are ignored.

**[Filename collisions]** → Two files with the same `paw_branch` would produce duplicate branch names. → The scanner returns all entries; the caller (`start --from-specs`) should detect and error on duplicate branches.

**[Shared parser location]** → Moving `parse_frontmatter` to `specs/mod.rs` creates a dependency ordering question with `openspec-integration`. → Both changes add the parser; whichever merges first provides it, the second replaces the stub. Since they're in Wave 2 and merge sequentially, this is clean.

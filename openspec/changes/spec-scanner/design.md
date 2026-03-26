## Context

The `--from-specs` flag on `git paw start` needs to discover spec files, determine which are pending, derive branch names, and extract prompt content. Two spec formats are planned: OpenSpec (`changes/` directory structure) and Markdown (frontmatter-based). The scanner provides the shared abstraction; format-specific logic lives in separate modules.

The `[specs]` config section (added by `init-command`) provides the `dir` (spec directory path) and `type` (format: `"openspec"` or `"markdown"`).

## Goals / Non-Goals

**Goals:**
- Define `SpecEntry` as the universal representation of a discovered spec
- Define `SpecBackend` trait so formats are pluggable
- Provide `scan_specs()` as the entry point that reads config and dispatches to the right backend
- Derive branch names from spec identifiers using `branch_prefix` from config
- Return empty list (not error) when no pending specs exist

**Non-Goals:**
- Implementing OpenSpec or Markdown backends (separate changes)
- Creating branches or worktrees (caller does that with the returned `SpecEntry` list)
- Validating spec content (backends may do this internally)

## Decisions

### Decision 1: Trait-based backend dispatch

```rust
pub trait SpecBackend {
    fn scan(&self, dir: &Path) -> Result<Vec<SpecEntry>, PawError>;
}
```

`scan_specs()` reads `config.specs.type` and instantiates the matching backend. Unknown types return `PawError::SpecError`.

**Why:** Adding a new spec format means implementing one trait, not modifying scanner logic. The trait is object-safe so it can be used dynamically (`Box<dyn SpecBackend>`).

**Alternative considered:** Enum dispatch instead of trait. Rejected — trait is more extensible and follows Rust conventions for pluggable behavior.

### Decision 2: `SpecEntry` as the universal spec representation

```rust
pub struct SpecEntry {
    pub id: String,              // unique identifier (folder name or filename)
    pub branch: String,          // derived: branch_prefix + id
    pub cli: Option<String>,     // per-spec CLI override (from paw_cli frontmatter)
    pub prompt: String,          // content to inject into worktree AGENTS.md
    pub owned_files: Option<Vec<String>>, // file ownership if declared
}
```

**Why:** All consumers (worktree creation, AGENTS.md generation, tmux launch) need the same information. A single struct avoids format-specific types leaking into the session launch flow.

### Decision 3: Branch derivation at scan time

Branch names are derived during scanning: `branch_prefix` (from config, default `"spec/"`) + spec `id`. This happens in `scan_specs()`, not in the backends — backends return the `id`, the scanner computes the `branch`.

**Why:** Branch naming is a git-paw concern, not a format concern. Centralizing it ensures consistent naming regardless of backend.

### Decision 4: New `SpecError` variant on `PawError`

Add `SpecError(String)` for spec scanning failures (missing directory, invalid format, backend errors).

**Why:** Distinct from `ConfigError` (config is valid but the specs dir is wrong) and `InitError` (init succeeded but specs are malformed).

### Decision 5: Backend registration via function, not registry

```rust
fn backend_for_type(spec_type: &str) -> Result<Box<dyn SpecBackend>, PawError> {
    match spec_type {
        "openspec" => Ok(Box::new(OpenSpecBackend)),
        "markdown" => Ok(Box::new(MarkdownBackend)),
        _ => Err(PawError::SpecError(format!("unknown spec type: {spec_type}"))),
    }
}
```

**Why:** Two backends is not enough to justify a registration pattern. A match statement is simple, readable, and compile-time checked.

**Note:** The actual backend structs are stubs in this change — `openspec-integration` and `markdown-integration` implement them.

## Risks / Trade-offs

**[Empty scan result]** → No pending specs returns an empty vec, not an error. The caller (`start --from-specs`) should handle this by printing "No pending specs found" and exiting cleanly.

**[Backend stubs]** → This change creates the trait and stub backends that return empty results. The real implementations come in Wave 2. → Acceptable — Wave 1 branches compile and test independently.

**[Spec directory validation]** → The scanner validates that `specs_dir` exists and is a directory before passing to the backend. Invalid paths produce `SpecError`, not a panic.

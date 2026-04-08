# Spec-Driven Launch

The `--from-specs` flag lets you define branches, CLI assignments, and prompts in spec files instead of using interactive selection. git-paw reads spec files from a configured directory, creates worktrees for each pending spec, and launches AI CLIs with the spec content injected into each worktree's `AGENTS.md`.

## Quick Example

```bash
# Initialize repo config (creates .git-paw/config.toml)
git paw init

# Add spec files to your specs/ directory, then launch
git paw start --from-specs
```

## Spec Formats

git-paw supports two spec formats: **OpenSpec** (directory-based) and **Markdown** (file-based).

### OpenSpec Format (default)

Each pending change lives in its own subdirectory under the specs directory. The subdirectory name becomes the branch identifier.

```
specs/
  add-auth/
    tasks.md          # Required — main prompt
    specs/
      jwt/spec.md     # Optional — supplementary spec
  fix-pagination/
    tasks.md
```

**tasks.md** contains the prompt content sent to the AI CLI. It supports optional YAML frontmatter:

```markdown
---
paw_cli: claude
---

## Implement JWT Authentication

Add JWT token support to the auth module.
```

The `paw_cli` field overrides the CLI for this specific spec. If omitted, the default resolution chain applies (see [CLI Resolution](#cli-resolution) below).

Supplementary spec files in `specs/<name>/spec.md` are appended to the prompt with section headers. File ownership can be declared with "Files owned:" or "Owned files:" followed by a markdown list.

### Markdown Format

Flat `.md` files in the specs directory. Each file uses YAML frontmatter to control status and branch mapping.

```
specs/
  add-auth.md
  fix-pagination.md
  design-notes.md      # ignored — no paw_status: pending
```

**Example file (`specs/add-auth.md`):**

```markdown
---
paw_status: pending
paw_branch: add-auth
paw_cli: claude
---

## Implement JWT Authentication

Add JWT token support to the auth module.
```

#### Frontmatter Fields

| Field | Required | Description |
|-------|----------|-------------|
| `paw_status` | Yes | Must be `"pending"` to be included. Other values (`"done"`, `"in-progress"`) are ignored. |
| `paw_branch` | No | Branch name suffix. Falls back to filename stem if absent. |
| `paw_cli` | No | CLI override for this spec. |

Only files with `paw_status: pending` are picked up by `--from-specs`.

## Configuration

Configure spec scanning in `.git-paw/config.toml` (or the global config):

```toml
# Default CLI for --from-specs mode (bypasses picker when set).
default_spec_cli = "my-cli"

# Prefix for spec-derived branch names (default: "spec/").
branch_prefix = "spec/"

# Spec scanning configuration.
[specs]
dir = "specs"         # Directory containing spec files (relative to repo root)
type = "openspec"     # "openspec" (directory-based) or "markdown" (file-based)
```

### Branch Naming

Branch names are derived as `<branch_prefix><id>`:

- **OpenSpec:** ID is the subdirectory name. `specs/add-auth/` with prefix `spec/` becomes branch `spec/add-auth`.
- **Markdown:** ID is `paw_branch` (if set) or the filename stem. `specs/add-auth.md` with prefix `spec/` becomes branch `spec/add-auth`.

## CLI Resolution

When `--from-specs` is used, CLIs are resolved in priority order:

1. **`--cli` flag** (highest) — applies to all specs, no prompt
2. **`paw_cli` in spec** — per-spec override from frontmatter
3. **`default_spec_cli` in config** — fills remaining specs without prompt
4. **`default_cli` in config** — pre-selects in picker for remaining
5. **Interactive picker** (lowest) — prompts for any unresolved specs

```bash
# Override all specs to use claude
git paw start --from-specs --cli claude

# Use per-spec paw_cli and default_spec_cli from config
git paw start --from-specs

# Preview without executing
git paw start --from-specs --dry-run
```

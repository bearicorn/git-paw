# Spec-Driven Launch

The `--from-all-specs` and `--specs` flags let you define branches, CLI assignments, and prompts in spec files instead of using interactive selection. git-paw reads spec files from a configured directory, creates worktrees for each pending spec, and launches AI CLIs with the spec content injected into each worktree's `AGENTS.md`.

## Quick Example

```bash
# Initialize repo config (creates .git-paw/config.toml)
git paw init

# Add spec files to your specs/ directory, then launch every discovered spec
git paw start --from-all-specs

# Narrow to specific specs without editing config
git paw start --specs add-auth,fix-session

# Open a multi-select picker (requires an interactive terminal)
git paw start --specs
```

## Picking specs at launch time

You have three ways to control which specs are launched:

- **`--from-all-specs`** — launches every discovered spec across the configured backend.
- **`--specs NAME[,NAME...]`** — comma-separated list of spec names. Mirrors the existing `--branches feat/a,feat/b` syntax. Unknown names exit with the discovered-set listed as candidates so you can correct quickly.
- **`--specs`** (bare, no values) — opens a multi-select picker showing every discovered spec. Each row shows the unit identifier; for Spec Kit features that decompose into multiple worktrees, the row also shows a worktree-count hint (e.g. `003-user-list — 3 worktrees: 2 [P] + 1 phase/`).

`--from-all-specs` and `--specs` are mutually exclusive — they express opposing intents — and clap rejects any invocation that combines them.

### Picker requires a TTY

The bare `--specs` form requires an interactive terminal. When stdin is not a TTY (CI, scripted invocation, redirected input), git-paw exits with an actionable error pointing at the explicit forms:

```
error: --specs without values requires an interactive terminal
  Use `--specs NAME[,NAME...]` to narrow explicitly, or
  `--from-all-specs` to launch every discovered spec.
```

### Name resolution rules

`--specs NAME` matches against the discovered set using these strategies in order:

1. **Exact match** on the spec id (case-sensitive). Matches OpenSpec change names, Markdown filename stems, or Spec Kit decomposed entry ids like `003-user-list-T009`.
2. **Spec Kit feature-name match** — the value matches a Spec Kit feature directory prefix (e.g. `003-user-list`). All decomposed entries belonging to that feature are launched together.
3. **Spec Kit numeric prefix** — a digits-only value (e.g. `003`) matches a unique feature directory whose name starts with `003` followed by a non-digit boundary. Ambiguous prefixes (two features both starting with `003`) are rejected with the candidate list.

Unknown names error out before any worktrees are created. The error message includes the unresolved name AND the discovered candidate list. There is no partial start.

## Spec Formats

git-paw supports three spec formats: **OpenSpec** (directory-based), **Markdown** (file-based), and **Spec Kit** (`.specify/`-based, [GitHub Spec Kit](https://github.com/github/spec-kit)).

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

**Discovery is by archive status, not task completion.** `--from-all-specs` includes every change subdirectory under `specs/` *except* anything under `specs/archive/`. A change with all tasks marked `- [x]` in its `tasks.md` is still picked up — task-completion is a progress tracker, not a discovery filter. After completing and verifying a change, run:

```bash
openspec archive <change-name>
```

before the next `git paw start --from-all-specs` invocation. `openspec archive` moves the change directory under `specs/archive/<date>-<change>/` and syncs its delta specs into the main specs at `specs/<capability>/spec.md`. The next session won't spawn a worktree for it.

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

Only files with `paw_status: pending` are picked up.

### Spec Kit Format

Spec Kit projects place each feature in its own directory under `.specify/specs/<feature>/`, alongside an optional `.specify/memory/constitution.md` for project-level rules.

```
.specify/
  memory/
    constitution.md            # Optional — project rules / governance
  specs/
    001-room-setup/
      spec.md                  # Feature requirements
      plan.md                  # Implementation plan
      tasks.md                 # Required — task list with phases
      checklists/              # Optional — advisory validation criteria
        security.md
    002-poker-voting/
      spec.md
      plan.md
      tasks.md
```

**Auto-detection.** When `.specify/specs/` exists at the repo root *and* you have no `[specs]` section in `.git-paw/config.toml`, `git paw start --from-all-specs` defaults to `specs.type = "speckit"` with `specs.dir = ".specify/specs"`. `git paw init` also detects `.specify/` and writes the matching `[specs]` section to the generated config so the choice is locked.

You can override with `--specs-format`:

```bash
# Force Spec Kit even when [specs] config says otherwise
git paw start --from-all-specs --specs-format speckit

# Force OpenSpec on a project that has a `.specify/` folder
git paw start --from-all-specs --specs-format openspec
```

**`tasks.md` decomposition.** Spec Kit's `tasks.md` files use `## Phase N: <Name>` headings and `- [ ] T<NNN>` task lines. git-paw decomposes the **current phase** (the first phase with any incomplete task) into one worktree per kind:

- Each incomplete `[P]`-marked task → its own worktree on `task/<task-id>-<slug>` (one agent per task, parallel).
- All remaining incomplete non-`[P]` tasks → one *consolidated* worktree on `phase/<feature>-<phase-slug>` (one agent works through them sequentially, since the absence of `[P]` indicates shared files or context).

```
## Phase 2: Foundational
- [ ] T009 [P] Contract test POST /auth/otp/request   → task/t009-contract-test-...
- [ ] T010 [P] Contract test POST /auth/otp/verify    → task/t010-contract-test-...
- [ ] T011 Setup database schema                       ┐
- [ ] T012 Create auth tables                          ├ phase/<feature>-foundational
- [ ] T013 Seed test data                              ┘
```

In this example, one feature directory produces three worktrees and three branches. This "one Spec Kit feature → multiple branches" model is intentional — `[P]` tasks parallelise across worktrees; non-`[P]` tasks sequence within a single worktree.

**Phase advancement.** Phases earlier than the current one are assumed complete (all `- [x]`). Phases later than the current one are deferred — they only produce worktrees on a future scan, after every task in the current phase has been ticked off. Fully completed features (every task in every phase is `- [x]`) are skipped silently.

**Boot prompts.** Each Spec Kit worktree's `AGENTS.md` contains:

1. **Feature Context** — the full `spec.md` content.
2. **Implementation Plan** — the full `plan.md` content (omitted when absent).
3. **Validation Criteria (advisory)** — each `checklists/<file>.md` content under its own heading. Checklists are advisory in this release; full enforcement is planned for v1.0.0.
4. **Your Task** — the single task description (`task/...` worktrees) or the ordered task list plus sequential-execution + `- [x]` writeback + `agent.artifact` (with `status: "done"`) instructions (`phase/...` worktrees).

**Task writeback.** Agents working in a `phase/...` worktree flip `- [ ]` to `- [x]` in the worktree's `tasks.md` as they complete each task. The writeback may be committed alongside the task's code change or as a separate commit. Per-line edits across worktrees are merged by git in the normal way; the `conflict-detection` layer catches the pathological case of two worktrees racing on the same task ID.

**Constitution wiring.** When `.specify/memory/constitution.md` exists, git-paw exposes its path via the SpecKit backend's `detect_constitution` probe. A future `[governance.constitution]` config slot (the `governance-config` change) will consume this path automatically, so projects that already have a Spec Kit constitution get governance configured for free.

## Configuration

Configure spec scanning in `.git-paw/config.toml` (or the global config):

```toml
# Default CLI for spec-mode launches (bypasses picker when set).
default_spec_cli = "my-cli"

# Prefix for spec-derived branch names (default: "spec/").
branch_prefix = "spec/"

# Spec scanning configuration.
[specs]
dir = "specs"         # Directory containing spec files (relative to repo root)
type = "openspec"     # "openspec" (default), "markdown", or "speckit"
```

The interpretation of `dir` depends on the format:

- **`openspec`** — directory of change subdirectories (`<dir>/<change>/tasks.md`).
- **`markdown`** — directory of `.md` files (`<dir>/<spec>.md`).
- **`speckit`** — directory of feature subdirectories (`<dir>/<feature>/tasks.md`); typically `.specify/specs`.

### Branch Naming

Branch names depend on the format:

- **OpenSpec:** `<branch_prefix><id>` where ID is the subdirectory name. `specs/add-auth/` with prefix `spec/` becomes branch `spec/add-auth`.
- **Markdown:** `<branch_prefix><id>` where ID is `paw_branch` (if set) or the filename stem. `specs/add-auth.md` with prefix `spec/` becomes branch `spec/add-auth`.
- **Spec Kit:** branch prefix is `task/` or `phase/` depending on the entry kind; `branch_prefix` is **not** applied. Example: `.specify/specs/003-user-list/tasks.md` with task `- [ ] T009 [P] Add login form` produces branch `task/t009-add-login-form`.

## CLI Resolution

When a spec-mode launch runs, CLIs are resolved in priority order:

1. **`--cli` flag** (highest) — applies to all specs, no prompt
2. **`paw_cli` in spec** — per-spec override from frontmatter
3. **`default_spec_cli` in config** — fills remaining specs without prompt
4. **`default_cli` in config** — pre-selects in picker for remaining
5. **Interactive picker** (lowest) — prompts for any unresolved specs

```bash
# Override all specs to use claude
git paw start --from-all-specs --cli claude

# Use per-spec paw_cli and default_spec_cli from config
git paw start --from-all-specs

# Narrow to a subset
git paw start --specs add-auth,fix-session --cli claude

# Preview without executing
git paw start --from-all-specs --dry-run
git paw start --specs add-auth --dry-run
```

## Combining `--from-all-specs` with supervisor mode

`--from-all-specs --supervisor` (or `--from-all-specs` with `[supervisor] enabled = true`
in your config) engages the supervisor flow against the discovered specs.
git-paw scans the configured specs directory, creates one worktree per spec,
launches the dashboard pane and per-spec agent panes, and starts the supervisor
CLI in your foreground terminal — same supervisor architecture as
`--branches`-driven sessions, just with branches discovered from specs.

```bash
# Spec-driven session with supervisor watching
git paw start --from-all-specs --supervisor

# Same outcome via [supervisor] enabled = true in .git-paw/config.toml
git paw start --from-all-specs

# Skip supervisor for this session even if enabled in config
git paw start --from-all-specs --no-supervisor
```

When `[broker] enabled = true` (the default in supervisor mode), each spec
agent pane receives a boot block via `tmux send-keys` carrying its
`BRANCH_ID`, broker URL, and curl-publish patterns. This applies to both
spec-mode-only (`--from-all-specs` without supervisor) and supervisor mode.

## Non-interactive launches

If `git paw start` is invoked from a non-interactive terminal — CI, scripted
invocation, or a harness tool that pipes stdin — git-paw skips the auto-attach
step and prints an attach hint instead:

```
Session 'paw-myproject' started in detached mode.
Attach with:  tmux attach -t paw-myproject
```

For supervisor mode in a non-interactive context, the foreground supervisor
CLI is also skipped (Claude/Codex etc. need a TTY to run interactively). The
session, dashboard pane, and agent panes are all created; you can attach later
from a real terminal and start the supervisor manually:

```bash
tmux attach -t paw-myproject
# in another terminal, in the repo root:
cd /path/to/repo && claude
```

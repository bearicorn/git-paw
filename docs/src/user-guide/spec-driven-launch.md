# Spec-Driven Launch

The `--from-all-specs` and `--specs` flags let you define branches, CLI assignments, and prompts in spec files instead of using interactive selection. git-paw reads spec files from a configured directory, creates worktrees for each pending spec, and launches AI CLIs with the spec content injected into each worktree's gitignored `.git-paw/AGENTS.local.md` sidecar (the tracked `AGENTS.md` is left untouched).

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
- **`--specs`** (bare, no values) — opens a multi-select picker showing every discovered spec. Each row shows the unit identifier; for Spec Kit features that decompose into multiple worktrees, the row also shows a worktree-count hint (e.g. `003-user-list — 3 worktrees: 2 [P] + 1 phase/`). The picker is a type-to-filter multi-select (identical to the branch picker): type to narrow the rows by case-insensitive substring match against the displayed label, Ctrl-U clears the filter, Space toggles, and Enter confirms. Selecting a filtered row still expands to every underlying entry for that unit, and selections persist across filter changes.

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

git-paw supports four spec formats: **OpenSpec** (directory-based), **Markdown** (file-based), **Spec Kit** (`.specify/`-based, [GitHub Spec Kit](https://github.com/github/spec-kit)), and **Superpowers** (`docs/superpowers/plans/`-based, [obra/superpowers](https://github.com/obra/superpowers)).

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

**Selecting Spec Kit.** git-paw does not auto-detect the spec system from the filesystem — you set it explicitly. Either configure `[specs] type = "speckit"` with `dir = ".specify/specs"` in `.git-paw/config.toml` (`git paw init` prompts you to choose the spec system and writes this section), or pass `--specs-format speckit` for a single launch.

Selecting a spec system for one launch with `--specs-format`:

```bash
# Force Spec Kit even when [specs] config says otherwise
git paw start --from-all-specs --specs-format speckit

# Force OpenSpec on a project that has a `.specify/` folder
git paw start --from-all-specs --specs-format openspec

# Force Superpowers on a project with docs/superpowers/plans/
git paw start --from-all-specs --specs-format superpowers
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

**Boot prompts.** Each Spec Kit worktree's `.git-paw/AGENTS.local.md` sidecar contains:

1. **Feature Context** — the full `spec.md` content.
2. **Implementation Plan** — the full `plan.md` content (omitted when absent).
3. **Validation Criteria (advisory)** — each `checklists/<file>.md` content under its own heading. Checklists are advisory.
4. **Your Task** — the single task description (`task/...` worktrees) or the ordered task list plus sequential-execution + `- [x]` writeback + `agent.artifact` (with `status: "done"`) instructions (`phase/...` worktrees).

**Task writeback.** Agents working in a `phase/...` worktree flip `- [ ]` to `- [x]` in the worktree's `tasks.md` as they complete each task. The writeback may be committed alongside the task's code change or as a separate commit. Per-line edits across worktrees are merged by git in the normal way. The [conflict detector](conflict-detection.md#ownership-violation) exempts writebacks to the shared `tasks.md` from ownership-violation detection; concurrent edits to it by two worktrees can still raise in-flight warnings.

**Constitution wiring.** With `[specs] type = "speckit"` and no explicit `[governance] constitution` setting, git-paw probes `<specs dir>/../memory/constitution.md` (normally `.specify/memory/constitution.md`) and, when it exists, wires it in as the governance constitution automatically — so a project that already has a Spec Kit constitution gets it configured for free. An explicit `[governance] constitution` value always wins; see [Configuration → Spec Kit constitution auto-wiring](../configuration/README.md#spec-kit-constitution-auto-wiring).

### Superpowers Format

[obra/superpowers](https://github.com/obra/superpowers) is a multi-CLI methodology plugin whose `writing-plans` skill saves implementation plans as flat Markdown files under `docs/superpowers/plans/`. git-paw scans those plans — **one worktree per incomplete plan**.

```
docs/superpowers/plans/
  2026-07-20-add-auth.md       # one plan = one worktree
  2026-07-21-export-csv.md
```

Unlike Spec Kit, a superpowers plan is **not** fanned out into per-task worktrees — a plan is a sequential TDD chain meant for a single `subagent-driven-development` worktree, so each plan file maps to exactly one branch: `plan/<plan-file-stem>`.

**Plan structure.** Each plan carries a header (`**Goal:**`, `**Architecture:**`, `**Tech Stack:**`) and one or more `### Task N: <name>` sections, each with a `**Files:**` block (exact paths) and bite-sized `- [ ]` checkbox steps plus `Run:` verification commands.

**Discovery.** A plan is picked up when it has at least one incomplete `- [ ]` step. A plan whose every step is `- [x]` is skipped (with a warning); a `.md` file with no `### Task`/step lines — e.g. a design doc under `docs/superpowers/specs/` — is skipped silently.

**Selecting Superpowers.** As with every format, git-paw does not auto-detect it — configure `[specs] type = "superpowers"` with `dir = "docs/superpowers/plans"` (`git paw init` prompts you to choose the spec system), or pass `--specs-format superpowers` for a single launch.

**Boot prompt.** Each worktree's `.git-paw/AGENTS.local.md` sidecar contains: **Plan Context** (Goal / Architecture / Tech Stack), **Your Tasks** (the plan's task sections with their `Files:` and `Run:` metadata), and an **Execution** instruction to work the steps in order, flip `- [ ]` → `- [x]` in the plan file as each lands, and commit your work when every step shows `- [x]`; the post-commit hook will auto-publish your completion.

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
type = "openspec"     # "openspec" (default), "markdown", "speckit", or "superpowers"
```

The interpretation of `dir` depends on the format:

- **`openspec`** — directory of change subdirectories (`<dir>/<change>/tasks.md`).
- **`markdown`** — directory of `.md` files (`<dir>/<spec>.md`).
- **`speckit`** — directory of feature subdirectories (`<dir>/<feature>/tasks.md`); typically `.specify/specs`.
- **`superpowers`** — directory of flat plan files (`<dir>/<plan>.md`); typically `docs/superpowers/plans`.

### Branch Naming

Branch names depend on the format:

- **OpenSpec:** `<branch_prefix><id>` where ID is the subdirectory name. `specs/add-auth/` with prefix `spec/` becomes branch `spec/add-auth`.
- **Markdown:** `<branch_prefix><id>` where ID is `paw_branch` (if set) or the filename stem. `specs/add-auth.md` with prefix `spec/` becomes branch `spec/add-auth`.
- **Spec Kit:** branch prefix is `task/` or `phase/` depending on the entry kind; `branch_prefix` is **not** applied. Example: `.specify/specs/003-user-list/tasks.md` with task `- [ ] T009 [P] Add login form` produces branch `task/t009-add-login-form`.
- **Superpowers:** `plan/<slugified plan-file stem>`; `branch_prefix` is **not** applied. Example: `docs/superpowers/plans/2026-07-20-add-auth.md` produces branch `plan/2026-07-20-add-auth`.

Spec Kit and Superpowers derive their branch name from spec text — a task
description, a plan-file stem — by lowercasing it and replacing every character
outside `[a-z0-9_]` with `-`. **Underscores survive into the branch name.** A
task reading `- [ ] T001 [P] Create ROOM_SETUP.md` produces branch
`task/t001-create-room_setup-md`, and a plan file `add_auth.md` produces branch
`plan/add_auth`: `_` is legal in a git ref, and the branch is durable state that
worktree recovery and `--resume` use to look up work, so it is left intact.

The agent's broker `agent_id` is a *separate* derivation from that branch name,
and it does **not** keep `_` — the broker's id grammar forbids it, so the
underscore maps to `-` there. The branch `task/t001-create-room_setup-md`
publishes as agent `task-t001-create-room-setup-md`. See
[Identifier Forms](coordination.md#identifier-forms--branch-vs-agent_id) for the
full conversion.

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
and builds one tmux session with the dashboard pane (pane 0) and the per-spec
agent panes from pane 1 — the orchestrator runs outside the session, reading
the context rendered into `.git-paw/AGENTS.local.md` — then returns with an
attach hint (or, with `--unattended`, runs the in-process drive loop instead)
— same supervisor architecture as `--branches`-driven sessions, just with
branches discovered from specs.

The [CLI Resolution](#cli-resolution) chain above applies identically here —
`--supervisor` never bypasses or alters it. In particular, `default_spec_cli`
is honoured for a spec-driven worker pane exactly as it is without
`--supervisor`, so a `[clis.<name>]` sandbox pinned via `default_spec_cli`
cannot be silently swapped for a different CLI just because the launch went
through the supervisor flow. `git paw doctor` also reports the effective
spec-worker CLI and warns if a configured `default_spec_cli` is never
actually reachable by any discovered spec — see [Doctor](doctor.md).

```bash
# Spec-driven session with supervisor watching
git paw start --from-all-specs --supervisor

# Same outcome via [supervisor] enabled = true in .git-paw/config.toml
git paw start --from-all-specs

# Skip supervisor for this session even if enabled in config
git paw start --from-all-specs --no-supervisor
```

When `[broker] enabled = true` — the broker defaults to disabled and must be
opted into explicitly in config (supervisor mode does not turn it on for
you) — each spec agent pane receives a boot block via `tmux send-keys`
carrying its `BRANCH_ID`, broker URL, and the patterns for publishing through
the bundled `broker.sh` helper. This applies to both spec-mode-only
(`--from-all-specs` without supervisor) and supervisor mode; a spec-mode-only
launch additionally submits the boot block (rather than leaving it
pre-filled) and appends the task prompt naming the agent's spec assignment,
so the agent starts work without a human completing the prompt by hand — see
[Boot-block submission and task-prompt injection](#boot-block-submission-and-task-prompt-injection)
below.

## Boot-block submission and task-prompt injection

Not every launch path treats its agent pane's initial prompt the same way:

- **Bare `git paw start`** (no spec selection) pre-fills the boot block into
  the pane's input line and stops — no `Enter` is sent. This is deliberate:
  the launch is human-driven, so you type your actual task after the boot
  instructions and submit it yourself.
- **A spec-driven launch** (`--specs` / `--from-all-specs`, with or without
  `--supervisor`) has no human typing the task — the task comes from the
  spec — so leaving the prompt pre-filled would strand the agent with
  nothing to act on. Both the supervisor path and the non-supervisor
  spec-mode path submit the combined boot block + task prompt automatically:
  the text is injected, then — after a brief per-CLI settle delay — a
  separate `Enter` submits it. The task prompt names the agent's spec
  assignment and the backend's apply invocation (e.g. `/opsx:apply <id>` for
  OpenSpec), so the agent drives its spec workflow and checks off tasks as it
  completes them, rather than improvising an ad hoc one.
- With `[broker] enabled = false`, the **non-supervisor** spec-mode path
  (bare `--specs` / `--from-all-specs`, no `--supervisor`) injects no boot
  block — it is broker-specific content — but still injects and submits the
  task prompt, since the agent's assignment does not depend on the broker.
  The **supervisor** path builds its agent panes' prompt the same way
  regardless of whether the broker is enabled, so a supervisor-mode agent
  still receives a boot block even with the broker off; only its
  broker-coordination instructions are then inert.

Every launch path also composes its pane's command from the [resolved
approval flags](../configuration/README.md#supervisor) (`agent_approval` and
any per-CLI `approval_args` override) — a bare or spec-driven launch is not
exempt just because no supervisor is watching.

## Non-interactive launches

If `git paw start` is invoked from a non-interactive terminal — CI, scripted
invocation, or a harness tool that pipes stdin — git-paw skips the auto-attach
step and prints an attach hint instead:

```
Session 'paw-myproject' started in detached mode.
Attach with:  tmux attach -t paw-myproject
```

Supervisor mode never auto-attaches, interactive terminal or not: the
orchestrator is your own agent session outside tmux, and (without
`--unattended`) the launch prints `Supervisor session '<name>' launched with
<N> coding agent(s).`, the orchestrator-context path
(`.git-paw/AGENTS.local.md`), and the same `Attach with:` hint. Attach later from a real
terminal:

```bash
tmux attach -t paw-myproject
```

# session-runtime-isolation Specification

## Purpose
Make parallel worktree sessions independent at runtime, not just in git. When a
worktree is created, git-paw provisions its runtime state: it copies declared
env files verbatim (never symlinks — a copy diverges cleanly and drift surfaces
at merge), allocates an index-derived, collision-free port block written to a
generated `.env.local`, and runs consumer-authored `on_create`/`on_remove`
lifecycle hooks (with `on_create` stdout merged into that `.env.local`) as the
general escape hatch for stack-specific isolation such as per-worktree
databases. Every mechanism is opt-in `[worktree.*]` config and export-agnostic:
git-paw ships copy / allocate / run-a-hook, never a stack, port list, or driver.

## Requirements
### Requirement: Env-file provisioning copies declared files into each worktree

git-paw SHALL, when creating a worktree, copy each file listed in
`[worktree.env] copy` verbatim from the repository root into the new worktree,
performing the copy after the worktree is created and before the agent process
is launched. The propagated file MUST be an independent copy, not a symlink, so
that edits in one worktree do not affect the source or any sibling worktree.

**Test:** `tests/worktree_env_provisioning.rs`

#### Scenario: Declared env file is copied into a new worktree
- **WHEN** `[worktree.env] copy = [".env"]` is configured, `.env` exists at the repository root, and a worktree is created
- **THEN** a byte-for-byte copy of `.env` exists at the root of the new worktree

#### Scenario: Copied env file is independent of the source
- **WHEN** a worktree's copied `.env` is modified after provisioning
- **THEN** the repository-root `.env` and every sibling worktree's `.env` are unchanged (the copy is not a symlink to shared state)

#### Scenario: Missing declared file is skipped without failing the create
- **WHEN** `[worktree.env] copy` lists a file that does not exist at the repository root
- **THEN** git-paw skips that file, emits a warning, and completes the worktree creation successfully

#### Scenario: Provisioning precedes agent launch
- **WHEN** a worktree with `[worktree.env] copy` configured is created for an agent session
- **THEN** the copied files are present in the worktree before the agent's CLI process starts

### Requirement: Per-worktree port allocation writes a generated `.env.local`

git-paw SHALL assign each worktree a distinct port block derived from its index
in session state, computed as `base + index * stride`, and MUST write one
`VAR=<port>` line per entry of `[worktree.ports] vars` into a generated
`.env.local` in the worktree, assigning ports sequentially within the block
(the Nth listed var receives `base + index * stride + N`). The generated
`.env.local` MUST NOT overwrite a copied `.env`; port values live only in
`.env.local`.

**Test:** `tests/worktree_port_allocation.rs`

#### Scenario: First worktree receives the base port block
- **WHEN** `[worktree.ports] base = 3000, stride = 10, vars = ["PORT", "VITE_PORT"]` and the worktree has session-state index 0
- **THEN** the worktree's generated `.env.local` contains `PORT=3000` and `VITE_PORT=3001`

#### Scenario: Subsequent worktrees receive non-overlapping blocks
- **WHEN** a second worktree has session-state index 1 under the same configuration
- **THEN** its `.env.local` contains `PORT=3010` and `VITE_PORT=3011`, and no port collides with any other active worktree

#### Scenario: Port allocation does not mutate the copied env file
- **WHEN** both `[worktree.env] copy = [".env"]` and `[worktree.ports]` are configured
- **THEN** the copied `.env` is left byte-for-byte identical to the source and the offset ports appear only in `.env.local`

#### Scenario: A removed worktree's port block is freed for reuse
- **WHEN** a worktree is removed or purged
- **THEN** its port block is released, and no two concurrently active worktrees are ever assigned overlapping blocks across add/remove churn

### Requirement: Runtime provisioning is opt-in and backward compatible

git-paw SHALL treat `[worktree.env]` and `[worktree.ports]` as optional, and
when neither is configured MUST create worktrees exactly as in prior versions —
copying no files and generating no `.env.local`. git-paw MUST NOT assume any
default filename, port, or stack; it acts only on explicitly declared
configuration.

**Test:** `tests/worktree_env_provisioning.rs`

#### Scenario: Absent configuration preserves prior behavior
- **WHEN** neither `[worktree.env]` nor `[worktree.ports]` is present in `.git-paw/config.toml` and a worktree is created
- **THEN** no files are copied, no `.env.local` is generated, and the worktree is identical to one created by the previous version

#### Scenario: No implicit defaults
- **WHEN** `[worktree.ports]` is configured with an empty `vars` list
- **THEN** no port lines are written to `.env.local` (git-paw supplies no default port variables)

#### Scenario: Existing configuration loads unchanged
- **WHEN** a `.git-paw/config.toml` written by a prior version (with no `[worktree.env]` or `[worktree.ports]` tables) is loaded
- **THEN** it parses successfully with runtime provisioning disabled

### Requirement: Worktree lifecycle hooks provision and tear down per-worktree resources

git-paw SHALL run the consumer-configured `[worktree.hooks] on_create` command,
if present, after env/port provisioning and before the agent process launches,
executing it with the new worktree as the working directory; and SHALL run
`[worktree.hooks] on_remove`, if present, when a worktree is removed or purged.
In both commands git-paw MUST substitute `{worktree_id}` with the worktree's
stable identifier and `{worktree_path}` with its absolute path before execution.

**Test:** `tests/worktree_lifecycle_hooks.rs`

#### Scenario: on_create runs in the worktree before agent launch
- **WHEN** `[worktree.hooks] on_create` is configured and a worktree is created for an agent session
- **THEN** the command runs with the new worktree as its working directory, after env/port provisioning and before the agent's CLI process starts

#### Scenario: Placeholders are substituted
- **WHEN** an `on_create` or `on_remove` command contains `{worktree_id}` or `{worktree_path}`
- **THEN** git-paw substitutes the worktree's stable id and absolute path, respectively, before executing the command

#### Scenario: on_remove runs at worktree teardown
- **WHEN** `[worktree.hooks] on_remove` is configured and a worktree is removed or purged
- **THEN** the command runs before the worktree directory is deleted

### Requirement: on_create stdout merges into the generated `.env.local`

git-paw SHALL capture the standard output of the `on_create` hook, parse it as
`KEY=value` lines, and merge those pairs into the same git-paw-managed block of
the worktree's `.env.local` that port allocation writes, so a hook that
provisions an isolated resource (e.g. a database) can surface its connection
details to the running agent. Non-`KEY=value` output lines MUST be ignored, and
the merge MUST NOT disturb `.env.local` content outside the managed block.

**Test:** `tests/worktree_lifecycle_hooks.rs`

#### Scenario: Hook output becomes environment in .env.local
- **WHEN** an `on_create` hook prints `DATABASE_URL=postgres://localhost/paw_wt1` to stdout
- **THEN** the worktree's `.env.local` managed block contains `DATABASE_URL=postgres://localhost/paw_wt1`

#### Scenario: Hook output coexists with allocated ports
- **WHEN** both `[worktree.ports]` and an `on_create` hook that prints a `KEY=value` line are configured
- **THEN** the `.env.local` managed block contains both the allocated port lines and the hook's key, and no content outside the managed block is altered

#### Scenario: Non-assignment output is ignored
- **WHEN** an `on_create` hook prints diagnostic lines that are not `KEY=value` pairs
- **THEN** those lines are ignored and do not appear in `.env.local`

### Requirement: Hook failure policy protects worktree integrity

git-paw SHALL treat an `on_create` hook's non-zero exit as a provisioning
failure — reporting the hook's exit status and standard error — while an
`on_remove` hook's non-zero exit MUST NOT block worktree removal but MUST be
surfaced as a warning, so a failed teardown never strands a worktree. git-paw
MUST NOT log the captured `KEY=value` values (they may contain secrets such as a
database password).

**Test:** `tests/worktree_lifecycle_hooks.rs`

#### Scenario: on_create failure is reported
- **WHEN** an `on_create` hook exits non-zero
- **THEN** git-paw reports the failure with the hook's exit status and stderr

#### Scenario: on_remove failure does not block removal
- **WHEN** an `on_remove` hook exits non-zero during `git paw remove` or `purge`
- **THEN** the worktree is still removed and git-paw emits a warning about the failed hook

#### Scenario: Captured secret values are not logged
- **WHEN** an `on_create` hook prints a `KEY=value` line whose value is sensitive (e.g. a connection string with a password)
- **THEN** git-paw writes it to `.env.local` without echoing the value to its own logs or stdout

### Requirement: Lifecycle hooks are opt-in and export-agnostic

git-paw SHALL treat `[worktree.hooks]` as optional and, when absent, MUST run no
hooks — behavior identical to configuring env/port provisioning alone. git-paw
MUST NOT assume any database, driver, or stack; it only executes the exact
commands the consumer declares.

**Test:** `tests/worktree_lifecycle_hooks.rs`

#### Scenario: Absent hooks configuration runs nothing
- **WHEN** no `[worktree.hooks]` table is present and a worktree is created or removed
- **THEN** git-paw runs no hook command and worktree lifecycle behaves exactly as without this feature

#### Scenario: No implied database policy
- **WHEN** `[worktree.hooks]` is configured
- **THEN** git-paw runs only the consumer's declared command and provisions no database or resource on its own


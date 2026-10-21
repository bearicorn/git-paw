# Configuration

git-paw uses TOML configuration files at two levels, with repo-level settings overriding global ones.

## Config File Locations

| Level | Path | Purpose |
|-------|------|---------|
| Global | `~/.config/git-paw/config.toml` (Linux; `$XDG_CONFIG_HOME/git-paw/config.toml` when that is set) or `~/Library/Application Support/git-paw/config.toml` (macOS) | Default CLI, custom CLIs, global presets |
| Per-repo | `.git-paw/config.toml` (in repo root) | Repo-specific overrides |

Both files are optional. git-paw works with sensible defaults when no config exists.

## Full Config Example

```toml
# CLI pre-selected in the interactive picker when --cli is not provided
default_cli = "my-cli"

# Default CLI for spec-mode launches (--from-all-specs, --specs); bypasses picker when set
# default_spec_cli = "my-cli"

# Prefix for spec-derived branch names (default: "spec/")
# branch_prefix = "spec/"

# Documentation site the bundled docs-fetch helper consults on demand
# (default: git-paw's published docs). Set to retarget a fork or mirror.
# docs_base_url = "https://bearicorn.github.io/git-paw"

# Enable mouse mode in tmux sessions (default: true)
mouse = true

# Where agent worktrees are created: "child" (in-repo .git-paw/worktrees/)
# or "sibling" (../<project>-<branch>). Absent = "sibling"; git paw init
# writes "child" for new repos and backfills it into a config that omits it.
worktree_placement = "child"

# Pane affordances: double-line borders, per-pane labels, active-pane highlight
# (default: true; set false to inherit your own tmux styling)
# [layout]
# border_affordances = true

# Per-worktree runtime isolation (all opt-in; absent = no copying, no
# .env.local, no hook). Files are copied verbatim into each worktree; each
# worktree gets its own port block base + slot * stride (keep stride >= vars
# length); hooks run your own commands at worktree create/remove.
# [worktree.env]
# copy = [".env", ".env.development"]
# [worktree.ports]
# base   = 3000
# stride = 10
# vars   = ["PORT", "VITE_PORT"]
# [worktree.hooks]
# on_create = "scripts/paw-db-branch.sh {worktree_id}"
# on_remove = "scripts/paw-db-drop.sh {worktree_id}"

# Custom CLI definitions
[clis.my-agent]
command = "/usr/local/bin/my-agent"
display_name = "My Agent"

[clis.local-llm]
command = "ollama-code"
display_name = "Local LLM"

# A custom CLI is also the seam for the optional FS-scoped sandbox: point
# `command` at a PATH-resolvable wrapper that execs the real CLI under
# sandbox-exec/bwrap. git-paw launches a CLI by its name on PATH, so the wrapper
# must be on PATH (not just a script path). See the FS-Scoped Sandbox guide.

# Named presets for quick launch
[presets.backend]
branches = ["feature/api", "fix/db"]
cli = "claude"

[presets.frontend]
branches = ["feature/ui", "feature/styles"]
cli = "codex"

# Spec scanning configuration
# [specs]
# dir = "specs"
# type = "openspec"    # "openspec", "markdown", "speckit", or "superpowers"

# opsx (OpenSpec) role gating — active only under the OpenSpec engine
# [opsx]
# role_gating = "warn"  # "warn" (default), "block", or "off"

# Session logging
# [logging]
# enabled = false

# Agent coordination broker
# [broker]
# enabled = false
# port = 9119
# bind = "127.0.0.1"
# wave_stall_minutes = 30

# Pointers to user-maintained governance docs (all optional)
# [governance]
# adr = "docs/adr"
# test_strategy = "docs/test-strategy.md"
# security = "docs/security-checklist.md"
# dod = "docs/definition-of-done.md"
# constitution = ".specify/memory/constitution.md"
# readme = "README.md"
# docs = "docs/src"
```

## Upgrading an Existing Config

`git paw init` is the upgrade path after a version bump: re-run it in an
already-initialised repo and it brings `.git-paw/config.toml` up to the current
defaults. The migration is **strictly additive** — it appends

- a `[supervisor]` section when the file has none (init asks whether to enable
  supervisor mode in an interactive terminal, and writes `enabled = false`
  otherwise), and
- any **top-level default key** the file is missing, written with its
  generated-default value (for example `worktree_placement`, added in v0.8.0).

An existing but empty `config.toml` is instead seeded with the full generated
template. The other sections appear in the template only as commented examples,
so they are not appended to a file you have already written.

It never modifies or removes a section, a key, or a commented example you have
already set, so your customised settings survive verbatim. Once `[supervisor]`
and every top-level default key are present the config is left untouched, and
— if nothing else needed creating — init reports
`Already initialized. Nothing to do.` (The bundled `.git-paw/scripts/` helpers
are refreshed on every run regardless.)

When something is added, init names it:

```text
  Updated .git-paw/config.toml (added missing keys: worktree_placement)
```

One backfilled key can change behaviour — see
[`worktree_placement`](#worktree_placement) for the warning init prints in that
case.

## Settings Reference

### `default_cli`

The AI CLI pre-selected in the interactive CLI picker when `--cli` is not
passed — you can still choose any other listed CLI. It does **not** skip the
picker: the assignment-mode prompt and the CLI picker still appear, and in
per-branch mode every branch's picker starts on this CLI too. Unset, or naming
a CLI that is not available, the picker starts on its first entry. `--cli`
skips the CLI prompt entirely.

```toml
default_cli = "my-cli"
```

### `default_spec_cli`

The AI CLI to use by default when launching with `--from-all-specs` or `--specs`. When set, skips the CLI picker for any specs that don't have a `paw_cli` override.

```toml
default_spec_cli = "my-cli"
```

See [Spec-Driven Launch](../user-guide/spec-driven-launch.md) for the full CLI resolution chain.

### `branch_prefix`

Prefix prepended to spec-derived branch names. Defaults to `"spec/"`.

```toml
branch_prefix = "spec/"
```

For example, a spec with ID `add-auth` produces branch `spec/add-auth`.

### `mouse`

Enable or disable tmux mouse mode for git-paw sessions. When enabled, you can click panes to switch, drag borders to resize, and scroll with the mouse wheel. This is set per-session and does not affect your other tmux sessions.

```toml
mouse = true  # default
```

### `worktree_placement`

Controls where git-paw creates an agent's worktree, relative to the
repository:

- `"child"` — inside the repo at `.git-paw/worktrees/<branch-slug>`. This is
  the **contained** layout: worktrees live under the project root, so a single
  permission grant for `.git-paw/worktrees/` covers every agent (no scattered
  sibling directories outside the repo). `git paw init` writes this for new
  repos.
- `"sibling"` — beside the repo at `../<project>-<branch-slug>` (the v0.7.0
  layout). This is the **default when the field is absent**, so pre-existing
  repos and sessions created before this field behave exactly as in v0.7.0.

```toml
worktree_placement = "child"
```

The `<branch-slug>` for the child layout is derived from the branch name
alone: `/` becomes `-` and characters outside `[A-Za-z0-9._-]` are stripped
(the project name is not prepended, since the directory already lives under
that project's `.git-paw/worktrees/`). For example branch `feat/auth-flow`
maps to `.git-paw/worktrees/feat-auth-flow/`, and `fix/issue#42` maps to
`.git-paw/worktrees/fix-issue42/`.

> **Gitignore note.** Child worktrees must be ignored or git would try to
> stage them as part of the repo. `git paw init` adds `.git-paw/worktrees/`
> to `.gitignore` automatically. If you opt into `worktree_placement =
> "child"` by editing the config manually (without re-running `git paw
> init`), add `.git-paw/worktrees/` to your `.gitignore` yourself.

Placement only governs **new** worktree creation. Existing worktrees stay
where they are: each session records the concrete worktree path it created,
and resume/status/purge operate on that recorded path — so flipping the
config mid-project never orphans an already-created worktree.

**Backfilled by `git paw init`.** Because `worktree_placement` is a top-level
default key, [re-running `git paw init`](#upgrading-an-existing-config) on a
config that omits it writes `worktree_placement = "child"` — the generated
default, not the `"sibling"` the field resolves to when absent. That is
deliberate: the backfill exists to bring a config up to the current defaults.

If the repo already has worktrees under the previous sibling layout, that
changes where the *next* worktree is created, so init prints a warning:

```text
warning: init backfilled worktree_placement = "child" — new agent worktrees will be created inside the repo at .git-paw/worktrees/.
  1 existing worktree(s) remain in their previous (sibling) location; init did not move them:
    /work/my-project-feat-auth
  To keep the previous layout, set worktree_placement = "sibling" in .git-paw/config.toml;
  otherwise leave it as "child" and new agents will use the contained layout.
```

Init **never moves, deletes, or re-registers a worktree** — it only warns. You
choose:

- **Keep the old layout** — set `worktree_placement = "sibling"` in
  `.git-paw/config.toml`. Init leaves an explicitly-set value alone on every
  later run.
- **Adopt the contained layout** — leave the backfilled `"child"`. Existing
  worktrees stay beside the repo and keep working (their recorded paths are
  unchanged); only newly created ones land in `.git-paw/worktrees/`.

A config that already sets `worktree_placement` (to either value) is never
touched, and the key is never duplicated.

### `docs_base_url`

Base URL of the documentation site the bundled [docs-fetch
skill](../user-guide/docs-fetch.md) consults on demand. Defaults to git-paw's
published documentation site; set it so a fork or mirror can point the helper
at its own docs.

```toml
docs_base_url = "https://bearicorn.github.io/git-paw"
```

The `docs-fetch.sh` helper resolves this value from `.git-paw/config.toml` for
both discovery (`llms.txt`) and page retrieval, falling back to the built-in
default when the field is absent. Documentation lookup is best-effort: an
unreachable site or missing page never blocks the agent.

## Worktree runtime provisioning

git-paw isolates each agent's *source* — every agent gets its own worktree —
but not its *runtime*. Sibling worktrees otherwise read the same `.env` and
collide on the same ports the moment two agents start a dev server, preview, or
debugger. The `[worktree]` table gives each worktree its own copy of the env
files you name, its own block of ports, and a pair of hooks for whatever else
your stack needs isolated.

Every sub-table is **opt-in**. With none present, worktree creation behaves
exactly as it did before this table existed: no files are copied, no
`.env.local` is generated, and no command runs. git-paw ships the mechanism
only — it assumes no filename, no port, and no stack, so you declare what your
project actually needs.

```toml
[worktree.env]
copy = [".env", ".env.development"]

[worktree.ports]
base   = 3000
stride = 10
vars   = ["PORT", "VITE_PORT"]

[worktree.hooks]
on_create = "scripts/paw-db-branch.sh {worktree_id}"
on_remove = "scripts/paw-db-drop.sh {worktree_id}"
```

### `[worktree.env]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `copy` | list of strings | `[]` | Repository-root-relative files copied into each new worktree |

Each entry is copied **verbatim** from the repository root into the new
worktree, after the worktree is created and before the agent's CLI starts — so
the agent's first read already sees a provisioned checkout.

The propagated file is an **independent copy, never a symlink**. That is
deliberate: a symlinked `.env` is shared mutable state across every agent, which
is the exact silent runtime corruption worktrees exist to prevent. A copy
diverges cleanly instead, and any drift surfaces at merge time.

A declared file that does not exist at the repository root is **skipped with a
warning** and the worktree is still created — per-branch divergence is normal
and a missing optional env file should not cost you a session. Entries that are
absolute paths, or that escape the repository root with `..`, are rejected with
a warning; provisioning only ever reads from the repository and writes into the
worktree.

### `[worktree.ports]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `base` | integer (u16) | `0` | First port of the first worktree's block |
| `stride` | integer (u16) | `0` | Block size per worktree |
| `vars` | list of strings | `[]` | Env-var names that receive sequential ports within the block |

Each worktree holds a **runtime slot** and its block starts at
`base + slot * stride`. The Nth entry of `vars` receives
`base + slot * stride + N`. With the example above, the first worktree gets
`PORT=3000` / `VITE_PORT=3001` and the second gets `PORT=3010` /
`VITE_PORT=3011`.

The assignments are written into a delimited managed block in the worktree's
`.env.local` — the override layer most stacks load last:

```
# >>> git-paw (managed) >>>
PORT=3000
VITE_PORT=3001
# <<< git-paw (managed) <<<
```

Anything you keep in `.env.local` outside that block is preserved, and
regenerating the block is idempotent. A copied `.env` is **never** modified;
offset ports live only in `.env.local`.

An empty `vars` list writes no port lines at all — git-paw supplies no default
port variables.

> **`stride` must be at least `vars.len()`.** A smaller stride would make
> adjacent worktrees' blocks overlap and hand two live agents the same port.
> git-paw clamps the stride up to the number of vars and warns when it loads a
> config that gets this wrong, but set it correctly — leave headroom for vars
> you add later.

> **Ports are deterministic, not probed.** git-paw does not scan the host for
> free ports, so an assigned port can still be occupied by an unrelated process.
> The trade is simplicity and stability: the same worktree keeps the same ports
> across restarts. If a block clashes with something else on your machine,
> change `base` or `stride`.

### Slots across add/remove churn

The slot is recorded in session state (`runtime_slot` on the worktree entry)
rather than derived from the worktree's position in the list. Position would
shift when a *middle* worktree is removed, handing a newly added agent a live
agent's ports.

A new agent takes the **lowest slot no live agent holds**, so blocks stay
bounded instead of climbing forever, and removing an agent frees its block for
the next `git paw add`. Two concurrently active worktrees are never assigned
overlapping blocks. Session state written before this field existed loads with
no slot and is assigned one on next provisioning.

### `[worktree.hooks]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `on_create` | string | unset | Command run in a new worktree after env/port provisioning, before the agent's CLI starts |
| `on_remove` | string | unset | Command run in the worktree at `git paw remove` / `git paw purge`, before the directory is deleted |

The third shared runtime resource — a **database** — cannot be isolated by a
built-in mechanism, because branching one is entirely stack-specific: a Neon or
PlanetScale branch API, a local `CREATE DATABASE`, a Docker volume, a Redis
namespace. git-paw therefore ships no driver and instead runs *your* command at
the two lifecycle points, letting you provision and tear down whatever
per-worktree resource your stack needs.

```toml
[worktree.hooks]
on_create = "scripts/paw-db-branch.sh {worktree_id}"
on_remove = "scripts/paw-db-drop.sh {worktree_id}"
```

Both keys are optional and independent — declare one, both, or neither. With no
`[worktree.hooks]` table, no command runs and worktree lifecycle is byte-identical
to configuring `[worktree.env]` / `[worktree.ports]` alone.

#### Placeholders

Before the command is executed, git-paw substitutes:

| Placeholder | Expands to |
|---|---|
| `{worktree_id}` | The worktree's stable identifier — the branch-derived slug that also names its directory (`feat/auth-flow` → `feat-auth-flow`) |
| `{worktree_path}` | The worktree's absolute path |

The id is restricted to `[A-Za-z0-9._-]`, so it is always safe to interpolate.
`{worktree_path}` is substituted verbatim — **quote it in your command**
(`db.sh '{worktree_path}'`) if your repository path may contain spaces.

Each command runs through `/bin/sh -c` **with the worktree as its working
directory**, so relative paths in your script resolve against the checkout and
`scripts/paw-db-branch.sh` means the copy in that worktree. Nothing extra is
injected into its environment.

#### `on_create` stdout becomes environment

git-paw reads `on_create`'s standard output and merges every `KEY=value` line
into **the same `.env.local` managed block** the port assignments are written
to. That is how a hook hands its connection details to the agent:

```console
$ cat scripts/paw-db-branch.sh
#!/bin/sh
createdb "paw_$1" >/dev/null
echo "DATABASE_URL=postgres://localhost/paw_$1"

$ cat .git-paw/worktrees/feat-two/.env.local
# >>> git-paw (managed) >>>
PORT=3010
VITE_PORT=3011
DATABASE_URL=postgres://localhost/paw_feat-two
# <<< git-paw (managed) <<<
```

A line counts as an assignment only if it is an identifier
(`[A-Za-z_][A-Za-z0-9_]*`) followed by `=`. Everything else — progress text,
blank lines, your script's chatter — is **ignored**, so a hook is free to be
talkative. The value is taken verbatim to end of line, so a URL keeps its own
`=` signs. Content outside the managed block is untouched, and re-provisioning
the same worktree is idempotent.

Because the ports are written *before* the hook runs, a hook that needs its
agent's port can read `.env.local` and use it.

#### Failure policy

The two hooks fail in deliberately opposite directions:

- **`on_create` non-zero fails that worktree's provisioning**, reporting the
  hook's exit status and its stderr. Handing an agent a half-built runtime is
  worse than refusing to start it.
- **`on_remove` non-zero only warns** and the removal continues. A failed
  teardown must never strand a worktree on disk — so write your teardown script
  to be **idempotent** and reconcilable out of band. git-paw does not track
  external resource state, and a crash or a force-purge can skip the hook
  entirely.

#### Secrets and trust

> **Print secrets to stdout, diagnostics to stderr.** git-paw logs only the
> *names* of the variables a hook merged, never their values, so a
> `DATABASE_URL` carrying a password never reaches a log line or an error. A
> failing hook's **stderr is surfaced**, so anything secret there would be
> printed.

> **`.env.local` is a plain file — gitignore it.** git-paw writes the hook's
> values into the worktree's `.env.local` with ordinary permissions. If your
> repository does not already ignore it, add it, or an agent may commit a
> credential.

> **Hooks run arbitrary consumer commands.** This is by design and is the same
> trust model as `[clis.*].command` and the `[supervisor]` command keys: the
> command comes from your repository's config file and runs with your
> privileges. git-paw never builds a hook out of broker, network, or
> agent-supplied input. Treat write access to `.git-paw/config.toml` as
> equivalent to command execution on your machine, and review changes to it in
> the same way you review a build script.

## Custom CLIs

Register custom AI CLIs that aren't in git-paw's built-in detection list.

A `[clis.<name>]` entry has two distinct names, and they are used for
different things:

- **`command` is what the pane launches.** Every launch path — the bare
  start, the spec-driven start, the supervised start, `git paw add`, and
  session recovery — spawns the entry's `command`, not the key. It is
  emitted as exactly one shell word: single-quoted (with embedded single
  quotes escaped) whenever it contains any character outside the portable
  safe set `A-Z a-z 0-9 _ - . / + , : @ %`, so a path containing spaces or
  shell metacharacters is neither word-split nor expanded. Approval flags
  and session-id / resume arguments are appended after it.
- **The `<name>` key is the CLI's identity.** Approval-flag, session-id and
  resume-template, prompt-profile, settle-delay, and MCP-capability lookups
  all resolve against the key, as does the `cli` recorded in each saved
  worktree entry. A CLI name with no matching entry (a built-in detected
  CLI, or an explicit pass-through name) launches under that name unchanged.

So `[clis.claude-variant] command = "claude"` launches `claude` while every
per-CLI override and the saved session still read `claude-variant`.

### Via config file

```toml
[clis.my-agent]
command = "/usr/local/bin/my-agent"   # absolute path
display_name = "My Agent"              # optional, shown in prompts

[clis.local-llm]
command = "ollama-code"               # binary name (resolved via PATH)
display_name = "Local LLM"

# A claude-family variant that reads a non-default config directory.
[clis.claude-variant]
command = "claude"
# Boot-prompt settle delay (ms) before the submit Enter. git-paw injects
# the boot block, waits this long for a paste-aware CLI to settle the
# paste, then sends Enter separately. Default suits most CLIs; raise it
# for a CLI whose large-paste handling needs longer before submit lands.
submit_delay_ms = 1500
# Path to this CLI's claude-format settings file. Parsed and preserved for
# compatibility only: git-paw never reads, writes, or seeds this file, and
# nothing in git-paw acts on it today. A leading `~` is expanded to the home
# directory.
settings_path = "~/.config/claude-variant/settings.json"
# Per-approval-level flag overrides, consulted BEFORE git-paw's built-in
# flag table when composing launch commands. Keys are the approval level
# names ("manual" | "auto" | "full-auto"); values are appended verbatim to
# the launch command at that level. This is how a variant CLI with no
# built-in table row gets native permission flags (e.g. full-auto for a
# claude-oss variant). Unknown level keys fail the config load.
approval_args = { "full-auto" = "--dangerously-skip-permissions" }
# Templates for assigning and resuming a worker CLI's own session identity.
# Each MUST contain the {{SESSION_ID}} placeholder; a template lacking it is
# rejected at config load, naming the key. Resolution is override → built-in
# → none, same order as approval_args. The built-in table covers `claude`
# (`--session-id {{SESSION_ID}}` / `--resume {{SESSION_ID}}`); a CLI without
# a built-in row and without these overrides launches unchanged (no id
# assigned, no resume on recovery).
session_id_args = "--session-id {{SESSION_ID}}"
resume_args = "--resume {{SESSION_ID}}"
# Whether this CLI speaks MCP, deciding which form of the coordination boot
# block it receives. Unset by default (and omitted from written config), in
# which case the built-in MCP-capable set decides. Only set `true` once
# git-paw's MCP server is registered for THIS CLI — see the field table.
# mcp = true

# Per-field overrides of the CLI's terminal-shape markers (readiness
# banner, approval-prompt wording, …), consulted before the embedded
# Claude Code default. See "CLI Prompt-Shape Profiles" in the user guide.
[clis.claude-variant.prompt_profile]
approval_markers = ["proceed? (y/n)"]
```

| Field | Required | Purpose |
|-------|----------|---------|
| `command` | yes | Command or path to the CLI binary — what the pane launches, emitted as one shell word. Must be non-empty; a blank value is rejected at config load, naming the entry. |
| `display_name` | no | Human-readable name shown in prompts. |
| `submit_delay_ms` | no | Boot-prompt settle delay (ms) before the submit `Enter`; per-CLI so the launcher stays CLI-agnostic. |
| `settings_path` | no | Path to the CLI's claude-format settings file. **Parse-only**: accepted and preserved across a load/save round-trip so existing configurations keep working, but git-paw never reads, writes, or acts on it — not even [`git paw setup-cli`](../user-guide/cli-setup.md), whose only target is the main checkout's project-local settings file. |
| `approval_args` | no | Map from approval-level name (`"manual"`, `"auto"`, `"full-auto"`) to the flags appended to the launch command at that level. Consulted **before** the [built-in flag table](#supervisor); an unknown level key is rejected at config load with an error naming the key. |
| `session_id_args` | no | Template (containing `{{SESSION_ID}}`) for the flag(s) that assign a fresh session identifier at launch. Consulted **before** the built-in table (`claude` only); a CLI resolving neither launches unchanged, with no id recorded. |
| `resume_args` | no | Template (containing `{{SESSION_ID}}`) for the flag(s) that resume a previously assigned session. Appended on recovery when the worktree's recorded id and this CLI's resolved template are both present; a template omitting the placeholder is rejected at config load. |
| `prompt_profile` | no | Per-field overrides of the CLI's terminal-shape markers, consulted before the embedded Claude Code default. Carries no permission policy — see [CLI Prompt-Shape Profiles](../user-guide/cli-prompt-profiles.md). |
| `mcp` | no | Boolean. Whether this CLI identity speaks MCP, selecting the MCP publish-tool form of the coordination boot block over the `broker.sh` helper form. Unset by default and omitted when the config is written back. |

`settings_path` used to feed a protected-path set that the auto-approve
classifier consulted before approving a write. That classifier is retired, so
the field now has no effect at all — it is kept so existing configurations
load and round-trip unchanged. Agent memory isolation is carried by the
bundled skills' doctrine, and mechanically by deny rules in the agent CLI's
own settings or by the [sandbox](../user-guide/sandbox.md). See
[Agent permissions](../user-guide/supervisor.md#agent-permissions).

### `mcp` and the boot-block form

Whether a CLI receives the MCP tool form of the
[coordination boot block](../user-guide/coordination.md#mcp-tool-form) is
resolved in this order:

1. `mcp = true` or `mcp = false` on the matching `[clis.<name>]` entry;
2. otherwise the built-in MCP-capable set — exactly `claude`, `codex`,
   `cursor-agent`, and `agy`;
3. otherwise not MCP-capable, so the `broker.sh` helper form is used.

The lookup uses the CLI **identity** (the `[clis.<name>]` key, or the
built-in / pass-through name), never the resolved `command`. That is why
`[clis.claude-oss] command = "claude"` does not inherit `claude`'s MCP form:
a variant typically reads a different config directory, which may not have
git-paw's MCP server registered.

> **Warning:** setting `mcp = true` for a CLI that does not have git-paw's
> MCP server registered strands the agent at boot — the boot block names
> publish tools the agent cannot call, so its very first coordination step
> fails. Register the server for that CLI first (see
> [MCP](../user-guide/mcp.md)), then opt in.

A config without an `mcp` key loads and behaves exactly as it did before the
field existed.

### Files git-paw writes

`setup-cli` adds no configuration field. It does write two files outside
`.git-paw/`, both only when you run it:

| Path | What it is |
|---|---|
| `<main checkout>/.claude/settings.local.json` | The `claude` allowlist item's only target. Merged into — every key and its order are preserved. |
| `<git-common-dir>/git-paw/cli-setup.json` | The setup record: what git-paw wrote, so `--undo` can take back exactly that. Owner-only, inside the git directory, so it can never be committed and never shows up in any worktree's `git status`. |

See [Per-CLI Setup](../user-guide/cli-setup.md).


### Via command line

```bash
# Add with absolute path
git paw add-cli my-agent /usr/local/bin/my-agent

# Add with binary name on PATH
git paw add-cli my-agent my-agent

# Add with display name
git paw add-cli my-agent my-agent --display-name "My Agent"

# Remove
git paw remove-cli my-agent
```

The `add-cli` and `remove-cli` commands modify the **global** config (see [Config File Locations](#config-file-locations)). `add-cli` stores a binary name given on the command line as its resolved absolute path.

### Listing CLIs

```bash
git paw list-clis
```

Shows both auto-detected and custom CLIs with their source:

```
NAME            SOURCE     PATH
claude          detected   /usr/local/bin/claude
codex           detected   /usr/local/bin/codex
my-agent        custom     /usr/local/bin/my-agent
```

## Presets

Presets save branch + CLI combinations for one-command launch.

### Defining presets

```toml
[presets.backend]
branches = ["feature/api", "fix/db-migration"]
cli = "claude"

[presets.full-stack]
branches = ["feature/api", "feature/ui", "feature/styles"]
cli = "agy"
```

### Using presets

```bash
git paw start --preset backend
```

This skips all interactive prompts and launches with the preset's branches and CLI.

## Specs

Configure spec file scanning for `--from-all-specs` and `--specs` mode.

```toml
[specs]
dir = "specs"         # Directory containing spec files (relative to repo root)
type = "openspec"     # "openspec" (OpenSpec changes), "markdown" (flat .md files), "speckit" (GitHub Spec Kit), or "superpowers" (obra/superpowers plans)
```

| Field | Default | Description |
|-------|---------|-------------|
| `dir` | `"specs"` | Directory to scan for spec files |
| `type` | `"openspec"` | Spec backend: `"openspec"` (directory-based OpenSpec changes), `"markdown"` (flat `.md` files with YAML frontmatter), `"speckit"` ([GitHub Spec Kit](https://github.com/github/spec-kit) `.specify/specs/<feature>/`), or `"superpowers"` ([obra/superpowers](https://github.com/obra/superpowers) `docs/superpowers/plans/*.md`) |

git-paw does not auto-detect the spec system from the filesystem: the `[specs]`
section is the source of truth. `git paw init` prompts you to choose the spec
system and writes this section. When `[specs]` is omitted, launching from specs
errors until you add it — or pass the `--specs-format` CLI flag, which selects
the spec system for a single launch.

See [Spec-Driven Launch](../user-guide/spec-driven-launch.md) for format details.

## Logging

Configure session output logging.

```toml
[logging]
enabled = true
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Whether to capture pane output to log files |

When enabled, logs are written to `.git-paw/logs/<session>/` using `tmux pipe-pane`. See [Session Logging](../user-guide/session-logging.md) for details.

## Broker

Configure the HTTP broker for agent coordination. When enabled, git-paw starts a lightweight HTTP server that lets agents share status updates, artifacts, and blocked requests.

```toml
[broker]
enabled = true
port = 9119
bind = "127.0.0.1"
wave_stall_minutes = 30
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Whether to start the coordination broker |
| `port` | `9119` | HTTP port for the broker server |
| `bind` | `"127.0.0.1"` | Bind address -- never bind to `0.0.0.0` on shared machines |
| `wave_stall_minutes` | `30` | Minutes without wave progress after which the broker reports the wave as stalled on `GET /status`, in `git paw status`, and on the dashboard status line. "Progress" is a wave phase advancing or a branch tip moving, and nothing else — see [Wave staleness](../user-guide/coordination.md#wave-staleness). `0` disables the `stalled` flag while still reporting `idle_seconds`. Omitted → the documented default |

The default of 30 minutes sits just above the unattended drive loop's 25-minute heartbeat, so a wave that is merely between heartbeats is not reported as stalled. A config written before the key existed loads unchanged and gets the default.

When the broker is enabled, git-paw injects the `GIT_PAW_BROKER_URL` environment variable into each agent pane, pointing to `http://<bind>:<port>`. Agents use this URL to communicate with the broker.

> **Warning — a non-loopback `bind` removes a protection and adds exposure.**
> The broker authenticates nothing: whoever reaches the port can read the whole
> coordination log and publish any message, including guidance that coding
> agents will act on — up to whatever their CLI's permission mode lets them
> run without asking. On the default
> loopback `bind`, git-paw checks every request's `Host` header so a web page
> cannot reach the broker by DNS rebinding. Setting `bind` to `"0.0.0.0"` or a
> LAN address **disables that check** — git-paw cannot know your network's
> names for the machine — and makes the broker reachable from the network with
> no authentication at all. It also **stops blocking DNS rebinding**: the
> `Origin` check still runs on every bind and still blocks browser writes and
> cross-origin reads, but a rebinding page is same-origin with the broker and
> its `GET`s carry no `Origin`, so it can read `/log`, `/status` and
> `/messages/*`. Only a loopback bind prevents that. Both the broker and the
> command that launches the session print a warning on stderr when you set a
> non-loopback bind. See [Security Posture](../user-guide/security-posture.md#browser-exposure--the-brokers-host-and-origin-guard).

### Filesystem watcher

The broker's filesystem watcher publishes `agent.status: working` whenever an agent's `git status` changes. By default, an agent that publishes `agent.artifact status: "committed"` and then keeps editing is re-entered into the `working` state — a file modification observed within a TTL window after the commit re-publishes `working`, so the dashboard reflects the agent's continued activity instead of sticking on `committed`.

```toml
[broker.watcher]
republish_working_ttl_seconds = 60
```

| Field | Default | Description |
|-------|---------|-------------|
| `republish_working_ttl_seconds` | `60` | Seconds after a `committed` event during which a file write re-publishes `working`. `0` disables the auto-republish (restoring the prior "committed is terminal until the agent itself republishes" behaviour). Non-zero values below `5` are clamped to `5` with a warning. |

Past the TTL window the agent is considered settled at `committed`; only an explicit `agent.status` from the agent itself transitions it out.

### Multi-repo port assignment

If you run git-paw sessions for multiple repositories at the same time, each session needs a different port. Set a unique `port` in each repo's `.git-paw/config.toml`:

```toml
# Repo A
[broker]
enabled = true
port = 9119

# Repo B (in its own .git-paw/config.toml)
[broker]
enabled = true
port = 9120
```

This is required, not advisory. Before binding, git-paw probes the configured port and asks the listener which repository it serves (`GET /status` reports `repo_root`). A broker for *this* repository is reattached to; a broker for a *different* repository is refused:

```
port 9119 is held by a git-paw broker for another repository
(/home/you/other-repo) — set a distinct [broker] port in
.git-paw/config.toml for one of the repositories
```

Give one of the two repositories its own `port` and start again. The refusal exists because reattaching would route this repository's agents into the other repository's broker while this repository's dashboard rendered an empty, unwatched state.

**The check runs before anything is created — and before you are asked anything.** `git paw start` (including `--from-specs`, the supervised launch, and session recovery) probes the port as soon as it has read your config, ahead of the branch and CLI pickers, so you are never asked to choose for a launch that cannot proceed and a refused launch leaves no session, no panes, and no session receipt behind. That ordering matters: git-paw injects `GIT_PAW_BROKER_URL` into every agent pane, so a refusal that came only when the dashboard started would already have handed your agents the other repository's broker.

Recovering a stopped session probes the port recorded in the saved session rather than the configured one, because that is the URL the rebuilt agent panes are given — they can differ if you changed `[broker] port` after the session was saved. `--dry-run` creates nothing and never probes.

`git paw status` and `git paw doctor` use the same repository-aware probe, so neither reports a neighbouring repository's broker as your session's own:

```
Broker:  http://127.0.0.1:9119 (held by another repository's git-paw broker)
```

```
⚠ broker          enabled — 127.0.0.1:9119 is held by another repository's git-paw broker
                  → set a distinct `[broker] port` in .git-paw/config.toml for one of the repositories
```

A variant of the message appears when the port holder does not report a repository at all:

```
port 9119 is held by a git-paw broker for another repository (it does not
report which — an older git-paw broker may be holding the port) — set a
distinct [broker] port in .git-paw/config.toml for one of the repositories
```

An unknown owner is never assumed to be yours. This is what you see when a session launched by an older git-paw is still running on the port — stop that session, or move this one to a different port.

A port held by something that is not git-paw at all reports the separate `port ... is already in use by another process` error.

Each session also keeps its own broker log, `<session_name>.broker.log`, so two repositories never write to — or purge — the same file. A session saved by an earlier git-paw recorded a single shared `broker.log`; that recorded path stays authoritative for both the broker and `purge` until the session is resumed or recovered, which migrates it to the per-session name.

See [Agent Coordination](../user-guide/coordination.md) for usage details.

## Supervisor

Configure the supervisor agent for orchestrating parallel coding sessions. When enabled, the supervisor monitors agents, runs tests, verifies work, and coordinates merges.

```toml
[supervisor]
enabled = true
cli = "claude"
test_command = "just check"
lint_command = "cargo clippy -- -D warnings"
build_command = "cargo build"
fmt_check_command = "cargo fmt --check"
doc_build_command = "mdbook build docs/"
doc_tool_command = "cargo doc --no-deps"
spec_validate_command = "openspec validate {{CHANGE_ID}} --strict"
security_audit_command = "cargo audit"
agent_approval = "auto"
approval = "full-auto"
verify_on_commit_nudge = true
strict_branch_guard = true
manual_approvals_log = true
no_progress_window_seconds = 1500
context_bloat_threshold_k = 250
blocked_on_supervisor_window_seconds = 900
branch_refresh = false
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Whether to use supervisor mode by default (can also use `--supervisor` flag, or override with `--no-supervisor` for a single session) |
| `cli` | (uses `default_cli`) | The orchestrator's CLI, named in its context (`.git-paw/AGENTS.local.md`); no pane runs it, and no orchestrator CLI is needed to launch. Also the coding agents' last CLI fallback (`--cli` > `default_cli` > `[supervisor].cli`) |
| `test_command` | (none) | Test runner — gate 1 (e.g. `"just check"`, `"cargo test"`, `"npm test"`, `"pytest"`) |
| `lint_command` | (none) | Lint check — gate 1 (e.g. `"cargo clippy -- -D warnings"`, `"npm run lint"`, `"ruff check ."`, `"golangci-lint run"`) |
| `build_command` | (none) | Compile step — gate 1 when build is distinct from test (e.g. `"cargo build"`, `"npm run build"`, `"mvn package"`, `"go build ./..."`) |
| `fmt_check_command` | (none) | Formatter check — gate 1 (e.g. `"cargo fmt --check"`, `"prettier --check ."`, `"gofmt -l ."`, `"black --check ."`) |
| `doc_build_command` | (none) | Documentation build — gate 4 (e.g. `"mdbook build docs/"`, `"sphinx-build"`, `"mkdocs build"`) |
| `doc_tool_command` | (none) | API-doc generator — distinct from `doc_build_command`; gates the per-language extractor for changed public items (e.g. `"cargo doc --no-deps"`, `"sphinx-build -W docs docs/_build"`, `"javadoc"`, `"npx typedoc"`, `"go doc"`). Renders empty (not `(not configured)`) when unset so the surrounding prose reads naturally |
| `spec_validate_command` | (none) | Spec validator — gate 3 (e.g. `"openspec validate {{CHANGE_ID}} --strict"` for OpenSpec). `{{CHANGE_ID}}` is substituted by the supervisor agent at verification time with the change name being audited; it is **not** expanded at config load |
| `security_audit_command` | (none) | Security audit tooling — gate 5 (e.g. `"cargo audit"`, `"npm audit"`, `"bandit -r ."`, `"gosec ./..."`) |
| `agent_approval` | `"auto"` | Permission level for coding agents: `"manual"`, `"auto"`, or `"full-auto"` |
| `approval` | (unset) | The orchestrator's **own** permission level: `"manual"`, `"auto"`, or `"full-auto"`, distinct from `agent_approval`. Still parsed, with no warning, so earlier configs load and launch unchanged — but no pane runs an orchestrator, so it changes **no** pane command; it is reserved for git-paw-initiated orchestrator invocations. Coding agents resolve from `agent_approval` alone |
| `verify_on_commit_nudge` | `true` | When on, the broker posts a `supervisor.verify-now` message to the supervisor inbox on every `agent.artifact { status: "committed" }`, so the supervisor verifies each commit promptly on an explicit event instead of batching. Set `false` to fall back to sweep-cadence verification |
| `strict_branch_guard` | `true` | When `true`, a per-worktree **pre-commit** hook refuses any commit whose checked-out branch differs from the branch the worktree was created for, blocking cross-worktree contamination (linked worktrees share `.git/refs`, so a stray `cd` can otherwise advance the wrong branch). Set `false` to disable *enforcement* — the **post-commit** hook still publishes an `agent.feedback` + `agent.learning` record when it detects a mismatch (detection without enforcement) |
| `manual_approvals_log` | `true` | When `true`, every live permission prompt the `--unattended` drive loop escalates for a human decision is appended to `.git-paw/sessions/<session>.manual-approvals.jsonl` and surfaced via [`git paw approvals`](../cli-reference.md). Applies to `--unattended` runs only — an attended session runs no drive loop and records nothing. On a pattern's first sighting a `permission_pattern` learning is also emitted (when `learnings = true`). Set `false` to suppress both the log writes and the learnings emission; the opt-out affects writes only, so `git paw approvals` still reads any pre-existing log |
| `no_progress_window_seconds` | `1500` (~25 min) | Read by `.git-paw/scripts/sweep.sh`: an agent is flagged `no-progress` when BOTH its completed-task-checkbox count AND its branch commit count stay unchanged for this many seconds. Set longer to tolerate long build/research steps, shorter to nudge sooner. Omitted → the documented default |
| `context_bloat_threshold_k` | `250` (thousand tokens) | Read by `.git-paw/scripts/sweep.sh`: when an agent's pane shows a `/clear to save <N>k tokens` hint whose `N` meets or exceeds this value, the agent is proactively flagged `context-bloat` so the supervisor can pre-empt the eventual freeze. Omitted → the documented default |
| `blocked_on_supervisor_window_seconds` | `900` (~15 min) | Read by `.git-paw/scripts/sweep.sh`: an agent whose latest unanswered `agent.blocked` names the supervisor as the blocker is flagged `blocked-on-supervisor` once it has waited longer than this window, forcing the supervisor to answer. Omitted → the documented default |
| `branch_refresh` | `false` | When `true`, the [unattended drive loop](../user-guide/supervisor.md#unattended-mode---unattended) rebases an idle, clean worker branch onto the default branch after a successful merge — see [Branch refresh](../user-guide/supervisor.md#branch-refresh-opt-in) for the full precondition gate list. Default off: history is rewritten under a live agent only on explicit opt-in |
| `learnings` | `false` | Master switch for [learnings mode](#learnings-mode-tuning): when `true` (and the broker is enabled), the session's friction signals are appended to `.git-paw/session-learnings.md`. Tuned by the `[supervisor.learnings_config]` sub-table |
| `auto_revert` | `false` | Used by [opsx role gating](#opsx-role-gating) in `block` mode: when `true`, the supervisor reverts a coding agent's archive commit without first confirming with you. Default off — a revert always asks first |

**Approval flag resolution.** When composing a pane's launch command, git-paw
resolves the permission flags for a `(CLI, level)` pair in order: (1) the
per-CLI [`[clis.<name>] approval_args`](#custom-clis) override, (2) the
built-in table below, (3) no flags. Coding-agent panes always resolve at
`agent_approval`; no pane resolves at `approval`. This resolution applies on **every**
launch path that spawns a worker CLI — supervisor mode, a bare `git paw
start`, a non-supervisor `--specs` / `--from-all-specs` launch, and `git paw
add` alike — not only supervisor sessions, even though `agent_approval` is
declared under `[supervisor]`. A bare or spec-driven launch with no
`[supervisor]` section at all still resolves `agent_approval` against its
default (`"auto"`).

| CLI | `full-auto` | `auto` |
|-----|-------------|--------|
| `claude` | `--dangerously-skip-permissions` | `--permission-mode acceptEdits` |
| `codex` | `--dangerously-bypass-approvals-and-sandbox` | `--sandbox workspace-write` |
| `agy` | `--dangerously-skip-permissions` | (none) |
| `qwen` | `--yolo` | (none) |

Any other CLI (or `manual` anywhere) resolves to no flags — use
`approval_args` to supply native flags for CLIs not in the table.

`auto` resolves to a deterministic flag wherever the CLI has a low-friction
"accept edits" mode distinct from `manual` and from full bypass, rather than
the empty string — an `auto` pane no longer silently inherits whatever
default its CLI's config directory happens to carry. For Claude,
`--permission-mode acceptEdits` has the CLI accept file edits without asking;
shell commands still raise a prompt.

These resolved flags are git-paw's only **automatic** influence on what an
agent runs without a prompt. git-paw approves no prompt itself: anything the
chosen mode leaves prompting is escalated for a human decision under
`--unattended`, and answered only by an explicit `sweep.sh approve`.

One further influence exists, and only because you ask for it:
[`git paw setup-cli <CLI> --allowlist`](../user-guide/cli-setup.md) writes
path-scoped allow rules for git-paw's own helper scripts into the agent CLI's
settings. Running that command is the consent — no launch, recovery, drive
loop, dashboard or MCP tool ever applies it, it prints its plan before
writing, and `--undo` removes exactly what it added. Those rules are then the
**CLI's** rules, enforced by the CLI's own permission system; git-paw still
decides nothing about any prompt. See
[Agent permissions](../user-guide/supervisor.md#agent-permissions).

**Gate-command templating.** The eight `*_command` keys feed the supervisor
skill's five verification gates (testing, regression analysis, spec audit, doc
audit, security audit). For each key set on this section, the supervisor skill
substitutes the matching `{{...}}` placeholder at session boot and the
supervisor agent runs the literal command during that gate. For each key
**omitted**, the placeholder renders as `(not configured)` and the supervisor
agent skips that tooling step — the gate's manual review still applies (e.g.
the OWASP-category diff scan for the security gate, the spec scenario
coverage check for the spec gate). Pre-v0.5.x configs that did not name any
of the six new keys continue to work; they just run a less-rigorous
verification cycle until the keys are filled in. A user wanting to explicitly
opt out of a single gate's tooling can set the field to `"(not configured)"`
verbatim — the supervisor agent recognises that as the same skip token.

**Resolution chain** — git-paw picks supervisor mode using the first matching rule:

1. `--no-supervisor` → off (highest precedence; overrides everything below).
2. `--supervisor` → on.
3. `[supervisor] enabled = true` → on.
4. `[supervisor] enabled = false` → off.
5. No `[supervisor]` section + `--dry-run` → off.
6. No `[supervisor]` section + interactive TTY → prompts you.
7. No `[supervisor]` section + non-TTY → off.

`--supervisor` and `--no-supervisor` are mutually exclusive — passing both produces a parse error.

**Approval levels:**

| Level | Behavior |
|-------|----------|
| `manual` | Agents prompt for every action (safest, slowest) |
| `auto` | Low-friction mode — the CLI's mapped `auto` flag from the table above (e.g. accept-edits for Claude); a CLI with no mapped flag keeps its default behavior |
| `full-auto` | Skip all permission prompts (fastest, agents run unattended) |

The supervisor translates the approval level into CLI-specific flags at launch (e.g. `--dangerously-skip-permissions` for Claude in full-auto mode).

### Deprecated auto-approval tables

`[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` are
**deprecated and have no effect.** They configured a git-paw command classifier
that decided an agent's command was safe and then answered the permission
prompt for it. That classifier is retired: three successive security reviews
each found new command shapes it wrongly called safe, and each fix opened the
next hole. git-paw now makes no safety judgement about an agent's command and
approves no permission prompt.

Both tables, and every field in them, **still parse and round-trip**, so an
existing configuration loads and saves unchanged — nothing breaks and no
migration is forced. But:

- none of their values is consulted by anything;
- each configuration file that declares either table prints **one deprecation
  warning per process**, naming the tables it found, on stderr;
- `git paw init` no longer writes either table into the default config.

```toml
# Deprecated — parsed, warned about, and ignored.
[supervisor.auto_approve]
enabled = true
safe_commands = ["just lint", "just test"]
stall_threshold_seconds = 30
approval_level = "safe"
approve_worktree_writes = true

[supervisor.common_dev_allowlist]
enabled = true
stacks = ["rust"]
extra = ["just check"]
```

Delete them whenever convenient. A configuration that git-paw v0.16.0 saved may
carry a `[supervisor.common_dev_allowlist]` table you never wrote by hand; it
will warn until you remove it.

**What to use instead.** Deciding whether a command may run is the agent CLI's
job:

- Move `safe_commands`, `stacks` and `extra` entries into **your agent CLI's own
  allow rules** — its project-level or user-level allowlist. That is the only
  place a standing grant can live, and it is what makes a routine `cargo test`
  stop prompting. `git paw approvals` reports which prompts recur, with a hint
  for which of those two allowlists fits.
- For a run that must not stop for prompts at all, set
  `[supervisor] approval = "full-auto"` and run inside the documented
  [sandbox](../user-guide/sandbox.md). Note that `full-auto` skips the prompts
  entirely, so nothing is escalated either.
- `approve_worktree_writes` and the protected-path and `.git/`-write rules it
  interacted with are gone. Those writes now reach a human like any other
  prompt; for mechanical enforcement use deny rules in the CLI's own settings,
  or the sandbox.

See [Agent permissions](../user-guide/supervisor.md#agent-permissions) and the
[Security Posture](../user-guide/security-posture.md).

> **Durable grants the classifier already accepted.** Up to v0.16.0 the
> classifier preferred the "Yes, and don't ask again" option for prompts it
> judged safe, and your agent CLI persisted those as permanent permission rules
> in its own settings file. Retiring the classifier stops new ones being
> created; it removes none. Review that file and delete grants you did not
> intend — git-paw never edits or deletes it.

### Conflict detector tuning

When supervisor mode is enabled, the broker runs an in-process conflict
detector that auto-emits `agent.feedback` (and optionally `agent.question`)
on forward, in-flight, and ownership conflicts. See
[Agent Coordination § Automatic Conflict Detection](../user-guide/coordination.md#automatic-conflict-detection-v050) for the runtime semantics; the table below
documents the configuration surface.

```toml
[supervisor.conflict]
window_seconds = 120
warn_on_intent_overlap = true
escalate_on_violation = true
```

| Field | Default | Description |
|-------|---------|-------------|
| `window_seconds` | `120` | Seconds the detector waits before escalating an unresolved in-flight conflict to the supervisor inbox via `agent.question`. |
| `warn_on_intent_overlap` | `true` | Master switch for forward-conflict feedback. When `false`, two agents declaring overlapping `agent.intent` files no longer trigger `agent.feedback`, but the intent tracker still records them (so in-flight and ownership detection continue to work). |
| `escalate_on_violation` | `true` | Whether ownership violations escalate to the supervisor inbox. When `false`, the violator still receives `agent.feedback`, but no follow-up `agent.question` lands in the supervisor inbox. |

The `[supervisor.conflict]` table is fully optional. A v0.4 config with
`[supervisor]` and no `[supervisor.conflict]` loads cleanly with every field
at the defaults above. Setting `[supervisor] enabled = false` (or omitting
the section) disables the detector subsystem entirely — no auto-emitted
warnings fire regardless of the values here.

### Correction loop tuning

The `[supervisor.correction]` table governs the self-healing correction cycle
the `--unattended` drive loop runs when a supervisor gate fails: instead of
publishing `agent.feedback` into an inbox a blocked worker never polls, the
loop injects the gate-tagged feedback straight into the worker's pane and
re-verifies. See
[Supervisor § Unattended correction loop](../user-guide/supervisor.md#unattended-correction-loop)
for the runtime walkthrough.

```toml
[supervisor.correction]
auto_loopback = true       # default false — opt in to enable the loop
max_cycles = 5
on_exhausted = "escalate"  # "escalate" (default) | "abandon"
escalate_after_cycles = 3
gate_tags = ["testing", "regression", "spec audit", "doc audit", "security audit", "scope"]
```

| Field | Default | Description |
|-------|---------|-------------|
| `auto_loopback` | `false` | Whether a failing gate automatically re-engages the worker's pane with that gate's feedback. **Off by default** so an existing unattended setup behaves identically after an upgrade; the recommended config above turns it on. When `false`, the feedback is published exactly as before and no keystrokes are sent. |
| `max_cycles` | `5` | Verify↔fix rounds the loop drives for one branch before giving up and applying `on_exhausted`. A branch that passes its gate leaves the cycle early and its counter resets. Must be a positive integer — `0` is a config error; to turn the loop off set `auto_loopback = false`. |
| `on_exhausted` | `"escalate"` | What to do with a branch that reaches `max_cycles`. `"escalate"` flags it to the orchestrator/human as unrecoverable; `"abandon"` marks it failed and leaves it un-corrected without an escalation. Neither re-engages the branch again. Any other value is a config error naming the accepted set. |
| `escalate_after_cycles` | `3` | Cycle count at which a slow-converging worker is flagged **once**, as an early heads-up, while re-engagement continues to `max_cycles`. No distinct early flag is emitted when this is greater than or equal to `max_cycles`. Must be a positive integer — `0` is a config error; to suppress the flag, set it to `max_cycles` or higher. |
| `gate_tags` | `["testing", "regression", "spec audit", "doc audit", "security audit", "scope"]` | Gate names recognised as a `[<gate>]` prefix on a supervisor `agent.feedback` error line; only a matching tag (case-insensitive) counts as a failing gate verdict that starts a correction cycle. The default reproduces git-paw's own five-gate framework plus `scope` exactly. A consumer whose review process names different gates lists them here instead — the correction *loop* itself is unaffected, only the tag vocabulary it recognises. Must be non-empty; an empty list is a config error (it would silently disable the loop even with `auto_loopback = true` — set `auto_loopback = false` instead). |

The `[supervisor.correction]` table is fully optional. A v0.13.0 config with
`[supervisor]` and no `[supervisor.correction]` loads cleanly with every field
at the defaults above — and because `auto_loopback` defaults to `false`, its
observable behaviour is unchanged. Cycle counts live in the drive loop's
in-process state and are **not persisted**: restarting the supervisor resets
every branch's budget.

When `[supervisor] learnings = true`, a branch that exhausts its budget also
appends a `correction_exhausted` learning carrying the branch, the worker CLI,
and the cycle count — the signal that tells you a worker's model tier is
under-powered for a class of task. With learnings disabled, nothing is
recorded.

### Routing through the supervisor (`/tell`)

The `[supervisor.tell]` table tunes the `/agents` and `/tell` commands you
type to the orchestrator to route prompts to individual agents (see
[Routing through the supervisor](../user-guide/supervisor.md#routing-through-the-supervisor)).

```toml
[supervisor.tell]
mode = "feedback"
inventory_max_age_seconds = 60
```

| Field | Default | Description |
|-------|---------|-------------|
| `mode` | `"feedback"` | Default delivery channel for `/tell`. `"feedback"` queues an `agent.feedback` the target picks up on its next inbox poll (safe for mixed-mode sessions). `"send-keys"` injects the prompt straight into the target pane with `tmux send-keys` — used only when the target is in accept-edits mode, otherwise `/tell` falls back to `feedback` and prints a note. |
| `inventory_max_age_seconds` | `60` | How stale the cached `/agents` inventory may be before `/tell`/`/agents` re-poll the broker. Lower it for tighter freshness at the cost of more frequent polling. |

The `[supervisor.tell]` table is fully optional. A v0.5.0 config with
`[supervisor]` and no `[supervisor.tell]` loads with both defaults above and
round-trips unchanged.

### Learnings mode tuning

When supervisor mode is active, the parent `[supervisor] learnings = true`
flag (default `false`) activates the learnings subsystem. Entries are
appended to `.git-paw/session-learnings.md` covering the four deterministic
categories (conflict events, where agents got stuck, recovery cycles,
permission patterns — see [Learnings](../user-guide/learnings.md)). The
`[supervisor.learnings_config]` sub-table tunes the flush cadence; the
master switch lives on the parent table.

```toml
[supervisor]
learnings = true

[supervisor.learnings_config]
flush_interval_seconds = 60
broker_publish = "auto"
```

| Field | Default | Description |
|-------|---------|-------------|
| `flush_interval_seconds` | `60` | How often the learnings aggregator flushes accumulated entries from memory to `.git-paw/session-learnings.md`. The file is append-only across sessions; a longer interval batches more entries per write. |
| `broker_publish` | `"auto"` | Whether flushed entries are *also* published to the broker as `agent.learning` messages. `"auto"` follows `[broker] enabled` (publish when the broker is running, file-only when it is not); `"force_off"` keeps file-only output even with an active broker. The Markdown file is written either way. |

There are **no configuration fields for the qualitative signals**
(`recurring_failure_shape`, `doc_gap`, `adr_drift`, `scope_mistake`,
`tooling_friction`). Their detection thresholds live in the supervisor skill
prose, not in `[supervisor.learnings_config]` — to tune how readily the
supervisor publishes a category, edit your local copy of the supervisor
skill rather than a config value. The supervisor publishes each qualitative
learning through the bundled `.git-paw/scripts/sweep.sh learn` helper, so no
broad `curl` allowlist grant is required.

See the [Learnings Mode chapter](../user-guide/learnings.md) for the
category-by-category walkthrough, the output-file format, and how to consume
learnings programmatically via the `agent.learning` broker variant and the
MCP `get_learnings()` tool.

## opsx role gating

```toml
[opsx]
role_gating = "warn"  # "warn" (default) | "block" | "off"
```

git-paw cannot add a permission check to the `/opsx:verify` and `/opsx:archive`
slash commands themselves (they live in the OpenSpec project). Instead, when the
session's spec engine is OpenSpec, a post-commit guard watches for **archive
activity committed from a coding-agent worktree** and reacts per `role_gating`:

| Mode | Behaviour on a coding-agent archive |
|---|---|
| `warn` (default) | publish an `agent.feedback` to the offending agent **and** record an `agent.learning` with category `permission_pattern` |
| `block` | warn behaviour **plus** publish an `agent.feedback` to the supervisor requesting it revert the offending commit (the supervisor performs the revert via its merge-orchestration skill — git-paw never runs `git revert` itself) |
| `off` | guard disabled entirely |

The guard is **inert under non-OpenSpec engines** (`speckit`, `markdown`, or no
spec source) regardless of the mode, and the `/opsx:` forbidden-command sections
are omitted from the bundled coordination/supervisor skills there too.

A commit is treated as archive activity when **either** its message matches the
canonical archive shape `chore(specs): archive <name>; sync deltas to main
specs` **or** its diff moves files into `openspec/changes/archive/<name>/`
and/or adds a main spec under `openspec/specs/<capability>/spec.md`. The
supervisor's own archives (`agent_id == "supervisor"`) never count as a
violation. See the [Supervisor guide](../user-guide/supervisor.md#opsx-role-gating)
for how to read the warning text and tune `block` mode with
`[supervisor] auto_revert`.

> **v0.6.0 behaviour change.** `role_gating` defaults to `warn`. Sessions where a
> coding agent archives a change will now see guard feedback and a learnings
> record. Set `role_gating = "off"` to restore the v0.5.0 (no-guard) behaviour.

## Governance

Point git-paw at your project's existing governance documents so the supervisor can read them as context. All fields are optional — list only the docs you have.

```toml
[governance]
adr = "docs/adr"                          # directory of ADR files
test_strategy = "docs/test-strategy.md"   # single Markdown file
security = "docs/security-checklist.md"   # single Markdown file
dod = "docs/definition-of-done.md"        # single Markdown file
constitution = ".specify/memory/constitution.md"  # single Markdown file
readme = "README.md"                      # repository README
docs = "docs/src"                         # documentation root directory
```

| Field | Kind | Description |
|-------|------|-------------|
| `adr` | directory | Architecture Decision Records. git-paw does not care which convention (Nygard, MADR, `adr-tools`) — point at the folder where they live. |
| `test_strategy` | file | The team's test-strategy document. |
| `security` | file | Security checklist (OWASP-style, project-specific, whatever the team uses). |
| `dod` | file | Definition of Done for completed work. |
| `constitution` | file | Project constitution. Spec Kit users normally let this auto-wire (see below). |
| `readme` | file | Repository README. Surfaced by the MCP `get_readme` tool ([MCP server](../user-guide/mcp.md)); unset → the tool returns `null`. |
| `docs` | directory | Documentation root. Surfaced by the MCP `list_docs`/`get_doc` tools, which enumerate and read `*.md` files confined to this directory; unset → those tools return empty/`null`. |

git-paw does not dictate the structure, format, or rubric of any of these documents. The supervisor LLM reads them as context and applies judgment during its existing audit flow. There is no `[governance.gates]` table and no per-doc enforcement switch — gating-per-doc would require git-paw to define "failure" for each doc type, and that is a process choice your team owns.

Paths are stored verbatim and resolved against the repository root at use time. Relative paths point at files inside the repo; absolute paths are accepted as-is. A path that does not exist still loads cleanly — git-paw does not stat the filesystem at config-load. If you point at a missing file, the runtime consumer flags it.

### Spec Kit constitution auto-wiring

When `governance.constitution` is unset AND `[specs] type = "speckit"`, git-paw probes for `<specs_dir>/../memory/constitution.md` and, if present, populates `governance.constitution` automatically. This means a typical Spec Kit project (with `.specify/specs/` and `.specify/memory/constitution.md`) gets the constitution wired up without any `[governance]` entry.

Explicit values always win. If `governance.constitution` is set to anything — including a path that does not exist or an empty string — auto-wiring is skipped:

```toml
[governance]
constitution = ""   # disables auto-wiring without deleting the slot
```

Auto-wiring only runs for the Spec Kit backend. With `[specs] type = "openspec"`, `type = "markdown"`, or no `[specs]` section, `governance.constitution` stays whatever the TOML says (defaulting to `None`).

### What the supervisor does with these paths

This `[governance]` table is the storage slot. The runtime consumer is the supervisor's boot prompt, which lists each configured doc so the supervisor can read it and apply it to its audit. See the [Governance](../user-guide/governance.md) chapter of the user guide for what that looks like end-to-end.

## MCP

```toml
[mcp]
name = "my-project"   # optional; default: "git-paw"
```

| Field | Default | Purpose |
|-------|---------|---------|
| `name` | `"git-paw"` | The identity the `git paw mcp` server advertises as `serverInfo.name` in the MCP `initialize` handshake. Set a custom value to distinguish multiple repositories that each run an MCP server. Distinct from the client-side `mcpServers` key (the display label in clients like Claude Desktop), which you rename independently. See the [MCP server](../user-guide/mcp.md) chapter. |

The section is optional; omitting it (or `name`) makes the server advertise `git-paw`.

## Dashboard

Configure the dashboard TUI rendered in its own pane when the broker is enabled (pane 0 in every mode; in supervisor mode it is the full-width top row).

```toml
[dashboard]
show_message_log = true

[dashboard.broker_log]
max_messages = 500
default_visible = true
height_lines = 20
```

| Field | Default | Description |
|-------|---------|-------------|
| `show_message_log` | `false` | When `true`, the dashboard renders the legacy scrolling broker-message panel. Superseded by the type-filterable **Broker log** panel below; leave `false` for a more compact layout. |

### Broker log panel

The `[dashboard.broker_log]` table configures the v0.6.0 **Broker log**
panel — a scrolling, type-filterable view of recent broker messages that
fills the screen region freed when v0.5.0 removed the prompt inbox.

| Field | Default | Description |
|-------|---------|-------------|
| `max_messages` | `500` | Maximum number of messages retained in the panel's in-memory ring buffer. When the buffer is full, the oldest message drops off as new ones arrive. The log is in-memory only and is not persisted across dashboard restarts. |
| `default_visible` | `true` | Whether the panel is shown when the dashboard first launches. The `l` hotkey toggles visibility at runtime regardless of this value. |
| `height_lines` | `20` | Number of terminal rows the panel occupies when visible — raised from the v0.6.0 fixed `12` so more messages show without scrolling. The agent-status table keeps a positive minimum height and absorbs slack, so the panel stays at this height on tall terminals and yields space before the table shrinks on short ones. |

An absent `[dashboard.broker_log]` table — as in any v0.5.0 config —
loads these defaults, so existing config files parse unchanged.

See [Dashboard](../user-guide/dashboard.md) for the panel's hotkeys,
filter chips, and details overlay.

### Multi-repo configuration

Each repository can have its own dashboard settings in `.git-paw/config.toml`. The repo-level config overrides the global config.

## Layout

Configure the visual styling git-paw applies to the tmux sessions it creates.

```toml
[layout]
border_affordances = true  # default
```

| Field | Default | Description |
|-------|---------|-------------|
| `border_affordances` | `true` | When `true`, git-paw applies *pane affordances* to the sessions it creates: double-line pane borders (`═║` instead of the default `─│`), a per-pane label bar showing each pane's index and role/branch (e.g. `0: supervisor`, `1: dashboard`, `2: feat/foo`), a dim border on inactive panes, and a cyan-bold border on the focused pane. Set to `false` to opt out and inherit your own tmux styling instead. |

These options are scoped to git-paw-managed sessions (`paw-*`) only — your other tmux sessions are never touched. They apply to both `git paw start` and supervisor-mode sessions.

**This setting is purely visual.** Independently of it, git-paw tags every pane it creates with a pane-scoped `@paw_role` tmux option naming that pane's role (`supervisor`, `dashboard`, or the agent's branch). The option draws nothing on its own — only the label bar above renders it — but `git paw status` and `git paw doctor` use it to tell a coding-agent pane from an infrastructure pane when they check whether a session's agents are still running. Turning affordances off therefore changes how a session looks, never how accurately its state is reported.

**When to disable.** Turn `border_affordances` off if you run a tmux theme you prefer, are on tmux older than 3.2 (where the double border lines aren't recognised — git-paw warns and continues, but you may prefer the consistent default look), or find the label strip noisy on small terminals.

See [Supervisor](../user-guide/supervisor.md) for how the labelled layout looks in a supervisor session.

## Merging Rules

When both global and repo configs exist, they merge with these rules:

| Field | Merge behavior |
|-------|---------------|
| `default_cli` | Repo wins |
| `default_spec_cli` | Repo wins |
| `branch_prefix` | Repo wins |
| `mouse` | Repo wins |
| `worktree_placement` | Repo wins |
| `docs_base_url` | Repo wins |
| `clis` | Maps merge (repo overrides per-key) |
| `presets` | Maps merge (repo overrides per-key) |
| `specs` | Repo wins |
| `logging` | Repo wins |
| `broker` | Repo wins |
| `supervisor` | Repo wins |
| `dashboard` | Repo wins |
| `governance` | Per-field merge (repo wins on each set field, unset fields fall back to global) |
| `layout` | Repo wins |
| `opsx` | Repo wins |
| `mcp` | Per-field merge (repo wins on each set field, unset fields fall back to global) |
| `worktree` | Repo wins (the whole table, including its `env`, `ports`, and `hooks` sub-tables) |

**Example:** If global config defines `[clis.my-agent]` and repo config defines `[clis.my-agent]` with a different command, the repo version wins. But a `[clis.other-tool]` in global config still appears — maps are merged, not replaced.

## Graceful Absence

If no config files exist, git-paw uses defaults:
- No default CLI (prompts for selection)
- Mouse mode enabled
- No custom CLIs
- No presets

## Checking your configuration

`git paw doctor` reports what git-paw actually resolved from the files above —
whether `.git-paw/config.toml` parses, the effective `worktree_placement`, the
resolved spec system and its spec count, the `[broker]` bind/port state, and
whether each `[supervisor]` gate command's binary is on `PATH`. It also flags
keys this version does not recognise, which is the quickest way to catch a
typo'd field that would otherwise be silently ignored.

Doctor adds **no configuration fields of its own** — it only reads the settings
documented on this page. See the [Doctor chapter](../user-guide/doctor.md).

```bash
git paw doctor          # grouped report
git paw doctor --json   # same checks, machine-readable
```

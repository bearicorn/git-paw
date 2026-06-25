# Configuration

git-paw uses TOML configuration files at two levels, with repo-level settings overriding global ones.

## Config File Locations

| Level | Path | Purpose |
|-------|------|---------|
| Global | `~/.config/git-paw/config.toml` | Default CLI, custom CLIs, global presets |
| Per-repo | `.git-paw/config.toml` (in repo root) | Repo-specific overrides |

Both files are optional. git-paw works with sensible defaults when no config exists.

## Full Config Example

```toml
# Default CLI used when --cli flag is not provided
default_cli = "my-cli"

# Default CLI for spec-mode launches (--from-all-specs, --specs); bypasses picker when set
# default_spec_cli = "my-cli"

# Prefix for spec-derived branch names (default: "spec/")
# branch_prefix = "spec/"

# Enable mouse mode in tmux sessions (default: true)
mouse = true

# Where agent worktrees are created: "child" (in-repo .git-paw/worktrees/)
# or "sibling" (../<project>-<branch>). Absent = "sibling"; git paw init
# writes "child" for new repos.
worktree_placement = "child"

# Pane affordances: heavy borders, per-pane labels, active-pane highlight
# (default: true; set false to inherit your own tmux styling)
# [layout]
# border_affordances = true

# Custom CLI definitions
[clis.my-agent]
command = "/usr/local/bin/my-agent"
display_name = "My Agent"

[clis.local-llm]
command = "ollama-code"
display_name = "Local LLM"

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
# type = "openspec"    # "openspec", "markdown", or "speckit"

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

## Settings Reference

### `default_cli`

The AI CLI to use when `--cli` is not passed and you want to skip the CLI picker.

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

## Custom CLIs

Register custom AI CLIs that aren't in git-paw's built-in detection list.

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
# Path to this CLI's claude-format settings file (the one carrying
# `allowed_bash_prefixes`). When set and the broker is enabled, git-paw
# seeds the broker-curl allowlist into this path too, so the CLI's
# boot-time broker `curl` does not raise a permission prompt. A leading
# `~` is expanded to the home directory. Use for claude-family variants
# that read a non-default config dir.
settings_path = "~/.config/claude-variant/settings.json"
```

| Field | Required | Purpose |
|-------|----------|---------|
| `command` | yes | Command or path to the CLI binary. |
| `display_name` | no | Human-readable name shown in prompts. |
| `submit_delay_ms` | no | Boot-prompt settle delay (ms) before the submit `Enter`; per-CLI so the launcher stays CLI-agnostic. |
| `settings_path` | no | Path to the CLI's claude-format settings file; broker-curl allowlist is seeded here so the boot-time broker `curl` doesn't prompt. |

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

The `add-cli` and `remove-cli` commands modify the **global** config at `~/.config/git-paw/config.toml`.

### Listing CLIs

```bash
git paw list-clis
```

Shows both auto-detected and custom CLIs with their source:

```
Name       Path                         Source
claude     /usr/local/bin/claude        detected
codex      /usr/local/bin/codex         detected
my-agent   /usr/local/bin/my-agent      custom
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
cli = "gemini"
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
type = "openspec"     # "openspec" (OpenSpec changes), "markdown" (flat .md files), or "speckit" (GitHub Spec Kit)
```

| Field | Default | Description |
|-------|---------|-------------|
| `dir` | `"specs"` | Directory to scan for spec files |
| `type` | `"openspec"` | Spec backend: `"openspec"` (directory-based OpenSpec changes), `"markdown"` (flat `.md` files with YAML frontmatter), or `"speckit"` ([GitHub Spec Kit](https://github.com/github/spec-kit) `.specify/specs/<feature>/`) |

When `[specs]` is omitted and `.specify/specs/` exists at the repo root, the
spec backend auto-detects to `type = "speckit"` with
`dir = ".specify/specs"`. Use the `--specs-format` CLI flag to override both
the config value and the auto-detection for a single launch.

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
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Whether to start the coordination broker |
| `port` | `9119` | HTTP port for the broker server |
| `bind` | `"127.0.0.1"` | Bind address -- never bind to `0.0.0.0` on shared machines |

When the broker is enabled, git-paw injects the `GIT_PAW_BROKER_URL` environment variable into each agent pane, pointing to `http://<bind>:<port>`. Agents use this URL to communicate with the broker.

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
verify_on_commit_nudge = true
strict_branch_guard = true
manual_approvals_log = true
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Whether to use supervisor mode by default (can also use `--supervisor` flag, or override with `--no-supervisor` for a single session) |
| `cli` | (uses `default_cli`) | CLI binary for the supervisor agent |
| `test_command` | (none) | Test runner — gate 1 (e.g. `"just check"`, `"cargo test"`, `"npm test"`, `"pytest"`) |
| `lint_command` | (none) | Lint check — gate 1 (e.g. `"cargo clippy -- -D warnings"`, `"npm run lint"`, `"ruff check ."`, `"golangci-lint run"`) |
| `build_command` | (none) | Compile step — gate 1 when build is distinct from test (e.g. `"cargo build"`, `"npm run build"`, `"mvn package"`, `"go build ./..."`) |
| `fmt_check_command` | (none) | Formatter check — gate 1 (e.g. `"cargo fmt --check"`, `"prettier --check ."`, `"gofmt -l ."`, `"black --check ."`) |
| `doc_build_command` | (none) | Documentation build — gate 4 (e.g. `"mdbook build docs/"`, `"sphinx-build"`, `"mkdocs build"`) |
| `doc_tool_command` | (none) | API-doc generator — distinct from `doc_build_command`; gates the per-language extractor for changed public items (e.g. `"cargo doc --no-deps"`, `"sphinx-build -W docs docs/_build"`, `"javadoc"`, `"npx typedoc"`, `"go doc"`). Renders empty (not `(not configured)`) when unset so the surrounding prose reads naturally |
| `spec_validate_command` | (none) | Spec validator — gate 3 (e.g. `"openspec validate {{CHANGE_ID}} --strict"` for OpenSpec). `{{CHANGE_ID}}` is substituted by the supervisor agent at verification time with the change name being audited; it is **not** expanded at config load |
| `security_audit_command` | (none) | Security audit tooling — gate 5 (e.g. `"cargo audit"`, `"npm audit"`, `"bandit -r ."`, `"gosec ./..."`) |
| `agent_approval` | `"auto"` | Permission level for coding agents: `"manual"`, `"auto"`, or `"full-auto"` |
| `verify_on_commit_nudge` | `true` | When on, the broker posts a `supervisor.verify-now` message to the supervisor inbox on every `agent.artifact { status: "committed" }`, so the supervisor verifies each commit promptly on an explicit event instead of batching. Set `false` to fall back to sweep-cadence verification |
| `strict_branch_guard` | `true` | When `true`, a per-worktree **pre-commit** hook refuses any commit whose checked-out branch differs from the branch the worktree was created for, blocking cross-worktree contamination (linked worktrees share `.git/refs`, so a stray `cd` can otherwise advance the wrong branch). Set `false` to disable *enforcement* — the **post-commit** hook still publishes an `agent.feedback` + `agent.learning` record when it detects a mismatch (detection without enforcement) |
| `manual_approvals_log` | `true` | When `true`, commands the supervisor forwards for a manual decision (prompts the auto-approve preset did not match) are appended to `.git-paw/sessions/<session>.manual-approvals.jsonl` and surfaced via [`git paw approvals`](../cli-reference.md). On a pattern's first sighting a `permission_pattern` learning is also emitted (when `learnings = true`). Set `false` to suppress both the log writes and the learnings emission; the opt-out affects writes only, so `git paw approvals` still reads any pre-existing log |

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
| `auto` | CLI default behavior — some prompts, some auto-approved |
| `full-auto` | Skip all permission prompts (fastest, agents run unattended) |

The supervisor translates the approval level into CLI-specific flags at launch (e.g. `--dangerously-skip-permissions` for Claude in full-auto mode).

### Auto-approve safe permission prompts

When supervisor mode is enabled, git-paw can automatically approve common,
known-safe permission prompts (`cargo test`, `git commit`, broker `curl` calls, etc.)
in stalled agent panes so the supervisor does not have to dismiss every prompt by hand.

```toml
[supervisor.auto_approve]
enabled = true
safe_commands = ["just lint", "just test"]
stall_threshold_seconds = 30
approval_level = "safe"
approve_worktree_writes = true
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `true` | Master switch for auto-approval. Set to `false` to disable. |
| `safe_commands` | `[]` | Project-specific command prefixes appended to the built-in defaults. |
| `stall_threshold_seconds` | `30` | Seconds an agent's `last_seen` must lag before its pane is polled (minimum `5`). |
| `approval_level` | `"safe"` | Coarse preset: `"off"`, `"conservative"`, or `"safe"`. |
| `approve_worktree_writes` | `true` | Auto-approve file write/edit/create prompts whose target resolves **inside the agent's own worktree**. Set `false` to require manual approval for all file operations. |

**Worktree-confined file edits.** Beyond the shell-command whitelist, auto-approval
also covers an agent's filesystem write/edit/create prompts when the target path
resolves inside that agent's own worktree root. The path from the prompt
(e.g. `"Do you want to allow this write to Containerfile?"`) is canonicalized and
checked with `starts_with(worktree_root)`, so a `..`/symlink path that escapes the
worktree fails the check and still requires manual approval. Worktrees are isolated,
so confining auto-approval to the worktree boundary is safe by construction; set
`approve_worktree_writes = false` to opt out. Paths outside the worktree (the parent
repo, your home directory, system paths) always require manual approval.

**Built-in safe commands:** `cargo fmt`, `cargo clippy`, `cargo test`, `cargo build`,
`git commit`, `git push`, `curl http://127.0.0.1:`.

**Approval-level presets:**

| Preset | Behavior |
|--------|----------|
| `off` | Forces `enabled = false`. No detection or approval runs. |
| `conservative` | Drops `git push` and `curl` from the effective whitelist. |
| `safe` (default) | Approve every entry in the built-in whitelist plus configured extras. |

**How it works:** when an agent's status is non-terminal (`done`, `verified`, `blocked`,
`committed` are skipped) and its `last_seen` exceeds the threshold, git-paw runs
`tmux capture-pane`, classifies the pending command, and either dispatches
`BTab Down Enter` (if safe) or publishes an `agent.question` to the supervisor inbox
(if not).

git-paw also seeds `.claude/settings.json::allowed_bash_prefixes` with the broker
endpoints (`/publish`, `/status`, `/poll`, `/feedback`) so the first broker call
never hits a permission prompt. Existing entries in that file are preserved.

### Common dev-command allowlist

On every supervisor session start, git-paw seeds a curated preset of dev-loop
prefix patterns into `.claude/settings.json::allowed_bash_prefixes` so agents
do not hit a permission prompt for each variant of `git commit`, `git diff`,
`grep`, etc. The mechanism is the same one Claude uses for its "Yes, don't ask
again" flow — but seeded up-front rather than approved one-by-one.

Each seeded value is a command **prefix** (a verb or verb + subcommand) that
subsumes every argument variant — `git diff` covers `git diff --stat HEAD~1`,
so a routine dev-loop command prompts at most once.

The preset is split into two tiers:

- a **universal** set that is always seeded — stack-neutral commands safe in any
  repository regardless of language or toolchain;
- **opt-in stack presets** (`rust` / `node` / `python` / `go`) plus a free-form
  `extra` list for everything tied to a particular toolchain. A bare project
  inherits only the universal set and never a toolchain it does not use.

```toml
[supervisor.common_dev_allowlist]
enabled = true
stacks = ["rust"]
extra = ["just", "mdbook build", "openspec validate"]
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `true` | Master switch for the seeder. Set to `false` to skip seeding entirely. |
| `stacks` | `[]` | Named, curated stack presets to opt into: `rust`, `node`, `python`, `go`. The seeder seeds the union of the universal preset, each selected stack, and `extra`. Unknown names contribute nothing. git-paw does **not** auto-detect your stack — selection is always explicit. |
| `extra` | `[]` | Additional project-specific prefix patterns appended to the universal preset and any selected stacks. |

**Universal preset (always seeded):**

- **Git (read)**: `git status`, `git log`, `git diff`, `git show`, `git fetch`
- **Git (write, non-destructive)**: `git commit`, `git push`, `git pull`,
  `git merge`, `git stash`, `git add`, `git restore`, `git rm`
- **Search (read-only)**: `find`, `grep`, `sed -n`

**Named stack presets (opt-in via `stacks`):**

- **`rust`**: `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt`,
  `cargo check`, `cargo tree`, `cargo deny`, `cargo update`
- **`node`**: `npm install`, `npm ci`, `npm test`, `npm run`, `pnpm install`,
  `pnpm test`, `pnpm run`, `yarn install`, `yarn test`
- **`python`**: `pytest`, `pip install`, `ruff`, `black`, `mypy`, `flake8`,
  `uv pip`, `uv sync`
- **`go`**: `go build`, `go test`, `go vet`, `go fmt`, `gofmt`, `go mod`,
  `golangci-lint`

Tools that don't belong to a named stack — `just`, `mdbook build`,
`openspec …`, etc. — go in `extra` (git-paw's own repo opts into `rust` and
lists those in `extra`).

**Intentional exclusions:** the universal set and every curated stack preset
omit destructive verbs — `cargo install`, `cargo run`, `cargo bench`, `go run`,
package-manager `publish`/`uninstall`, `git rebase`, `git reset`,
`git checkout`, `git push --force`, and `sed` without `-n`. Add any of these via
`extra` if you accept the wider surface for your project (`extra` entries are
never validated).

**Behaviour:**

- Independent of broker status — non-broker supervisor sessions still benefit.
- Idempotent: re-seeding on session re-attach never duplicates entries.
- Non-fatal: write failures log a warning to stderr and session start continues.
- Targets `<repo>/.claude/settings.json` always; also writes each configured
  `[clis.<name>].settings_path` whose parent directory already exists (the
  CLI-agnostic alt-config path — register a claude-family variant's settings
  file there to have it seeded too) but never creates a missing directory.
- Entries persist after `git paw stop` — prune `.claude/settings.json` manually
  if you want a clean slate.

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

### Routing through the supervisor (`/tell`)

The `[supervisor.tell]` table tunes the `/agents` and `/tell` commands you
type in the supervisor pane to route prompts to individual agents (see
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
appended to `.git-paw/session-learnings.md` covering the five deterministic
categories tracked in v0.5.0 (stuck duration, recovery-cycle count, forward
conflicts, in-flight conflicts, ownership violations). The
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
(`recurring_failure_shape`, `doc_gap`, `adr_drift`, `scope_mistake`) added
in v0.6.0. Their detection thresholds live in the supervisor skill prose,
not in `[supervisor.learnings_config]` — to tune how readily the
supervisor publishes a category, edit your local copy of the supervisor
skill rather than a config value.

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

This `[governance]` table is the storage slot. The runtime consumer — boot-prompt injection so the supervisor can read each doc and apply it to its audit — lives in the parallel `governance-context` capability. See the [Governance](../user-guide/governance.md) chapter of the user guide for what that looks like end-to-end.

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

Configure the dashboard TUI rendered in pane 0 when the broker is enabled.

```toml
[dashboard]
show_message_log = true

[dashboard.broker_log]
max_messages = 500
default_visible = true
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
| `border_affordances` | `true` | When `true`, git-paw applies *pane affordances* to the sessions it creates: heavy pane borders (`━┃` instead of the default `─│`), a per-pane label strip showing each pane's index and role/branch (e.g. `0: supervisor`, `1: dashboard`, `2: feat/foo`), a dim border on inactive panes, and a cyan-bold border on the focused pane. Set to `false` to opt out and inherit your own tmux styling instead. |

These options are scoped to git-paw-managed sessions (`paw-*`) only — your other tmux sessions are never touched. They apply to both `git paw start` and supervisor-mode sessions.

**When to disable.** Turn `border_affordances` off if you run a tmux theme you prefer, are on tmux older than 3.2 (where the heavy border lines aren't recognised — git-paw warns and continues, but you may prefer the consistent default look), or find the label strip noisy on small terminals.

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
| `clis` | Maps merge (repo overrides per-key) |
| `presets` | Maps merge (repo overrides per-key) |
| `specs` | Repo wins |
| `logging` | Repo wins |
| `broker` | Repo wins |
| `supervisor` | Repo wins |
| `dashboard` | Repo wins |
| `governance` | Per-field merge (repo wins on each set field, unset fields fall back to global) |
| `layout` | Repo wins |

**Example:** If global config defines `[clis.my-agent]` and repo config defines `[clis.my-agent]` with a different command, the repo version wins. But a `[clis.other-tool]` in global config still appears — maps are merged, not replaced.

## Graceful Absence

If no config files exist, git-paw uses defaults:
- No default CLI (prompts for selection)
- Mouse mode enabled
- No custom CLIs
- No presets

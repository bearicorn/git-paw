## Context

Every worker pane command is built by `CliProfile::launch_command` (`src/agent_cli/profile.rs`).
It concatenates the executable, `[clis.<name>].args`, native sandbox arguments, approval flags, then
session-id or resume arguments. All worker launch paths call it: `commands/start.rs` (bare,
spec-driven, supervised, readiness relaunch), `commands/launch.rs` (`add` / attach, dry-run line)
and `commands/recover.rs`. Per-family facts live behind the `CliProvider` trait
(`src/agent_cli/mod.rs`) with Null-Object defaults. The drive loop's headless orchestrator one-shot
does **not** use this builder: `src/supervisor/escalation.rs` assembles its own argv from
`headless_args`. The pane command reaches the pane as a shell line typed into it, so a leading
`NAME=value` assignment is scoped to that one process.

Claude Code facts, from the official docs (code.claude.com, read 2026-11-11):
- `--setting-sources` takes `user`, `project` and `local`, and works in interactive mode. Managed
  settings always apply.
- Auto memory is keyed to the git repository, so every worktree shares the main checkout's memory.
  `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` turns it off. Setting it to `0` forces it on, so git-paw
  writes `1`.
- A fresh `CLAUDE_CONFIG_DIR` reads a different credentials file and Keychain entry, so it forces a
  new login.
- `--safe-mode` keeps authentication but also turns off CLAUDE.md, skills, custom commands and MCP.
- `--bare` does not read OAuth or Keychain credentials at all.
- **Not stated by the docs:** that excluding `user` drops user-enabled plugins and user-settings
  hooks. Plugin enable state lives in `enabledPlugins` in settings, and synced plugins are
  documented as not loading without `user`, so it is inferred. Verified at implementation (D5).

## Goals / Non-Goals

**Goals:**
- One provider facet decides a family's isolation, and the one builder places it.
- Default on for every worker, with a per-CLI escape hatch.
- The consumer's project and local settings keep working in workers.

**Non-Goals:**
- Isolating the operator's driving session or the headless orchestrator one-shot.
- Per-worker or per-profile config directories (re-login cost; see D1).
- Families other than `claude` until their facts are verified (v0.23.0 audit).
- Reading or writing any agent-CLI settings file. git-paw still touches none.

## Decisions

### D1 — Exclude the user settings source, not relocate the config directory

`--setting-sources project,local` drops exactly the operator-personal layer and keeps everything
the repository declares. Alternatives:
- A per-worker `CLAUDE_CONFIG_DIR`: rejected. It forces a login per directory, and the only
  documented way to share credentials is a long-lived token, which changes the operator's auth model.
- `--safe-mode`: rejected. It also drops CLAUDE.md, skills, commands and MCP, which a worker needs
  for `/opsx:apply`, the injected instructions and the MCP boot form.
- `--bare`: rejected. It cannot use subscription credentials.
- `--settings` with `disableAllHooks` and per-plugin `enabledPlugins: false`: rejected. git-paw
  would have to read the operator's settings to list their plugins, and a second `--settings` next
  to a user-supplied one has undocumented merge behaviour.

### D2 — Memory off through the environment, not a settings flag

`CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` is per process, needs no JSON quoting in a shell line, and
does not compete with a user's own `--settings` in `args` or `approval_args`. Relocating memory
with `autoMemoryDirectory` was rejected: a worker's memory would outlive its worktree and still need
a path git-paw chooses inside operator territory.

### D3 — A provider facet: `operator_isolation() -> OperatorIsolation { env, args }`

This follows the existing Strategy pattern. The default returns empty and the `claude` provider
overrides it. The builder emits `env` as `NAME=value ` before the executable, and `args` after the
CLI's `args`, ahead of sandbox and approval flags. Putting it before the approval flags keeps the
approval and session parts last, where the readiness and recovery code already expects them. Values
are compile-time constants, so there is no quoting or injection surface. The ordering requirement
is ADDED next to *Native sandbox arguments precede the approval flags once*, the same way the
sandbox change slotted in without rewriting the builder requirement.

### D4 — Opt-out key `inherit_operator_config` on `[clis.<name>]`

The CLI entry is the opt-out point because the reason to opt out is a property of the CLI setup.
Example: a variant whose user settings carry `env` for a proxy or model provider, or a
`CLAUDE_CONFIG_DIR` variant whose settings live in its own `settings.json`. A built-in `claude`
user opts out with `[clis.claude] command = "claude"` plus the key. A global switch was considered
and left out (Simplicity First). v0.21.0's `[workers]` table can gain one if needed.
`#[serde(default, skip_serializing_if = "std::ops::Not::not")]` keeps old configs byte-stable.

### D5 — Fact gate before the `claude` row ships

Like `cli-setup-consent`, the row ships only after a recorded check against a real Claude Code
(version noted in the commit):
- a user-settings hook and a user-enabled plugin do not fire or load in a worker launched with the
  arguments;
- a project-local allow rule still applies;
- `/opsx:apply` (a project skill and command) and the git-paw MCP boot form still work;
- no memory file is written.

If plugins or hooks still load, the row keeps only what is verified. The change then records the
gap in the docs rather than shipping a claim.

## Risks / Trade-offs

- [The operator's user-level allow rules and model default no longer reach workers, so workers may
  prompt more] → documented. Repository allow rules (`setup-cli`, project-local settings) still
  apply. The model belongs in `[clis.<name>].args`.
- [Setups that need user-settings `env` (a proxy, a provider) fail to authenticate or route] →
  `inherit_operator_config = true`. The docs name this case.
- [A shell that does not accept a leading assignment (fish before 3.1)] → acceptable on supported
  platforms. The dry-run line shows the exact command.
- [Claude Code changes the flag or the environment variable] → the v0.23.0 per-CLI conformance
  suite owns re-verification. The facet keeps the change to one place.
- [Leak not closed here] → "Yes, and don't ask again" in a worktree writes the rule to the main
  checkout's `.claude/settings.local.json` (Claude Code worktrees doc). That is project-local, so
  this change does not touch it. It is noted for the v0.23.0 audit.

## Migration Plan

No config migration. After the upgrade, Claude-family workers launch isolated. An operator who
needs the old behaviour adds `inherit_operator_config = true` to the CLI's entry. Rollback is a
revert. Session files are untouched, so recovery after a downgrade relaunches without the
arguments.

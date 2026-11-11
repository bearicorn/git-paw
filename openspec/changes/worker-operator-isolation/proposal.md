## Why

A worker git-paw launches runs with the operator's own agent-CLI user configuration. In the v0.19.0
dogfood, the operator's personal Claude Code plugins and hooks ran inside every worker. They injected
hook chatter (DF-14). A turn-end review plugin spawned audit subagents at uncontrolled cost (DF-27)
and overrode an orchestrator's "do not change the branch" with its own demand (DF-40). A worker also
reached for the operator's auto-memory directory (DF-39). Claude Code keys auto memory to the git
repository, so every worktree shares the main checkout's memory, and a worker's memory write lands
in the operator's own sessions. git-paw's skill already tells workers to keep writes in their
worktree (`core-memory-isolation`). Nothing stops the CLI itself from loading the operator's
extensions or writing that memory.

## What Changes

- **Workers launch without the operator's personal extensions by default.** For each CLI family
  with verified facts, the launch builder adds that family's operator-isolation arguments and
  environment. Every worker launch path gets them: bare, spec-driven and supervised `start`, `add`,
  recovery and the readiness relaunch.
- **`claude` family:** `--setting-sources project,local` stops the operator's user-level settings
  from loading, which removes their user hooks and user-enabled plugins.
  `CLAUDE_CODE_PLUGIN_DIRS=` (empty) stops plugins the operator's environment points at, which
  the flag does not reach (verified, design D5).
  `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` turns auto memory off, so a worker can never write the
  repository's shared memory. Project and local settings still load, so the consumer's
  project configuration and the main checkout's local allow rules still apply. Authentication is
  unaffected, because the worker keeps the operator's config directory.
- **Other families get nothing until their facts are verified** (the `generic` Null Object). The
  v0.23.0 per-CLI audit is where `codex`, `agy` and the rest gain theirs.
- **Opt-out per CLI:** `[clis.<name>] inherit_operator_config = true` launches that CLI's workers
  with the operator's full configuration, as before. This is for setups that rely on user settings
  to work at all, such as a proxy endpoint or model set in the user settings' `env` block.
- **Behaviour change (not breaking for configs):** a Claude-family worker no longer sees the
  operator's user-level permission rules, model default, hooks, plugins or auto memory. No config
  file needs editing. The dry-run launch line and the docs show the added arguments.
- Not in scope: the operator's own driving session, the drive loop's headless orchestrator
  one-shot (it is the orchestrator, not a worker), and delegate runs. `agent-profiles` applies this
  facet to delegates when it generalises the headless runner. Per-profile config directories are out:
  a fresh `CLAUDE_CONFIG_DIR` forces a new login.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `cli-provider`: ADDED — operator isolation is a provider facet with a `generic` fallback; the
  launch builder places its environment before the executable and its arguments after the CLI's
  `args`; `[clis.<name>] inherit_operator_config` opts a CLI out.
- `core-memory-isolation`: ADDED — workers are launched with the operator's personal extensions
  and the repository-shared auto memory off, wherever the CLI's family encodes how; the operator's
  session and the headless orchestrator one-shot are left as they were.

## Impact

- **Code:** `src/agent_cli/mod.rs` (a new `CliProvider` method with the Null Object default and the
  `claude` override), `src/agent_cli/profile.rs` (`CliProfile::launch_command` emits the environment
  prefix and the isolation arguments unless the entry opts out), `src/config/cli.rs`
  (`CustomCli.inherit_operator_config`, `#[serde(default, skip_serializing_if)]`). No launch path
  changes, because they all build through `CliProfile::launch_command`.
- **Struct-literal ripple (GP-40):** `CustomCli {` literal sites in `src/` and `tests/` gain the
  field. List them up front (`grep -rn 'CustomCli {' src tests`).
- **Tests:** existing tests that assert an exact `claude` launch line change on purpose. This is the
  behaviour change, not a regression. New: per-family builder tests, the opt-out, and a
  `start`/`add`/recovery parity test.
- **Docs:** `docs/src/configuration/README.md` (`inherit_operator_config`), the security posture
  chapter and the CLI provider / custom-CLI chapter (what a worker does and does not inherit),
  `CHANGELOG` via the commit.
- **Fact gate:** the `claude` facts are verified against a real Claude Code before the row ships.
  The docs state `--setting-sources` values and the auto-memory switch. They do not say outright
  that excluding `user` drops user-enabled plugins and user hooks. If verification fails, the
  `claude` row falls back to the arguments that are verified, and the change says so.
- **Security / safety:** reduces a worker's reach into operator territory. It adds no grant, and it
  never widens a permission. Review with `security-and-safety-review`.
- **Dependencies:** none. **Order in v0.20.0:** independent of the skill-package changes. It lands
  before `agent-profiles`, which reuses the facet for delegates.

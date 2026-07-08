## Why

The bundled helper scripts (`broker.sh`, `sweep.sh`, and now `docs-fetch.sh`) live under `<repo>/.git-paw/scripts/`, written by `git paw init` and **gitignored**. An agent worktree is a fresh checkout of a feature branch, so `.git-paw/scripts/` does not exist inside it — the agent, which runs *in the worktree* and invokes `.git-paw/scripts/broker.sh` relative to its own cwd, finds nothing. Every dogfood since v0.8.0 has shown agents papering over this by hand: `cp assets/scripts/broker.sh .git-paw/scripts/ …` at boot, which costs an approval prompt and only works because `assets/` happens to be in the repo. This is recurring, avoidable boot friction.

## What Changes

- `git paw start` and `git paw add` SHALL **provision the bundled helper scripts into each agent worktree's `.git-paw/scripts/`** at worktree setup — so `broker.sh` / `sweep.sh` / `docs-fetch.sh` are present and executable before the agent boots, with no manual `cp` and no approval prompt.
- Provisioning SHALL be **idempotent** (re-running `start`/`add`, or attaching to an existing worktree, refreshes the scripts rather than erroring) and SHALL source the scripts from the same bundled assets `git paw init` uses, so a worktree's helpers match the installed version.

Non-goals: this does not change what the helpers do, and does not address per-CLI allowlist seeding for alternate config dirs (`CLAUDE_CONFIG_DIR`) — that remains the v1.0.0 per-CLI hook-provider work (noted below).

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `agent-broker-helper`: ADD a requirement that the bundled helper scripts are provisioned into every agent worktree on `start`/`add` (idempotent, executable, version-matched), so agents never have to hand-copy them.

## Impact

- `src/main.rs` (`attach_agent` / worktree setup) and/or `src/agents.rs`: after `create_worktree`, write the bundled helper scripts into the worktree's `.git-paw/scripts/` (mkdir -p, chmod +x), from the same embedded assets `init` uses.
- Removes the boot-time `cp` friction seen every dogfood since v0.8.0.
- **Related (deferred):** the alternate-config-dir allowlist gap (`CLAUDE_CONFIG_DIR=~/.claude-oss` agents still prompt on the first helper call) is per-CLI allowlist seeding — tracked under the v1.0.0 Per-CLI Broker-Curl Allowlist Seeding work, not this change.

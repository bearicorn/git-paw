## Context

`OperatorIsolation { env, args }` (`src/agent_cli/mod.rs`) is one value. `CliProfile::operator_isolation`
(`src/agent_cli/profile.rs`) returns it whole or, on `inherit_operator_config = true`, the empty
value. Delegates get the same through `commands/delegate.rs`. See proposal.md for why it splits.

## Decisions

### D1 — Split by purpose, not by mechanism

Auto memory and prompt suggestions are about git-paw working correctly: one shared memory across
worktrees, and the Enter-submits-a-prediction hazard. They don't reflect the operator's taste, so
no opt-out should turn them back on. `--setting-sources` and `CLAUDE_CODE_PLUGIN_DIRS=` decide which
of the operator's extensions load, which is exactly what a worker config folder wants to control
itself. The wrapper script sets or unsets `CLAUDE_CODE_PLUGIN_DIRS` itself, because it runs after
the `env` prefix and wins.

Shape: either `OperatorIsolation { safety_env, config_env, config_args }` or two facet methods. Pick
whichever keeps the emitted order `DISABLE_AUTO_MEMORY, PLUGIN_DIRS, PROMPT_SUGGESTION` for the
default launch. Default lines must stay byte-identical.

### D2 — Recommend a worker config folder in docs, don't build one in

A separate config folder (`CLAUDE_CONFIG_DIR` for claude, `CODEX_HOME` for codex) is the only route
that removes user-global skills, which no Claude Code flag hides. It costs one login per CLI,
not per worker (worker-operator-isolation D1 rejected one folder per worker). git-paw already
launches it as a wrapper `[clis.<name>]` entry with `family` and `settings_path`, so no new config
key is needed. Alternatives dropped: a per-item allowlist (needs ongoing verification per CLI
version, and claude has no switch for user-global skills) and building folder management into
git-paw (it would mean managing credentials).

### D3 — Delegates follow the same split

A delegate on an opted-out CLI keeps the worker-safety environment. Nothing else about delegates
changes.

## Risks / Trade-offs

- [An existing user relied on the opt-out restoring memory or suggestions] → v0.20.0 isn't released,
  so no released behaviour changes.
- [The codex example in the docs is unverified] → Marked as such. The v0.23.0 audit verifies it.

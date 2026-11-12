## Why

v0.20.0's operator isolation is all or nothing. `inherit_operator_config = true` brings back every
operator extension. It also turns back on two switches git-paw needs to work safely: auto memory
off, so workers never write the memory every worktree shares, and prompt suggestions off, so a
lone Enter from git-paw can't submit a predicted message. Users who want some extensions in their
workers have no clean route.

A per-item allowlist was explored and dropped. Claude Code has no switch that hides only user-global
skills, so an allowlist would need ongoing verification against each CLI version. A separate worker
config folder (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`) gives exact control with one login, and git-paw can
already launch it as a wrapper CLI entry.

Not yet released, so the meaning of `inherit_operator_config` can still change before v0.20.0
ships.

## What Changes

- **The claude isolation facet splits in two:**
  - *Worker safety* (always applied, opt-out or not): `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` and
    `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false`.
  - *Operator config* (dropped by `inherit_operator_config = true`): `--setting-sources
    project,local` and `CLAUDE_CODE_PLUGIN_DIRS=`.
- **`inherit_operator_config = true` now means "this CLI's own config decides what loads"**, not
  "launch exactly as before v0.20.0". Default launches (without the key) are unchanged.
- **A new docs chapter, "Customising workers"**, makes the worker config folder the recommended
  route. It covers the wrapper script, logging in once, installing the wanted plugins and skills
  there, the `[clis.<name>]` entry (`family`, `settings_path`, `inherit_operator_config = true`),
  and pointing workers at it with `default_profile`, a worker profile or `--cli`. It has a claude
  example, and a codex example marked as not yet verified.
- Not in scope: per-item allowlists, a launch-time probe and non-claude isolation facts (v0.23.0
  audit).

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `cli-provider`: *Operator isolation is a provider facet*, *A CLI may inherit the operator's
  configuration* and *Claude-family workers launch without prompt suggestions* change to the
  two-part facet. Opting out drops only the operator-config part.
- `core-memory-isolation`: a worker never writes the shared auto memory, even when its CLI opts out.
- `cli-agent-profiles`: *A delegate's argv is built from its CLI record without a shell* — an
  opted-out CLI's delegate keeps memory and prompt suggestions off.

## Impact

- **Code:** `src/agent_cli/mod.rs` (`OperatorIsolation` gains a split, or a second facet),
  `src/agent_cli/profile.rs` (`CliProfile::operator_isolation`), `src/commands/delegate.rs` (same
  rule for delegates).
- **Tests:** opt-out tests change on purpose (`opting_out_restores_the_previous_launch`, the
  prompt-suggestion opt-out test, the delegate opt-out test). Default-launch exact-line tests stay
  byte-identical.
- **Docs:** new `docs/src/user-guide/customising-workers.md` plus a `SUMMARY.md` entry; the
  `inherit_operator_config` row in `configuration/README.md`; `security-posture.md`,
  `supported-clis.md`, `cli-reference.md`, `agent-profiles.md`, `supervisor.md` wherever they say
  the opt-out restores the pre-v0.20.0 launch; CHANGELOG.
- **Security:** an opted-out worker can no longer write shared memory or get prompt suggestions.
  This only narrows what it can do.

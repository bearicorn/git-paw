## Context

A CLI has two names that the code treats as one: its **identity** (the `[clis.<name>]` key, or a
built-in / pass-through name such as `claude`) and its **launch command** (`[clis.<name>].command`).
Today:

- `detect.rs` `resolve_custom_clis_in` resolves `def.command` on PATH purely to decide
  availability, then stores `binary_name: def.name` — the resolved path is never used to launch.
- Every pane-launch site builds `PaneSpec.cli_command` from the identity: `main.rs`
  `attach_agent` (`cli_command = ctx.agent_cli …`, used by the supervisor and `add` paths),
  `commands/start.rs` `cmd_start` / `cmd_start_with_specs` (`cli_command: cli.clone()`),
  `commands/supervisor.rs` (`supervisor_cli_command`), `commands/recover.rs`
  (`cli_command_for_recovery` → `entry.cli`, and `recover_supervisor_session`).
- `tmux::gate_pane_for_injection(session, pane, cli_command, clis)` uses its one string both to
  resolve the prompt profile (`resolve_prompt_profile` → `base_token(cli_command)`) and to relaunch
  a still-bare pane.
- `skills::cli_speaks_mcp(name)` matches a hard-coded `MCP_CAPABLE_CLIS` table; `CustomCli` has no
  way to opt in or out.
- `KNOWN_CLIS` and `MCP_CAPABLE_CLIS` list `cursor`, which is usually the Cursor editor launcher;
  the agent CLI is `cursor-agent`.

Every per-CLI resolver (`resolve_approval_flags`, `resolve_session_id_args`, `resolve_resume_args`,
`resolve_submit_delay_ms`, `resolve_prompt_profile`) already keys on the identity, and
`WorktreeEntry.cli` stores it. That is correct and stays.

Note for implementers: the `git-paw` `[[bin]]` target sets `test = false`, so `#[cfg(test)]`
modules in `src/main.rs` and `src/commands/*.rs` are **not** compiled or run by `cargo test`. Tests
for bin-side behaviour MUST be integration tests under `tests/`; pure helpers go in the library
(`src/config/cli.rs`, `src/skills.rs`) where unit tests run.

## Goals / Non-Goals

**Goals:**
- One library helper turns an identity into the pane launch command; every launch path uses it.
- The identity remains the key for everything except the spawned binary.
- `[clis.<name>].mcp` overrides the built-in MCP table.
- Cursor's agent CLI is detected as `cursor-agent`.

**Non-Goals:**
- Launching via the absolute path `detect.rs` resolved (the pane's own PATH lookup is kept; see D1).
- Supporting `command` strings that embed arguments or environment assignments (they never passed
  availability resolution, which runs `which <command>`; quoting makes that explicit).
- Inferring MCP capability from the launch command's basename (see D4).
- A `git paw add-cli --mcp` flag — config-file only for now.

## Decisions

### D1 — `launch_command(cli, &clis)` in `src/config/cli.rs`, substituting the configured string

`pub fn launch_command<S: BuildHasher>(cli: &str, clis: &HashMap<String, CustomCli, S>) -> String`:
split the identity into its leading whitespace-delimited token and the remainder (the same
`base_token` convention the other resolvers use); if the token names a `[clis]` entry, return
`shell_word(entry.command) + remainder`; otherwise return `cli` unchanged.

It substitutes the configured `command` string, not `CliInfo.path`: the launch sites only have the
identity and `config.clis` in hand (not the detection result), and the pane's shell resolving
`command` on its own PATH matches how built-in CLIs launch today. Alternative considered — thread
`CliInfo.path` from detection to each launch site: rejected, it touches every resolver signature,
and recovery has no detection result at all.

### D2 — Quote only when needed, via `domain::shell_quote`

`shell_word(s)` returns `s` verbatim when every character is in the portable safe set
`A-Z a-z 0-9 _ - . / + , : @ %`, else `crate::domain::shell_quote(s)`. Bare names stay readable in
pane scrollback and existing tests that assert `cli_command == "codex"` hold; a path with a space or
metacharacter becomes one literal word. Alternative — always quote: simpler, but turns every
configured `claude` into `'claude'` in scrollback and test assertions for no safety gain.

The trailing remainder (flags carried in an identity such as a preset `cli = "x --model y"`) is
appended verbatim — it was already shell text before this change and is not re-quoted.

### D3 — The readiness gate takes the identity separately

`gate_pane_for_injection(session, pane, cli_id, cli_command, clis)`: `resolve_prompt_profile(cli_id,
…)` for markers, `relaunch_cli_into_pane(…, cli_command)` for the relaunch. `main.rs`
`gate_pane_or_fail_on_dialog` gains the same parameter; callers in `commands/supervisor.rs` and
`commands/add.rs` pass the identity they already hold (`supervisor_cli`,
`effective_agent_cli(&branches[idx])`, `agent_cli`). Without this, a variant with a
`prompt_profile` override would silently fall back to Claude's markers once its launch string
differs from its key.

### D4 — MCP capability: explicit opt-in on the identity

`CustomCli.mcp: Option<bool>` (`#[serde(default, skip_serializing_if = "Option::is_none")]`).
`skills::cli_speaks_mcp(cli, clis)` looks up the identity's leading token: `Some(b)` wins; else the
built-in table (`claude`, `codex`, `cursor-agent`, `agy`); else `false`. It deliberately does NOT
fall through to the launch command's basename: `[clis.claude-oss] command = "claude"` reads a
different config dir that may not have git-paw's MCP server registered, and an MCP boot block naming
tools the agent cannot call strands it at t=0 (the original design D4 rationale for the safe
default). The user opts in once they have registered the server for that variant.

`skills.rs` gains a `use crate::config::CustomCli` import — the same direction (`skills → config`)
the other per-CLI resolvers already establish for callers.

### D5 — `cursor` → `cursor-agent`, pass-through preserved

Edit `KNOWN_CLIS`, add `"cursor-agent" => "Cursor Agent"` to `known_display_name`, edit
`MCP_CAPABLE_CLIS`. Bare `--cli` values are not validated against detection, so `--cli cursor`
keeps launching `cursor`; spec-driven resolution (which validates availability) needs a
`[clis.cursor]` entry, exactly like the `gemini` precedent.

### D6 — Roster ↔ docs sync guard

A new `tests/supported_clis_docs_sync.rs` parses the binary cells of the README and
`docs/src/supported-clis.md` tables and compares them with the roster. The roster is private to
`detect.rs`; expose it read-only as `pub fn known_cli_names() -> &'static [&'static str]` (doc
comment required) rather than duplicating the list in the test.

## Risks / Trade-offs

- [Users relying on `cursor` auto-detection lose it] → called out as BREAKING in the proposal,
  CHANGELOG (via the commit), and both Supported-CLIs tables; pass-through keeps explicit use working.
- [A config whose `[clis.claude] command = "/abs/path/claude"` now launches that path instead of
  PATH's `claude`] → that is the documented meaning of `command`; noted in the config reference.
- [`mcp = true` on a CLI without git-paw's MCP server registered strands the agent at boot] →
  documented next to the field; default stays the helper form.
- [`command` is interpolated into a tmux `send-keys` shell line] → emitted as a single quoted word
  (D2); a real-shell round-trip test covers a path with a space.
- [Merge friction with `start-spec-launch-parity` / `broker-repo-identity` on the same lines] → this
  change is sequenced after both; `launch_command` composes with their resolved approval flags
  (`format!("{} {flags}", launch_command(cli, clis))`, no trailing separator when flags are empty).

## Migration Plan

No data migration. Existing configs load unchanged (`mcp` optional). Saved sessions keep
`cli = <identity>`; recovery now launches the identity's configured command, which is the fix.
Rollback is a plain revert.

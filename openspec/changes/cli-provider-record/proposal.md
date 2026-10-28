## Why

git-paw identifies an agent CLI by a string — the `[clis.<name>]` key or a known binary name — and
every per-CLI fact is a separate table keyed by that string. Two v0.17.0 dogfood findings are this
one defect seen twice:

- **GP-25.** Before v0.17.0 the pane was launched with the config **key**, so every custom entry
  needed a PATH shim named exactly like its key (the dogfood workers pinned to Sonnet needed
  `claude-oss-sonnet` on PATH). v0.17.0 `cli-launch-identity` launches the configured `command`,
  but a leftover shim is now bypassed without a word, and there is still no config seam for
  per-worker launch arguments (model, effort, extra flags).
- **GP-29.** `skills::cli_speaks_mcp` matched the CLI *name* against a fixed list, which no custom
  key could satisfy. v0.17.0 added the `[clis.<name>] mcp` opt-in, so a genuine Claude Code variant
  now gets the MCP boot block only by restating that fact on its own entry.

The same name-keyed lookup governs `config::approval_flags` (native auto-mode), the session-id and
resume templates, and — from v0.18.0 — the headless one-shot template. A variant of a known CLI gets
none of them. Recovery (`commands/recover.rs::cli_command_for_recovery`) assembles its pane command
separately from the start paths.

## What Changes

- **A resolved CLI record.** Each agent CLI resolves once into a `CliProfile`: its id, display name,
  the executable to launch, launch arguments, its provider family, and the per-CLI facets (approval
  flags per level, session-id / resume / headless templates, MCP capability, prompt-shape profile).
  Consumers read the record; nothing re-derives a CLI's identity from a name or a launch string.
- **Provider families as a compiled Strategy.** In-tree providers for the CLIs whose facts the code
  encodes today (Claude Code, Codex, Antigravity, Qwen, Cursor) plus a `generic` Null Object. No
  runtime plugins.
- **`[clis.<name>].family`** (optional) declares which provider a custom entry is a variant of.
  Absent, a known name keeps its own family and anything else is `generic` — never inferred from the
  command's basename, so existing configs resolve exactly as before.
- **GP-25 finished:** v0.17.0 already launches the configured `command`; the record now carries
  it. A custom key that *also* resolves on PATH to a different executable than `command` (the old
  shim workaround) is refused with an error naming both, so a wrapper — for example a sandbox
  wrapper — is never silently bypassed.
- **`[clis.<name>].args`** (optional) is appended to every launch of that CLI. Per-worker variation
  is one `[clis.*]` entry per worker profile.
- **One launch-command builder** for every worker launch path — start (bare, spec-driven,
  supervised), `add`, recovery, and the readiness relaunch: executable, args, approval flags, then
  session-id or resume arguments. Recovery gains the `args` it omits today.
- **Built-in tables keyed by family**: approval flags, session-id / resume templates, MCP capability
  (GP-29), and v0.18.0's headless template. Lookup stays override → built-in → none. For MCP
  capability the override is v0.17.0's `[clis.<name>] mcp`: an explicit value wins, and the family
  supplies the default only when it is unset.
- **DS-6 groundwork:** the detection roster becomes a field of the provider records, so the v0.23.0
  audit can correct or drop an entry in one place (v0.17.0 already replaced `cursor` with
  `cursor-agent`).

## Capabilities

### New Capabilities

- `cli-provider`: the resolved per-CLI record, the compiled provider families with a generic
  fallback, family declaration, launch executable and arguments, the single launch-command builder,
  family-keyed built-in tables, and the MCP boot-form decision.

### Modified Capabilities

- `cli-resolution`: *Per-CLI session-id and resume argument templates* — built-in rows keyed by
  provider family.
- `supervisor-config`: *Permission flag mapping* — built-in rows keyed by provider family. The delta
  starts from the text v0.17.0 `classifier-retirement` gives this requirement (no classifier, prompts
  the resolved mode leaves in place are escalated, never approved) and changes only the table key
  and the variant scenarios.
- `session-state`: *Recovery relaunches a worker with its recorded session id* — recovery builds the
  full launch command (args and approval flags included).
- `core-security-posture`: *Documented FS-scoped sandbox workflow* — the wrapper no longer has to be
  PATH-resolvable by the key's name.
- `boot-block`: *Per-CLI MCP capability override* — when `mcp` is unset, the default comes from the
  provider family instead of a list of built-in names; an explicit `mcp` still wins.

## Impact

- **Code:** new `src/agent_cli/` (`CliFamily`, `CliProvider` trait + in-tree providers, `CliProfile`,
  resolution); `src/detect.rs` (`KNOWN_CLIS` derived from the records; resolved path kept);
  `src/config/cli.rs` (`family`, `args`), `src/config/mod.rs` (validation),
  `src/config/supervisor.rs` (`approval_flags`, `built_in_session_id_args`,
  `built_in_resume_args` become provider data); `src/skills.rs` (`cli_speaks_mcp` reads the
  record: the `mcp` override, then the family default; `MCP_CAPABLE_CLIS` removed); launch sites `src/main.rs::attach_agent`,
  `src/commands/{start,add,recover,supervisor}.rs`, `src/tmux/readiness.rs::relaunch_cli_into_pane`.
  `resolve_submit_delay_ms` and `resolve_prompt_profile` stop keying on a launch string's leading
  token.
- **Config:** two optional `[clis.<name>]` fields; `git paw init` commented examples. Existing configs
  load unchanged. Persisted state unchanged: `WorktreeEntry.cli` stays the id.
- **Behaviour an existing user notices:** the ambiguous-shim case is refused with a fix-it message;
  recovered workers get their `args`.
- **Enum-variant ripple:** none (`BrokerMessage`, `SpecBackendKind` untouched). `CliFamily` is new.
- **Not covered:** the v0.17.0 `git paw setup-cli` fact table (`cli-setup-consent`) stays keyed by
  CLI identity; a declared variant does not inherit a setup row (design Non-Goals).
- **Cross-version dependencies:** rebases over v0.17.0 `start-spec-launch-parity` (adds approval flags
  to the bare and spec launch paths — this change folds them into the one builder),
  `cli-unavailable-detection-gaps` (per-CLI marker lookup) and `classifier-retirement` (whose
  MODIFIED *Permission flag mapping* this change's delta builds on, so archiving it after v0.17.0
  does not bring back the classifier wording); v0.18.0 `cmd-library-surface` (moves
  `attach_agent` and the launch paths into the library), `supervisor-pane-removal` (no supervisor
  pane command to build) and `orchestrator-escalation-handoff` (its `headless_args` table becomes a
  family-keyed facet). Followed by `cli-profile-threading`, `worker-sandbox-mode`,
  `worker-usage-budget` and `verified-agent-send`, which all consume the record.
- **Dependencies:** none.

## Rebase note (2026-10-19)

v0.17.0 pulled part of this forward as `cli-launch-identity`: every launch path spawns the resolved `[clis.<name>].command` (GP-25), a new `[clis.<name>] mcp = true|false` overrides the MCP-capable table (GP-29), and the roster replaces `cursor` with `cursor-agent` (DS-6). This change is built on the archived v0.17.0 text. An explicit `mcp` wins over the family; `family` supplies the default only when `mcp` is unset (the MODIFIED `boot-block` *Per-CLI MCP capability override*).

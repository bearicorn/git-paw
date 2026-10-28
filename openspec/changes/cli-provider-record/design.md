## Context

Per-CLI knowledge is spread over string-keyed lookups, each with its own idea of what "the CLI" is:

| Fact | Where | Keyed by |
|---|---|---|
| detection roster | `detect::KNOWN_CLIS` | binary name |
| launch target | `config::launch_command` (v0.17.0 `cli-launch-identity`) | `[clis.<name>].command` substituted for the identity's leading token; a CLI with no entry by its name |
| native auto-mode flags | `config::approval_flags` (claude, agy, codex, qwen) | exact CLI string |
| session-id / resume | `config::built_in_session_id_args` / `built_in_resume_args` (claude) | exact CLI string |
| MCP boot form | `skills::cli_speaks_mcp`: `[clis.<name>] mcp`, then `MCP_CAPABLE_CLIS` (claude, codex, cursor-agent, agy) | leading token of the CLI identity |
| settle delay, prompt profile | `resolve_submit_delay_ms`, `resolve_prompt_profile` | leading token of the CLI identity |
| headless one-shot (v0.18.0) | `orchestrator-escalation-handoff` `headless_args` table | CLI string |

v0.17.0 `cli-launch-identity` launches `command` while keeping every lookup on the leading token of
the CLI identity, so each consumer still parses that string itself. The record replaces the
convention.

Launch strings are typed into the pane's shell by `tmux send-keys` (`tmux/command.rs`), so the
executable must be quoted, as v0.17.0's `config::launch_command` already does. Recovery
(`commands/recover.rs`) assembles its own pane command (configured `command`, approval flags, resume
arguments); the readiness relaunch reuses whatever string it was handed.

The user's own setup shows the cost: `~/.local/bin/claude-oss-sonnet` is a shim that sets
`CLAUDE_CONFIG_DIR`, `--model sonnet --effort medium` and `--permission-mode acceptEdits`, existing
because the key had to be executable before v0.17.0 and there is still no args seam; its config comment notes the
permission flag is baked in because the spec launch path ignored `approval_args` (fixed by v0.17.0
`start-spec-launch-parity`).

## Goals / Non-Goals

**Goals:**

- One resolved record per agent CLI, read by every consumer.
- Launch what the user configured; make per-worker model/effort a config edit, not a shim.
- Variants of a known CLI (`family = "claude"`) get that CLI's native flags, templates and MCP form.
- Existing configs resolve exactly as before unless they hit the ambiguous-shim refusal (D4).
- A provider resolvable with no PATH lookup and no config file, so v0.23.0's conformance suite can
  drive each family directly.

**Non-Goals:**

- Threading the record into the detectors — `cli-profile-threading`.
- New third-party facts. Only rows the code already encodes move into providers; everything else is
  left for the v0.23.0 audit to verify.
- Native lifecycle-hook providers (see Open Questions), a support matrix, or tiers (v0.23.0).
- An `env` field on `[clis.<name>]`; a wrapper script (now pointed at by `command`) covers
  environment such as `CLAUDE_CONFIG_DIR`.
- The v0.17.0 `git paw setup-cli` fact table (`cli-setup-consent`). It stays keyed by CLI identity,
  not by family: `cli-setup` says no other identity — including a `[clis.<name>]` entry that launches
  a variant of a declared CLI — has a row. A declared `family = "claude"` variant therefore gets the
  native flags, templates and MCP form, but `setup-cli <variant> --allowlist` still answers "not
  supported". Extending the setup items to variants is left to the v0.23.0 audit, which verifies the
  facts per CLI.

## Decisions

**D1 — Module and types.** New `src/agent_cli/` (git-paw's term; `cli` already means git-paw's own
clap surface): `CliFamily` (closed enum: `Claude`, `Codex`, `Antigravity`, `Qwen`, `Cursor`,
`Generic`), a `CliProvider` trait with one in-tree impl per family, and `CliFamily::provider() ->
&'static dyn CliProvider` as the only exhaustive match. `CliProfile` is the resolved record. Same
shape as `spec-backend-strategy`: enum for the parseable identity, trait for behaviour. MILESTONE's
`HookProvider` name is not used — hooks are one deferred facet, not the contract.

**D2 — Provider methods default to "none".** The trait's defaults are the Null Object: no approval
flags, no templates, not MCP-capable — and, for the shape facets, the
embedded defaults every CLI gets today (the prompt profile and the first-run dialogs), so an
unrecognised CLI keeps today's detection rather than losing it. `Generic` overrides nothing. This
leaves `cli-prompt-profiles` *An unrecognised CLI resolves a usable profile* (the Claude default for
unknown CLIs) unmodified. The v0.23.0 audit's "launch-only = generic profile" tier would change that
requirement; that is its decision and its delta, made with the per-CLI evidence this cycle lacks. Each family overrides only what the code encodes
today: Claude (approval rows, session-id/resume, MCP, prompt profile), Codex (approval rows, MCP),
Antigravity (full-auto row, MCP), Qwen (full-auto row), Cursor (MCP).

**D3 — Family is declared, never inferred.** `[clis.<name>].family` accepts the family names
(`claude`, `codex`, `agy`, `qwen`, `cursor-agent`, `generic`). Absent: a known name keeps its family,
anything else is `generic`. Inferring from `command`'s basename was rejected: it would silently add
permission flags to entries like `[clis.foo] command = "/usr/bin/claude"` that run flagless today,
and it cannot see through a wrapper script anyway. The name `family` matches the "Claude family"
wording used throughout the roadmap; `provider` and `base` were the alternatives.

**D4 — Launch as v0.17.0 does; refuse the ambiguous shim.** Panes keep v0.17.0's
`cli-launch-identity` rule: a configured entry launches its `command` (one shell word), and a
built-in CLI with no entry launches by name, so PATH-based version managers keep working as they do
in the user's shell (decided 2026-10-25; launching the detected absolute path was rejected because
it changes how every built-in CLI starts). This change adds one refusal: when a custom key that is
not a known name also resolves on PATH to a different file (compared canonically) than `command`,
launching that CLI fails with an error naming both paths and the fix. Rationale: before v0.17.0
such a setup launched the PATH shim; since v0.17.0 it launches `command` — and if the shim was a
sandbox or config-dir wrapper, that switch drops the sandbox or changes the login without a word.
Known names are exempt: setting `[clis.claude] command = "/opt/…/claude"` is an explicit override
of a detected CLI, which the docs already promise. Alternative (launch `command` and warn) rejected
for the sandbox case.

**D5 — `args` is a verbatim string.** It is appended after the executable, before the approval
flags, matching how `approval_args` and the templates are already appended. A token list with
per-token quoting was considered; rejected for consistency with the existing fields and because
`[clis.*]` is already trusted input (it names the executable; `[worktree.hooks]` runs arbitrary
commands under the same trust model).

**D6 — One builder, every launch path.** `CliProfile::launch_command(level, session)` returns the
pane command: executable, `args`, approval flags for `level`, then `session_id_args` (fresh launch)
or `resume_args` (recovery with a recorded id). Empty parts leave no separators. Recovery uses the
current config's `agent_approval`, as it does since v0.17.0; it cannot know the level at original
launch.

**D7 — Tables keyed by family, overrides by id.** Lookup order stays `[clis.<id>]` override →
family built-in → none. Override keys stay the CLI id, so `[clis.claude]` overrides keep working.
For MCP capability the override is v0.17.0's `[clis.<id>] mcp`: an explicit `true` or `false` wins,
and the family supplies the default only when `mcp` is unset. A known CLI resolves its own family,
so with no config the MCP-capable CLIs stay exactly `claude`, `codex`, `cursor-agent` and `agy`, and
an existing `mcp = true` on an undeclared entry keeps its MCP form. The `boot-block` requirement
*Per-CLI MCP capability override* is MODIFIED to say so.
The v0.18.0 `headless_args` built-in rows move into the Claude and Codex providers the same way.

**D8 — Roster from the records.** `KNOWN_CLIS` becomes the list of detection names declared by the
roster entries (17 today, each mapped to a family, `generic` for the 12 without encoded facts). The
list is unchanged here.

**D9 — Capability name.** `cli-provider`, not the roadmap's `per-cli-provider`, to follow the
`<namespace>-<sub>` convention (`cli-` already holds `cli-resolution` and `cli-prompt-profiles`).

## Risks / Trade-offs

- **The ambiguous-shim refusal is a behaviour change** → since v0.17.0 such entries launch `command`
  and silently skip the shim; D4 refuses the case where the two disagree, and the migration note
  tells users with shims what to set.
- **Variants gain flags only when declared** → a `claude-oss` user must add `family = "claude"` to
  get the native flags (an existing `mcp = true` keeps the MCP form); documented as the upgrade step,
  and nothing changes for them until they do.
- **Duplicate flags** (a shim or `args` that already passes `--permission-mode`) → documented: put
  permission flags in `approval_args`, not `args`. Whether a CLI rejects a repeated flag is to be
  verified by the v0.23.0 audit.
- **Recovery now carries `args`** → intended (one builder for every launch path); called out in the
  changelog.
- **Concurrent edits** → `attach_agent` and the launch paths move in v0.18.0 `cmd-library-surface`;
  this change lands after it and edits the moved code.

## Migration Plan

No config migration is required. Users with PATH shims may delete them: point `command` at the real
binary or wrapper, move fixed flags into `args`, permission flags into `approval_args`, and add
`family` for a variant of a known CLI. Rollback is reverting the change. The config structs do not
deny unknown fields, so an older binary ignores `family` and `args` — it launches `command` without
the `args` and without the family's flags; the changelog says configs that moved flags into `args`
or rely on `family` need v0.19.0+.

## Open Questions

- **Native lifecycle hooks (not specced here).** The roadmap's per-CLI hook providers would deliver
  agent lifecycle events (turn finished, waiting for approval) from each CLI's own hook mechanism,
  with today's screen observation as the fallback. Two blockers: no CLI's hook format is encoded or
  verified in this codebase, and the obvious delivery (a settings file in the worktree) is forbidden
  by `approval-command-safety` *No permission grants are seeded into agent CLI settings files*
  (as v0.17.0 `cli-setup-consent` words it: no automatic write and no vendor settings directory
  inside a worktree; hook configuration is written only by the explicit `git paw setup-cli`, whose
  `--hooks` item no CLI declares yet). A launch-flag delivery might avoid that — to verify by the
  v0.23.0 audit. Needs a decision before it is specced.
- **Cursor's executable (DS-6).** Settled by v0.17.0 `cli-launch-identity`: the roster detects
  `cursor-agent`, and the bare `cursor` (the editor's launcher) is no longer a known CLI. The Cursor
  provider's family is `cursor-agent`.
- **Facts for the other twelve known CLIs** (native auto-mode flags, MCP support, resume flags) —
  none are encoded, so they resolve `generic`. To verify by the v0.23.0 audit.

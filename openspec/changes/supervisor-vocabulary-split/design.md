## Context

The word "supervisor" reaches users through five channels at this tip, plus what v0.18.0 and
v0.19.0 add:

- **CLI:** `start --supervisor` / `--no-supervisor` (`cli.rs::Command::Start`, fields `supervisor`,
  `no_supervisor`; `--unattended` `conflicts_with = "no_supervisor"`); the "Start in supervisor
  mode?" prompt (`commands/supervisor.rs::TerminalSupervisorPrompt`); help text of `start`, `add`,
  `remove`, `init`, `approvals`, `doctor`, `__prompt-profile`.
- **Config:** `[supervisor]` → `config::SupervisorConfig` (41 keys and sub-table keys at this tip;
  v0.18.0 adds `[supervisor.escalation]` and v0.19.0 the two budget caps), merged whole-section in `PawConfig::merged_with`
  (`overlay.supervisor.or_else(|| self.supervisor)`), resolved by
  `resolve_supervisor_mode` (flag → `[supervisor]` present ? `enabled` : prompt). `init` appends a
  `[supervisor]` section (`prompt_supervisor_section`). `sweep.sh` reads `[supervisor]`
  `test_command` and three windows through `tomllib` with a regex fallback keyed on the literal
  header `[supervisor]`.
- **Persisted and machine output:** `SessionMode` serializes `"bare"` / `"supervisor"`
  (`session.rs`); MCP `SessionSnapshot.mode` reports the same strings (`mcp/query/session.rs`);
  doctor's `GROUP_SUPERVISOR = "Supervisor"` appears in `--json` `group`.
- **Broker wire:** the root identity `"supervisor"` is the `agent_id` regex's literal
  (`broker/server.rs`), the inbox name in `delivery.rs::route_message` (questions, verify-now), the
  `from` / `verified_by` value the drive loop's correction loop counts (`drive::gate_failure`,
  `SUPERVISOR_AGENT_ID`) and that v0.18.0 `broker-verdict-authority` gates, the dashboard's pinned
  row (`dashboard/view.rs::SUPERVISOR_AGENT_ID`), the inventory's excluded row, and every `sweep.sh`
  publish; `BrokerMessage::VerifyNow` is `#[serde(rename = "supervisor.verify-now")]`.
- **Prose:** skills, boot-block templates, inter-agent rules, hooks, governance header, learnings
  lines, MCP tool descriptions, docs.

`config-schema-versioning` (this release) adds the `version` key, the pure `toml::Value` migration
chain and the `config::wire` / model split; `wire-format-dtos` puts the session file and the broker
messages behind wire types; `uniform-json-output` introduces doctor check ids. Those are the seams
this change uses. `cli-contract-freeze` (v1.0.0) planned a deprecation registry for 1.0; it is needed
now.

## Goals / Non-Goals

**Goals:**
- Ship the new names with every old name still working and warning, so 1.0.0 can remove them.
- Never rewrite a user's file to migrate it; never change stdout or an exit code to warn.
- One canonical root identity inside the process, so no check can be bypassed with the old spelling.
- A deprecation mechanism that later deprecations (`--from-specs`, `tmux_running`, …) reuse.

**Non-Goals:**
- Renaming Rust items (`src/supervisor/`, `SupervisorConfig`, `cmd_supervisor`): not user-visible,
  and the library API is outside the contract (`cli-contract-freeze` D6).
- Renaming the `supervisor-*` capability directories or rewriting every existing requirement that
  mentions an old spelling (D10 — the 1.0.0 sweep).
- Renaming doc files (D11); removing anything (`remove-supervisor-vocabulary`).

## Inventory

Disposition: **R** renamed here (old form aliased or read, with a warning where it is an input),
**P** prose reworded here (no alias needed), **K** kept (reason given), **1.0** handled by
`remove-supervisor-vocabulary`.

| # | Surface | Where | Disposition |
|---|---|---|---|
| 1 | `start --supervisor` | `cli.rs` | R → `--coordinated`; hidden alias + warning; 1.0 rejects |
| 2 | `start --no-supervisor` | `cli.rs` | R → `--bare`; hidden alias + warning; 1.0 rejects |
| 3 | `start --unattended` help + conflict | `cli.rs` | P "coordinated wave"; conflicts with `--bare` (and its alias) |
| 4 | `start` long_about examples | `cli.rs` | P (`--coordinated`) |
| 5 | "Start in supervisor mode?" prompt | `commands/supervisor.rs` | P → "Start in coordinated mode?" |
| 6 | `start --dry-run` plan header | `commands/supervisor.rs` | P → "coordinated session plan" |
| 7 | `init` prompt + appended `[supervisor]` | `init.rs` | R → "Use coordinated mode by default?", writes `default_mode` |
| 8 | `init`, `add`, `remove`, `approvals`, `doctor`, `__prompt-profile` help | `cli.rs` | P (new table/key names, "orchestrator") |
| 9 | `git paw remove supervisor` refusal | `commands/remove.rs` | R refuses `orchestrator` and `supervisor`; 1.0 only `orchestrator` |
| 10 | `[supervisor] enabled` | config | R → top-level `default_mode` (D2) |
| 11 | `[supervisor] cli` | config | R → `[orchestrator] cli` |
| 12 | `[supervisor] approval` | config | R → `[orchestrator] approval` |
| 13 | `[supervisor.escalation]` (v0.18.0) | config | R → `[orchestrator.escalation]` |
| 14 | `[supervisor] verify_on_commit_nudge` | config | R → `[orchestrator]` (refined, D1) |
| 15 | `[supervisor] auto_revert` | config | R → `[orchestrator]` (refined, D1) |
| 16 | `[supervisor] test_command` | config | R → `[orchestrator.gates] test_command` |
| 17 | `[supervisor] lint_command` | config | R → `[orchestrator.gates]` |
| 18 | `[supervisor] build_command` | config | R → `[orchestrator.gates]` |
| 19 | `[supervisor] doc_build_command` | config | R → `[orchestrator.gates]` |
| 20 | `[supervisor] doc_tool_command` | config | R → `[orchestrator.gates]` |
| 21 | `[supervisor] spec_validate_command` | config | R → `[orchestrator.gates]` |
| 22 | `[supervisor] fmt_check_command` | config | R → `[orchestrator.gates]` |
| 23 | `[supervisor] security_audit_command` | config | R → `[orchestrator.gates]` |
| 24 | `[supervisor] agent_approval` | config | R → `[workers] approval` (leaf renamed, D1) |
| 25 | `[supervisor.auto_approve]` | config | Dropped by the v1→v2 step (deprecated 0.17.0 by `classifier-retirement`, no effect); never moved to `[workers]`; registry entry with no removal release (D13) |
| 26 | `[supervisor.common_dev_allowlist]` | config | Dropped as row 25 |
| 27 | `[supervisor.tell]` | config | R → `[workers.tell]` |
| 28 | `[supervisor] strict_branch_guard` | config | R → `[workers]` |
| 29 | `[supervisor] branch_refresh` | config | R → `[drive]` (refined, D1) |
| 30 | `[supervisor.correction]` | config | R → `[drive.correction]` |
| 31 | `[supervisor.conflict]` | config | R → `[drive.conflict]` |
| 32 | `[supervisor] learnings` | config | R → `[drive.learnings] enabled` (D1) |
| 33 | `[supervisor.learnings_config]` | config | R → `[drive.learnings]` (merged with 32) |
| 34 | `[supervisor] manual_approvals_log` | config | R → `[drive]` |
| 35 | `[supervisor] no_progress_window_seconds` | config | R → `[drive]` |
| 36 | `[supervisor] context_bloat_threshold_k` | config | R → `[drive]` |
| 37 | `[supervisor] blocked_on_supervisor_window_seconds` | config | R → `[drive] blocked_on_orchestrator_window_seconds` |
| 38 | `[supervisor] budget_max_tokens` / `budget_max_cost` (v0.19.0) | config | R → `[drive]` (refined, D1) |
| 39 | `[broker] wave_stall_minutes` (v0.18.0) | config | K — not a `[supervisor]` key; already in its right table |
| 40 | config `version` | `config/migrate.rs` | R → 2; v1→v2 step (D3) |
| 41 | `init` default config template | `config::generate_default_config` | R → commented `[orchestrator]`, `[orchestrator.gates]`, `[workers]`, `[drive]` blocks |
| 42 | Script-read keys (`sweep.sh`) | `assets/scripts/sweep.sh` | R → new keys first, `[supervisor]` fallback (D12) |
| 43 | Config error / warning texts (`[supervisor.correction]`, stall clamp) | `config/supervisor.rs` | P → new table names |
| 44 | Pre-commit hook remedy (`[supervisor] strict_branch_guard`) | `assets/hooks/pre-commit.sh` | P → `[workers]` |
| 45 | Role-guard feedback (`[supervisor] auto_revert`) | `opsx/role_guard.rs` | P → `[orchestrator]` |
| 46 | Session file `mode: "supervisor"` | `session.rs` | R readers accept both from here; writers switch at 1.0 (D8) |
| 47 | MCP `get_session_status.mode` | `mcp/query/session.rs` | R → add `session_mode`; `mode` deprecated; 1.0 removes `mode` |
| 48 | Doctor group "Supervisor" (human, `--json` `group`) | `doctor.rs` | R → "Coordination"; ids `coordination.*` (D9) |
| 49 | Doctor check names / remedies | `doctor.rs::check_supervisor` | P |
| 50 | Discovery-file `orchestrator` entry | `session.rs::RepoOrchestratorEntry` | K — already "orchestrator"; v0.18.0 stops writing it |
| 51 | `PAW_SUPERVISOR_PID` marker line | `agents.rs` | K — undocumented internal marker read only by git-paw; listed for the 1.0 sweep |
| 52 | Root identity `agent_id = "supervisor"` | broker | R → `orchestrator`; inbound alias normalized (D7); 1.0 rejects |
| 53 | Sender fields `verified_by`, `from` (feedback, answer, blocked, advanced-main), learning `agent_id` | broker | R as 52 |
| 54 | `GET /messages/supervisor` | `broker/server.rs` | R → `/messages/orchestrator`; old path served from the same inbox; 1.0 rejects |
| 55 | Type `supervisor.verify-now` | `broker/messages.rs` → `broker/wire.rs` | R → `orchestrator.verify-now`; old decodes; 1.0 rejects on publish |
| 56 | `agent_id` shape rule literal | `broker/server.rs` | R adds `orchestrator`; 1.0 drops `supervisor` |
| 57 | Stuck flag `blocked-on-supervisor` | `sweep.sh` | R → `blocked-on-orchestrator` (no program parses it) |
| 58 | Dashboard pinned root row | `dashboard/view.rs` | R follows 52 |
| 59 | Exit summary per-agent root row | `supervisor/drive.rs::DriveSummary` | R follows 52 |
| 60 | `sweep.sh` phantom filter, `inbox`, `status-publish`, `learn`, usage text | `sweep.sh` | R follows 52/54 |
| 61 | Correction / escalation texts ("Supervisor gate … failed", "supervisor correction budget") | `supervisor/drive.rs` | P |
| 62 | Skill name `supervisor` (v0.18.0 alias) and `supervisor.md` override | `skills.rs`, MCP `get_skill` | R warns; 1.0 removes |
| 63 | Coordination skill prose | `assets/agent-skills/coordination.md` | P |
| 64 | Orchestrator skill prose (`/messages/supervisor`, keys) | `assets/agent-skills/orchestrator.md` | P |
| 65 | Boot-block templates | `assets/boot-block*.md` | P |
| 66 | Inter-agent rules | `assets/inter-agent-rules.md` | P |
| 67 | Governance section header | `assets/governance-section-header.md` | P |
| 68 | Pre-push hook message | `assets/hooks/pre-push.sh` | P |
| 69 | Escalation framing (v0.18.0) | `assets/escalation-framing.md` | P |
| 70 | Learnings lines ("supervisor told …") | `broker/learnings.rs` | P |
| 71 | MCP tool descriptions, server instructions | `mcp/tools/*`, `mcp/server.rs` | P |
| 72 | Env vars | — | K — none contains "supervisor" |
| 73 | Rust modules, types, functions | `src/supervisor/`, `SupervisorConfig`, … | K — not user-visible (Non-Goals) |
| 74 | `supervisor-*` capability names, requirement texts | `openspec/specs/` | K here (reading rule, D10); 1.0 sweep |
| 75 | `docs/src/user-guide/supervisor.md` | docs | P retitled "Coordinated mode"; file renamed at 1.0 (D11) |
| 76 | Configuration reference | `docs/src/configuration/README.md` | R new tables + migration table |
| 77 | CLI reference | `docs/src/cli-reference.md` | R flags + Deprecations section |
| 78 | README, architecture, other user-guide pages | docs | P |

78 rows. Kept surfaces: 39 (wave_stall_minutes), 50 (discovery `orchestrator` entry), 51
(`PAW_SUPERVISOR_PID`), 72 (env vars), 73 (Rust items), 74 (capability names, until 1.0).

## Decisions

### D1 — Key placement (refines the user-approved table)
`[orchestrator]` = how git-paw invokes and nudges the external orchestrator: `cli`, `approval`,
`.escalation`, `.gates`, plus `verify_on_commit_nudge` (its only recipient is the orchestrator's
inbox) and `auto_revert` (read by the orchestrator's revert flow through the role-guard feedback).
`[workers]` = policy applied to worker panes and branches: `approval` (was `agent_approval`; the
leaf is renamed so `[orchestrator] approval` and `[workers] approval` read as a pair), `.tell`,
`strict_branch_guard`. `[drive]` = git-paw's own coordination machinery: `.correction`,
`.conflict`, `.learnings`, `manual_approvals_log`, the three stuck windows, `branch_refresh` and the
budget caps. Departures from the approved table: `verify_on_commit_nudge` and `auto_revert` go to
`[orchestrator]`, `branch_refresh` and the budget caps to `[drive]` (both are `DriveConfig` inputs:
the refresh runs in the drive loop, and on a budget breach the drive loop stops the spend it
initiates); `wave_stall_minutes` stays in `[broker]` (it was never a `[supervisor]` key). Two leaf changes ride along because the move is the only cheap moment:
`blocked_on_supervisor_window_seconds` → `blocked_on_orchestrator_window_seconds`, and
`learnings` + `[supervisor.learnings_config]` merge into `[drive.learnings]` (`enabled`,
`flush_interval_seconds`, `broker_publish`), removing the "named `_config` to avoid colliding with
the boolean" wart. `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` are not
carried into version 2 at all (D13).
*Alternative:* `[broker.conflict]` / `[broker.learnings]` (both run in the broker host). Rejected to
keep the user's concern split: a user looks for "what git-paw does automatically" in one table.

### D2 — `enabled` becomes top-level `default_mode`
`default_mode = "coordinated" | "bare"`, beside `default_cli`, mirroring `--coordinated` / `--bare`
and the session mode. Resolution: `--bare` → bare; `--coordinated` or `--unattended` → coordinated;
`default_mode` → its value; absent → the prompt (bare when stdin is not a terminal or on
`--dry-run`). The migration maps `[supervisor] enabled = true` → `"coordinated"`, `false` →
`"bare"`, and a `[supervisor]` table without `enabled` → `"bare"` — today a present section means
"do not prompt", and that is preserved. `default_mode` is not an active template key, so init's
generic top-level backfill never adds it; only the init prompt writes it.
*Alternatives:* `[drive] enabled` (the drive loop runs only with `--unattended`, so the name lies);
`[orchestrator] enabled` (coordinated mode is git-paw's, not the orchestrator's).

### D3 — Version 2 is a step in `config-schema-versioning`'s chain
`CURRENT_CONFIG_VERSION` becomes 2. `migrate_v1_to_v2(&mut toml::Value) -> Vec<MigrationNote>` is a
pure table move driven by one static mapping (old path → new path) that is also the source of the
registry's config entries, the configuration reference's migration table, and `config check`'s
known-key tree; a test asserts the three agree. It runs only for documents that declare version 1
or nothing. Rules:
- an old path whose new path is already present in the same document → `ConfigError` naming both
  paths (a sub-table's counterpart is the whole new sub-table);
- a document declaring `version = 2` that contains `[supervisor]` → `ConfigError` naming the table
  and pointing at the migration table (version 2 has one spelling);
- `init` backfills `version = 2` only into a file with no `[supervisor]` table; otherwise it leaves
  `version` absent and prints one line pointing at `git paw config check`. Without this guard the
  version-1 file would be stamped version 2 and then refused;
- `add-cli` / `remove-cli` already rewrite the global file through the wire type, so a global
  `[supervisor]` comes out in the version-2 layout; they print one line saying so, naming any
  deprecated auto-approval table the rewrite dropped (D13).
*Alternative:* accept `[supervisor]` in any document. Rejected: two spellings in the current format
and an ambiguous meaning of `version`.

### D4 — The coordination tables merge as one unit
Today the repo's `[supervisor]` replaces the global one wholesale. Per-table precedence over the
split tables would change the merged result for existing pairs of files — a global
`agent_approval = "full-auto"` would start leaking into a repo whose own `[supervisor]` never set
it. So `[orchestrator]`, `[workers]` and `[drive]` merge as one unit: if the repo file sets any of
them (after its own migration), the global file's three tables are ignored. `default_mode` is an
ordinary top-level scalar (repo over global); D2's implicit `"bare"` keeps every old pair's result.
The internal model keeps one optional coordination group, so `merged_with` is unchanged.

### D5 — One registry, one helper (`core-deprecation`)
`src/deprecation.rs`: `Deprecation { item, kind, since, replacement, removed_in: Option<&str> }`
in a static `REGISTRY`, where `replacement` is either the new spelling or a no-effect marker (for
the 0.17.0 auto-approval tables, D13, whose line ends "it has no effect and can be removed"); a pure `notice(&Deprecation, context) -> String`; `warn(&Deprecation,
context)` printing to stderr once per (item, context) per process; `scan_args(raw) -> Vec<&Deprecation>`
for flag spellings, stopping at `--`. Format: `warning: [<context>: ]<item> is deprecated since
<since>[ and will be removed in <removed_in>]; use <replacement> instead`. Kinds: flag, config key,
skill name, wire value, output field (output fields are published, never warned — nothing "uses"
them). Config entries have `removed_in = None`: version-1 files keep loading through 1.x
(`remove-supervisor-vocabulary`), so promising a removal would be false. A unit guard fails when
`removed_in <= CARGO_PKG_VERSION` and the entry still exists, which forces every 1.0.0 removal to
happen in 1.0.0. `config-check-command`'s D4 table becomes the registry's config entries; v0.24.0
registers `--from-specs` here.
*Alternative:* clap's own deprecation support — covers flags only, and two mechanisms drift.

### D6 — Old flags are clap aliases plus a raw-argument scan
`#[arg(long, alias = "supervisor")] coordinated` and `#[arg(long, alias = "no-supervisor")] bare`:
clap hides aliases from `--help` and completions and applies the conflict rules to them. clap does
not report which spelling matched, so `main` calls `deprecation::scan_args` on `std::env::args`
after parsing (the same approach v0.24.0 designed for `--from-specs`).

### D7 — Wire identity: rename to `orchestrator`, normalize the old spelling at the wire edge
Recommended. The `broker::wire` → model mapper rewrites `supervisor` to `orchestrator` in the
envelope `agent_id`, the payload sender fields and the learning `agent_id`; the `/messages/:agent_id`
handler maps the old path parameter; decoding accepts `supervisor.verify-now`. Everything inside the
process — validation, the verdict-authority gate, routing, the correction loop, the log, `/status`,
the dashboard — sees only `orchestrator`, so the alias cannot open a second path past any check.
The broker host warns once per process through the helper. Emitted bytes change once, in the same
release that first versions the broker protocol (`wire-format-dtos`), so no versioned consumer ever
saw `supervisor`.
*Recorded alternative — keep `supervisor` as an opaque protocol constant:* zero migration risk and
no mixed-version hazard, but v1.0.0 would freeze `supervisor` in the broker wire forever (agent ids
on `/status`, the inbox path, a message type), against the goal of this change.

### D8 — Persisted values and output fields stay additive
The session wire type reads `mode` `"supervisor"` or `"coordinated"` and keeps writing
`"supervisor"` until 1.0.0, so a binary older than this release, still installed beside it, never
meets a value it cannot parse and skips the session file (which could then be overwritten). MCP
`SessionSnapshot` gains `session_mode` (`"bare"` | `"coordinated"`) and keeps `mode` with its old
values, registered as a deprecated output field.

### D9 — Doctor's group becomes "Coordination"
`uniform-json-output` gives doctor checks stable ids in this release, so they are born
`coordination.mode`, `coordination.gate-<name>`, `coordination.sweep-sh`; no id ever says
`supervisor`. `group` / `name` are display text, as that change states.

### D10 — Specs: reading rule now, textual sweep at 1.0.0
Rewriting every requirement that names an old spelling (65 mentions in `cli-parsing` alone, most
capabilities also rewritten by v0.17.0–v0.20.0 changes) would collide with every pending delta.
This change modifies only requirements whose text would contradict the new behavior, and adds
reading rules (as `orchestrator-skill-export` did for the skill): requirements naming an old flag,
key, or root-identity spelling are read through the mapping. `remove-supervisor-vocabulary` performs
the verbatim sweep and the `supervisor-*` capability rename when nothing else is in flight.

### D11 — Docs: retitle now, rename files at 1.0.0
`user-guide/supervisor.md` keeps its path (later changes' tasks name it) but becomes "Coordinated
mode"; 1.0.0 renames it with an mdBook redirect.

### D12 — Script fallback outlives the deprecation
`sweep.sh` reads `[orchestrator.gates] test_command` and the `[drive]` windows first, then the
`[supervisor]` keys, through `tomllib` and the regex fallback (section headers `[orchestrator.gates]`,
`[drive]`, `[supervisor]`). The fallback is the shell mirror of the v1→v2 step, so it stays as long
as version-1 files load. Heredoc bodies stay apostrophe-free; `bash -n` after each edit.

### D13 — The 0.17.0 deprecated auto-approval tables are dropped, not renamed
v0.17.0 `classifier-retirement` left `[supervisor.auto_approve]` and
`[supervisor.common_dev_allowlist]` parsed with no effect, with one stderr line per file per
process. Renaming them into `[workers]` would give dead keys a current-format spelling that v1.0.0
would then freeze. So the v1→v2 step drops them: the mapping marks both "dropped", the
configuration reference's migration table reads "dropped — deprecated 0.17.0, no effect", and the
version-2 wire types have no field for them (the version-1 parse-only structs stay, for reading).
Their registry entries are configuration keys with since `0.17.0`, the no-effect marker instead of
a replacement, and no removal release, because version-1 files are read through migration.
v0.17.0's per-file warning (`deprecated_table_warning` and its once-per-file set) is re-routed
through `deprecation::warn` with the file as context, so one helper prints every deprecation; the
line still names every declared table in one line. The MODIFIED *Deprecated auto-approval tables
are parsed but have no effect* replaces v0.17.0's round-trip clause: a save writes only version 2,
so `add-cli` / `remove-cli` drop the tables from the global file and name them in the rewrite
notice, and no other file is rewritten.
*Alternative:* carry them as `[workers.auto_approve]` / `[workers.common_dev_allowlist]` (the
earlier draft). Rejected: they have no effect, and a version-2 spelling would outlive v1.0.0.

## Risks / Trade-offs

- [A session started by an older binary is reattached by this one, or the reverse] → the broker
  accepts both identity spellings; the session file keeps the old `mode` spelling until 1.0.0. An
  older broker rejects `agent_id = "orchestrator"` (its shape rule has no `-` or `/` in it):
  documented — restart the session after upgrading, as for v0.18.0.
- [Stale exported orchestrator skill copies or user scripts still say `supervisor`] → inbound alias
  until 1.0.0; changelog and configuration migration table.
- [Warning noise: every command loading a version-1 config warns] → one line per key family per
  file per process, stderr only; `git paw config check` lists exact renames; `--strict` lets CI
  enforce the migration.
- [Merge-unit rule surprises a new-layout user splitting tables across global and repo files] →
  documented in the configuration reference; it is today's `[supervisor]` behavior.
- [Reading rules leave spec text naming old spellings until 1.0.0] → bounded; the sweep is a
  `remove-supervisor-vocabulary` task with a no-mention guard.

## Migration Plan

Seam-first, `just check` green at each step:
1. `src/deprecation.rs` with an empty registry (no behavior change).
2. One root-identity constant and `is_root_identity`, still `"supervisor"` (no behavior change).
3. Config version 2 (step, wire tables, mapper, merge unit, errors, init, template); registry config
   entries.
4. Flags and the raw-argument scan.
5. Root identity flip with wire normalization; scripts; dashboard; skills.
6. Session mode reading, MCP `session_mode`, doctor group, `config check` findings, skill-name warning.
7. Prose, help, docs.

Rollback: revert. A file the new binary wrote with `version = 2` and new tables is read by an older
binary as unknown keys (ignored) — that is the downgrade cost `config-schema-versioning` accepts.

## Open Questions

1. `default_mode` (recommended) vs `[drive] enabled` / `[orchestrator] enabled` for the old
   `enabled` key.
2. The D1 refinements and the two leaf changes — confirm or revert to the approved table.
3. Wire identity: rename with normalization (recommended) vs opaque constant.
4. Whether `status --json` `tmux_running` (`uniform-json-output`) is registered now, and with what
   removal (it decides whether anything is scheduled for 2.0.0).

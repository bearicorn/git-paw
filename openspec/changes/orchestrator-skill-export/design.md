## Context

The bundled `supervisor.md` (1,909 lines in v0.17.0) is the procedure for the agent supervising a session:
bootstrap, introspection, sweeps, verification (five gates), spec audit, merge orchestration, stuck
detection, stream-timeout recovery, qualitative learnings, `/agents` + `/tell`, and — since v0.17.0
`classifier-retirement` — the permission ladder (the CLI decides, prompts are relayed to the human,
`sweep.sh approve` only on the human's explicit instruction) and a per-CLI setup section pointing at
the bundled `cli-setup` skill (`cli-setup-consent`).
Until now it reached its reader by being rendered into the repo-root sidecar that the supervisor pane's
framing prompt pointed at. After pane removal, the reader is the operator's own agent session — Claude
Code, Codex, whatever they drive with — running at the repo root, possibly remote-controlled. It needs
the procedure as a skill it can load, plus a small per-session context (which session, which broker,
which helpers), plus a way to discover both.

What v0.17.0 already ships in that content: the pane mapping is roster-based (`supervisor.md:243-275`,
DS-2), and verdicts go through `sweep.sh verified` / `feedback-gate` / `conflict-ack`
(`:162/:182/:1185`). What it still
carries: 20 raw `curl` lines (PP-31) — reads of `/status` and `/messages/supervisor`, `/publish` posts of
`agent.answer`, `agent.question`, `agent.intent` and `agent.advanced-main`, and a stall message telling a
worker to curl `agent.blocked`; 16 `{{GIT_PAW_BROKER_URL}}` and 13 `paw-{{PROJECT_NAME}}` session
references; and an unconditional "Ask the human before merging" rule (`:1529`, PP-25).

Constraints: `export-agnosticism` (no stack tokens, no vendor paths, guard shipped with any new export
surface); `skill-standardized` already defines the agentskills.io layout git-paw accepts for user
skills; `skills::resolve("supervisor")` is public API; in production it passes no config directory
(`skills.rs:181`), so the only override location it reads is `.agents/skills/<name>/SKILL.md` found from
the current directory upward (v0.20 `skill-override-resolution` restores a user-level location with one
explicit order); the loader accepts a `SKILL.md` without frontmatter and its entry point
(`load_skill_from_directory`) is private; `mcp-server`'s
`get_skill` renders any named skill; `core-lang-agnostic` requires agent-facing prose to live in bundled
assets; `boot-agents-md` owns marker-delimited injection.

## Goals / Non-Goals

**Goals:**
- One procedure, one asset, two names (`orchestrator`, alias `supervisor`), zero lost content.
- Durable export in the standardized layout, rendered with the consumer's gate commands, refreshed by
  `start`.
- A thin per-session context file and a permanent, agnostic discovery pointer.
- Workflow documentation for driving from outside; the quick start replaced.
- Guards for the new export surfaces in the same change.
- Export the `cli-setup` skill beside the orchestrator skill, as `cli-setup-consent` expects (D11).

**Non-Goals:**
- Rewriting the skill's substance (gates, audit, merge doctrine) — content conversion is pane-phrasing
  only.
- Gate-auditor skills — v0.20 ships them as `references/gates/*.md` inside the orchestrator skill
  package (skills-packaging); here the five-gate procedure stays inside the single skill.
- Installing the skill into any vendor's skill directory (`.claude/skills/`, …) — consumers copy or
  symlink; git-paw stays agnostic.
- Headless escalation (`orchestrator-escalation-handoff`) — it consumes `SKILL.md` as-is.

## Decisions

### D1 — Rename with an alias, not a break

`orchestrator` is the canonical name; `supervisor` resolves to the same template so scripts, MCP calls
and overrides keep working. An override directory `.agents/skills/orchestrator/` wins over
`.agents/skills/supervisor/`; both apply to both names. That is the one override location production
`resolve` reads today; this change does not invent another (no flat `<name>.md` file), and v0.20's
resolution order inherits the same alias rule per level. The asset file is renamed and its frontmatter
becomes `name: orchestrator`; the `drop-serde-yaml` fixture
(`tests/fixtures/skill_frontmatter_expected.json`, which pins `name: supervisor` for
`assets/agent-skills/supervisor.md`) is updated with it. The `## Supervisor Skills` heading and internal
"supervisor" *role* wording may stay where they name the broker identity (`agent_id = "supervisor"` is
wire-frozen).

### D2 — Two artifacts: durable procedure vs per-session context

- `.git-paw/skills/orchestrator/SKILL.md` — standardized layout, rendered from the template with
  `{{TEST_COMMAND}}` etc. from config. Written by `init` (so it exists before any session) and
  re-rendered by `start` (so it never lags a config edit). Gitignored, binary-managed, like
  `.git-paw/scripts/`. **It carries no session values** (design call 6): `init` has no session, and a
  file that embeds one session's broker URL or tmux name is wrong for the next. So the template drops
  `{{GIT_PAW_BROKER_URL}}` (all 16) and `paw-{{PROJECT_NAME}}` (13; a session can carry a collision
  suffix): broker access is only through the bundled helpers and MCP tools, which discover the running
  session, and a raw `tmux` target names `<session>` from the sidecar. The render inputs are the
  `[supervisor]` gate commands (unset → `(not configured)`; `doc_tool_command` unset → empty),
  `{{SPEC_PATH_DOCTRINE}}` and the OpenSpec-only regions from the **configured** spec format (`[specs]`;
  the no-backend sentinel when absent) rather than from the specs a session launched, and
  `{{CHANGE_ID}}` left verbatim. Init and start therefore render the same bytes for the same config
  and binary.
- `.git-paw/AGENTS.local.md` (repo root) — the *session context*: session name, broker URL, helper
  paths, discovery-file path, governance section, drive-loop directive when unattended, and a pointer to
  `SKILL.md`. Today it is the whole rendered skill; shrinking it removes a 1.9k-line duplicate and gives
  the driving agent a short first read. It is the only file holding the session's values. `start`
  announces both paths through the launch use case's `Output` sink (`cmd-library-surface`, archived
  before this change), so the CLI prints them and MCP `start_session` receives them as data.

*Alternative rejected:* keep rendering the whole skill into the sidecar and skip `SKILL.md`. It works for
a local agent that is told to read the sidecar, but is invisible to skill-aware CLIs, unavailable before
`start`, and duplicates content.

### D3 — Discovery: a permanent pointer section in the root AGENTS.md, distinct markers

`AGENTS.md` is the cross-CLI convention the driving agent reads. `init` injects a short section
(`<!-- git-paw-orchestrator:start/end -->`) from a bundled asset: "when a git-paw session is active you
are its orchestrator; read `SKILL.md` and `.git-paw/AGENTS.local.md`". It is permanent configuration
(like `config.toml`), so it uses its own marker pair and is **not** stripped by `stop`/`purge` — the
retired per-session `<!-- git-paw:start/end -->` block and its `remove_session_boot_block` self-healing
are untouched. It carries no broker URL (that changes per session and lives in the sidecar) and no
secrets.

*Alternative rejected:* per-session injection into the tracked `AGENTS.md`. That dirties the consumer's
working tree during every session and was already retired for worktrees in v0.8 for the same reason.

### D4 — Standardized format, checked by a public validator

The exported dir is exactly what `skill-standardized` accepts from users, so a consumer can point
their CLI at it directly. Frontmatter: `name: orchestrator` — equal to the directory name, as the
agentskills.io format requires (the earlier `git-paw-orchestrator` in a directory `orchestrator/` broke
that rule) — and a one-sentence `description`. Body: the rendered skill. The loader cannot serve as the
validator: it accepts a `SKILL.md` with no frontmatter (no metadata, body kept) and its entry point is
private. So the change adds a public `skills::validate_skill_dir(path)` that requires the frontmatter,
non-empty `name` and `description`, and `name` = directory name, and reports the offending field; the
loader stays lenient for user skills.

### D5 — Guards in the same change (standing rule)

`tests/lang_agnostic_skill_audit.rs` gains the rendered `SKILL.md` (frontmatter included, every backend),
the exported `cli-setup` `SKILL.md` and the pointer asset. The vendor-path check (`.claude/`, product
names) applies outside `<!-- allowlist-prose -->` spans as today. A second guard asserts that no
rendering of the orchestrator skill contains the token `curl`, the broker URL or a session name (D2,
D12), and the export is checked by the public skill-directory validator (D4). The export-agnosticism
dev skill's inventory lists the three new surfaces (and the two new `sweep.sh` verbs ride on the
existing helper surface).

### D6 — Docs: replace the quick start, extend the user guide

`docs/src/quick-start-supervisor.md` → `docs/src/quick-start-orchestrator.md` with an mdBook
`[output.html.redirect]` entry so external links survive; `docs/src/SUMMARY.md` and README updated. The
user-guide chapters live under `docs/src/user-guide/` (`supervisor.md`, `skill-templates.md`, `mcp.md`,
`agents-md.md`, the new `remote-control.md`). `docs/src/user-guide/supervisor.md` keeps its
name (the CLI flag is still `--supervisor` and the chapter documents the drive loop) but gains
"Driving the session from your own agent" and retitles the input-handling section. The remote/mobile
point from the MILESTONE ("your driving session is already remote-controllable") is one paragraph here,
not a cycle of work.

### D7 — `/agents` and `/tell` survive as session directives

They were "slash commands" only in the sense that the operator typed them at the supervisor pane and the
skill told the model how to interpret them. Typed at the orchestrator's own session they work
identically; the skill's inventory/routing machinery (`sweep.sh` inventory, delivery modes, learnings
records) is unchanged, except that the inventory's `supervisor` row, when present, carries no pane index — the orchestrator has no pane
and pane 0 is the dashboard (`supervisor-pane-removal` D10).

### D8 — DS-2: map panes through the roster, by full path (done in v0.17)

v0.17.0 `supervisor-boot-wiring` already rewrote the section: the pane's `pane_current_path` is matched
against the session roster's `worktree_path` (`supervisor.md:243-275`), never a basename. Adding a new
requirement for that would duplicate the released *Supervisor skill SHALL teach `pane_current_path`
for pane→agent resolution* and contradict its "SHOULD cache the mapping once per session". So that
requirement is MODIFIED instead: the full-path roster match stays (the roster may also be read through
the helpers or MCP `get_session_status`), the session-long cache goes — an orchestrator outside the
session reads the roster when it needs a pane, and in any case after `add` / `remove` — and the tmux
target names the sidecar's session rather than `paw-{{PROJECT_NAME}}`.

### D9 — GP-33: teach the event surface

The monitoring sections move from "sweep panes on an interval and diff captures" to "block on
`wait_for_events` / `sweep.sh events` with the carried-forward cursor; capture a pane when an event or a
stalled wave says to look". Pane capture stays available for inspection; it is no longer the detector.
An escalated permission prompt arrives as an event like any other; the skill relays it to the human
and runs `sweep.sh approve` only on the human's explicit instruction (v0.17.0's doctrine, unchanged).

### D11 — Permission doctrine re-addressed, and `cli-setup` exported

v0.17.0 `classifier-retirement` already rewrote the permission doctrine; this change only re-addresses
it. Two v0.17.0 requirements still say the drive loop hands prompts to "the supervisor's pane" and
that the supervisor relays "in its own pane" — they are MODIFIED so escalations are read from the
broker and the human is addressed in the orchestrator's own session. The user-guide requirement is
MODIFIED from v0.17.0's text (its "Agent permissions" section stays).

`cli-setup-consent` ships the bundled `cli-setup` skill and expects this change to export it. `init`
and `start` write it to `.git-paw/skills/cli-setup/SKILL.md` beside the orchestrator skill, from
`skills::resolve("cli-setup")`, in the main checkout only: coding agents never run `git paw setup-cli`,
and `.git-paw/skills/` is gitignored so no worktree receives it. The orchestrator skill's per-CLI setup
section names the path as well as MCP `get_skill`. The skill already carries standardized
frontmatter, so it is written as-is; the standardized loader and the no-leak audit cover the exported
copy.

### D10 — Remote-control chapter, facts dated

The phone workflow is documentation (`docs/src/user-guide/remote-control.md`): the driving session is
already remote-controllable through its own CLI and keeps git-paw's MCP server attached. The security
posture chapter already lists "Remote control (documented workflow)" in its controls table
(`docs/src/user-guide/security-posture.md:63`) with nothing to link to; this chapter is that link. The supported-CLI table comes from the July research
recorded in the roadmap and is third-party fact — the chapter prints its check date and the
implementation re-verifies each row ("to verify" until then). The local-process caveat (~10-minute
outage ends the remote session) is kept verbatim in substance.

### D12 — No raw `curl` in the exported skill (PP-31)

`setup-cli` deliberately never grants `curl`, so under `auto` each of the 20 raw curls is a prompt, and
each bakes a session's broker URL into an exported file. Every one maps to a helper:

| Today | After |
|---|---|
| `curl …/status`, `…/messages/supervisor`, `…?since=` | `sweep.sh status`, `sweep.sh inbox`, `sweep.sh events` (or MCP reads) |
| `/publish` `agent.question` from `supervisor` | `broker.sh --agent supervisor question` |
| `/publish` `agent.intent` (main-side work) | `broker.sh --agent supervisor intent` |
| `/publish` `agent.answer` | new `sweep.sh answer <agent> <text> [re]` |
| `/publish` `agent.advanced-main` (+ shell variable assembly) | new `sweep.sh advanced-main <merged-branch> <summary>`; the helper resolves base, SHA and time |
| stall message telling a worker to curl `agent.blocked` | the worker's own `.git-paw/scripts/broker.sh blocked` |

`broker.sh` is granted wholesale and already accepts `--agent`, so the question and intent verbs need no
change. `answer` and `advanced-main` carry supervisor authority (an answer is authoritative guidance; a
main-advance event makes the drive loop rebase worker branches), so they go to `sweep.sh`, the
root-side helper, and are listed as never granted, like `verified` / `feedback-gate` / `conflict-ack`:
the grant target is shared with worker sessions. Prose mentions of `curl` are reworded too, so the guard
is the simplest possible check — no `curl` token in any rendering. The released requirements that
demanded curl examples (*Supervisor skill SHALL reference the bundled sweep helper*'s one-curl
exception, *Embedded supervisor skill*'s intent example, *Merge orchestration*'s mechanisms, and
`broker-watcher-and-state` *Supervisor publishes on merge to main*) are MODIFIED accordingly;
`broker-verdict-authority` already made the two verdict examples helper-only.

### D13 — Unattended merges follow the orchestration trigger (PP-25)

The skill's rules say "Ask the human before merging", while its merge-orchestration trigger says merge
once every expected agent is verified. Unattended, the first rule wins and the wave stalls at every merge
on an `agent.question`. The rule becomes conditional: with a drive loop running (the session context's
directive), merge as soon as the trigger holds, fast-forward only, and escalate only on a conflict
(non-fast-forward or a dependency cycle), an exhausted correction cycle, or a regression; attended runs
may keep confirming the merge order with the human. `orchestrator-escalation-handoff`'s merge-decision
one-shot carries the same rule.

### D14 — `init` reports what it changed (PP-30)

`run_init` prints `Updated …` for every bundled script it rewrites, even when the bytes are identical,
and its closing line ignores script writes, so a repo with a stale helper prints `Updated
.git-paw/scripts/sweep.sh` followed by "Already initialized. Nothing to do." Since this change adds
three more binary-managed writes to `init` (two skills, the pointer), it fixes the report for all of
them: `Updated` only when content changed, and the closing line counts every create or change.

## Risks / Trade-offs

- [Skill-content tests hard-code `assets/agent-skills/supervisor.md`] → mechanical re-point; the
  relocation requirement pins that their assertions pass unchanged.
- [A consumer's AGENTS.md is generated or lint-checked] → the section is marker-delimited and
  idempotent; documented how to keep it out of generated files (init prints what it wrote).
- [Two files to read] → the sidecar is short and points at `SKILL.md`; `start` prints both paths.
- [Typed delivery prompts the operator] → typed delivery (typing an answer into the asking agent's
  pane, paste-buffer recovery, `/tell` in send-keys mode) keeps v0.17's raw `tmux send-keys` in v0.18.
  Under `agent-hook-and-tmux-guards`' ask rule on `Bash(tmux send-keys *)`, which the orchestrator's
  root session reads too, each such keystroke now prompts the operator — accepted interim friction in
  an attended session until v0.19 `verified-agent-send` adds `sweep.sh tell`. The skill prefers broker
  delivery (`sweep.sh answer`, `/tell` feedback mode) so typed keystrokes are rare.
- [Rendered `SKILL.md` goes stale] → `start` re-renders; `doctor` may later check parity (not this
  change).
- [Export-agnosticism regression in frontmatter prose] → audited.

## Migration Plan

1. Rename asset + alias in `skills.rs`; `name: orchestrator`; re-point content tests and the
   frontmatter fixture; relocation requirement tests.
2. Content conversion: "your session", never-non-agent-surface, `/agents` + `/tell` phrasing, verdict
   examples via helper (already in v0.17), roster read when needed, no session values, no `curl` (new
   `sweep.sh answer` / `advanced-main`, never granted), the unattended merge rule.
3. `SKILL.md` renderer (session-independent inputs) + `init` install (orchestrator and `cli-setup`) +
   gitignore entry + the PP-30 report; the public skill-directory validator and its tests.
4. Pointer asset + `init` injection with its own marker pair; idempotency tests; stop/purge leave it.
5. Sidecar becomes session context; `start` re-renders both exported skills; prints the sidecar and
   orchestrator-skill paths.
6. Audit extension; dev-skill inventory update.
7. Docs: quick-start rename + redirect, `SUMMARY.md`, README, user guide sections, `skill-templates.md`,
   `mcp.md`, `agents-md.md`.

Rollback: revert; consumers keep a stale `SKILL.md` and pointer section, both inert.

## Open Questions

- Whether `init` should offer `--no-agents-md` to skip the pointer for consumers who manage AGENTS.md
  elsewhere. Recommendation: not now; the section is idempotent and removable, and init is rare.

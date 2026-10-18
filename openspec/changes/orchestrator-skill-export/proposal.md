## Why

`supervisor-skill-discipline` and the bundled `assets/agent-skills/supervisor.md` (1,909 lines in v0.17.0) are
literally skill content — procedure for the agent that supervises a session: the five gates, spec
audit, merge orchestration, stuck detection, stream-timeout recovery, qualitative learnings, `/agents`
and `/tell`, the permission ladder (relay prompts to the human). With the supervisor pane gone, that agent is the operator's
own session driving git-paw from outside tmux, and it needs the procedure as a **skill it can load**,
not as a pane's injected context. The roadmap's capability retirement map marks this capability
*convert*: it becomes the exported orchestrator skill set, plus workflow documentation that replaces
`quick-start-supervisor.md`.

Defects live in the content being converted:

- **DS-2** — done in v0.17.0 (`supervisor-boot-wiring`): the skill already maps panes to agents by the
  roster's `worktree_path` (`supervisor.md:243-275`), not by basename. One sentence of the released
  requirement remains wrong for an orchestrator outside the session: it "SHOULD cache the mapping once
  per session" — the roster changes with `add` / `remove`, so the skill reads it when needed.
- **GP-33 (skill side)** — the skill teaches sweeping by pane capture; the v0.17 rehearsal showed a
  sampled capture misses a prompt answered and replaced between samples. `orchestrator-read-surface`
  ships the event-driven read; the skill must teach it.
- **PP-25** — the skill's merge rules say "Ask the human before merging" (`supervisor.md:1529`),
  contradicting its own merge-orchestration trigger: unattended peer waves stalled at every merge on an
  `agent.question` until an operator pre-approved merges.
- **PP-31** — the skill drives the broker with raw `curl` (20 occurrences: reads, raw `/publish` posts,
  and an instruction telling stalled workers to curl). `setup-cli` deliberately never grants `curl`, so
  under `auto` every one is a prompt — and a session-specific broker URL is baked into the text.
- **PP-30 (ride-along, `init.rs` is touched here anyway)** — `git paw init` prints "Updated
  .git-paw/scripts/…" and then "Already initialized. Nothing to do."

Archive position 18. Export-agnosticism applies in full — new export surfaces ship with their guard.

## What Changes

- **The supervisor skill becomes the orchestrator skill.** `skills::resolve("orchestrator")` returns it;
  `skills::resolve("supervisor")` stays as an alias. Overrides at either name apply to both, in the one
  override location production `resolve` reads in this release — `.agents/skills/<name>/SKILL.md`
  (it passes no config directory; the user-level location is restored by v0.20
  `skill-override-resolution`). The asset moves to `assets/agent-skills/orchestrator.md`, frontmatter
  `name: orchestrator`. Pane-bound phrasing changes ("your session", never type into a non-agent
  surface, `/agents` and `/tell` as directives in the orchestrator's session).
- **Content fixes in the conversion:** the roster mapping is kept (DS-2, done in v0.17) and read when
  needed rather than cached for the session (MODIFIED of the released *Supervisor skill SHALL teach
  `pane_current_path` for pane→agent resolution*); monitoring by the broker event cursor
  (`wait_for_events` / `sweep.sh events`) and the wave-staleness signal instead of capture-diff polling
  (GP-33); verdicts only through `sweep.sh verified` / `feedback-gate` / `conflict-ack` (already so
  in v0.17, `supervisor.md:162/182/1185`; only the test remains). v0.17.0's permission doctrine (relay every prompt to
  the human; `sweep.sh approve` only on the human's explicit instruction) is kept and re-addressed:
  escalations are read from the broker, not handed to a pane, and the human is addressed in the
  orchestrator's own session.
- **Unattended merges follow the trigger (PP-25).** Under `--unattended` the merge rule is the
  orchestration trigger: once every expected agent is verified, merge fast-forward only without asking
  first; escalate only on a conflict (non-fast-forward or dependency cycle), an exhausted correction
  cycle or a regression. Attended runs may keep confirming the merge order with the human. This
  replaces the unconditional "Ask the human before merging" rule.
  `orchestrator-escalation-handoff`'s merge-decision one-shot follows the same rule.
- **No raw `curl` in the exported skill (PP-31), with a guard.** Reads go through `sweep.sh status` /
  `inbox` / `events` or the MCP read tools; publishes through `sweep.sh` verbs or
  `broker.sh --agent supervisor question|intent`; the stalled-worker instruction names the worker's own
  `broker.sh blocked`. The two publishes no helper covers today get new root-side `sweep.sh` verbs,
  `answer` and `advanced-main`, listed as never granted by `setup-cli` (they carry supervisor
  authority, like the verdict verbs). A guard test asserts no `curl` token in any rendering.
- **The exported skill carries no session values (design call 6).** No broker URL — the template drops
  all 16 `{{GIT_PAW_BROKER_URL}}` placeholders — and no tmux session name; broker access is only through
  the helpers and MCP (which discover the session), and the session values live only in the sidecar.
  `init`, which runs with no session, renders the gate commands from config, `{{SPEC_PATH_DOCTRINE}}`
  from the configured spec format (the no-backend sentinel when none), and leaves `{{CHANGE_ID}}`
  verbatim; `start` re-renders with the same inputs, so the bytes match.
- **`init` output (PP-30).** A binary-managed file is reported `Updated` only when its content changed,
  and the closing line agrees with what was written.
- **`cli-setup` joins the export.** `git paw init` (and `start`) also writes v0.17.0's bundled
  `cli-setup` skill to `<repo>/.git-paw/skills/cli-setup/SKILL.md`, beside the orchestrator skill, in
  the main checkout only. The orchestrator skill's per-CLI setup section names that path.
- **Two artifacts, one procedure.** `git paw init` renders the skill into the agentskills.io layout at
  `<repo>/.git-paw/skills/orchestrator/SKILL.md` (frontmatter + rendered body with the consumer's gate
  commands, no session values), gitignored, refreshed by `start`. The repo-root sidecar
  `.git-paw/AGENTS.local.md` becomes the short **session context** (session name, broker URL, helper
  paths, roster pointer, governance section, drive-loop directive when unattended) pointing at
  `SKILL.md`. `start` announces both paths through the launch command's `Output` sink
  (`cmd-library-surface`, archived before this change), so MCP `start_session` receives them as data.
- **Discovery.** `git paw init` injects an idempotent, marker-delimited pointer section
  (`<!-- git-paw-orchestrator:start/end -->`) into the consumer's root `AGENTS.md` from a bundled,
  project-agnostic asset; `stop` / `purge` leave it. MCP `get_skill({name: "orchestrator"})` serves the
  same content.
- **Workflow documentation.** `docs/src/quick-start-supervisor.md` is replaced by
  `docs/src/quick-start-orchestrator.md` (init → start → load the skill in your own agent → follow
  events, verify, merge via helpers or MCP → `--unattended` pairing) with a redirect;
  `docs/src/user-guide/supervisor.md` gains "Driving the session from your own agent"; a
  **remote-control workflow chapter** (`docs/src/user-guide/remote-control.md`) documents driving the
  session from a phone through the driving CLI's own remote control — no git-paw bridge — keeping the
  caveat that the local process must stay running (~10-minute outage limit) and a dated supported-CLI
  table. It also fills the dangling "Remote control (documented workflow)" row of the controls table in
  `docs/src/user-guide/security-posture.md:63`, which names the workflow without linking anywhere.
- **Guards.** `tests/lang_agnostic_skill_audit.rs` covers the rendered `SKILL.md` and the pointer asset;
  a no-`curl` guard over every rendering; a new public skill-directory validator checks the export (the
  loader itself accepts a `SKILL.md` without frontmatter, and its entry point is private).
- **Not in this change — gate-auditor skills.** The roadmap's v0.18 section mentions them, but they ship
  in v0.20 as `references/gates/*.md` inside the orchestrator skill *package* (skills-packaging); this
  change keeps the five-gate procedure inside the single skill and leaves the split to that cycle.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `skill-agent`: orchestrator skill = supervisor skill relocated (alias, no pane assumptions, no
  session values); reads the session through the event surface (escalated prompts are relayed to the
  human); the permission ladder reads unattended escalations from the broker, not a pane; resolution
  order gains the alias (on the override path production reads); interactive-input section addresses
  the operator's session; MODIFIED *Supervisor skill SHALL teach `pane_current_path`…* (roster read
  when needed, no session-long cache), *Supervisor skill SHALL reference the bundled sweep helper* (no
  `curl` at all), *Embedded supervisor skill* (main-side intent via `broker.sh`) and *Supervisor skill —
  Merge orchestration* (unattended merge rule, helper reads); the event-surface requirement also
  teaches re-attaching a drive loop after a heartbeat exit.
- `broker-watcher-and-state`: *Supervisor publishes on merge to main* uses `sweep.sh advanced-main`,
  not a curl example.
- `broker-agent-helper`: `sweep.sh answer` and `sweep.sh advanced-main` (never granted by
  `setup-cli`).
- `supervisor-skill-discipline`: never-own-pane rule becomes never-non-agent-surface; the
  escalation-first doctrine reads the session context and relays in the orchestrator's own session;
  `/agents` and `/tell` are directives in the orchestrator's session, and the `supervisor` row has no
  pane index.
- `supervisor-launch`: the sidecar is the session context pointing at `SKILL.md` (and the only home of
  the session's values); `start` refreshes both exported skills and announces the paths through the
  `Output` sink; quick-start requirements re-pointed at `quick-start-orchestrator.md`; the user
  guide gains the workflow section (on top of v0.17.0's agent-permissions section); a remote-control
  workflow chapter (`docs/src/user-guide/remote-control.md`) is added and linked from the
  security-posture controls table.
- `core-init`: init installs the standardized orchestrator skill (rendered with no session values),
  the `cli-setup` skill and the AGENTS.md pointer section; its report and closing line agree with what
  it wrote (PP-30).
- `skill-standardized`: the bundled orchestrator and `cli-setup` skills are exported in the
  standardized format, checked by a public skill-directory validator.
- `core-lang-agnostic`: the exported skills and the pointer asset are covered by the no-leak audit.

## Impact

- **Predecessors (implementation order):** archive position 18, after `orchestrator-read-surface` (3:
  `wait_for_events`, `sweep.sh events`, wave staleness), `supervisor-pane-removal` (6, which modified
  *Supervisor AGENTS.md from supervisor skill template* and *Quick Start Supervisor chapter is internally
  consistent* first — this change modifies both again), `agent-hook-and-tmux-guards` (8: `tmux
  send-keys` on `ask` in the shared `setup-cli` target file, so the skill's typed delivery prompts the
  operator until v0.19 `verified-agent-send` adds `sweep.sh tell`; see design, Risks), `drive-loop-reattach` (10:
  `start --unattended` re-attaches a drive loop to a live session, which the skill teaches),
  `cmd-library-surface` (15: the `Output` sink the
  start announcement goes through), `broker-verdict-authority` (16: verdict helpers; it MODIFIES the two
  *corrected … curl example* requirements to helper-only, which this change does not touch again) and
  `mcp-orchestrator-write-tools` (17: the tools the skill and docs describe). v0.17: `classifier-retirement`
  (this change MODIFIES its *Supervisor skill — permission decisions belong to the agent CLI and the
  human*, *Escalation-first, no blanket-approve when a drive loop is running* and
  *`docs/src/user-guide/supervisor.md` SHALL consolidate the supervisor surfaces*, building on its
  text), `cli-setup-consent` (the bundled `cli-setup` skill this change exports; its *Embedded
  cli-setup skill* is not modified), `drop-serde-yaml` (standardized-skill frontmatter parsed without
  YAML — the export must parse with it), `worker-guidance-read-tools` (ADDS a coordination-skill
  requirement to `skill-agent`; no shared header), `bundled-helper-discovery` (the helpers read the
  roster the skill now relies on).
- **Successors:** `orchestrator-escalation-handoff` (20) points its one-shot at `SKILL.md` and applies
  the same unattended merge rule to its merge-decision context; v0.20 packages the
  skill and adds `references/gates/*.md`; v0.20 `skill-package-layout` registers the `cli-setup` skill
  (root-only) and `skill-override-resolution` makes it resolve from the embedded copy only, which this
  export then follows.
- **Assets (export surfaces):** `assets/agent-skills/orchestrator.md` (renamed + converted: no `curl`,
  no broker URL or session name), new `assets/agents-md-orchestrator-section.md`, `assets/scripts/sweep.sh`
  (new `answer` and `advanced-main` verbs), and v0.17.0's `assets/agent-skills/cli-setup.md` exported to
  a new location (`.git-paw/skills/cli-setup/SKILL.md`).
- **Code:** `src/skills.rs` (name + alias, standardized render without session values, public
  skill-directory validator), `src/init.rs` (skill dirs for `orchestrator` and `cli-setup`, gitignore
  entry, AGENTS.md pointer, changed-only `Updated` reporting and an agreeing closing line),
  `src/agents.rs` (second marker pair), `src/commands/supervisor.rs` (sidecar = session context;
  re-render both exported skills; announce through the `Output` sink), `src/mcp/tools/project.rs`
  (`get_skill` alias), `src/cli_setup/facts.rs` (`answer`, `advanced-main` in `SWEEP_NEVER_GRANTED`),
  `tests/fixtures/skill_frontmatter_expected.json` (the `drop-serde-yaml` fixture pins
  `name: supervisor` for `assets/agent-skills/supervisor.md`).
- **Tests:** `src/skills.rs` and `tests/*_skill_content.rs` re-pointed; `lang_agnostic_skill_audit.rs`
  extended; no-`curl` / no-session-value guard over every rendering; `cli_init_*` for the skill dirs,
  gitignore, pointer and the PP-30 report; validator tests (both exports pass; no frontmatter and a
  name/directory mismatch fail); `sweep.sh` verb tests; `agent_skills_conform.rs` unchanged.
- **Docs:** `docs/src/quick-start-orchestrator.md` (rename + redirect), `docs/src/SUMMARY.md`,
  `docs/src/user-guide/remote-control.md` (new; linked from the controls table in
  `docs/src/user-guide/security-posture.md`), `docs/src/user-guide/supervisor.md`,
  `docs/src/user-guide/skill-templates.md`, `docs/src/user-guide/mcp.md`,
  `docs/src/user-guide/agents-md.md`, README Quick Start links; the export-agnosticism dev skill's
  inventory gains the three surfaces.
- **Enum-variant ripple:** none.
- **Security / safety:** verdict guidance defers to the credential-carrying helper; the pointer and
  the exported skill carry no URL or secret. The two new `sweep.sh` verbs publish with supervisor
  authority and are never granted by `setup-cli` (its target file reaches worker sessions too), so a
  worker cannot forge an answer or a main-advance event without a prompt. Dropping every raw `curl`
  removes the pressure to grant `curl`. The unattended merge rule merges fast-forward only after every
  gate is verified, and escalates conflicts, exhausted cycles and regressions instead of merging. No
  stack tokens, vendor paths or commit-format prose in exports.
- **Dependencies:** none.

## Rebase note (2026-10-19)

v0.17.0 pulled DS-2 forward as `supervisor-boot-wiring`: the bundled supervisor skill already resolves panes by matching `pane_current_path` against the session roster's `worktree_path`. This change keeps that roster-based mapping when it converts the skill; its DS-2 text is built on the archived v0.17.0 text.

## Rebase note (2026-10-20): classifier retirement

v0.17.0 `classifier-retirement` REMOVES *Supervisor skill — tiered permission model and safe-command policy*, rewrites the skill's permission doctrine (relay every prompt to the human; `sweep.sh approve` only on the human's explicit instruction) and replaces the user guide's "Common dev-command allowlist" section with "Agent permissions". This change was re-scoped onto it: the relocation scenario no longer cites the tiered model; the event-surface requirement no longer says the drive loop approves classifier-safe prompts; the MODIFIED user-guide requirement starts from v0.17.0's text; and two v0.17.0 requirements that addressed the supervisor's pane (*Supervisor skill — permission decisions belong to the agent CLI and the human*, *Escalation-first, no blanket-approve when a drive loop is running*) are MODIFIED to the orchestrator outside the session. v0.17.0 `cli-setup-consent` expects this change to export its `cli-setup` skill; it does.

## Rebase note (2026-10-25): audit against released v0.17.0

- DS-2 is done in v0.17.0 (roster-based mapping, `supervisor.md:243-275`): the ADDED roster requirement
  became a MODIFIED of the released *Supervisor skill SHALL teach `pane_current_path` for pane→agent
  resolution*, replacing its "SHOULD cache the mapping once per session" with reading the roster when
  needed. Task 2.5 (verdicts via helper verbs) is done except its test.
- The alias/override scenarios now use the path production `skills::resolve` actually reads
  (`.agents/skills/<name>/SKILL.md`; it passes no config directory, `skills.rs:181`) — no flat-file
  override. "Missing frontmatter would be rejected" is backed by a new public skill-directory validator
  (the loader accepts a frontmatter-less `SKILL.md` and its entry point is private).
- Design call 6: frontmatter `name: orchestrator` (= directory); the exported `SKILL.md` carries no
  session values (no `{{GIT_PAW_BROKER_URL}}`, no session name); what `init` renders with no session
  is specified, including `{{SPEC_PATH_DOCTRINE}}`. The `drop-serde-yaml` fixture update joins task 1.2.
- Absorbed PP-25 (unattended merge follows the trigger), PP-31 (no raw `curl`, new `sweep.sh answer` /
  `advanced-main` verbs, guard test; MODIFIED *reference the bundled sweep helper*, *Embedded supervisor
  skill*, *Merge orchestration*, and `broker-watcher-and-state` *Supervisor publishes on merge to main*,
  which all required curl examples) and PP-30 (init report).
- Doc paths corrected to `docs/src/user-guide/`; the remote-control chapter fills the dangling
  reference at `security-posture.md:63`; the start announcement goes through `cmd-library-surface`'s
  `Output` sink (archived at 15, before this change). Ordering prose updated to archive position 18.
- `sweep.sh conflict-ack` joins `verified` / `feedback-gate` wherever the verdict verbs are listed
  (task 2.5, requirements, design): it publishes `agent.feedback` from `supervisor`
  (`assets/scripts/sweep.sh:826`, taught at `supervisor.md:1185`), and `broker-verdict-authority` makes
  all three credential-carrying root verbs.
- Pane-less orchestrator: the `/agents` `supervisor` row carries no pane index (`supervisor-pane-removal`
  D10); the event-surface requirement teaches re-running `git paw start --unattended` after a
  heartbeat exit to re-attach a drive loop (`drive-loop-reattach`, 10).
- Typed delivery via `sweep.sh tell` is deferred to v0.19 `verified-agent-send`: answer typing,
  paste-buffer recovery and `/tell` in send-keys mode keep v0.17's raw `tmux send-keys` here, which
  `agent-hook-and-tmux-guards` (8)'s ask rule now makes prompt the operator (design, Risks).

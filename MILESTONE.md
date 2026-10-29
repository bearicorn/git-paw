# git-paw — Milestone Roadmap

v0.1.0 through v0.18.0 shipped; v0.19.0 is next.

---

## Release history at a glance

| Version | Date | Theme | One-line scope |
|---------|------|-------|----------------|
| v0.1.0 | — | Foundation | Core `git paw` CLI + worktree-based agent sessions |
| v0.2.0 | — | Specs & Sessions | `init`, `--from-specs`, per-worktree AGENTS.md, logging/replay |
| v0.3.0 | — | Broker & Dashboard | HTTP broker (opt-in), coordination skill, read-only TUI |
| v0.4.0 | — | Supervisor Mode | Supervisor agent, spec audit, hook injection, merge verification |
| v0.5.0 | — | Forward Coordination | supervisor-as-pane, `agent.intent` + conflict detection, Spec Kit, learnings, `[governance]` |
| v0.6.0 | 2026-06-17 | Supervisor/Coordination Hardening | `git paw add`/`remove`, `/tell`, opsx role-gating, per-commit verify, broker roster hygiene, dashboard broker-log |
| v0.7.0 | 2026-06-23 | Read-only MCP Server | `git paw mcp` (7 tool categories incl. docs + source), broker `/watch`, learnings no-telemetry disclosure, server-identity + `[mcp]` config |
| v0.8.0 | 2026-06-26 | Internal worktrees + frictionless dogfood | embedded `.git-paw/worktrees/` (configurable, child-default), broker.sh boot helper (F1), prefix dev-allowlist + de-opinionated preset (F2-lite), gitignored AGENTS.md sidecar (F10), session-orchestration robustness, broker-log height (F3), commit-discipline skill (F7), drop dead Summary column (F4) |
| v0.9.0 | 2026-07-06 | Unattended operation | `--unattended` drive loop, broker-mediated approvals (drift F), battle-tested classifier, stuck/bloat detection — the full approval architecture deferred from v0.8.0; + `selftest` harness (F8), verify-at-tip (F9), remove-dirty-check fix, qualitative supervisor learnings |
| v0.10.0 | 2026-07-13 | Agent-friendly documentation + dogfood hardening | machine-readable docs site + docs-fetch skill + doc-consistency pass + carried fixes (dashboard CPU-leak, remove-dirty-check) + **dogfood-built hardening: worktree-helper-provisioning, dashboard-orphan-exit-hardening** + test/nit hardening |
| v0.11.0 | 2026-07-17 | Unattended hardening | supervisor-native auto-mode + agent-memory-isolation + same-region conflict detection (W15-22a) + agent.answer variant (W15-11) + classifier stack de-opinionation + approve-send-gate hardening + agent-worktree dev-allowlist seeding + docs/CI closeout |
| v0.12.0 | 2026-07-25 | Superpowers spec format + unattended-hardening riders | `superpowers` as a fourth `[specs] type` / `--specs-format` (scan obra/superpowers plan documents into spec-driven sessions) + 7 fixes from the v0.11.0 build dogfood (broker delivery-cursor bug, per-pane approval serialization, pane→agent re-resolution, approve-on-detect, send-gate parity, question re-publish suppression, variant-ripple checklist; the 8th, port-rebind flake, was already resolved in v0.10.0) |
| v0.13.0 | 2026-08-05 | `git paw doctor` + pre-freeze quality hardening | `git paw doctor` preflight diagnostics (+ `--live` smoke arm; `selftest` folded under it) + path-injection hardening (tmux session-name sanitization + shell-quoted pipe-pane/dashboard paths via domain newtypes) + wire-api-freeze relax (serde defaults + declared MSRV) + broker runtime hardening (spawn_blocking offload, poison-lock recovery) + code-analysis refactor (config/tmux/main.rs module splits + CommandRunner seam) + interactive CLI-contract PTY e2e + test-suite consolidation + spec traceability audit + consolidation + Gemini→Antigravity CLI transition + dev agent-skills. Release patched via MSRV 1.97→1.96 on the tag so cargo-dist could build the darwin binaries |
| v0.14.0 | 2026-08-11 | Autonomous orchestrator + pre-freeze architecture hardening | supervisor becomes the autonomous orchestrator (unattended pump → supervisor brain, human = exception handler) + `[supervisor.correction]` self-healing loop + tiered-model workflow guide + classifier single-source (`git paw __classify`) + pre-freeze architecture hardening via the `code-architecture` capability (interactive / dashboard-MVU / git-`CommandRunner`-seam / worker-lifecycle module refactors + shared `_paw_common.sh`) and the enforced **per-pane approval claim** (behavior change) + riders (50/50 layout, dashboard status-activity, unattended-loop stall fixes, init config-backfill) |
| v0.15.0 | 2026-09-02 | Per-worktree runtime isolation + security posture | opt-in `[worktree.env]` verbatim copy + `[worktree.ports]` index-derived `.env.local` allocation + `[worktree.hooks]` consumer `on_create`/`on_remove` (the DB-branch/teardown escape hatch) — parallel sessions independent at *runtime*, not just source (`session-runtime-isolation`); + the **docs-only** FS-scoped sandbox workflow (`sandbox-exec`/`bwrap` via the custom-CLI seam, no runtime engine) with a consolidated security disclaimer + a read-only `git paw doctor` sandbox-availability check (`core-security-posture`) |
| v0.16.0 | 2026-09-14 | Unattended boot & friction-killers | make a fresh consumer's `--unattended` run reach its first tool call (peer-4-poker dogfood proved it can't): **boot trio** GP-01 (allowlist → `permissions.allow`), GP-02 (bundled-helper classifier + `normalize_command` leading-`VAR=`/`env`/`nohup`), GP-03 (`--permission-mode auto` + launch liveness), GP-04 (`.git-paw/` conflict FP + `.git/`-write safety); + **unattended-hardening** GP-05 (exit-summary outcome), GP-06 (purge/stop orphan drive-loop), orchestrator-in-`session.json`; + **quick CLI DX** shell completions, `git paw attach`, strike phantom `resume`. Carved ahead of the remote/mobile victory-lap so the flagship boots on a fresh repo |
| v0.17.0 | 2026-10-25 | Drive-loop reliability + `--resume`; classifier retired | the post-0.16.0 data-safety chain fixed (exit summary from git refs + `cli-unavailable` non-success exit; purge/remove never delete unmerged branches) + launch correctness (boot and task prompts submitted on every path, readiness gate, helpers provisioned, governance wired) + **`--resume`** (worker CLI session ids captured and resumed); **BREAKING:** the auto-approve classifier is retired (git-paw approves no prompt; the drive loop detects and escalates), `git paw setup-cli` seeds allow rules only on explicit consent, panes launch the configured `command` (`cursor-agent` detected), the broker listener checks `Host`/`Origin`, `stop` lost `--force` |
| v0.18.0 | 2026-10-29 | **Orchestrator relocation** | the orchestrator moved out of the session: the **supervisor pane** is gone, your own driving session takes its place with the exported orchestrator skill, and the **drive loop** stays (**BREAKING**); the broker host runs in its own process in a `broker` window, with the dashboard as a thin client; **verdict authority** in the broker (only the root session's credential may publish verdicts; **BREAKING**); MCP **write tools** (session lifecycle, verdicts) and consented spec authoring confined to the change directory; the drive loop hands judgment calls to a headless orchestrator one-shot; the unattended heartbeat measures progress, not wall-clock time; plus a cursor-based event read surface, `cmd_*` as a library, a terminal backend port and a pure drive-loop core |
| v0.19.0 | *planned* | Per-CLI foundation | one resolved record per CLI (`[clis.<name>].family` / `args`, the ambiguous-shim refusal) threaded through every consumer, per-worker sandbox mode, usage and cost attribution with a budget cap, verified `sweep.sh tell`, and the spec-format Strategy. Hook providers, trust pre-acceptance and allowlist seeding were dropped; consented seeding shipped in v0.17.0 as `setup-cli` |
| v0.20.0 | *planned* | Skills packaging + agent profiles | skills become agentskills.io packages (dirs: `SKILL.md` + `scripts/`/`references/`/`assets/`, consolidating today's split `agent-skills/*.md` + `scripts/*.sh`; injected `AGENTS.md` references the packages) + per-CLI skill templates; on that foundation, **agent profiles** in config (`[profiles.<name>]`: a CLI pinned to a model + a role skill): worker profiles launch as panes, delegate profiles run headless one-shot for the orchestrator (explorer + gate auditors). Replaces the vendor subagent-persona plan |
| v0.21.0 | *planned* | Pre-freeze contract + DX hardening | lock the machine/config contract before the freeze: config schema versioning + migration, the supervisor vocabulary split (config version 2 with `[orchestrator]`/`[workers]`/`[drive]`, `--coordinated`/`--bare`, the `orchestrator` wire identity; old names warn through the new `core-deprecation` registry), one versioned `--json` contract (`add`, `remove` and `list-clis` gain `--json`), `git paw finish <branch>` (merge an agent's work into a target branch via `--into`, then remove the worktree — the pure-manual merge capstone), GP-09 merge-queue, session-state reconciliation + state-file locking (carried from v0.13); plus DX polish (**BREAKING (view):** the dashboard becomes an opt-in usage view and the roster moves into `status`; configurable layout, `config check`, terminal robustness, README landing-page slim-down) |
| v0.22.0 | *planned* | Session recording & replay, rethought | `git paw replay` can't recreate a session — `pipe-pane` raw capture of a full-screen agent TUI (alt-screen + cursor-addressed redraws + animated spinners/thinking counters) is unreadable ANSI-stripped and not a faithful recording (confirmed unusable, peer-4-poker 2026-08-31). Rethink the record: a semantic timeline from broker events, commits and the CLI's own JSONL transcript becomes the default (**BREAKING**: `--color` and the ANSI-stripped view go), with an optional asciinema v2 recording played through `asciinema play` (no built-in VT player); capture on every path that creates an agent (`add` and recovery included; the `--from-specs`-only gap was fixed in 02649dc), one record per run. Freeze-adjacent — replay is a shipped command, so fix it pre-1.0 or deprecate it at the freeze |
| v0.23.0 | *planned* | Per-CLI support audit | verify every detected CLI end to end against the worker lifecycle (detection, launch, readiness, boot block, prompt shapes, approvals, send/submit, auto-mode/sandbox/hooks, resume, headless one-shot, exit detection, cost); a per-CLI conformance suite (recorded prompt-shape fixtures in CI + an opt-in live smoke); a published support matrix with tiers (first-class / supported / launch-only) that v1.0.0 freezes |
| v0.24.0 | *planned* | Pre-freeze hardening | feature freeze: fix the audit findings, de-flake the suite, a back-compat matrix for v0.16+ configs/sessions, an error/exit-code pass, a five-gate audit of the release candidate, and a full external-orchestrator dogfood of the v0.18.0 relocation before the freeze |
| v1.0.0 | *planned* | Stable CLI freeze | contract freeze (commands/flags/config/exit-codes, semver) + a deprecation policy + **BREAKING:** `--from-specs` removal, the old supervisor vocabulary removed (`--supervisor`/`--no-supervisor`, the `supervisor` wire identity and skill name), and layer isolation (each layer installs only its own surface; a coordinated session always gets the broker); deliberately thin — locks a surface proven across 0.13–0.24 |
| v1.1.0 | *planned* | Multi-repo registry | first post-freeze feature: a registry of git-paw repos (`<data dir>/git-paw/repos.json`) so one MCP server, opted in with `git paw mcp --all-repos`, serves many repos (`list_repos()` + a `repo` param on every tool) and resolves per-repo CLI preferences — the single-node precursor that v1.4.0 (cross-repo coordination) builds on |
| v1.2.0 | *planned* | A2A interface | expose sessions over A2A 1.0 (stable 2026-03-12) as an opt-in external interface on top of the broker — one session Agent Card, workers as A2A tasks, loopback + root credential only; the broker bus and helpers stay (A2A can't replace a coordination bus); builds on v0.17.0's `Host`/`Origin` guard against DNS rebinding |

---

## v0.17.0 — Drive-loop reliability, `--resume`, classifier retired (shipped 2026-10-25)

> **Status 2026-10-25:** released. Every change archived; `v0.17.0` tagged on `main`
> (`337aa1e chore: prepare v0.17.0 release`), curated to one commit per change (authored
> 2026-09-15 to 2026-10-17, merged 2026-10-25). v0.18.0 has since shipped (2026-10-29), and
> the later spec branches (v0.19.0–v1.6.0) are now restacked on v0.18.0. Full findings record:
> `.git-paw/v0.17.0-dogfood-findings.md`; dated release notes: `CHANGELOG.md`.

Planned as a small, fast reliability release for the 0.16.0 post-release data-loss chain plus
`--resume`. Self-dogfooding it (an external orchestrator standing in for the v0.18.0 relocation)
kept surfacing defects, and the standing rule "fix dogfood bugs in the release" grew it into
the release that retired the command classifier.

**What shipped:**
- **Data safety.** The exit summary is derived from git refs and an unavailable CLI is an
  explicit non-success outcome (GP-21, with the marker gaps GP-36/GP-50). `purge` and `remove`
  never delete a branch with unmerged commits (GP-20). `stop` never asks and lost `--force`
  (BREAKING).
- **Launch correctness.** Spec-driven `start` submits the boot and task prompts, and every
  launch path waits for readiness (GP-22/23/24, DS-5, GP-71). Bundled helpers are provisioned
  and discoverable on every path (GP-26/28/42/43). Governance docs and launch specs reach the
  launch (DS-1/2/3). Exited panes report as stopped and the pickers honour `default_cli`
  (DS-8/9).
- **Drive loop.** The live-prompt gate is anchored on the modal block (GP-31/48/53), and nudges
  are verified by input-box state (GP-19).
- **Permissions (BREAKING).** `classifier-retirement`: git-paw approves no prompt. The drive
  loop detects each live prompt, escalates it without blocking the wave, and logs it for a human
  decision (absorbing DS-4). `__classify` is gone, and `[supervisor.auto_approve]` /
  `[supervisor.common_dev_allowlist]` still parse but do nothing. `git paw setup-cli claude
  --allowlist` writes least-privilege helper grants only on explicit consent, records them, and
  can undo them (pulled from v0.19.0).
- **Identity and broker (BREAKING).** Panes launch a CLI's configured `command`, `cursor-agent`
  is detected instead of the Cursor editor, and `[clis.<name>] mcp` opts a variant into the MCP
  boot form (GP-25/29, DS-6; pulled from v0.19.0). The broker listener checks `Host` and
  `Origin` against DNS rebinding (pulled from the A2A cycle) and refuses a port held by another
  repository's broker (GP-41).
- **Key feature.** `--resume`: worker CLI session ids are captured and resumed on recovery.
- **Smaller fixes.** Conflict-escalation decisions persist across commits (GP-34); learning ids
  use a stable FNV-1a hash; MCP accepts the Superpowers backend (GP-44); the coordination skill
  teaches read-tool and scratch-path discipline (GP-37/32); shipped boot blocks, prompts and
  messages corrected; `serde_yaml` replaced by a dependency-free parser (GP-39); no
  `unwrap`/`expect` in non-test code, lint-enforced, and the binary's own tests now run (EXP-1,
  GP-64); tests isolated from the real session directory (GP-27); the sandbox guide notes setuid
  `ps` (GP-14); the target-architecture guide.

**Dropped as superseded by the classifier retirement:** `classifier-compound-segments`,
`broad-grant-scope-safety` (GP-38/51/56), `approval-send-identity-check` and
`approval-pane-map-refresh` (GP-45). GP-35 is moot; GP-16 was already fixed; GP-03/03b/04 were
re-verified as shipped in v0.16.0 (03b is its launch-dialog handling).

**Carried forward, not yet specced unless noted:**
- **v0.18.0:**
  - `add`/`remove` on bare sessions (GP-49/69; specced in `supervisor-pane-removal`).
  - Prompt-to-prompt visibility for a turn-based orchestrator (GP-33; specced in
    `orchestrator-read-surface`).
  - Launch prompts: submit a slash-command entry point as its own composer line (GP-54), and name
    the worktree root (GP-55).
  - Worker guidance and `setup-cli` entries against agents driving tmux or bypassing hooks
    (GP-58/62: `ask` or deny rules for `git -c core.hooksPath` and `--no-verify`).
  - The boot-block section-order wording (GP-63).
  - The eight weakly covered `classifier-retirement` scenarios (GP-68 test debt).
  - Forks as a per-CLI capability (GP-67), also input to the v0.23.0 audit.
- **v0.24.0:** the wall-clock load flakes GP-61/65/66/70 (readiness waits instead of fixed
  deadlines) join `test-suite-determinism`.
- **Process:**
  - Struct-field ripple is the same hazard as enum-variant ripple; AGENTS.md §7 should say so
    (GP-40).
  - An agent's self-reported suite total is not evidence; only the merged-tree Gate 2 counts
    (GP-60), and Gate 3/5 needs a reader independent of the implementer.
  - Dry-run `openspec archive` on a clean export, because `--strict` misses a renamed MODIFIED
    header.
  - Decline a prompt, wait until the pane is idle, then type (GP-67 addenda).

**Skill or code?** Of the seventeen planning-time findings, two were skill-shaped; everything
else was an actuator defect. That is the same ratio the v0.18.0 analysis found. git-paw's value
is in the actuator, not the prose it ships.

---

## v0.18.0 — Orchestrator relocation (shipped 2026-10-29)

> **Status 2026-10-29:** released. `v0.18.0` tagged on `main` (`096c12bb chore: prepare v0.18.0
> release`) and published to crates.io, Homebrew and the GitHub release; all 21 changes archived.
> Findings record: `.git-paw/v0.18.0-dogfood-findings.md`; dated release notes: `CHANGELOG.md`.
> The plan below is kept as written until this section is condensed.
>
> **Specs committed 2026-10-19 on `feat/v0.18.0-specs`** (14 changes including the later `mcp-spec-authoring-tools`, all `--strict` valid; the
> seven 2026-09-14 drafts were refreshed against this section and the architecture direction).
> Seven more joined during the cycle, for 21 in all: `agent-hook-and-tmux-guards`,
> `agent-id-slug-safety`, `setup-cli-adopted-entries`, `launch-prompt-entry-point`,
> `drive-loop-reattach`, `unattended-heartbeat-progress` (moved from v0.24.0) and
> `broker-window-pane-scope`.
> v0.17.0 has since pulled DS-1/2/3 (`supervisor-boot-wiring`) forward, and its
> `classifier-retirement` absorbed DS-4 and made GP-45 moot. The affected v0.18 changes carried a
> "Rebase note" and were rebased over the archived text.
> **Re-scoped 2026-10-20 for the classifier retirement:**
> - The drive loop's core decision is now detect → dedupe → record → escalate, with no approval
>   decision (`drive-loop-functional-core`).
> - The broker host runs no approver.
> - GP-46 becomes *Keystrokes reach only coding-agent surfaces*.
> - The headless escalation one-shot never gets permission prompts and may not run
>   `sweep.sh approve`.
> - Stall detection counts only a branch tip moving or a phase advancing.
> - `sweep.sh events` joins setup-cli's grant table as read-only.
> - The exported skills include `cli-setup`.
> In implementation order: `supervisor-role-typing` → `supervisor-pane-removal` (GP-46, DS-1)
> → `pane-less-session-parity` (GP-49, DS-5, the drive loop re-reads its roster) →
> `terminal-backend-seam` (GP-45) → `broker-host-separation` (DS-3) →
> `orchestrator-read-surface` (GP-33, wave staleness) → `cmd-library-surface` →
> `broker-verdict-authority` → `mcp-orchestrator-write-tools` → `drive-loop-functional-core` →
> `orchestrator-skill-export` (DS-2) → `orchestrator-escalation-handoff` (DS-4) →
> `doctor-spec-workflow-check` (GP-30). Port vocabulary: `TerminalBackend`, `SurfaceId`,
> `spawn_surface` / `send_text` / `send_key` / `capture_tail` / `stream_to` / `list_surfaces` /
> `liveness` / `kill` / `attach`. The spec-authoring MCP tools (`create_spec` /
> `write_spec_artifact` / `validate_spec`) were added 2026-10-19 as `mcp-spec-authoring-tools`.
> They are gated by one consent key in the repo's own config, `[mcp] allow_repo_writes`, off by
> default and root-only; writes are confined to the change directory; OpenSpec only for now.
> Stale here: `/log` already accepts `since` — what is missing is a client
> that uses it and a long-poll.

Theme: **move the orchestrator out of the session.** Today git-paw runs a second LLM in
pane 0 against a 1,862-line `supervisor.md`. But the tool is in practice driven *through*
a capable agent CLI already — and that driver is the orchestrator. A second one inside
tmux is redundant, is the most vendor-coupled surface in the codebase, and has been the
most persistent source of dogfood friction across cycles (self-stall deadlock, slow
approvals, addressing the user in-pane, idle-nudge, layout complaints).

So: **delete the supervisor pane; keep the drive loop.** The orchestrator becomes the
external driving session, equipped with orchestrator *skills* plus a documented workflow.
This is not removing a capability — it is **relocating the orchestrator from inside the
session to outside it.** Frame it that way in docs and changelog.

This **absorbs the old v0.17.0 remote/mobile theme** rather than displacing it (see
"Remote/mobile arrives free" below), so nothing shifts version.

### The pane / loop split

git-paw's "supervisor" is two separable things, and only the first goes:

1. **The LLM supervisor pane** — judgment: gates, merge decisions, escalation. → **out**,
   replaced by the external orchestrator + skills.
2. **The mechanical drive loop** (`src/supervisor/`, ~14k LoC) — polls panes, classifies
   prompts, auto-answers safe ones, detects stuck states, nudges, runs correction cycles,
   refreshes branches. → **stays.** It is the actuator; an external turn-based orchestrator
   cannot replace a 250ms poll, and this is where most of v0.14–v0.16's value was built.

### Capability retirement map

An audit of the 10 supervisor/approval capabilities says the change is much better
bounded than the directory size suggests — **only one capability is genuinely skill-shaped;
six are mechanism and stay untouched:**

| Capability | Disposition |
|---|---|
| `approval-command-safety` | **keep** — classifier, danger list, whitelist. Security-critical mechanism; must never become skill content. |
| `approval-auto` | **keep** — keystroke gate, send-gate re-confirm, file-edit approval. Simplify the pane-0 special-casing that exists only because the supervisor occupied a pane. |
| `approval-pattern-surfacing` | **keep** — arguably *more* valuable: escalations now leave the tool, so the pattern log is how the allowlist gets tuned. |
| `supervisor-unattended-operation` | **keep** — the in-process drive loop. The actuator. |
| `supervisor-correction-loop` | **keep** — mechanism unchanged; only the source of gate verdicts moves outside. |
| `supervisor-learnings` | **keep** — aggregator unchanged; the qualitative channel's publisher becomes the external orchestrator. |
| `supervisor-launch` | **trim** — supervisor pane, its layout, and `layout.rs` go; worktree creation, agent grid, dashboard pane, boot prompts stay. The bulk of the actual deletion. |
| `supervisor-config` | **trim** — `approval_flags` for the *supervisor's own* CLI goes; worker flags and the rest of `[supervisor]` stay. |
| `supervisor-autonomous-orchestrator` | **rewrite** — the hand-off survives, its delivery changes (see below). |
| `supervisor-skill-discipline` | **convert** — this is literally skill content. Becomes the exported orchestrator skill set. |

### The no-pane path already exists

`supervisor-autonomous-orchestrator` already specifies that **"with no supervisor pane
nothing is injected, so a pure `--unattended` run is byte-for-byte the prior behavior."**
That path is specced and tested today. Removing the pane means *making it the only path* —
substantially less risky than building it fresh.

### Autonomy stays self-contained

Losing the pane does **not** mean losing unattended runs, and does not make autonomy
depend on the driver having a scheduler:

- the drive loop keeps sweeping mechanically, as now;
- when it hits a judgment call it cannot make, it **invokes the orchestrator CLI headlessly,
  one-shot**, with the orchestrator skill and the escalation context.

git-paw already knows how to launch CLIs. A one-shot invocation is also strictly better
than a long-lived pane: it cannot self-stall, which the pane demonstrably does.

### Verdict authority moves to the broker, not to a skill

**Authority cannot be a skill** — a worker agent can ignore an instruction. Skills carry
*procedure* (the five-gate sequence, what each gate checks); the broker must enforce *who
may publish a verdict*.

The primitive already exists: `agent_id` derives from the worktree branch, and an
orchestrator driving from the repo root has no worktree branch. So the rule is structural
and enforced in Rust at `/publish`:

> **root session may publish `agent.verified` / `agent.feedback`; worktree sessions may not.**

This also resolves cleanly against `mcp-boot-event-publish` (v0.16.0), which specs those
two verbs off the MCP write surface: same seam, orchestrator-only, tools stay
identity-derived rather than parameter-driven. It is what makes the planned
`publish_agent_feedback` MCP tool safe to ship.

### Remote/mobile arrives free

The old v0.17.0 described two topologies. **Topology B** — remote-control a separate
control client that drives git-paw, with "the git-paw supervisor + workers running on
anything, because you're controlling git-paw *through* the client" — **is exactly this
architecture.** Removing the pane deletes topology A and makes B canonical.

So the phone workflow is no longer a cycle of work: your driving session is already a
remote-controllable Claude Code session with git-paw's MCP server attached. Keep the
supported-CLI research and the Remote-Control guide from the old section; drop the
topology-A column and the "which CLI can be a remote-controlled supervisor" question
entirely — it stops existing. Retain the caveat that the local process must stay running
(~10-min outage timeout), which is also the argument for the daemon in the tmux spike below.

### The code of this cycle

Mostly already planned under the old v0.17.0, and it is exactly the substrate an external
orchestrator needs:

- **`cmd_*` library refactor** — expose `cmd_start`/`pause`/`stop`/`purge` as callable
  functions, not CLI-only entry points. Also a v0.19.0 prerequisite.
- **MCP write tools** — `create_spec` / `write_spec_artifact` / `validate_spec`,
  `start`/`pause`/`stop`/`purge_session`, `publish_agent_feedback`. Now gated by the
  root-session authority rule above.
- **Broker-enforced verdict authority** (root vs worktree session).
- **Separate the broker host from the dashboard view.** Today `cmd_dashboard()` *constructs*
  the broker (`BrokerState::new`, watcher targets, conflict detector, learnings aggregator,
  verify-on-commit nudge) and hard-gates on `TMUX` being set. So the dashboard pane **is** the
  broker's host: turning the dashboard off would take down conflict detection, learnings, the
  watcher and every agent's publish target. The v1.0.0 layer model has this backwards — it
  lists "+ Dashboard … Requires broker" as layer 5, but in the implementation the broker
  requires the dashboard. A concrete instance of the layer-isolation work item.

  **The pattern is already shipped: copy the MCP server.** `src/mcp/mod.rs` holds no
  `BrokerState` — it resolves `broker_url` from the session file and talks HTTP, degrading
  gracefully when no session is live. The dashboard becomes the same kind of thin client.

  **Belongs in this cycle** because verdict authority is enforced *in* the broker, the MCP
  write tools assume a broker that outlives any view, and the pane layout is already being
  restructured by the supervisor-pane removal.

  **Open sub-question — where the broker then lives.** Not the drive loop: that runs only
  under `--unattended`, and a manual session still needs a broker. So either its own process,
  or a long-lived **session-host** process owning the broker (and hosting the drive loop when
  unattended). Note the latter is the same daemon a PTY backend would require — doing this now
  partially pre-builds the tmux option.
- **Seam-first `drive.rs` split** — make "supervisor pane" an explicit role in the type
  system, let the compiler enumerate every site, *then* remove the variant. `drive.rs` is
  6.5k lines that interleave sweeping the supervisor pane with sweeping worker panes;
  refactor-then-delete turns archaeology into a mechanical change. Precedent in-repo:
  `WorkerPhase`, the dashboard MVU restructure, the `CommandRunner` seam.
- **Orchestrator skill set** — converted from `supervisor-skill-discipline` +
  `supervisor.md`, exported for the *driving* agent rather than injected into a pane.
  No gate-auditor skills shipped here; they arrive with v0.20.0's delegate profiles.
- **Workflow documentation** — how to drive a session from an external agent. Replaces
  `quick-start-supervisor.md`.

### Knock-on effects

- **v0.20.0 reshapes.** Vendor subagent files are dropped. Personas become config profiles
  (`agent-profiles`): delegate profiles give the external orchestrator a git-paw-run, model-pinned
  executor for the gate audits, alongside its own native subagents.
- **v0.19.0 shrinks.** The supervisor pane is the most CLI-coupled surface; the "which CLI
  runs the supervisor" dimension disappears. Workers still need prompt-shape profiles.
- **v1.0.0's layer model needs rewriting** — "+ Supervisor" as layer 4 of 7 no longer
  describes the tool. v1.0.0 may carry several breaking changes; this one should land
  *before* the freeze and be dogfooded through a cycle, not land *at* it.
- **Worker CLI session id / `--resume`** — **pulled forward to v0.17.0** as that cycle's key
  feature (resume a crashed/detached worker with its context). It pairs with this cycle's
  per-CLI provider layer (v0.19.0) but no longer waits for it.

### Evidence from the v0.17.0 dogfood (2026-09-15)

The v0.17.0 dogfood ran deliberately as a relocation rehearsal — `--no-supervisor`, no drive
loop, an external orchestrator session doing the supervising. Three results bear directly on
this cycle:

- **GP-33 — a turn-based orchestrator cannot replace a sub-second poll.** The external
  orchestrator ran a 20s pane-state poll and *silently missed a permission prompt*: the agent
  answered one and hit the next inside the same window, so both samples read PROMPT and no
  transition fired. `detect-stuck` caught it only after ~5 minutes wedged. The in-process loop
  has no such failure mode — it dedups on **command identity** (`approval_dedup_key` = agent +
  command slice), so a second distinct prompt is a distinct event regardless of sampling phase.
  **This confirms the split as written: the pane goes, the loop stays.** It also argues the
  orchestrator's read surface must be event-driven (broker watch / long-poll), not
  capture-diff polling — fold that into `orchestrator-skill-export`.
- **Externally-driven supervision costs a turn per prompt.** Over one three-agent wave the
  orchestrator hand-handled ~15 approvals, two conflict adjudications and one CLI-death
  recovery. Nothing was *impossible*, but the cadence tax is real and is the thing the drive
  loop must keep absorbing after the pane is gone.
- **A stalled wave is invisible.** The dogfood session sat on an unanswered prompt for ~13 days
  with all three panes alive and the broker reporting agents as `working`. Whatever replaces
  the pane needs a wave-level staleness signal, not just per-agent heartbeats.

### Deferred here from the v0.17.0 dogfood

**Docs-sweep findings (2026-10-18, verified where marked).** Found by the
`docs/architecture-refresh` sweep, triaged into the cycle that reworks the code involved:

- **DS-1 (verified) — the governance boot section is never injected.**
  `skills::governance_section_paths` has no production caller; all six call sites are in the
  test module, so `boot-block`'s *Boot prompt includes governance documents section* has never
  been met. It targets the supervisor prompt this cycle replaces — carry the requirement into
  the orchestrator's prompt or retire it explicitly, but do not leave it silently unmet.
- **DS-2 — the bundled supervisor skill maps panes to agents by path basename**
  (`assets/agent-skills/supervisor.md` ~:249-252), which misses the child-worktree layout.
  The skill is rewritten here.
- **DS-3 — `set_spec_id` has no production caller**, so forward conflicts are never labelled
  intra-spec. Belongs with the broker-authority work.
- **DS-4 — the manual-approvals log is written only by the dashboard's poll loop**, so
  unattended runs record nothing. Belongs with the drive-loop / orchestrator split.
- **DS-5 — the launch-readiness gate covers only supervisor `start` and `add`**
  (`commands/supervisor.rs`, `commands/add.rs`); bare and spec-driven launches are ungated and
  a stuck first-run dialog fails the launch. Same class as GP-22/23/24; once every session is
  pane-less it must cover that path.
- **GP-47 addendum — `git paw stop` and `--force`: the specs and `--help` disagree with the
  code.** `cli-parsing` and `broker-runtime` and the `--help` text say stop prompts and
  `--force` skips the prompt; `cmd_stop(_force)` never prompts and ignores the flag, and
  `tests/cli_prompt_matrix.rs` asserts no prompt. **Decided 2026-10-18: the code is right** —
  stop is recoverable, so it never prompts, and `--force` is **removed** (breaking; scripts
  passing it get an unexpected-argument error). Pulled forward into v0.17.0 as
  `stop-without-confirmation`.

- **GP-49 — `git paw add` refuses bare sessions; after this cycle every session is bare.**
  Opening wave 2 of the v0.17.0 dogfood with `git paw add --from-spec …` failed:
  `git paw add supports supervisor-mode sessions … Session 'paw-git-paw' was started in bare
  (no-supervisor) mode, whose tiled grid is not re-tiled incrementally in v0.6.0`. Today that is
  a narrow limitation. Removing the supervisor pane makes every session bare, so `add` and
  `remove` — the whole hot-attach surface — stop working unless the bare grid learns to re-tile.
  The relocation must carry the supervisor-mode tiling over to the pane-less layout or retire
  hot-attach explicitly; it cannot inherit this refusal silently. (The error text also still
  says "v0.6.0".) It also means an external orchestrator cannot open a new wave without a
  teardown between waves, which is exactly the step it most wants to avoid.

- **GP-30 — the spec workflow the boot prompt names is not reachable from the worktree.**
  `build_task_prompt` tells OpenSpec agents to run `/opsx:apply <id>`, but the opsx command
  definitions live in a gitignored `.claude/`, so a worktree has none of its own. It works
  today only by accident of `worktree_placement = "child"` (the repo root's `.claude/` is an
  *ancestor* of the worktree, so the CLI's upward search finds it); with external/sibling
  placement the instructed command simply does not exist. Wants a `doctor` check that warns
  when the instructed workflow is unreachable — not a launch-path change, so it waits for the
  cycle that touches orchestration anyway.

### Architecture direction — code analysis (2026-10-18)

**Pattern: ports and adapters at the edges, a functional core in the middle.** Inside, this
is "functional core, imperative shell" (Rust's *sans-IO*): the drive loop becomes
`observe → decide → apply`. Only `decide(state, observation) → (state, effects)` is pure; the
ports observe and apply. The dashboard's MVU (`update` is pure) and the classifier
(`git paw __classify` is a pure function of captured text) are this pattern already. Hexagonal
says where the boundaries are; the functional core says what lives inside them.
Clean/Onion-style layering and runtime plugins are rejected as ceremony for a single-binary
CLI.

Grounded state (read 2026-10-18, before v0.17.0 shipped; line numbers have moved since):

- **Terminal.** 17 raw tmux spawns in 8 files outside `src/tmux/`, none through
  `CommandRunner`. The `"{session}:0.{n}"` target is hand-built in 7 files, and
  agent-owns-pane lookup is implemented 4× (once in `sweep.sh`). About 7 supervisor traits
  are typed `(session, pane_index)`, and `tmux/readiness.rs` imports
  `supervisor::permission_prompt` — the dependency points the wrong way. The code needs 13
  domain operations. Port: `TerminalBackend` keyed by an opaque `SurfaceId`, with
  `spawn_surface`, `send_text`, `send_key`, `capture_tail`, `list_surfaces → (id, cwd)`,
  `stream_to`, `liveness`, `kill`, `attach` (layout optional). One domain-level fake replaces
  ~19 per-trait fakes. **There is no headless path today:** every launch calls
  `ensure_tmux_installed`, and `--unattended` hard-wires the tmux implementations.
- **Broker.** `cmd_dashboard` is the only production host and is `$TMUX`-gated
  (`main.rs:908`). `pause` stops the broker by killing that pane, and watchers, conflict
  detection, learnings and auto-approve all die with it. There is no streaming: clients pull
  the whole `/log` without `since`, so every poll is O(n). The event-driven read surface that
  GP-33 asks for does not exist yet.
- **Supervisor role.** `PaneRole::Supervisor` is decided by pane index 0, flattened to the
  string `"supervisor"`, then re-detected by string comparison (`drive.rs`). Separate pane-0
  constants live in `approval_gate.rs`, `approve.rs` and `layout.rs`. Type the role end to
  end first, then delete it.
- **Command layer.** The `cmd_*` handlers are `pub(crate)` in the binary crate and import
  helpers back from `main.rs`; `cmd_purge` still lives there. They read `current_dir()`,
  `println!` 4–24× each, block on attach or the drive loop, and return `Result<()>`. The MCP
  write tools need lib-crate functions that take repo + config, return structured results,
  and print through an injected output sink. **This is where the *driving* half of hexagonal
  starts to pay:** from this cycle the CLI and MCP both drive the same use cases.
- **Drive loop.** About 7.2k lines (≈3k production). Its 11 side-effect traits already are
  the port list; what is missing is pulling the decisions out from between the effects. Do
  that seam-first, one decision at a time, starting with the nearly pure ones: approval
  (classify → option → gate), nudge, exit summary. Effects as data also yield an audit log
  that v0.22.0's replay timeline can project.

Deferred here (the rewrite above deletes the code they live in):

- **GP-45 — the dashboard auto-approve pane map goes stale.** It is positional
  (`idx + SUPERVISOR_PANE_OFFSET`), computed once at dashboard start (`main.rs:1146-1153`).
  After `add`/`remove` it misses approvals or reads the wrong pane; the classifier still
  gates safety. **Moot since v0.17.0:** the dashboard auto-approve thread was deleted with the
  classifier.
- **GP-46 — the pane-0 gates ignore session mode, and they disagree.** In Bare mode pane 0
  is the dashboard (`session.rs:40-47`). `poll.rs` and `drive.rs` also apply opposite pane-0
  rules (poll says the supervisor has no pane; drive approves pane 0). A typed `PaneRole` and
  the pane removal delete both.
- **GP-47 — `cmd_stop` ignores its `--force` argument.** Fix it in the `cmd_*` library
  refactor, or drop the flag before the freeze.

### Sequencing & process

**Follows the v0.17.0 reliability release** (shipped 2026-10-25). The target structure is
documented in `docs/src/architecture-direction.md` (Target Architecture, shipped in v0.17.0) and
applied through the `concept-decoupling` dev skill. Every change in this cycle follows it.

Implementation order. Each step is seam-first and behavior-preserving until it deletes something.
1. **Type the supervisor role end to end.** Today `PaneRole::Supervisor` is decided by pane
   index 0 and re-detected by string. Making it a real type is the precondition for deleting it.
2. **Remove the supervisor pane.** This deletes the pane-0 logic (GP-46) and the
   supervisor-only layout.
3. **Add the `TerminalBackend` port keyed by `SurfaceId`, with tmux as the first adapter.**
   Route the scattered tmux calls through it. After step 2 it needs no supervisor-pane cases,
   and it removes GP-45.
4. **Move the broker into its own host process,** with the dashboard as a thin HTTP client.
   Verdict authority and the MCP write tools depend on this.
5. **Driving side:** make the `cmd_*` handlers a library (repo + config in, structured result
   out, injected output sink). Add the root-vs-worktree verdict-authority rule and the MCP
   write tools.
6. **Throughout:** pull decisions out of `drive.rs` into pure functions, one at a time. Start
   with approval, nudge and exit summary.
7. **Last:** the orchestrator skill set and workflow docs.

Spec-first: the OpenSpec change set was authored before any implementation started.

### Confirmed for this cycle (regardless of the tmux decision)

Two items are **agreed and in scope for v0.18.0**. Neither commits to any tmux outcome, and
both pay off even if tmux stays forever:

1. **Close the tmux seam leak, into a backend trait (refactor).** Not merely "route calls
   through `src/tmux/`" — make the terminal surface a **pluggable backend interface** with
   tmux as the first implementation. Pure refactor, no behaviour change and not a bugfix.

   **Why a trait rather than a tidier module:** it makes the tmux / PTY / headless fork
   *non-binary*. Each becomes a backend that can be shipped, A/B-tested and chosen on
   evidence, instead of an architectural bet argued in advance. tmux is not unique —
   **zellij** (`zellij action write-chars` / `dump-screen`), **WezTerm mux** (`wezterm cli`,
   plus persistence *and* native Windows), **GNU screen** (`-X stuff` / `hardcopy`) and
   **kitty** (`kitty @`, no persistence) all expose the needed verbs.

   **Design rule — the interface is defined in git-paw's domain terms, never tmux's verbs.**
   A trait exposing `get_pane_current_path()` is a tmux-shaped hole only tmux fits. The right
   shape is *"which agent owns this surface?"* (tmux answers via `pane_current_path`, a PTY
   backend knows by construction, headless never asks) and *"give me this agent's output
   stream"* (not "pipe-pane this").

   **Scope warning — larger than a call-site sweep.** tmux's *model*, not just its calls, has
   leaked into the domain layer: `pane_current_path` → `skills.rs`, `coordination/inventory.rs`,
   `commands/remove.rs`, `supervisor/drive.rs`; `pipe-pane` → `domain.rs`, `cli.rs`,
   `logging.rs`, `commands/helpers.rs`; `split-window` → `commands/{start,supervisor,recover}.rs`;
   and **`pane_index` is persisted in the session file's agent entries**, so the work reaches
   the state schema and carries a back-compat obligation.

   **Compile-time trait with in-tree impls — not runtime plugin loading.** Runtime plugins
   would mean a stable ABI commitment immediately before the v1.0.0 freeze plus dynamic-loading
   machinery against a strict dependency list. In-tree backends that shell out cost **zero**
   new dependencies (git-paw already shells to tmux). If external backends ever matter, use a
   process-based adapter contract — subprocess-per-call is already the status quo.

   **Test burden is plausibly net-negative:** today's 3,167 lines of tmux tests double as
   *logic* tests. A fake backend (the `CommandRunner` precedent) lets the logic be tested once
   and each real backend only smoke-tested.
2. **Record each worker's CLI session id → `--resume`.** **Pulled forward to v0.17.0** as that
   cycle's key feature — it unlocks resume after a crash, detach, or intentional relaunch under
   *every* backend option below (including staying on tmux). Small, per-CLI; pairs with the
   v0.19.0 provider layer but no longer waits for it.

### Open question (tied to the tmux/headless fork): does the dashboard survive?

**Decided 2026-10-19: it survives as an opt-in usage view.** With the orchestrator outside the
session, the roster duplicates `git paw status`. What is worth a live view is usage: tokens and cost
per worker, per tier and for the orchestrator's one-shot calls, against the budget caps, plus the
live event log. From v0.21.0 (`dashboard-usage-view`, replacing the tabbed-roster change) the
dashboard is off by default and shows usage and events. The roster lives in `git paw status`. The
analysis below is kept for the record.

(Original framing: unresolved, and it **gates the tabbed-dashboard request** — the two pull in
opposite directions, so decide the direction before spending on either.)

**Downstream of the broker/dashboard split above.** Once the broker is its own host and
the dashboard is a thin HTTP client, keeping or dropping the view becomes a cheap,
reversible product call rather than an architectural one — so the split lands first and
this question can wait for evidence.

The dashboard (3,910 lines) exists because agents are opaque TUI panes needing a structured
summary. Headless relocates that need rather than removing it:

- **If a git-paw TUI survives**, the dashboard stops being one pane among panes and becomes
  the *entire frontend* — more central, and the tabs/scrolling request is worth building.
- **If no TUI survives**, it is largely redundant and deletable.

Decompose rather than keep-or-delete. Only one third of it has no substitute:

| Function | Where | Substitute today |
|---|---|---|
| Agent roster / status | part of `view.rs` (1,415) | ✓ `git paw status`, incl. `--json` |
| **Live broker log stream** | `broker_log.rs` (1,676) | ✗ none — the unique part |
| TUI shell / layout | `dashboard.rs` (819) | only needed if a TUI survives |

So the minimal end state is `git paw status [--json]` for state + a `logs --follow`-style
command for the stream, and no TUI — a large deletion for little functional loss.

Deciding factor: **does the human need an ambient, zero-cost view?** Asking the orchestrator
costs a turn and is *pull*; a dashboard is *ambient*. But `watch -n5 git paw status` recovers
most of that, which narrows the gap considerably. Working-style call, not a technical one.

### Proposal (not committed): drop tmux

**Reframed 2026-10-19.** With v0.18.0's `TerminalBackend` port, tmux is just the first adapter, so
"dropping tmux" becomes "add backends and choose a default on evidence". The **headless** adapter is
specced as the additive post-1.0 **v1.3.0**: workers run their CLI's non-interactive mode, with no
PTY and no daemon. tmux stays the default through the freeze. A PTY-owning interactive backend
remains unscheduled. The analysis below is kept for the record.

Being evaluated, **not scheduled**. Owning PTYs directly instead of shelling to tmux would
kill whole bug classes rather than mitigate them — `send-keys` unreliability, paste
buffers, follow-up-Enter, pane-index mapping, pane-claim TOCTOU — and native Windows
becomes possible (tmux is the sole reason for WSL-only). Coupling is deep: 47 files mention
tmux, 89 non-test `send-keys`/`capture-pane` call sites outside `src/tmux/`.

Two honest obstacles, to be settled by a spike **during** v0.18.0 — decision only, no code:

1. **Persistence — narrower than it first looks.** Durability splits three ways: the worktree
   survives anything (it is on disk), the CLI session id restores context (`--resume`, item 2
   above), and only *progress while detached* needs a live process. So crash/detach is
   recoverable, not fatal.

   The only genuinely at-risk capability is **unattended-while-detached**. Two things shrink it
   further: (a) it is **already unreliable** — detached `--unattended` under `nohup`/no-TTY
   sweeps a while then dies, stranding the worker (the heartbeat early exit; its cause was a
   wall-clock timer, not the missing TTY, and v0.18.0 fixed it), with
   "don't fire-and-forget" as the standing workaround; and (b) **persistence is a PTY problem,
   not a general one** — a daemon is needed only because closing a PTY master sends SIGHUP.
   Headless workers (the v1.3.0 backend) writing to a log survive via `setsid` + redirection, from
   the OS, with **no daemon at all**. So the daemon cost belongs to *keeping interactive TUIs
   without tmux*, not to dropping tmux.

   Calibration: `Ctrl+Z` on today's supervisor stops progress exactly as a dead TUI would; only
   *tmux detach* keeps agents working while away. That single behaviour is what is in question
   — not "resume".
2. **Rendering.** Pass-through needs no terminal emulation; **multiplexing and tabs do.**
   Switching away from a tab and back requires per-pane screen state, which means a VT
   emulator (`vt100`/`wezterm-term`) against a deliberately strict dependency list. A
   tabbed UI is wanted, so this cost is probably unavoidable rather than designable-away.

Note **replay is not solved for free by this.** Capture is not the current failure —
`pipe-pane` already yields raw bytes; playback is, because `replay.rs` strips ANSI from a
cursor-addressed stream. PTY ownership improves timing fidelity only; a VT player (or the
semantic-timeline alternative) is still required. See v0.22.0.

Removing the supervisor pane first makes this materially cheaper: it deletes the one pane
that must be human-interactive and attachable, leaving workers, which are far more
automatable.

---

## v0.19.0 — Per-CLI foundation (planned)

Theme: make **every** CLI a first-class worker, not just the Claude family.
Pulled out of v1.0.0's "Per-CLI Specialisation" so the freeze locks a **proven** per-CLI
surface rather than brand-new code. This is the direct enabler of the tiered-model
thesis — cheap workers on arbitrary CLIs are only *flawless* once the per-CLI
friction-killers exist (today the approval / send / boot machinery works cleanly only
for the Claude family). Sibling to v0.18.0: that named *which CLIs can drive git-paw
remotely*; this makes *each CLI a first-class participant*. Additive, opt-in, no
breaking changes; inherits the `cmd_*` library refactor pulled forward in v0.18.0. The
Claude-family *boot trio* (GP-01/02/03/04) was carved ahead into **v0.16.0** so the
flagship boots on a fresh repo now; this cycle **generalizes** that machinery across CLIs.

All of it hangs off one abstraction — a **resolved per-CLI record** (`CliProfile`, capability
`cli-provider`: compiled provider families with a `generic` fallback) — with the same
"native-first, generic-fallback" shape throughout:

**DS-6 — `KNOWN_CLIS` detected `cursor`**, which on most machines is the Cursor editor's
launcher rather than its agent CLI. Fixed in v0.17.0, which detects `cursor-agent`; checking the
rest of the detection roster is the v0.23.0 audit's job.

**CLI identity is the shared root cause (v0.17.0 dogfood, 2026-09-15).** Two findings from
that dogfood are the same bug seen twice, and both belong to this abstraction:

- **GP-25 — a `[clis.<name>]` key must itself be executable on PATH.**
  `detect::resolve_custom_clis_in` resolves `command` only to decide *availability*, then
  discards the resolved path: the pane is launched with `CliInfo.binary_name`, which is the
  config **key**. So `[clis.foo] command = "/usr/local/bin/bar"` never launches `bar`.
  Confirmed again when pinning dogfood workers to Sonnet needed a PATH shim named exactly
  `claude-oss-sonnet`. Corollary: **there is no config seam at all for per-worker launch
  arguments** (model, effort, extra flags) — `approval_args` is the only place flags can be
  appended, it is keyed by approval level, and the non-supervisor path ignores it anyway
  (GP-24, fixed in v0.17.0).
- **GP-29 — a custom Claude variant does not get the MCP boot block.**
  `skills::cli_speaks_mcp` matches on CLI *name*, which no custom `[clis.<name>]` key can
  satisfy, so a genuine Claude Code variant is handed the shell-helper boot block instead.

Both resolve the same way: CLI identity must be a resolved capability record, not a string
key. v0.17.0 (`cli-launch-identity`) already launches the configured `command` and added the
`[clis.<name>] mcp` opt-in; this cycle builds the record on top (`family`, `args`, the
ambiguous-shim refusal).

- *(Per-CLI hook providers, trust-folder pre-acceptance and broker-curl allowlist seeding were
  dropped; see "Not specced" below.)*
- **Per-CLI native auto-mode** — generalize the v0.11 Claude-first supervisor auto-mode
  across CLIs via the same provider seam.
- **Per-CLI native sandbox** — `[clis.<name>].sandbox = "native" | "wrapper"` declares how a
  CLI's workers are confined. Only Codex ships native sandbox arguments (`--sandbox
  workspace-write`); other CLIs wait for the v0.23.0 audit. Full-auto never switches a declared
  native sandbox off.
- **Interaction contract + cost attribution** — the per-CLI prompt-shape / option-selection
  / model-flag contract that makes the send-gate + detectors work per CLI, plus the
  **usage** facet: per-role token/cost visibility for workers and the drive loop's headless
  orchestrator one-shots (the operator's own driving session is outside git-paw and never
  counted) — the tiered-split's economics made visible (move #2 from the original analysis,
  homed here).
- **Budget guardrail** — with cost attribution in place, an optional cap (`[supervisor]
  budget_max_tokens` / `budget_max_cost`) enforced by the drive loop: on breach it escalates
  once and stops every spend it initiates (nudges, re-engagement, correction cycles,
  one-shots), and with `on_exhausted = "abandon"` exits `budget-exhausted`. It never
  interrupts or kills a worker. The orchestrator skill gains the matching budget step. Bounds
  *spend* the way v0.15's sandbox bounds *blast radius*: the safety complement for an
  unattended cheap-worker fleet driven from a phone.

Per-CLI config-path detail (which settings file per CLI, including the Gemini→`agy` refresh)
lives in the `cli-provider-record` change (capability `cli-provider`) — MILESTONE stays at the
roadmap level.

Post-1.0 follow-on: a **CLI-auth preflight** (`auth_ok()` per provider, surfaced via
`doctor` — catches "installed but not logged in", a top launch failure) is a natural
*additive* extension of this provider layer, deferred past the freeze. Reliability varies
per CLI (there's no standard "am I authenticated?" check), so it's real per-CLI work, not
a quick win.

### Friction-killers that remain here (the boot-critical head moved to v0.16.0)

The **boot trio** (GP-01/02/03), **GP-04**'s `.git/`-write safety + `.git-paw/` conflict
false-positive, the classifier bundled-helper allow-list + `normalize_command`
leading-assignment / `env` / `nohup` normalization, and the GP-05/06 +
orchestrator-in-`session.json` unattended-hardening items were carved into **v0.16.0** —
they gate a fresh consumer's boot and ship ahead of this cycle. What stays here is the
*per-CLI generalization* of that machinery plus the interaction-contract items that need
the provider seam:

- **The pump can't clear a worker's edit-approval.** The unattended pump approves
  classifier-safe *shell* prompts but has no path for a Claude-Code file-edit approval, so
  a worker stalls on its first create/edit until a human enables accept-edits — a per-CLI
  interaction-contract item (boot workers in accept-edits, or teach the pump the
  edit-prompt shape).
- **No auto-approver above the orchestrator.** The orchestrator pane self-stalls on its
  *own* read-only monitoring commands because native auto-mode covers only worker panes;
  extend it to the orchestrator's own safe reads (the per-CLI native-auto-mode seam).
- **A sandboxed worker should run in skip-permissions.** With the v0.15 FS-scoped sandbox
  kernel-confining a worker to its worktree, the entire approval treadmill (edit prompts,
  non-classifier `perl` / `export …` commands) is *unnecessary* — the highest-leverage
  friction-killer, and a natural fit with this cycle's native-sandbox + per-CLI-contract
  work. (The unattended pump also exited early on a "heartbeat" when launched detached,
  stranding the worker — a pump-robustness item to fix alongside.)
- *(GP-07 — the Spec Kit boot prompt pointing workers at OpenSpec paths — shipped in v0.16.0.)*
- **Per-CLI profiles are mostly inert — thread a `CliProfile` instead of calling Claude's**
  (code analysis 2026-10-18). `[clis.<name>]` config is data-driven, but
  `claude_prompt_profile()` was called directly at 16 production sites before v0.17.0; after the
  classifier retirement **13 remain**: `permission_prompt.rs` ×6, `drive.rs` ×3, plus
  readiness, manual_approvals, approval_gate and inventory. Only two sites resolve a profile at
  all, and `drive.rs` documents its shape checks as Claude-specific. The `cli-provider` work is
  mostly this: resolve one `CliProfile` per surface and pass it as context. Use the Strategy pattern with a Null
  Object (`generic`) fallback, not a new subsystem.
- **Make `SpecBackend` a real Strategy.** The trait only has `scan`. Everything else matches
  `SpecBackendKind` in about 9 scattered places (`main.rs`, `skills.rs`,
  `mcp/query/specs.rs` — one with a `_` arm, `speckit.rs`) or dispatches on the raw type
  string (`specs/mod.rs`, `main.rs`, `init.rs`, `mcp/tools/project.rs` — see GP-44). The
  format itself has three unconverted forms (`SpecsFormat`, `&str`, `SpecBackendKind`).
  Move per-kind behavior (path doctrine, task prompt, MCP backend name, title) into trait
  methods, keep one format type, and the variant-ripple checklist shrinks to the trait.

Send friction from the same run: **paragraph-length `send-keys` land in the CLI's paste buffer
unsubmitted** — the supervisor must capture-pane and send a second Enter (3×). Auto-append a
delayed second Enter after paragraph-length sends (a `sweep.sh tell` verb). Extends the
buffered-input detector / follow-up-Enter rule — a per-CLI send-contract item.

C5's per-pane approval claim (v0.14.0) now guards `sweep.sh approve`, the drive loop's nudges
and the new `sweep.sh tell` (`verified-agent-send`), so a typed message can't race an approval
or another send.

### Sequencing & process

Build on v0.18.0's `cmd_*` refactor. Land the `cli-provider` record first, then profile
threading, then sandbox mode, usage and the budget cap, and verified send on top.

**Specs committed 2026-10-19 on `feat/v0.19.0-specs`** (7 changes with `add-keeps-worktree-state`,
added 2026-10-29; all `--strict` valid; the
capability is named `cli-provider`): `cli-provider-record` (`[clis.<name>].family` / `.args`,
native auto-mode; GP-25, GP-29 and DS-6 were since pulled into v0.17.0 as `cli-launch-identity`,
so this change rebases over it) → `cli-profile-threading` (the 13 remaining
`claude_prompt_profile()` sites)
→ {`worker-sandbox-mode`, `worker-usage-budget`, `verified-agent-send`}; `spec-backend-strategy`
lands any time after v0.17.0. Not specced, with reasons:
- **Stale:** GP-07 shipped in v0.16.0. The pump approves nothing since v0.17.0; file edits don't
  prompt because Claude's `auto` level maps to `acceptEdits`.
- **Contradicts the shipped "no permission grants are seeded into agent CLI settings files"
  rule:** per-CLI hook providers and broker-curl allowlist seeding.
- **Already answered at launch:** trust-folder pre-acceptance (GP-03b answers the dialog).
- **Gone with the v0.18.0 pane:** the orchestrator auto-approver.
- **Owned elsewhere:** DS-6 goes to the v0.23.0 audit. The heartbeat exit was fixed in v0.18.0
  (`unattended-heartbeat-progress`).

`cli-setup-consent` (`git paw setup-cli`, an explicit consented run, never at launch) was drafted
here on 2026-10-19, then **moved into v0.17.0** on 2026-10-20 together with the classifier
retirement: v0.17 ships it trimmed to Claude's allowlist, and the v0.19 copy was dropped. Folder
trust, hooks and other CLIs wait for the v0.23.0 audit's facts.

**Re-scoped 2026-10-20 for v0.17.0's `classifier-retirement`** (git-paw no longer approves any
prompt; the drive loop detects and escalates). The v0.19 changes lose their classifier parts:
- `cli-profile-threading` covers only the detectors that survive.
- `worker-sandbox-mode` documents a sandboxed `full-auto` as the route to a truly unattended run.
- `verified-agent-send` lists `sweep.sh tell` in setup-cli's grant table as never granted, with
  an ask rule.

**Restacked on the released v0.17.0 (2026-10-25), and again on v0.18.0 (2026-10-29).**
`cli-provider-record` keeps v0.17.0's launch rule (decided 2026-10-25):
- A configured CLI launches its `command`; a built-in one launches by name.
- It adds only the refusal of an ambiguous PATH shim, the `args` slot and `family`.
- An explicit `[clis.<name>] mcp` wins over `family`, which only supplies the default.
- The family value is `cursor-agent`, matching the CLI name v0.17.0 detects.

**Added 2026-10-29: `add-keeps-worktree-state`** (GP-87, from the 0.18.0 dogfood). Swapping a
worker's CLI (`remove --keep-worktree`, then `add --cli`) failed when the kept worktree had
uncommitted changes, because `add` always rebases an existing branch. `git paw add --no-rebase`
matches `start --no-rebase`, and `add` never rebases a dirty worktree: it skips the rebase with a
notice and touches nothing in the tree.

---

## v0.20.0 — Skills packaging + agent profiles (planned)

> **Reshaped 2026-10-19.** Vendor subagent files are out. The persona half is now
> `agent-profiles`: named worker and delegate profiles in config. The *packaging* half is
> unchanged. The section below is the original plan; the "Specs committed" note at the end is
> current.

Theme: skills become **packages**, and the first thing that rides on them — the
orchestrator's own **subagents** — ships in the same cycle. Pulled forward from v1.0.0
(skills) and the old v1.1.0 (subagents): the packaging is the foundation the subagent
personas *are built out of*, so shipping them together is cohesive and gets both
dogfooded before the freeze. Additive and opt-in, with two upgrade effects: the moved helpers
need `setup-cli` re-run (until then their calls prompt), and a user override under the
documented directory now takes effect while a stray `.agents/skills/` above the repo no longer
does. Builds on v0.19.0's per-CLI provider layer.

**Skills packaging:**

- **Skills as packages, not single markdown files** — adopt the full
  [agentskills.io](https://agentskills.io) layout: a skill is a directory with a `SKILL.md`
  (YAML frontmatter) that bundles its own `scripts/`, `references/`, and `assets/`.
  Concretely this **consolidates the split that exists today** — the flat
  `assets/agent-skills/*.md` (coordination / orchestrator / docs-fetch / cli-setup) and the separate
  `assets/scripts/*.sh` (sweep / broker / docs-fetch) become unified skill packages, each
  owning its helper instead of referencing a bare `.git-paw/scripts/*.sh` path. (`skills.rs`
  already resolves directory-form skills for the `.agents/skills/` + user-override paths;
  this makes the *embedded/exported* defaults true packages too, keeping template vars +
  override resolution intact.)
- **Injected `AGENTS.md` references the packages** — the managed
  `<!-- git-paw:start … -->` block git-paw writes into each worktree's `AGENTS.md` points at
  the packaged skills rather than inlining raw script paths, so the boot block and the skill
  export stay in one place. Export-agnostic per the design principle: the packages carry
  git-paw's own coordination surface; project conventions still come from the consumer's
  `AGENTS.md`.
- **Per-CLI skill templates** — one base template plus per-CLI fragments
  (`assets/cli/<family>/skill.md` and `boot.md`), not four copies; git-paw selects the
  fragment matching each pane's CLI family, falling back to `generic`. Only `claude` and
  `generic` ship; `codex` / `agy` fragments ship only for facts verified against those CLIs.

**Orchestrator subagents (folded in from the old v1.1.0):** give the orchestrator
**context isolation** — long unattended sessions fill the supervisor's context with file
dumps, pane captures, and gate output, so it should **delegate bounded, context-heavy
work to subagents** that each run in their own context window and report back a
*compressed summary*. (The same move a capable model already makes by hand — this very
roadmap was scouted by isolated explorer agents.) Personas are defined per-CLI (for
Claude-family orchestrators, in `.claude/agents/`) with their own role, tool access, and
**model** — the intra-orchestrator analogue of the session-level orchestrator/worker
split. They ride on the skills-package machinery above:

- **explorer** — read-only, broad file/subsystem sweeps; returns a map, not the files.
  Cheap/fast model. Keeps "where is X handled?" out of the orchestrator's context.
- **gate auditors** — one persona per verification gate (spec-audit, doc-audit,
  security); each runs in isolation and returns a compressed verdict, so the five-gate
  run neither floods the orchestrator's context nor serialises — gates fan out into
  parallel isolated contexts. Capable model.

Export-agnostic (per `AGENTS.md`): bundled personas MUST be project-agnostic — a
security-audit persona sources its checks generically, never git-paw's own
cargo / clippy / just toolchain; project-specific gate commands still come from the
consumer's `[supervisor]` config and `AGENTS.md` verification cycle (v0.14.0).

### Sequencing & process

Packaging + per-CLI templates first (the foundation), then `orchestrator-gate-references`
(the three audit gates become package references), then `agent-profiles`, which ships all four
delegate role skills (explorer and the three auditors) together.

**Specs committed 2026-10-19 on `feat/v0.20.0-specs`** (4 changes + 2 added later, all `--strict` valid; new
capabilities `skill-packages` and `cli-agent-profiles`): `skill-package-layout` (behavior-preserving refactor first) →
`skill-package-deploy` (installed at `.git-paw/skills/<pkg>/`; `.git-paw/scripts/` kept as a
deprecated copy until v1.0.0; setup-cli's grants move to the package paths, with a second deny
rule on edits under `.git-paw/skills/`) → `per-cli-skill-templates` → `orchestrator-gate-references`.
**No vendor persona files** (no `.claude/agents/`): they are vendor formats, two of three can't
be verified, and v0.18.0 rules out writing into vendor directories. Personas are **config
profiles** instead (decided 2026-10-19, `agent-profiles` below). `skill-hot-reload` stays deferred.

Added 2026-10-19: `agent-profiles`, which lands last, after `orchestrator-gate-references`. A
profile is a CLI pinned to a model plus a role skill, with a stated purpose; users define as many
as they want under `[profiles.<name>]` with `kind = "worker" | "delegate"`.
- **Worker profiles** launch as normal workers. They are chosen with `--profile`,
  `default_profile`, the picker, or `paw_profile` in spec frontmatter, and recovery relaunches them
  with the same profile. The role skill is added after the coordination skill, never instead of it.
- **Delegate profiles** run headless and one-shot for the orchestrator, through
  `git paw delegate <name> --task …` only. There is no MCP tool, so the MCP server still never
  launches an agent CLI. They reuse v0.18.0's headless runner. Bundled role skills: `explorer`, `spec-auditor`,
  `doc-auditor`, `security-auditor`.
- Delegates are least-privilege: `manual` approval by default, `full-auto` refused, no nesting,
  and no verdict publishing.
- Usage is reported per profile.
- Named "delegate", not "helper", because "helper" already means the bundled helper scripts.

**Re-scoped 2026-10-20 for v0.17.0's classifier retirement and `setup-cli`:**
- `skill-package-deploy`: moving the helpers to `.git-paw/skills/<pkg>/scripts/` also moves
  setup-cli's grants, and adds a deny rule on edits under `.git-paw/skills/`. After the upgrade,
  doctor reports the setup as out of date and the user re-runs `setup-cli` (undo, then
  `--allowlist`); git-paw never rewrites grants itself.
- `skill-override-resolution`: the bundled `cli-setup` skill can't be overridden.
- `agent-profiles`: `broker.sh` and `sweep.sh` refuse to run inside a delegate run.
- The classifier-trust parts are gone.

Added 2026-10-19: `skill-override-resolution`, which lands after `skill-package-layout` and before
`skill-package-deploy`.
- **The defect:** user-level skill overrides have not loaded since v0.4.0, because
  `skills::resolve()` passes `None` for the config directory. The specs, the docs and the code
  describe several different lookup orders.
- **The fix:** one order — the main checkout's `.agents/skills/`, then
  `<config_dir>/git-paw/agent-skills/`, then the embedded default.
  - Agent worktrees are never read.
  - `get_skill` and doctor report the source each skill came from.
  - Overrides change instructions only: an override's `scripts/`, `references/` and `assets/`
    are ignored, so it never decides whether a command runs.

---

## v0.21.0 — Pre-freeze contract + DX hardening (planned)

Theme: the contract-locking cycle — lock the machine/config *contract* and clean up the DX
rough edges so v1.0.0 is purely the freeze + layer-isolation. (v0.23.0's per-CLI audit and
v0.24.0's hardening follow it before the freeze.) Additive / opt-in, with one break:
`dashboard-usage-view` removes the agent roster from the dashboard, which is off by default.

*(DS-7/8/9 from the 2026-10-18 docs sweep shipped in v0.17.0 as `retire-stale-requirements` and
`status-and-picker-truth`.)*

**Approval-friction narrowing (GP-35):** moot. v0.17.0 retired the classifier, so git-paw no
longer rates `echo "$VAR"` or any other command; the agent CLI's own permission rules decide.

**Contract-locking (must be pre-freeze):**

- **Config schema versioning + migration** — add a `version` field to
  `.git-paw/config.toml` plus a migration shim, so the config format can evolve *after*
  the freeze without breaking existing files (git-paw's backward-compat rule: old configs
  must still load). Freezing the config contract without this is a durable regret.
- **Uniform `--json` output** — extend the machine-readable surface beyond `status` to
  `list` / `add` / `remove` / `doctor`, with a stable schema. The v0.18 remote/MCP story
  and the v0.10 agent-friendly theme depend on programmatic output; freezing an
  inconsistent one strands those consumers.
- **Split wire DTOs from internals before freezing them** (code analysis 2026-10-18). Five
  persisted or wire formats are the *same type* as the in-memory model, so freezing the
  format would freeze the internals too:
  - global session JSON (`Session` / `WorktreeEntry`)
  - `config.toml` (`PawConfig`)
  - broker wire / `broker.sh` payloads (`BrokerMessage`)
  - approvals JSONL (`ManualApproval`)
  - `broker.log` (via `BrokerMessage`'s `Display`)

  None has a version field, and shell scripts parse `config.toml` keys (`tomllib`) and
  hand-build `BrokerMessage` JSON, so those names are external contracts too. HTTP clients
  re-declare the broker's response shapes instead of sharing them (`BrokerLogEntry`,
  `StatusAgent`, `AgentStatusRow`). Do: add version fields; add DTO + mapper at those five
  edges only (the per-repo session file and MCP responses already have their own DTOs); add
  one shared `broker::wire` module. Not everywhere — only boundaries that freeze here or
  are exposed through A2A at v1.2.0. Also unify: three slugify rules (`broker::messages::slugify_branch`,
  `logging::sanitize_branch_for_filename`, the scripts' Python `slugify`) into one value
  object; the claim TTL defined in both Rust and `sweep.sh`; and adopt the `domain.rs`
  newtypes, which are barely used (91 `session_name: &str` / `session: &str` signatures).

**Command surface:**

- *(`git paw attach` + striking the phantom `resume` pulled forward to v0.16.0.)*
- **`git paw finish <branch>`** — the merge/complete capstone the pure-manual tier lacks
  (in coordinated mode the orchestrator merges, with the same command). Verify the worktree is clean/committed,
  integrate its changes into a target branch, and remove the worktree — completing the
  manual loop `init → start → work → finish`. A **`--into <branch>`** flag selects the
  destination the changes are moved into (defaults to the default branch; if `<branch>`
  doesn't exist it is created, so the work can be moved onto a fresh branch — e.g. for a
  PR). Merge policy is ff-only by default (config/flag for a merge commit); PR creation
  stays out of scope (v1.5.0 provider integration).
- **GP-09 (ENHANCEMENT) — merge queue / mechanical rebase of disjoint branches** — with N verified branches
  cut from one base, ff-only integration today serialises the wave tail on a
  directive→agent-rebase→watch→merge cycle *per branch* (bottlenecked on agent latency).
  The orchestrator runs `git paw finish --verified --rebase`: it integrates the verified
  branches in order, rebases only provably-disjoint ones itself (the mechanical
  conflict-free case, gated by `git merge-tree`), and sends each skipped branch one batched
  rebase directive. The drive loop never integrates on its own. Pairs with `git paw finish` and the in-tool ordered
  merge-orchestration carried from v0.12 (W15-22b). Dogfood-surfaced 2026-08-31 (peer-4-poker).

**Robustness (carried from the v0.13 intent — that cycle is already under way):**

- **Session-state reconciliation / crash recovery** — self-heal when the world changes
  underneath git-paw (broker died, tmux session killed externally, a worktree `rm`'d by
  hand, stale `session.json`): reconcile `session.json` against actual tmux/git/broker
  state. Extends v0.13's `doctor`; you've been bitten here (the dashboard-orphan CPU
  leak). Makes the frozen baseline trustworthy.
- **State-file locking (concurrency safety)** — take an advisory lock around every
  `session.json` read-modify-write so concurrent writers (a `git paw add` / `remove`
  racing the `--unattended` drive loop, or two invocations) can't clobber each other or
  corrupt the file. Small; the *prevention* complement to reconciliation's *repair*.
*(GP-05 exit-summary + GP-06 purge/stop orphan-loop pulled forward to v0.16.0.)*

**DX polish:** *(shell completions pulled forward to v0.16.0.)*

- **Session resume UX** — better recovery messaging + a worktree diff summary (resume is
  `git paw start`; the phantom `resume` verb is struck in v0.16.0).
- **Additional skills** — refined from real usage. Not specced; deferred with no target
  version (decided 2026-10-29).
- **Configurable layout** — `[layout] max_agents` + `agents_per_row` (+ `--max-agents`),
  beyond the hard-coded 25-agent / 5-col ceiling.
- *(Dashboard tabs / scrollable roster: replaced on 2026-10-19 by `dashboard-usage-view`; the
  roster moves into `git paw status`.)*
- **Config validation (`git paw config check`)** — validate the config (never rewrite it);
  scoped to not duplicate v0.13's `doctor` (diagnose vs. validate); fold into `doctor` if
  the overlap proves total.
- **Terminal robustness** — honour `NO_COLOR`, degrade on non-truecolor / limited-palette
  terminals, and render cleanly at narrow widths and in non-TTY / piped output (CI logs)
  instead of spraying escape codes or breaking the dashboard layout.
- **README as a landing page** — slim `README.md` to *what git-paw is and why it exists*
  (the pitch) plus a demo, install, and a minimal quick-start, then **forward everything
  else to the docs**. Today it carries a ~40-line changelog-style `## Features` dump
  (version stamps, per-config-table descriptions like `[supervisor.conflict]` /
  `[supervisor.learnings_config]`) and verbose supervisor/config prose that duplicates the
  mdBook user guide + CLI/config references — cut it, keeping only pointers. Safe to do
  here because those docs already cover the detail; landing the freeze at v1.0.0 with a
  bloated front door undercuts the "stable, polished" moment.

### Feature — shared dependency store symlinked into worktrees (DX; dogfood tip 2026-08-06)

Each git-paw worktree is a fresh checkout, so per-ecosystem dependency installs
(`node_modules`, `.venv`, vendored deps) get redone in every worktree — slow and
wasteful when N agents run in parallel. Let git-paw symlink a **shared, already-
downloaded** dependency store into each created worktree so agents do not
re-download / re-install. Delivery: a **config** block listing the paths to link
(`[worktree.shared_deps] links`, explicit entries only, no stack presets) plus a note in the
coordination skill telling the worker to use the shared store and not clobber it.

**The caveat that shapes the design — do not ship the naive version.** Only link
resources safe to *share read-mostly* across parallel worktrees: the content-
addressed download **cache/store** (pnpm store, cargo registry, pip cache, go mod
cache). Do NOT symlink a per-project **build/install output** dir that agents write
concurrently (`target/`, a mutated `node_modules`, a per-project `.venv`) — parallel
writes corrupt it. The safe, valuable version links the shared *download* layer
(kills the network cost) while each worktree keeps its own build output; for
ecosystems whose install dir is per-project (Node) prefer a shared store + per-
worktree link farm (pnpm-style) over symlinking `node_modules` wholesale.

Thematically adjacent to v0.15.0 (per-worktree provisioning) but the deliberate
*opposite* choice — v0.15.0 **copies** env files for isolation; this **shares** a
read-mostly dep cache for speed. Placed here as DX polish; specced as
`worktree-shared-dep-cache`.

### Sequencing & process

The contract-locking cycle. Land the **contract-locking** items (config version, uniform
`--json`) first — they gate the freeze — then command surface, robustness, and DX.

**Specs committed 2026-10-19 on `feat/v0.21.0-specs`** (15 changes, plus one added later; all
`--strict` valid). Four
items moved into v0.17.0, so their v0.21 drafts were dropped:
- DS-7 → `retire-stale-requirements`
- DS-8 and DS-9 → `status-and-picker-truth`
- GP-35's `classifier-compound-segments`, later dropped as moot when v0.17.0 retired the
  classifier (008dd148)

The drafts by area:
- **Contract:**
  - `config-schema-versioning`
  - `supervisor-vocabulary-split` (added 2026-10-19, user decision):
    - `[supervisor]` splits into `[orchestrator]` (with `[orchestrator.gates]`), `[workers]` and
      `[drive]`.
    - `default_mode = "coordinated" | "bare"` and `--coordinated` / `--bare` arrive.
    - The root wire identity becomes `orchestrator`.
    - Old names keep working with a warning, and old config files migrate to config version 2.
    - It adds the deprecation registry and the single warning helper (new capability
      `core-deprecation`), which v0.24.0 and v1.0.0 reuse.
    - It lists 78 user-visible uses of "supervisor" in its design.
  - `uniform-json-output`, with the new capability `cli-machine-output`
  - `wire-format-dtos`: `broker.log` is declared a non-contract diagnostic log, and machine
    consumers use `GET /log`
  - `domain-value-objects`
- **Robustness:** `session-state-locking` → `session-state-reconciliation` (`doctor --fix`
  repairs only git-paw's own records).
- **Commands:**
  - `finish-command`: exit `3` when the merge policy refuses; with `--json`
  - `finish-merge-queue` (GP-09)
  - `config-check-command`
  - `start-resume-summary`
  - `configurable-agent-grid`
- **DX:** `terminal-output-robustness`, `readme-landing-page`, `worktree-shared-dep-cache`, and
  `dashboard-usage-view`. That change replaced `dashboard-tabbed-roster` (2026-10-19 decision: the
  dashboard becomes an opt-in usage view).
  - The dashboard is off by default (`[dashboard] enabled`, `--dashboard`). A config migrated
    from version 1 keeps it, and a session keeps whatever dashboard it recorded.
  - It shows tokens and cost per agent and per tier, the orchestrator's one-shot usage, and the
    live event log.
  - The agent roster moves into `git paw status` (status, age, pane; also in `--json`).
  - Bare sessions use the same agent grid and cap (`configurable-agent-grid` edited to match).

**Re-scoped 2026-10-20 for v0.17.0's classifier retirement:**
- Config version 2 drops `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]`
  instead of renaming them into `[workers]`.
- `config check` reports each of the two as a no-effect table.
- `finish` carries `NotAttempted` (exit 2) next to `IntegrationRefused` (exit 3).
- The shared dependency cache keeps explicit `links` only; its stack presets depended on the
  deleted allowlist presets.

Stale premises found while speccing:
- **GP-35.** `echo "$VAR"` already classifies safe; that command escalated because `which` isn't
  allowlisted.
- **Shared dependency store.** Cargo, pip, Go and pnpm download caches are already shared
  machine-wide, so only caches kept inside the repo benefit.
- **The three slugify rules** serve three different purposes and can't be merged without renaming
  worktree directories and log files.
- **`list`** doesn't exist, and `doctor --json` already does.

---

## v0.22.0 — Session recording & replay, rethought (planned)

`git paw replay` cannot do the one thing it exists for — recreate a session. It reads a
`pipe-pane` **raw byte capture** and strips ANSI, but the CLIs git-paw drives are full-screen
TUIs (alternate screen, cursor-addressed redraws, animated spinners + thinking-token
counters). Stripping ANSI from that is unreadable soup — hundreds of spinner-frame repaints
and fragmented, out-of-order content — and it is not a faithful recording either. Confirmed
unusable in the peer-4-poker dogfood (2026-08-31). Rethink what a session record *is* and how
to capture it; the current mechanism is wrong for the target.

Two directions the change chooses between (or layers):

- **Faithful animated replay (asciinema-style).** Keep the raw byte stream but capture it
  *with timing* (asciinema v2 `.cast`, or raw + a timing sidecar) and replay through a player
  that re-executes the escape sequences at speed in a real terminal — a screen recording that
  reconstructs the session exactly. `pipe-pane` already yields the bytes; what's missing is
  timing + a player (shell out to `asciinema play` when present, else a minimal built-in VT
  player). Directly fixes "recreate the session."
- **Semantic timeline (readable / greppable).** Drop terminal-paint capture for a structured
  event log git-paw can already assemble: the broker event stream (agent.status / artifact /
  question / verify), per-commit history, and the CLI's own transcript (e.g. Claude Code's
  JSONL). Render a linear "what each agent did, when" record — the actual audit log, which is
  what most people want from "logs." No escape-code problem by construction.

The design decision: is `replay` an **animation** (screen recording) or a **transcript**
(audit log)? They need different capture; the spinner/thinking-token noise argues the
transcript is the valuable artifact and the animation is a nice-to-have. A layered answer
makes the semantic timeline the default and offers `--cast` / `--raw` for the animation.

Fold in the **logging-wiring gap** while here. The original gap, logging wired only on
`--from-specs`, was fixed in 02649dc (2026-08-03). What remains: agents added with `git paw add`
and panes respawned by cold recovery are not captured, and every run of a repo appends to the same
files. So "what is captured, and when" is part of the same rethink.

**Freeze tie-in (why this is placed pre-1.0):** `git paw replay` is a shipped command that
freezes into the stable contract at v1.0.0. Freezing a known-broken command is the mistake
v0.21's "don't freeze a bloated front door" guards against. This cycle either makes replay
work *before* the freeze (placed here) — or, if you defer the rethink post-1.0, v1.0.0 must
**deprecate / hide `replay`** rather than freeze it as-is. Decided: placed here, with the
fallback below.

### Sequencing & process

A feature-shaped cycle, not contract-locking — but freeze-adjacent (see the tie-in). Prototype
the semantic timeline first (it reuses data git-paw already has) and treat the asciinema-style
animation as the optional second layer.

**Specs committed 2026-10-19 on `feat/v0.22.0-specs`** (3 changes, all `--strict` valid):
- `session-record-capture`:
  - a record directory per run with `manifest.json` and `events.jsonl`
  - capture on every path that creates an agent, `add` and recovery included
  - owner-only file permissions; deleting old records is opt-in
- `replay-semantic-timeline` (**BREAKING**: drops `--color` and the ANSI-stripped output): the
  default `replay` becomes a merged timeline of broker events, commits and the CLI's transcript,
  with `--json` and `--raw`.
- `replay-terminal-cast`: optional asciinema v2 recording, played back by calling `asciinema
  play`. There is no built-in player, because a VT player needs the unapproved `vt100` crate.

If `replay-semantic-timeline` misses the v0.24.0 feature freeze, v1.0.0 hides and deprecates
`replay`.

**Decided 2026-10-29: v0.22 adopts v0.21's machine-output and terminal-output contracts.**
`replay --json` carries `schema_version` and joins the `cli-machine-output` roster, and
`--raw` / `--cast` replace `replay --color` as `core-terminal-output`'s requested-escape
exception. The v0.22 specs still need the MODIFIED deltas for both capabilities.

---

## v0.23.0 — Per-CLI support audit (planned)

Theme: before the freeze locks which CLIs git-paw supports, find out what each one actually
does end to end. Today `KNOWN_CLIS` detects 17 binaries by name, but there is one built-in
prompt profile (`claude_prompt_profile`) and one built-in session-id/resume row (`claude`).
Every other CLI runs on Claude's prompt shapes unless the user overrides them. v0.19.0 builds
the per-CLI provider layer; this cycle checks it CLI by CLI and publishes the result.

- **Audit every detected CLI against the worker lifecycle:**
  - detection: is the binary actually the agent CLI? (DS-6: `cursor` is usually the editor)
  - launch, including per-worker args
  - trust and first-run dialogs, launch readiness
  - boot-block delivery (MCP or helper scripts)
  - prompt-shape detection: permission prompts, the live prompt, stuck shapes
  - approval: an explicit `sweep.sh approve` answers once (option 1 is present and is not a
    durable grant)
  - send and submit: paste buffer, follow-up Enter
  - native auto-mode, sandbox and hooks
  - session id and cold resume
  - the headless one-shot that v0.18.0's escalation hand-off uses
  - exit and crash detection
  - usage and cost reporting
- **Support tiers**, published as a matrix on the supported-CLIs page and in `git paw doctor`:
  - **first-class**: every lifecycle step verified, with recorded fixtures in CI
  - **supported**: works, with the gaps documented
  - **launch-only**: the generic profile; git-paw launches the CLI and nothing more

  Each row records the CLI version it was verified against.
- **A per-CLI conformance suite.** Captured prompt-shape fixtures per CLI version, replayed
  through git-paw's detectors and `sweep.sh approve`'s option-1 guard with no network, so it
  runs in CI. Plus an opt-in live
  smoke (a `doctor --live` arm) for the CLIs installed on the machine.
- **Fix the small findings in the cycle.** Larger ones go to v0.24.0 or lower the CLI's tier. A
  CLI that can't reach launch-only is dropped from detection.
- **Freeze tie-in:** v1.0.0 freezes the tier definitions and the first-class list, not the whole
  detection list.

### Sequencing & process

After v0.19.0 (provider layer), v0.20.0 (per-CLI skill templates) and v0.22.0. Build the
conformance suite first, run the audit per CLI, then publish the matrix. Spec-first: one change
for the audit harness and support matrix. The per-CLI findings are filed as they come.

**Specs committed 2026-10-19 on `feat/v0.23.0-specs`:** `cli-support-audit` (`--strict` valid; new
capability `cli-support-matrix`).
- **Lifecycle steps.** There are twelve, each verified, gap or unverified. A pure function
  computes the tier from them.
- **Fixtures.** They live at `tests/fixtures/clis/<binary>/<version>/` and are captured with a
  hidden `git paw __capture-fixture`.
- **Live smoke.** It needs an explicit `doctor --live --cli <name>`, because plain `--live` never
  starts an AI CLI.
- **One compiled table.** It feeds the roster, doctor, the docs page and the README, with parity
  tests.

Code facts:
- Four CLIs already carry approval-flag rows (claude, agy, codex, qwen).
- `is_live_prompt` and `classify_pane_readiness` ignore per-CLI overrides today.

**Added 2026-10-29 (GP-88): opencode.** A spike on opencode 2.1.2 found it resolves Claude's
profile, its idle screen has none of Claude's markers, a "what's new" notice takes input after an
update until dismissed with Esc, and its free-usage limit appears as a dialog. `cli-support-audit`
gives it an `opencode` family, with a profile and start-up dialogs built only from reviewed
captures (capturing them is the first, blocking task). The launch gate learns provider-declared
notices (dismissed once, then readiness is polled) and blocking dialogs (never answered; the
launch fails loudly).

---

## v0.24.0 — Pre-freeze hardening (planned)

Theme: the last cycle before v1.0.0, and a **feature freeze**. No new features, only fixes,
reliability and polish on the surface v1.0.0 will lock. It also gives the v0.18.0 relocation the
full dogfood cycle it needs before the freeze.

- **Fix the v0.23.0 audit findings.**
- **Test-suite reliability.** De-flake the known load-sensitive tests: pause e2e port release,
  the stuck-detection dedup window, the worktree env-provisioning CLI launch, and the llvm-cov
  profraw merge under a live session. `just check` must pass cold, not only next to a live
  session.
- **Upgrade path.** A back-compat matrix: configs, session files and broker logs written by
  v0.16.0 onward load and resume under the release candidate.
- *(Unattended reliability: the heartbeat early exit that stranded detached runs was fixed in
  v0.18.0 by `unattended-heartbeat-progress`.)*
- **Errors and exit codes.** Every user-facing error is actionable, and exit codes are
  consistent. This feeds v1.0.0's contract.
- **Five-gate audit of the release candidate:**
  - every spec scenario maps to a test
  - docs consistent across the four layers
  - a security-and-safety review of the whole exported surface
  - coverage ≥ 80%
  - `deny` and `audit` clean
  - MSRV at or below the cargo-dist runner's rustc
- **Release-candidate dogfood.** A full external-orchestrator cycle on a repo other than
  git-paw, on at least two first-class CLIs.

### Sequencing & process

Findings-driven, so most of the change set is filed during the cycle. Spec the known items up
front: de-flaking, the back-compat matrix and the five-gate audit. Anything that needs a new
feature waits for v1.1.0.

**Specs committed 2026-10-19 on `feat/v0.24.0-specs`** (now 3 changes, all `--strict` valid):
- `test-suite-determinism`. The release-candidate gate and dogfood are a process checklist in
  its design.
- `upgrade-compat-matrix`, with the new capability `core-upgrade-compatibility`.
- `unattended-heartbeat-progress` moved to v0.18.0 on 2026-10-25 (P4P's PP-24: every unattended
  wave stops pumping after 25 minutes).
- `error-exit-code-contract`, defining exit codes `0`/`1`/`2`/`3`:
  - `2` means nothing was attempted, including clap's argument errors and a declined `purge`.
  - `3` means the command ran but stopped short and reported the outcome. That covers an
    unattended wave that didn't complete and a refused `finish`.
  - `--from-specs` gets a deprecation warning.

**Re-scoped 2026-10-20 for v0.17.0's classifier retirement:**
- The heartbeat counts progress only when a branch tip moves or a phase advances; re-escalating
  the same prompt doesn't count (shipped in v0.18.0).
- An `--unattended` run that escalated any prompt exits `3`, even if all its work completed.
- `NotAttempted` (from v0.17's `setup-cli`) is the exit-2 variant.
- There is no `cli-unavailable` outcome after v0.18.0. A coding agent's unavailable CLI is an
  escalation.

Root causes found while speccing:
- **The "detached heartbeat exit"** has nothing to do with the terminal. `HEARTBEAT_INTERVAL`
  (25 min) counted from the loop's start, so any run longer than 25 minutes stopped pumping.
  Fixed in v0.18.0.
- **The flaky tests** aren't failing from load alone. Five test files use the default tmux socket,
  some tests use whichever `git-paw` is on `PATH`, and the env-provisioning probe reads the
  developer's own shell startup files.
- **CI's cold-start job** is `continue-on-error`, so a cold failure never blocks a merge.

The classify→send TOCTOU (specced for v0.17.0 as `approval-send-identity-check`) is moot: v0.17.0
retired the classifier and dropped that change, and `sweep.sh approve` re-confirms the live
prompt itself and only ever sends option `1`.

Known approval follow-ups that stay here, not yet specced (two more, the `sweep.sh approve` →
`__classify` gap and the `auto_approved` audit message, went away with v0.17.0's classifier
retirement):
- **`approval_dedup_key` has no file-path fallback**, so all of one agent's file-operation
  escalations collapse into a single alert within the 5-minute window.
- **Unverified:** if Claude Code confirms on the option digit alone, the trailing `Enter` that
  `sweep.sh approve` sends could land on a queued next prompt.

---

## v1.0.0 — Stable CLI freeze

CLI contract freeze — no breaking changes after v1.0.0. Deliberately thin: v1.0.0 locks
a surface already proven across the 0.13–0.24 cycles. See Scope below.

> **Updated for v0.18.0's orchestrator relocation (shipped 2026-10-29).** The supervisor pane
> is gone, so the layer model below is `layer-isolation`'s, and v1.0.0 carries **several**
> breaks (see the specs note under Scope).

### Theme: pure-manual must give standalone value

v1.0.0's primary theme is making the v0.1.0-style **pure manual**
mode a first-class shipping experience. Every advanced feature is
a config opt-in that *layers on top* of a working manual baseline.
Each layer delivers value standalone:

```
1. Manual (always)      — multi-tmux CLI orchestrator, branch picker,
                          worktrees, session logging, replay,
                          per-pane CLI dispatch.
                          [no broker, no skills, no coordination]
2. + Skills             — agent-skills templates with template vars +
   (docs_base_url)        override resolution. Content layer; no
                          broker dependency.
3. + Broker             — broker host, message types, delivery,
   ([broker] enabled)     watcher, git hooks, broker.sh. The
                          dashboard is an optional view of it.
4. + Coordinated        — inter-agent rules and the orchestrator
   (--coordinated)        procedure (gates, verdicts, merges).
                          Requires Broker.
5. + Unattended         — the drive loop. Requires Coordinated.
   (--unattended)

MCP is a client, not a layer: it installs nothing.
```

Each layer is *additive and isolated* — it installs only its own surface, and disabling
it removes exactly that surface — so a project can run pure-manual sessions
indefinitely and never opt into any of the higher tiers. The CLI
freeze must preserve the pure-manual ergonomics first; advanced
features can evolve faster post-v1.0.0 since they're opt-in.

The mode-aware install/inject fixes (issue 10) are this theme's
operational backbone — they close the gap where the layers are opt-in on
paper but the install path still leaks: `git paw init` installs every
helper script whatever the config enables, and a coordinated session
without `[broker] enabled` gets a boot block pointing at a `broker.sh`
that was never installed.

### Scope — what v1.0.0 actually ships

Deliberately small; everything else has shipped earlier or lands in the 0.14–0.24 / 1.1
cycles.

1. **CLI contract freeze** — audit and document every command, flag, config field,
   **exit code**, and wire format; semver guarantees from here on. Exit codes are part of
   the contract (scripts/CI depend on them — `git paw doctor && deploy`, `git paw finish`
   returning non-zero when the merge isn't fast-forwardable), so they're audited,
   documented, and frozen too.
2. **Remove the `--from-specs` alias** — drop the v0.5.0 compatibility shim; passing it
   emits a clear "renamed to `--from-all-specs`" migration error. One of several breaking
   changes (see below).
3. **Layer isolation — each layer installs only its own surface (the substantive work).**
   The install/inject path must compose *exactly* the layers you enabled and nothing
   more. This is broader than "keep pure-manual clean" (the zero-layer extreme) — it holds
   at every tier: broker-on/supervisor-off gets broker surface but no supervisor language;
   skills without a broker get no broker-curl examples; and so on. Concretely:
   - install hooks and helper scripts only for enabled layers (hooks are already gated on
     `[broker] enabled`; `init`'s helper scripts are not);
   - render the `AGENTS.md` git-paw section from the *set of enabled layers*, each
     contributing only its own bits;
   - render skill content per enabled layer (no broker-curl instructions without a broker,
     no supervisor discipline without a supervisor).
   Plus a **modes** docs page (each layer documents its own prerequisites + surface) and a
   **CI smoke matrix** asserting each layer combination installs *exactly* its own surface
   — the broker-off / pure-manual case being the strictest check.
4. **Deprecation policy (mostly docs — it defines what the freeze means).** A documented
   rule for how flags and config deprecate *after* the freeze — a warning for N minor
   versions before removal — plus a small warn-helper so deprecations actually emit a
   warning. This is what makes "semver from 1.0" a real commitment, not a slogan. (The
   **security disclaimer** moved to v0.15.0, bundled with the sandbox.)

**Specs committed 2026-10-19 on `feat/v1.0.0-specs`** (3 changes, plus one added later; all
`--strict` valid):
- `cli-contract-freeze`, with the new capability `core-stability-contract`:
  - a stability chapter and a contract manifest checked by a guard test
  - the exit-code table: `0`/`1`/`2`/`3`, as settled by v0.24.0
  - the v0.23.0 tier definitions and first-class list
  - the **new** vocabulary only (see below)
  - a deprecation policy: warn for at least two minor releases, remove only at a major, using
    v0.21.0's deprecation registry and its single warning helper. Every pre-1.0 deprecation is
    resolved at the freeze.
- `remove-from-specs-alias` (**BREAKING**).
- `layer-isolation` (**BREAKING** behavior):
  - the layer model without the supervisor pane: Manual → Skills → Broker → Coordinated →
    Unattended; the dashboard is the broker's view and MCP is not a layer
  - coordinated sessions always get the broker
  - `init` adds and removes files per enabled layer, and a CI matrix checks it
- `remove-supervisor-vocabulary` (**BREAKING**, added 2026-10-19 on the user's decision):
  - `--supervisor` / `--no-supervisor` exit `2` with "renamed to `--coordinated` / `--bare`".
  - The `supervisor` skill alias and the `supervisor` wire identity are removed.
  - The `supervisor-*` capabilities are renamed.
  - A guard keeps "supervisor" out of help text, docs and assets.
  - Old config files keep loading through 1.x, with a warning, via the v0.21.0 migration.

**The supervisor vocabulary is retired before the freeze.** v0.21.0's `supervisor-vocabulary-split`
introduces the new names, and v1.0.0 removes the old ones:

| Old | New |
|-----|-----|
| `[supervisor]` | `[orchestrator]` with `[orchestrator.gates]` and `[orchestrator.escalation]`, `[workers]`, `[drive]` |
| `enabled` | top-level `default_mode = "coordinated" \| "bare"` |
| `--supervisor` / `--no-supervisor` | `--coordinated` / `--bare` |
| root wire identity `supervisor` | `orchestrator` |
| broker message `supervisor.verify-now` | `orchestrator.verify-now` |
| MCP `get_session_status` field `mode` | `session_mode` |

Nothing is now scheduled for removal at 2.0.0.

**Open (`cli-contract-freeze`):** if v0.22.0's replay rework has not shipped by the v0.24.0
freeze, `replay` does not enter the stable surface: v1.0.0 hides it from `--help` and the
manifest and registers it deprecated or experimental, so it can be redesigned in 1.x.

`--from-specs` is not "the one breaking change". `layer-isolation` and the vocabulary removal break
too, and v0.17.0 already removes `stop --force`.

The post-commit hook does not leak; it is gated on `[broker] enabled`. The real leaks:
- `init` installs all four helper scripts unconditionally.
- A coordinated session without `[broker] enabled` has had no broker host since v0.18.0 (the
  host starts only when the broker is enabled), yet its boot block points agents at a
  `broker.sh` and hooks that were never installed. `layer-isolation` gives every coordinated
  session the broker.

---

## v1.1.0 — Multi-repo registry (planned)

First post-freeze feature — additive, opt-in, outside the v1.0.0 CLI contract. It's the
single-node **precursor** to v1.4.0's cross-repo coordination (orchestrator↔orchestrator over
A2A). v1.2.0's A2A face is per session and doesn't need it.

Collapse the per-repo MCP-client-config burden into **one** MCP server entry that knows
about all your git-paw repos:

- A registry file (`<data dir>/git-paw/repos.json`, next to `sessions/`), written by
  `git paw init`, listing each managed repo.
- The MCP server exposes **`list_repos()`** and takes a **`repo`** parameter on every
  tool, so a single client-config entry serves N repos instead of N entries.
- The same registry resolves **per-repo CLI preferences** through v0.19.0's `cli-provider`
  record. (The trust-folder half was dropped, decided 2026-10-29.)

(Promoted from the mcp-server proposal's Pattern 3; was v1.0.0 issue 18.)

Why post-freeze / why here: it's additive (a new registry file + MCP surface, no change
to existing single-repo behavior), so it doesn't belong in the freeze; and it's the
foundation v1.4.0's cross-repo coordination needs.

### Sequencing & process

After the v1.0.0 freeze. Registry file + `list_repos()` + the `repo` tool parameter
first; per-repo CLI resolution second.

**Specs committed 2026-10-19 on `feat/v1.1.0-specs`** (2 changes, all `--strict` valid, new
capability `core-repo-registry`):
- `multi-repo-registry`:
  - `repos.json` lives in git-paw's platform data directory, next to `sessions/`, not at
    `~/.git-paw/`. Only `git paw init` writes it, and `[mcp] register = false` opts a repo out.
  - The multi-repo surface needs the explicit `git paw mcp --all-repos` flag.
  - `repo` accepts only a registry id, never a path.
  - Other repos are read-only; write tools act only on the repo the server started in.
- `registry-repo-profiles`: read-only CLI preferences per repo, resolved through v0.19.0's
  `cli-provider-record`. **Decided 2026-10-29: the trust half is dropped** ("trust-folder
  state" does not exist in the code, and v0.19.0 dropped trust pre-acceptance); the spec still
  carries it and has to be trimmed.

---

## v1.2.0 — A2A interface (planned; was v2.0.0)

Expose a session to A2A clients through the stable A2A protocol, version `1.0`: v1.0.0
was released 2026-03-12, and the normative text and proto (`lf.a2a.v1`) are taken from patch
v1.0.1 (2026-05-28).
Sources: https://a2a-protocol.org/latest/specification/ and
https://github.com/a2aproject/A2A/releases.

**A2A is an opt-in external face on the broker, not a replacement for it.** The original plan
was to swap git-paw's broker JSON for A2A. Researching the stable spec showed why that can't work:
- A2A gives an agent no way to publish its own status.
- It has no broadcast.
- It declares its messages unreliable (§3.7).

The broker bus, `broker.sh` and the worker-facing skills stay. A2A clients see the session through
one Agent Card, and each worker appears to them as an A2A task. Corrections to the old plan:
- The card lives at `/.well-known/agent-card.json` (renamed in A2A v0.3.0), and there is one
  card per server, not one per agent.
- The lifecycle has 8 task states, including input-required. Task ids come from the server, and a
  finished task accepts no further messages. "send → get → complete" doesn't describe it.
- The skills on the card describe the session to A2A clients. They don't replace the
  worker-facing skill text or `broker.sh`.

**Specs committed 2026-10-19 on `feat/v1.2.0-specs`** (6 changes, all `--strict` valid, new
capability `broker-a2a`). The listener's `Host`/`Origin` guard (`broker-listener-origin-guard`)
closes a DNS-rebinding exposure in the shipped broker, so it moved into v0.17.0 and this cycle
builds on it:
- `a2a-wire-model`: hand-written A2A types, a pure mapping from broker events to tasks, and a
  git-paw extension.
- `a2a-task-lifecycle`: off by default (`[broker.a2a]`), loopback only, JSON-RPC at `/a2a`. Every
  call needs the session's root credential. Clients can get, list and cancel tasks and send to
  existing ones, but can't create tasks or control processes.
- `a2a-agent-card`: one session card with three capability skills.
- `a2a-legacy-coexistence`: the current broker messages and helper scripts stay stable and are
  not deprecated for the rest of the 1.x line. A2A is a layer that installs nothing in worktrees.
- `a2a-task-streaming`: live updates over SSE. **Blocked** until `futures-util` is approved as a
  direct dependency (it is already in `Cargo.lock` through axum).
- `a2a-migration-guide`.

**Renumbered from v2.0.0 to v1.2.0 (user decision 2026-10-19).** Nothing in this cycle is
breaking: the origin guard moved to v0.17.0, the legacy wire is not deprecated, and after the
supervisor vocabulary was retired at 1.0.0 nothing is scheduled for removal at 2.0.0. Semver makes it
a minor release. 2.0.0 is kept for a future breaking change. The follow-ons renumber with it:
cross-repo coordination becomes v1.4.0 and provider integration becomes v1.5.0 (v1.3.0 is the headless terminal backend).

**Not specced:** the MCP HTTP transport. HTTP+SSE has been deprecated since MCP 2025-03-26; its
replacement, Streamable HTTP, was reshaped again on 2026-07-28 (the pinned `rmcp` 1.7 may predate
that). There is no demand signal, and a loopback endpoint can't reach web clients such as ChatGPT
Web anyway.

**Open:**
- the official Rust SDK (`a2a-lf` 0.4.2, still 0.x) vs hand-written types (recommended:
  hand-written);
- broker signals for a worker being removed or failing, which A2A's `canceled` / `failed` /
  `auth-required` mappings need.

---

## Future (post v1.2.0)

| Version | Theme | Key Features |
|---------|-------|-------------|
| v1.3.0 | Headless terminal backend | A second `TerminalBackend` adapter: workers run their CLI's non-interactive mode with no terminal, approvals come from the CLI's own permission policy or sandbox, and output streams to the session record. Opt-in and additive; tmux stays the default. Specced 2026-10-19 on `feat/v1.3.0-specs`, see below. |
| v1.4.0 | Multi-repo | Orchestrator-to-orchestrator coordination via A2A, cross-repo dependency graphs, tmux windows per repo |
| v1.5.0 | Provider Integration | Git provider abstraction (GitHub/GitLab/Bitbucket/Azure DevOps), `--from-issues`, auto PR |
| v1.6.0 | Remote workers | A worker whose surface is a vendor's cloud session instead of a local pane: git-paw starts the session from the branch's pushed base, relays its questions to the orchestrator, and commits its final patch on the agent's local branch, so verify, gates and merge run unchanged. Jules first, proved by a PoC; Codex cloud, Copilot's coding agent and Claude Code cloud sessions later on the same seam. Added 2026-10-28, see below. |

**v1.3.0 specs committed 2026-10-19 on `feat/v1.3.0-specs`** (3 changes, all `--strict` valid, new
capability `session-headless-backend`), in order:
1. `headless-backend-adapter`: the `HeadlessBackend` adapter behind v0.18's terminal port. A worker
   is a conversation run one turn at a time through the CLI's headless form, by a hidden
   `__headless-runner` started with `setsid`; it exits when its queue is empty, so there is no
   daemon. Not user-selectable yet.
2. `headless-drive-adaptation`: the drive loop and broker host on surfaces with no screen. A failed
   turn is escalated at once (`turn-failed`, or `cli-unavailable` when the CLI's marker shows);
   only an interrupted turn gets one continuation message, and a second interruption escalates
   (`turn-interrupted`). The broker host wakes a worker for messages addressed to it.
3. `headless-backend-selection`: `start --backend <tmux|headless>` and a top-level
   `default_backend`, behind a safety gate checked before anything is created:
   - the CLI needs a headless form plus session-id and resume templates (only the Claude family
     today);
   - the approval level can't be `manual`;
   - `full-auto` needs a declared sandbox;
   - git-paw never raises a level.

   Plus doctor, a `headless-worker` column in the support matrix outside the tiers, MCP, and docs.

Not specced: a detached drive-loop service, mixed-backend sessions, switching backend on recovery,
a dashboard for headless sessions, and native Windows.

**v1.6.0 — Remote workers (added 2026-10-28; specced on `feat/v1.6.0-specs`, see below).** Designed first as a route in the
Fleet toolbox (llm-toolbox `fleet/`): design
`<llm-toolbox checkout>/.claude/worktrees/fleet-catalog/docs/superpowers/specs/2026-10-24-fleet-remote-workers-design.md`,
plus its reviewed plan in `../plans/2026-10-24-fleet-remote-workers.md` (both uncommitted, by
choice). **Fleet builds it first, as the proof of concept** (decided 2026-10-28): a real Jules route
in Fleet's runner, end to end. This version is git-paw's own, built on what that proves. Fleet keeps
choosing routes (which free route, under which terms and limits). Once git-paw has remote workers,
Fleet's git-paw mode hands remote tasks to it like local ones.

Theme: **a worker whose surface is a vendor's cloud session.** The unit of work stays the local
worktree and branch. The session starts from the branch's base on GitHub, and its final patch is
applied and committed on the agent's branch. Everything after that commit (verify, gates, verdicts,
merge) is unchanged.

Placement: after v1.3.0, because a remote session is a surface with no screen and reuses the
headless adapter's drive-loop and broker adaptations. It needs neither multi-repo (v1.4.0) nor the
provider abstraction (v1.5.0), so it can move ahead of both. It stays after the freeze on purpose:
the Jules API is `v1alpha`, and an alpha vendor API must not sit under the 1.0 contract.

Shape (the Fleet design's decisions and its review fixes, carried over; the spec settles them):
- **A third `TerminalBackend` adapter, or a sibling port.** `spawn_surface` creates the session,
  `send_text` is `:sendMessage`, `capture_tail` reads the newest activities, `liveness` is the
  session state, and `list_surfaces` reports the local worktree the patch lands in as the cwd.
  `send_key` and `attach` mean nothing here (attach can print the session URL), and the API has no
  cancel. **v0.18.0 is locked** (user, 2026-10-28). If its port assumes every surface is a local
  process with a screen, the fix is a change in this version or a new one, never in v0.18.0.
- **Mixed-backend sessions.** The point is to offload some branches while others run in panes, so
  this version needs what v1.3.0 left unspecced: one session holding tmux and remote workers.
- **git-paw never pushes.** A worker is eligible only when `origin` is on GitHub, the base branch on
  GitHub equals the local tip, and the branch has no local commits. Otherwise the launch refuses it
  before anything is created, like `headless-backend-selection`'s gate.
- **No pull request by default.** Sessions are created without `AUTO_CREATE_PR`, and the patch comes
  back over the API. Opening pull requests belongs to v1.5.0.
- **Questions escalate.** `AWAITING_USER_FEEDBACK` becomes an `agent.question`, and the
  orchestrator's answer goes back through `:sendMessage`, once per distinct question. A plan
  approval escalates the same way: git-paw approves nothing.
- **A failed verify gets one continuation message** carrying the failing output to the same
  session, then escalates (v1.3.0's rule).
- **The patch is checked before it touches the worktree.** Refuse paths under `.git/` or
  `.git-paw/`, absolute or `..` paths, `.gitmodules`, symlinks, submodules, and anything over 5 MB.
  Apply with `--index`, then `--3way`, and leave the tree clean on failure.
- **Recovery reuses the session.** The session name is stored in the session file; recover and
  `--resume` re-attach to it and never start a second one. A per-session time cap (4 h in the Fleet
  design) ends git-paw's follow and leaves the session to the human.
- **Limits.** Per-plan daily tasks and concurrency come from config (Jules: 15/3 free, 100/15 Pro,
  300/60 Ultra, over a rolling 24 h). A limit error stops new remote launches for a cool-down, and
  v0.19.0's usage attribution counts remote tasks, not tokens.
- **Credentials.** The key comes from the environment only (`JULES_API_KEY`), the host is pinned,
  redirects are refused, and the key never reaches a log, the session file or an error.
- **Terms first.** An adapter ships only after its vendor's terms are read. Jules's
  (<https://jules.google.com/legal>, behind a sign-in) were read on 2026-10-28 (see Terms below).
  Also known: the API is documented for automating tasks, users must be 18 or older, and paid
  plans are for personal @gmail.com accounts only.

**PoC: Fleet's Jules route** (llm-toolbox `fleet/`, built from the plan above from 2026-10-28). It
runs real Jules sessions from Fleet's runner: eligibility, creating the session, polling, answering
questions, applying and verifying the patch locally, one retry, limits and recovery. Its first live
runs answer what the Jules docs don't, before this version's spec hardens:
1. Does the API work on the free plan?
2. What does a limit error look like (status and body)?
3. Do activities come back in `createTime` order, and is the newest patch cumulative?
4. Does a session without `AUTO_CREATE_PR` still hand back a patch, in the activities or in
   `outputs`?
5. How long does a small task take, and what does a question look like?

Fleet creates its sessions with `AUTO_CREATE_PR`, so question 4 needs one extra session created
without it.

Status: built in Fleet on 2026-10-28 (llm-toolbox `worktree-fleet-catalog`, 9 commits, 697 tests
against a fake Jules, opus-reviewed with all six Important findings fixed).

Terms (2026-10-28): Jules's terms link goes to its legal page, which holds only the Jules Privacy
Notice (last updated 2025-10-07). Nothing there limits automated use, and the FAQ says Jules works
autonomously. Sessions on public repos may train Google's models. Fleet switched the free plan on
for public hobby repos. The live runs can go ahead once the user installs the GitHub app and
exports a key.

Lessons from the PoC's review, for this version's specs:
- **Anything that gates a new session must exempt picking one up again.** Fleet's daily budget
  first blocked a resume and a verify retry from reusing their own session, which was then
  abandoned.
- **Every session that is still running is either reused or watched,** never left behind, including
  when a resume can't use the route (no key) or the task moves to a local worker.
- **A stop during verify says nothing about the patch.** Keep it as "verifying", and verify again on
  recovery.
- **Replacing a patch must be able to put the earlier one back** if the new one is refused or
  doesn't apply.
- **Never apply a patch to a dirty worktree, and never clean one.** A local worker's uncommitted
  changes are its output.
- **Compare refused paths case-insensitively** (`.fleet`, `.git`, `.gitmodules`): macOS's default
  disk is case-insensitive.
- **Refuse a key with a line break before any request.** The HTTP layer quotes a bad header value
  in its error.

**Not in this version:**
- pull-request creation (v1.5.0);
- adapters other than Jules, which follow on the same seam as their terms are read: Codex cloud
  (`codex cloud exec`), Copilot's coding agent (`gh agent-task create`) and Claude Code cloud
  sessions (`claude --cloud`);
- repos not hosted on GitHub;
- choosing between free and paid routes, which is Fleet's job.

**Specs committed 2026-10-28 on `feat/v1.6.0-specs`** (now based on v0.18.0; 5 changes, one commit
each, all `--strict` valid; the whole-stack archive simulation onto the v0.18 tip passes). In order:
1. `terminal-port-surface-capabilities`: v0.18.0's port does assume a local process with a screen,
   and v0.18 is locked, so the fix lands here. Surface capabilities are answered per surface,
   interactive-only operations return `Unsupported`, callers check first, and a surface with no
   layout position resolves by its owner.
2. `remote-surface-adapter`: a third terminal adapter over a vendor-neutral `RemoteAgentService`
   port. A follow thread in the broker host turns questions and plan approvals into
   `agent.question` and sends one answer per question. `[remote] follow_hours` (default 4) caps it,
   and there is no cancel.
3. `mixed-backend-sessions`: one session holding tmux, headless and remote workers, with
   `--worker-backend` and a backend recorded per worker.
4. `jules-remote-vendor`: the only vendor. Task 0.1 (read and record Jules's terms) is done
   (2026-10-28). What blocks implementation now is task group 1, the PoC's live-run
   confirmations (1.1–1.6): one per open question in its design, plus the published plan limits.
5. `remote-work-landing`: the eligibility gate (checked with `git ls-remote`; git-paw never
   pushes), patch checks, `--index` then `--3way` apply, the commit on the agent's branch, one
   verify continuation, re-attach on recovery, and a doctor check.

Decided 2026-10-28:
- HTTPS goes through the system `curl` via `CommandRunner`, with the key on stdin, so no new crate.
- Remote workers come from a spec and require the broker.
- A failed session's partial patch is escalated, never committed. `remote-work-landing` should
  say so explicitly, with a scenario.
- An exported `JULES_API_KEY` is readable by local workers; this is documented, and the key is
  stripped from git commands.

Still open: if the live run shows no patch without `AUTO_CREATE_PR` (question 4), either pull
pull-request support forward from v1.5.0 or fetch the vendor's branch. No version number appears in
requirements, scenarios, tasks or change names, so a renumber edits proposal prose only.

---

## Released so far (v0.1.0 → v0.16.0)

Detail lives in the repo's spec history. Brief recap below for context.

### v0.1.0 — Foundation
Initial public release. Core `git paw` CLI and worktree-based agent
session model.

### v0.2.0 — Specs and Sessions
`git paw init`, `--from-specs` driver (OpenSpec + plain Markdown),
per-worktree `AGENTS.md`, session logging and replay.

### v0.3.0 — Broker and Dashboard
HTTP broker (axum/tokio, opt-in), cursor-based polling, single
`coordination.md` skill template with `{{BRANCH_ID}}` and
`{{GIT_PAW_BROKER_URL}}` template variables, read-only dashboard TUI,
`assume-unchanged` on per-worktree `AGENTS.md`. 542 tests.

### v0.4.0 — Supervisor Mode
Supervisor agent that auto-starts all coding agents, verifies their
work, and escalates ambiguous decisions. Three-level control surface
(chat with supervisor / dashboard / tmux panes). Spec audit, hook
injection, merge-time verification, topological merge ordering.
Interactive prompt inbox. New broker message types `agent.verified`
and `agent.feedback`.

Scope additions from v0.3.0 dogfood: CLI-agnostic auto-approve patterns,
boot-prompt standardization, agent skills migrated to agentskills.io
format, recover-session fix, terminal-status stickiness.

Deferred out: conflict detection (→ v0.5.0), per-CLI specialisation
(→ v1.0.0).

Behaviour changes worth knowing about: pre-push hook now blocks
`git push` from worktrees when broker is enabled (supervisor pushes);
coordination skill template syntax moved from `${VAR}` to `{{VAR}}`.

### v0.5.0 — Forward-Looking Coordination, Learnings, and Governance
Supervisor moves into its own tmux pane (`supervisor-as-pane`); merge
orchestration moves from Rust into the supervisor skill. New
`agent.intent` broker variant and rewritten `coordination.md` teach
agents to publish intended file touches *before* editing; conflict
detection catches forward, in-flight, and ownership-violation shapes.
Spec Kit joins OpenSpec and plain Markdown as a third spec backend
(auto-discovery, mid-flight `- [x]` writeback, `[P]`-aware parallel/
consolidated worktree layout). Spec-launch surface restructured:
`--from-all-specs` (canonical) plus `--specs NAME[,NAME...]`
(narrowing + multi-select picker); `--from-specs` kept as a hidden
alias slated for v1.0.0 removal. New `--no-supervisor` single-session
override. Learnings mode writes `.git-paw/session-learnings.md`
(file-only). `[governance]` path-pointer config injects ADR / DoD /
security / test-strategy / constitution paths into the supervisor's
boot prompt. Dashboard prompt-inbox panel removed. New `git paw pause`
/ `resume` soft-stop primitive. v0.4.0 panic-surface backlog
(high + medium) closed; scenario coverage 86 → 97.3 %.

Mid-cycle dogfood scope additions: `from-specs-launch-fixes`,
`prompt-submit-fix`, `worktree-resume-fix`, `boot-prompt-full-body`,
plus a Batch 3/4 cleanup pass (`supervisor-bugfixes-v0-5-x`,
`supervisor-gate-templating-v0-5-x`, `coordination-skill-followups-2`,
`v0-5-0-audit-cleanup`) closing dogfood-surfaced drift.

Deferred out: `agent.learning` broker variant + LLM-driven qualitative
learnings (→ v0.6.0, alongside MCP); per-CLI session resumption
(→ v0.6.0, Claude-only initial cut); per-CLI specialisation beyond
Claude (→ v1.0.0); configurable supervisor-pane layout beyond the
v0.5.0 25-agent / 5-col cap (→ v1.0.0).

Behaviour changes worth knowing about: supervisor mode UX changes from
"your terminal is the supervisor" to "the supervisor is a pane in
tmux"; coordination skill rewritten around `agent.intent`; Spec Kit
`tasks.md` is written to mid-flight; `--from-specs` is now a hidden
deprecated alias (use `--from-all-specs` or `--specs`); `cmd_supervisor`
returns immediately after launching the session (no foreground-CLI
blocking, no auto-attach); the Rust auto-merge loop is gone — the
supervisor agent merges per its skill instead.

### v0.6.0 — Supervisor/Coordination Hardening + `git paw add` (shipped 2026-06-17)
Not the MCP release it was originally scoped as; the dogfood turned it
into a large hardening cycle. `git paw add` / `remove` hot-attach/detach
a worktree+pane mid-session; `/tell` + `/agents` user→agent routing;
opsx role-gating; approval-pattern surfacing (`git paw approvals`);
per-commit verification + no-fail-fast mandate; stream-timeout recovery;
`agent.advanced-main` + `agent.learning` broker variants + qualitative
learnings; region-level forward-conflict detection; broker roster
hygiene; dashboard broker-log panel + supervisor introspection + pane
affordances; CLI-agnostic boot + config-driven dev-allowlist; cold-start
CI parity. ~30 capability specs. **MCP slipped to v0.7.0.**

### v0.7.0 — Read-only MCP Server (shipped 2026-06-23)
The MCP release. `git paw mcp` runs a standalone, client-spawned,
read-only Model Context Protocol server over stdio (no session/broker/
supervisor required), built on the `rmcp` v1.7 SDK (Apache-2.0). Seven
read-only tool categories: coordination (`get_intents`/`get_conflicts`),
governance, project-knowledge (`get_specs`/`get_skill`/…), documentation
(`get_readme`/`list_docs`/`get_doc`), source/files (`list_files`/
`read_file`/`search_code`, gitignore-excluded + repo-confined), session
state, and git context. Graceful degradation throughout; no agent CLI is
ever invoked as an inference backend. Plus: broker `POST /watch` (live
watch-target registration for hot-added worktrees); learnings
no-telemetry disclosure (local opt-in file + session-start notice +
opt-in GitHub-issue sharing); server-identity fix (advertises `git-paw`
+ crate version, not the SDK default) with per-repo `[mcp].name`; new
`[governance].readme`/`.docs` config. Verified via the five-gate
framework (incl. a path-traversal review of the file-reading tools);
1592 lib tests green. **Dogfood produced findings F1–F14 → v0.8.0
candidates (see below).**

### v0.8.0 — Internal worktrees + frictionless dogfood (shipped 2026-06-26)
One headline user-facing feature plus the babysitting-friction fixes the
v0.7.0 dogfood exposed — deliberately small and cohesive, with the big
approval-architecture rebuild held for v0.9.0. **Eight OpenSpec changes**,
all applied and archived (deltas synced to `openspec/specs/`):

- **Embedded worktrees** (`worktree-embedded-placement`, headline) — agent
  worktrees move from siblings (`../<project>-<branch>`) into the repo at
  `.git-paw/worktrees/<branch-slug>/`. Configurable via
  `worktree_placement = "child" | "sibling"`; **child is the default for
  new repos** (`git paw init` writes it + gitignores the dir), sibling
  stays the absent-config fallback so pre-existing repos/sessions are
  unaffected. Enables a contained, project-scoped permission model (one
  grant for `.git-paw/worktrees/` vs. scattered sibling dirs).
- **broker.sh agent helper** (`agent-broker-helper`, F1) — replaces the
  boot block's raw `curl …/publish` (which dead-stalled all three agents
  ~37 min in the v0.7.0 dogfood) with a bundled `.git-paw/scripts/broker.sh`
  (mirrors `sweep.sh`), installed by `git paw init` and allowlisted by
  precise script path instead of `curl *` (least-privilege).
- **AGENTS.md sidecar** (`agents-md-sidecar-injection`, F10) — the managed
  skill block moves to a gitignored `.git-paw/AGENTS.local.md`; the tracked
  `AGENTS.md` is no longer `assume-unchanged`, so legitimate edits stage
  and commit normally (the product bug that bit the v0.7.0 release twice).
  The boot prompt points **every backend** (OpenSpec / Spec Kit / Markdown)
  at the sidecar.
- **Session-orchestration robustness** (`session-orchestration-robustness`)
  — launch-readiness gate (no boot block dumped into a bare shell),
  add/remove pane integrity (kill-by-pane-id, no collateral pane loss), and
  equal-width agent rows with a minimum usable size, plus the regression
  tests that would have caught the dogfood failures.
- **dev-allowlist prefix grants** (`dev-allowlist-prefix-grants`, F2-lite)
  — prefix-matchable grants so routine dev commands stop re-prompting every
  cycle; `DEV_ALLOWLIST_PRESET` de-opinionated from git-paw's own
  cargo/just/openspec stack into universal commands + named stack presets.
  (The full classifier / broker-mediated approval stays v0.9.0.)
- **Dashboard broker-log height** (`dashboard-broker-log-height`, F3) —
  larger, configurable broker-log panel (pulled forward from v0.9.0).
- **Commit-discipline skills** (`coding-agent-commit-discipline`, F7) —
  "stand by after commit" (stops the `/opsx:archive` reflex) plus the
  "each commit is a releasable unit; amend fixups" guidance (pulled forward
  from v0.9.0's clean-history discipline).
- **Drop dead Summary column** (`dashboard-drop-summary-column`, F4) from
  the agent table (`delivery.rs:417` hardcoded it empty; broker-log
  `derive_summary` left intact).

Plus a `fix(init)` to gitignore `.git-paw/session-learnings.md` so the
per-session learnings output is never swept into a commit, and a README
logo header. 2126 tests passing; the four CI gates green. **Dogfood
surfaced one deferred bug (the remove-dirty-check race, see v0.9.0) and a
test-suite-consolidation backlog (~330 candidate cuts → v0.13.0).**

### v0.9.0 — Unattended operation (shipped 2026-07-06)
The original supervisor-mode vision made real: run a multi-agent wave to
completion with **no human babysitting**. **Twelve OpenSpec changes**, all
applied + archived (deltas synced to `openspec/specs/`), dogfooded across six
supervised waves on `feat/v0.9.0-specs`:

- **`unattended-drive-loop`** (headline) — `git paw start --unattended`: an
  in-tool poll loop that replaces the external `wave*-monitor.sh` babysitter.
  Encodes the operator heuristics (live-prompt detection in the last pane
  lines, per-pane explicit capture, `(agent_id, shape)` dedup within 5 min,
  pane→agent via `pane_current_path`, send-keys follow-up Enter). Auto-clears
  the supervisor pane's own read-mostly verbs (W15-3).
- **`auto-approve-classifier`** — the battle-tested safe/unsafe classification
  productised; pre-approves worktree-confined `git add`/`git commit` (the
  commit step itself used to stall). The brain the drive loop runs on.
- **`broker-mediated-approvals`** (drift F / F5 / F6) — in-band
  chat-with-options so the blind send-keys race dies; live-prompt-gated
  pane-scrape ship path.
- **`supervisor-stuck-bloat-detection`** — detects timeouts, `/clear`-hint
  bloat, no-progress, and session/rate-limit states from **live pane state**
  (not tips/file-counts); productive ≠ frozen.
- **`selftest-harness`** (F8) — `git paw selftest` + E2E isolation (dummy CLI,
  private socket, `.git-paw/tmp/` repo); `pick_broker_port` moved to an
  OS-assigned ephemeral port (collision-proof at any concurrency).
- **`verify-at-tip`** (F9) — the change-level gate re-resolves the worktree
  tip and diffs vs merge-base, never a stale first-commit SHA.
- **`searchable-pickers`** — fuzzy-filter multi-select for `select_branches`
  and `select_specs` (ratatui-based; usable on repos with many branches).
- **`conflict-detector-additive-vs-true`** — only escalates true same-anchor
  collisions to a human; additive overlaps downgrade to informational
  `agent.feedback` (3 of 4 escalations last dogfood were additive false alarms).
- **`broker-helper-full-surface`** (G4) — the bundled helper covers the full
  `agent.status` payload + all message types, so the supervisor never falls
  back to raw curl (least-privilege by-path allowlist intact).
- **`learnings-supervisor-observation-channel`** — qualitative supervisor
  learnings wired into the sweep loop (`sweep.sh learn`, `tooling_friction`
  category, opportunistic + session-end synthesis, `### Tooling friction`
  renderer).
- **`remove-dirty-check-ignores-managed-files`** — the v0.8.0 regression:
  the remove dirty-check now skips git-paw's own managed/excluded files.
- **`commit-prefix-de-opinionation`** — the EXPORTED skill defers commit
  format entirely to the consumer's `AGENTS.md`; + a `lang-agnostic-skills`
  convention-neutrality audit enforcing the export/repo-specific split.

Plus the user-authored **`## Behavioral Guidelines`** section in `AGENTS.md`
(Think Before Coding / Simplicity First / Surgical Changes / Goal-Driven
Execution) and a logo/banner/README refresh. Shipped on all three channels
(crates.io, GitHub release, Homebrew tap). **Dogfood surfaced follow-ups →
v0.10.0/v1.0.0: per-CLI supervisor native auto-mode (the dominant residual
babysitting cost), agent-memory-isolation (an agent wrote the operator's
`MEMORY.md`), a temporal-ordering assertion nit, and two test-isolation
flakes.**

### v0.10.0 — Agent-friendly documentation (shipped 2026-07-13)
Make git-paw's own docs consumable by agents, plus carried bug fixes and
dogfood-surfaced hardening. Dogfooded on `feat/v0.10.0-specs` and shipped on
all three channels (crates.io, GitHub release, Homebrew):

- **`agent-friendly-docs-site`** (headline) — `robots.txt` / `llms.txt` /
  `sitemap.xml` + per-page structured metadata emitted at docs-build time, so
  an agent fetches exactly the section it needs.
- **`docs-fetch-skill`** (headline) — bundled skill that pulls conventions
  and examples from the docs site on demand; no boot bloat, no doc content
  baked into the binary.
- **Dashboard CPU-leak saga** (`dashboard-orphan-exit` +
  `dashboard-orphan-exit-hardening` + `dashboard-tty-hangup-exit`) — orphan
  self-exit (`getppid()==1`) and exit-on-all-paths, then the real fix: a
  bounded `poll(2)` POLLHUP wait, because `event::poll` never returns on a
  hung-up pty (crossterm busy-loops internally), so the orphan gate was
  architecturally unable to fire. Idle tick raised to 800 ms.
- **`remove-dirty-check-porcelain-parse`** — `--porcelain -z` parsing +
  `.git-paw/` subtree filter (the v0.8.0 remove-flake regression).
- **`worktree-helper-provisioning`** — `start`/`add` drop the bundled
  helpers into each agent worktree (recurring boot friction since v0.8.0).
- Doc-accuracy sweep + nit hardening: `init-config-template-completeness`,
  `doc-consistency-pass`, `spec-purpose-backfill` (+ CI guard),
  `spec-index-completeness`, `approval-log-temporal-ordering`,
  `test-isolation-hardening` (connect-probe port checks). Docs pipeline gained
  a `workflow_dispatch` recovery trigger.

Dogfood follow-ups → v0.11.0: supervisor-native auto-mode, agent-memory-
isolation, same-region conflict detection, and the approve-send-gate work.

---

### v0.11.0 — Unattended hardening (shipped 2026-07-17)
Finish what v0.9.0 started — the last supervision friction and one safety bug
from the unattended-operation dogfood. Dogfooded on `feat/v0.11.0-specs`; every
change traces to a v0.9.0 dogfood finding or the 2026-07-07 export audit; no
breaking changes:

- **`supervisor-native-auto-mode`** (headline) — `[supervisor] approval =
  "auto" | "full-auto" | "prompt"`; `full-auto` maps the supervisor pane's CLI
  to its native skip-permissions mode (Claude + claude-oss wired natively,
  other CLIs fall back via a lookup seam to v1.0.0's `HookProvider`). Kills the
  dominant residual babysitting cost — the compound verify commands a human had
  to clear by hand.
- **`agent-memory-isolation`** (safety) — config-driven protected-path denial
  stops agents/supervisor reaching the operator's memory/config outside the
  worktree (CONFIRMED Wave-5: a coding agent wrote the operator's `MEMORY.md`).
- **`conflict-detector-same-region`** (W15-22a) — normalized cross-kind
  matching catches two changes editing the *same* region; the false-negative
  complement to v0.9.0's additive-vs-true detector.
- **`agent-answer-variant`** (W15-11) — `agent.answer` broker variant with
  targeted delivery, completing the in-band supervisor→agent reply flow.
- **`classifier-stack-de-opinionation`** — the auto-approve whitelist is now
  sourced from the resolved stack preset (not git-paw's own
  `cargo`/`just`/`openspec`), plus worktree-confined dev-test rules; Rust
  classifier and `sweep.sh` mirror kept in lockstep.
- **`approve-send-gate-hardening`** — detect multi-option prompts by option
  glyph / `Do you want to proceed?` marker over a wide window, re-confirm the
  prompt is live on a fresh capture before sending, and parse the affirmative
  option instead of a blind `Down`+`Enter`.
- **`agent-worktree-dev-allowlist`** — seed each agent worktree's
  `.claude/settings.json` with the resolved preset verbs at start/add/recovery,
  mirroring broker-endpoint seeding, so agents stop prompting on preset-safe
  commands.
- Docs/CI closeout: `skill-prose-de-opinionation` (export-audit F1–F4) +
  `doc-polish-closeout` (consistency-sweep LOW tail); the `build-docs` →
  `deploy-docs` two-job split with least-privilege Pages grants.

Dogfood follow-ups → v0.12.0: broker delivery-cursor bug, per-pane approval
serialization, pane→agent re-resolution, approve-on-detect, and question
re-publish suppression (`session-learnings.md`, 2026-07-14/15).

### v0.12.0 — Superpowers spec format + unattended-hardening riders (shipped 2026-07-25)
Ship the planned fourth spec format and fix the worst correctness bugs the v0.11.0
unattended-mode dogfood surfaced (`session-learnings.md`, 2026-07-14/15). Built on
`feat/v0.12.0-specs`; no breaking changes:

- **`superpowers-spec-format`** — `superpowers` as a fourth `[specs] type` /
  `--specs-format`, scanning obra/superpowers `docs/superpowers/plans/*.md` (checkbox
  steps with mid-flight writeback + per-task Files:/Run: metadata) into spec-driven
  sessions; extends the existing Spec Kit checkbox scanner. Export-agnostic
  (CLI-neutral markdown).
- **`broker-delivery-hardening`** — monotonic poll cursor (no more re-serving a stuck
  `question` while hiding later `artifact`/`verify-now`) + duplicate-question
  suppression once a matching `agent.answer` is in the inbox.
- **`supervisor-loop-escalation-tiering`** — resolves the approval-dispatch race by
  de-duplication: the drive loop is the sole approver of classifier-safe prompts and
  escalates the rest to a uniform broker channel; the supervisor (told a loop is
  running) drains escalations first and never blanket-approves. Disjoint approval
  sets → no race.
- Riders: `broker-variant-ripple-checklist` (AGENTS.md Change Checklist), the CI
  cache/coverage-split, and the `broker-agent-id-any-prefix` retro-spec + rustdoc fix.
  (`broker-port-rebind-flake` was dropped — already resolved in v0.10.0.)

Carried to v1.0.0: the `E0063` merge shape (a branch adds a required field while a
sibling constructs that struct in new test literals — each compiles alone, the merge
fails), a conflict-detection design problem that belongs with in-tool ordered
merge-orchestration (W15-22b).

---

### v0.13.0 — `git paw doctor` + pre-freeze quality hardening (shipped 2026-08-05)
Preflight diagnostics + the first pre-v1.0 quality pass. Built on `feat/v0.13.0-specs`;
no breaking changes:

- **`git paw doctor`** — preflight diagnostics (env / CLIs / config / spec-system /
  bundled-scripts / broker / supervisor / hygiene), a `--live` smoke arm, `selftest` folded
  under it.
- **path-injection hardening** — tmux session-name sanitization + shell-quoted
  pipe-pane / dashboard paths via the domain newtypes (`SessionName` / `BranchSlug` /
  `WorktreePath`).
- **broker runtime hardening** — `spawn_blocking` offload for the role-guard git; poison-lock
  recovery.
- **code-analysis refactor (R1+R2+R3-seam)** — `config.rs` / `tmux.rs` / `main.rs` module
  splits + the injectable `CommandRunner` process seam (the R3 tail — interactive + dashboard —
  was deferred to v0.14.0).
- Riders: wire-api-freeze relax (serde defaults + a declared MSRV), interactive CLI-contract PTY
  e2e, test-suite consolidation + spec-traceability audit, Gemini→Antigravity CLI transition,
  and repo-local dev agent-skills.

Release note: the tag was force-moved onto an MSRV-1.96 commit (declared 1.97 → 1.96) so
cargo-dist's runner could build the darwin binaries — don't set MSRV to current-stable.

---
### v0.14.0 — Autonomous orchestrator + pre-freeze architecture hardening (shipped 2026-08-11)
Make the tiered-model workflow real (a smart *orchestrator* driving a cheap *worker* fleet,
human as exception handler) and land the pre-freeze structural + reliability hardening. Built
on `feat/v0.14.0-specs`; no breaking changes:

- **`supervisor-autonomous-orchestrator`** (headline) — the `--unattended` drive loop wires the
  mechanical pump to the supervisor "brain": each judgment call (risky/unknown prompt, worker
  question, merge decision) is injected into the orchestrator pane instead of a dead inbox, and
  the human becomes the exception handler.
- **`supervisor-correction-loop`** (headline) — a configurable `[supervisor.correction]`
  self-healing loop (`auto_loopback`, `max_cycles`, `on_exhausted`) that closes the verify↔fix
  cycle without a human hop and emits an exhaustion learning (the tiered-model tuning signal).
- **tiered-model workflow guide** (docs) — planning-is-a-pre-step, task-level-detailed specs,
  and the per-project verification cycle defined in the consumer's `AGENTS.md` (export-agnostic;
  git-paw ships only the generic five-gate scaffold).
- **`classifier-single-source`** — `sweep.sh` delegates to a hidden read-only `git paw
  __classify` seam, retiring its ~490-line embedded-Python classifier mirror (no more shell↔Rust
  drift on the security-critical auto-approve path).
- **pre-freeze architecture hardening** — restores/extends the `code-architecture` capability and
  lands its behavior-preserving refactors: `interactive.rs` → `interactive/{resolver,prompter,
  picker}`; the dashboard TUI as **Model-View-Update** (the SIGHUP/`poll_tty` FFI left
  byte-identical); `git.rs` routed through the `CommandRunner` seam (unit-testable, byte-identical
  git); the worker lifecycle as an explicit `WorkerPhase` enum; and the shared `_paw_common.sh`
  shell preamble sourced by `broker.sh`/`sweep.sh`/`docs-fetch.sh`. **Behavior change:** an
  enforced **per-pane approval claim** (an atomic claim file honored by both the Rust drive loop
  and `sweep.sh`) replaces the soft sole-approver convention — a double-approval on one pane is
  now structurally impossible, proven by an 8-thread race test.
- Riders: supervisor 50/50 layout, dashboard status-activity + broker-log heartbeat dedup, four
  unattended-loop stall fixes, `git paw init` config backfill, child worktree placement.

Dogfood follow-ups (the build dogfood, 2026-08 — the autonomous mode is **not yet fully
hands-off**): the safe-command classifier over-escalates common safe shapes it should recognize
(`bash -n … && echo`, `for`/`until` loops, `$(…)` command-substitution) → they escalate as
`unknown` and stall a pane; the pump has no path to clear a worker's Claude-Code *edit* approval;
and there is no auto-approver above the orchestrator, so it self-stalls on its own read-only
monitoring commands. All three are autonomous-mode **friction-killers**: the classifier-shape
normalization landed in the **v0.16.0** boot-friction cycle; the edit-approval + orchestrator
auto-approver items are in the **v0.19.0** per-CLI cycle. C5's per-pane
claim already makes broader auto-approval race-safe, so loosening the classifier or adding a
second-tier approver won't reintroduce the double-approval hazard.

---
### v0.15.0 — Per-worktree runtime isolation + security posture (shipped 2026-09-02)
Make parallel worktree sessions independent at *runtime*, not just in git. Built on
`feat/v0.15.0-specs`; all opt-in `[worktree.*]` config, no breaking changes:

- **`session-runtime-isolation`** — `[worktree.env]` copies declared env files verbatim (a copy,
  never a symlink, so drift surfaces at merge); `[worktree.ports]` allocates each worktree an
  index-derived, collision-free port block written to a generated `.env.local` (freed on
  remove/purge, tracked via a `runtime_slot` on the session-state entry); `[worktree.hooks]` runs
  consumer `on_create`/`on_remove` commands with `on_create` `KEY=value` stdout merged into
  `.env.local` — the general escape hatch for stack-specific isolation such as per-worktree
  databases. Also fixed `--dry-run` to preview the worktree path per the configured placement
  (child → `.git-paw/worktrees/<slug>`) instead of hardcoding `../`.
- **`core-security-posture`** — a consolidated trust-model disclaimer (what the sandbox /
  classifier / allowlists / protected-paths / remote control each do and do **not** guarantee) and
  a per-OS **docs-only** FS-scoped sandbox workflow (`sandbox-exec`/`bwrap` wrapped through the
  existing `[clis.*].command` seam — git-paw neither invokes nor ships a profile), plus a read-only
  `git paw doctor` sandbox-availability check (extends `preflight-diagnostics`). The sandbox was
  **validated live on macOS and the Linux smoke container** (kernel-confined writes, commits +
  `cargo build` still work, `.git/hooks`/`config` RO closes the persistence gap).

Dogfood follow-ups (routed to future cycles, not v0.15.0): the auto-approve classifier still
escalates a leading `VAR=value` / `env` / `nohup` prefix — into **v0.16.0** (GP-02); the
`--unattended` pump exits early on a "heartbeat" when detached, and the sandbox proved the
headline thesis that **sandboxed workers should run in skip-permissions** (the kernel is the
boundary, so the approval treadmill is exactly what it removes) — both into **v0.19.0**. A
dashboard **tabbed layout / scrollable roster** (the top agent roster can't scroll with many
agents) was queued into **v0.21.0**'s DX-hardening bucket, then replaced there by
`dashboard-usage-view` (the roster moves into `git paw status`).

### v0.16.0 — Unattended boot & friction-killers (shipped 2026-09-14)
Make a fresh consumer's `--unattended` run reach its first tool call. The peer-4-poker dogfood
(Spec Kit, 2026-08-31) proved it couldn't, because three boot defects masked each other. Built
on `feat/v0.16.0-specs`; 17 changes:

- **Boot** (`unattended-boot-hardening`, GP-02/03/04): the bundled helpers and leading
  `VAR=` / `env` / `nohup` prefixes classify safe; `.git/` writes are danger; `auto` maps to
  Claude's `acceptEdits`; the launch acceptance dialog is gated and dead panes report dead; the
  conflict detector ignores `.git-paw/`. GP-01 was reframed as exported worker-guidance skills,
  and the inert allowlist seeding was removed.
- **Unattended lifecycle:**
  - The exit summary resolves each agent's final phase (GP-05).
  - The drive loop is bound to its session, so purge and stop leave no orphan (GP-06).
  - The orchestrator pane is recorded in `session.json`.
  - Nudges submit reliably and stop at wind-down (GP-15/16).
  - The integration branch is refreshed before verification.
  - Per-CLI prompt-shape profiles were added.
- **Launch correctness:**
  - `default_spec_cli` is honoured, one default-branch resolver has an `origin/HEAD` fallback,
    and `start` reattaches instead of forking (GP-10/12/13).
  - `purge --stale` is scoped to the current repo (GP-18).
  - Spec Kit agents are pointed at their spec path (GP-07).
  - Peers are no longer told to cherry-pick each other's branches, the root cause of GP-17/18.
  - Shared task-list edits are exempt from ownership conflicts (GP-17).
- **DX:**
  - `git paw attach` and shell completions; the phantom `resume` struck from the docs.
  - An MCP boot-event publish tool.
  - The sandbox guide's heredoc fix (GP-11).
  - git-paw's own conventions removed from exported prose.

The post-release dogfood (2026-09-14) found a data-loss chain: a false "N/N merged" summary
(GP-21, an over-correction of GP-05) followed by a `purge --force` that deleted the unmerged
branches (GP-20, latent since the broker era, not a 0.16.0 regression). Both were fixed in
v0.17.0 rather than in a v0.16.1. The full pre-release plan for this cycle is in
`.git-paw/MILESTONE.prerelease-v0.17.0.bak`.

---
## Appendix — Dogfood decision record & design principles (pre-v0.10.0)

> Retained here because it is **not captured in `openspec/specs/`**: empirical
> decision data and durable governance. The per-version shipped-feature detail
> that used to precede this appendix was removed on 2026-07-06 — it now lives
> in `openspec/specs/` (capabilities), `CHANGELOG.md` (per-version), and the
> mdBook (`docs/src/`). Cross-references below to a "LOCKED SCOPE" / change
> numbers refer to the v0.9.0 change list (see CHANGELOG.md + specs).

### Standing design principle — "exported by the binary" vs "git-paw-repo-specific"

*(Reaffirmed 2026-06-29.)* Anything the binary EXPORTS to a consumer (bundled
skills `assets/agent-skills/*`, scripts, boot blocks, the `git paw init` default
config, allowlist presets) MUST be project-agnostic. Per-project *conventions*
— commit-message format, stack/test commands, governance docs — belong to the
CONSUMER and are supplied by their injected `AGENTS.md` / config, never baked
into the export. git-paw's OWN conventions (Conventional Commits for its
changelog, its cargo/just/openspec stack) are repo-specific and live ONLY in
git-paw's own `AGENTS.md`/`CLAUDE.md`/`cliff.toml`. Prior instances:
`DEV_ALLOWLIST_PRESET` de-opinionation (v0.8.0), `lang-agnostic-skills`
stack-neutrality (v0.6.0); v0.9.0 #11 adds *convention*-neutrality (commit
format) + a leak-audit that enforces it. New bundled assets are checked against
this audit.

### Battle-tested classification (captured 2026-06-09, do not lose)

The exact escalate-vs-auto-approve logic the v0.6.0 dogfood monitors
converged on across 8 waves. This is the perishable input for the
`--unattended` classifier + the `auto-approve-scope` extension —
seed the implementation from this, don't re-derive it.

**ESCALATE to the human** (never auto-approve) when the prompted
*command* (the slice between the `Bash command` / `Bash(` header and
the confirmation question — NOT the surrounding agent narration)
matches any of:

- `rm -rf` / `rm -fr` — **unless** the only target is repo/OS scratch:
  a `/tmp/paw-*` path or `.git-paw/tmp/...` (incl. `rm -rf "$VAR"`
  where `VAR=/tmp/paw-…` or `.git-paw/tmp/…`). Those scratch deletes
  auto-approve.
- `git push`, `--force`, `force-push`
- `reset --hard`, `git rebase`, `git checkout ` (branch switch),
  `branch -D`
- `git worktree remove` / `clean -fd` / `clean -fdx`
- `sudo `, `mkfs`, `dd if=`, `> /dev/`, `chmod -R`, `chown -R`
- `pkill` / `kill` (process kills — review the pattern; targeted
  `pkill -f 'paw-dogfood-…'` proved safe but still worth a glance)

**AUTO-APPROVE** (safe):

- File-edit prompts (`Do you want to make this edit`) and
  create/write prompts — the agent editing its own worktree.
- A prompted command whose verb is in the read-mostly allowlist:
  `curl cat ls grep rg git echo sed awk find wc head tail jq mkdir
  touch openspec just export tmux env`.
- "Don't ask again for: X" (the broad grant, option 2) **only** when
  X is in that allowlist AND is **not** an arbitrary-code runner
  (`python`, `bash -c`, `sh -c`, `eval`, `node `, ` -c `). For
  arbitrary-code commands use one-time "Yes" (option 1), never the
  broad grant. (A real catch: a `python3 -c` task-list one-liner must
  not get a permanent grant.)

**Prompt-detection heuristics** (avoid false positives):

- Act only on a **live** prompt: the footer (`Esc to cancel`) must be
  in the last ~4 non-blank lines. Otherwise the supervisor merely
  *narrating* about a pane's prompt trips a phantom approval.
- For 2-option Yes/No prompts, option 1 = Yes; for 3-option
  Yes / Yes-don't-ask / No, pick option 2 only per the allowlist rule
  above, else option 1.

**STUCK / BLOAT signals** (intervene/restart, don't wait forever):

- `Request timed out` in a pane → the CLI's API call failed; the agent
  is likely stalled.
- `/clear to save <N>k tokens` with N past a threshold (~250k seen
  this cycle) → context bloat; the agent degrades and eventually
  freezes. Proactively flag/restart rather than waiting.
- No progress across a heartbeat (task-checkbox count AND commit count
  unchanged over ~25 min) → nudge or investigate.

**Operator-loop shape that worked**: a poll loop (~15s) that sweeps the
supervisor + agent panes, applies the rules above, and exits to
re-engage the human on (a) a genuinely risky prompt, (b) a stuck/bloat
signal, (c) a completion signal (PASS/FAIL report or all-tasks-checked),
or (d) a ~25-min heartbeat. The throwaway prototypes live in
`.git-paw/scripts/wave*-monitor.sh` (local, gitignored) for reference
when implementing `--unattended`.

**OS portability (macOS / Linux — the only two targets; Windows is
WSL = Linux).** For bash/zsh the command *verbs* barely deviate, so a
single classification covers both. The deviations that DO matter:

- **Temp-path resolution.** macOS symlinks `/tmp` → `/private/tmp`, so
  the same scratch path appears under both prefixes — the safe-scratch
  whitelist MUST match `/tmp/paw-*` **and** `/private/tmp/paw-*` (the
  v0.6.0 dogfood teardown literally globbed both). `$TMPDIR` also
  differs (macOS per-user `/var/folders/…`, Linux usually `/tmp`).
  **Mitigation: prefer the repo-local `.git-paw/tmp/` scratch** (already
  the supervisor-verify default) — it is OS-independent and sidesteps
  the whole `/tmp` vs `/private/tmp` vs `$TMPDIR` matrix. Treat OS temp
  paths as the fallback the whitelist must still cover.
- **A few OS-specific dangerous commands** to add to the escalate set:
  Linux — `mkfs*`, raw device paths (`/dev/sd*`, `/dev/nvme*`);
  macOS — `diskutil`, `/Volumes/…` deletes, `rm -rf ~/Library/…`.
  The shared core (`rm -rf`, `git push`/`--force`/`reset --hard`,
  `sudo`, `chmod -R`) is identical on both.
- **Flag dialect** (BSD vs GNU, e.g. `sed -i` differs) affects how the
  tool *runs* helpers, not the danger classification — the classifier
  matches verbs/targets, which are stable across both. Build the list
  as a shared base + a small per-OS escalate addendum, not two lists.

### Learnings taken into account (v0.6.0 → v0.8.0 dogfoods + CI)

Sourced from `.git-paw/session-learnings.md` (quantitative),
`.git-paw/v0.8.0-dogfood-findings.md`, and the captured supervisor/
babysitter feedback. These reshape the v0.9.0 change list above:

1. **Even `git commit` stalls on an approval prompt** (v0.8.0 dogfood, "F2
   strong evidence"): three agents sat at the `git add && git commit`
   prompt with commits queued; approving (whitelist git add/commit) landed
   them in seconds. An unattended run dies here. → **#5 classifier MUST
   pre-approve worktree-confined `git add`/`git commit`** (safe, confined).
2. **Don't judge "stuck" from tips/file-counts — read pane state.** An
   idle-looking agent is almost always *blocked on a live prompt*, not
   refusing. One agent showed **1054 min blocked "waiting on supervisor"**
   in the v0.8.0 learnings. → **#6 stuck/bloat detection MUST capture live
   pane state**, distinguish "blocked on prompt" (apply classifier) from
   "no progress" (nudge/restart), and treat "blocked on supervisor" as a
   detectable stuck state with its own timeout.
3. **Conflict escalations are mostly false alarms.** 3 of 4
   human-input-requested `agent.question` escalations last dogfood were
   purely *additive* overlaps (different structs; hunks 75–112 lines
   apart) — only one was a true same-anchor collision. In an unattended
   run these stall the wave waiting on input that isn't needed. → **new
   change #9 `conflict-detector-additive-vs-true`.**
4. **Verify-isolation flakes are concurrency, not the live session** (F8
   correction): `pick_broker_port = 24000 + pid%200` collides when N
   agents run `cargo test` at once. → **#2 selftest-harness uses an
   OS-assigned ephemeral port (`bind 127.0.0.1:0`)**; serialize verify runs
   ([[feedback_serialize_e2e_verification_runs]]). Also: a local pass with
   inherited dogfood tmux/broker state ≠ CI pass — verify cold
   ([[feedback_local_pass_doesnt_equal_ci_pass]]).
5. **The supervisor still leaks raw curl** (G4): the bundled helper's
   surface is narrower than the messages the skill emits (no phase/detail),
   so rich `agent.status` hand-rolls `curl`, forcing a broad allowlist. →
   **new change #10 `broker-helper-full-surface`** keeps the by-path,
   least-privilege allowlist intact (no `curl *`).
6. **Operator-loop heuristics are perishable — encode them in #7.** From
   the babysitter feedback: act only on a *live* prompt (footer in the last
   ~4 non-blank lines) to avoid phantom approvals; capture each pane
   explicitly, no `for $p` loops ([[feedback_avoid_loop_pane_capture]]);
   resolve pane→agent via `pane_current_path`, not index/arg order
   ([[feedback_verify_pane_to_agent_mapping]]); send-keys nudges need a
   *follow-up* Enter ([[feedback_sendkeys_nudge_needs_followup_enter]]);
   for 2-option Yes/No prompts send `"1" Enter` directly, not Down+Enter
   ([[feedback_sweep_approve_interface]]); dedup repeated alerts by
   `(agent_id, shape)` within 5 min ([[feedback_inbox_dedup_unknown_classification]]).
7. **Feedback cycles run high before a verify passes** (mcp-server 7,
   dev-allowlist 6, worktree-embedded 4 cycles). → #7's drive loop must
   tolerate *multiple* feedback→fix→re-verify cycles per agent and NOT
   treat "not yet verified after N cycles" as stuck; budget for it.

### Dogfood findings — consolidated record (folded in 2026-06-29)

The per-cycle dogfood logs (`.git-paw/{supervisor-findings,v0.7.0-dogfood-
findings,v0.8.0-dogfood-findings}.md`) were verified against `openspec/specs/`
+ `src/` and folded into this doc; the source files were then deleted. This
section is the single record. Shipped findings are kept as a traceability
ledger; still-open findings carry their full context (cross-referenced to the
LOCKED SCOPE change numbers above).

#### Shipped-findings traceability (verified implemented)

**v0.6.0-era supervisor dogfood (Waves 1.5–4):**

| ID | Finding | Capability that resolved it |
|----|---------|-----------------------------|
| W15-1 | Boot prompt injected but not submitted (claude-oss) | `cli-submit-profile` (split-send + settle-delay Enter) |
| W15-6 | Agents stall on boot-time broker-curl prompt | `agent-broker-helper` + `curl-allowlist` (path-scoped) |
| W15-8 | Broker curls should be skill helper scripts | `agent-broker-helper` (`broker.sh` subcommands) |
| W15-10 | Conflict-detector fn-granularity false-positive | `conflict-detector-fn-granularity` (`agent.intent` regions) |
| W15-12 | File-edit prompts are a distinct shape (drift C) | `auto-approve-file-edits` (+ `worktree-branch-guard`) |
| W15-14 | Per-section commit-cadence drift (~16 uncommitted) | `coding-agent-commit-discipline` |
| W15-15 | Dashboard CLI column empty for coding agents | `broker-roster-hygiene` (`with_seeded_cli`) |
| W15-16 | Phantom "human" agent from feedback `from` field | `broker-roster-hygiene` (publisher-only rows) |
| W15-18 | Supervisor `cd <wt> && git` cwd-leak | `supervisor-skill-discipline` (`git -C`, never `cd`) |
| W2-2 | Supervisor pane CLI launch eaten by zsh prompt | `robust-cli-launch` (clean line, verify-and-retry) |
| W2-3 | Recovery explodes panes + corrupts session JSON | `session-recovery-integrity` |
| W2-6 | Agents use `cd <dir> && git` (agent-side) | `coordination.md` "Stay inside your worktree" |
| W2-7 | Five-gate verify false-positive (fail-fast masks) | `no-fail-fast-verification` |
| W2-8 | auto-approve-scope stale test vs its own spec | `status-republish-on-write` (reconciled at merge) |
| W2-9 | Flaky paste-buffer test under parallel load | hardened to `wait_for_bodies` (no fixed sleep) |
| W3-1 | First agent pane created in repo root (drift D) | `supervisor-first-agent-cwd` |
| W4-1 | Finished agents draft `/opsx:archive` | `opsx-role-gating` + stand-by-after-commit |

*POSITIVE/informational (no action): W15-2, W15-4, W15-9, W15-17, W15-20,
W2-1, W3-VALIDATED, W4-VALIDATED.*

**v0.7.0 MCP-release dogfood:**

| ID | Finding | Shipped | Capability |
|----|---------|---------|------------|
| F1 | Agent boot-publish → bundled broker.sh helper | v0.8.0 | `agent-broker-helper` |
| F3 | Dashboard broker-log panel taller/configurable | v0.8.0 | `dashboard-broker-log` (`height_lines`, default 20) |
| F4 | Drop dead "Summary" column | v0.8.0 | `dashboard` (column gone; `derive_summary` intact) |
| F7 | Coding agents reflexively `/opsx:archive` | v0.8.0 | `coding-agent-commit-discipline` |
| F10 | `assume-unchanged` blocks AGENTS.md commits | v0.8.0 | `agents-md-injection` (gitignored sidecar) |
| F11 | MCP has no docs/README tool | v0.7.0 | `mcp-read-tools` (get_readme/list_docs/get_doc) |
| F12 | Governance bring-your-own | v0.7.0 | kept BYO (product decision; no scaffolding) |
| F13 | MCP advertised SDK identity "rmcp" | v0.7.0 | `mcp-server` identity + `[mcp].name` |
| F14 | MCP couldn't browse source | v0.7.0 | `mcp-read-tools` (list_files/read_file/search_code) |

**v0.8.0 dogfood:** G1 (launch-readiness gate), G2 (remove-by-pane-id +
reconcile), G3 (equal-width grid rows) — all shipped in
`session-orchestration-robustness`.

#### Still-open findings (full context — feed the LOCKED SCOPE above)

**F2 — auto-approve doesn't clear dev-essential stalls.** *Lite SHIPPED
v0.8.0* (`dev-allowlist-prefix-grants`: prefix-matchable grants + skill nudge
to drop `echo "…$?"` exit-probes). *Full classifier → v0.9.0 #5.* Every agent
repeatedly stalls on safe dev prompts (file edits, `cargo check/test`,
dir-access) that the auto-approve thread + supervisor sweep should batch-clear.
The CLI's "don't ask again for: <cmd>" whitelists some commands by **exact
string**, so `echo "... exit $?"` wrappers (varying text) re-prompt endlessly,
while prefix-matchable ones (`python3 -c`, `cargo check *`) generalize. Two
angles: (a) prefer prefix grants [shipped]; (b) agents shouldn't wrap dev
commands in `echo "...$?"` exit-probes that defeat whitelisting.

**F2 strong evidence + babysitter lesson (v0.8.0)** → v0.9.0 #5/#6. At a
wind-down decision I judged "agents won't commit" from branch tips + file
counts — but 3 agents were sitting at the `git add && git commit` APPROVAL
PROMPT with commits queued; approving (whitelist git add/commit) landed them
in seconds. (a) Even the COMMIT step stalls on an approval prompt — an
unattended run dies here, so #5 MUST pre-approve worktree-confined `git add`/
`git commit` (safe, confined). (b) NEVER judge "stuck/refusing" from tips/file-
counts alone — #6 MUST capture live pane state; an idle-looking agent is
usually blocked on a prompt.

**F5 — approval keystrokes land as stray chat input when no prompt is active**
→ v0.9.0 #8 (reinforced by design constraints W15-13/W15-19). When an approver
sends `1`/`2`+Enter to a pane whose prompt already cleared (agent moved on, or
another approver won the race), the digits land as a literal chat message —
polluting context and potentially leaving stray commands (e.g. an unsubmitted
`/opsx:archive`) in the input box. This is the blind-send-keys race drift F
eliminates. Mitigations: (a) re-confirm an active "Do you want to proceed?"
prompt immediately before sending keys; (b) prefer a broker-mediated approval
signal over raw send-keys.

**F6 — approvals should be broker "chat-with-options", not per-pane CLI
modals** → v0.9.0 #8 (drift F; subsumes F2-full + F5). Instead of the CLI's
blocking permission modal (which pulls the operator into each agent pane),
the agent surfaces the decision as a broker/chat message with options the
supervisor answers in-band. The modal is the CLI permission layer (not
agent-controllable via prompt) and no CLI renders an approval as structured
chat output — so this is a **coordination-layer** design, and git-paw already
has the primitives (`agent.question`/`agent.blocked` + supervisor response).
Agents publish a proposed gated action; supervisor approves/denies in its own
flow; agent polls and proceeds. *Stop-gaps available today (no new code):*
(a) `agent_approval = "full-auto"` → CLI launches with
`--dangerously-skip-permissions` (acceptable for worktree-isolated +
branch-guarded agents; trades away the auto-approve dogfood signal);
(b) broaden `permissions.allow` prefix rules + `acceptEdits` mode.

**F8 — in-session verify false-negatives → CORRECTED: concurrent-test port
collision, not live-session collision** → v0.9.0 #2. The E2E verify failures
(`cli_supervisor_no_config`, `e2e_supervisor_stop` exit 101) are NOT a live-
session collision — the live broker is on :9119, outside the test range, and
tmux is isolated via `TMUX_TMPDIR`. Real cause: `pick_broker_port() = 24000 +
(pid % 200)` gives only 200 PID-keyed ports, so N concurrent `cargo test`
runs (5 agents + the verify worktree) collide mod 200 → "address already in
use" → exit 101. Fix: OS-assigned ephemeral port (`bind 127.0.0.1:0`, read
back) — collision-proof at any concurrency — and/or serialize verify runs
([[feedback_serialize_e2e_verification_runs]]).

**F9 — supervisor verifies a stale first-commit SHA, not the branch tip**
→ v0.9.0 #3. mcp-server made 3 commits (impl → E2E tests → docs); the
supervisor set up its verify worktree at the FIRST commit and ran all five
gates against that stale snapshot, then flagged docs as MISSING when they were
committed at the tip. A change-level gate (docs/tests present?) MUST verify the
full change at the branch TIP, not one commit — re-resolve and re-check out the
tip before change-level gates ([[feedback_verify_against_merge_base_not_tip]]).

**G4 — supervisor falls back to raw curl for rich agent.status (helper too
narrow)** → v0.9.0 #10. The supervisor publishes structured agent.status via
raw `curl … -d '{…,"phase":"audit","detail":{…}}'` because `sweep.sh
status-publish` only accepts a plain message string — it doesn't cover the
full payload (phase + detail), so raw curl leaks back in (verbose, requires a
broad curl allowlist). Same anti-pattern as F1 but supervisor-side. Fix:
extend the bundled helper to cover the FULL agent.status shape + all message
types; route ALL broker calls through it (never raw curl), allowlisted by path.

**Conflict-detector escalation noise — additive overlaps flagged as
human-input conflicts** → v0.9.0 #9. With 5 agents, the detector escalated
FOUR `agent.question` "human input requested" items after a 120s window; a
diff-audit showed 3 of 4 were purely ADDITIVE (different structs; shared fns
with hunks 75–112 lines apart → clean merge), and only one
(`coordination.md`, both inserting at anchor L82) was a true collision. The
detector flags any same-file in-flight overlap with no notion of additive vs
conflicting hunks — in an unattended run these false alarms stall the wave.
Fix: before escalating, test whether the overlapping hunks are
line-adjacent/same-anchor vs disjoint; only escalate true collisions,
downgrade additive overlaps to informational `agent.feedback`.

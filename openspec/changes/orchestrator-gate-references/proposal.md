## Why

MILESTONE v0.20.0's second half — orchestrator **subagents** (an explorer persona and one
gate-auditor persona per audit gate, defined per CLI with their own role, tools and model) — was
written for an orchestrator running *inside* the session. v0.18.0 moves the orchestrator out: it
becomes the operator's own agent CLI, which already has subagents natively, so git-paw no longer
needs to define personas for it. What remains is the problem the personas were for: the
orchestrator skill is ~2,050 lines (v0.19.0) loaded whole into the driving session's context, and the
five-gate run floods that context and serialises. v0.18.0's `orchestrator-skill-export` shipped
without gate-auditor skills and left them to v0.20.0; this is that change, done CLI-agnostically on
top of the v0.20.0 packages.

## What Changes

- **No vendor persona files ship.** git-paw exports no CLI-specific subagent files (no vendor agents
  directory, no model or tool lists). Purpose-built personas are configuration instead: the
  follow-on v0.20.0 change `agent-profiles` defines delegate profiles — a CLI pinned to a model plus
  a role skill, with a stated purpose — that git-paw runs headless, ships auditor role skills that
  render this change's gate references, and puts delegate profiles first in this change's delegation
  contract. Rationale in design D1.
- **The three audit gates become package references.** The spec-audit, doc-audit and
  security-audit procedures move (not copy) out of the orchestrator `SKILL.md` into
  `references/gates/{spec-audit,doc-audit,security-audit}.md`, each self-contained: its inputs
  (re-resolved tip, merge-base diff, change id, the gate's rendered command), v0.19.0's rule to run
  that command bare and never pipe it, its procedure, its verdict format. Testing and regression
  stay in `SKILL.md` — they are mechanical and gate the rest. Each audit gate keeps its Workflow
  step (6, 6a, 6b today) as a pointer naming the installed reference path
  (`.git-paw/skills/orchestrator/references/gates/<gate>.md`), so the headless escalation one-shot,
  which is handed only `SKILL.md`, still reaches the gates. The always-loaded skill shrinks
  (agentskills.io progressive disclosure); each reference is a ready-made brief for an isolated
  context.
- **A CLI-agnostic delegation contract in `SKILL.md`.** When the orchestrator's CLI can run isolated
  subagents, it may run the three audits concurrently, each handed only its reference plus
  branch/tip/change id, each returning a compressed verdict (gate, pass/fail, one line per finding
  with location and the `feedback-gate` prefix). Gate order and the all-five-pass rule for
  `agent.verified` are unchanged. Without subagent support, the orchestrator reads the reference and
  runs the gate inline. Broad read-only exploration may be delegated the same way, returning a map.
- **Package-wide content rule.** Existing requirements that the orchestrator skill contain gate
  content are satisfied by the package (SKILL.md plus the references it names).
- **MCP `get_skill` returns the whole package** — `SKILL.md` followed by each reference — so MCP
  clients lose no gate content.
- **Self-hosted sessions run the branch's own build (DF-58).** When the repository being
  orchestrated is git-paw's own source, the orchestrator skill tells the orchestrator to confirm at
  bootstrap that the `git paw` the session runs is the one built from the branch under test, not an
  installed copy, and to have the human rebuild and restart on that build otherwise — a stale
  installed binary silently brings back bugs the branch has fixed. Phrased generically: it names no
  build tool or command (the project's own build supplies it).

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `skill-agent`: orchestrator package carries the audit gates as self-contained references, with
  Workflow pointers; delegation contract (verdict shape, orchestrator-only publishing, inline
  fallback); package-wide satisfaction of orchestrator-skill content requirements; self-hosted
  sessions run the binary built from the branch under test.
- `mcp-server`: `get_skill` includes a package's references.

## Impact

- **Assets:** `assets/agent-skills/orchestrator/SKILL.md` (today the flat
  `assets/agent-skills/orchestrator.md`, 2,051 lines; `skill-package-layout` moves it): the bodies
  of Workflow steps 6 (Spec audit), 6a (Doc audit), 6b (Security audit) and the Spec Audit
  Procedure with its governance sub-step move out; a short "Delegating gates" section, per-gate
  pointers, and the self-hosted-session check (in Bootstrap) are added.
  `assets/agent-skills/orchestrator/references/gates/*.md` (new, rendered with the gate-command
  placeholders).
- **Code:** `src/skills.rs` (render references; full-package render helper), registry entries,
  `src/mcp/tools/project.rs` (`get_skill` content = package; today it serves the orchestrator as
  `render_standardized` output, and
  `get_skill_serves_the_orchestrator_skill_as_exported_under_both_names` compares against that, so
  the test moves to the package render). Installation of references is already covered by
  `skill-package-deploy`.
- **Tests:** the orchestrator skill-content tests (`src/skills.rs` tests, `tests/*_skill_content.rs`
  — `verify_at_tip_skill_content.rs` pins the shared tip / merge-base rules, which stay in
  `SKILL.md` and are restated in each reference — `tests/governance_supervisor_boot.rs`,
  `tests/skill_stuck_shapes_prose.rs`, `tests/lang_agnostic_skill_audit.rs`) assert against the
  rendered package rather than `SKILL.md` alone; new tests for reference self-containment, the
  Workflow pointers, the delegation section, the self-hosted-session check, and `get_skill`.
- **Docs:** `docs/src/user-guide/supervisor.md` (verification section: references + delegation),
  `docs/src/user-guide/skill-templates.md` (references), `docs/src/user-guide/mcp.md` (`get_skill`).
- **Export-agnosticism:** references are new export surfaces; `skill-package-layout`'s registry-driven
  audit covers them automatically. The delegation section names no vendor, model or persona format.
  The self-hosted-session check names git-paw (the tool driving the session, already named
  throughout the skill) but no build tool, package manager or recipe, and applies only when the
  orchestrated repository is git-paw's own source.
- **Enum-variant ripple:** none.
- **Cross-version dependencies:**
  - Builds on v0.18.0 `orchestrator-skill-export` (shipped: the orchestrator skill, its rename,
    its pane-free wording, its export to `.git-paw/skills/orchestrator/SKILL.md`) — this change
    restructures that content. v0.18.0 shipped neither gate-auditor skills nor a delegation
    section, so this change adds both.
  - v0.19.0 `worker-friction-fixes` (shipped) added "Run dev commands bare" and "Never pipe a
    must-succeed step"; a delegated context sees only its reference, so each reference restates the
    bare-command rule for its gate command.
  - v0.18.0 `orchestrator-escalation-handoff` (`supervisor-autonomous-orchestrator`): the headless
    one-shot is handed `.git-paw/skills/orchestrator/SKILL.md` and, for a merge call, told to
    verify against every gate (`assets/escalation-framing.md`); the Workflow pointers name the
    installed reference paths so that still works without changing the hand-off.
  - v0.18.0 `broker-verdict-authority` (verdicts via `sweep.sh verified` / `feedback-gate`) — the
    delegation contract leaves publishing to the orchestrator itself, never to a subagent.
  - Requires v0.20.0 `skill-package-layout` (references in the registry, audit) and
    `skill-package-deploy` (references installed and rendered).
  - Followed by v0.20.0 `agent-profiles`, which ADDs a `skill-agent` requirement putting configured
    delegate profiles ahead of the CLI's own subagents in the delegation section, and whose bundled
    `spec-auditor` / `doc-auditor` / `security-auditor` packages render these references (one source).
  - `mcp-server`'s *Project knowledge tools* (last modified by v0.17.0 `mcp-superpowers-support`,
    shipped) stays untouched; this change ADDs a separate requirement instead of modifying it.
- **Dependencies:** none.

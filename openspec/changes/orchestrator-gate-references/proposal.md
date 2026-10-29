## Why

MILESTONE v0.20.0's second half — orchestrator **subagents** (an explorer persona and one
gate-auditor persona per audit gate, defined per CLI with their own role, tools and model) — was
written for an orchestrator running *inside* the session. v0.18.0 moves the orchestrator out: it
becomes the operator's own agent CLI, which already has subagents natively, so git-paw no longer
needs to define personas for it. What remains is the problem the personas were for: the
orchestrator skill is ~1,970 lines loaded whole into the driving session's context, and the
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
  (re-resolved tip, merge-base diff, change id, the gate's rendered command), its procedure, its
  verdict format. Testing and regression stay in `SKILL.md` — they are mechanical and gate the
  rest. The always-loaded skill shrinks (agentskills.io progressive disclosure); each reference is a
  ready-made brief for an isolated context.
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

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `skill-agent`: orchestrator package carries the audit gates as references; delegation contract;
  package-wide satisfaction of orchestrator-skill content requirements.
- `mcp-server`: `get_skill` includes a package's references.

## Impact

- **Assets:** `assets/agent-skills/orchestrator/SKILL.md` (gate sections 6, 6a, 6b and the Spec
  Audit Procedure move out; a short "Delegating gates" section and per-gate pointers remain),
  `assets/agent-skills/orchestrator/references/gates/*.md` (new, rendered with the gate-command
  placeholders).
- **Code:** `src/skills.rs` (render references; full-package render helper), registry entries,
  `src/mcp/tools/project.rs` (`get_skill` content = package). Installation of references is already
  covered by `skill-package-deploy`.
- **Tests:** the orchestrator skill-content tests (`src/skills.rs` tests, `tests/*_skill_content.rs`,
  `tests/governance_supervisor_boot.rs`, `tests/lang_agnostic_skill_audit.rs`) assert against the
  rendered package rather than `SKILL.md` alone; new tests for reference self-containment, the
  delegation section, and `get_skill`.
- **Docs:** `docs/src/user-guide/supervisor.md` (verification section: references + delegation),
  `docs/src/user-guide/skill-templates.md` (references), `docs/src/user-guide/mcp.md` (`get_skill`).
- **Export-agnosticism:** references are new export surfaces; `skill-package-layout`'s registry-driven
  audit covers them automatically. The delegation section names no vendor, model or persona format.
- **Enum-variant ripple:** none.
- **Cross-version dependencies:**
  - Requires v0.18.0 `orchestrator-skill-export` (the orchestrator skill, its rename and its
    pane-free wording) — this change restructures that content. v0.18.0 shipped neither
    gate-auditor skills nor a delegation section, so this change adds both.
  - v0.18.0 `broker-verdict-authority` (verdicts via `sweep.sh verified` / `feedback-gate`) — the
    delegation contract leaves publishing to the orchestrator itself, never to a subagent.
  - Requires v0.20.0 `skill-package-layout` (references in the registry, audit) and
    `skill-package-deploy` (references installed and rendered).
  - Followed by v0.20.0 `agent-profiles`, which ADDs a `skill-agent` requirement putting configured
    delegate profiles ahead of the CLI's own subagents in the delegation section, and whose bundled
    `spec-auditor` / `doc-auditor` / `security-auditor` packages render these references (one source).
  - v0.17.0 `mcp-superpowers-support` MODIFIES `mcp-server`'s *Project knowledge tools*; this change
    ADDs a separate requirement instead of modifying it.
- **Dependencies:** none.

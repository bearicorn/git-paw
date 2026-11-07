## Why

The v0.19 dogfood (findings DF-04, -08, -09, -12, -13, -19, -20, -38, -41, -43, -44, -47, -52) showed
workers losing time to friction git-paw itself causes: the injected coordination skill teaches raw
`curl` to the broker, so every intent and poll raises a permission prompt; the tracked `AGENTS.md`
tells every worker it is the orchestrator; doctor shows ✓ for a workflow command it could not find;
an emptied session can be neither refilled nor relaunched; and the orchestrator skill archives
merges it never confirmed landed.

## What Changes

- **Group A — helper-only broker traffic.** Every broker example in the coordination skill uses
  `.git-paw/scripts/broker.sh` verbs; no `curl` aimed at the broker remains. The helper gains the
  three forms the examples need and had no verb for: `status --activity <phrase>`, repeatable
  `intent --region <path>#<kind>=<value>`, and a `last_seq` cursor line printed by `poll`. The skill
  and the boot block say to run each helper call as a bare command (no `; echo`, no pipe).
- **Group B — orchestrator pointer scoped to the root session.** The `## git-paw orchestrator`
  section `git paw init` writes into the tracked `AGENTS.md` states it applies only to the session at
  the repository root, and that a coding agent in a worktree ignores it.
- **Group C — honest spec-workflow row.** When doctor cannot verify a CLI's workflow command
  reachability, the row is ⚠ (was ✓), with a remedy naming `[worktree.env] copy`. The
  spec-driven-launch guide documents provisioning workflow command files that way. git-paw still
  writes no agent CLI settings or command files.
- **Group D — an emptied session can be refilled.** A live session whose receipt records zero
  agents is alive (not "agents exited"), so `git paw add` works on it; a session that still records
  agents whose panes all closed stays "agents exited", so `start` recovers it. The pane probe stays
  receipt-blind; the receipt decides. `git paw start` with
  `--branches` / `--specs` / `--from-all-specs` against a session recording zero agents no longer
  ignores the flags: a stopped one is discarded and the selection launched fresh; a live one is
  refused with an error naming `git paw add`.
- **Group E — merges confirmed before archive.** The orchestrator skill's merge loop checks
  `git merge-base --is-ancestor <tip> HEAD` after each fast-forward, before archiving or
  publishing; forbids piping a must-succeed step; runs the spec archive only after that check; and
  re-runs `{{TEST_COMMAND}}` after any archive step that writes to the tracked tree, and names
  no literal target or agent branch.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `skill-agent`: coordination skill routes every broker call through `broker.sh` and asks for bare
  helper calls; orchestrator skill gains the landed-merge check and archive ordering.
- `broker-agent-helper`: `status --activity`, `intent --region`, and the `poll` cursor line.
- `boot-block`: the boot block asks for bare helper calls.
- `core-init`: the orchestrator pointer section is scoped to the root session.
- `preflight-diagnostics`: an unverified spec-workflow CLI is ⚠ with a `[worktree.env] copy` remedy.
- `session-state`: a live session whose receipt records zero agents is alive.
- `git-add-remove-branch`: `add` works on a live session with zero agents.
- `tmux-orchestration`: `start` never silently ignores launch-selection flags on a zero-agent session.

## Impact

- **Assets (exported, must stay project-agnostic):** `assets/agent-skills/coordination.md`,
  `assets/scripts/broker.sh`, `assets/boot-block-template.md`, `assets/boot-block-mcp-template.md`
  (if it carries the helper paragraph), `assets/agents-md-orchestrator-section.md`,
  `assets/agent-skills/orchestrator.md`.
- **Code:** `src/doctor.rs` (`check_spec_workflow`, hygiene probe), `src/session.rs`
  (`Session::reconcile_liveness`, applied by `src/commands/helpers.rs` `session_has_live_agents`,
  `status`, `pause` and doctor), `src/commands/start.rs` (`start`, `start_with_specs`,
  `revive_existing` callers), `src/commands/supervisor.rs`, `src/agents.rs` (worker role line, not
  written for the root session's own context).
- **Tests pinning text that changes:** `src/skills.rs` (`embedded_coordination_contains_all_operations`,
  the "When main advances" `/messages/{{BRANCH_ID}}` assertion), `src/agents.rs`
  (`embedded_coordination_still_contains_optin_operations`, orchestrator pointer tests),
  `tests/coordination_region_skill_content.rs`, `src/doctor_tests.rs` (unknown-CLI cases),
  `tests/doctor_e2e.rs`, `tests/broker_sh_conventions.rs`, `tests/lang_agnostic_skill_audit.rs`
  (must still pass), `tests/orchestrator_skill_content.rs`.
- **No enum variant or struct field is added** (`BrokerMessage`, `SpecBackendKind`, `Session`,
  `WorktreeEntry` unchanged). The wire payloads the helper emits already exist (`activity`,
  region objects).
- **Docs:** `docs/src/user-guide/coordination.md`, `docs/src/user-guide/spec-driven-launch.md`,
  `docs/src/user-guide/doctor.md`, `docs/src/user-guide/session-lifecycle.md`,
  `docs/src/cli-reference.md` (start/add behaviour notes).

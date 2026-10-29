## Why

The 1.0 theme is that pure-manual git-paw is a complete product and every advanced feature is
an opt-in layer that installs only its own surface — so a project can stay manual forever, or
opt in tier by tier, and turning a tier off takes its surface away. The freeze locks the
install paths, so they must be right first. Checked against the code:

- **Per-session pure manual is already clean, but by accident.** Hooks, the coordination skill,
  `broker.sh` and the boot block are gated on `[broker] enabled` in `cmd_start`,
  `cmd_start_with_specs` and `attach_agent` — through about 30 hand-written
  `broker_config.enabled` checks in `commands/{start,supervisor,add}.rs` and `main.rs`. The
  spec disagrees: `git-hook-injection`'s *Hook installation by git paw start* requires hooks on
  every start.
- **`git paw init` leaks.** It installs `sweep.sh`, `broker.sh`, `docs-fetch.sh` and
  `_paw_common.sh` unconditionally, whatever the config enables.
- **Coordinated without the broker is incoherent.** Answering "yes" to init's coordinated-mode
  prompt writes `default_mode = "coordinated"` (before v0.21.0, `[supervisor] enabled = true`) and
  no `[broker]`. Such a session still runs a broker inside the `__dashboard` pane, yet its agents
  get no hooks, no `broker.sh` and no coordination skill — while their boot block tells them to run
  `.git-paw/scripts/broker.sh`, and the inter-agent rules tell them a post-commit hook publishes
  for them and to poll `/messages`. After v0.18.0's `broker-host-separation` the broker host starts
  only when `[broker] enabled`, so the same config would leave the drive loop and verdict
  publishing with no broker at all.
- **Turning a layer off leaves its surface.** git-paw's managed blocks in the common git hooks
  are never stripped (`git-hook-injection` only says purge SHOULD), and init never removes a
  helper it wrote.

## What Changes

- A **layer model for the pane-less tool**: Manual (always) → Skills (`docs_base_url`) →
  Broker (`[broker] enabled`; the dashboard is an optional view of it, off by default since
  v0.21.0) → Coordinated (`--coordinated` /
  `default_mode = "coordinated"`, requires Broker) → Unattended (`--unattended`, requires
  Coordinated). MCP is a client, not a layer: it installs nothing. `[worktree.*]` and `[logging]`
  stay independent opt-ins of the manual baseline.
- **One pure resolver and one surface plan** decide the layers and the exact artifacts; every
  launch path, `git paw add`, session recovery and `git paw init` use them, replacing the
  scattered `broker_config.enabled` gates.
- **BREAKING (behaviour):** a coordinated or unattended session always gets the Broker layer.
  With `[broker] enabled` false or absent it now also gets the git hooks, `broker.sh`, the
  coordination skill and `GIT_PAW_BROKER_URL`, and git-paw prints a one-line notice. This makes
  the boot block and inter-agent rules those agents already receive true.
- **Hooks only with the Broker layer**, on every path that creates a worktree; `purge` strips
  git-paw's managed hook blocks once no git-paw worktree remains, keeping the user's hook content.
- **Instruction content per layer**: the sidecar's managed block, the skills in it and the first
  prompt name only surfaces of enabled layers — no broker endpoint, helper or hook text without
  the Broker layer; no inter-agent rules or orchestrator procedure without Coordinated.
- **BREAKING (behaviour):** `git paw init` installs the repository-level files (helper scripts,
  the orchestrator skill and its `AGENTS.md` pointer from v0.18.0) only for layers the config
  enables, and removes git-paw-managed files of layers it does not; a pure-manual init writes no
  helper script. `git paw start` provisions (never removes) the files its session's layers need,
  so `start --coordinated` works in a repo whose config leaves coordinated mode off.
- `git paw add` and recovery reuse the session's layers instead of re-reading the config.
- A **modes** chapter in the user guide and a **CI layer matrix** that launches each layer
  combination for real and asserts it installed exactly its plan; the manual row also asserts no
  broker token appears in anything git-paw wrote.

## Capabilities

### New Capabilities

- `core-layer-isolation`: the layer model, the resolution of a session's layers and their
  dependencies, the per-layer surface plan, layer-scoped instruction content and
  repository-level files, layer reuse by `add` and recovery, the modes chapter and the CI
  matrix. Purpose for the archived spec: "Compose every git-paw session from opt-in layers on
  top of a pure-manual baseline — resolving which layers a session has, enforcing their
  dependencies, and installing exactly those layers' surface and nothing else."

### Modified Capabilities

- `git-hook-injection`: *Hook installation by git paw start* (Broker layer only, every launch
  path); *Hooks and markers are cleaned up on purge* (managed blocks stripped — SHOULD becomes
  SHALL).
- `core-init`: *Init installs the agent-broker helper script* and *Init installs the shared
  shell preamble* (per enabled layer; removal reported); *Init installs the orchestrator skill
  in the standardized format* and *Init injects an orchestrator pointer section into the root
  AGENTS.md* (Coordinated layer only) — these two are added by v0.18.0's
  `orchestrator-skill-export` and are modified here on top of that draft's text.
- `skill-agent`: *`git paw init` SHALL install the bundled sweep helper* (Coordinated layer
  only).
- `mcp-agent-docs`: *Bundled docs-fetch skill and helper* (Skills layer only), on top of v0.17.0
  `classifier-retirement`'s text: init installs the helper at an exact path a permission rule can
  name, and grants nothing itself (since v0.17.0 only `git paw setup-cli` writes grants).
- `broker-runtime`: *Start flow conditionally creates dashboard pane* (the broker condition of the
  v0.21.0 opt-in becomes the resolved Broker layer, so a coordinated session without `[broker]`
  can show its view when the dashboard is enabled; it stays off by default). The dashboard still
  sends no keystrokes to any other pane (v0.17.0 `classifier-retirement`).
- `core-layer-isolation` (new) also states how `git paw doctor`'s CLI setup check
  (`preflight-diagnostics`, from v0.17.0 `cli-setup-consent`) reads a granted helper that a
  disabled layer removed: not installed, not modified. It does not modify that requirement.

## Impact

- **Code:** new pure module (`src/layers.rs`: `LayerSet`, `resolve_layers`, `SurfacePlan`,
  `surface_plan`) at the core layer; `src/commands/{start,supervisor,add,recover}.rs` and
  `src/main.rs::attach_agent` read `LayerSet` instead of `broker_config.enabled`;
  `src/init.rs::run_init` / `provision_worktree_helpers` install per plan and remove disabled
  files; `src/agents.rs` gains managed-hook-block stripping (inverse of `chain_hook`) called from
  `cmd_purge`; the coordinated launch path (`src/commands/supervisor.rs`) gains the
  implied-broker notice; `src/doctor.rs`'s CLI setup check reports an absent granted helper as
  not installed rather than modified.
- **Persisted state:** none new; `add` and recovery derive Coordinated from `Session.mode` and
  Broker from `Session.broker_port`.
- **Tests:** new `tests/layer_isolation_matrix.rs` (+ a CI job running one combination per
  matrix entry), unit tables for `resolve_layers` / `surface_plan`, a modes-chapter parity test;
  `tests/cli_init_writes_*_script.rs` and `hook_integration.rs` gain layer preconditions; the
  roughly 15 coordinated-mode tests that configure coordinated mode without `[broker]` now see
  broker surface and are re-pinned.
- **Docs:** new `docs/src/user-guide/modes.md`; `SUMMARY.md`; `user-guide/coordinated-mode.md`
  (renamed from `supervisor.md` by `remove-supervisor-vocabulary`) and `configuration/README.md`
  (`default_mode = "coordinated"` implies the broker); `architecture.md`; README quick-start
  notes; `quick-start-*` pages that run `git paw init`.
- **Export surfaces:** init's installed helpers and the orchestrator pointer section become
  conditional — the `export-agnosticism` inventory notes the gates; no exported content changes.
- **Enum-variant ripple:** none.
- **Security / safety:** strictly narrower by default — a pure-manual repo gets no hooks in
  `.git/hooks/`, no helper scripts and no pointer; the implied broker binds `127.0.0.1` as before.
  Hook stripping edits only content between git-paw's own markers.
- **Cross-version dependencies:**
  - v0.17.0 `bundled-helper-discovery` (worktree helper provisioning per layer, which this
    extends to the repo root), `start-spec-launch-parity` (task prompt without the broker; boot
    block only with it), `purge-unmerged-branch-safety` (touches the purge flow this extends).
  - v0.17.0 `classifier-retirement` MODIFIES *Bundled docs-fetch skill and helper* (exact install
    path, no grant) and *Start flow conditionally creates dashboard pane* ("sends no keystrokes to
    any other pane"); this change's deltas keep both. It also deletes the attended auto-approver,
    so the Coordinated layer's surface no longer lists one. v0.17.0 `cli-setup-consent` adds the
    doctor CLI setup check whose modified-helper ⚠ the new *A helper removed with its layer is
    reported as not installed, not as modified* qualifies; if v0.20.0 `skill-package-deploy`
    moves the granted helpers into skill packages, the same rule applies to the package paths.
  - v0.18.0 `supervisor-pane-removal` (the pane-less coordinated layout; *Start flow
    conditionally creates dashboard pane* is modified on top of its version),
    `broker-host-separation` (the broker host that the implied Broker layer launches),
    `orchestrator-skill-export` (the orchestrator `SKILL.md` and root `AGENTS.md` pointer that
    become Coordinated-only — its two `core-init` requirements are modified here on top of the
    draft text and must exist in `openspec/specs/core-init` before this change archives),
    `orchestrator-escalation-handoff` (unattended hand-off).
  - v0.21.0 `supervisor-vocabulary-split` (the `--coordinated` / `--bare` flags, `default_mode`
    and `[orchestrator.gates]` this change's text uses; its migration drops the deprecated
    `[supervisor.auto_approve]` table rather than renaming it into `[workers]`) and v1.0.0
    `remove-supervisor-vocabulary` (removes the old spellings; renames the user-guide page).
  - *Start flow conditionally creates dashboard pane* is also modified by v0.18.0
    `supervisor-pane-removal` and v0.21.0 `dashboard-usage-view`. This delta is written on top of
    the v0.21.0 version (and uses `--from-all-specs`, per `remove-from-specs-alias`).
  - v0.20.0 skills packaging — if helpers move into skill packages, "helper scripts" here means
    the packages that own them; the per-layer rule is unchanged.
  - v0.21.0 `dashboard-usage-view` answered v0.18.0's "does the dashboard survive?" question. The
    dashboard is an opt-in usage-and-events view (`[dashboard] enabled` / `start --dashboard`, off
    by default). This change keeps that switch and only re-keys its broker condition on the
    resolved Broker layer.

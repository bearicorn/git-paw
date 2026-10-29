## Context

The install and inject paths, as they are at the tip this change was written against (Rust
identifiers keep their pre-rename names; only the user-facing vocabulary changed in v0.21.0):

| Surface | Where it is installed | Gate today |
|---|---|---|
| git hooks + `paw-agent-id` marker | `agents::install_git_hooks`, called from `commands/start.rs` (`cmd_start`, `cmd_start_with_specs`) and `main.rs::attach_agent` (coordinated launch, `add`) | `broker_config.enabled` at each call site |
| worktree `broker.sh` / `docs-fetch.sh` / `_paw_common.sh` | `init::provision_worktree_helpers` (from `attach_agent`; v0.17.0 `bundled-helper-discovery` extends it to every path) | broker enabled / `docs_base_url` set |
| coordination skill in the sidecar block | `skills::resolve("coordination")` in `cmd_start`, `cmd_start_with_specs`, `cmd_supervisor`, `cmd_add` | `broker_config.enabled` |
| docs-fetch skill | same call sites | `config.docs_base_url.is_some()` |
| inter-agent rules (`assets/inter-agent-rules.md`: post-commit hook, `/messages` poll, `agent.blocked`) | `attach_agent` via `inter_agent_rules_peers` | coordinated — **not** broker |
| boot block (`broker.sh` / MCP publish calls) | `attach_agent` → `skills::build_boot_block` | coordinated — **not** broker; bare/spec paths gate on broker |
| dashboard pane (hosts the broker until v0.18.0; opt-in from v0.21.0) | `commands/start.rs`, `commands/supervisor.rs` | bare: broker; coordinated: **always** (v0.21.0: `[dashboard] enabled` / `--dashboard` and the broker) |
| `GIT_PAW_BROKER_URL`, `state.broker_port` | `cmd_start*`, `cmd_supervisor` | `broker_config.enabled` |
| repo `.git-paw/scripts/{_paw_common,sweep,broker,docs-fetch}.sh` | `init::run_init` | **none — always** |
| orchestrator skill → repo-root sidecar | `cmd_supervisor` → `setup_worktree_agents_md(repo_root, repo_root, …)` | coordinated |

`BrokerConfig::default().enabled` is `false`, and init's mode prompt writes only
`default_mode = "coordinated" | "bare"` (before v0.21.0, `[supervisor] enabled = true|false`), so
the config init produces for a coordinated repo has no broker. About 15 tests
(`tests/e2e_supervisor_launch.rs`, `supervisor_integration.rs`, `add_remove_e2e.rs`, …) configure
exactly that. `cmd_dashboard` builds `BrokerState` without checking `broker.enabled`, so those
sessions do run a broker — on the default port, but with no URL exported to agents and no hooks,
while the boot block and inter-agent rules assume both. After v0.18.0, `broker-host-separation`
launches the `__broker` host only "when the broker is enabled", and the dashboard becomes a thin
client, so the same config would have no broker.

The MILESTONE's claim that the post-commit hook "leaks into all sessions regardless of config" is
stale for the per-session path (it is gated) but the spec still requires it unconditionally
(`git-hook-injection` *Hook installation by git paw start*), and the repo-level leak through
`init` is real.

## Goals / Non-Goals

**Goals:**
- One definition of "which layers does this session have" and "what does each layer install".
- Pure manual installs nothing beyond worktrees and their assignment; every tier installs exactly
  its own surface; turning a tier off removes its repository-level surface.
- The coordinated-without-broker combination made coherent.
- A CI matrix that would catch a regression at any tier.

**Non-Goals:**
- Any change to the dashboard itself. v0.21.0 `dashboard-usage-view` made it an opt-in view
  (`[dashboard] enabled`, off by default); this change only re-keys that switch's broker
  condition on the resolved Broker layer.
- Changing what any layer's surface *contains* (skill prose, hook bodies, boot block text).
- Layer-scoping `.gitignore` entries (preventive and inert; left as is).
- Recording layers in the session file (derived instead; v0.21.0 owns the session DTO).

## Decisions

### D1 — The layer model after the orchestrator relocation

Manual → Skills → Broker → Coordinated → Unattended. The old "+ Supervisor" layer (an in-session
LLM pane) no longer exists after v0.18.0, and v0.21.0 renamed what remains: the *coordinated
session* — git-paw's coordination machinery (drive loop, conflict detector, learnings, inter-agent
rules) plus the hand-off of judgment to the external orchestrator, whose context (skill, root
sidecar, pointer) git-paw installs. Unattended is split out because it adds its own surface (the
foreground loop, the drive-loop directive). The dashboard is an optional view of the Broker layer,
off by default. In the code the broker needed the dashboard, not the reverse. After v0.18.0 the
view is a thin client of the broker, and v0.21.0 made it opt-in (`[dashboard] enabled` /
`--dashboard`). It is a switch inside the Broker layer, not a layer, because it installs nothing
but its own pane. MCP is client-spawned and installs nothing, so it is not a layer. A2A (v1.2.0) is out
of scope. "Skills" is kept as a layer for the one broker-free skill (docs-fetch, gated on
`docs_base_url`); every other skill belongs to the layer whose surface it describes —
coordination to Broker, orchestrator to Coordinated — which is how "render skill content per
enabled layer" is satisfied.

### D2 — Coordinated implies Broker (with a notice), rather than an error or a stripped mode

Every coordinated capability — the drive loop's status reads, verdict publishing (v0.18.0
`broker-verdict-authority`), the conflict detector, learnings, the boot block, the inter-agent
rules — runs on the broker; `broker-runtime` already says coordinated mode "is not meaningful"
without it. Of the three ways to make the combination coherent:

- **Imply the broker (chosen).** Keeps every existing config launching. The init-default
  coordinated config and the tests that use it now get hooks, `broker.sh`, the coordination skill
  and the URL — which is what their boot block and rules already claimed. A one-line notice says
  so. This is the behaviour change the proposal marks BREAKING.
- *Fail fast* ("coordinated sessions need `[broker] enabled = true`") — breaks every repo init
  configured for coordinated mode, for no benefit.
- *Strip broker text from coordinated sessions* — leaves a coordinated session with no drive-loop
  inputs and no way to publish a verdict; not a useful mode.

### D3 — Functional core: `resolve_layers` and `surface_plan`

`src/layers.rs` (core, no I/O): `LayerSet` (a small bitset or five bools behind accessors),
`resolve_layers(&LayerInputs) -> LayerSet` where `LayerInputs` is config-derived data plus
`--coordinated` / `--bare` / `--unattended` and the prompt answer (the prompt itself stays in the
shell, with the start-mode resolution), and `surface_plan(LayerSet, &PlanInputs) -> SurfacePlan`
listing artifacts as data (hook names, script names per location, managed-block sections, prompt
parts, panes, env vars). The launch shells (`cmd_start`, `cmd_start_with_specs`,
`cmd_supervisor`/`attach_agent`, `cmd_add`, recovery, `run_init`) apply the plan through the
existing adapters (`install_git_hooks`, `provision_worktree_helpers`, `setup_worktree_agents_md`,
tmux builders). This replaces the ~30 `broker_config.enabled` checks with `layers.broker()` and
lets the CI matrix compare "installed" against the same plan the code used — one source of truth.
Seam-first: the first step introduces `LayerSet` with exactly today's semantics (Broker =
`[broker] enabled`), routes every gate through it with no behaviour change, and only then flips the
Coordinated ⇒ Broker implication in its own commit.

*Alternative rejected:* a `Layer` trait with an `install()` method per layer — runtime
polymorphism for a fixed, five-member set; the enum/bitset lets the compiler enumerate sites, per
`architecture-direction.md` ("use an enum when the set is small and fixed").

### D4 — `init` reconciles repository files to the config; `start` only provisions

`init` is the place a user changes a repository's standing setup, so it both installs and removes
(binary-managed files only — the same ownership model under which it already overwrites them).
`start` may run with a flag the config does not have (`--coordinated`), so it provisions what its
session needs and never deletes. A later `init` may then remove a flag-provisioned `sweep.sh`;
the next coordinated `start` puts it back. The root `AGENTS.md` pointer is removed by deleting
only its own marker pair (mirroring `agents::remove_git_paw_section` for the
`<!-- git-paw-orchestrator:… -->` markers).

Removing a helper never touches the agent CLI's settings. `git paw setup-cli`'s grant table
(v0.17.0) is fixed and independent of configuration, so a repository can hold a grant for a helper
whose layer is off. Doctor's CLI setup check reads such a helper as *not installed*, not as
*modified*: a missing file is not a tamper, and the tamper rule from the same setup write still
denies the agent's file tools that path. A helper that is present and differs stays the ⚠ case.
git-paw does not remove the grant; the user runs `git paw setup-cli <cli> --undo` if they want it
gone.

### D5 — Purge strips git-paw's hook blocks once no marked worktree remains

The inverse of `agents::chain_hook`: remove the `HOOK_START_MARKER`…`HOOK_END_MARKER` block;
if what remains is empty or only `#!/bin/sh`, delete the file. Guarded on "no linked worktree
still carries a `paw-agent-id` marker" (enumerated via `git worktree list --porcelain` and each
gitdir), so an agent detached with `remove --keep-worktree` keeps its hooks. `chain_hook` trims
trailing whitespace of the pre-existing hook when it appends; that trim is not reversible, which
the spec tolerates (it promises no change to lines outside the markers).

### D6 — `add` and recovery derive layers from the saved session

A coordinated `Session.mode` (written `"coordinated"` from 1.0.0, `"supervisor"` by 0.x) ⇒
Coordinated; `Session.broker_port.is_some()` ⇒ Broker. Skills follow the current config because
`docs_base_url` is not saved and adding it to the session schema belongs to v0.21.0's DTO work.
Today `cmd_add` re-reads `config.broker.enabled`; this fixes a latent mismatch rather than adding a
feature.

### D7 — Pre-push and pre-commit stay in the Broker layer

They share the `paw-agent-id` marker with the post-commit dispatcher (`install_git_hooks`
installs all three), and that marker carries `PAW_BROKER_URL`. Moving the push block and the
branch guard to the Manual layer would protect pure-manual agents too, but changes pure-manual
behaviour (pushes from agent worktrees start failing) — not a freeze-time change. Open question.

### D8 — CI matrix design

`tests/layer_isolation_matrix.rs`: one table row per combination, each building a throwaway repo
(`tests/helpers::setup_test_repo`), an isolated `HOME`, a private tmux socket
(`tests/helpers::tmux_test_env`), an OS-assigned broker port, and a stub CLI (`cat`, as
`selftest` does); it runs `git paw init` (non-interactive, with the row's config) and
`git paw start` (non-TTY, detached), then inventories hooks, markers, scripts, sidecar sections,
the session environment, panes and the first prompt sent to each pane (from the pane capture),
and diffs that inventory against `layers::surface_plan` for the row. The Unattended row sets the
existing `GIT_PAW_DRIVE_POLL_MS` / `GIT_PAW_DRIVE_HEARTBEAT_MS` overrides so the loop exits in
seconds. A CI job `layer-matrix` (ubuntu, tmux installed) uses `strategy.matrix.combo` and runs
`cargo test --test layer_isolation_matrix -- <combo>` so each combination reports separately.

### D9 — Modes chapter parity

`docs/src/user-guide/modes.md` carries one section per layer with a bulleted artifact list; a
test reads the chapter and checks every `LayerSet` member and every artifact name
`surface_plan` can emit appears. The same names are used in `--help` for `--coordinated` /
`--unattended` ("enables the broker").

## Risks / Trade-offs

- [Coordinated repos without `[broker]` start installing hooks into `.git/hooks/`] → that is the
  coherent behaviour their agents were already told about; the notice and the changelog's
  `BREAKING CHANGE:` footer make it visible; `purge` now removes the hooks.
- [A pure-manual user relied on `init` writing `sweep.sh` / `broker.sh`] → the files only work
  against a broker or a coordinated session, both of which now provision them on `start`.
- [Test churn: ~15 coordinated-mode tests re-pinned] → done in the implication commit, after the
  behaviour-preserving seam commit, so the diff shows exactly which expectations moved.
- [Removing files in `init`] → limited to git-paw's binary-managed set and reported line by
  line; a consumer who tracks `.git-paw/scripts/` sees the deletion in `git status`.
- [The v0.18.0 `core-init` requirements this modifies change before they archive] → rebase this
  delta on their archived text.

## Migration Plan

1. `src/layers.rs` + unit tables; route every gate through `LayerSet` with today's semantics
   (no behaviour change; the existing suite must pass unchanged).
2. Flip Coordinated ⇒ Broker; add the notice; re-pin the affected tests.
3. Repository-level reconciliation in `init`; provisioning in `start`; v0.18.0's orchestrator
   export gated on Coordinated.
4. Hook stripping on purge.
5. `add` / recovery from the saved session.
6. CI matrix, modes chapter and its parity test, docs.

Rollback: revert; persisted state is unchanged (no new fields).

## Open Questions

- **Dashboard switch.** Resolved by v0.21.0 `dashboard-usage-view`: `[dashboard] enabled` /
  `start --dashboard`, off by default, inside the Broker layer. The surface plan takes "dashboard
  enabled" as a plan input next to the layer set.
- **Pre-push / branch guard in pure manual** (D7). Recommendation: leave in Broker for 1.0.
- **Should init write `[broker] enabled = true` when the user chooses coordinated mode?** Not
  needed with the implication; recommendation: no, to keep init's config minimal.
- **Skills after v0.20.0.** If skills become packages that own their scripts, "helper scripts"
  in this change means the packages; the per-layer gates are unchanged.

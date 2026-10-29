## 1. Seam: layers as data (no behaviour change)

- [ ] 1.1 Add `src/layers.rs` (core, no I/O): `LayerSet`, `LayerInputs`, `resolve_layers`, `SurfacePlan`, `surface_plan`; module and item docs; Broker resolves from `[broker] enabled` only in this step
- [ ] 1.2 Unit tables: the resolution table (every row of "Resolution table" plus `--bare` precedence) and "The resolution is pure"
- [ ] 1.3 Unit tables: `surface_plan` per layer and the union property, with "dashboard enabled" as a plan input. They cover "The manual plan installs nothing…", "The broker plan has no coordinated surface", "The plan is the union of its layers" and "The dashboard pane follows its own switch".
- [ ] 1.4 Route every `broker_config.enabled` / coordinated gate in `commands/{start,supervisor,add,recover}.rs`, `main.rs::attach_agent` and `init.rs` through `LayerSet`; grep `src/` for `broker_config.enabled` and `broker.enabled` until only `layers.rs` reads them
- [ ] 1.5 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast` passes unchanged (behaviour-preserving checkpoint; commit here)

## 2. Coordinated implies Broker

- [ ] 2.1 `resolve_layers`: Coordinated (and therefore Unattended) adds Broker; the shell prints the implied-broker notice once when the config did not enable it
- [ ] 2.2 Every Broker-layer artifact follows the resolved layer in the coordinated path: broker host, the dashboard's broker condition (the pane still needs `[dashboard] enabled` / `--dashboard`), `GIT_PAW_BROKER_URL`, `state.broker_port`, hooks + marker, worktree `broker.sh`, coordination skill, boot block
- [ ] 2.3 Integration tests: "The init-default coordinated config gets a working broker", "No notice when the broker is configured", "A non-coordinated session without the broker stays manual", "A coordinated session without a broker section gets hooks", and the `broker-runtime` scenarios "A coordinated session has no dashboard by default", "The config key enables the dashboard in a bare broker session", "The flag enables the dashboard in a coordinated session", "A coordinated session without a broker section can show the dashboard", "The flag without the Broker layer fails before launching", "Broker disabled produces no dashboard pane", "Dashboard pane title"
- [ ] 2.4 Re-pin the coordinated-mode tests that configure coordinated mode without `[broker]` (`e2e_supervisor_launch.rs`, `supervisor_integration.rs`, `add_remove_e2e.rs`, `e2e_supervisor_returns.rs`, `cli_supervisor_non_tty.rs`, …) to the new surface; list each changed expectation in the commit body

## 3. Instruction content per layer

- [ ] 3.1 Managed block and first-prompt composition read the plan: coordination only with Broker, docs-fetch only with Skills, inter-agent rules only with Coordinated, boot block only with Broker, task prompt on spec-driven launches
- [ ] 3.2 Tests: "A manual session's instructions carry no broker text", "A skills-only session carries the docs skill without broker text", "A broker session carries coordination but no coordinated surface" (assert on the written sidecars and the captured first prompts, using stable anchors, not prose pins)

## 4. Hooks

- [ ] 4.1 Hooks + marker installed only with Broker on every worktree-creating path, including `add` and recovery; tests "Hooks installed in every agent worktree" (with a Broker precondition), "Idempotent re-install", "No hooks without the broker layer"
- [ ] 4.2 Add the managed-block stripper (inverse of `agents::chain_hook`; delete a file left with only a shebang) and call it from the purge flow when no linked worktree carries a `paw-agent-id` marker; unit table for the stripper (user content before/after the block, block only, no block, truncated block left untouched)
- [ ] 4.3 Tests: "Purge removes the per-worktree marker", "Purge strips the managed block and keeps the user's hook", "A hook file git-paw alone wrote is deleted", "Hooks stay while another git-paw worktree remains"
- [ ] 4.4 Security review (`security-and-safety-review`): stripping touches only lines between git-paw's own markers in the common hooks dir; no path is built from untrusted input

## 5. Repository-level files

- [ ] 5.1 `init::run_init` resolves layers from the resulting config and installs `broker.sh` (Broker), `sweep.sh` (Coordinated), `docs-fetch.sh` (Skills), `_paw_common.sh` (any of them); removes git-paw-managed copies for disabled layers; reports created / updated / removed
- [ ] 5.2 Gate v0.18.0's orchestrator `SKILL.md` and the root `AGENTS.md` pointer on Coordinated in `init`; remove them (pointer: its marker pair only, never creating `AGENTS.md`) when Coordinated is off
- [ ] 5.3 `start` provisions (never removes) the repository-level files of its session's layers before the first prompt
- [ ] 5.4 Tests: "A pure-manual init writes no layer files", "Turning a layer off and re-running init removes its files", "A flag-enabled layer is provisioned by start", "Removing the pointer keeps the rest of AGENTS.md", plus each modified `core-init`, `skill-agent` and `mcp-agent-docs` scenario (extend `tests/cli_init_writes_{broker,common,docs_fetch,sweep}_script.rs` and `cli_init_writes_supervisor_block.rs` with layer preconditions and the removal rows)
- [ ] 5.5 `src/doctor.rs` CLI setup check: a granted helper that is absent (its layer off) is reported in the detail as not installed and never as modified; a present helper that differs stays ⚠. Tests: "A helper removed with its layer is not a modified helper", "A present but modified helper is still flagged" (assert the settings file is byte-identical across the init that removed the helper)
- [ ] 5.6 `export-agnosticism` dev skill inventory: note that init's helper scripts and the orchestrator pointer are now layer-gated; `tests/lang_agnostic_skill_audit.rs` unaffected

## 6. Add and recovery

- [ ] 6.1 `cmd_add` and recovery derive Coordinated from `Session.mode` (either spelling) and Broker from `Session.broker_port`; Skills from current config
- [ ] 6.2 Tests: "add matches the running session after a config edit", "Recovery reinstalls the launch-time layers"

## 7. CI matrix

- [ ] 7.1 `tests/layer_isolation_matrix.rs`: one row per combination (Manual; Manual+Skills; Manual+Broker; Manual+Skills+Broker; Coordinated; Unattended) in an isolated environment with a stub CLI; inventory vs `surface_plan`; the Manual row also scans every file git-paw wrote for the broker tokens; failures name the combination and artifact — covers the three matrix scenarios
- [ ] 7.2 `.github/workflows/ci.yml`: a `layer-matrix` job (ubuntu, tmux installed) with `strategy.matrix.combo`, each running `cargo test --test layer_isolation_matrix -- <combo>`
- [ ] 7.3 Prove the leak check: a temporary local patch installing `broker.sh` in Manual makes only the Manual row fail (do not commit the patch)

## 8. Docs

- [ ] 8.1 Write `docs/src/user-guide/modes.md` (each layer: how enabled, requirements, every artifact, how to turn off and what removes it; where the dashboard, an optional off-by-default view of the Broker layer, and MCP sit); add to `SUMMARY.md` and the user-guide index
- [ ] 8.2 Parity test: every layer and every `surface_plan` artifact is named in the chapter — covers "The chapter covers the plan" and "An undocumented artifact fails the check"
- [ ] 8.3 `--help` for `start --coordinated` / `--unattended`: state that they enable the broker
- [ ] 8.4 `configuration/README.md` (`[broker] enabled`, `default_mode = "coordinated"` implies the broker, `docs_base_url` gates the docs skill), `user-guide/coordinated-mode.md`, `user-guide/agents-md.md`, `architecture.md` (the layer model), README quick-start notes, any quick start that runs `git paw init` and expects scripts
- [ ] 8.5 Backfill `## Purpose` in `openspec/specs/core-layer-isolation/spec.md` at archive (text in proposal.md) and add the capability to the specifications page

## 9. Gates

- [ ] 9.1 Commit the implication (group 2) and the init reconciliation (group 5) as breaking: `!` plus a `BREAKING CHANGE:` footer each, describing the new surface and how to opt out
- [ ] 9.2 Every scenario in the six delta specs maps to a test named above
- [ ] 9.3 `cargo fmt`, then `just lint`
- [ ] 9.4 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 9.5 `just deny`
- [ ] 9.6 `mdbook build docs/`
- [ ] 9.7 `openspec validate layer-isolation --strict`

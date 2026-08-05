# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.13.0] - 2026-08-05

### Features

- *(broker)* Default absent no-contract list fields to empty vectors
- *(cli)* Give doctor a live session-lifecycle smoke arm
- *(cli)* Add git paw doctor read-only preflight diagnostics
- *(cli)* Hide selftest from the public command surface
- *(skills)* Add spec-organization dev skill for the Gate-3 spec audit
- *(detect,config)* Replace gemini cli with antigravity (agy)
- *(skills)* Add doc-completeness, security-review, safety-review, definition-of-done skills
- *(skills)* Add export-agnosticism agent skill across all export surfaces
- *(skills)* Consult the project's .agents/skills standards at author and review time
- *(skills)* Add code-standards agent skill (patterns, NFRs, do-not-touch)
- *(skills)* Add test-strategy agent skill and conformance guard

### Bug Fixes

- *(broker)* Offload blocking publish work and recover a poisoned state lock
- *(dashboard,cli)* Shell-quote the binary path in the __dashboard command
- *(tmux,logging)* Shell-quote the pipe-pane log path
- *(tmux,session)* Sanitize the tmux session name at construction
- *(logging)* Attach pipe-pane on every launch path
- *(specs)* Reconcile the auto-approval live-prompt window contradiction
- *(specs)* Weave the agy-roster correction into merged cli-resolution

### Refactor

- *(tmux)* Route the runtime path through the CommandRunner seam
- *(cli)* Move supervisor mode into commands::supervisor (repoint source_audit)
- *(cli)* Extract the recover/start/add/remove handlers into src/commands
- *(cli)* Split main.rs handlers into src/commands modules
- *(git,tmux)* Add domain newtypes as the construction seam
- *(tmux)* Introduce CommandRunner seam at the builder execute site
- *(tmux)* Split tmux.rs into a tmux/ module tree (R1, behavior-preserving)
- *(config)* Split config.rs into a config/ module tree (R1, behavior-preserving)
- Code-analysis-refactor R1 idiom/hygiene (behavior-preserving)
- *(skills)* Merge security-review + safety-review into one security-and-safety-review skill

### Documentation

- *(user-guide)* Document git paw doctor and fold selftest under it
- *(specs)* Doctor checks for a python3 interpreter
- *(skills)* Capture the net-first consolidation doctrine in test-strategy
- *(skills)* Fold the namespacing doctrine into the spec-organization skill
- *(specs)* Spec-consolidation wave 6 — domain-grouped Specifications index
- *(specs)* Omit the deprecated --from-specs alias from the cli-interaction-e2e matrix

### Testing

- *(broker)* Make the H1 non-stall assertion deterministic on constrained CI
- *(broker)* Assert populated artifact payloads round-trip byte-equivalently
- *(cli)* Cover the doctor exit-code, --json and read-only contracts
- *(tmux)* Reap the isolated tmux server on TmuxTestEnv drop
- *(interactive)* Cover init migrate-supervisor prompt gating
- *(worktree)* Parse list-panes with a space delimiter
- *(cli)* Delete source-grep cli_specs_tty_proceeds_to_picker (behaviorally replaced)
- *(cli)* Add bare --specs spec-picker shown-gate (PTY)
- *(cli)* Add spec-launch dispatch rows to the cli-interaction-e2e matrix
- *(broker)* Trim raw-TCP per-code dups + table-ify delivery routing
- *(config)* Fold absent-section backward-compat fixtures into one table
- *(session)* Delete redundant session_integration.rs (unit tests fully cover it)
- *(config)* Table-ify default/section-absent/enum-parse clusters (safe wave)
- *(cli,supervisor)* Table-ify cli arg-parse + classifier clusters (safe wave)
- *(broker)* Table-ify messages.rs getter/slugify/payload clusters (safe wave)
- Consolidate error/layout/dashboard unit clusters into tables (safe wave)
- *(session)* Add session-logging capture-on-launch guard (net-first)
- *(cli)* Add CLI-resolution-chain guard (net-first)
- *(approval)* Add protected-path / out-of-worktree enforcement guard (net-first)
- *(specs)* Add superpowers backend integration guard (net-first)
- *(cli)* Fix clippy lints in prompt-matrix helpers
- *(interactive)* Add destructive-confirmation gating rows (stop, purge --force)
- *(interactive)* Add start-family PTY shown picker rows
- *(interactive)* Add start-family bypass rows to the prompt matrix
- *(interactive)* Promote shared PTY harness and add the init prompt-matrix slice
- *(skills)* Explicitly guard the standards-consult wording in the lang-agnostic audit

### CI/CD

- *(smoke)* Install python3 in the container smoke image
- *(container)* Run smoke-container as non-root + --init to mirror the CI runner

### Miscellaneous

- *(skills)* Rustfmt tests/standards_skill_consult.rs
## [0.12.0] - 2026-07-24

### Features

- *(specs)* Select the spec system from config/CLI only; remove filesystem auto-detection
- *(specs)* Add superpowers as a fourth spec backend
- *(supervisor)* Tier approval so the drive loop and supervisor act on disjoint sets

### Bug Fixes

- *(broker)* Monotonic poll cursor and duplicate-question suppression
- *(broker)* Accept any branch prefix in agent_id, not just feat ([#58](https://github.com/bearicorn/git-paw/pull/58))

### Documentation

- *(agents)* Enum-variant-ripple checklist (BrokerMessage + SpecBackendKind) and gate-exit-code note
- *(broker)* Update validate() rustdoc to the generalized agent_id regex

### CI/CD

- Scope caches to main and split coverage from the test matrix
## [0.11.0] - 2026-07-17

### Features

- *(supervisor)* Isolate operator memory via config-driven protected-path denial
- *(supervisor)* Supervisor-native approval mode decoupled from agent approval
- *(supervisor)* Seed per-worktree claude settings allowlists at attach and recovery
- *(supervisor)* Harden the approval send gate
- *(supervisor)* Source auto-approve whitelist from stack presets, add worktree dev-test rules
- *(broker,dashboard)* Add agent.answer message variant with targeted delivery (W15-11)
- *(broker)* Detect same-region edits via normalized cross-kind matching (W15-22a)

### Documentation

- *(user-guide)* Close doc-audit low-severity gaps in config, pause, and specs index
- *(skills)* De-opinionate exported verify/archive and spec-layout prose

### Testing

- *(e2e)* Use cat stand-in in add-remove e2e to avoid bare-shell boot-block dirt
- *(e2e)* Force remove_middle_agent past the bare-shell boot-block artifact

### CI/CD

- *(docs)* Split docs job into build-docs and deploy-docs
## [0.10.0] - 2026-07-13

### Features

- *(cli,init)* Provision bundled helpers into agent worktrees
- *(skills)* Bundle the on-demand docs-fetch skill and helper, gated on docs_base_url
- *(docs)* Generate agent-friendly discovery surface at build time
- *(init)* Document every config section in the generated init template

### Bug Fixes

- *(dashboard)* Exit on tty hang-up instead of trapping the input poll
- *(dashboard)* Harden orphan-exit against bind-failure and shell-reparent busy-loops
- *(dashboard)* Exit when orphaned and stop busy-redrawing while idle
- *(git,agents)* Harden the remove dirty-check against porcelain parse bleed

### Documentation

- *(specs)* Index every capability on the specifications page
- *(specs)* Backfill Purpose sections across archived capability specs
- Align docs with shipped behavior and add design principles

### Testing

- *(e2e)* Make broker-port-release checks TIME_WAIT-immune
- *(supervisor)* Assert approval-log rows track first/last-seen
- *(specs)* Guard against the archived Purpose placeholder

### CI/CD

- *(docs)* Add workflow_dispatch for on-demand pages redeploy
## [0.9.0] - 2026-07-06

### Features

- *(supervisor)* Add git paw start --unattended in-process drive loop
- *(supervisor)* Capture qualitative learnings via sweep.sh learn and the tooling_friction category
- *(supervisor)* Add a broker-mediated approval-send gate with live-prompt re-confirm
- *(broker)* Widen the sweep.sh helper surface for status-publish and by-path grants
- *(supervisor)* Detect stream-timeout, context-bloat, no-progress, and blocked stalls
- *(broker)* Classify in-flight overlaps as additive vs true conflicts
- *(cli)* Add git paw selftest subcommand with an isolated lifecycle harness
- *(supervisor)* Auto-approve classifier for safe permission prompts
- *(interactive)* Add fuzzy-filter multi-select branch and spec pickers

### Bug Fixes

- *(supervisor)* Run the five verification gates against the re-resolved branch tip
- *(skills)* De-opinionate commit-message format in the bundled coordination skill
- *(worktree)* Ignore git-paw-managed files in the remove dirty-check

### Documentation

- *(governance)* Add behavioral guidelines to AGENTS.md
- *(readme)* Refresh logo and banner assets, resize logo to 288px
## [0.8.0] - 2026-06-26

### Features

- *(agents)* Inject managed block into gitignored sidecar, not tracked AGENTS.md
- *(agent)* Bundle broker.sh helper; least-privilege boot allowlist
- *(dashboard)* Enlarge broker-log panel with configurable height
- *(orchestration)* Launch-readiness gate, remove-by-pane-id, equal-width rows
- *(supervisor)* Prefix-grant dev-allowlist + genericise DEV_ALLOWLIST_PRESET
- *(worktree)* Configurable worktree placement (child vs sibling)
- *(skills)* Add stand-by-after-commit + releasable-unit commit discipline
- *(dashboard)* Drop the always-blank Summary column from the agent table

### Bug Fixes

- *(init)* Gitignore .git-paw/session-learnings.md

### Documentation

- *(readme)* Add centered logo to the README header
- Add logo to README header ([#54](https://github.com/bearicorn/git-paw/pull/54))
## [0.7.0] - 2026-06-24

### Features

- *(mcp)* Add source-browsing tools (list_files, read_file, search_code)
- *(mcp)* Add read-only documentation tools (get_readme, list_docs, get_doc)
- *(mcp)* Add read-only MCP server (`git paw mcp`) over stdio
- *(learnings)* Disclose no-telemetry stance and opt-in sharing
- *(broker)* Live watch-target registration for hot-added agents

### Bug Fixes

- *(mcp)* Advertise git-paw server identity + configurable [mcp] name
## [0.6.0] - 2026-06-17

### Features

- *(ci)* Cold-start CI parity with containerised smoke recipes
- *(config,init)* CLI-agnostic boot, config-driven dev-allowlist, repo-local tmp scratch
- *(session,cli)* Git paw add/remove, session bugfixes, launch/recovery robustness
- *(supervisor,skills)* /tell routing, opsx role-gating, verification discipline, lang-agnostic skills
- *(dashboard,tmux)* Broker-log panel, supervisor introspection, pane affordances
- *(broker)* Advanced-main + learning message variants, region-level conflict detection, roster hygiene

### Documentation

- *(governance)* Document PR-based release flow
- V0.6.0 user guide, CLI reference, and configuration updates

### Testing

- Cover v0.6.0 capabilities (broker, dashboard, supervisor, session, init)
## [0.5.0] - 2026-05-25

### Features

- *(cli,main,interactive)* SpecMode dispatcher, pause subcommand, --no-supervisor, --specs picker, --from-specs --supervisor routing
- *(init,skills)* Bundle sweep.sh helper installed by git paw init, idempotent merge against existing configs
- *(supervisor)* Supervisor-as-pane, dev-allowlist seeding, default-config fallback, auto-approve, stall detection, layout helper
- *(tmux,git,agents,session,dirs)* Pause primitives, idempotent worktrees, AGENTS.md boot-block lifecycle, worktree base rebase
- *(dashboard)* Supervisor-as-pane row, prompt-inbox removal, phase-aware status, layout collapse
- *(config)* [governance], [common_dev_allowlist], supervisor gate-command keys, user_config_path override
- *(skills)* Supervisor + coordination skill v0.5.0 doctrine
- *(specs)* Spec Kit backend, backend-tagged SpecEntry, per-backend boot-prompt dispatch
- *(broker)* Agent.intent, learnings aggregator, conflict detector, status payload metadata, agent_id validation
- *(supervisor)* Auto-approve patterns
- *(supervisor)* Mode with merge loop, session summary, recovery, question forwarding
- *(cli,config,init,git)* Supervisor + force flags, supervisor config schema, branch handling
- *(dashboard)* Committed counter, prompt-inbox interactivity, message log panel, layout
- *(broker)* Hook injection, watcher, sticky terminal status, real uptime, verified/feedback messages
- *(skills)* Standardize agent-skill resolution to agentskills.io layout
- *(detect)* Expand auto-detection to cover 10 additional AI CLI tools

### Bug Fixes

- *(tmux,test-isolation)* CI failures from -p vs -l N% split syntax and test-process env leakage
- *(tmux)* Pass -x/-y plus set default-size for headless tmux environments
- *(docs)* List all crates in third-party licenses page

### Documentation

- Align README, mdBook, AGENTS.md, and user-guide with v0.5.0 surface
- *(specs)* V0.5.0 OpenSpec changes, archive plan, and main-spec alignment
- Align README, mdBook, and AGENTS.md with v0.4.0 surface
- *(specs)* V0.4.0 OpenSpec changes, archive plan, and main-spec alignment

### Testing

- Behavioral coverage for v0.5.0 surfaces + tmux/config-integration isolation harness
- Behavioral integration and unit tests for v0.4.0
## [0.3.0] - 2026-04-10

### Features

- *(broker)* Wire broker into session lifecycle and update docs ([#43](https://github.com/bearicorn/git-paw/pull/43))
- Add dashboard, skills, and agent coordination
- *(broker)* Add HTTP broker with message types, delivery, and config

### CI/CD

- *(deps)* Switch dependabot to monthly and ignore cargo-dist actions
- *(deps)* Bump actions/deploy-pages from 4 to 5 (#40) ([#40](https://github.com/bearicorn/git-paw/pull/40))

### Build

- *(deps)* Bump toml from 0.9.12+spec-1.1.0 to 1.1.2+spec-1.1.0 (#41) ([#41](https://github.com/bearicorn/git-paw/pull/41))
## [0.2.0] - 2026-04-08

### Features

- Add v0.2.0 spec-driven launch, init, logging, replay, and AGENTS.md integration ([#42](https://github.com/bearicorn/git-paw/pull/42))
## [0.1.0] - 2026-03-25

### Features

- Add CLI tool for parallel AI coding sessions across git worktrees
[0.13.0]: https://github.com/bearicorn/git-paw/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/bearicorn/git-paw/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/bearicorn/git-paw/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/bearicorn/git-paw/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/bearicorn/git-paw/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/bearicorn/git-paw/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/bearicorn/git-paw/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/bearicorn/git-paw/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/bearicorn/git-paw/compare/v0.3.0...v0.5.0
[0.3.0]: https://github.com/bearicorn/git-paw/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/bearicorn/git-paw/compare/v0.1.0...v0.2.0


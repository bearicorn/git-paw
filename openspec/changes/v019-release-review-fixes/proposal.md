# Proposal

## Why

The whole-release regression review of v0.19.0 and the post-merge audit of `architecture-conformance-fixes` found one upgrade break, two silent or CLI-only messages, and spec, doc and test drift. A v0.18 repository run under v0.19 keeps its old `sweep.sh` while the re-exported orchestrator skill teaches `sweep.sh tell`, which that script rejects. The rest are small, but they ship in the same release, so they land before it is cut.

## What Changes

- **R1** A supervised launch (fresh or a drive loop bound to an existing session) refreshes the bundled root helper scripts under `.git-paw/scripts/` — the same set `git paw init` installs — wherever it re-exports the orchestrator skill. A script is written only when its content differs. The supervisor guide gains an upgrade note.
- **R2** `git paw start --unattended` with `--branches`, `--specs` or `--from-all-specs` on a session that already has agents prints the same one-line "launch selection was not applied" notice the attended path prints.
- **R3** MCP `start_session` warnings and errors no longer tell the client to run `git paw add`: the CLI hint is stripped from them, as the `--no-rebase` hint already is. The CLI wording of the two `git paw add` hints becomes one shared suffix ("add agents with …").
- **A1** `code-architecture` "Core modules report warnings as values" is narrowed to what the code does: the agent-CLI module and launch resolution. The config loader keeps its `eprintln!` dedupe (roadmap).
- **A2/A3** The configuration reference's `prompt_profile` row and the CLI prompt-profiles guide name `menu_cursor_glyphs` / `menu_footer_markers`, their defaults, and that an empty list fails the load.
- **A4** The prompt-profile field enumeration names the menu fields.
- **A5** The `core-project-conventions` Purpose says "Highlights" instead of "Features" (scenario titles unchanged).
- **A7** `tests/readme_features.rs` comments cite the current requirement title.
- **A9** `supervisor-config` gains a scenario and a dedicated test for "no worker CLI configured → no cost-cap warning".
- **A10** "Drive-loop keystrokes are unchanged" states the expected key operations instead of "same as before this refactor".
- **A11** The dispatch audit also flags `.contains(` / `contains(&`, with positive controls; the shared source filter blanks lines inside `/* */` block comments.
- **A12** Two stacked comment blocks in `src/commands/start.rs` merge into one.
- **H1** (found by R1's regression run) Every broker HTTP call in the bundled helpers gets a 2s connect / 10s total bound, and a loopback `GIT_PAW_BROKER_URL` overrides the configured broker. The drive loop kills a `sweep.sh learn` that outlives its bound and passes the session's recorded broker to it. Before, a broker port that accepted but never answered hung the unattended loop forever.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `supervisor-launch`: a supervised launch refreshes the bundled root helper scripts.
- `tmux-orchestration`: an unattended re-start reports an ignored launch selection.
- `mcp-server`: `start_session` messages name no CLI command.
- `code-architecture`: warnings-as-values scope (A1), dispatcher keystroke scenario (A10), source filter skips block comments (A11).
- `cli-prompt-profiles`: the field enumeration names the menu fields (A4).
- `cli-provider`: the CLI-name audit catches membership tests (A11).
- `supervisor-config`: no worker CLI configured raises no cost-cap warning (A9).
- `broker-agent-helper`: bundled helpers bound every broker HTTP call; `GIT_PAW_BROKER_URL` override (H1).
- `supervisor-unattended-operation`: recording a learning never holds the drive loop (H1).
- `core-ci-hygiene`: script-running tests clear the session broker address (H1).

## Impact

- Code: `assets/scripts/{_paw_common,broker,sweep}.sh` (bounded curl, URL override), `src/supervisor/drive.rs` (`SweepLearningSink` bound and broker URL), `src/init.rs` (new `refresh_root_scripts`, used by `run_init`), `src/commands/supervisor.rs` (`write_orchestrator_context`, the unattended existing-session branch), `src/commands/start.rs` (shared `ADD_AGENTS_CLI_HINT`, comment merge), `src/mcp/tools/session.rs` (hint stripping for warnings and the error).
- Tests: `tests/e2e_supervisor_launch.rs`, `tests/unattended_drive_e2e.rs`, `src/init.rs` unit tests, `src/mcp/tools/session.rs` unit tests, `src/doctor_tests.rs`, `tests/source_audit.rs`, `tests/helpers/source_scan.rs` consumers, `tests/readme_features.rs` (comments only).
- Docs: `docs/src/user-guide/supervisor.md` (upgrade note), `docs/src/user-guide/session-lifecycle.md` (notice wording), `docs/src/configuration/README.md`, `docs/src/user-guide/cli-prompt-profiles.md`.
- Specs: `openspec/specs/core-project-conventions/spec.md` Purpose is edited directly (a delta cannot change a Purpose).
- No enum variant or struct field is added; no new dependency; no config or wire-format change.

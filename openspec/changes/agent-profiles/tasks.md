## 1. Config seam (no behaviour change without profiles)

- [ ] 1.1 New `src/config/profiles.rs`: `ProfileName` (validated newtype), `ProfileKind { Worker, Delegate }`, `AgentProfile` (`kind`, `description`, `cli`, `args`, `skill`, `approval`, `timeout_seconds`), effective-approval / effective-timeout accessors; `PawConfig.profiles` and `PawConfig.default_profile` with `#[serde(default, skip_serializing_if = …)]`; `merged_with` merges `profiles` by name and `default_profile` like `default_cli`; doc comments on every public item
- [ ] 1.2 Load-time validation with errors naming `profiles.<name>` and the key; unit table tests for "A worker and a delegate profile load", "Missing or unknown kind is rejected", "A multi-line description is rejected", "A full-auto delegate is rejected", "A delegate without a role skill is rejected", "The orchestrator skill is not a profile role", "Timeout rules", "An invalid profile name is rejected", "The repository entry replaces the global entry"
- [ ] 1.3 Round-trip test: a config without profiles serializes byte-identically; existing config tests green unchanged

## 2. Worker choice through every selection path

- [ ] 2.1 `WorkerChoice::{Cli, Profile}`; `SelectionResult.mappings` carries it; `run_selection`, `resolve_cli_for_specs` and the `Prompter` CLI pickers take the profile list and the effective default (`default_profile`, else `default_cli`) on top of v0.17.0 `status-and-picker-truth`'s default parameter; picker items append worker profiles as `<name> — <description> [<cli>]`
- [ ] 2.2 Tests (fake prompter): "Profiles appear with their descriptions", "Picking a profile maps the branch to it", "A profile sharing a CLI's name stays distinct", "default_profile is the starting entry", "No profiles leaves the picker unchanged"
- [ ] 2.3 `SpecEntry.profile` from `paw_profile` (OpenSpec, Markdown backends; Spec Kit / Superpowers set `None`); spec chain levels per the MODIFIED requirement; tests "--profile flag overrides everything", "paw_profile in spec overrides config", "A spec with both paw_profile and paw_cli is rejected", "paw_profile naming an unknown or delegate profile is rejected", "default_profile pre-selects in picker ahead of default_cli"; the existing chain scenarios stay green unchanged
- [ ] 2.4 `src/cli.rs`: `--profile` on `start` / `add` (conflicts with `--cli`, and `--preset` on start), help + `long_about` examples; parse tests "start parses --profile", "add parses --profile", "--profile conflicts with --cli and --preset", "--profile and --cli conflict"
- [ ] 2.5 One pure resolver for non-spec paths and `add` (flag > preset > picker / fallback with `default_profile` before `default_cli`; `add`: flag > spec > first entry's profile or CLI > `default_profile` > `default_cli`) used by start, supervised start, `add`; tests "--profile maps every branch", "add inherits the session's profile", "default_profile naming a delegate fails"

## 3. Launch shaping and session state

- [ ] 3.1 Profile `args` appended after the CLI's `args` in v0.19.0's launch builder; profile approval overrides the worker default for that worker only; errors for an unavailable CLI / unresolvable role skill / delegate-as-worker; tests "Profile arguments follow the CLI's arguments", "Profile approval applies only to its worker", "An unavailable profile CLI fails clearly" (e2e, no worktree created), "A delegate profile cannot launch a worker"
- [ ] 3.2 Role skill: resolve through `skills::resolve(name, repo_root)`, render with the coordination inputs, write after the coordination skill in the managed block (`agents::WorktreeAssignment` gains the rendered role section; `skill-package-deploy`'s skills index lists it); e2e "The role skill is injected after the coordination skill"; unit "Without a role skill the block is unchanged"
- [ ] 3.3 `WorktreeEntry.profile` (serde default / skip); status JSON `profile`; tests "The profile round-trips", "Older session files load with None", "Status JSON carries the profile"
- [ ] 3.4 Recovery resolves the recorded profile from the current config, falling back to the recorded CLI with one warning; tests "A recovered worker keeps its profile's arguments", "A removed profile falls back with a warning"
- [ ] 3.5 E2E "No profiles leaves behavior unchanged": start + recover + status on a profile-free config, pane commands / blocks / session file / human status byte-identical to the pre-change baseline

## 4. Shared headless runner (behaviour-preserving refactor)

- [ ] 4.1 Extract from v0.18.0's `src/supervisor/escalation.rs` a pure `HeadlessInvocation` builder (argv, env add/remove, cwd, stdin, timeout) over the `CliProfile`, and a shell runner on `CommandRunner` (new process group, stdin, stdout capture or discard, stderr to log, process-group kill on timeout); escalation hand-off tests green unchanged

## 5. `git paw delegate`

- [ ] 5.1 `src/delegate.rs` (pure): document assembly from `assets/delegate-framing.md` + rendered role skill + task; output truncation (64 KiB + truncation line); ledger line; `admit(active, level)`; unit tests "The document carries framing, role skill and task in order", "Oversized output is truncated" (truncation half), "A fifth read-only delegate is refused", "A writing delegate runs alone", "A dead run does not block admission"
- [ ] 5.2 `src/cli.rs`: `delegate [NAME] [--task] [--list]` with about / long_about / help; parse tests "delegate parses a name and a task", "delegate --list needs no name", "delegate without a name or --list is rejected", "delegate appears in help"
- [ ] 5.3 `src/commands/delegate.rs` (shell): refusals (linked worktree, `GIT_PAW_DELEGATE` set, unknown / worker profile, unavailable CLI, no headless form), argv order, cwd = main working tree, env (`GIT_PAW_DELEGATE`, no `GIT_PAW_BROKER_URL`, no credential), session-id assignment, active-run records under `.git-paw/logs/delegates/active/`, per-run log + `runs.jsonl`, exit status; fake-runner tests "A delegate runs at the root with its arguments in order", "Environment marks the run and carries no broker URL or credential", "A CLI without a headless form is refused", "A worker profile is not a delegate", "An empty task is refused"
- [ ] 5.4 E2E with a stub CLI script in a temp repo: "Refused from a linked worktree", "No nested delegates", "The task may arrive on stdin", "A successful run prints its result", "Oversized output is truncated" (log holds full output), "A failing delegate exits non-zero", "Timeout kills the process group" (child included), "Listing shows delegates only"
- [ ] 5.5 Add `assets/delegate-framing.md` to the no-language-leak audit set; test "The framing is an audited asset"

## 6. Authority and MCP

- [ ] 6.1 `git paw __authority` refuses under `GIT_PAW_DELEGATE`; tests "No credential inside a delegate run", "A delegate cannot publish a verdict through the helper" (`sweep.sh verified` against a live test broker)
- [ ] 6.2 Bundled `broker.sh` and `sweep.sh`: refuse every subcommand when `GIT_PAW_DELEGATE` is set (non-zero exit, message naming the delegate run, no broker contact, no keystroke); keep both scripts apostrophe-free and `bash -n` clean, sync the tracked copies; tests "broker.sh publishes nothing inside a delegate run", "sweep.sh publishes nothing inside a delegate run" and "The helpers are unaffected outside a delegate run" against a live test broker
- [ ] 6.3 `RepoContext`: a server whose environment carries `GIT_PAW_DELEGATE` is non-root; tests "A delegate's MCP server has no root tools", "A by-name call is refused"
- [ ] 6.4 No MCP delegate tool (design D10): the existing "No agent CLI process is spawned by any tool" test stays unchanged and covers every tool

## 7. Bundled delegate role packages and the orchestrator skill

- [ ] 7.1 `assets/agent-skills/{explorer,spec-auditor,doc-auditor,security-auditor}/SKILL.md`; registry entries; auditor inclusion of the orchestrator package's `references/gates/*.md` resolved at render; tests "Auditors render the orchestrator's gate references", "Each gate procedure has one source", "Auditors define the verdict and never publish", "Explorer returns a map", "Delegate role packages never reach a worktree", "The role packages are vendor-neutral" (registry-driven audit)
- [ ] 7.2 Orchestrator `SKILL.md` delegation section: delegate profiles first, gate → auditor role mapping, explorer, four-reader limit, orchestrator-only publishing, fallbacks; tests "Delegate profiles come first", "Gates map to auditor roles", "Publishing stays with the orchestrator", "The section stays vendor-neutral"

## 8. Usage attribution

- [ ] 8.1 `git paw status --json` `usage_by_profile` (workers by recorded profile; delegate ledger runs since session creation via their CLI session ids and v0.19.0's usage facet); tests "Worker and delegate usage are grouped by profile", "Runs before the session are excluded", "No profiles yields an empty object"
- [ ] 8.2 Drive-loop exit summary per-profile lines; tests "Profiles appear in the usage section", "No profile leaves the usage section unchanged"

## 9. Init template

- [ ] 9.1 `config::generate_default_config`: commented `default_profile`, one worker profile, one delegate profile per bundled role, placeholder `cli` / `args`; tests "The template shows profile examples", "The examples are inert and agnostic"

## 10. Docs

- [ ] 10.1 New `docs/src/user-guide/agent-profiles.md` (worker and delegate profiles, selection order, delegate bounds and privileges with the D7 limits, bundled role skills, usage attribution) + `SUMMARY.md`
- [ ] 10.2 `docs/src/configuration/README.md`: `[profiles.<name>]` keys and `default_profile`; test "The configuration reference documents every profile key"
- [ ] 10.3 `docs/src/cli-reference.md`: `--profile` on start / add, `git paw delegate`; `README.md` CLI table and a profiles bullet
- [ ] 10.4 Orchestrator workflow guide (v0.18.0 `quick-start-orchestrator.md` / `user-guide/supervisor.md`) and `user-guide/tiered-model-workflow.md`: worker profiles per tier replace per-model `[clis.*]` shims; delegating gates and exploration; `user-guide/spec-driven-launch.md` (`paw_profile`); `user-guide/mcp.md` (a delegate's MCP server is never root; delegation goes through the command); `user-guide/security-posture.md` (what a delegate can do); `user-guide/skill-templates.md` (role skills, bundled delegate roles)

## 11. Gates

- [ ] 11.1 Every scenario in this change maps to a named test
- [ ] 11.2 Security review with `security-and-safety-review` (process spawning, root-level execution, env marker limits) and export-agnosticism review of the four packages, the framing asset and the init examples
- [ ] 11.3 `just check` (exit code read directly, not through a pipe), `just deny`, `mdbook build docs/` green; full suite vs merge-base green
- [ ] 11.4 At archive, confirm `openspec/specs/cli-agent-profiles/spec.md` carries its `## Purpose` and the Specifications page lists the capability

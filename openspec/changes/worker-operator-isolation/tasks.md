## 1. Fact gate (before any code depends on it)

- [ ] 1.1 Against a real Claude Code (record its version), launch `claude --setting-sources project,local` with `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` in a scratch git repo with a linked worktree, and confirm: a hook in user settings does not fire; a plugin enabled only in user settings does not load; an allow rule in the main checkout's `.claude/settings.local.json` still applies; a project skill and command (for example `/opsx:apply`) and the git-paw MCP boot form still work; nothing is written under the repository's auto-memory directory. Write the results into design.md D5. If a fact fails, cut the `claude` row to the verified parts and amend the spec before coding

## 2. Provider facet and builder

- [ ] 2.1 `src/agent_cli/mod.rs`: `OperatorIsolation { env: &'static [(&'static str, &'static str)], args: &'static str }` and `CliProvider::operator_isolation()` with the empty Null-Object default; the `claude` provider returns `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` and `--setting-sources project,local`; doc comments
- [ ] 2.2 `src/config/cli.rs`: `CustomCli.inherit_operator_config: bool` with `#[serde(default, skip_serializing_if = "std::ops::Not::not")]` and a doc comment; update every `CustomCli {` literal in `src/` and `tests/` (14 files at proposal time, re-grep)
- [ ] 2.3 `CliProfile::launch_command`: unless the entry sets `inherit_operator_config`, prefix the env assignments and place the isolation args after the CLI's `args`, before sandbox and approval flags; empty parts add no separator
- [ ] 2.4 Unit tests in `src/agent_cli/tests.rs`: "The claude family declares its isolation", "A family without isolation facts contributes nothing", "A claude worker is launched isolated" (exact string), "Recovery keeps the isolation", "Opting out restores the previous launch"
- [ ] 2.5 Config test "An existing config round-trips without the key"; existing config tests green
- [ ] 2.6 Update the existing tests that assert an exact claude-family launch line to the new command (deliberate behaviour change; list them in the commit body)

## 3. Every path, and the orchestrator left alone

- [ ] 3.1 Integration test: `git paw start` and `git paw add` with the same claude-family CLI produce identical isolation (scenario "Start and add stay identical"); recovery through `git paw start` on a stopped session carries it
- [ ] 3.2 Test that the drive loop's headless orchestrator one-shot argv contains neither `--setting-sources` nor the memory variable ("The orchestrator one-shot is unchanged")
- [ ] 3.3 The dry-run launch line (`dry_run_launch_line`) shows the isolated command; assert it in its existing test
- [ ] 3.4 Map the `core-memory-isolation` scenarios "A worker does not run the operator's user plugins or hooks", "A worker does not write the shared auto memory" and "Repository settings still apply to an isolated worker" to the launch-command assertions plus the 1.1 fact record (they depend on Claude Code's behaviour, which no in-repo test can run); note the mapping with `Test:` pointers

## 4. Docs

- [ ] 4.1 `docs/src/configuration/README.md`: `inherit_operator_config` row (default, effect, when to set it)
- [ ] 4.2 `docs/src/user-guide/security-posture.md`: what a worker inherits and what it does not (user settings, hooks, plugins, auto memory), the opt-out, and the main-checkout `settings.local.json` note
- [ ] 4.3 `docs/src/supported-clis.md` (or the custom-CLI section): the claude family's isolation arguments; other families none until the v0.23.0 audit
- [ ] 4.4 `mdbook build docs/` succeeds

## 5. Gates

- [ ] 5.1 `just check` and `just deny` pass (exit codes checked, not piped)
- [ ] 5.2 Security and safety review (`security-and-safety-review` skill): no grant added, no settings file read or written, constant values only in the launch line
- [ ] 5.3 `openspec validate worker-operator-isolation --strict` passes

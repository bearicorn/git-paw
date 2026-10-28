## 1. Seam: the record and the providers (behaviour-preserving)

- [ ] 1.1 Add `src/agent_cli/` with `CliFamily` (closed enum + `FromStr` / `as_str`), the `CliProvider` trait whose default methods are the Null Object, and one impl per family carrying only the facts the code encodes today (design D2); `CliFamily::provider()` is the only exhaustive match
- [ ] 1.2 Move the rows of `config::approval_flags`, `built_in_session_id_args`, `built_in_resume_args` and `skills::MCP_CAPABLE_CLIS` into the providers; keep the old functions as thin delegates for now
- [ ] 1.3 Table-driven unit test: for every (known CLI name, level) the provider lookup returns exactly what the old tables returned, and every unknown name returns the old fallback
- [ ] 1.4 Add `CliProfile` and its resolver over (known roster ∪ `[clis.<name>]`), keyed by id; expose a constructor per family that needs no PATH lookup or config (conformance seam for the v0.23.0 suite)
- [ ] 1.5 Derive `detect::KNOWN_CLIS` from the roster entries in the records; keep the resolved `CliInfo.path`; assert the detected set still equals the 17 names in `cli-resolution`
- [ ] 1.6 `just check` green with no behaviour change before touching any launch path

## 2. Config fields

- [ ] 2.1 Add optional `family` and `args` to `CustomCli` with `#[serde(default, skip_serializing_if)]`; round-trip test proves configs without them serialize byte-identically
- [ ] 2.2 Validate `family` at load: unknown value fails naming `family` and the accepted values (test)
- [ ] 2.3 Resolve family: declared → known name's own → `generic`; tests for the declared-variant, undeclared-entry and command-basename scenarios
- [ ] 2.4 Add commented `family` / `args` examples to the `git paw init` default config, multi-CLI, and extend the all-commented default-config test

## 3. Launch as v0.17.0 does; refuse the ambiguous shim (GP-25)

- [ ] 3.1 Keep v0.17.0's `cli-launch-identity` tests green while the launch reads the record (`[clis.foo] command = "<tmp>/bar"` with no `foo` on PATH launches `bar`)
- [ ] 3.2 Build the pane command from `CliProfile` through v0.17.0's `config::launch_command` (configured `command` as one shell word, a built-in CLI by name); test "A built-in CLI still launches by name"
- [ ] 3.3 Implement the ambiguous-shim refusal for non-known keys (canonical path comparison) with an error naming both paths and the fix; tests for the refused case and for the known-name override that must succeed
- [ ] 3.4 Replace every leading-token lookup (`resolve_submit_delay_ms`, `resolve_prompt_profile`, `cli_speaks_mcp`, readiness) with the record so an executable that differs from the id keeps its overrides (test: settle delay and profile still resolve from `[clis.fast]`)

## 4. One launch-command builder

- [ ] 4.1 Implement `CliProfile::launch_command(level, session)` (executable, `args`, approval flags, session-id or resume args; no empty separators) with a table-driven unit test covering each combination
- [ ] 4.2 Route `attach_agent` / the v0.18.0 library launch path, bare and spec-driven start, `git paw add`, recovery (`cli_command_for_recovery`) and `tmux::readiness::relaunch_cli_into_pane` through it; grep for any remaining hand-built pane command and remove or justify it
- [ ] 4.3 Tests: start and add build identical commands; recovery carries approval flags then resume args; `args` appended on every path; two entries sharing one executable keep their own `args`
- [ ] 4.4 Fold v0.18.0 `orchestrator-escalation-handoff`'s `headless_args` built-in rows into the Claude and Codex providers; existing headless tests pass unchanged

## 5. Family-keyed facets and the MCP boot form (GP-29)

- [ ] 5.1 Switch approval, session-id and resume resolution to override-by-id → family row → none; test the declared-variant and undeclared-variant scenarios from `supervisor-config` and `cli-resolution`
- [ ] 5.2 Pick the boot-block form from the record's MCP capability at every `build_boot_block` call site (`attach_agent`, `cmd_start`, `cmd_start_with_specs`): an explicit `[clis.<id>] mcp` wins, else the family default; test a `family = "claude"` variant gets the MCP form, an undeclared one the helper form, `mcp = false` beats `family = "claude"`, and `mcp = true` gives an undeclared entry the MCP form; keep v0.17.0's `cli_speaks_mcp_*` tests green
- [ ] 5.3 Delete the now-unused name-keyed tables and `MCP_CAPABLE_CLIS`; confirm no other list of CLI names remains in `src/`

## 6. Spec traceability

- [ ] 6.1 Map every scenario in `specs/cli-provider`, `specs/cli-resolution`, `specs/supervisor-config`, `specs/session-state`, `specs/core-security-posture` and `specs/boot-block` to a named test
- [ ] 6.2 Update `tests/security_posture_docs.rs` for the reworded wrapper scenario (anchor on `command` and the refusal, not the old PATH-name wording)

## 7. Docs

- [ ] 7.1 `docs/src/configuration/README.md`: `family` and `args` reference; correct the "launches a CLI by its name on PATH" comment
- [ ] 7.2 `docs/src/supported-clis.md`: what a provider family gives a CLI, how to declare a variant, the generic fallback; correct "override the path" (now true) and add a migration note for PATH shims
- [ ] 7.3 `docs/src/user-guide/sandbox.md`: point `command` at the wrapper; explain the ambiguous-shim refusal
- [ ] 7.4 `docs/src/user-guide/tiered-model-workflow.md`: per-worker model via one `[clis.*]` entry per profile with `args`
- [ ] 7.5 `docs/src/architecture.md` module list gains `src/agent_cli/`; README feature bullet for custom CLI variants
- [ ] 7.6 Changelog-facing note: the ambiguous-shim refusal, recovery now carries `args`, `family` supplies the MCP default only when `mcp` is unset, older binaries ignore `family` / `args`

## 8. Quality gates

- [ ] 8.1 `cargo fmt`, then `just lint`
- [ ] 8.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 8.3 `just deny`
- [ ] 8.4 `mdbook build docs/`
- [ ] 8.5 `openspec validate cli-provider-record --strict`

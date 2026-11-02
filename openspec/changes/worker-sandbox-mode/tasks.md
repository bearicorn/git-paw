## 1. Config and provider facets

- [x] 1.1 Add optional `sandbox` (`native` | `wrapper`) and `sandbox_args` to `CustomCli` with serde defaults; round-trip test shows configs without them serialize unchanged
- [x] 1.2 Validate at load: unknown `sandbox` value (error names key and accepted values); `native` with no resolvable arguments (error names `sandbox` and `sandbox_args`); tests for both
- [x] 1.3 Add the native-sandbox facet to `CliProvider` (Codex: `--sandbox workspace-write`; every other family none) and the sandbox-disabling mark on approval rows (Codex `full-auto`); unit table over all families

## 2. Launch builder

- [x] 2.1 Insert native sandbox arguments after `args` and before approval flags; skip them when the resolved approval flags already contain them verbatim; table test covering Codex at manual / auto, an override, and an undeclared CLI (command unchanged)
- [x] 2.2 Refuse the launch for a `native` CLI whose approval flags come from a marked row, with an error naming CLI, level and `[clis.<name>].approval_args`; assert no pane runs the CLI; test that an explicit override is used as given
- [x] 2.3 Test that a declared sandbox never changes the approval level (`auto` stays `auto`) and that a `wrapper` declaration leaves the command unchanged at every level

## 3. Doctor

- [x] 3.1 Add the sandbox-mode check beside `check_sandbox`: list each worker-resolvable CLI with `native` / `wrapper` / `none`, labelled declared; ⚠ for effective `full-auto` with no declaration; never ✗; read-only
- [x] 3.2 Extend `tests/doctor_sandbox_check.rs` with the declared-wrapper, unsandboxed-full-auto and sandboxed-full-auto scenarios, including `--json` output

## 4. Security review (Gate 5)

- [x] 4.1 Run the `security-and-safety-review` checklist against the diff: no new grant, no implicit escalation, `sandbox_args` appended under the same trust model as `approval_args`, the refusal path leaves no pane running
- [x] 4.2 Confirm the residual-risk list in the posture chapter matches what the documented sandbox profile actually leaves open (network, secret reads, shared `.git` refs, credentialed push)

## 5. Spec traceability

- [x] 5.1 Map every scenario in `specs/cli-provider`, `specs/preflight-diagnostics` and `specs/core-security-posture` to a named test (the posture scenarios via `tests/security_posture_docs.rs` anchors)

## 6. Docs

- [x] 6.1 `docs/src/user-guide/security-posture.md`: sandboxed full-auto section — the prompt-free route for an `--unattended` wave (git-paw approves no prompt), the residual risks (no command put to a human), and "declared, not verified"
- [x] 6.2 `docs/src/user-guide/sandbox.md`: `sandbox = "wrapper"`, `sandbox = "native"`, the Codex example, the refusal, and the recipe `agent_approval = "full-auto"` + a declared sandbox
- [x] 6.3 `docs/src/configuration/README.md`: `sandbox` and `sandbox_args` reference; `git paw init` commented examples (multi-CLI, all commented)
- [x] 6.4 `docs/src/user-guide/doctor.md`: the sandbox-mode check
- [x] 6.5 `docs/src/user-guide/tiered-model-workflow.md`: sandboxed full-auto cheap workers

## 7. Quality gates

- [x] 7.1 `cargo fmt`, then `just lint`
- [ ] 7.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [x] 7.3 `just deny`
- [x] 7.4 `mdbook build docs/`
- [ ] 7.5 `openspec validate worker-sandbox-mode --strict`

## 8. Review round 1

- [x] 8.1 Refuse a native sandbox's effective approval flags (built-in or override) that carry a family sandbox-disabling flag, or set the sandbox flag to another value than `sandbox_args`
- [x] 8.2 Apply the native sandbox arguments and the refusal to the headless orchestrator hand-off
- [x] 8.3 Doctor: ⚠ "declared, not verified" for every declared CLI at full-auto, ⚠ for a `wrapper` whose `command` is a known agent CLI binary, ✗ where a launch would be refused
- [x] 8.4 Warn once per CLI at launch (start/add/recover) when a pane runs full-auto with no declared sandbox
- [x] 8.5 Docs: posture lists `sandbox_args` as pane shell text; sandbox, doctor, configuration and README updated

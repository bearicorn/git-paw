## 1. Config surface

- [ ] 1.1 Add `approval: Option<ApprovalLevel>` to `SupervisorConfig` (`src/config.rs`) with `#[serde(default, skip_serializing_if = "Option::is_none")]`; unit tests: absent → `None`, all three kebab values parse, invalid value errors, round-trip
- [ ] 1.2 Add `approval_args: HashMap<String, String>` to `CustomCli` with `#[serde(default, skip_serializing_if = "HashMap::is_empty")]`; validate keys against the three kebab level names at load (error names the bad key); tests: parse, round-trip, pre-v0.11 configs unchanged, invalid key rejected
- [ ] 1.3 Extend the built-in flag table with `gemini`/`qwen` `FullAuto → "--yolo"`; verify the gemini, qwen, and codex flags against upstream docs and amend the spec table if any differ
- [ ] 1.4 Implement config-aware flag resolution (override → built-in table → `""`), replacing direct `approval_flags` calls; deterministic; tests for precedence and the claude-oss override scenario

## 2. Launch wiring

- [ ] 2.1 `cmd_supervisor` start flow (`src/main.rs:1162`, pane build at `1306-1316`): resolve supervisor flags from `approval.unwrap_or(agent_approval)`; agents keep `agent_approval`; test: split levels produce flags on pane 0 only
- [ ] 2.2 Recovery flow (`src/main.rs:2323`): same resolution; test: recovered supervisor pane carries the flag
- [ ] 2.3 Warn-and-degrade: `full-auto` resolving to `""` prints a warning naming the CLI and `[clis.<name>].approval_args`; launch proceeds flagless; test asserts the warning and successful launch
- [ ] 2.4 Dry-run plan prints supervisor vs agent approval levels distinctly when they differ; test on output
- [ ] 2.5 Back-compat guard test: config without `approval` builds byte-identical supervisor + agent commands to the previous resolution

## 3. Init template

- [ ] 3.1 Add a commented `# approval = "manual"  # supervisor pane's own level: "manual" | "auto" | "full-auto"` line to the init `[supervisor]` commented block (`src/config.rs` template) and assert it in the init-template test

## 4. Docs

- [ ] 4.1 Configuration reference: `[supervisor] approval` (trusted-pane semantics, inherit default) and `[clis.<name>] approval_args`
- [ ] 4.2 Unattended-operation chapter: recommend `approval = "full-auto"` for fully unattended runs; blast-radius note referencing agent-memory-isolation
- [ ] 4.3 `mdbook build docs/` passes

## 1. Scope the enum ripple up front

- [ ] 1.1 Grep `PermissionType::Cargo` and `PermissionType` across `src/` and `tests/`; list every site before editing (known: `src/supervisor/poll.rs` signatures and fixtures, `src/commands/supervisor.rs:224`).
- [ ] 1.2 Confirm no test fixture depends on the `Cargo` classification as its only assertion.

## 2. Remove the toolchain leak

- [ ] 2.1 Remove the `cargo fmt` / `cargo clippy` / `cargo test` / `cargo build` arms from `classify_command_class` (`src/supervisor/permission_prompt.rs:96-103`).
- [ ] 2.2 Remove the `PermissionType::Cargo` variant and update every site found in 1.1.
- [ ] 2.3 Update the module doc comment (`permission_prompt.rs:35-41`) so it no longer describes a toolchain class.
- [ ] 2.4 Test: the permission-type set contains no toolchain-specific class.
- [ ] 2.5 Test: a build-tool prompt is not specially classified, and the auto-approval decision is unchanged (made by the safe-command classifier).
- [ ] 2.6 Confirm `Curl` and `Unknown` classification and the "`Unknown` never auto-approves" rule are untouched.

## 3. Configurable gate vocabulary

- [ ] 3.1 Add the gate-vocabulary field under `[supervisor.correction]` with `#[serde(default)]` and `skip_serializing_if`.
- [ ] 3.2 Default it to exactly the current `GATE_TAGS` list including `scope` (D2).
- [ ] 3.3 Replace the `GATE_TAGS` constant use in `src/supervisor/drive.rs:550-557` with the resolved config value.
- [ ] 3.4 Decide and pin the empty-list behaviour — reject at load, or fall back to the default. Add a test for whichever is chosen.
- [ ] 3.5 Test: default vocabulary reproduces current behaviour.
- [ ] 3.6 Test: a configured custom gate name is recognised and starts a correction cycle.
- [ ] 3.7 Test: an unrecognised tag is not treated as a gate verdict and does not start a correction cycle (D3) — verify the conflict detector's `[conflict-detector]` tag still does not.
- [ ] 3.8 Test: a config written before this field existed loads unchanged with the default list.

## 4. Docs

- [ ] 4.1 Configuration reference entry for the gate-vocabulary field.
- [ ] 4.2 Supervisor guide's correction-loop section notes the gate names are configurable and that the framework itself is unchanged.
- [ ] 4.3 Drop `Cargo` from any doc enumerating permission types.
- [ ] 4.4 Changelog entry states the removed classification was coarse and superseded, so it does not read as a safety reduction.
- [ ] 4.5 `mdbook build docs/` succeeds.

## 5. Quality gates

- [ ] 5.1 `cargo fmt` before committing.
- [ ] 5.2 `just check` — verify by real exit code, not piped output.
- [ ] 5.3 Confirm the danger list, safe-command whitelist semantics, correction budget and exhaustion policy are untouched.

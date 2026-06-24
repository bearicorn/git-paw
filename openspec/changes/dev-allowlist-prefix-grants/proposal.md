## Why

During the v0.7.0 dogfood, agents stalled on the *same* safe dev-command prompts every cycle. Two root causes: (1) the CLI's "don't ask again for: <cmd>" appears to whitelist by **exact string** for some commands, so an agent's `cargo test … && echo "EXIT $?"` wrapper (text varies each run) re-prompts forever even after approval, while prefix-matchable grants (`cargo check *`, `python3 -c`) generalise fine; (2) the seeded dev-allowlist doesn't cover enough prefixes. The result was constant human babysitting. This is the *lite* slice of the approval problem — better allowlist seeding + a skill nudge — distinct from the full broker-mediated approval architecture (drift F, v0.9.0).

## What Changes

- **Prefer prefix grants in the seeded dev-allowlist.** Seed prefix-matchable allow rules (e.g. `cargo *`, `git *`, `just *`) into the CLI settings so routine dev-loop commands don't re-prompt per variant. Audit `[supervisor.common_dev_allowlist]` seeding to emit prefix forms.
- **Skill nudge against exit-code-probe wrappers.** Add coordination/supervisor skill guidance: don't wrap dev commands in `&& echo "… $?"` / `EXIT=$?` probes — they defeat the CLI's command-string whitelisting and force re-prompts. Run the command bare and read the exit status directly.
- **Genericise `DEV_ALLOWLIST_PRESET` (de-opinionation).** The preset hardcodes git-paw's *own* stack into every consumer project's agent allowlist (`cargo build/test/clippy/fmt/check/tree/deny/update`, `just`, `mdbook build`, `openspec …`). A non-Rust/non-just/non-OpenSpec project gets these useless grants while its own `pytest`/`npm`/`go test` aren't covered. **Fix:** hardcode only universal commands (`git *`, `find`, `grep`, `sed -n`, …); move the stack-specific set to config (`[supervisor.common_dev_allowlist] extra` already exists) and/or named opt-in stack presets (`rust`/`node`/`python`/`go`). git-paw's own repo then declares its cargo/just/openspec set in its config.
- Out of scope (→ v0.9.0): the full classifier, broker-as-trigger, and broker-mediated "chat-with-options" approvals.

## Capabilities

### New Capabilities
<!-- None. -->

### Modified Capabilities
- `dev-command-allowlist`: seed prefix-matchable grants (not exact-string) so dev-loop commands generalise across invocations.
- `skill-standardization` (or `lang-agnostic-skills`): add the "don't wrap dev commands in exit-code echo probes" guidance to the bundled skills.

## Impact

- Affected code: the dev-allowlist seeding (`src/supervisor/dev_allowlist.rs` / `src/config.rs`), bundled skill assets (`assets/agent-skills/*.md`).
- Tests: allowlist-seeding test asserts prefix forms; skill-content test asserts the no-exit-probe guidance is present.
- Docs: configuration reference (`common_dev_allowlist`), skill docs.
- Backward compatible: additive — broader/prefix grants only reduce prompts; no behaviour removed.

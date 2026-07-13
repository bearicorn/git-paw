## Why

Export-audit F5 (HIGH, 2026-07-07): the auto-approve classifier hard-codes git-paw's own stack as always-safe for EVERY consumer — `cargo fmt/clippy/test/build` in `default_safe_commands()` (`src/supervisor/auto_approve.rs:49-82`) and `openspec`/`just` in `READ_MOSTLY_VERBS` (33-36), mirrored in `sweep.sh`'s `EXPLICIT_SAFE`/`READ_MOSTLY` (1297-1301). This is the un-migrated sibling of the v0.8.0 `DEV_ALLOWLIST_PRESET` de-opinionation, and it violates that spec's single-source-of-truth clause ("no other location in the codebase may hard-code preset patterns"). A consumer's auto-approver should reflect THEIR stack, sourced from the same `[supervisor.common_dev_allowlist]` stacks/extra the allowlist seeder already resolves — today no bridge exists between the two systems. Rider (dogfood learning 2026-07-08, 3× friction): worktree-confined dev-test syntax (`bash -n`, `chmod` on own files, `mktemp`, interpreter runs of worktree scripts) Unknown-escalates generically instead of classifying by the worktree-confinement rules the classifier already applies to file ops and git ops.

## What Changes

- **De-opinionate the built-ins**: remove `cargo fmt/clippy/test/build` from the default whitelist and `openspec`/`just` from the read-mostly verbs (Rust + `sweep.sh` mirror in lockstep). What remains built-in is stack-neutral: generic read-mostly verbs, `git commit`, broker-localhost `curl`.
- **Bridge to the stack presets**: the effective whitelist composition additionally folds in `dev_allowlist::effective_patterns(stacks, extra)` — the resolved `[supervisor.common_dev_allowlist]` universal + named stack presets + extras — so a rust-stack project gets its `cargo` verbs from the same declaration that seeds the CLI allowlist. `[supervisor.auto_approve].safe_commands` keeps working as the per-project extension.
- **sweep.sh mirror**: `sweep.sh classify` composes the same lists by reading the resolved stacks from `.git-paw/config.toml`; add a lockstep list-parity guard (today parity is behavioral-only — the audit flagged the gap).
- **Rider — worktree-confined dev-test rules**: `bash -n <worktree-file>`, non-recursive `chmod` on worktree paths, `mktemp`, and interpreter execution of a worktree-resident script (no `-c` code strings) classify safe by the existing worktree-confinement pattern; `chmod -R` stays danger, `-c` runners stay unknown.
- git-paw's OWN dogfood config declares `stacks = ["rust"]` + `openspec`/`just` extras so its behavior is unchanged for itself (conventions move to the consumer side, per the export policy).

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `safe-command-classification`: MODIFY "Whitelist of safe command classes" (de-opinionated composition + stack-preset bridge); ADDED "Worktree-confined dev-test commands classify safe" (rider).
- `approval-configuration`: MODIFY "Configurable safe-command list" (scenario no longer pins a toolchain verb as a default).

## Impact

- `src/supervisor/auto_approve.rs` (constants + composition), `src/config.rs` (`effective_whitelist()` folds `effective_patterns`), `src/supervisor/dev_allowlist.rs` (unchanged; consumed), `assets/scripts/sweep.sh` (mirror + config-driven composition; `bash -n`; apostrophe-free heredocs).
- Tracked-drift note: `.git-paw/scripts/sweep.sh` is provisioned from assets since `8da7391` — no tracked copy to sync.
- Tests: composition (with/without stacks), rider scenarios, sweep.sh behavioral fixtures + NEW list-parity guard, Conservative-preset stripping still applies post-composition.
- `.git-paw/config.toml` (git-paw's own): declare rust stack + extras.
- Docs: configuration reference (auto-approve sourcing), supervisor chapter.

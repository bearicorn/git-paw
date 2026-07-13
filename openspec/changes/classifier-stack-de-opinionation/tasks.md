## 1. De-opinionate + bridge (Rust)

- [ ] 1.1 Remove `openspec`/`just` from `READ_MOSTLY_VERBS` and `cargo fmt/clippy/test/build` from `default_safe_commands()` (`src/supervisor/auto_approve.rs`); keep stack-neutral entries; update `read_mostly_verbs_are_whitelisted`
- [ ] 1.2 Fold `dev_allowlist::effective_patterns(stacks, extra)` into `AutoApproveConfig::effective_whitelist()` (`src/config.rs`) — composition order built-ins → stack → extra → `safe_commands`, dedup, THEN Conservative strip; thread `[supervisor.common_dev_allowlist]` into the whitelist assembly call sites (`drive_unattended_loop`, `poll.rs`)
- [ ] 1.3 Tests: stack-neutral default (no cargo/openspec/just), rust stack contributes cargo, node stack does not, `safe_commands` still extends, Conservative strips post-composition

## 2. Rider — worktree-confined dev-test rules

- [ ] 2.1 Implement `bash -n <worktree-path>`, non-recursive `chmod` on worktree paths, `mktemp`, and interpreter-of-worktree-script (no `-c`, no out-of-worktree path args) as safe-by-pattern rules reusing `is_path_inside_worktree`; one-time-only for interpreter runs (never broad grant)
- [ ] 2.2 Tests per spec scenarios: each rule positive, `chmod -R` danger, `-c` unmatched, out-of-worktree unmatched, supervisor pane (no worktree root) unaffected

## 3. sweep.sh mirror + parity guard

- [ ] 3.1 De-opinionate `EXPLICIT_SAFE`/`READ_MOSTLY` in `assets/scripts/sweep.sh`; compose stack patterns by reading `[supervisor.common_dev_allowlist]` from `.git-paw/config.toml` (fail-safe: built-ins only when unreadable); mirror the rider rules where the classify path sees them; apostrophe-free heredocs; `bash -n`
- [ ] 3.2 Add a list-parity test asserting sweep.sh's verb arrays equal the Rust constants (closes the audit-flagged behavioral-only gap); extend `tests/sweep_sh_classify.rs` fixtures for stack-driven decisions

## 4. Dogfood config + docs

- [ ] 4.1 git-paw's own `.git-paw/config.toml`: `stacks = ["rust"]`, `extra = ["openspec", "just", "mdbook build"]`
- [ ] 4.2 Configuration reference: auto-approve whitelist sourcing (three sources, composition order, Conservative semantics); release-notes migration note
- [ ] 4.3 `mdbook build docs/` passes

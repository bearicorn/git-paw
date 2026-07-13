## Context

Two independent allowlist systems exist: the dev-allowlist seeder (de-opinionated in v0.8.0 — universal preset + named stack presets + extra, `src/supervisor/dev_allowlist.rs`) and the auto-approve classifier whitelist (`AutoApproveConfig::effective_whitelist()` at `src/config.rs:921-935` = `default_safe_commands()` + `safe_commands`). The classifier was never de-opinionated: `cargo*` in `default_safe_commands()`, `openspec`/`just` in `READ_MOSTLY_VERBS`, mirrored at `sweep.sh:1297-1301`. Confirmed by grep: no bridge between `common_dev_allowlist` and the classifier exists today. Parity between Rust and sweep.sh is behavioral-only (`tests/sweep_sh_classify.rs`); no list-equality guard.

## Goals / Non-Goals

**Goals:** consumer classifiers reflect the consumer's declared stack; one source of truth for stack patterns; Rust ↔ sweep.sh lockstep enforced by test; the dogfood-friction dev-test shapes classify by worktree confinement.
**Non-Goals:** changing the dev-allowlist presets themselves; new config surface (reuses `stacks`/`extra`/`safe_commands`); compound-command classification (that's #1's full-auto territory).

## Decisions

- **D1 — Bridge via `effective_patterns`, not a new list.** `effective_whitelist()` folds in `dev_allowlist::effective_patterns(stacks, extra)` from `[supervisor.common_dev_allowlist]`. Rationale: the v0.8.0 split already forces projects to declare their stack there; two declarations for the same fact would drift. The dev-allowlist module's exported constants stay the single source of truth (its spec's clause finally holds codebase-wide).
- **D2 — Composition order and the Conservative preset.** Compose (built-ins → stack patterns → `extra` → `safe_commands`, dedup), THEN apply the Conservative strip (`git push`/`curl` removal) so the preset governs the whole composed set, matching today's semantics.
- **D3 — sweep.sh reads stacks from config.toml.** The helper already reads session/broker config; it composes `READ_MOSTLY`/`EXPLICIT_SAFE` + the stack patterns for the declared stacks. A new list-parity test extracts the helper's arrays and compares them to the Rust constants byte-for-byte (closing the audit-flagged gap). Heredocs stay apostrophe-free; `bash -n` after editing.
- **D4 — Rider rules ride the worktree-confinement pattern.** `bash -n`, non-recursive `chmod`, `mktemp`, and interpreter-of-worktree-script reuse `is_path_inside_worktree` (canonicalized, fail-closed). Interpreter runs are one-time-only (existing arbitrary-code-runner broad-grant restriction) — executing your own worktree script is no more privileged than `cargo test`, but a permanent grant on `python3` would be.
- **D5 — Dogfood parity for git-paw itself.** git-paw's own `.git-paw/config.toml` declares `stacks = ["rust"]` and `extra = ["openspec", "just", "mdbook build"]`, so its own sessions behave exactly as before — the conventions move from the binary to the consumer config, which is the whole point.

## Risks / Trade-offs

- **Behavior change for consumers who relied on the baked cargo verbs without declaring a rust stack**: their `cargo test` prompts stop auto-approving. Called out in the changelog/docs; the fix is a one-line `stacks = ["rust"]`. Strictly a de-escalation (safer default), consistent with the v0.8.0 precedent.
- **Whitelist growth**: folding the universal dev preset into the classifier adds git-verb prefixes that overlap the read-mostly `git` verb — dedup keeps the list sane; danger-first ordering is unchanged.
- sweep.sh config parsing adds a failure mode (missing/odd config) — helper falls back to built-ins-only composition (fail-safe: fewer auto-approvals, never more).

## Migration Plan

Release-notes entry: projects using auto-approve with a Rust/Node/Python/Go toolchain declare it in `[supervisor.common_dev_allowlist] stacks` (most already have, for the seeder); anything bespoke goes in `safe_commands` as before. git-paw's own config updated in this change.

## Open Questions

None.

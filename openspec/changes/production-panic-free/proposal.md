## Why

AGENTS.md mandates "No `unwrap()` or `expect()` in non-test code", but nothing enforces it:
`clippy::unwrap_used` / `clippy::expect_used` are not enabled, so `just lint` and CI accept
violations. A clippy run with both lints (test code exempted) finds **20** `expect()` calls in
non-test code — 14 static-regex compiles, 4 code-invariant asserts, 2 clap-guarantee asserts — and
the convention has no spec, so it cannot be audited at the review gate.

A second gap in the same gates (GP-52, found while authoring this change): the `[[bin]]` target
in `Cargo.toml` has carried `test = false` since the first commit, so the 60+ `#[test]` functions
in `src/main.rs` and `src/commands/*.rs` are never compiled, run, or linted by `cargo test`,
`just check`, or CI. They pass today only when run explicitly (`cargo test --bin git-paw`).

## What Changes

- Remove all 20 non-test `expect()` calls:
  - **Static regexes (14)** — `agents.rs` (2), `broker/server.rs` (2), `opsx/role_guard.rs` (3),
    `specs/speckit.rs` (3), `specs/superpowers.rs` (1), `supervisor/auto_approve.rs` (2),
    `supervisor/drive.rs` (1) — compile through ONE crate-private helper taking `&'static str`,
    the only non-test code allowed to `expect` (narrowly scoped `#[allow]` + comment); every static
    pattern is compiled by a unit test.
  - **Code invariants (4)** — `specs/speckit.rs` (2), `coordination/inventory.rs` (1),
    `mcp/query/learnings.rs` (1) — restructured so the impossible `None` is unrepresentable (the
    helper returns the index; `Option::insert` returns the reference; `if let` on the map entry),
    rather than inventing unreachable error paths.
  - **clap guarantees (2)** — `commands/add.rs`, `commands/replay.rs` — `ok_or_else` into
    `PawError::BranchError` / `PawError::ReplayError`.
- Enforce the rule: `#![warn(clippy::unwrap_used, clippy::expect_used)]` at the crate roots
  `src/lib.rs` and `src/main.rs`, plus a new `clippy.toml` with `allow-unwrap-in-tests = true` and
  `allow-expect-in-tests = true`, so `just lint` (`-D warnings`) fails on a violation in library or
  binary code while unit tests stay exempt.
- Specify the convention as a requirement so Gate 3/5 can audit it.
- Remove `test = false` from the `[[bin]]` target so binary-crate unit tests run under plain
  `cargo test` and are linted by `cargo clippy --all-targets` (GP-52).

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `core-project-conventions`: ADDED *Non-test code does not call unwrap or expect* and
  *Static regular expressions compile through one audited helper*.
- `core-ci-hygiene`: ADDED *Unit tests in the binary crate run under the default test and lint
  gates* (GP-52).

## Impact

- `Cargo.toml` — `test = false` removed from `[[bin]]` (GP-52); `[lints.clippy]` deliberately
  NOT used for the two panic lints (see design D1).
- New `clippy.toml` (repo root).
- `src/lib.rs`, `src/main.rs` — crate-level `#![warn(...)]`; `src/lib.rs` declares the new
  private `static_regex` module.
- New `src/static_regex.rs` — `pub(crate) fn static_regex(pattern: &'static str) -> Regex`.
- `src/agents.rs` (`SUPERVISOR_PID_REGEX`, `LAST_VERIFIED_COMMIT_REGEX`), `src/broker/server.rs`
  (agent-id and placeholder regex accessors), `src/opsx/role_guard.rs` (archive-message,
  `archive_move_re`, `spec_addition_re`), `src/specs/speckit.rs` (phase-heading, incomplete-task,
  complete-task regexes; `ensure_implicit_phase`), `src/specs/superpowers.rs` (task-heading regex),
  `src/supervisor/auto_approve.rs` (`file_prompt_regex`, option-line regex),
  `src/supervisor/drive.rs` (paste-buffer regex), `src/coordination/inventory.rs`
  (`get_or_refresh`), `src/mcp/query/learnings.rs` (`parse`), `src/commands/add.rs` (`cmd_add`),
  `src/commands/replay.rs` (`cmd_replay`).
- `docs/src/architecture.md` (new module); `CONTRIBUTING.md` (~:79) and `docs/src/contributing.md`
  (~:72) — the rule is now lint-enforced, with the static-regex helper as the sole exception;
  `AGENTS.md` Code Style bullet gains the same pointer (repo-local, not exported).
- No behaviour change, no CLI/config/wire change, no new dependency.
- Integration-test crates under `tests/` are outside the rule (585 helper-function call sites in 96
  files would otherwise fire — design D1).
- **Collisions**: none — no pending change touches `core-project-conventions`; the edited source
  files overlap pending changes only textually (`start-spec-launch-parity` / `cli-launch-identity`
  touch neither `add.rs:88` nor `replay.rs`); rebase-level only.

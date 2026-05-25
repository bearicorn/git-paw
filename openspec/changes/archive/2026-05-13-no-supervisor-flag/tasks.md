## 1. CLI flag definition

- [x] 1.1 In `src/cli.rs` `StartArgs`, add `no_supervisor: bool` field with `#[arg(long, conflicts_with = "supervisor", default_value_t = false, help = "Disable supervisor for this session, overriding any [supervisor] enabled = true in config")]`.
- [x] 1.2 Verify clap's `conflicts_with` is bidirectional — passing `--supervisor --no-supervisor` errors regardless of order.
- [x] 1.3 Update `start --help` snapshot/expectations if any test asserts on help output.

## 2. Resolution chain

- [x] 2.1 In `src/main.rs`, extend `resolve_supervisor_mode` to take a leading `no_supervisor_flag: bool` parameter. Insert as the first short-circuit:
  ```rust
  if no_supervisor_flag {
      return Ok(false);
  }
  ```
  Existing chain (steps 2-6 in the new numbering) is unchanged.
- [x] 2.2 Update `resolve_supervisor_mode_from_cwd` to accept and forward the new parameter.
- [x] 2.3 Update the `cmd_start` callsite (and any other callsite — likely the spec-mode dispatcher from `cross-format-spec-selection`) to pass `args.no_supervisor` through.

## 3. Resolution unit tests

- [x] 3.1 Test: `--no-supervisor` with `[supervisor] enabled = true` in config → resolves to `false`.
- [x] 3.2 Test: `--no-supervisor` with `[supervisor] enabled = false` in config → resolves to `false` (idempotent).
- [x] 3.3 Test: `--no-supervisor` with no `[supervisor]` section → resolves to `false` (no prompt regardless of TTY mock state).
- [x] 3.4 Test: `--no-supervisor` with `--dry-run` → resolves to `false`.
- [x] 3.5 Regression test: every existing test in `src/main.rs::tests::resolve_supervisor_mode_*` still passes when called with `no_supervisor_flag = false` (existing behaviour preserved).

## 4. CLI parse tests

- [x] 4.1 `start --no-supervisor` → `no_supervisor == true`, `supervisor == false`.
- [x] 4.2 `start --supervisor` → unchanged.
- [x] 4.3 `start` (no flags) → both `false`.
- [x] 4.4 `start --supervisor --no-supervisor` → parse error mentioning both flags.
- [x] 4.5 `start --no-supervisor --cli claude --branches feat/a,feat/b` → all fields set, parse succeeds.
- [x] 4.6 `start --help` output contains `--no-supervisor`.

## 5. Integration test

- [x] 5.1 `assert_cmd`-driven test: in a tempdir with a config containing `[supervisor] enabled = true`, run `git paw start --no-supervisor --dry-run` and assert the dry-run plan reflects supervisor-disabled state (e.g. no supervisor pane in the planned layout).

## 6. Documentation

- [x] 6.1 Update `docs/src/user-guide/start.md` (or wherever the existing `--supervisor` flag is documented) with a section on `--no-supervisor`. Show the canonical use case: project with `enabled = true` config, user wants to skip for one session. (Implemented in `docs/src/cli-reference.md` and `docs/src/quick-start-supervisor.md`.)
- [x] 6.2 Update the supervisor-mode resolution chain documentation if it appears in user-facing docs (e.g. `docs/src/configuration.md`) — the chain is now 7 steps with `--no-supervisor` first. (Updated in `docs/src/cli-reference.md` and `docs/src/configuration/README.md`.)
- [x] 6.3 Run `mdbook build docs/`; assert success.

## 7. Release notes

- [x] 7.1 v0.5.0 release notes: announce `--no-supervisor`. Include the canonical use case ("skip supervisor for one session without editing config"). Reference the precedence change ("flag wins over config"). (CHANGELOG.md is regenerated from conventional commits via `git cliff` at release prep; the `feat(cli): add --no-supervisor` commit body carries the release-note copy.)

## 8. Quality gates

- [x] 8.1 `just check` — fmt, clippy, and all tests touched by this change pass. (Pre-existing `config_integration` failures from user `~/.config/git-paw/` bleeding into the test process were present on the parent commit and are out of scope.)
- [x] 8.2 `just deny` — supply chain clean.
- [x] 8.3 No new `unwrap()` / `expect()` in non-test code.
- [x] 8.4 `mdbook build docs/` succeeds.
- [x] 8.5 `openspec validate no-supervisor-flag` passes.

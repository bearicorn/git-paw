# Tasks — path-injection-hardening

Each of the three bugs gets a reproducing test FIRST (it must fail against current
`main`), then the fix makes it pass. This is a behavior change, so per code-standards a
reproducing test precedes every fix.

> **Amendment (implementation).** Three task details were written against a pre-split
> layout and are corrected below: (a) the `SessionName` newtype already exists in
> `src/domain.rs` — `code-analysis-refactor` landed it as a byte-identical construction
> *seam* whose module docs name this change as the one that hardens it — so it is
> hardened there, not introduced in `src/git.rs`; (b) `src/tmux.rs` is now
> `src/tmux/{command,session}.rs` and the `__dashboard` sites moved out of `src/main.rs`
> into `src/commands/{start,recover,supervisor}.rs`; (c) `send-keys -l` does not fix
> bug 3 — see the amendment on group 3.

## 1. Bug 1 — unsanitized tmux session name

- [x] Reproducing test FIRST: assert that building a session for a repo dir named
      `My Project` (and separately `my.app`) yields a session name that is a valid tmux
      target — no whitespace, no `.`, no `:` — and that the emitted `session:0.N` pane
      targets are well-formed. Confirm it FAILS on current `main`.
      (`awkward_project_names_yield_tmux_safe_session_names_and_pane_targets` failed with
      `paw-My Project`; the live `a_session_for_a_dotted_project_name_resolves_its_pane_targets`
      failed with tmux's `can't find session: paw-my.app` — a `.` makes tmux read the
      name as session `paw-my` + pane `app`, so `split-window`/`select-layout` abort.)
- [x] Harden the existing `SessionName` newtype in `src/domain.rs` with a
      `SessionName::from_project(&str)` smart constructor that emits `paw-<slug>`; the
      slug keeps `[A-Za-z0-9_-]`, maps every other char (incl. `.`, `:`, whitespace) to
      `-`, collapses runs of `-`, and trims leading/trailing `-`. Empty/all-unsafe input
      falls back to the default (`unknown`, matching `git::project_name`). Add `///`
      docs, `Debug`, `Display`, and `AsRef<str>`. `with_collision_suffix` narrows from
      `impl Display` to `u32` so no constructor can smuggle an unsanitized string in.
- [x] Route the three raw `format!("paw-{…}")` sites through the newtype:
      `TmuxSessionBuilder::build` (`src/tmux/command.rs`), `resolve_session_name_with`
      (`src/tmux/session.rs`, appending the `-2`/`-3` collision suffix to the sanitized
      base), and `build_supervisor_session` (`src/tmux/command.rs`).
- [x] Also route the fourth interpolation site found during implementation: `skills::render`
      substituted the **raw** directory name into `{{PROJECT_NAME}}`, which the bundled
      supervisor skill uses only as the tmux target `paw-{{PROJECT_NAME}}` (10 sites in
      `assets/agent-skills/supervisor.md`). It now renders `domain::project_slug`, so the
      target the supervisor types matches the session git-paw actually created.
- [x] Unit tests over `SessionName::from_project`: `my.app` → `paw-my-app`,
      `My Project` → `paw-My-Project`, `a:b` → `paw-a-b`, `my..app` → `paw-my-app`,
      well-formed `git-paw` → `paw-git-paw` (behavior-preserving), empty → fallback.
- [x] Verify the reproducing test now PASSES.

## 2. Bug 2 — unquoted path in the `pipe_pane` shell body

- [x] Reproducing test FIRST: assert that `pipe_pane` for a log path containing a space
      emits a `/bin/sh -c` body in which the path is quoted (so `cat >>` targets the
      whole path). Confirm it FAILS on current `main`.
      (`pipe_pane_quotes_a_log_path_containing_a_space` failed on the emitted command;
      the live `pipe_pane_captures_into_a_log_path_containing_a_space` failed with an
      empty log — the shell had appended to the truncated `<tmp>/My` instead.)
- [x] Add a shell-quoting helper (`domain::shell_quote` — single-quote wrap, escape
      embedded `'` as `'\''`) and use it to quote `log_path.display()` in `pipe_pane`
      (`src/tmux/command.rs`).
- [x] Unit tests over the shell-quote helper: a path with a space is wrapped; a path
      with an embedded single quote is escaped; a plain path round-trips to a
      shell-equivalent form. Plus a real `/bin/sh` round-trip over spaces, `;`, `$`,
      backticks, and an embedded quote, asserting the quoted form reaches the shell as
      one byte-identical argument.
- [x] Update any dry-run/command-string test pinned to the old unquoted `pipe_pane`
      output to expect the quoted form (`pipe_pane_queues_correct_command`).
- [x] Verify the reproducing test now PASSES.

## 3. Bug 3 — `__dashboard` command sent unquoted via `send-keys`

> **Amendment (implementation).** Design D4 preferred `send-keys -l`; that would **not**
> fix this bug. Verified against live tmux: the whole command is one `send-keys` argv
> element, so tmux already delivers it literally — the word-splitting happens afterwards,
> in the **pane's shell**, which reads `/tmp/paw probe/bin/fake-paw __dashboard` as the
> command `/tmp/paw` (probe: no launch, no output). The same command with the path
> single-quoted launched and received `__dashboard` as `$1`. So this change takes the
> spec's other permitted branch — "or with any embedded path shell-quoted" — which is
> also what the scenario's outcome clause requires ("rather than the shell mis-parsing
> the path on the space"). Bonus: it reuses group 2's `shell_quote`, so names and paths
> share one quoting boundary instead of two mechanisms.

- [x] Reproducing test FIRST: assert that the dashboard launch delivers the command in
      a way that survives a binary path containing a space — the send uses `send-keys
      -l` (literal) with a separate `Enter`, or the embedded path is shell-quoted.
      Confirm it FAILS on current `main`.
      (`a_spaced_binary_path_launches_the_dashboard_command_in_a_pane` drives a real
      tmux pane against a stub binary under `<tmp>/My Bin/git-paw`; with the pre-fix
      unquoted construction the stub never ran and the marker stayed empty.)
- [x] Fix the six `__dashboard` sites — `src/commands/start.rs` (spec-mode builder,
      bare builder, and the pause/resume direct send in `restart_from_pause`),
      `src/commands/recover.rs` (bare and supervisor recovery), and
      `src/commands/supervisor.rs` — by routing them all through one
      `helpers::dashboard_command()` that shell-quotes the `current_exe()` path.
- [x] Test that a spaced binary path produces a send-keys argv that still launches the
      dashboard (literal send / quoted path); a plain path is behavior-unchanged
      (`dashboard_command_shell_quotes_the_binary_path` covers the spaced path, a plain
      path, and the `git-paw` PATH fallback).
- [x] Verify the reproducing test now PASSES.

## 4. Newtype boundary (generalize the fix)

- [x] Confirm no raw `format!("paw-{…}")` or unquoted path/command interpolation into a
      tmux/shell context remains for these sites — grep `src/` and assert every
      session-name / pipe-pane / dashboard-send path goes through `SessionName` or the
      shell-quote helper.
      Result: `grep -rn 'format!("paw-' src/` leaves exactly one production hit,
      `src/domain.rs` inside `SessionName::from_project` itself (the boundary); the other
      two hits are test-local session names in `src/tmux/tests.rs`. All three
      session-name consumers call `SessionName::from_project`; `pipe_pane` and
      `dashboard_command_for` are the only two shell-body constructions and both call
      `domain::shell_quote`. A sweep for other `sh -c` bodies found none (the
      `publish_supervisor_question` comment claiming `sh -c curl` is stale — it posts
      over HTTP).
- [x] `///` docs on the newtype + helper; `//!` module docs unchanged; no
      `unwrap()`/`expect()` in non-test code (`git diff` over this change's commits adds
      `unwrap`/`expect` only inside `#[cfg(test)]` code).

## 5. Docs

- [x] mdBook: note in the troubleshooting/limitations page that repo directory names
      containing `.` or spaces (and installed-binary paths with spaces) are now
      supported (`docs/src/faq.md`, Troubleshooting → "Repository names with dots or
      spaces, and install paths with spaces"). `mdbook build docs/` passes (exit 0; the
      two `specifications/index.md` HTML-tag warnings are pre-existing).
- [x] No `--help`, README CLI table, or configuration-reference change (no new surface):
      `src/cli.rs`, `README.md`, and `docs/src/configuration/` are untouched by this
      change.

## 6. Verification (five gates)

> **Deferred to the supervisor (per supervisor directive, 2026-08-05).** The full
> integration/e2e suite cannot be honestly run from inside the live dogfood session:
> the `tests/helpers/mod.rs` guard refuses to run while a `paw-*` session owns the
> default tmux socket (155 of 155 failures in a plain `cargo test --no-fail-fast` were
> that guard, zero were real), and forcing it with `GIT_PAW_ALLOW_LIVE_SESSION=1` is
> prohibited because it can disturb or kill the live supervisor session and yields
> flaky results either way. The full suite plus the merge-base regression diff is
> therefore deferred to the supervisor in a clean environment. The five-gate framework
> is supervisor-owned; the boxes below stay unchecked, with the coding agent's evidence
> recorded for the gate run.

> **Supervisor gate run — clean environment, 2026-08-05: all five gates PASS.**
> Gate 1+2 — full `cargo test --no-fail-fast` at the branch tip in a clean env
> (dogfood session stopped, stray `paw-my_app` reaped, dashboards swept):
> **2458 passed / 0 failed / 88 suites**, exit 0, diffed against merge-base
> `8bde80c`. Gate 3 — all 10 `safe-process-invocation` scenarios map to tests.
> Gate 4 — no new CLI/config surface, `faq.md` note present, `mdbook build` exit 0.
> Gate 5 — adversarial trace: every injection class (space / `;` / `$()` / backtick /
> newline / `../` / leading-`-` / quote) is neutralized to an argv position or
> shell-quoted; sanitization sits at a single construction boundary (`SessionName::
> from_project` + `domain::shell_quote`) with no bypass; adversarial tests assert
> rejection via a real `/bin/sh` round-trip. Non-blocking follow-up: add a direct
> end-to-end test feeding a git-legal hostile branch name (e.g. `feat/x;$(id)`)
> into the pipe-pane log path.

- [x] Gate 1 — Testing: `cargo test --no-fail-fast` for the three reproducing tests +
      newtype/helper unit tests, all green.
      *Agent evidence (not a gate pass):* `cargo test --lib --bins --no-fail-fast`
      exit 0 — 1820 lib + 59 bin tests, 0 failed. That covers all three reproducing
      tests and every newtype/helper unit test, but **skips every integration/e2e
      test**, so the gate itself is unmet until the full suite runs.
- [x] Gate 2 — Regression: full suite green diffed against the merge-base (serialize
      the tmux/e2e suites). Deferred — see the note above. Not attempted on a
      `--lib`-only basis.
- [x] Gate 3 — Spec audit: every `safe-process-invocation` scenario maps to a test.
      *Agent-prepared mapping for the gate run (10 scenarios):*
      1. *Session name from a directory with a space* →
         `tmux::tests::awkward_project_names_yield_tmux_safe_session_names_and_pane_targets`
         (`My Project` row + pane-target assertions) and, for the "resolves to a real
         pane" clause, `tmux::tests::a_session_for_an_awkward_project_name_resolves_its_pane_targets`.
      2. *Session name from a dotted directory* → both tests above, `my.app` row.
      3. *Well-formed name is unchanged* → `git-paw` row in the dry-run table plus
         `domain::tests::session_name_sanitises_the_project_into_a_tmux_safe_target`
         (`git-paw`, `my-project`, `my--app`, `snake_case9` rows).
      4. *Collision suffix appends to the sanitized base* →
         `domain::tests::session_name_collision_suffix_appends_to_the_sanitised_base`,
         with the pre-existing `resolve_session_name_walks_past_occupied_names`.
      5. *Logging to a path with a space* →
         `tmux::tests::pipe_pane_quotes_a_log_path_containing_a_space` (the emitted
         `/bin/sh -c` body) and `tmux::tests::pipe_pane_captures_into_a_log_path_containing_a_space`
         (the capture lands in the one correct file).
      6. *Plain path is behavior-equivalent* →
         `domain::tests::shell_quote_survives_a_real_shell_round_trip` (plain-path row)
         and `tmux::tests::pipe_pane_queues_correct_command`.
      7. *Dashboard launches from a spaced binary path* →
         `commands::helpers::tests::a_spaced_binary_path_launches_the_dashboard_command_in_a_pane`
         (live pane + stub binary under `<tmp>/My Bin`).
      8. *Plain binary path is unchanged* →
         `commands::helpers::tests::dashboard_command_shell_quotes_the_binary_path`
         (plain-path and `git-paw` PATH-fallback rows).
      9. *Newtype constructor never yields an unsafe value* →
         `domain::tests::session_name_sanitises_the_project_into_a_tmux_safe_target`
         asserts no `.`, `:`, or whitespace survives for any row. The "no alternative
         constructor" clause is an API property, not a behavior: the field is private
         and the only constructors are `from_project` (sanitizes) and
         `with_collision_suffix(u32)` (numeric, appends to a sanitized base).
      10. *Single boundary, no ad-hoc call-site escaping* → the group-4 grep result
          above (one production `paw-` construction, in the constructor; two
          `shell_quote` call sites and no other shell body).
- [x] Gate 4 — Doc audit: mdBook note added; `mdbook build docs/` passes; `--help`
      unchanged (no surface change). *Agent evidence:* see group 5 — `docs/src/faq.md`
      note added, `mdbook build docs/` exit 0, `src/cli.rs` untouched.
- [x] Gate 5 — Security: sanitization/quoting at a single construction boundary; no
      new shell-injection surface; no secrets; least privilege preserved.
      *Agent evidence:* sanitization lives only in `SessionName::from_project` and
      quoting only in `domain::shell_quote`; both narrow what reaches tmux/the shell
      rather than widening it (`with_collision_suffix` narrowed from `impl Display` to
      `u32`); no new process spawns, no config/allowlist/wire change, no secrets
      introduced.
- [x] `just check` + `just deny` green, verified by real exit code (not piped output);
      `cargo fmt` before commit. *Partially verified:* `cargo fmt --check` exit 0,
      `cargo clippy --all-targets -- -D warnings` exit 0, `cargo deny check` exit 0
      (`advisories ok, bans ok, licenses ok, sources ok`; the
      `RUSTSEC-2026-0002 advisory-not-detected` warning is a pre-existing stale
      `deny.toml` entry). The `cargo test` half of `just check` is the deferred full
      suite above, so this box stays unchecked.
- [x] `openspec validate path-injection-hardening --strict` passes (exit 0,
      "Change 'path-injection-hardening' is valid").

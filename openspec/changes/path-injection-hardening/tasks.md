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

- [ ] Reproducing test FIRST: assert that `pipe_pane` for a log path containing a space
      emits a `/bin/sh -c` body in which the path is quoted (so `cat >>` targets the
      whole path). Confirm it FAILS on current `main`.
- [ ] Add a shell-quoting smart-constructor helper (single-quote wrap, escape embedded
      `'` as `'\''`) and use it to quote `log_path.display()` in `pipe_pane`
      (`src/tmux.rs` ~:230-237).
- [ ] Unit tests over the shell-quote helper: a path with a space is wrapped; a path
      with an embedded single quote is escaped; a plain path round-trips to a
      shell-equivalent form.
- [ ] Update any dry-run/command-string test pinned to the old unquoted `pipe_pane`
      output to expect the quoted form.
- [ ] Verify the reproducing test now PASSES.

## 3. Bug 3 — `__dashboard` command sent unquoted via `send-keys`

- [ ] Reproducing test FIRST: assert that the dashboard launch delivers the command in
      a way that survives a binary path containing a space — the send uses `send-keys
      -l` (literal) with a separate `Enter`, or the embedded path is shell-quoted.
      Confirm it FAILS on current `main`.
- [ ] Fix the six `__dashboard` sites (`src/main.rs` ~:632, :1397/:1417, :2017, :2188,
      :2409, :2481) and the pause/resume direct send (~:2226) to send the command
      literally (`-l` + follow-up `Enter`) or shell-quote the `current_exe()` path.
      Prefer the literal path to match the existing `build_send_keys_args` seam.
- [ ] Test that a spaced binary path produces a send-keys argv that still launches the
      dashboard (literal send / quoted path); a plain path is behavior-unchanged.
- [ ] Verify the reproducing test now PASSES.

## 4. Newtype boundary (generalize the fix)

- [ ] Confirm no raw `format!("paw-{…}")` or unquoted path/command interpolation into a
      tmux/shell context remains for these sites — grep `src/` and assert every
      session-name / pipe-pane / dashboard-send path goes through `SessionName` or the
      shell-quote helper.
- [ ] `///` docs on the newtype + helper; `//!` module docs unchanged; no
      `unwrap()`/`expect()` in non-test code.

## 5. Docs

- [ ] mdBook: note in the troubleshooting/limitations page that repo directory names
      containing `.` or spaces (and installed-binary paths with spaces) are now
      supported. `mdbook build docs/` passes.
- [ ] No `--help`, README CLI table, or configuration-reference change (no new surface).

## 6. Verification (five gates)

- [ ] Gate 1 — Testing: `cargo test --no-fail-fast` for the three reproducing tests +
      newtype/helper unit tests, all green.
- [ ] Gate 2 — Regression: full suite green diffed against the merge-base (serialize
      the tmux/e2e suites).
- [ ] Gate 3 — Spec audit: every `safe-process-invocation` scenario maps to a test.
- [ ] Gate 4 — Doc audit: mdBook note added; `mdbook build docs/` passes; `--help`
      unchanged (no surface change).
- [ ] Gate 5 — Security: sanitization/quoting at a single construction boundary; no
      new shell-injection surface; no secrets; least privilege preserved.
- [ ] `just check` + `just deny` green, verified by real exit code (not piped output);
      `cargo fmt` before commit.
- [ ] `openspec validate path-injection-hardening --strict` passes.

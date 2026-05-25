# Design — test-coverage-v0-5-0

## Context

The 42 uncovered scenarios fall into five test-shape buckets, each with a
known testing pattern in the existing suite or a known gap that justifies
deferral. This design documents the decisions for each shape so the
implementing agent does not re-derive them, and so future coverage audits
recognise the deferred items as intentional skips rather than new gaps.

## Decisions

### D1 — Pure-function tests for `build_task_prompt`

**Decision:** lift `build_task_prompt` from private `fn` to `pub(crate) fn` in
`src/main.rs`. Call it directly from the existing `#[cfg(test)] mod tests {}`
block in `src/main.rs`.

**Why:** `build_task_prompt` is documented in `supervisor-launch/spec.md`
(via the boot-prompt-full-body delta) as a *pure function* — "no I/O side
effects ... callable from `cfg(test)` without launching tmux". The shipped
function already meets this contract (no filesystem reads, no process spawns,
takes `Option<&SpecEntry>` and returns `String`). It's already called from
three tests in `src/main.rs` (lines 2154, 2167, 2184) — which compile because
the test module is in the same crate. The remaining two scenarios
("spec-derived task prompt contains AGENTS.md + spec id", "build_task_prompt
is a pure function") need:

- A test that builds a `SpecEntry` with `id = "governance-config"`, calls
  `build_task_prompt(Some(&entry))`, and asserts the return string contains
  `"AGENTS.md"` AND `"openspec/changes/governance-config"` AND does NOT
  contain the spec's first body line.
- A "purity" test that calls `build_task_prompt(Some(&entry))` twice with the
  same input and asserts the outputs are byte-equal (deterministic), AND
  surrounds the call with `tempdir`/`std::env::current_dir` instrumentation
  asserting no filesystem read occurred from `build_task_prompt`'s body
  (statically — by reading the function's source via `include_str!("main.rs")`
  and grepping for `std::fs::`, `File::open`, `Command::new` between the
  function's start and end). The static-grep approach side-steps needing a
  process-tracing harness; it's brittle if the function body grows complex,
  but for a 20-line function it's the cheapest reliable purity assertion.

**Alternative considered:** put the function in `src/skills.rs` or
`src/specs/mod.rs` as `pub`. Rejected — the function is a `cmd_supervisor`
helper, not a public API surface; widening visibility beyond crate-internal
adds a stability commitment we don't want.

### D2 — "Thread runs inside subprocess" properties

**Decision:** **defer** to a follow-up `test-coverage-v0-5-0-followup`
change. Document the deferral in `tasks.md` under "Deferred per design.md
D2".

**Why:** the `supervisor-as-pane` spec asserts "the auto-approve poll thread
SHALL run inside the dashboard's `__dashboard` subprocess, NOT inside the
`cmd_supervisor` process". Verifying this property requires:

1. Spawning `git-paw __dashboard` as a real subprocess (or simulating it).
2. Inspecting the subprocess's thread list (via `/proc/<pid>/task/` on Linux,
   `pthread_get_threads_np` or `proc_listpids` on macOS) to find the
   auto-approve thread by name or by stack signature.
3. Asserting `cmd_supervisor`'s process does NOT spawn an auto-approve
   thread.

This is platform-specific (Linux vs macOS thread-listing differs), requires
a live broker port, and has historically been flaky in CI. The structural
property is enforced at the *source-code* level by where the thread is
spawned (`run_dashboard` in `dashboard/mod.rs` vs `cmd_supervisor` in
`main.rs`); a grep-based negative-assertion test would catch a regression
much more cheaply than a process-introspection test. The tasks.md ships a
grep-based assertion as the fallback rather than the live-subprocess test:
`tests/source_audit.rs` grep-asserts that `cmd_supervisor` (between its
opening brace and closing brace, located via static parsing) does NOT
contain the substring `spawn_auto_approve_thread` (or whatever the spawner
function is named).

**Alternative considered:** use `std::process::Command` to launch
`git-paw __dashboard` with a `--print-threads-and-exit` debug flag. Rejected
— adds a permanent debug-only CLI surface for the sake of one test, and the
spec property "thread runs inside subprocess" is most cheaply caught at the
source-grep level. Live subprocess introspection is reserved for the
follow-up if the grep proves insufficient.

### D3 — Interactive `Ctrl+C` and zero-selection cancellation

**Decision:** use the existing `TrackingPrompter` stub in
`src/interactive.rs:464` (and `for_specs` builder at line 861+). Extend it
with two new builder methods if needed:

- `TrackingPrompter::cancel_on_specs()` — returns
  `Err(PawError::UserCancelled)` from `select_specs`.
- `TrackingPrompter::for_specs_empty()` — returns `Ok(vec![])` from
  `select_specs` (the "user confirmed with zero rows" path).

The two test cases assert that the caller (the spec-resolution flow that
invokes `select_specs`) propagates the cancellation as expected:

- `cancel_on_specs()` → caller returns `Err(PawError::UserCancelled)`.
- `for_specs_empty()` → caller treats empty-Vec as cancellation per spec
  ("User confirming with zero rows selected → `PawError::UserCancelled`"),
  so the test asserts the *caller* maps `Ok(vec![])` to `UserCancelled`.
  This places the mapping responsibility in the picker caller, not in
  `select_specs` itself — `select_specs` returns `Ok(vec![])` faithfully,
  and the caller wraps. If the current code maps inside `select_specs`, the
  test calls `select_specs` directly and asserts the Err result; if it maps
  in the caller, the test calls the caller. Either is consistent with the
  spec wording; the test inspects the shipped code first.

**Why:** the `Prompter` trait was introduced specifically so picker flows
could be unit-tested without a live TTY. The existing `TrackingPrompter`
covers `select_branches` and (per recent extensions) `select_cli` and
`select_specs`; the cancel-on-specs builder is a one-line addition matching
the existing `cancel_on_branches` / `cancel_on_cli` pattern.

**Alternative considered:** spawn a real `git paw start --specs` subprocess
with a pty harness that sends literal Ctrl+C bytes. Rejected — the
`Prompter` indirection exists precisely to avoid this; using it bypasses pty
machinery, runs in <1ms, and tests the spec property (cancellation maps to
`PawError::UserCancelled`) directly.

### D4 — `cmd_supervisor` flow without live tmux

**Decision:** rely on three layered assertions, no live tmux required:

1. **Dispatch-target unit test.** Parse a `git paw start --supervisor`
   invocation through clap and assert the dispatch goes to `cmd_supervisor`
   (not `cmd_start_from_specs`). The dispatch table in `src/main.rs:79-85`
   is testable via the existing `dispatch_*` test fixtures.
2. **Tmux command-string contract.** `tmux::build_supervisor_layout_args`
   (or whichever helper builds the argv) is a pure function returning a
   `Vec<String>` of tmux-command tokens. Assert its return value contains
   the expected `split-window`, `resize-pane`, and `select-pane` tokens in
   the right order for a given agent count. This is the same pattern as the
   existing `build_boot_inject_args` test in `src/tmux.rs`.
3. **Stdout-assertion E2E.** Run `git-paw start --supervisor --branches a,b`
   via `assert_cmd` from a non-TTY context. Assert:
   - Exit code 0.
   - Stdout contains `"Supervisor session 'paw-"` and `"tmux attach -t paw-"`.
   - Process returns within 10 seconds (asserting `cmd_supervisor` does NOT
     block on a foreground process).
   The non-TTY guard in `cli-parsing/spec.md` ensures the actual tmux session
   is created in detached mode without the supervisor CLI being spawned;
   the test cleans up the session in a `Drop` guard.

**Why:** `cmd_supervisor`'s spec assertions decompose into "dispatched
to", "produces this argv", and "returns immediately with this stdout". None
of those require attaching to a live pty. The full attach flow is the one
property left untestable (per design.md D5).

**Alternative considered:** mock the tmux binary with a shim that records
its argv. Rejected — adds a build-time dependency on a fake-tmux binary;
the spec's wire-format claim is most directly tested at the
`build_*_args` boundary.

### D5 — Deferred gaps (live broker / live tmux properties)

The following gaps are **not** addressed by this change. They are
documented here so the next coverage audit recognises them as intentional
skips:

1. **TTY launch attaches as before** (from-specs-launch-fixes).
   `tmux::attach(...)` calls `execvp` to replace the process with `tmux
   attach`. The only observable property is "the call was made"; this
   requires either a live TTY harness or a `Command`-mocking layer the suite
   doesn't currently have. Deferred. The non-TTY scenarios (which DO have
   a test) backstop the same code path.

2. **ConflictConfig partial fields** (conflict-detection). Already covered
   by the existing `ConflictConfig defaults when section absent` and
   `ConflictConfig with all fields populated` tests — the partial-fields
   case is a linear combination of the two and would not catch a
   regression neither of those does. Drop from coverage requirement.

3. **Unknown spec rejection E2E** (cross-format-spec-selection). The
   `Unknown spec name is rejected with candidate list` scenario in
   `interactive-selection/spec.md` is already covered by the
   name-resolution unit test. The "E2E" framing referred to running
   `git paw start --specs no-such-spec` through `assert_cmd`; the unit
   test asserts the same property at a lower layer. Drop.

4. **Whitespace-only question rejection** (v040-hardening). Already covered
   by the existing validation test for `payload.question` (the test asserts
   the error message identifies `question`; whitespace-only is a
   sub-case of the existing assertion). Drop.

5. **Tasks attach to phase / branch slug safe chars / tasks.md writeback**
   (spec-kit-format). Three scenarios marked "covered" in the audit input —
   they already have tests in the spec-kit parser test module. Drop.

6. **Auto-approve thread runs inside dashboard subprocess** (supervisor-as-
   pane). Per D2 above, replaced with a source-grep assertion that
   `cmd_supervisor` does not contain the auto-approve spawn call. The live
   process-introspection version is deferred to a follow-up change.

These are recorded in `tasks.md` under a `## Deferred` footer so the next
auditor sees them explicitly skipped.

## Risks / Trade-offs

- **Pub(crate) visibility lift.** Lifting `build_task_prompt` to
  `pub(crate)` couples the test module to the function's existence by name;
  future renames break the test. Acceptable — the function is documented
  *by name* in the spec, so the test is already coupled at the spec level.

- **Source-grep purity test.** The "purity" assertion via static source
  inspection is brittle: a future contributor adding `std::fs::read_to_string`
  inside `build_task_prompt` would silently violate the spec without the
  grep catching it if the call is hidden behind an alias. Mitigation: the
  grep pattern is documented in `tasks.md` and runs against
  `cfg(test)`-only fixtures, so a regression at code-review time is the
  primary catch; the test is the secondary catch.

- **Deferred items accumulate.** Five deferred items here are five future-
  audit landmines if the deferral rationale is not preserved. Mitigation:
  the deferral list lives in the archived spec at
  `openspec/specs/test-coverage-v0-5-0/spec.md` (post-archive) under a
  dedicated requirement, so the next auditor reads them as documented
  skips rather than as new gaps.

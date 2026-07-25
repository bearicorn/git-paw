# Design — git-paw-doctor

## Context

There is no single command that answers "why won't it launch?". Failure modes are spread
across environment (tmux/git absent), detection (no CLIs), config (unparseable / spec
system unconfigured), and provisioning (missing/stale bundled scripts — the `sweep.sh`-gone
case). `git paw doctor` consolidates these into one read-only preflight report before the
v1.0.0 freeze locks the CLI surface.

## Decisions

### D1 — Status model: a three-state enum with a remedy

Each check yields `CheckStatus { Ok, Warn, Fail }` plus a `name`, `detail`, and (for
non-`Ok`) a `remedy`. The report groups checks under fixed headings (Environment, CLIs,
Config, Spec system, Bundled scripts, Broker, Supervisor, Hygiene).

### D2 — Exit code = worst status

`Fail` anywhere → non-zero exit; otherwise 0 (a `Warn` never fails the process). This makes
`doctor` usable as a CI/pre-launch gate that only blocks on true blockers.

### D3 — Checks are pure functions over injected state (testability)

Each check is written as a pure function from already-probed inputs (a `which` result, a
parsed config, a directory listing, an embedded-script hash) to a `CheckResult`. The
command's I/O layer does the probing; the checks do the judging. This keeps every check
unit-testable without a real environment, and keeps the module clear of hidden I/O.

### D4 — `--json` shares the same check results

Human and `--json` rendering consume the identical `Vec<CheckResult>`; only the renderer
differs, guaranteeing the two surfaces (and their exit codes) never diverge.

### D5 — Diagnose-only; `--fix` deferred

v0.13.0 ships diagnose-only. `--fix` (re-run idempotent init repairs: reinstall bundled
scripts, add missing gitignore entries) was considered and cut to keep the headline lean and
the pre-freeze surface minimal. If added later it re-runs only safe, idempotent init steps —
never anything destructive.

### D6 — Complements `selftest`, does not overlap

`selftest` (v0.9.0) launches a live dummy-CLI session to smoke the orchestration path.
`doctor` is static: it never spawns tmux sessions or CLIs, only inspects. The two are
adjacent and both referenced from each other's help.

### D7 — Export-agnostic supervisor check

The Supervisor gate-command check resolves the verbs to probe from the *resolved stack
preset* (`[supervisor.common_dev_allowlist]` / configured gate commands), never from
git-paw's own cargo/just toolchain. This upholds the `AGENTS.md` export-agnosticism
principle — `doctor` must not assume every consumer builds with cargo.

### D8 — Module placement

Land as `src/doctor.rs` with a `Doctor` variant on the `Commands` enum and a dispatch arm in
`main.rs`. If the v0.13.0 code-analysis workstream extracts a `commands/` module, doctor
moves there with the other handlers — no contract impact.

## Non-goals

- No repair/mutation (see D5).
- No live session or CLI spawning (that is `selftest`).
- No new config fields.

## Risks

- **Low.** Additive, read-only. The main correctness risk is a check that reports a false ✗
  (e.g. a version-parse edge or a port-probe race); each check's threshold is unit-tested,
  and ambiguous states resolve to ⚠ rather than ✗ so doctor never blocks spuriously.

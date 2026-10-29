## Context

Inventory at this branch's tip:

| Source | Exit | Notes |
|---|---|---|
| `main()` → `run()` `Ok` | 0 | includes `--unattended` outcomes `completed`, `escalated-for-review`, `stuck`, `heartbeat`, `session-torn-down`; a declined `purge` ("Purge cancelled.") |
| `PawError::exit()` (the only `process::exit`) | `exit_code()` | `UserCancelled` and `NotAttempted` (v0.17.0, `git paw setup-cli`) → 2, `IntegrationRefused` (v0.21.0, `git paw finish`) → 3, every other variant → 1 |
| clap `Cli::parse()` | 2 | unknown flag, missing value, conflicting flags; 0 for `--help`/`--version` |
| `doctor` | 1 via `DoctorFailed` | ✗ only; ⚠ is 0 (`doctor::exit_code`) |
| `run_drive_loop` | 1 via `SessionError` | only `cli-unavailable` (an outcome v0.18.0 retires: the orchestrator leaves the session, and a coding agent's unavailable CLI is an escalation) |
| `cmd_purge` without a TTY or `--force` | 2 | dialoguer fails → `map_err(|_| UserCancelled)` → `error: Cancelled.` |

`PawError` has 20 variants; 12 wrap a free-form `String` and carry ~360 construction sites
(`SessionError` alone ~126). `IoError` is `#[from] std::io::Error`, so a bare `?` produces a
message with no path. Config parse errors already name the file (`load_config_file`) — the model
the rest should follow.

The coordinator's reading of the v1.0.0 contract: freeze `0/1/2`, with `2` meaning *nothing was
attempted* (an argument error including clap's, or a cancelled prompt), and give the
`--unattended` non-completion outcomes a distinct non-zero code now, because v1.0.0 is the last
chance to change it. That code is `3`, defined generically as *ran but stopped short of its
goal, reported the outcome, left nothing half-done*, so it also covers v0.21.0's `finish`
refusal (`IntegrationRefused`), which already uses `3`. The v1.0.0 freeze table carries four
codes.

## Goals / Non-Goals

**Goals:**

- One meaning per code, stated in one table, true in every command.
- Every user-reachable failure tells the user what to do next.
- `--from-specs` warns for one release before v1.0.0 removes it.

**Non-Goals:**

- Exit codes of hidden IPC subcommands (`__prompt-profile`, `__dashboard`) and of
  bundled scripts — internal or owned by v0.20's packaging work.
- Rewording every message. The audit fixes messages that lack an object or a next step; it does
  not restyle the rest.
- Structured (`--json`) error output — v0.21's `uniform-json-output`.

## Decisions

### D1 — The table

| Code | Meaning | Produced by |
|---|---|---|
| 0 | The command did what was asked | `Ok` |
| 1 | It was attempted and failed | every `PawError` not listed below, including an error that stops the drive loop; `doctor` ✗ |
| 2 | Nothing was attempted | clap argument errors; `UserCancelled` (cancelled selection or prompt); `NotAttempted` (a declined proceed-confirmation, a required confirmation with no terminal, an unsupported or disabled item) |
| 3 | The command ran but stopped short of its goal | `WaveIncomplete`: an `--unattended` run ended `escalated-for-review`, `stuck`, `heartbeat` or `session-torn-down`; `IntegrationRefused`: `finish` declined by its merge policy after changing nothing |

Keeping clap and cancellation together on 2 is deliberate: both mean "nothing changed, safe to
re-run differently", which is what a script needs to know. *Alternative rejected:* moving
cancellation to 130 (the shell's SIGINT convention) to separate it from usage errors — it breaks
the documented `2 = cancelled` for no script-visible benefit and contradicts v1.0's draft.

`3` is scoped to those two outcomes, not to "anything that needs attention". `doctor` ✗ stays 1:
it is a failed check, and its documented behaviour does not change. *Alternatives for the
unattended outcomes:* reuse 1 (rejected — indistinguishable from a crash or setup error, so a
wrapper cannot decide whether to resume); one code per outcome (rejected — the summary on stdout
already names the outcome, and four codes freeze four contracts).

### D2 — `WaveIncomplete` carries the outcome to the one exit path

`run_drive_loop` prints the summary as today, then returns `Err(PawError::WaveIncomplete(label))`
for the four outcomes; `exit_code()` maps it to 3 and `exit()` prints
`error: unattended run ended without completing (<label>) — see the summary above`. No second
`process::exit` call appears, and the `cmd_*` library work in v0.18 keeps returning results.

### D3 — Declining and no-terminal confirmations use `NotAttempted`

v0.17.0's `cli-setup-consent` added `PawError::NotAttempted` (exit 2) for exactly these cases in
`git paw setup-cli`: a declined confirmation, a confirmation that cannot be shown because stdin is
not a terminal, and an unsupported item. Its message states the condition and names the flag that
proceeds, and never reads only `Cancelled.`. `purge` reuses it instead of a new variant:

- a declined proceed-confirmation (`purge`'s "n") returns `NotAttempted` stating that the
  confirmation was declined, and exits 2;
- a confirmation that cannot be shown because stdin is not a terminal returns `NotAttempted` —
  "`purge` needs confirmation and stdin is not a terminal; re-run with `--force`" — also exit 2.

`UserCancelled` stays for a cancelled selection or prompt (empty picker, Ctrl+C). Question prompts
that choose an option (`init`'s broker prompt, the supervisor-mode prompt) are answers, not
cancellations, and are unaffected. The earlier draft's separate ADDED requirement for the
no-terminal case is folded into *Exit codes distinguish cancellation from errors*, which already
owns `NotAttempted`'s message rule.

### D4 — I/O context is enforced by the compiler

Replace `IoError(#[from] std::io::Error)` with a variant holding the operation, the path and the
source, constructed through `PawError::io(op, path, err)`. Removing `#[from]` makes every bare `?`
on an `io::Result` a compile error, so the audit of those sites is finished when the crate builds.
The `Display` is `I/O: <op> <path>: <source>`. *Alternative rejected:* a lint or source scan —
heuristic, and the compiler already does it exactly.

### D5 — The message rule, and how it is checked

A user-facing message SHALL name what failed, the object (path, branch, session, CLI, port,
config key) and a next step when one exists. The check is behavioural: `tests/error_matrix_e2e.rs`
is a table of user-reachable failures driven through the compiled binary — not a git repo, tmux
missing, no CLI on `PATH`, unknown `--cli`, unknown preset, invalid config TOML, I/O failure while
provisioning a worktree, `purge` without a terminal, an argument error — asserting the exit code
and that stderr names the object and the next step (stable anchors, not exact prose). The
construction-site walk is a task; messages it finds lacking are fixed in place.

### D6 — `--from-specs` warning

clap aliases do not report which spelling matched. v0.21.0's `supervisor-vocabulary-split`
already solved this for `--supervisor` / `--no-supervisor`: `main` runs `deprecation::scan_args`
over the raw arguments (up to a `--` terminator) after `Cli::parse()` and warns through
`deprecation::warn`, both driven by the registry in `src/deprecation.rs` (`core-deprecation`).
This change only adds the registry entry `{ item: "--from-specs", kind: flag, since: "0.24.0",
replacement: "--from-all-specs", removed_in: Some("1.0.0") }`; no second warn function exists.
Exit code and stdout are unchanged. The registry's removal guard makes v1.0.0's
`remove-from-specs-alias` delete the entry when it removes the alias.

### D7 — Spec reconciliation (flagged)

The MODIFIED *Actionable error messages* requirement drops six "`<Variant>` includes detail"
scenarios and the "CliNotFound includes CLI name" scenario. Their `Test:` pointers name tests that
`src/error.rs` deleted as tautologies (they re-checked that thiserror interpolates `{0}`). They are
replaced by the behavioural error matrix, which checks what the user actually sees. The separate
*Debug representation is derivable* requirement also points at a deleted test
(`test_debug_derived`); it is left untouched here and reported for the pre-freeze spec audit.

### D8 — An escalated prompt exits 3 even when the work completed

Since v0.17.0 (`classifier-retirement`) the drive loop escalates every live permission prompt and
approves none, and any escalation makes the outcome `escalated-for-review`. So a run in which a
prompt went to a human exits 3 even if, by the end, every agent finished and every branch merged.
This is kept on purpose: the wrapper cannot see from exit 0 that a human decision was involved, and
the summary's escalation list is what it should read. The contract states it in every layer that
documents exit codes, and an e2e test pins it.

*Alternative rejected:* exit 0 when every branch merged despite escalations. It would hide that a
prompt waited on (or was answered by) a human, which is the signal an unattended wrapper needs.

## Risks / Trade-offs

- [Scripts relying on `--unattended` exiting 0 after a heartbeat] → Pre-1.0 and called out with a
  `!` breaking marker in the changelog; the summary text is unchanged, and the new code is the
  signal those scripts were missing.
- [Scripts relying on a declined `purge` exiting 0] → Declining is interactive-only; a script
  passes `--force`. Called out in the changelog.
- [The I/O variant change touches many files] → Mechanical, compiler-driven, and confined to
  error construction; done as its own commit before any message rewording.
- [Message audit scope creep] → Bounded by the rule in D5; style-only rewording is out of scope.

## Open Questions

- Whether v1.0.0 adopts `3` into its frozen table (recommended — the coordinator's input asks for
  a distinct code). If v1.0 declines, `WaveIncomplete` falls back to 1 and the table loses a row;
  no other part of this change depends on it.

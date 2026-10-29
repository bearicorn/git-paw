## Context

`src/cli.rs` declares `from_all_specs: bool` with `#[arg(long, alias = "from-specs", …)]`, so clap
parses both spellings into the same field and hides the alias from `--help`. `cli-parsing`'s
*--from-specs is a hidden alias* requirement pins that behaviour and schedules its removal for
v1.0.0. Since v0.24.0 (`error-exit-code-contract`) the alias warns: `src/deprecation.rs`
(`core-deprecation`, from v0.21.0) holds `{ item: "--from-specs", kind: flag, since: "0.24.0",
replacement: "--from-all-specs", removed_in: Some("1.0.0") }`, and its removal guard
(`no_entry_outlives_its_removal`) fails at crate version 1.0.0 while that entry exists. The tests at `src/cli.rs` (`start_with_from_specs_alias_*`,
the help test, several parse-table rows) and about 25 integration-test invocations still use the
alias. The generated default config (`config::generate_default_config`, an exported asset) says
"Bypass the CLI picker entirely for --from-specs mode", and the `default_spec_cli` doc comment and
`commands/start.rs` module doc name the alias too. User docs already use only `--from-all-specs`.

## Goals / Non-Goals

**Goals:**
- `--from-specs` stops working, loudly and helpfully, with the frozen "argument rejected" code.
- No surface — specs, docs, default config, help, completions, the deprecation registry —
  advertises the old spelling.

**Non-Goals:**
- Any change to what `--from-all-specs` does.
- Fixing stale function names in specs (`cmd_start_from_specs` vs the code's
  `cmd_start_with_specs`) — the rename here touches only the flag and the retired supervisor
  vocabulary, verbatim otherwise.
- A removed-flags framework beyond one flag → replacement table (shared with
  `remove-supervisor-vocabulary`).

## Decisions

### D1 — Reject with a clap usage error, not plain clap "unexpected argument"

Simply deleting the alias would make clap print `unexpected argument '--from-specs'` (exit 2) with
a "similar argument" tip at best. The requirement asks for an explicit migration message, so keep
a hidden `#[arg(long = "from-specs", hide = true)] removed_from_specs: bool` on `Start` and, in
`main` right after parsing, when it is set, emit
`Cli::command().error(ErrorKind::UnknownArgument, "--from-specs was renamed to --from-all-specs …")`
and `.exit()`. That goes through clap's own error path: same `error:` formatting and usage
footer, exit code 2, no git-paw code runs. Hidden args are skipped by `--help` and by
`clap_complete`, which the scenarios check. The message comes from a removed-flag table
(flag → replacement) that this change introduces with its one row; `remove-supervisor-vocabulary`,
which lands after it in v1.0.0, adds `--supervisor` → `--coordinated` and `--no-supervisor` →
`--bare`. The same change deletes the registry entry, so the `core-deprecation` scan no longer
warns about `--from-specs` and the removal guard passes.

*Alternatives:* (a) delete the alias and accept clap's generic message — fails the "clear
migration message" requirement; (b) map to a new `PawError` variant — would exit 1 and print after
git-paw starts doing work, contradicting the frozen meaning of 2.

### D2 — Hard error at 1.0.0 after one warning release

The deprecation policy (`cli-contract-freeze`) governs removals *after* 1.0. This alias predates
the policy and was documented as "removed in v1.0.0" since v0.5.0; it was silent until v0.24.0,
which registered it in `core-deprecation` so it warns for one minor release before this error. The
message names the fix, and the changelog carries a `BREAKING CHANGE:` footer.

### D3 — Rename sweep in the requirements that invoke the flag

Four `cli-parsing`, two `boot-block`, one `cli-resolution` and one `core-configuration`
requirement name `--from-specs` as an invocation (a fifth `cli-parsing` one, *Start subcommand
with optional flags*, is already restated with `--from-all-specs` by v0.21.0's
`supervisor-vocabulary-split`, so it is not modified here). Leaving them would make scenarios
describe a command that now fails (a phantom flag in the frozen spec). Each is re-stated verbatim
with only the flag changed; where the text meant "a spec-driven launch" generally, it says so and
names both `--from-all-specs` and `--specs`, which both route through
`interactive::resolve_cli_for_specs` today. The `boot-block` spec's `## Purpose` also mentions the
alias; Purpose is not a requirement, so it is edited by hand at archive.

The re-stated text outlives 1.0.0, so it also uses the coordinated-mode vocabulary of v0.21.0
instead of the spellings `remove-supervisor-vocabulary` removes: `--coordinated` / `--bare`,
`default_mode`, "bare mode", and "the coordinated launch path" for `cmd_supervisor` (v0.21.0's
own wording in *--unattended flag*). *Spec mode + supervisor mode dispatch* is RENAMED *Spec mode +
coordinated mode dispatch*. Because this change archives before `remove-supervisor-vocabulary`,
the RENAMED header matches the text on disk, and that change's verbatim sweep finds nothing left
to rewrite in these requirements.

One flagged reconciliation (spec-organization §5, spec lags tested code): *Non-TTY launch
handling* said the coordinated path "skips the foreground supervisor-CLI launch" on a non-TTY and
runs it on a TTY. The code never runs that CLI in the foreground (`commands/supervisor.rs` prints
the launch line, then the attach hint or the drive loop, with or without a terminal;
`tests/cli_supervisor_non_tty.rs` pins the non-TTY half), and after v0.18.0
`supervisor-pane-removal` there is no orchestrator CLI to launch. The requirement now says the
coordinated path never attaches and never runs an agent CLI in the foreground; the bare paths are
unchanged.

## Risks / Trade-offs

- [Scripts break on upgrade] → intended; exit 2 plus a one-line fix in the message; changelog.
- [v0.18.0 `launch-prompt-entry-point` (built on the archived v0.17.0 `start-spec-launch-parity`
  text) changes *Boot-block injection in cmd_start_from_specs* again before archive] → this delta
  copies that change's version; rebase it if it moves.
- [Another cycle edits a renamed requirement later] → that delta must carry the new spelling; the
  no-mention guard test catches regressions in docs and assets, not in specs.
- [`remove-supervisor-vocabulary` archives first by mistake] → its sweep would rename *Spec mode +
  supervisor mode dispatch* and this delta's RENAMED/MODIFIED headers would no longer match;
  both proposals state the order, and `remove-supervisor-vocabulary`'s first task checks it.

## Migration Plan

1. Hidden arg + removed-flag table + post-parse rejection; delete the registry entry; tests for
   the four scenarios.
2. Migrate test invocations; drop the alias and deprecation-warning tests; add the no-mention guard.
3. Fix the default-config comment and code comments; confirm docs (Deprecations section).
4. Commit with `!` and a `BREAKING CHANGE:` footer naming the replacement.

Rollback: restore the alias attribute and the registry entry.

## Open Questions

None. (The earlier question — warn before 1.0.0's hard error — was settled by v0.24.0's
`error-exit-code-contract`, which registers the alias in `core-deprecation`.)

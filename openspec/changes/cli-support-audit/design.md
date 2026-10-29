## Context

How much per-CLI support git-paw has today, grounded in this branch's code:

- **Detection.** `src/detect.rs` `KNOWN_CLIS` lists 17 binaries. `known_display_name` special-cases
  only `agy`.
- **Built-in per-CLI data.**
  - `src/config/prompt_profile.rs`: one compiled prompt profile, `claude_prompt_profile()`.
    `resolve_prompt_profile` falls back field by field to that profile for every other CLI.
  - `src/config/supervisor.rs`: `approval_flags` has rows for `claude`, `agy`, `codex`, and `qwen`,
    verified against upstream docs on 2026-07-15. `built_in_session_id_args` and
    `built_in_resume_args` have a `claude` row only.
  - `src/tmux/readiness.rs`: the first-run dialog markers (`DIALOG_MARKERS`) are compiled constants.
- **Detectors that bypass a CLI's profile.** Several detectors call `claude_prompt_profile()`
  directly: `is_live_prompt`, `classify_pane_readiness` (and `detect_prompt_shape`, which v0.17.0's
  `classifier-retirement` deletes). They ignore any
  per-CLI override. MILESTONE counts 16 such production sites, and v0.19.0 threads a `CliProfile`
  through them.
- **Doctor.** `src/doctor.rs` reports the CLIs group as a single "detected CLIs" line
  (`check_clis`). `--live` folds in `selftest`, which drives a dummy CLI (`cat`) and spawns no AI CLI,
  and `--json` already exists (`render_json`, `Report { status, checks }`).
- **Docs.** `docs/src/supported-clis.md` lists all 17 with a "Cold resume" column. The README carries
  a second table.
- **Tests.** v0.17.0's `tests/supported_clis_docs_sync.rs` checks that both docs tables carry
  exactly the known-CLI roster's binaries (`cursor-agent`, never `cursor`); nothing checks a tier or
  a version. No fixture directory exists.

This change builds on later layers:

- **v0.17.0.** `classifier-retirement`: git-paw no longer classifies or approves commands. Prompt
  detection lives in `src/supervisor/permission_prompt.rs` (`has_approval_marker`, the live-prompt
  gate, `extract_command_slice`, `extract_path_from_file_prompt`, the option-line helpers), the drive
  loop escalates every live prompt with `dedup_shape(capture)` as the command, and the only approval
  path is an explicit `sweep.sh approve`, which sends `1` + `Enter` after an option-1 guard that
  refuses a missing option 1 or one carrying the profile's `broad_grant_marker`.
  `cli-setup-consent`: `git paw setup-cli` and its per-CLI fact table, whose items carry a
  verification status (verified, or plan-only).
- **v0.18.0.** The terminal backend (`terminal-backend-seam`: `capture_tail`, `spawn_surface`,
  `kill`, `send_text` / `send_key`), pane-less sessions, and `headless_args`
  (`orchestrator-escalation-handoff`).
- **v0.19.0.** The per-CLI provider layer: a `CliProfile` Strategy with a `generic` Null Object, plus
  trust, allowlist, auto-mode, sandbox, hook, and cost data.
- **v0.20.0.** Per-CLI skill templates.

The audit itself, which runs each CLI through the harness and files findings, happens inside this
change's tasks. The findings do not: each one is its own change.

## Goals / Non-Goals

**Goals:**
- Define tiers that a pure function can derive from per-step evidence, so a tier cannot be claimed
  without proof.
- Make offline CI replay of recorded CLI output the main evidence, at an exact CLI version.
- Keep one source for the matrix, with the roster, doctor, docs, and README all checked against it.
- Ship no built-in per-CLI behaviour that lacks evidence.

**Non-Goals:**
- The per-CLI findings and fixes. Each is filed as a change; larger ones go to v0.24.0.
- An authentication preflight. That is post-1.0 per MILESTONE v0.19.0.
- Warning at launch about a launch-only worker (see Open Questions).
- Showing tiers in `git paw list-clis`.
- Freezing the matrix. v1.0.0 freezes the tier definitions and the first-class list.

## Decisions

### D1 — One change, not two

MILESTONE asks for "one change for the audit harness and support matrix". The tasks follow its
sequence: harness, then the per-CLI audit, then publication. The user-visible display and the
floor guard land last, so no intermediate commit shows an unaudited matrix.

*Alternative:* split it into `cli-conformance-suite` (harness, capture, live arm) and
`cli-support-tiers` (tiers, publication, roster rule), so the harness could be archived before the
audit finishes. That is rejected for now. The tier rules and the evidence format are defined
together, and the split would also need a `preflight-diagnostics` requirement added in one change
and modified in the next. It remains easy to split if the audit runs long.

### D2 — Tiers are derived, never declared

`derive_tier(&SupportRow) -> Option<Tier>` is a pure function over the twelve step statuses. The docs
page and doctor render the derived value.

*Alternative:* a hand-written tier column plus a consistency test. That is rejected because it is two
sources of truth. A derived tier also makes "lower the CLI's tier" (MILESTONE) mechanical: record a
`gap` and the tier follows.

### D3 — The matrix is a compiled Rust table; docs are parity-tested, not generated

The matrix is a `&'static [SupportRow]` in a new support module, placed beside v0.19.0's per-CLI
records; until then, next to `src/detect.rs`. Each row holds:

- `binary`
- `verified_version`
- `verified_on`
- `version_args` (default `["--version"]`)
- `steps: [StepStatus; 12]`, indexed by `LifecycleStep`

The fixed-size array makes the compiler reject a row that omits a step. `KNOWN_CLIS` becomes the
matrix's `binary` column, with names and order unchanged and `known_display_name` untouched, so
detection is byte-identical.

The docs page stays hand-written. A parity test reads it with `include_str!` and checks each row's
binary (in backticks), tier word, and version. This follows the repo's existing sync pattern
(`tests/specifications_page_lists_every_capability.rs`).

*Alternatives:*
- A TOML asset parsed at runtime. Rejected: it loses compile-time exhaustiveness and needs a
  parse-failure path.
- A generated Markdown fragment pulled in with `{{#include}}`. Rejected: it adds a generation step
  ahead of `mdbook build` and drops the hand-written gap prose next to each row.
- A matrix that lives only in the docs. Rejected: doctor needs it offline, and `tests/` is not
  packaged.

### D4 — The evidence bar: fixtures for what git-paw decides, attestations for what only a live run shows

The spec fixes which evidence each step needs. The code holds the same map once, as
`required_evidence(step)`. The fixtures replay through these decisions:

| Step | Fixture replays through | Attestation |
|---|---|---|
| `detection` | `parse_cli_version` (new) over the recorded banner | — |
| `launch` | token presence: every flag or subcommand the provider renders at each approval level (`resolve_approval_flags`, or its v0.19.0 successor) appears in the recorded help text | — |
| `readiness` | readiness reading (`classify_pane_readiness_with_profile`, the provider's start-up dialogs, which v0.19.0 moved out of `DIALOG_MARKERS`), including the dialog answer: option, dismiss key, or `blocking` | for a notice git-paw dismisses with a key, that the key dismissed it and readiness followed |
| `boot-block` | — | the boot block arrived and the agent registered (MCP or helper path, per the v0.20.0 template) |
| `prompt-shapes` | the `read_surface` approval-marker, live-prompt, escalation-command, mid-response, stuck, and CLI-unavailable fields | — |
| `approval` | the `read_surface` option-1 reading, and `sweep.sh approve`'s option-1 guard run against a stand-in terminal (must agree) | — |
| `send-submit` | buffered input (`input_box_text`, `pane_has_buffered_input`) | — |
| `native-modes` | token presence for the v0.19.0 auto-mode and sandbox flags | each approval level's flags behave as documented (a prompt-free level raised no prompt); hooks and native modes behave as configured |
| `resume` | token presence for `resolve_session_id_args` and `resolve_resume_args` | a resumed worker kept its context |
| `headless` | token presence for `headless_args` (v0.18.0) | a one-shot read stdin and exited |
| `exit` | the liveness rule over `process_name` (today the shell-name test in `has_live_agent_pane_with`, `SHELL_COMMAND_NAMES`) | exit and crash read as not alive (the live arm's output) |
| `usage` | v0.19.0's usage parser over the recorded artifact | — |

`boot-block` is attestation-only because delivery shows up at the broker (the agent's first
publish), not as a pure function of recorded output.

Since v0.17.0 retired the classifier, git-paw makes no decision about a prompted command. The
drive loop escalates every live prompt it recognises, so `prompt-shapes` is the step the escalation
guarantee rests on: a prompt form the profile does not recognise is never escalated. The `approval`
step shrinks to what the explicit `sweep.sh approve` relies on, "answers once": option `1` exists and
is not a durable grant. Its replay runs the helper's own guard, because the helper is the only
sender, and checks that it sends `1` + `Enter` exactly when the compiled reading is `once`. Whether
an approval level is prompt-free is now the CLI's own permission system's job, so `native-modes`
attests that the flags git-paw renders per level behave as documented; `launch` keeps checking only
that the tokens exist.

Token presence is a deliberately thin check. It proves the flag exists in that version, not that it
behaves, and the attestation covers behaviour. The step-to-decision column is the current mapping;
when v0.18.0 or v0.19.0 renames a function, the replay follows the new name, because the spec names
readings, not functions.

### D5 — Fixture format and location

The layout is `tests/fixtures/clis/<binary>/<version>/manifest.toml`, plus `.txt` case files. The
directory is not packaged, because `Cargo.toml`'s `include` already omits `tests/`. The manifest is
deserialised with `serde` and `toml`, both approved, using `deny_unknown_fields`. `format = 1` lets
the shape evolve.

The `expect` table is a partial `SurfaceReading` (D6) for `screen` cases, and a small per-step
record for `text` cases. Only the keys present are asserted, so one capture can pin just the facts
the reviewer vouched for.

The `{{WORKTREE_ROOT}}` and `{{HOME}}` placeholders make fixtures portable and private. Replay
substitutes `/tmp/paw-fixture/worktree` and `/tmp/paw-fixture/home`, so escalation commands and file
targets that quote a path read the same on every machine, and replay runs with no config.

### D6 — `read_surface`: one pure observation, composed from the existing detectors

The tests need a public, pure entry point. A per-detector harness would widen several private
`drive.rs` helpers one by one and would duplicate the composition the drive loop already does. So add

`read_surface(tail: &str, profile: &CliProfile, ctx: &ReplayContext) -> SurfaceReading`

It returns:

- readiness
- `dialog`: `none`, or the recognised start-up dialog with its answer (option, dismiss key, or
  `blocking`), from the same classification the launch gate uses
- `approval_marker` (`has_approval_marker`)
- `live_prompt`
- the escalation command (`dedup_shape`: the command slice, or the file target)
- `option_one` (`once`, `durable`, or `missing`)
- `mid_response`
- `buffered_input`
- stuck kind
- CLI-unavailable line

It composes the existing functions and adds no detection logic of its own, so it is
behaviour-preserving. The one reading with no compiled producer after v0.17.0 is `option_one`, which
only the shell guard in `sweep.sh approve` makes. It is a few lines over the moved option-line
helpers and the profile's `broad_grant_marker`, mirroring that guard. It is needed so
`__capture-fixture` can pre-fill an `approval` case, and the helper replay keeps the two identical.
It is also the "observe" half of the v0.18.0 observe → decide → apply split. If v0.18.0 or v0.19.0
has already introduced an equivalent observation type, use that instead.

*Alternative:* run the conformance suite as a `#[cfg(test)]` module inside the crate so it can reach
private functions. Rejected: it hides the contract the drive loop actually relies on, and it puts
filesystem fixture loading into unit tests.

### D7 — Known gaps fail by design; unreviewed captures fail CI

A `known_gap` case asserts the correct behaviour and is expected to fail. When a fix makes it pass,
the suite fails until someone clears the flag and re-records the step. Findings are therefore
recorded as executable tests, and v0.24.0 inherits a ready regression net.

`reviewed = false` fails the suite. This stops a pre-filled `expect`, which is git-paw's reading at
capture time and possibly wrong, from silently becoming the definition of correct.

### D8 — Capture is a hidden subcommand that reads the screen like the drive loop

`git paw __capture-fixture --agent <id> --case <name> --step <step> --out <dir> [--stdin]`.

- **Why a subcommand:** fidelity. Replay is only meaningful if the capture uses the same
  screen-read call, window, and join flags as the drive loop. Reusing the terminal backend's
  `capture_tail` guarantees that by construction.
- **Why `--out` is required:** export-agnosticism. A shipped binary must not assume git-paw's own
  repository layout.
- **Safety:** it never sends input. It refuses to overwrite a case, and it rejects case names outside
  `^[a-z0-9]+(-[a-z0-9]+)*$`, which blocks `..` traversal. It prints a reminder that captures get
  committed and must be scrubbed.
- `process_name` is copied from the live arm's report (D9), not captured by this command.

*Alternative:* a documented manual procedure using `tmux capture-pane -p -J -S …`. Rejected because
it drifts from the drive loop's capture parameters and does not survive a non-tmux backend.

### D9 — The live smoke: explicit names, model-free, git-paw's state isolated, the CLI's not

The command is `--live --cli <NAME>`, repeatable. Plain `--live` keeps its shipped contract, under
which `selftest` with `cat` spawns no AI CLI. A real CLI can cost quota, trigger an updater, or write
its own state, so the user names each CLI explicitly.

The smoke does not run `git paw start`. It opens a private terminal-backend instance and a throwaway
repository under `.git-paw/tmp/` directly, then:

- renders the CLI's launch command at `manual` approval, with no permission-bypass flags
- runs the launch-readiness gate with the CLI's resolved profile
- reads the foreground process through the liveness rule
- sends the terminal's interrupt, then end-of-input, a bounded number of times
- polls liveness until the CLI exits

It sends no boot block and no prompt, so no model turn is requested. Whether a CLI's own startup
calls the network is to be verified per CLI by the audit.

The CLI keeps the invoking user's environment. It needs its real authentication and config, so an
isolated `HOME` would produce false failures. Two consequences:

- The CLI may write its own state (history, a trust entry for the throwaway path). This is disclosed
  in `--help` and the docs.
- Answering a trust dialog leaves a stale entry for a deleted path, which is harmless.

How each check is scored:

| Check | Status | Reason |
|---|---|---|
| A running CLI reads as not alive | ✗ | It would let `purge --stale` treat a live session as stale. |
| An unresolved dialog | ✗ | The real launch fails loudly. |
| A non-generic profile does not recognise readiness | ⚠ | The real launch falls back and still injects, so it is not a blocker. It does signal version drift. |
| Readiness under the generic profile | ✓ (informational) | Nothing is expected to match. |

The JSON report adds the observed `process_name`, which is fixture input for the `exit` step.

*Rejected:*
- Folding the arm into plain `--live` for every detected CLI: it breaks the shipped contract and
  costs quota.
- An isolated `HOME`: it would show every CLI as not logged in.

### D10 — Static doctor shows tiers without spawning anything

The CLIs group becomes one check per detected CLI: `<binary>  <tier> · verified <version> (<date>)`.
These are informational ✓; an older or newer installed version is not something the user must fix.

Comparing installed and verified versions needs the CLI to run. The existing `--live` requirement
says static doctor "SHALL NOT spawn … an AI CLI", so that comparison lives only in the live arm.

The `--json` report gains an additive top-level `support` array. If v0.21.0's uniform `--json`
change lands first with a versioned doctor schema, the array follows its naming and version rules.

### D11 — The built-in data rule, and the generic profile

A parity test walks each built-in table and asserts that the row's matching step is not
`unverified`. The tables are:

- `approval_flags` → `launch`
- `built_in_session_id_args` and `built_in_resume_args` → `resume`
- `headless_args` → `headless`
- compiled profiles → `prompt-shapes`
- provider start-up dialogs (acceptance, trust, notice, blocking) → `readiness`
- the v0.19.0 provider's trust, mode, sandbox, and hook data, and usage parsing → the steps in
  the spec
- the `git paw setup-cli` fact rows (v0.17.0, carried by the v0.19.0 provider record) → the step of
  each declared item (allowlist and hooks → `native-modes`, trust → `readiness`)

The setup-cli fact table already records its own verification status, and an unverified item is
plan-only. Two records of the same fact must not disagree, so the parity test also requires an item
recorded verified to have its step `verified`, and an item whose step is `gap` or `unverified` to be
plan-only. A `gap` is enough to ship other data, but not to write into a user's CLI settings.

Today's unverified rows, such as `qwen --yolo`, therefore either gain evidence in the audit or are
removed. That is the "guessed profile misdetects confidently" principle already stated on
`supported-clis.md`.

A launch-only CLI resolves the `generic` profile. That depends on v0.19.0 replacing the Claude
default for unrecognised CLIs (see Open Questions).

The step rule alone does not stop someone from inventing a marker: a step can be `gap` while one of
its literals is a guess. So a second parity check walks every literal in a CLI's *own* profile and
dialog data and requires it to appear verbatim in one of that CLI's reviewed `screen` cases at the
row's version. A guessed string fails CI. The embedded default (Claude's profile and dialogs, which
every CLI without its own inherits) is outside this check. Claude's reference set (task 5.2) records
its shapes under the step rule like any other row's.

### D12 — Bounded version probe, std-only

The live arm runs `<binary> <version_args>` with a 5 s deadline, using `std::process::Child` with
`try_wait` polling and a kill on expiry. It adds no dependency.

`parse_cli_version` takes the first token that matches `\d+\.\d+(\.\d+)?([-+.][0-9A-Za-z.-]+)?` on
the banner's first non-empty line. The doctor's existing `parse_version` returns only (major, minor)
for git and tmux, which is not enough here.

### D13 — Start-up notices and opencode (GP-88)

The 2026-10-29 spike on opencode 2.1.2 found three things:

- opencode resolves Claude's profile, and none of its readiness, mid-response, or input-sigil
  markers match opencode's idle screen.
- On the first launch after an update, opencode shows a "what's new" notice that takes input until
  it is dismissed with Esc.
- Its free-usage limit is also shown as a dialog.

Today's gate would wait out its budget, fall back, and type the boot block into that notice.

- **Two new dialog kinds, both provider data.** A *notice* records a dismiss key. The gate sends
  the key once, then polls for readiness. A *blocking* dialog is never answered: the launch fails
  loudly, and the error says the CLI reports it is unavailable. Both follow v0.19.0 D5: they are
  compiled and cannot be overridden from a profile, because sending a key to a CLI is an action and
  should not be reachable from a worktree-writable file. A recognised notice or blocking dialog
  takes precedence over a readiness marker on the same screen, because a dialog drawn over the idle
  screen can leave its markers visible.
- **The dismiss key goes through the existing named-key operation.** The terminal port's `Key` gains
  `Escape`, and the gate sends it with `send_key`. The alternative was to widen v0.18.0's
  `answer_dialog(surface, option)` to take a key. That means a MODIFIED delta of the `code-architecture`
  port requirement, which v1.6.0 `terminal-port-surface-capabilities` also modifies. Adding a key
  variant changes no requirement text. Only the launch gate sends `Escape`: the drive loop's nudges
  and submits keep using `Enter` and `ClearLine`, because Esc interrupts a running turn in some CLIs.
- **The usage-limit text lives in two places, from one capture.** The blocking dialog stops a
  launch. The same text in opencode's `cli_unavailable_markers` lets the drive loop escalate
  `cli-unavailable` when the limit appears mid-session. The literal check covers both.
- **opencode gets a family.** v0.19.0's provider families are where built-in per-CLI data lives,
  and `generic` resolves the embedded Claude default. An `opencode` family carries only a profile
  and start-up dialogs; its other facets are the Null Object's. `[clis.<name>].family` accepts
  `opencode`, so a route wrapper (how the reporting user drives opencode) can select it.
- **No invented strings.** The spike saved no screen captures, only the observation. So the first
  opencode task captures the screens. It blocks every opencode datum, and a shape that cannot be
  captured at the row's version ships no data and records its step as `gap` or `unverified`. The
  free-usage limit needs a hosted free model whose allowance runs out. A local model has no such
  limit, so this capture may land late or not at all.

## Risks / Trade-offs

- **[CLIs ship weekly, so fixtures go stale quickly]** → A tier claims only its recorded version.
  The live arm shows the installed version against the verified one. Re-verifying means adding a new
  version directory; older ones keep replaying, which catches a git-paw regression against a version
  still in use.
- **[Captures leak secrets or personal paths]** → Placeholders, a mandatory review that includes a
  scrub check, and `reviewed = false` failing CI.
- **[First-class is out of reach for CLIs that genuinely lack a feature (no cost output, no hooks)]**
  → Accepted for now: honest gaps make the CLI supported. Revisit before v1.0.0 (Open Questions).
- **[The docs-table parity check is brittle]** → It checks stable anchors only: the backticked
  binary, the tier word, and the version string on one row. Prose is never checked.
- **[The audit drops CLIs users rely on]** → The pass-through for an explicit
  `default_cli`/`[clis.*]` is kept, and the changelog and docs record the route.
- **[v0.18.0, v0.19.0, or v0.20.0 land a different shape than assumed]** → The spec names readings
  and steps, not functions. D4 and D6 map them to whatever lands.
- **[A notice's dismiss key acts on the CLI]** → It is compiled provider data, never profile or
  config, and is sent at most once per attempt, only after a screen read that recognised the notice,
  and only by the launch gate. A blocking dialog gets no keystroke at all.
- **[opencode's screens cannot all be captured]** → An uncaptured shape ships nothing, and its step
  records `gap` or `unverified`. The tier shows the shortfall.
- **[Live smoke side effects]** → Model-free, explicit names only, `manual` approval, git-paw
  writes confined to `.git-paw/tmp/`, and the CLI-side writes disclosed.

## Migration Plan

1. Seam (behaviour-preserving): the support module and types, `KNOWN_CLIS` derived from it,
   `read_surface`, and `parse_cli_version`. Detection output is byte-identical.
2. Harness: the fixture loader, offline replay, the review and known-gap rules, and helper parity.
   Claude's fixtures are recorded first as the reference set.
3. Tooling: the capture subcommand and the `doctor --live --cli` arm.
4. Audit: each of the 17 CLIs is run through capture and the live arm. Rows are filled, and every
   finding is filed as its own change. opencode (GP-88) comes right after the Claude reference set,
   and its family, profile, and dialogs land here once its captures are reviewed.
5. Publication: doctor tier checks, the `support` JSON array, the docs page, the README, the parity
   and floor guards, and any removals the floor forces, with changelog entries.

Rollback: revert the change. No config or session format changes. A CLI removed in step 5 comes back
by restoring its row.

## Open Questions

- **The generic-profile fallback (gating).** The current `cli-prompt-profiles` requirement *An
  unrecognised CLI resolves a usable profile* resolves the embedded Claude default. This change
  assumes v0.19.0's `cli-provider-record` replaces that default with the `generic` Null Object, as
  MILESTONE v0.19.0 recommends. If v0.19.0 keeps the Claude default, add a MODIFIED delta of that
  requirement here before implementation, or drop the generic-profile clause.
  *Recommendation:* generic.
- **First-class strictness.** Should a step the CLI lacks by design (no hooks, no usage output) count
  as a gap, as it does now, or get a `not-applicable` status? *Recommendation:* keep the three
  statuses; revisit at v1.0.0 if the first-class list comes out empty.
- **Ending the CLI in the live arm.** Interrupt plus end-of-input may not exit every CLI; this is to
  be verified per CLI by the audit. *Recommendation:* add an optional per-row `exit_input` only if
  the audit shows a need.
- **CLIs the maintainers cannot obtain** (paid-only, region-locked). Under the floor rule, an
  unverifiable CLI is removed. *Recommendation:* remove it, since it can return through a finding
  change once verified.
- **To verify by the audit, not assumed here:**
  - whether each CLI supports `--version`
  - whether startup without a prompt consumes quota
  - each CLI's foreground process name (a shell-script wrapper would read as a shell)
  - whether `cursor-agent`, which v0.17.0 detects in place of the editor launcher `cursor` (DS-6), is
    Cursor's agent CLI on every install
  - each CLI's prompt, dialog, and stuck shapes
  - for opencode: whether Esc still dismisses the post-update notice at the audited version (Esc
    was observed at 2.1.2 and is re-attested per version), whether the notice can be brought up
    again without waiting for an update, and whether its free-usage limit can be captured at all
  - its native-mode, resume, and headless flags

## 1. Seam: support-matrix types and the detection roster (behaviour-preserving)

- [ ] 1.1 Add the support module beside the v0.19.0 per-CLI records (or `src/detect.rs` until they
      exist). It holds:
      - `LifecycleStep`: twelve variants with stable kebab ids and `ALL`
      - `StepStatus { Verified, Gap(&'static str), Unverified }`
      - `Tier { FirstClass, Supported, LaunchOnly }`
      - `SupportRow { binary, verified_version, verified_on, version_args, steps: [StepStatus; 12] }`
      - `SUPPORT_MATRIX`: one row per current `KNOWN_CLIS` binary, in the same order, every step
        `Unverified` until the audit fills it

      Every public item gets `///` docs, and the module gets `//!` docs.
- [ ] 1.2 Implement `derive_tier(&SupportRow) -> Option<Tier>` and `required_evidence(LifecycleStep)`
      (design D4). Test: the unit table *Tiers follow the step statuses*.
- [ ] 1.3 Derive `KNOWN_CLIS` from `SUPPORT_MATRIX`'s `binary` column. Keep `known_display_name`
      untouched. Existing `detect::tests` and `tests/cli_resolution_integration.rs` must pass unchanged.
      Test: *The detection roster is the support matrix* (same binaries, same order).
- [ ] 1.4 Implement `parse_cli_version(banner) -> Option<String>` (design D12). Test it with a table
      covering a bare version, a product-name prefix, a `v` prefix, a pre-release suffix, a
      multi-line banner, and no version.
- [ ] 1.5 Add a pure `read_surface(tail, profile, ctx) -> SurfaceReading` that composes the existing
      detectors (design D6) without changing any of them:
      - `classify_pane_readiness_with_profile`, and the `dialog` reading (`none`, or the recognised
        start-up dialog with its answer) from the provider's dialog data
        that the launch gate classifies with
      - `permission_prompt::has_approval_marker`
      - `live_prompt_markers_at_tail`
      - `drive::dedup_shape` (the escalation command: command slice or file target)
      - the `option_one` reading (`once` / `durable` / `missing`) over the moved option-line helpers
        and the profile's `broad_grant_marker`, mirroring `sweep.sh approve`'s option-1 guard
      - the `drive.rs` helpers `input_box_text`, `pane_has_buffered_input`, `cli_unavailable_line`,
        and the stuck-shape fields

      Test: a characterization table. For each capture literal the existing detector tests already
      use, `read_surface` must agree with the individual detectors. For `option_one`, use the
      captures of `tests/sweep_sh_approve_options.rs` (three-option prompt → `once`; option 1 carrying
      the broad-grant marker → `durable`; no numbered option → `missing`).
- [ ] 1.6 Extract the pure liveness rule from `has_live_agent_pane_with`: a foreground process name
      outside `SHELL_COMMAND_NAMES` reads as a live agent. Behaviour must not change. Test: a table of
      process names.
- [ ] 1.7 Gate: `just check` passes, and detection output is byte-identical (no test edited to pass).

## 2. Conformance fixtures and offline replay

- [ ] 2.1 In `tests/helpers/conformance.rs`, add the fixture manifest types (`serde`/`toml`,
      `deny_unknown_fields`, `format = 1`) and a loader for `tests/fixtures/clis/<binary>/<version>/`.
      The loader checks that `cli` and `version` match the directory, that every named file exists,
      that every file is named by the manifest, and that every step id is valid.
- [ ] 2.2 Tests on tempdir fixture trees: *A well-formed fixture set loads*, *An unknown step id is
      rejected*, *An orphan case file is rejected*.
- [ ] 2.3 Replay:
      - substitute the `{{WORKTREE_ROOT}}`/`{{HOME}}` placeholders with the fixed synthetic paths
      - read a `screen` case with `read_surface` under the CLI's no-config resolved profile
      - read a `text` case with its step's decision: version parse, rendered-token presence
        (`resolve_approval_flags`, `resolve_session_id_args`, `resolve_resume_args`, `headless_args`,
        native-mode flags), or usage parse
      - read `process_name` with the liveness rule
      - assert only the keys present in `expect`

      A failure names the fixture, key, expected value, and actual value. Tests: *A matching case
      passes*, *A mismatch names the fixture, key, and values*.
- [ ] 2.4 Helper parity: replay each stuck-shape case through the bundled helper's stuck evaluation
      (`sweep.sh stuck-eval` today, at its v0.20.0 package path once moved), with the capture on
      stdin, and compare readings. Test: *The bundled helper must agree*, using a synthetic
      disagreement fed to the comparison.
- [ ] 2.5 Approve-guard parity: replay each `approval` case through `sweep.sh approve` (its v0.20.0
      package path once moved) against a stand-in terminal on `PATH` that serves the capture to
      `capture-pane` and records `send-keys`. The guard must record `1` then `Enter` exactly when the
      compiled `option_one` reading is `once`, and nothing otherwise. Test: *The approve guard must
      agree on option 1*.
- [ ] 2.6 Review and known-gap rules. An unreviewed case fails the suite. A `known_gap` case must
      fail its expectation, and an unexpected pass fails the suite with a "clear `known_gap`" message.
      Tests: *An unreviewed case fails the suite*, *A known gap that starts passing fails the suite*.
- [ ] 2.7 `tests/cli_conformance.rs` walks every fixture directory and replays all cases in-process.
      It runs in the standard CI job, which has no agent CLI installed and needs no network. This is
      the test for *Replay needs no CLI, multiplexer, or network*.

## 3. Fixture capture subcommand

- [ ] 3.1 Add the hidden `CaptureFixture` subcommand to `src/cli.rs`, with `about`, `long_about`,
      and a help string per flag. Flags: `--agent`, `--case`, `--step`, required `--out`, `--stdin`,
      and `--cli-version` for a new manifest. Add `cmd_capture_fixture` to the command library. It:
      - resolves the agent's surface through the terminal backend
      - reads the screen with the drive loop's capture parameters
      - substitutes placeholders
      - pre-fills `expect` from `read_surface` under the agent's resolved profile
      - writes the case and its manifest entry with `reviewed = false`
      - refuses to overwrite a case and rejects non-kebab-case names
      - prints a reminder to scrub the capture
      - never sends input
- [ ] 3.2 E2E, socket-isolated, `#[serial]`, with a tmux skip guard: a stub CLI prints a fake
      permission prompt that quotes its worktree path. Tests:
      - *A screen capture is written unreviewed with placeholders*: the surface content is unchanged
        and the surface received no input
      - *An existing case is not overwritten*
      - *A traversing case name is rejected*

      Also add a hidden-command parse test.
- [ ] 3.3 Run the security and safety review (`security-and-safety-review` skill): no input sent,
      case-name and path validation, explicit `--out`, and the scrub reminder. Run the
      export-agnosticism check: no git-paw repository layout is baked into the binary.

## 4. Per-CLI live smoke arm (`doctor --live --cli`)

- [ ] 4.1 Add `Doctor { cli: Vec<String> }`, repeatable, with `requires = "live"`. Update
      `long_about` with the new example and the disclosure that the CLI under test uses the invoking
      user's environment and may write its own state. Test: *`--cli` requires `--live`*.
- [ ] 4.2 Probe layer: `probe_live_cli`. It:
      - resolves the binary
      - runs the bounded version probe (5 s)
      - opens a private terminal-backend instance and a throwaway repository under `.git-paw/tmp/`
      - launches the CLI at `manual` with no boot block and no prompt
      - runs the readiness gate with the resolved profile
      - reads liveness and the foreground `process_name`
      - sends interrupt and end-of-input to end the CLI, then polls liveness
      - cleans up on every path

      It returns a plain-data `LiveCliProbe`.
- [ ] 4.3 Pure `check_live_cli(&LiveCliProbe, Option<&SupportRow>) -> Vec<CheckResult>` with the
      ✗/⚠/✓ mapping from the spec. Test: a unit table over probe outcomes, including generic-profile
      readiness, a missing CLI, an unresolved dialog, a running CLI read as not alive, and an exit not
      observed. Add the installed and verified versions and `process_name` to the JSON detail.
- [ ] 4.4 E2E with stub CLIs configured as `[clis.stub]`, socket-isolated and `#[serial]`. Tests:
      - *A CLI that reaches readiness passes*, including the cleanup assertion
      - *A named CLI that is not installed fails*, exiting non-zero
      - *An unresolvable first-run dialog fails*
      - *Readiness under the generic profile is informational*
      - *`--json --live --cli` stays parseable*
      - *Plain `--live` still spawns no AI CLI*, using a stub that records each execution
- [ ] 4.5 Run the security and safety review of the live arm: `manual` approval, no prompt, explicit
      names only, git-paw writes confined to `.git-paw/tmp/`, and the disclosure present in help and
      docs.

## 5. The audit

- [ ] 5.1 Write the audit procedure in `docs/src/contributing.md` under "Adding or re-verifying a
      CLI". It covers:
      - installing the CLI and running `doctor --live --cli`
      - capturing each shape the step map (design D4) needs
      - the review checklist: the correct expectation (not just git-paw's reading), and a scrub for
        secrets and personal data
      - writing attestations, including the `native-modes` attestation that each approval level's
        flags behave as documented (a prompt-free level raised no permission prompt)
      - recording the row's version, date, and statuses
      - recording findings as `known_gap` cases and filing each finding as its own change
- [ ] 5.2 Reference set: audit `claude` first, end to end. Commit its fixture set and attestations,
      and fill its row. From now on, `tests/cli_conformance.rs` asserts at least one fixture set
      exists.
- [ ] 5.3 Start-up notices in the launch gate (design D13). This task needs no opencode data:
      - extend the provider dialog data (v0.19.0 `cli-profile-threading` D5) with a *notice* kind
        (recognition text and dismiss key) and a *blocking* kind (recognition text), compiled and
        not profile-readable
      - add `Key::Escape` to the terminal port, with the tmux mapping and the fake backend
      - in the gate, classify a recognised notice or blocking dialog ahead of readiness markers;
        send a notice's key at most once per attempt, then poll; treat a notice that is still
        showing, and any blocking dialog, as an unresolved dialog; quote the recognised line in the
        blocking error
      - only the gate sends `Escape`; add a source audit that no drive-loop, submit, or `__send`
        path sends it
      - extend `read_surface`'s `dialog` reading (1.5) with the two kinds

      Tests (fake backend, synthetic provider data): *A notice is dismissed and the boot block
      follows*, *A notice that does not clear fails the launch*, *A notice drawn over the ready screen
      still comes first*, *A blocking dialog is never answered*, *Another CLI's notice text is not
      acted on*. Existing gate tests pass unchanged.
- [ ] 5.4 **Blocking for every opencode datum: capture opencode's screens (GP-88).** The spike saved
      no captures, so nothing for opencode is written from memory, release notes, or the spike's
      prose. On a real install at the version being audited, using `doctor --live --cli opencode` and
      `__capture-fixture`, capture:
      - the idle (ready) screen
      - the input box holding unsubmitted text, and the same box after submit
      - a working (mid-response) screen
      - the notice shown on the first launch after an install or update
      - any first-run or trust dialog, and each permission-prompt form shown at `manual`
      - the free-usage-limit notice, if a hosted free model's allowance can be exhausted

      Write a `readiness` attestation recording the key that dismissed the notice and that the CLI
      then reached readiness. Review and scrub every case. Record each shape that could not be
      captured, so 5.5 records its step as `gap` or `unverified` instead of shipping data for it.
- [ ] 5.5 opencode family and data, from 5.4's reviewed captures only:
      - add the `opencode` family (`CliFamily::Opencode`, its provider, and `opencode` in the
        `family` parser and its error's accepted-values list)
      - give it a prompt-shape profile (readiness markers, input sigil, mid-response markers, and
        `cli_unavailable_markers` only if the limit notice was captured), copying each literal from a
        capture
      - give it a notice for the post-update screen with the attested key, and a blocking dialog
        for the limit notice only if that notice was captured
      - fill opencode's matrix row

      Tests: *opencode's idle screen reads as ready under its own profile*, *The post-update notice
      reads as a dismiss-key dialog*, *The usage-limit notice reads as blocking and unavailable* (or
      *An uncaptured shape ships no data* if the notice was not captured), *A wrapper entry selects
      the opencode family*.
- [ ] 5.6 Audit each remaining matrix binary with the same procedure. Record its version, date,
      statuses, fixtures, and attestations. File each finding as a separate change: small ones are
      fixed this cycle, and larger ones go to v0.24.0 or are recorded as `gap`. Do not fix findings
      inside this change (opencode, 5.4 and 5.5, is the one exception; see the proposal).
- [ ] 5.7 Built-in data rule. Each built-in datum whose step is still `unverified` either gains
      evidence or is removed in a finding change. The datums are: `approval_flags` rows,
      `built_in_session_id_args`/`built_in_resume_args`, `headless_args`, compiled profiles, the
      v0.19.0 provider data, and the `git paw setup-cli` fact rows. A setup-cli item recorded verified
      whose step is not `verified` is set back to plan-only (unverified) in a finding change.

## 6. Publication and guards

- [ ] 6.1 Write the evidence and floor checks in `tests/helpers/conformance.rs` as functions over
      `(&[SupportRow], fixture root)`:
      - a gap note must not be empty
      - a `verified` step must have its `required_evidence` in the row's version directory
      - a step with a `known_gap` case must not be `verified`
      - the recorded version directory must exist
      - every row must be at least launch-only
      - a built-in datum's step must not be `unverified`
      - a setup-cli item recorded verified must have its step `verified`, and an item whose step is
        `gap` or `unverified` must be plan-only
      - every marker literal in a CLI's own built-in profile or start-up dialog data must appear
        verbatim in one of that CLI's reviewed `screen` cases at the row's version

      In `tests/cli_support_matrix_parity.rs`, run them on `SUPPORT_MATRIX` and on synthetic
      matrices and trees. Tests:
      - *Every row states every step*
      - *A gap without a note is rejected*
      - *A verified step without fixtures fails the suite*
      - *A live-only part needs an attestation*
      - *A known gap contradicts a verified status*
      - *The recorded version must have a fixture set*
      - *An older version keeps replaying*
      - *A row below the floor fails the suite*
      - *An unverified step cannot carry built-in data*
      - *A verified setup item needs a verified step*
      - *A built-in marker absent from every capture fails*
- [ ] 6.2 Remove every CLI the floor rule forces out, taking out both the row and the roster entry.
      Record each removal, with its reason and the `[clis.<name>]` route, for the changelog and the
      Supported AI CLIs page. Test: *A removed CLI still resolves when named explicitly*.
- [ ] 6.3 Test *A launch-only CLI resolves the generic profile*. This is gated on v0.19.0's generic
      fallback (design Open Questions); if that fallback is missing, add the `cli-prompt-profiles`
      MODIFIED delta first.
- [ ] 6.4 Static doctor:
      - `check_clis` emits one informational check per detected CLI from the matrix
      - a custom CLI with no row is labelled as outside the matrix
      - a custom override of a matrix binary is marked as applying to the built-in launch
      - `Report` gains the additive `support` array, following v0.21.0's schema conventions if that
        change has landed

      Tests:
      - *A detected matrix CLI shows its tier and version*
      - *A custom CLI outside the matrix is labelled*
      - *The JSON report carries the support array*
      - *Tier reporting does not run the CLI*, using a stub that records each execution
- [ ] 6.5 Rewrite `docs/src/supported-clis.md` with:
      - the tier definitions and the step list
      - the matrix table: CLI, binary, tier, verified version, verified on
      - each gap note, folding the per-step resume status into the matrix in place of the old "Cold
        resume" column
      - the evidence summary
      - removed CLIs

      Add parity tests against the real page and against synthetic page text: *A wrong tier on the
      docs page fails*, *A binary missing from either side fails*.
- [ ] 6.6 README Supported AI CLIs section: either a table with a tier column, or a link if v0.21.0
      has slimmed the README. Tests: *A README table matching the matrix passes*, *A README naming a
      binary outside the matrix fails*, *A README that links instead of listing passes*. Point the
      existing `tests/supported_clis_docs_sync.rs` at the matrix (skipping the README side when it
      links instead): *README and mdBook CLI tables list every matrix binary and only those*.

## 7. Documentation

- [ ] 7.1 Update `--help` for `doctor` (`--cli`, the disclosure, an example) and the hidden
      `__capture-fixture` long help.
- [ ] 7.2 Update `docs/src/cli-reference.md` (the doctor section) and
      `docs/src/user-guide/doctor.md` (the tier lines, the per-CLI live arm, and how to read ✗/⚠/✓).
- [ ] 7.3 Update `docs/src/user-guide/cli-prompt-profiles.md`: built-in profiles ship only with
      evidence, and their marker literals come from captures; unrecognised CLIs fall back to the
      generic profile; opencode has its own profile; and a link to the matrix. Describe the launch
      gate's notice and blocking dialogs where the docs describe first-run and trust dialogs.
- [ ] 7.4 Update `docs/src/contributing.md` (the fixture format reference next to the 5.1 procedure)
      and `docs/src/architecture.md` (the support module and `read_surface`).
- [ ] 7.5 Add a README feature line for the support matrix. In the configuration reference, add
      `opencode` to the accepted values of `[clis.<name>].family`, with a wrapper example. There are
      no new config fields.
- [ ] 7.6 Check that `mdbook build docs/` passes.

## 8. Spec bookkeeping and quality gates

- [ ] 8.1 Add `Test:` pointers under every scenario in the five delta specs. Before archive,
      re-diff the `cli-provider` delta against v0.19.0 `cli-provider-record`'s archived text.
- [ ] 8.2 After archive, backfill `## Purpose` for `cli-support-matrix` (the text in the proposal)
      and add it to `docs/src/specifications/README.md`. `tests/spec_purpose_backfilled.rs` and
      `tests/specifications_page_lists_every_capability.rs` must pass.
- [ ] 8.3 Run `just check`, `just deny`, and `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`,
      verifying each by its real exit code. Coverage must be ≥ 80% on the new logic.
- [ ] 8.4 Run the five-gate self-check (`definition-of-done` skill): spec ↔ code, every scenario ↔
      test, all four doc layers consistent, security and safety (tasks 3.3, 4.5, and 5.3's
      gate-only `Escape`), and export-agnosticism.

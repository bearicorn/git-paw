## Why

`KNOWN_CLIS` (`src/detect.rs`) auto-detects 17 binaries by name, but git-paw ships per-CLI behaviour
for very few of them: one built-in prompt-shape profile (`claude_prompt_profile`), one session-id /
resume row (`claude`), and approval-flag rows for four CLIs. Every other CLI is driven on Claude's
prompt shapes unless the user overrides them, and nothing records which lifecycle steps actually work
for which CLI at which version. v1.0.0 will freeze which CLIs git-paw supports; before that, each
detected CLI has to be checked end to end on the v0.19.0 per-CLI provider layer, and the result
published with evidence that CI keeps honest.

## What Changes

- **Worker-lifecycle vocabulary.** Twelve named steps (detection, launch, readiness, boot block,
  prompt shapes, approval, send/submit, native modes, resume, headless one-shot, exit, usage). Each
  CLI records a status per step: verified, gap (with a note), or unverified. Since v0.17.0 git-paw
  approves nothing on its own, so `prompt-shapes` (every live prompt is recognised and so escalated)
  carries the escalation guarantee. `approval` checks only that an explicit `sweep.sh approve`
  answers once: option 1 is present and is not a durable grant. `native-modes` attests that each
  approval level's flags behave as documented.
- **Support tiers derived from step statuses.** first-class (every step verified), supported
  (detection and launch verified, every other step verified or a documented gap), launch-only
  (detection and launch verified). A tier is computed by a pure function, never typed in by hand.
- **Evidence bar.** A step counts as verified only with evidence at the row's recorded CLI version:
  reviewed fixtures replayed in CI for every part of the step that git-paw decides from recorded CLI
  output, and a dated attestation for the parts that can only be observed live.
- **Per-CLI conformance suite.** Recorded fixtures per CLI and version under
  `tests/fixtures/clis/<binary>/<version>/` (a `manifest.toml` plus captured text). CI replays them
  offline, with no network, tmux, or CLI binary, through git-paw's own detectors and the bundled
  helper where it reads the same profile field (the stuck shapes, and `sweep.sh approve`'s option-1
  guard against a stand-in terminal). Unreviewed fixtures fail the suite. Known-gap fixtures must
  keep failing until the gap is fixed.
- **Fixture capture.** A hidden maintainer subcommand, `git paw __capture-fixture`, captures an
  agent's surface through the same path the drive loop uses. It replaces machine-local paths with
  placeholders and writes the case unreviewed, with git-paw's current reading pre-filled. It never
  sends input.
- **Opt-in live smoke per CLI.** `git paw doctor --live --cli <NAME>` launches each named installed
  CLI in a throwaway repository on a private terminal-backend instance. It runs at `manual` approval,
  with no boot block and no prompt, and reports detection (installed version), launch, readiness,
  liveness and exit. Plain `--live` is unchanged.
- **Published matrix.** One compiled table is the single source. `git paw doctor` (human and
  `--json`) shows each detected CLI's tier and verified version. The supported-CLIs page and the
  README are checked against the table by parity tests.
- **Roster rule.** The detection roster is the matrix. Every row must reach at least launch-only. A
  CLI that cannot is removed from detection, but still works through `[clis.<name>]`.
  **BREAKING (for users who rely on auto-detecting a dropped CLI)**: an explicit config entry or a
  saved session that names the dropped CLI still resolves.
- **Built-in data needs evidence.** A built-in per-CLI datum (prompt profile, approval flags,
  session-id/resume or headless templates, native-mode flags, `git paw setup-cli` fact rows) ships
  only for a step the CLI's row does not record as unverified. A setup-cli item is verified only when
  its step is verified, and plan-only otherwise. A launch-only CLI resolves the generic profile.
  Every marker literal in a CLI's own built-in profile or start-up dialog data must appear verbatim
  in one of its reviewed captures.
- **Start-up notices in the launch gate.** Besides acceptance and trust dialogs, a provider can
  declare a *notice* (dismissed once with a key it records, then readiness is polled) and a
  *blocking* dialog (never answered; the launch fails loudly saying the CLI reports it is
  unavailable). Both are compiled provider data and take precedence over a readiness marker on the
  same screen. The `readiness` step covers them.
- **opencode, recorded before the audit (GP-88).** A 2026-10-29 spike on opencode 2.1.2 found that
  opencode resolves Claude's profile, and its idle screen has none of Claude's readiness or
  mid-response markers or input sigils. On its first launch after an update it shows a "what's new"
  notice that takes input until dismissed with Esc, and its free-usage limit is also shown as a
  dialog. opencode gets its own provider family, with a profile and start-up dialogs built only from
  reviewed captures. The spike saved no screen captures, so capturing them is the first, blocking
  task. `[clis.<name>].family` accepts `opencode`, so a wrapper entry can select it.

Out of scope: the per-CLI findings and fixes the audit produces. They are filed as separate changes
as they come; small ones land this cycle, larger ones go to v0.24.0 or lower the CLI's tier. The one
exception is opencode (GP-88). It was found before the audit started, and its fix consists entirely
of the data and gate handling that this change's rules govern.

## Capabilities

### New Capabilities
- `cli-support-matrix`: the per-CLI support contract. It covers the worker-lifecycle steps and step
  statuses, tier definitions and derivation, the evidence bar, recorded versions, the conformance
  fixture format, offline replay, review, and capture, the compiled matrix and its docs parity, the
  roster floor and drop rule, and the evidence rule for built-in per-CLI data. Purpose (to backfill
  on archive): *Define which agent CLIs git-paw supports and how well, as tiers derived from
  per-lifecycle-step verification at a recorded CLI version, backed by conformance fixtures that CI
  replays offline, and published from one compiled table to `git paw doctor` and the docs.*

### Modified Capabilities
- `cli-resolution`: *Auto-detect known AI CLIs on PATH* now scans the support matrix's rows instead
  of an enumerated list. *README Supported AI CLIs table matches `src/detect.rs`* is rewritten
  against the matrix. It keeps v0.17.0's README/mdBook binary-set parity test, adds a tier per README
  row, and allows a link in place of the README table.
- `preflight-diagnostics`: two ADDED requirements. Doctor reports each detected CLI's support tier,
  and the opt-in per-CLI live smoke arm `--live --cli`.
- `tmux-orchestration`: one ADDED requirement. The launch gate dismisses a provider-declared notice
  and stops at a provider-declared blocking dialog.
- `cli-provider`: *A custom CLI declares its provider family* accepts `opencode`. The delta is
  built on v0.19.0 `cli-provider-record`'s text, which must archive first.

## Impact

- **Code:** new matrix module (rows, `LifecycleStep`, `StepStatus`, `Tier`, `derive_tier`,
  `parse_cli_version`), placed beside v0.19.0's per-CLI provider records. `src/detect.rs`
  (`KNOWN_CLIS` derived from the matrix, with names and order unchanged). A pure `read_surface`
  observation over a captured tail and a resolved profile, composed from the existing detectors
  (`classify_pane_readiness_with_profile`, `permission_prompt::has_approval_marker`,
  `live_prompt_markers_at_tail`, `drive::dedup_shape`, the moved option-line helpers with the
  profile's `broad_grant_marker` for the option-1 reading, and the private `drive.rs` helpers
  `input_box_text`, `pane_has_buffered_input`, `cli_unavailable_line`). `src/doctor.rs` (`check_clis`, a new live-CLI
  probe and check, the `Report` JSON). `src/cli.rs` (`Doctor { cli }`, hidden `CaptureFixture`).
  `src/main.rs` / the command library (`cmd_capture_fixture`).
- **Tests:** `tests/fixtures/clis/**` (not packaged: `Cargo.toml` `include` already omits `tests/`),
  `tests/cli_conformance.rs`, `tests/cli_support_matrix_parity.rs`, a doctor live-CLI e2e with a stub
  CLI, and a capture-command e2e.
- **Docs:** `docs/src/supported-clis.md` (tiers, matrix, gaps), `docs/src/user-guide/doctor.md`,
  `docs/src/cli-reference.md`, `docs/src/contributing.md` (adding or re-verifying a CLI),
  `docs/src/user-guide/cli-prompt-profiles.md`, README, `docs/src/architecture.md`.
- **Dependencies:** none new (`toml` and `serde` are approved; the bounded version probe uses
  `std::process`).
- **Enum ripple:** `BrokerMessage` and `SpecBackendKind` are untouched. The new enums are fresh.
  `CliFamily` gains `Opencode` (its match sites are `CliFamily::provider()` and the `family`
  parser), and the terminal `Key` gains `Escape` (each backend's key mapping and the fake backend).
- **Config:** no new fields. `[clis.<name>].family` accepts one more value, `opencode`. Doctor
  `--json` gains one additive top-level array.
- **Cross-version dependencies:**
  - v0.17.0 changes (archived) that the fixtures replay through: `cli-unavailable-detection-gaps` (new
    `cli_unavailable_markers`), `live-prompt-tail-window` (the live-prompt window),
    `idle-pane-nudge-submission` (buffered-input submit). Also builds on the shipped
    `worker-session-id-capture` (resume templates).
  - v0.17.0 `classifier-retirement`: the classifier, `__classify`, `PermissionType`,
    `detect_prompt_shape` and `select_option_index` are gone; detection lives in
    `permission_prompt.rs`, and `sweep.sh approve`'s option-1 guard is the `approval` step's
    subject. v0.17.0 `cli-setup-consent`: the setup-cli fact table whose verification status the
    built-in data rule checks.
  - v0.19.0 `cli-profile-threading` threads the resolved profile into the surviving detectors and the
    approve guard; replay reads them under the fixture CLI's profile.
  - v0.18.0: `terminal-backend-seam` (`SurfaceId`, capture-tail, spawn, liveness; used by the live
    arm and the capture command), `supervisor-pane-removal` (agents are addressed by agent, not
    pane), `orchestrator-escalation-handoff` (`headless_args`, the headless step),
    `cmd-library-surface` (where `cmd_capture_fixture` lives).
  - opencode (GP-88): v0.19.0 `cli-profile-threading` moves first-run dialogs into provider data
    (its D5), and this change adds the notice and blocking kinds there. v0.19.0
    `cli-provider-record` defines the family set that gains `opencode`. The `CliFamily` enum's only
    exhaustive match is `CliFamily::provider()`. The terminal port's `Key` gains `Escape` for a
    notice's dismiss key. Adding a key does not modify the port requirement, which v1.6.0
    `terminal-port-surface-capabilities` modifies.
  - v0.19.0 `cli-provider-record` (capability `cli-provider`): the `CliProfile` Strategy with a `generic` Null Object, provider data
    (trust, allowlist, auto-mode, sandbox, hooks), cost attribution, and DS-6 identity. These are the
    decisions the steps verify.
  - v0.20.0 `skills-packaging`: per-CLI skill templates (the boot-block step) and the helper's new
    location (helper parity).
  - v0.21.0: uniform `--json` (the doctor array follows its schema conventions if that change lands
    first) and the README landing-page slim-down (the README requirement allows a link instead of a
    table).
  - Consumed by v0.24.0 (fixes the findings; known-gap fixtures become its regression tests) and
    v1.0.0 (freezes the tier definitions and the first-class list).
  - MODIFIED requirements: both `cli-resolution` deltas are built on the archived v0.17.0 text, where
    `cli-launch-identity` replaced `cursor` with `cursor-agent` in the roster and the docs tables (DS-6).
    v0.18.0's `orchestrator-escalation-handoff` adds an unrelated `cli-resolution` requirement. v0.19.0's
    `cli-provider-record` modifies neither requirement.

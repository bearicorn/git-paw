## Why

v1.0.0 promises that an upgrade never strands existing state. Today that promise rests on
`#[serde(default)]` discipline and a handful of hand-written unit fixtures for single fields
(`session.rs::v020_session_json_loads_with_broker_fields_as_none`, `repo_session_file_without_orchestrator_loads`).
Nothing loads a file that a *released binary* actually wrote, and nothing checks that a session
recorded by an older release can still be resumed, stopped and purged. Between v0.16.0 and the
freeze, v0.18 changes the pane layout (`dashboard_pane`, `pane_index` offsets, the orchestrator
entry), v0.21 adds version fields and wire DTOs, and v0.22 may start reading `broker.log`. Each is
a chance to break a user who upgrades mid-session.

## What Changes

- A committed **fixture matrix**: for every release from v0.16.0 on, the files that release wrote
  in a scripted session — repo `config.toml`, global `config.toml`, the global session file, the
  per-repo discovery file, `broker.log`, and the manual-approvals JSONL — captured from the
  released binary, with machine-specific paths and timestamps replaced by placeholders.
- A matrix test that, for every row, loads each file under the current build and drives the
  operations a user runs after upgrading: `status`, restart/recovery via `start`, `stop`, `purge`,
  `approvals`, and a broker appending to the old log. Keys the current build documents as
  deprecated with no effect (`[supervisor.auto_approve]`, `[supervisor.common_dev_allowlist]`
  since v0.17.0) must load but are not counted as lost values.
- From v0.17.0 the capture script writes the manual-approvals log through an `--unattended` run
  whose stub agent shows a live permission prompt, because the drive loop's escalation is the only
  writer of that log since the classifier's retirement.
- A completeness guard: every release at or after v0.16.0 listed in `CHANGELOG.md` must have a
  fixture row, so a release cannot ship without adding its own.
- The v0.16.0, v0.17.0 and v0.18.0 rows can be captured now: all three releases are on crates.io,
  and the completeness guard requires their rows. Rows for v0.19.0 through v0.23.0 are added by
  each release as it ships; the release-candidate gate runs the full matrix.

No product behaviour changes. Where the matrix finds an incompatibility, the fix is filed as its
own change.

## Capabilities

### New Capabilities

- `core-upgrade-compatibility`: persisted files written by any release since v0.16.0 load and
  resume under the current build, proven by a committed fixture matrix that grows with every
  release.

### Modified Capabilities

<!-- none -->

## Impact

- **Tests:** new `tests/upgrade_compat_matrix.rs`; fixtures under `tests/fixtures/compat/<version>/`
  with a `README.md` recording how each row was captured; a capture script under
  `tests/fixtures/compat/capture.sh` (test tooling, not bundled).
- **Production code:** none.
- **Docs:** `CONTRIBUTING.md` / release section of `AGENTS.md` gain "capture the compat fixture
  row before tagging"; `docs/src/contributing.md` mirrors it. No user-facing surface.
- **Enum-variant ripple:** none. The matrix observes `BrokerMessage` only through the log text.
- **Dependencies:** none.
- **Security:** fixtures are captured on a throwaway repo; the capture script replaces home
  directories, usernames, absolute paths and timestamps with placeholders, and the README
  forbids committing a real config.
- **Cross-version dependencies:**
  - **v0.21.0 `config-schema-versioning` and `wire-format-dtos` (as drafted):** pre-v0.21 files
    carry no version and must read as version 1. The matrix is where that rule is exercised
    against real old files; its assertions become "unversioned → v1 → migrated to current" once
    those land. If v0.21 creates a format-versioning capability, fold this capability into it at
    archive time.
  - **v0.18.0 `supervisor-pane-removal`** (agent offset derived from the recorded
    `dashboard_pane`, `RepoSessionFile.orchestrator` still deserializes) and
    **`broker-host-separation`** (pre-relocation sessions fall back to the dashboard-pane pause):
    the v0.16.0 and v0.17.0 rows are the test of both fallbacks, and the v0.18.0 row is the first
    with the relocated layout.
  - **v0.22.0 replay** may add a `broker.log` reader; the matrix row then covers it.
  - **v0.17.0 `classifier-retirement`:** the deprecated parse-only tables (exempt from the
    no-lost-value assertion) and the manual-approvals log written only by the `--unattended` drive
    loop's escalations (the capture script drives one from the v0.17.0 row on). **v0.21.0
    `supervisor-vocabulary-split`:** its v1 → v2 config migration drops both deprecated tables, so
    the exemption still holds after migration.
  - Builds on the released v0.17.0 `test-session-state-isolation` and this release's
    `test-suite-determinism` (HOME and socket isolation for the resume scenarios).

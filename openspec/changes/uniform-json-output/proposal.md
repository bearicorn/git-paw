## Why

v1.0.0 freezes git-paw's machine-readable output, which today is three unrelated JSON shapes.
`status --json`, `approvals --json` and `doctor --json` are built three different ways:
`status` and `approvals` with ad-hoc `serde_json::json!`, `doctor` with a private `Report`
struct. None carries a schema version. Doctor checks have no stable identifier, and
`status --json` names a terminal implementation (`tmux_running`) that v0.18.0's terminal
backend makes incidental. The commands an external orchestrator calls most after v0.18 have
no JSON at all: `add`, `remove` and `list-clis`. Freezing this as it stands would lock in the
inconsistency and leave the v0.18 remote/MCP consumers to scrape human output.

## What Changes

- One machine-output contract (new capability `cli-machine-output`) covers every command that
  takes `--json`. Under it:
  - the command prints exactly one JSON object on stdout and nothing else;
  - the object carries a top-level integer `schema_version` (1);
  - keys are snake_case, absent values are `null`, and the output has no ANSI escapes;
  - the exit code is the same as in human mode;
  - a failing command prints nothing on stdout and its error on stderr.
  - Within a schema version, changes are additive only.
- Existing outputs gain `schema_version` and keep every current key:
  - `status --json` also gains a terminal-neutral `terminal_alive` (the same value as
    `tmux_running`, which stays for compatibility) and a `broker` object (`url`, `state`), so it
    carries what the human output already shows.
  - `doctor --json` checks gain a stable `id`.
  - `approvals --json` is unchanged apart from the version.
- New `--json` flags:
  - `add`: `session`, `branch`, `agent_id`, `cli`, `worktree_path`, `paused`.
  - `remove`: `session`, `branch`, `worktree_path`, `worktree_kept`.
  - `list-clis`: `clis[]` with `name`, `display_name`, `source`, `path`.
- Each output is a typed `Serialize` struct in one output module, not an inline `json!`. A
  standing guard checks that every `--json` command appears in the contract's roster and in the
  CLI reference.
- The `cli-parsing` requirements for `status` and `list-clis` stop saying "no additional
  arguments". For `status` that text was already wrong: `--json` has existed since v0.5.0.

No breaking change: every key a consumer reads today keeps its name, type and meaning.

## Capabilities

### New Capabilities

- `cli-machine-output`: the shared contract for `--json` output (single object, stdout
  discipline, `schema_version`, additive evolution, error and exit-code semantics, no ANSI) and
  the roster of commands that provide it.

### Modified Capabilities

- `session-state`: ADDED requirement for the versioned `status --json` document
  (`schema_version`, `terminal_alive`, `broker`).
- `preflight-diagnostics`: ADDED requirement for the versioned doctor document with stable
  check ids.
- `approval-pattern-surfacing`: ADDED requirement for the versioned approvals document.
- `git-add-remove-branch`: ADDED requirements for `add --json` and `remove --json`.
- `cli-parsing`: MODIFIED *Status subcommand* and *List-CLIs subcommand* (accept `--json`).

## Impact

- **Code:**
  - a new `src/output.rs` holding the output DTOs (`StatusOutput`, `DoctorOutput`,
    `ApprovalsOutput`, `AddOutput`, `RemoveOutput`, `ClisOutput`) and the shared
    `print_json` helper;
  - `src/commands/{status,approvals,add,remove,clis}.rs` and `src/doctor.rs` (`render_json`,
    `CheckResult` gains `id`) render through it;
  - `src/cli.rs` adds `--json` to `Add`, `Remove` and `ListClis`.
- **Interaction:** `add --json` / `remove --json` never prompt and print no progress on stdout.
  Warnings stay on stderr, as they are today.
- **Tests:** `tests/status_json_cli_session_id.rs`, `tests/session_stale_hygiene.rs`,
  `tests/approvals_integration.rs`, `tests/doctor_e2e.rs` and `src/doctor_tests.rs` keep
  passing unchanged, because no key is removed. New tests: `tests/json_output_contract.rs`
  (contract and roster guard), plus `add`/`remove`/`list-clis` JSON e2e tests.
- **Docs:** `docs/src/cli-reference.md` (a "Machine-readable output" section with each schema;
  the `status`, `doctor` and `approvals` entries get the new keys; `add`, `remove` and
  `list-clis` gain `--json`), `--help` for the three new flags, and the README CLI table if it
  lists flags.
- **Enum-variant ripple:** none.
- **Dependencies:** none.
- **Overlap with the other v0.21.0 track:**
  - Terminal robustness (`NO_COLOR`, non-TTY) must keep JSON free of escapes; this contract
    states it.
  - `config-check-command`'s `git paw config check --json` is on this contract's roster. Its document is that change's to define.
  - v0.17.0's `status-and-picker-truth` keeps `tmux_running` as the has-session fact; `terminal_alive` carries the same fact.
  - `finish-command` defers `--json` to this change. It is not on the roster: `finish`'s result shape belongs to that change, which joins the roster by adding the flag under this contract.
  - DS-9 (status wording for dead panes) changes human output only.
  - `git paw finish`, if it has a `--json`, follows this contract.
- **Cross-version dependencies:**
  - Builds on the archived v0.17.0 change `broker-repo-identity`. Its repository-aware broker probe
    decides `broker.state`, and a broker serving another repository reports as `foreign`.
  - Rebases over v0.18 `cmd-library-surface`, whose typed command outcomes become the input to
    these DTOs (entry renders, the library returns data), and over `terminal-backend-seam`,
    whose session liveness feeds `terminal_alive`.
  - `session-state-reconciliation` (this release) adds a `repairs` array to the doctor
    document under this contract.

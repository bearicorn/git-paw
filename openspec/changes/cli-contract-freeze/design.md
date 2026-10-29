## Context

What exists today, at the tip this change was written against:

- **Exit codes** (v0.18.0). `src/error.rs` defines `exit_code::ERROR = 1`,
  `exit_code::USER_CANCELLED = 2` and `exit_code::NOT_ATTEMPTED` (also 2); `PawError::exit_code()`
  maps `UserCancelled` and `NotAttempted` to 2 and everything else to 1, and `PawError::exit()`
  is the only `process::exit` in non-test code (called once from `main`). `main` parses with
  `Cli::parse()`, so clap's own usage errors exit **2** and `--help`/`--version` exit 0 — the
  documented table in `docs/src/cli-reference.md` ("2 = User cancelled") omits this.
  `doctor::exit_code` maps any ✗ to `ERROR` (surfaced as `PawError::DoctorFailed`).
  `supervisor::drive::run_drive_loop` returns `Ok` (exit 0) for every `DriveOutcome` —
  `Completed`, `EscalatedForReview`, `Stuck`, `Heartbeat` and `SessionTornDown` — and `Err` only
  for an error that stops the loop itself (exit 1). v0.18.0 has no `CliUnavailable` outcome: a
  coding agent's unavailable CLI is a `cli-unavailable` escalation, so its run ends
  `EscalatedForReview`. v0.24.0's `error-exit-code-contract` maps the four non-completion
  outcomes to exit 3. `status`, `stop`, `pause` with no session print a line and return `Ok`.
- **Contract records.** None at this tip. `core-error-handling` pins the `PawError` → code
  mapping. v0.21.0's `supervisor-vocabulary-split` adds `src/deprecation.rs` (`core-deprecation`):
  the registry, the once-per-process stderr warn helper, the raw-argument scan, a build guard that
  fails when an entry outlives its removal release, and the registry-sourced deprecated-key ⚠ in
  `config check` and doctor.
- **Parsing tolerance.** No serialized type uses `deny_unknown_fields`, so unknown config keys
  and session fields are already ignored on load (`doctor` reports unknown config keys).
- **Upstream cycles this builds on.** v0.21.0 adds a `version` field to config and the wire
  formats, DTOs at the five serialized edges, and uniform `--json`; v0.23.0 adds support tiers;
  v0.24.0 runs an errors-and-exit-codes consistency pass and a back-compat matrix. v0.18.0
  renames the bundled `supervisor` skill to `orchestrator` with the old name as an alias.
  v0.21.0 renames the rest of the supervisor vocabulary (`--coordinated` / `--bare`,
  `default_mode`, `[orchestrator]` / `[workers]` / `[drive]`, broker root identity
  `orchestrator`) with every old spelling a deprecated alias scheduled for removal in 1.0.0;
  v1.0.0's `remove-supervisor-vocabulary` removes them.

## Goals / Non-Goals

**Goals:**
- One place that says what 1.x promises, and a build failure when a change breaks it.
- A concrete, frozen exit-code table grounded in what the code returns.
- A deprecation path with a real warning, so post-1.0 evolution has a lawful route.
- Freeze the coordinated-session vocabulary v0.21.0 introduced, with no "supervisor" left in it.

**Non-Goals:**
- Changing any exit code, flag, key or format (renumbering belongs to v0.24.0's pass).
- Defining the wire DTOs, version fields or `--json` schemas themselves (v0.21.0).
- Choosing support tiers per CLI (v0.23.0) — only freezing the definitions and the list.
- Stabilising the Rust library API.

## Decisions

### D1 — Contract = a prose chapter + a generated manifest + a guard test

The chapter (`docs/src/stability.md`) states the rules; the manifest
(`docs/src/stability-manifest.txt`, `{{#include}}`d by the chapter) lists entries; the guard
(`tests/contract_manifest.rs`) regenerates the code-backed sections and diffs them. Sections and
their sources:

| Section | Source |
|---|---|
| commands / flags / values | `git_paw::cli::Cli::command()` walk, skipping `is_hide_set()` args and commands |
| config keys | `schemars` schema of the v0.21.0 config DTO (types + defaults) |
| exit codes | a `const` table next to `error::exit_code` (code, meaning) |
| MCP tools | `tools/list` from a spawned `git paw mcp` (names + input schema) |
| broker message types | the v0.21.0 `broker::wire` type strings |
| first-class CLIs | v0.23.0's support-tier table |
| deprecations | `deprecation::REGISTRY` |
| broker HTTP routes, helper-script paths + subcommands | hand-maintained in the manifest; the guard probes each route against a test broker (non-404) and runs each helper's `--help` |

The manifest header carries `contract-major: 1`; the guard compares it to
`env!("CARGO_PKG_VERSION")`'s major. Regeneration is an explicit opt-in
(`GIT_PAW_BLESS_CONTRACT=1 cargo test --test contract_manifest`), so an accidental removal
cannot be blessed silently in the same diff without the reviewer seeing the manifest change.

*Alternatives rejected:* prose only (no enforcement — the failure mode the freeze exists to
prevent); `cargo-semver-checks` (covers the Rust API, which is deliberately out of contract, and
not the CLI, config or wire surface); `trycmd`-style golden help output (pins human text, which is
out of contract, and adds a dependency).

### D2 — Freeze the four exit codes v0.24.0 settles

The table freezes the contract v0.24.0's `error-exit-code-contract` lands: `0` done; `1` attempted
and failed; `2` nothing attempted (clap's parse failures, `UserCancelled`, and v0.17.0's
`NotAttempted`, such as a `setup-cli` item that is not supported); `3` ran but stopped short
(`WaveIncomplete`: an `--unattended` wave ending `escalated-for-review`, `stuck`, `heartbeat` or
`session-torn-down`, which includes any run that escalated a permission prompt; and
`IntegrationRefused` from `finish`). `2` keeps one meaning for both usage errors and declined or
impossible confirmations, because in every case no work began, and it matches the clap/getopt
convention. The table, scenarios and `tests/exit_code_contract.rs` rows are re-diffed against the
archived v0.24.0 text before apply.

*Alternative rejected:* more per-outcome codes at the freeze (one per unattended outcome, 130 for
cancellation); the outcome label is in the summary, and one "stopped short" code is what a wrapper
needs.

### D3 — Freeze the coordinated-session vocabulary

User decision (2026-10-19): after v0.18.0 no supervisor agent exists in a session — the operator's
driving agent is the orchestrator, every pane is a worker, and git-paw's own part is the drive loop
plus coordinated mode — so "supervisor" names nothing real. v0.21.0's `supervisor-vocabulary-split`
renamed the surface split by concern with deprecated aliases, and `remove-supervisor-vocabulary`
removes the aliases in 1.0.0. The frozen vocabulary is: `start --coordinated` / `--bare`;
`default_mode`; `[orchestrator]` (how git-paw invokes the external orchestrator, with `.gates` and
`.escalation`), `[workers]` (policy for worker panes and branches), `[drive]` (git-paw's own
coordination machinery); the broker root identity `orchestrator`. The stability chapter defines
*coordinated session*, *orchestrator*, *worker* and *drive loop*. Version-1 config files that still
use `[supervisor]` keep loading through migration for the whole 1.x line, but those keys are not
manifest entries.

*Alternative, superseded:* keep and freeze `--supervisor` / `[supervisor]` (this change's earlier
draft) — it would freeze a name for a role that no longer exists.

### D4 — Deprecation: warn ≥ 2 minors, remove only at a major, one registry

The registry, helper and consumers are v0.21.0's `core-deprecation` (`src/deprecation.rs`:
`REGISTRY: &[Deprecation { item, kind, since, replacement, removed_in: Option<&str> }]`, a pure
`notice`, a once-per-process stderr `warn`, `scan_args` for flag spellings; consumed by `main`
after parsing, config load, `skills::resolve`, the broker wire decoder, `config check` and
doctor). This change adds the policy on top: an entry deprecated in a 1.x release SHALL schedule
its removal at a later major (`X.0.0`) or not at all, and the manifest and stability chapter render
the registry. The "two minor releases" window is a release-checklist rule (AGENTS.md) checked at
release prep, because a test cannot know the last minor of the previous major; the guard tests
enforce the mechanical half — `core-deprecation`'s removal-release guard, plus the manifest guard
allowing a removal only at a major and only of a registered item.

*Alternative rejected:* `#[deprecated]` attributes / clap's built-in deprecation support — the
former is compile-time and invisible to users; the latter does not cover config keys or skill
names, and two mechanisms would drift.

### D5 — Pre-1.0 deprecations are resolved at the freeze

v0.21.0 and v0.24.0 registered their deprecations with removal release 1.0.0: `--supervisor`,
`--no-supervisor`, the `supervisor` skill name (v0.18.0's alias), the broker root spelling
`supervisor`, `supervisor.verify-now`, MCP's `mode` field and `--from-specs`. The
`core-deprecation` guard fails the 1.0.0 build while any of them remains, so
`remove-supervisor-vocabulary` and `remove-from-specs-alias` remove them and the first manifest
records a registry holding only the version-1 `[supervisor]` config paths (no removal release:
version-1 files load through migration for the whole 1.x line). Those include the
`[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` tables that v0.17.0's
`classifier-retirement` made parse-only with no effect and that v0.21.0's v1 → v2 migration drops
rather than renames. If the configuration schema still carries their parse-only structs, the
manifest generator excludes them from the configuration section, so a key that does nothing is
never frozen as a stable entry; they are listed only under deprecations. No entry is scheduled for
2.0.0 by this change. The stability chapter lists what 1.0.0 removed, each with its replacement.

### D6 — The Rust library API is outside the contract

The `git-paw` crate publishes a library target because the binary and the integration tests are
built on it (and v0.18.0's `cmd-library-surface` widens it for the MCP write tools). Promising
semver on that API would freeze internals the contract deliberately keeps movable (the v0.21.0
DTO split exists for exactly this). The stability chapter says so and the crate-level rustdoc
repeats it. Cargo's semver tooling will still read the crate version; that is accepted and
documented.

### D7 — Support tiers: freeze definitions and the first-class list only

Per v0.23.0's freeze tie-in. Detection-list churn (a binary renamed upstream, an editor shadowing
a CLI name — DS-6) must stay possible in minors; a first-class CLI is a promise users plan around.

## Risks / Trade-offs

- [The manifest becomes a rubber stamp — reviewers bless without reading] → regeneration needs an
  explicit env opt-in, the diff shows removed lines, and the failure message names the
  deprecation policy.
- [Hand-maintained manifest sections drift] → each hand entry is probed by the guard (route
  answers, helper `--help` succeeds); only the *existence* of those entries is hand-kept.
- [Exit code 2 conflates two causes] → documented as one meaning ("nothing attempted"); scripts
  that need to tell them apart read stderr, which is acceptable because neither case did work.
- [`start --unattended` exits 3 whenever any prompt was escalated, even if all work completed] →
  intended (v0.24.0): a prompt went to a human, so a wrapper must not read the run as clean.
- [A deprecation warning on stderr during an MCP session is invisible to some clients] →
  acceptable; the result is unchanged and the registry is published (behaviour owned by
  `core-deprecation` since v0.21.0).

## Migration Plan

1. Rebase the exit-code table and the format-compatibility rows on v0.24.0's outcome; rebase the
   config and wire sections on v0.21.0's DTOs; take the support-tier table from v0.23.0.
2. Land the exit-code `const` table and the major-only removal rule in `src/deprecation.rs` (no
   behaviour change).
3. After `remove-supervisor-vocabulary` and `remove-from-specs-alias` have emptied the registry of
   1.0.0 entries, generate the manifest; add the guard and exit-code tests.
4. Render the registry in the manifest and the stability chapter.
5. Write the stability chapter, update the CLI reference, README, `supported-clis.md`, and the
   AGENTS.md release checklist.

Rollback: revert; nothing persisted changes.

## Open Questions

- **Replay (gated on v0.22.0).** If the replay rethink has not shipped by the v0.24.0 feature
  freeze, `replay` SHALL NOT enter the stable surface as is: v1.0.0 then hides it from `--help`
  and the manifest, lists it under the unstable surfaces, and registers it deprecated (or
  experimental) so it can be redesigned in 1.x. If v0.22.0 ships, `replay` and its flags are
  frozen like any other command. Decide when the manifest is first generated; not a committed
  requirement of this change.
- **Helper-script interfaces in the stable tier.** Specced stable (paths and documented
  subcommands), because consumer allowlists grant by path and the exported orchestrator skill
  calls the subcommands. If v0.20.0's skill packaging moves the scripts, the frozen paths are the
  packaged ones.

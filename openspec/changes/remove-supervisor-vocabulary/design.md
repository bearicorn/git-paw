## Context

After v0.21.0 (`supervisor-vocabulary-split`) every old spelling is an alias recorded in the
`core-deprecation` registry with a removal release:

| Item | Kind | Removal |
|---|---|---|
| `--supervisor`, `--no-supervisor` | flag | 1.0.0 |
| root identity `supervisor` (agent id, sender fields, inbox path) | wire value | 1.0.0 |
| `supervisor.verify-now` | wire value | 1.0.0 |
| skill name `supervisor` and the `supervisor` override directory | skill name | 1.0.0 |
| MCP `get_session_status.mode` | output field | 1.0.0 |
| every version-1 `[supervisor]` path | config key | none (read through migration) |
| `[supervisor.auto_approve]`, `[supervisor.common_dev_allowlist]` (no effect since 0.17.0, `classifier-retirement`) | config key | none (dropped by the version-1 → version-2 migration, never renamed) |

v0.24.0 adds `--from-specs` (removal 1.0.0). The registry guard fails the build at version 1.0.0
while any 1.0.0 entry remains, so this change is forced to happen in 1.0.0. The session file has
been read in both spellings since v0.21.0 but still written as `"supervisor"`.

## Goals / Non-Goals

**Goals:**
- No removed spelling is accepted on any 1.0.0 input surface, and each one fails loudly with its
  replacement.
- Every config file that loaded in 0.x still loads in 1.x.
- The 1.0.0 stable surface — and the user-facing text around it — has no "supervisor" in it.

**Non-Goals:**
- Renaming Rust modules and types (library API is outside the contract, `cli-contract-freeze` D6);
  done opportunistically, not required.
- Changing any behavior behind the new names.

## Decisions

### D1 — Config files: keep translating version-1 files (option (a)), recommended
The version-1 → version-2 step stays in the migration chain for the whole 1.x line, with its
deprecation warning, and `sweep.sh` keeps its `[supervisor]` fallback (the shell mirror of that
step). Reasons: AGENTS.md's compatibility rule says existing configs load without error;
`config-schema-versioning` exists precisely so older layouts keep loading after a layout change;
`cli-contract-freeze` requires a format change to "keep reading the previous version for the rest of
the major"; and a config is a file users rarely revisit, unlike a flag in a script that fails loudly
on the next run. The cost is small and contained: one pure step, one fallback, and a registry entry
with no removal release. The `[supervisor]` keys do not enter the stable surface (the manifest is
generated from the version-2 wire schema), and a `version = 2` file still refuses `[supervisor]`, so
the current format has one spelling.
*Alternative (b):* refuse version-1 files at 1.0.0 with a migration error listing each rename.
Simpler code, but it breaks every repository that did not migrate during 0.21–0.24 on the first
command after upgrade, for no user benefit. Rejected.

### D2 — Flags, aliases and wire forms are removed, not translated
Unlike a config file, each of these fails at the point of use with an actionable message, so the
user fixes it once. Keeping them would freeze two spellings for 1.x.
- **Flags:** hidden `--supervisor` / `--no-supervisor` args that exist only to be rejected after
  parsing through `Cli::command().error(ErrorKind::UnknownArgument, …)` — exit 2, clap's own error
  path, no git-paw code runs. One removed-flag table (flag → replacement) serves these and
  `remove-from-specs-alias`'s `--from-specs`: that change lands first and introduces the table, and
  this one adds the two rows.
- **Wire:** the decode mapper's rewrite is deleted; a request carrying `supervisor` as the root
  identity, or publishing `supervisor.verify-now`, gets `400` with the shared error body naming
  `orchestrator` (never a silent drop: an unknown `blocked.from` would otherwise route nowhere). The
  shape rule no longer lists `supervisor`. Reading back event records persisted by 0.x releases
  keeps the old-spelling mapping, so replaying an old run is unaffected.
- **Skill name:** `resolve("supervisor")` returns the unknown-skill result with a message naming
  `orchestrator`; a `supervisor` override directory is ignored, with a one-line notice when it is the only
  override present.
- **Output field:** MCP `mode` is removed; `session_mode` stays.

### D3 — Session files: write the new spelling now, read both forever
Every reader since v0.21.0 understands `"coordinated"`, so 1.0.0 writes it. Files written by 0.x
(`"supervisor"`) — including a session that is live across the upgrade — keep loading; this is
format history, not a deprecated input, so it has no registry entry.

### D4 — Vocabulary sweep as a direct, verbatim spec reorganization
v0.21.0 added reading rules instead of rewriting every requirement that names an old spelling. This
change does the rewrite when nothing else is in flight: a direct edit of `openspec/specs/`
(spec-organization §2, §6) that replaces old spellings verbatim, keeps the requirement count and
the requirement→test map unchanged, deletes the reading rules, and renames the capabilities —
proposed: `supervisor-autonomous-orchestrator` → `orchestrator-escalation`,
`supervisor-skill-discipline` → `orchestrator-skill-discipline`, `supervisor-config` →
`drive-config`, `supervisor-correction-loop` → `drive-correction-loop`, `supervisor-branch-refresh`
→ `drive-branch-refresh`, `supervisor-learnings` → `drive-learnings`,
`supervisor-unattended-operation` → `drive-unattended-operation`, `supervisor-launch` →
`session-coordinated-launch` — with the specifications page includes fixed in the same commit and the
spec-organization skill's namespace list updated. Sibling changes still open at that point archive
with `--skip-specs` per §6 — except `remove-from-specs-alias`, which archives before this change
(its deltas target pre-sweep headers), so the sweep runs on its text and finds its requirements
already in the new vocabulary.

### D5 — A standing no-mention guard
`tests/no_retired_vocabulary.rs` scans `git paw --help` and every subcommand's `--help`, README,
`docs/src/` (except the configuration reference's migration table, the changelog and the stability
chapter's removal list), `assets/`, the generated default config, MCP tool descriptions and
`git paw doctor` output, and fails on `supervisor` (case-insensitive). v0.17.0's deprecation of
`[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` added prose about them to the
configuration reference and the user guide; by 1.0.0 those mentions live only as migration-table
rows ("dropped — deprecated 0.17.0, no effect", per v0.21.0), so the guard needs no extra
exception. Their deprecation warning goes to stderr when a version-1 file loads, which the guard
does not scan. `openspec/specs/` is scanned
with an allowlist of the requirements that name a removed spelling on purpose (the rejection and
migration requirements). In `openspec/specs/` the scan also skips backticked code identifiers and
paths and `Test:` annotation lines: Rust module, type and function names (`src/supervisor/`,
`cmd_supervisor`, `supervisor_layout`, …) are internal and are not renamed, so the sweep in task
3.2 leaves them alone.

### D6 — Doc file rename with a redirect
`docs/src/user-guide/supervisor.md` → `docs/src/user-guide/coordinated-mode.md`, with
`[output.html.redirect]` in `docs/book.toml` so external links survive (the pattern v0.18.0 used for
the quick start).

## Risks / Trade-offs

- [Scripts using the old flags break on upgrade] → exit 2 with the replacement in the message;
  `BREAKING CHANGE:` footer; they warned for four minor releases.
- [An orchestrator skill copy exported before 0.21.0 still posts as `supervisor`] → `400` naming
  `orchestrator`; `git paw init` re-exports the current skill.
- [Mixed versions: a 0.x broker still running after upgrade] → it accepts `orchestrator` (since
  0.21.0); a pre-0.21 broker does not — restart the session after upgrading, as documented since
  0.21.0.
- [The sweep touches many requirements] → verbatim replacement only, count invariant and test map
  checked before and after.

## Migration Plan

1. Delete the 1.0.0 registry entries; let the compiler and the guard enumerate the alias sites.
2. Flag rejection (shared removed-flag table), wire rejection, skill-name result, MCP field, session
   write spelling, `remove` refusal.
3. Spec sweep + capability rename + includes; doc file rename + redirect; README.
4. No-mention guard.
5. Commit as breaking with a `BREAKING CHANGE:` footer naming each removal and its replacement.

Rollback: revert; 0.24.x reads everything 1.0.0 writes.

## Open Questions

1. Option (a) for config files (recommended) vs (b).
2. The proposed capability names in D4.
3. Whether `PAW_SUPERVISOR_PID` (an internal marker line, not a user surface) is renamed in the
   sweep — recommended only if v0.18.0 left it in place.

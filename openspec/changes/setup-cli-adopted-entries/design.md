## Context

`git paw setup-cli claude --allowlist` plans only the grant-table entries the target file lacks
(`src/cli_setup/plan.rs:433-454`) and records only what it writes; undo removes only recorded
entries that are still present, equal and of a git-paw shape (`src/cli_setup/undo.rs:439-460`). The
setup record is, by design, "what git-paw wrote, so it can take back exactly that"
(`src/cli_setup/record.rs:1`). Doctor's setup check then asks the record, not the file, whether the
current grant table is applied (`src/cli_setup/doctor.rs:140-152`), so any entry the user already
had reads as missing, forever (PP-22).

The broad-grant advisory (`doctor.rs:299-353`) reads only the allowlist target's
`permissions.allow` and ignores helper rules that name a subcommand; nothing reads the shared
`.claude/settings.json`.

## Goals / Non-Goals

**Goals:**
- A freshly applied allowlist reports ✓ whatever the file held before; undo + re-apply converges.
- Doctor names known-dangerous leftovers (self-approval, blanket curl, the inert seed) as SAFETY
  warnings, with a by-hand remedy.

**Non-Goals:**
- Changing apply, the record format or undo.
- Removing or rewriting any grant (git-paw only writes through `setup-cli`, and only its own
  entries).
- Inspecting user-level settings or the `.claude/settings.json` of individual worktrees (pre-v0.16
  seeding also wrote there, but worktrees are recreated per session).

## Decisions

### D1 — Doctor checks the file; the record keeps its meaning

Two fixes were on the table (PP-22's own routing): record pre-existing entries as "adopted" (undo
skips them, doctor counts them), or have doctor check the file. Chosen: **doctor checks the file**.
- Undo safety holds by construction. An "adopted" flag would make the record the only thing
  separating a user's entry from git-paw's, so one wrong flag removes a user's grant. A v0.17
  binary reading a v0.18 record ignores the unknown field and would undo adopted entries as its
  own.
- Smallest surface: one pure function and the probe; the apply/record/undo requirements and the
  record schema stay as released.
- The name of the change stays: doctor now treats pre-existing matching entries as applied.

"Applied" = the planned value is present at its key path in the item's target file. The probe
already reads every file the record names (`src/doctor.rs:1379-1384`); it also reads the target
path from the fact row so the check does not depend on which entries happen to be recorded.

### D2 — Both directions of "differ"

The released requirement says "differ"; the code checks only planned-but-missing. The check also
warns when the record holds a value the running binary no longer plans (a withdrawn grant). The
remedy text (`--undo`, then `--allowlist`) is unchanged and correct for both.

### D3 — Residue is a separate, closed-list SAFETY check

The broad-grant advisory answers "is this rule broad?" and stays as released. Residue answers
"is this a known-dangerous leftover?" and is a closed list:
- blanket curl — `curl` followed only by a wildcard. A specific loopback rule is not residue;
- a helper subcommand the grant table never grants, or a wildcard in its place. The set is
  `SWEEP_NEVER_GRANTED` plus `docs-fetch.sh get` (`facts.rs:113`, `:151-157`), so the check follows
  the grant table instead of a second list. Matched by the helper name anywhere in the specifier,
  so absolute and interpreter-prefixed forms are caught;
- a top-level `allowed_bash_prefixes` key.

Files: the fact row gains the CLI's project-shared settings path (`.claude/settings.json`) beside
`allowlist_target`; doctor reads both from the main checkout. A rule may appear in both checks
(`Bash(curl *)`, `Bash(.git-paw/scripts/sweep.sh *)`); that is acceptable — one is advisory about
breadth, the other names a specific danger. Abbreviation reuses `abbreviate` (`doctor.rs:480`).

## Risks / Trade-offs

- [A user deliberately keeps a residue entry] → the warning persists; that is intended for
  self-approval, and the detail says why. Never ✗.
- [Doctor reads a file the user tracks and shares] → read-only, prints only abbreviated matching
  entries.

## Migration Plan

Doctor-only. Rollback: revert. No persisted state changes.

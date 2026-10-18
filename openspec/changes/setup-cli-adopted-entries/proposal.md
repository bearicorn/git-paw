## Why

Two doctor gaps around `git paw setup-cli`, both reported by the peer-4-poker dogfood on 0.17.0
(`.git-paw/peer4poker-0.17.0-findings.md`):

- **PP-22 — "out of date" forever.** Apply skips a planned entry the settings file already holds and
  records only what it added (`src/cli_setup/plan.rs:446` skips a present value; the module rule at
  `plan.rs:13-15` says "an entry already present is neither planned nor recorded"). Doctor then
  decides "out of date" by comparing the running binary's planned entries against the **record**
  (`src/cli_setup/doctor.rs:140-152`: `applied` comes from the record, `wanted` from the grant
  table). Every entry the user already had is "missing", so a freshly applied allowlist warns, and
  the remedy it prints (`--undo`, then `--allowlist`) cannot clear it: undo leaves the pre-existing
  entries in place and re-apply skips them again. `orchestrator-read-surface` (archive position 3)
  and `agent-hook-and-tmux-guards` (position 8) both give "doctor says out of date → undo →
  re-apply" as their upgrade path, so it has to converge first.
- **Upgrade residue nobody flags (SAFETY).** The peer's `.claude/settings.local.json` still held
  durable grants from before 0.17 — `Bash(curl *)` and `Bash(.git-paw/scripts/sweep.sh approve *)`,
  the second of which lets any agent session in the repository approve its own permission prompts —
  and `.claude/settings.json` still carried the inert `allowed_bash_prefixes` seed that git-paw
  wrote before v0.16 (removed by `1dbdb0e`, `fix(supervisor): stop seeding inert command allowlists
  into cli settings`). Checked against the released code: the broad-grant advisory does list
  `Bash(curl *)` (a single-word prefix, `doctor.rs:421-424`), but only as a generic "review"
  advisory; it never lists a helper rule that names a subcommand (`doctor.rs:407-410` returns "not
  broad" for any `.git-paw/scripts/<helper> <word>` rule), so the self-approval grant is silent; and
  the probe reads only the allowlist target (`src/doctor.rs:1428-1443`), so nothing in
  `.claude/settings.json` is ever seen.

## What Changes

- **Doctor judges "out of date" against the settings file, not the record.** An entry the running
  binary would plan counts as applied when the item's target file holds it at its key path, whoever
  added it. The setup check also warns when the record holds an entry the running binary no longer
  plans (the released requirement already says "differ"; the code only checks one direction). The
  remedy text is unchanged. Apply, the setup record and undo are unchanged: the record still lists
  only what git-paw wrote, and undo still never removes an entry git-paw did not write.
- **Doctor warns about dangerous grant residue.** A new read-only check reads the main checkout's
  project-shared and project-local settings files of each CLI whose fact row declares the allowlist
  item, and warns (⚠, never ✗) on: a blanket `curl` allow rule; an allow rule that grants a helper
  subcommand the grant table never grants (`sweep.sh approve`, `verified`, `feedback-gate`,
  `conflict-ack`, `docs-fetch.sh get`), including a wildcard in the subcommand position; and the
  inert `allowed_bash_prefixes` key. The remedy says to remove them by hand: git-paw never rewrites
  grants it did not write. Entries are abbreviated as in the broad-grant advisory.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `preflight-diagnostics`: *Doctor reports CLI setup state* judges applied entries against the
  target file and checks both directions; a new grant-residue check.

## Impact

- **Predecessors (implementation order):** archive position 2, after `supervisor-role-typing` (1),
  which does not touch `src/cli_setup/`. No earlier v0.18 change modifies *Doctor reports CLI setup
  state*; the MODIFIED delta is written on the released v0.17.0 text (`preflight-diagnostics`
  `spec.md:349`). `doctor-spec-workflow-check` (4) only ADDs a requirement to the same capability.
- **Successors:** `orchestrator-read-surface` (3) and `agent-hook-and-tmux-guards` (8) grow the grant
  table; their upgrade path (doctor reports out of date → `--undo` → `--allowlist`) converges once
  this lands, even when the settings file already held some entries.
- **Not modified:** `cli-setup` (apply, record and undo keep their released contract), including
  *Allowlist entries are path-scoped to git-paw's helper scripts*, which
  `orchestrator-read-surface` modifies.
- **Code:** `src/cli_setup/doctor.rs` (`check_against_current_plan` reads the target file from the
  probe; a pure `check_grant_residue(&ResidueProbe)`), `src/cli_setup/facts.rs` (the `claude` row
  records its project-shared settings file beside `allowlist_target`), `src/doctor.rs`
  (`probe_cli_setup` always includes the item's target file; `probe_grant_residue`; wiring in
  `run_checks`).
- **Tests:** pure table tests in `cli_setup::doctor::tests` for every scenario; `tests/doctor_e2e.rs`
  for the PP-22 round trip (a pre-seeded entry, apply, doctor ✓, undo + re-apply, doctor ✓, the
  pre-seeded entry still on disk) and for residue in both files; the existing "byte-identical"
  e2e covers the new reads.
- **Docs:** `docs/src/user-guide/doctor.md` (setup check wording, the residue check),
  `docs/src/user-guide/cli-setup.md` (upgrade note: pre-existing entries count as applied; residue
  to remove by hand), `docs/src/user-guide/security-posture.md` (self-approval residue).
- **Enum-variant ripple:** none.
- **Security / safety:** read-only. The residue check closes a silent self-approval path left by
  upgrades; it never edits a grant and never influences how a prompt is handled. Undo safety is
  unchanged by construction: the record still lists only entries git-paw wrote. Output is
  abbreviated so a credential embedded in a rule is not echoed.
- **Dependencies:** none.

## Why

v0.4.0 has `--supervisor` (boolean, default `false`) but no negation flag. A user whose `.git-paw/config.toml` contains `[supervisor] enabled = true` cannot disable supervisor for a single session without editing the config file. The existing `--supervisor` flag is a one-way switch — it forces supervisor on, but you can't force it off.

This is a real gap, not a UX nicety: projects that default to supervisor-on (the recommended setup for active development) have no escape hatch when a user wants to do a quick non-supervised run (e.g. a one-off branch flip, a debug-only session, a test of how the system behaves without the supervisor). Today the workaround is "edit `.git-paw/config.toml`, run, edit it back" — error-prone and easy to forget the revert.

## What Changes

- Add a `--no-supervisor` boolean flag (default `false`) to the `start` subcommand.
- When `--no-supervisor` is passed, supervisor mode SHALL be disabled for the session regardless of what `[supervisor] enabled` says in config. The flag wins over config.
- Make `--supervisor` and `--no-supervisor` mutually exclusive at parse time. clap's `conflicts_with` produces a clear "cannot be used together" error if both are passed.
- Extend the existing supervisor mode resolution chain (`src/main.rs::resolve_supervisor_mode`) to take `--no-supervisor` as the first short-circuit step, before `--supervisor` and before consulting config:
  1. `--no-supervisor` → off (new short-circuit)
  2. `--supervisor` → on (existing v0.4 step 1)
  3. `[supervisor] enabled = true` in config → on (existing)
  4. `[supervisor] enabled = false` in config → off (existing)
  5. No section + dry-run → off (existing)
  6. No section + interactive TTY → prompt (existing)
  7. No section + non-TTY → off (existing fallback)
- The new flag SHALL appear in `git paw start --help` output with a brief description.

Not in scope:
- The `prompt_on_start` config field originally floated in MILESTONE — dropped because the v0.4 prompt only fires when `[supervisor]` is absent (a one-time event for fresh repos), so the proposed config field provides no new capability. See MILESTONE drift item 17.
- Renaming `--supervisor` or restructuring the existing resolution chain. Only an additive new flag.
- Changing the default value of `--supervisor` (still `false`).
- Removing the prompt fallback (still fires when no section + TTY + not dry-run). Users who want to permanently disable the prompt run `git paw init` once, which writes the section.

## Capabilities

### New Capabilities
*(none — this change extends existing capabilities only)*

### Modified Capabilities
- `cli-parsing`: add `--no-supervisor` flag definition; add mutual-exclusion rule between `--supervisor` and `--no-supervisor`.
- `supervisor-cli`: extend the supervisor-mode resolution chain to short-circuit on `--no-supervisor` before any other step. The existing chain (config → prompt) is preserved for the case where neither flag is passed.

## Impact

**Code**:
- `src/cli.rs` — `StartArgs` gains `no_supervisor: bool` with `#[arg(long, conflicts_with = "supervisor", help = "Disable supervisor for this session, overriding any [supervisor] enabled = true config")]`.
- `src/main.rs` — `resolve_supervisor_mode` (and its callsite-shim `resolve_supervisor_mode_from_cwd`) gains a leading `if no_supervisor_flag { return Ok(false); }` step. The function signature grows one boolean parameter; all callers updated.
- The `cmd_start` entry path (and `cmd_start_with_specs` / `cmd_start_from_specs`, whichever name the cross-format-spec-selection change settles on) wires the new `args.no_supervisor` field into the resolution call.
- Help text and docs updated for the new flag.

**Tests**:
- CLI parse:
  - `start --no-supervisor` → `no_supervisor == true`, `supervisor == false`.
  - `start --supervisor` → unchanged.
  - `start --supervisor --no-supervisor` → parse error mentioning both flags.
- Resolution:
  - `--no-supervisor` with `[supervisor] enabled = true` config → resolved to `false`.
  - `--no-supervisor` with `[supervisor] enabled = false` config → resolved to `false` (idempotent).
  - `--no-supervisor` with no `[supervisor]` section → resolved to `false` (no prompt, regardless of TTY).
  - `--no-supervisor` with `--dry-run` → resolved to `false`.
  - Without `--no-supervisor` (and without `--supervisor`), the existing v0.4 chain produces the same outcome it did before this change — full regression coverage.
- Help output:
  - `git paw start --help` contains `--no-supervisor`.

**Backward compatibility**: fully additive. Every v0.4 invocation that worked before continues to work identically. The new flag is opt-in — its absence preserves v0.4 behaviour exactly.

**Mismatches surfaced (already tracked under MILESTONE.md items 17 and 18)**:
- 17: dropping the proposed `prompt_on_start` config field because v0.4's prompt is already gated on section-absent (not "every bare invocation").
- 18: confirming `--no-supervisor` does not exist in v0.4 — this change adds it.

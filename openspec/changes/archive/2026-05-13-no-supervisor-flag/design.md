## Context

`src/main.rs::resolve_supervisor_mode` is the single point of truth for "does supervisor run this session?" It takes `(supervisor_flag: bool, dry_run: bool, config: &PawConfig, prompt: &mut dyn SupervisorPrompt)` and returns `Result<bool, PawError>`. The existing chain:

1. `--supervisor` → on
2. `[supervisor] enabled = true` → on
3. `[supervisor] enabled = false` → off
4. No section + `--dry-run` → off
5. No section + TTY → prompt
6. No section + non-TTY → off (per `TerminalSupervisorPrompt::ask` line 143-145)

This change inserts a new step 0 (highest precedence): `--no-supervisor` → off.

## Goals / Non-Goals

**Goals:**
- Add `--no-supervisor` as a session-level override that beats both `--supervisor` (mutual exclusion enforced at parse time) and any config setting.
- Preserve the existing chain bit-for-bit when `--no-supervisor` is absent.
- Keep `resolve_supervisor_mode` testable in isolation (it already is — the existing tests at `src/main.rs:1741+` exercise the chain via a mock `SupervisorPrompt`).
- Keep the function signature minimal — one new boolean parameter, ordered before `supervisor_flag` for readability.

**Non-Goals:**
- Restructuring the resolution chain (no rename of steps, no reordering of existing steps).
- Adding a separate "explicit-off" state to distinguish `--no-supervisor` from `[supervisor] enabled = false` downstream. Both produce `false`; downstream code doesn't need to know which path got there.
- Logging or telemetry on flag usage.

## Decisions

### D1. New parameter goes first

Function signature becomes `resolve_supervisor_mode(no_supervisor_flag: bool, supervisor_flag: bool, dry_run: bool, config: &PawConfig, prompt: &mut dyn SupervisorPrompt) -> Result<bool, PawError>`. Putting the new param first reads as "negation has highest precedence" and matches the order of the resolution chain.

### D2. Mutual exclusion enforced by clap, not at runtime

Using `#[arg(long, conflicts_with = "supervisor")]` on the new field means clap rejects the combination at parse time with its standard error message. The resolver function is therefore guaranteed never to see both flags `true` simultaneously. No runtime defensive check needed; if both are somehow `true`, that's a programming bug, not a user error.

### D3. No partial-disable / soft-disable variants

We considered `--no-supervisor=auto-merge-only` (disable supervisor's verification but keep auto-merge) and `--no-supervisor=verify-only` (the reverse). Rejected: split modes are a v1.0.0+ concern. The v0.5.0 flag is a clean on/off override.

### D4. `--no-supervisor` does not affect the prompt

When neither `--supervisor` nor `--no-supervisor` is passed and the chain falls through to the prompt step, the prompt still fires (gated on TTY + not-dry-run, as today). `--no-supervisor` is a session override, not a "permanent disable." Users who want to silence the prompt forever set `[supervisor] enabled = false` in config — same as today.

### D5. Help text wording

The flag's help string is `"Disable supervisor for this session, overriding any [supervisor] enabled = true in config"`. Two key bits:
- "for this session" — clarifies it's a one-shot override, not a permanent setting.
- "overriding any [supervisor] enabled = true in config" — names the specific scenario users will recognise.

The help text intentionally does NOT say "the opposite of `--supervisor`" — that's correct but not actionable. The phrasing above tells users the use case.

## Risks / Trade-offs

- **[Risk] Users discover `--no-supervisor` and use it without understanding the resolution chain, then wonder why config doesn't seem to take effect.** → **Mitigation:** the help text names the precedence ("overriding any [supervisor] enabled = true"). Release notes call out the new flag and its precedence. The mdBook chapter for `git paw start` documents the full chain.
- **[Trade-off] Adding a fourth way to express supervisor state.** Today: `--supervisor` flag, `[supervisor] enabled = true/false`, prompt. Now: + `--no-supervisor`. Multiple ways to express the same outcome can be confusing, but the new flag fills a *one-session* gap that none of the others address. The "many ways" framing is also normal for CLI tools (e.g. git's many ways to say "don't push").

## Migration Plan

Additive. No migration step needed.

1. Land the change. Existing scripts unaffected (they don't pass `--no-supervisor`).
2. Users with `[supervisor] enabled = true` who want to skip a session run `git paw start --no-supervisor`.
3. Rollback: revert. `--no-supervisor` parses as unknown flag (clap error); workaround for users who want to skip a session is "edit config, run, edit it back" — same as v0.4.

## Open Questions

- **Should `--dry-run --no-supervisor` print "supervisor disabled per --no-supervisor" in the dry-run header?** Decision: not in scope. The dry-run output already shows whether supervisor will run; adding flag attribution is polish, not capability. Revisit if dogfood shows users confused about "why did it pick this state."
- **Should `git paw init` mention `--no-supervisor` somewhere?** Decision: yes, in the chapter docs but not in the init flow itself. Init is for setting durable defaults; `--no-supervisor` is for one-off overrides. Mixing them in the init prompt would muddy intent.

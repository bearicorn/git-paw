## Context

`SessionLiveness` has three values: `Alive`, `Stale` and `Indeterminate`. GP-03c added an
agent-pane check to `session_liveness_for` and reused `Stale` for "every agent pane reverted to
a shell", so callers of that probe cannot tell this case from "tmux session absent".
`DisplayStatus::from_receipt` turns `(Active, Stale)` into `🔴 stale`. `cmd_status` builds every
line from that, including the false "tmux session no longer exists" and `tmux_running: false`.
`doctor` also uses `session_liveness_for`, so it files the session as "tmux session is gone" with
a `purge --stale` remedy. That remedy does nothing here: `cmd_purge_stale` uses the plain
`has-session` probe, which says alive.

For DS-8, `Prompter::select_cli` already has a `default` parameter. Its documented and
implemented behaviour (`prompter.rs`): when `default` matches a CLI's `binary_name`, the dialoguer
cursor starts on that entry; otherwise it starts at index 0. The picker is always shown. Only
`resolve_cli_for_specs` passes a value. `run_selection` has nowhere to take one from. The
per-branch method `Prompter::select_cli_for_branch` has no `default` parameter at all, and
`TerminalPrompter` hard-codes `.default(0)` there. And `cmd_supervisor`, on its interactive arm (no
`--branches`, no specs), calls `run_selection` and then keeps only the branch names
(`selection.mappings.into_iter().map(|(b, _)| b)`), so the CLI the user picked is discarded and
every worker launches with `agent_cli` (`--cli` > `default_cli` > supervisor CLI).

## Goals / Non-Goals

**Goals:**
- Only an absent tmux session is reported as `stale`. A session whose agent panes exited is
  reported `stopped`, with tmux running and a working remedy (`git paw start`).
- `default_cli` pre-selects in the bare-start picker in both assignment modes, and both docs say
  exactly that.
- A supervised launch launches each worker with the CLI picked for it.

**Non-Goals:**
- Changing what `start`, `purge --stale` or `attach` do with any session.
- Changing the launch grace period or the shell-name list.
- Letting `default_cli` skip the picker.
- Adding new JSON fields to `status --json`.

## Decisions

**D1 — A new `SessionLiveness::AgentsExited` variant, not a second probe in `status`.** The
probe that runs `list-panes` already knows the session exists; returning a separate value keeps
that knowledge with it. The other option, `status` and `doctor` each running an extra
`has-session` to tell the two `Stale`s apart, repeats the fork in every caller and costs another
tmux call. Every `session_liveness_for` caller other than status and doctor uses
`matches!(…, Alive)`, so the new variant means "not alive" to them, exactly as `Stale` did, and
their behaviour does not change. `classify_liveness` and `session_liveness_with` are untouched, so
the plain-probe callers (`invalidate_if_stale`, `cmd_purge_stale`, `attach`) can never see the new
variant.

**D2 — `from_receipt` maps `AgentsExited` to `Stopped` for every receipt status.** This matches
the existing `effective_status` table (not alive means Stopped) and the existing requirement. The
arm is written out explicitly instead of left to the `_` wildcard, so the mapping is visible and
covered by a test.

**D3 — Status rendering.** `tmux_running` becomes `matches!(liveness, Alive | AgentsExited)`.
When liveness is `AgentsExited`, status prints the hint
`↳ agent panes exited — run 'git paw start' to recover them` under the `Status:` line. The stale
hint stays as it is and now prints only for `DisplayStatus::Stale`. JSON keeps its shape:
`status` becomes `"stopped"` and `tmux_running` becomes `true`.

**D4 — Doctor gets its own finding.** `HygieneProbe` gains `exited_agent_sessions: Vec<String>`,
filled when the probe returns `AgentsExited` for an `active` or `paused` receipt. `check_hygiene`
keeps the single `session state` check row:
- stale sessions → today's ⚠ with the `purge --stale` remedy (unchanged);
- otherwise exited-agent sessions → ⚠ "agent panes exited: <names>" with remedy
  "run `git paw start` to recover the session";
- otherwise ✓.
At most one session is probed per repository (`find_session_for_repo`), so the two findings
cannot both apply. Dropping the dead-panes warning altogether was rejected: doctor would then show
✓ for a session with no running agents, which breaks *Liveness reflects dead agent panes*.

**D5 — `run_selection` gets a trailing `default_cli: Option<&str>` parameter.** It is passed
unchanged to `select_cli` in the `CliMode::Uniform` arm and to `select_cli_for_branch` for every
branch in the `CliMode::PerBranch` arm. `Prompter::select_cli_for_branch` gains a
`default: Option<&str>` parameter with exactly `select_cli`'s semantics; `TerminalPrompter`
replaces its hard-coded `.default(0)` with the same name → index lookup `select_cli` uses (sharing
one helper). Passing `&PawConfig` instead was rejected: `run_selection` is config-free on purpose
(it is driven by flags and the prompter), and one `Option<&str>` keeps it that way. Both callers
pass `config.default_cli.as_deref()`.

**D6 — The supervised launch keeps the selection's CLI mapping.** `cmd_supervisor` already has a
per-branch override map (`spec_cli_overrides`) that `effective_agent_cli` consults before falling
back to `agent_cli`; the spec-driven arm fills it from `resolve_cli_for_specs`. The interactive arm
fills it from `selection.mappings` the same way, so dry-run output, pane commands, approval-flag
resolution and the session record all see the picked CLI with no new plumbing. The `--branches`
arm shows no picker and keeps the `agent_cli` fallback. Rejected: overriding `agent_cli` with the
uniform pick — it cannot express per-branch picks and would also change the supervisor-CLI
fallback semantics.

## Risks / Trade-offs

- [A script parses `status --json` and treats `"stale"` as "agents dead"] → For this case the
  value changes from `"stale"` to `"stopped"`. That is the fix: `"stale"` was wrong, and the
  existing *JSON status output adds stale value* contract has always tied `"stale"` to an absent
  tmux session.
- [A second, unrelated `SessionLiveness` consumer is added later and forgets the variant] →
  Exhaustive matches fail to compile. The `matches!` sites already treat the variant as "not
  alive", which is the safe default.
- [The picker pre-selects a `default_cli` that is not installed] → `select_cli` falls back to
  index 0 when the name is not in the detected list. That behaviour is already specified in
  *CLI picker with optional pre-selection*.
- [A supervised user relied on the picker being ignored] → Unlikely (the picker visibly asks for
  a CLI), and the new behaviour is what the prompt promises. Users who want the session-wide
  default without a picker pass `--branches` or `--cli`.
- [`Prompter` is a public trait; adding a parameter to `select_cli_for_branch` breaks external
  implementors] → The trait is internal plumbing for testability; every implementor is in-tree.

## Migration Plan

No data migration. Receipts are not changed. Rollback is a plain revert.

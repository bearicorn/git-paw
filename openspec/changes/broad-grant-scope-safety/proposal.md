## Why

The option resolver selects a prompt's permanent broad grant by **index**, without
reading what that option actually grants. Claude Code's option 2 is not one thing: for a
simple command it is a narrow prefix grant naming the exact command
(`Yes, and don't ask again for: cargo test --bin git-paw recover::`), but for a
**compound** command it is a scope-wide grant instead
(`Yes, and don't ask again for similar commands in /…/worktrees/<branch>`). The resolver
takes option 2 in both cases.

A scope-wide grant permanently allowlists a *class* of commands for an entire worktree on
the strength of one safe instance. Three instances were observed across two agents in a
single dogfood wave, and the trigger is systematic rather than incidental: agents issue
compound commands constantly (`cargo fmt && cargo test`, `cmd; echo ---; cmd`), and the
CLI cannot form a single narrow prefix grant for a multi-statement command, so it offers
the directory-wide one. Every compound command is therefore an opportunity to silently
widen the worktree's permanent allowlist, invisibly to the operator.

The existing arbitrary-code carve-out does not cover this. It refuses the broad grant for
`bash -c` / `python -c` code strings, but `timeout 1 bash /tmp/script.sh` is not a code
string — it slipped through and was resolved to a directory-wide grant, which had to be
overridden by hand.

## What Changes

- The broad-grant rule SHALL gain a **scope** condition independent of its existing
  classification condition: the auto-approver SHALL select a permanent grant option only
  when that option's text grants a **specific command**, and SHALL NEVER select one that
  grants a directory, a path prefix, or a class of commands — whatever the verdict on the
  command itself.
- Option selection SHALL be resolved from the option's **text**, not from its position in
  the list. An option whose grant scope cannot be determined SHALL be treated as
  scope-wide and refused.
- When the only permanent-grant option offered is scope-wide, the auto-approver SHALL
  fall back to the one-time "Yes" option rather than escalating — the command itself
  already classified safe, so the wave keeps moving under the narrower approval.
- The bundled `sweep.sh approve` helper SHALL apply the same scope rule, so the helper and
  the in-tool approver cannot disagree.
- The scope determination SHALL NOT be profile-sourced. `cli-prompt-profiles` requires
  that "Security decisions are never profile-sourced", and the breadth of a permanent
  permission grant is exactly such a decision — a consumer must not be able to widen what
  auto-approves by editing a profile field. The rule lives in the classifier.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `approval-auto`: *Broad grant restricted to allowlisted non-arbitrary-code verbs* gains
  the grant-scope condition; *Option-index selection for Yes/No prompts* requires
  text-based rather than index-based resolution of the broad-grant option.

## Impact

- `src/supervisor/auto_approve.rs` — broad-grant rule and option resolution
  (`--resolve-option` path used by `git paw __classify`).
- `assets/scripts/sweep.sh` and the tracked `.git-paw/scripts/sweep.sh` copy — the helper
  reads the resolved option from `__classify --resolve-option`, so the fix is centralised,
  but the helper's own option parsing must be checked for the same assumption.
- **Security-relevant**: this narrows what is auto-approved. It cannot widen the
  allowlist; the failure mode of a bug here is extra prompts, not extra grants.
- **Delta coordination**: modifies different `approval-auto` requirements than
  `live-prompt-tail-window`, and touches no `cli-prompt-profiles` requirement, so it does
  not collide with `drive-loop-exit-summary-truth`'s deltas. All three archive
  independently.

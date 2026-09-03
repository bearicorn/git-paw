## Context

Two independent convention leaks, grouped because both are small, both live in compiled
runtime code rather than in `assets/`, and both are therefore invisible to the
export-agnosticism audits.

The `cargo` classification is a leftover: `classify_command_class` predates the
stack-preset mechanism, and `permission_prompt.rs:35-41` already records that the poll
loop supersedes this coarse classification. It is dead weight that also contradicts the
stack-neutrality `auto_approve.rs:30-34` enforces.

`GATE_TAGS` is a different shape of leak — not stale, but universal where it should be
per-project. git-paw's five-gate framework is a genuine git-paw convention; hard-coding
its vocabulary means a consumer with different gates simply cannot participate in the
correction loop.

## Goals / Non-Goals

**Goals:**

- Remove the toolchain leak rather than parameterise it — nothing needs a
  language-specific permission class.
- Let a consumer name their own gates, with git-paw's list as the default.
- Keep both changes provably behaviour-preserving for existing installations.

**Non-Goals:**

- Changing the safe-command whitelist, the danger list, the correction budget, the
  exhaustion policy, or any safety gate.
- Redesigning `PermissionType` beyond removing the one variant.
- Making the five-gate *framework* optional. This changes the recognised gate
  *vocabulary*, not the supervisor's verification model.

## Decisions

**D1 — Delete the `Cargo` variant rather than generalise it to a `Build` class.**
*Considered and rejected:* renaming it to a stack-neutral `Build` class sourced from the
stack preset. That keeps a classification the poll loop already supersedes, and adds a
config surface for something nothing consumes. Deletion is smaller and removes the leak
outright; if a build-tool class turns out to be needed, it can be added deliberately from
the stack preset with its own justification.

**D2 — Default the gate vocabulary to the current list, including `scope`.** The default
must reproduce today's constant exactly, `scope` included, or existing correction cycles
change behaviour silently. The spec pins the default list so a future edit that drops an
entry is a test failure.

**D3 — An unrecognised tag stays a non-gate producer.** This is the existing behaviour
(the conflict detector's `[conflict-detector]`, the branch guard and peer messages are
deliberately not gate verdicts) and it must survive becoming configurable. Specified
explicitly, because "make it configurable" invites accidentally treating *any* tag as a
gate.

**D4 — Group the two leaks in one change.** *Considered and rejected:* two changes. They
share no code, but each is a handful of lines with the same motivation and the same
audit gap; separate proposals would cost more review overhead than they save. They are
independently revertible.

## Risks / Trade-offs

- **Removing an enum variant ripples further than the diff suggests.** `PermissionType`
  flows through `poll.rs` signatures, `commands/supervisor.rs:224`, and test fixtures. →
  Grep `PermissionType::Cargo` across `src/` and `tests/` up front so the work is scoped
  rather than discovered mid-build; this enum is not on the AGENTS.md watchlist, which is
  precisely why it is easy to under-scope.
- **Someone reads "removed cargo classification" as reduced safety.** → It was a coarse
  labelling step superseded by the poll loop; auto-approval decisions are made by the
  classifier, which is untouched. Worth stating plainly in the changelog.
- **A configurable gate vocabulary could be set to an empty list**, silently disabling
  correction cycles. → Consider whether an empty configured list should be rejected or
  fall back to the default; resolve during implementation and pin the choice with a test.

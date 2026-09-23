## Context

`git paw __classify --resolve-option` returns the option index the auto-approver should
dispatch, and both the in-tool approver and `sweep.sh approve` take their answer from it,
so there is one implementation rather than two that agree. The broad-grant rule it applies
reasons entirely about the **command**: is it an arbitrary-code runner, did it classify
safe, is its verb read-mostly. It does not read the option text it is selecting.

That was sound while a permanent-grant option always meant "this exact command". It is not
sound now. Claude Code offers two different permanent grants depending on the prompt:

```
2. Yes, and don't ask again for: cargo test --bin git-paw recover::
2. Yes, and don't ask again for similar commands in /…/worktrees/feat-worker-session-id-capture
```

The second permanently allowlists a class of commands across a whole worktree. Observed
three times in one dogfood wave across two agents.

The trigger is the shape of the command, not its content: a CLI cannot form a single
narrow prefix grant for a multi-statement command, so a compound command gets the
directory-wide option. Agents issue compound commands constantly, so the exposure is
continuous rather than occasional.

## Goals / Non-Goals

**Goals:**

- Never auto-select a permanent grant broader than the command that was classified.
- Keep the wave moving: fall back to the one-time option rather than escalating, since
  the command itself already classified safe.
- Keep one implementation shared by the in-tool approver and the helper.
- Fail closed on anything ambiguous.

**Non-Goals:**

- Changing which *commands* classify safe, with one exception: the folded-in managed-script
  trust fix narrows the managed helper-script rule to its exact location (see Decisions).
  Otherwise the classifier's verdict is untouched; only what we do with that verdict
  changes.
- Making the rule configurable. See Decisions.
- Detecting or cleaning up grants already written by previous runs. Out of scope, worth a
  separate look.
- Changing how compound commands are classified. An earlier draft of this design said
  "each segment is already evaluated" — that was wrong: today the shell allowlist matches
  only the first segment of a command and ignores the rest. Per-segment evaluation is
  delivered by `classifier-compound-segments`, which lands first (wave 3). This change
  relies on it and does not re-implement it.

## Decisions

**Read the option's text, not its index.** The defect is that index 2 was treated as
meaning one thing. The fix is to classify the grant the option offers — specific command
versus directory / path / command class — and gate on that. This is a second, independent
condition alongside the existing arbitrary-code and classification conditions.

**Fall back to one-time, do not escalate.** Escalating would be the conservative reflex,
but it is the wrong trade: the command already classified safe, so a human adds nothing by
confirming it, and the dogfood showed that unnecessary escalations are the dominant tax on
an unattended wave. Selecting option 1 is both safe and non-blocking.

**Fail closed on undeterminable scope.** If the option's text cannot be resolved to a
specific command, treat it as scope-wide. The cost of a false negative is one extra
one-time approval; the cost of a false positive is a permanent directory-wide allowlist.

**Not profile-sourced — deliberately.** It is tempting to put grant-shape patterns in
`cli-prompt-profiles` next to `broad_grant_marker`, since the wording is CLI-specific. It
is rejected because that capability states outright that "Security decisions are never
profile-sourced". The breadth of a permanent permission grant is a security decision: if a
consumer could widen it by editing a profile field, the profile becomes a privilege-
escalation surface. The rule belongs in the classifier, where the rest of the safety
decisions live.

**Managed-script trust is an exact location, not a subtree.** The managed helper-script
rule exists so an agent's boot-time `broker.sh` call never stalls on a prompt. It
currently asks `agents::is_managed_path` whether the token is "managed", and that answer
is "anything under `.git-paw/`" — correct for `remove` (which must not refuse on
git-paw's own bookkeeping) but far too broad for an execution grant: `.git-paw/tmp/` is
the scratch directory agents write freely, and the relative token is never normalised,
so `.git-paw/../../tmp/sweep.sh` passes too. The rule instead normalises the path,
refuses any `..` component outright (rather than trying to resolve it), and requires the
result to be exactly `<worktree>/.git-paw/scripts/<name>` (or, with no worktree, the
relative `.git-paw/scripts/<name>`). Refusing `..` instead of resolving it keeps the
check lexical and fail-closed, and no legitimate helper call uses `..`.

A content check against the embedded asset (`src/init.rs` `BROKER_SCRIPT` /
`SWEEP_SCRIPT` / `DOCS_FETCH_SCRIPT`) closes the remaining gap — an agent overwriting the
helper in its own worktree — and is specified as SHOULD. Its cost: a repo-root helper
left stale after a binary upgrade (until `git paw init` refreshes it) would escalate
instead of auto-approving. `bundled-helper-discovery` refreshes worktree copies from the
embedded assets on every attach, which keeps agent panes unaffected.

**This change strictly narrows.** A bug in it produces extra prompts, never extra grants.
That asymmetry is worth preserving explicitly in review.

## Risks / Trade-offs

- **More prompts for compound commands.** Every compound command now takes a one-time
  approval instead of a permanent grant, so an iterate-until-clean loop prompts more. That
  is the correct trade — the permanent grants it was writing were unwanted — but it makes
  the `worker-guidance-read-tools` change (reuse one scratch path, prefer native read
  tools) more valuable, since those habits reduce the prompt count honestly.
- **Grant-wording drift.** The scope test keys on how a CLI words its options. A reworded
  grant would fall into the undeterminable bucket and be refused — a safe failure, but it
  would quietly increase prompting. Worth a test fixture per known wording and a note that
  silence here means extra prompts, not extra grants.
- **Existing grants are not revoked.** Any directory-wide grants already written by
  earlier runs remain in the consumer's CLI settings. This change stops new ones; it does
  not clean up old ones, and the operator has no visibility into what accumulated.
- **Interaction with `live-prompt-tail-window`.** Both modify `approval-auto`, but
  different requirements, so the deltas are independent. Neither blocks the other; if both
  are in flight at once the implementer should expect to touch
  `src/supervisor/auto_approve.rs` in both.

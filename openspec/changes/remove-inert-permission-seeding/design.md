## Context

Three seeding paths write permission grants at session startup:
`curl_allowlist.rs` (the broker helper grant), `dev_allowlist.rs` (stack dev commands),
and `worktree_allowlist.rs` (per-worktree seeding plus `ensure_claude_dir_excluded`).
All converge on writing an `allowed_bash_prefixes` array into a CLI settings file.

Claude Code reads `permissions.allow`, not `allowed_bash_prefixes`. The key is git-paw's
own invention; the array is written, is read back only by git-paw's tests, and is never
consulted by the CLI it targets. The machinery is inert on its design target and
structurally meaningless elsewhere — no other supported CLI has a comparable file.

The replacement is already designed and specced, and it is not code.
`worker-guidance-skill-export` is a pure `skill-agent` change: it puts worktree
orientation into the coordination skill and the tiered permission model into the
supervisor skill, with `git paw __classify` as the single authoritative policy source.
Together with `unattended-boot-hardening` resolving each CLI's permission mode to an
explicit flag, that is the whole of GP-01's answer. This change only deletes what those
two made redundant.

## Goals / Non-Goals

**Goals:**

- Delete code that never functioned, and stop creating vendor-specific directories in
  worktrees running other vendors' CLIs.
- Correct the guidance that credits the seeded allowlist for prompt-free operation.
- Leave the codebase with no successor mechanism to maintain.

**Non-Goals:**

- **Any successor seeding mechanism, including a "strategy" declaration.** See D2.
- Implementing a working `permissions.allow` seeder for Claude Code. Correcting the key
  would make the seeding *function*, which is a different decision with its own security
  surface — auto-granting bash prefixes into a user's real permission file deserves its
  own proposal, not a drive-by key rename.
- Deleting settings files git-paw previously wrote.
- Changing the classifier, the danger list, or any safety gate.
- Touching memory isolation's independent use of the settings path.

## Decisions

**D1 — Remove rather than repair.** *Considered and rejected:* changing the key to
`permissions.allow`. That converts a no-op into a live permission grant written into the
user's own settings file — a materially larger security decision that should be proposed
and reviewed on its own terms, not smuggled in as a bug fix. Removing the inert code is
the honest, reversible step; adding real seeding later remains open.

**D2 — No successor mechanism, not even an off-by-default one.** An earlier draft of this
change specced a per-CLI "seeding strategy" declaration defaulting to none. That is
dropped. Since D1 declines to build a real seeder, the declaration would have had zero
implementors: a config field every consumer sets to "off" and nothing ever sets otherwise,
plus spec requirements and tests describing behaviour that never executes. The project's
own guidance forbids abstractions for single-use code and configurability that was not
requested. The seam for a future seeder is the `settings_path` field that already exists
and is retained for memory isolation; nothing further is needed to keep the option open.

**D3 — Do not delete pre-existing settings files.** git-paw stops writing; it does not
clean up. A user may have added their own entries to a file git-paw created, and deleting
it would destroy their configuration to tidy ours.

**D4 — Sequence after the replacement lands.** Removing the seeding before
`unattended-boot-hardening` and `worker-guidance-skill-export` would leave guidance
pointing at machinery that no longer exists. The removal is safe only once the
replacement is documented.

**D5 — Preserve `settings_path` for memory isolation.** `core-memory-isolation` keys off
the *parent directory* of each configured `settings_path`, independent of seeding. This
is the easiest thing to break while deleting seeding code, so it is specified as an
explicit requirement with its own scenario rather than left as a code comment.

## Risks / Trade-offs

- **Someone believes seeding was load-bearing and reports a regression.** → It was inert;
  the proposal states the evidence (the key mismatch, and that the array was read back
  only by git-paw's own tests). Worth restating in the changelog entry, because "we
  removed the allowlist" reads alarming without the "it never worked" context.
- **Deleting three seeding paths risks removing something adjacent that did work.** →
  `ensure_claude_dir_excluded` and the memory-isolation `settings_path` use are the two
  known adjacencies; the latter is spec-pinned by D5. Audit each deletion for a second
  caller before removing it. `dev_allowlist.rs`'s stack-preset data in particular may
  have a consumer beyond seeding — check before deleting the data along with the writer.
- **Asset conflict with `worker-guidance-skill-export`.** Both edit the supervisor
  skill's permission section — that change adds the tiered model, this one removes the
  false attribution. → Land that change first (D4) and edit around its result.
- **Future need for real seeding.** → Retained `settings_path` plus a fresh proposal.
  Nothing here forecloses it, and D2 avoids paying for it now.

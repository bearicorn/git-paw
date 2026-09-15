## Context

Both defects here are conformance failures, not design gaps. The requirements exist and
the implementation does not meet them.

`broker-agent-helper`'s *Helpers provisioned into agent worktrees* has required worktree
provisioning since it was written. Nothing implements it: the bundled scripts are
`include_str!`-ed only in `src/init.rs`, which serves repo-level `git paw init`, and
`worktree_provision::provision_worktree` — the seam every launch path calls — writes env
files, allocates runtime slots and runs lifecycle hooks, but no scripts. The gap was
invisible because this repository has run `git paw init`, so the *repo root* has
`.git-paw/scripts/`; a worktree is a separate checkout and `.git-paw/` is gitignored, so
it has none.

For session discovery the spec set is the problem. Three capabilities mention where
session state lives and they do not agree: `broker-watcher-and-state` and
`approval-pattern-surfacing` say `.git-paw/sessions/…`, `core-ci-hygiene` says the XDG
data directory. The code uses XDG (`session::session_state_dir()` →
`crate::dirs::data_dir()`). `sweep.sh` implements the repo-relative one, which is exactly
what two of the three specs told it to do.

## Goals / Non-Goals

**Goals:**

- Make the session-state location unambiguous, in one place, so the next implementer
  cannot pick the wrong answer in good faith.
- Let a bundled helper resolve the session from the repository root, since that is where
  an external orchestrator runs.
- Provision helpers at the one seam every launch path shares, so no path can miss it.
- Keep the `$TMUX` fallback working for the in-session case.

**Non-Goals:**

- Moving the session-state location. XDG is right and `core-ci-hygiene` already depends
  on it; this change documents it, it does not change it.
- Rewriting the two stale incidental path references. See Decisions.
- Teaching helpers to work without a session at all.
- Changing what the helpers do once they have found the session.

## Decisions

**Make `session-state` the single authority, and say so in the requirement.** The usual
fix would be to correct every stale mention. The reason to add an explicit authority
clause instead is that the stale mentions are *incidental* — each sits inside a
requirement about something else (roster seeding; the manual-approval log). Those
requirements will keep being edited for their own reasons, and each edit is another
chance to restate a path. An authority clause makes every such mention non-normative by
construction, which is durable in a way that one round of corrections is not.

**Leave the two stale mentions to the pre-freeze spec audit, and record it.** Correcting
them properly means reproducing 113 lines of unrelated requirement text in MODIFIED
deltas, inside a change about helper discovery. That trade is bad: it bloats the diff,
puts unrelated requirements at risk of transcription error, and buries the actual fix.
The authority clause removes the harm in the meantime, and the debt is written down
rather than forgotten.

**Provision at `provision_worktree`, not at each call site.** It is the single seam the
supervisor path, the bare path, the spec-driven path and `git paw add` all already call.
Fixing it there satisfies the requirement everywhere at once and makes it structurally
hard for a future launch path to miss — which is precisely how this one was missed.

**Share `init.rs`'s embedded constants.** A second `include_str!` set would let the
worktree copy drift from the init copy, reintroducing the version-skew the requirement's
"matching the running binary's version" clause exists to prevent.

## Risks / Trade-offs

- **Writing into every worktree.** Provisioning writes files into each agent worktree on
  every attach. The requirement already demands idempotence; the risk is clobbering a
  helper a user deliberately modified in a worktree. That is an acceptable loss — the
  requirement explicitly wants the worktree helper to match the running binary — but it
  should be stated in the docs rather than discovered.
- **Helper behaviour changes for existing users.** A helper that currently resolves via
  `$TMUX` will start resolving via the data directory. If both are present and disagree
  (two sessions, one stale), the resolution order matters. Prefer the authoritative
  lookup and treat `$TMUX` as the fallback, so the in-session case stays correct.
- **The bash mirror problem again.** `assets/scripts/sweep.sh` and the tracked
  `.git-paw/scripts/sweep.sh` both exist and have drifted before. Both must be updated,
  and `bash -n` run on each — these files carry a known heredoc-quoting hazard.
- **Two specs will still say the wrong thing.** Mitigated by the authority clause, but a
  reader who finds only the stale mention is still misled. The pre-freeze audit item is
  the real fix and should not be allowed to lapse.

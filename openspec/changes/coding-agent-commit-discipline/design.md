# Design — coding-agent-commit-discipline

## Context

Three coding-agent commit behaviours surfaced as recurring friction across the
v0.6.0 and v0.7.0 dogfood cycles. All three are properties of the *prose* the
bundled coordination skill injects into each coding agent's context — not of any
git-paw code path — so all three are fixable by editing
`assets/agent-skills/coordination.md` and pinning the new guidance with
skill-content tests.

1. **Self-archive reflex (F7).** In both the v0.6.0 and v0.7.0 dogfoods, finished
   coding agents typed `/opsx:archive` immediately after their final commit. The
   `opsx-role-gating` capability already blocks the *execution* of `/opsx:verify`
   and `/opsx:archive` from a coding-agent worktree (a post-commit watcher detects
   archive activity and publishes `agent.feedback`, with an optional `block` mode
   that asks the supervisor to revert). But that guard is *reactive*: the agent
   still burns a turn attempting the command, and the supervisor must intervene
   after the fact. The skill never tells the agent what to do *instead* of
   reaching for verify/archive — it only forbids the commands. The missing piece
   is a positive "stand by" instruction.

2. **Messy commit history.** Agents produce streams of `fix typo` /
   `address feedback` micro-commits, each a tiny follow-up to a commit the agent
   made moments earlier. This forced manual squashes at release time: v0.6.0
   collapsed 148 commits to 10, and a v0.7.0 feature squashed 4 commits to 1.
   The existing "Commit cadence" section teaches per-group commit boundaries and a
   ~10-file ceiling, but says nothing about what to do when you need to *fix the
   commit you just made*. The fix is a "releasable unit + amend the just-made
   commit" rule.

3. **Over-opinionated commit-format prose.** `coordination.md` currently
   prescribes Conventional Commits (`feat(<scope>):`, `fix(<scope>):`, etc.) and
   tells the agent to derive the scope from the change name. But commit-MESSAGE
   FORMAT is a *per-project* convention — git-paw happens to use Conventional
   Commits, but the bundled skill ships into arbitrary host repos whose conventions
   differ, and the recurring "no AI trailer" problem lives in the same per-project
   space. git-paw already injects the host project's `AGENTS.md` into the agent
   context (the `agents-md-injection` capability). The generic bundled skill should
   defer message format to that injected file rather than hardcode one project's
   choice.

## Goals

- Add a positive **stand-by-after-commit protocol** to the coordination skill: after
  the final commit, publish the `committed`/`done` signal and wait; do not reach
  for `/opsx:verify` or `/opsx:archive`. This complements `opsx-role-gating` by
  turning a forbidden-commands list into an actionable next-step.
- Add **releasable-unit + amend-fixups** discipline to the existing "Commit cadence"
  section: each commit must build and pass its own gates; a small follow-up to the
  commit you *just* made (not yet verified, not yet moved past) should be folded in
  with `git commit --amend` rather than landed as a separate micro-commit. Earlier
  or already-verified commits must NOT be amended.
- **De-opinionate the commit-format prose**: replace the hardcoded Conventional
  Commits prescription with "follow the project's commit-message conventions (see
  the project's `AGENTS.md`)."
- Pin all three with skill-content tests (the existing
  `tests/*_skill_content.rs` pattern) so a future edit cannot silently regress them.

## Non-Goals

- **No code changes.** This is a bundled-skill-prose change. No new Rust, no config
  fields, no wire-format additions. The skill is rendered and injected by the
  existing `skills.rs` machinery unchanged.
- **No new enforcement guard.** The stand-by protocol is guidance that *complements*
  the existing `opsx-role-gating` execution guard; this change does not add a new
  detector, block mode, or hook. Enforcement of the role boundary stays where it is.
- **No prescription of a specific commit-message format.** The point of part 3 is to
  *remove* git-paw's opinion about message format, not to substitute another. The
  format lives in the host project's `AGENTS.md`.
- **No change to the per-group cadence or ~10-file ceiling.** Those are kept; the
  amend rule is layered on top of them, scoped to the just-made commit only.

## Decisions

### D1 — All three belong in the bundled skill prose, not in code

Each of the three behaviours is something the agent *reads and follows*, not
something git-paw can mechanically enforce without false positives:

- Stand-by is a judgment about *what to do next* after committing; a hook can block
  the wrong command but cannot author the right behaviour.
- Releasable-unit + amend is orchestration discipline: "is this a fixup to the commit
  I just made, or a new unit of work?" requires understanding the work, which the
  agent has and a guard does not.
- Commit-format is intentionally being *de-opinionated* — encoding a format in code
  would be the opposite of the goal.

So the change edits `coordination.md` and asserts the prose with skill-content
tests, matching every prior skill-prose change (`coordination-context-budget`,
`conflict-detector-fn-granularity`, `supervisor-skill-discipline`, etc.).

### D2 — Releasable-unit + amend is *generic* orchestration discipline → bundled skill

One might argue "clean commit history" is a per-project style preference that belongs
in `AGENTS.md` alongside the message format. It is not. The releasable-unit rule is a
property of git-paw's *orchestration model*, independent of any host project's style:

- **Per-commit verification.** The supervisor's five-gate verification (and the
  `per-commit-verification` capability) treats each `agent.artifact{status:"committed"}`
  event as a verification-relevant boundary. A commit that does not build/pass on its
  own breaks that contract regardless of the host project.
- **Changelog hygiene at supervisor merge.** The supervisor cherry-picks and merges
  agent branches onto the release line; micro-commit noise directly bloats the
  changelog the supervisor must curate. The 148→10 (v0.6.0) and 4→1 (v0.7.0) squashes
  are evidence this is an *orchestration* cost, paid by the supervisor, not a host
  project's stylistic preference.

Because the cost is borne by git-paw's orchestration layer (per-commit verification +
supervisor-curated changelog), the discipline that avoids it belongs in git-paw's
generic bundled skill — it applies in every host repo git-paw orchestrates.

### D3 — Commit-MESSAGE FORMAT is per-project → defer to injected `AGENTS.md`

The *shape* of a commit message (`feat(scope):` vs a bare imperative subject vs a
ticket prefix), and rules like "no AI-assistant trailer", are conventions that vary
per host repo. git-paw already injects the host project's `AGENTS.md`
(`agents-md-injection`). The bundled skill therefore points the agent at that file for
message format instead of hardcoding git-paw's own Conventional-Commits choice. This
keeps the generic skill generic and lets each project own its format in one place.

The releasable-unit/amend rule (D2) and the message-format deferral (D3) sit
side-by-side in the "Commit cadence" section but are distinct: *when/whether to make a
commit* (orchestration, bundled) vs *how to phrase its message* (style, per-project).

### D4 — Stand-by complements `opsx-role-gating`, it does not duplicate it

`opsx-role-gating` already ships a forbidden-commands block in `coordination.md`
(`<!-- opsx-role-gating:begin -->` … `:end -->`) plus a post-commit detection guard.
That block is the *execution guard* (what you must not run). The stand-by protocol is
the *positive next-step* (what to do instead): publish the terminal signal and wait
for `agent.verified` / `agent.feedback` / further `agent.intent`. The new prose
cross-references the existing role-gating block rather than restating the
forbidden-commands list, so the two stay consistent. The change touches prose
*outside* the `opsx-role-gating` sentinel markers so it does not collide with that
capability's owned region.

### D5 — Reuse the existing "Terminal action" section as the stand-by home

`coordination.md` already has a `### Terminal action: commit then publish, never archive`
section whose closing line is *"Commit, let the post-commit hook publish, then wait for
`agent.verified` …"*. That is the natural anchor for an explicit **STAND BY** protocol.
The change strengthens that section (and the existing role-gating block's tail) into a
named stand-by step rather than adding a competing top-level section, keeping the skill
from growing a second "what to do when done" location.

## Risks / Trade-offs

- **R1 — Prose drift from `opsx-role-gating`.** Two sections now describe the
  post-commit boundary. Mitigation: the stand-by prose cross-references the role-gating
  block instead of restating the forbidden commands, and the skill-content tests for
  *this* change assert only the positive stand-by guidance (not the forbidden list, which
  `opsx-role-gating`'s own tests own).
- **R2 — Amend rule misapplied to verified commits.** An agent could over-apply
  `--amend` and rewrite an already-verified or earlier-group commit, corrupting the
  supervisor's verification boundary. Mitigation: the prose states the rule narrowly —
  amend ONLY the commit you just made and have not moved past or had verified — and a
  skill-content test asserts the "do not amend an already-verified / earlier commit"
  caveat is present.
- **R3 — De-opinionating leaves projects without a format if their `AGENTS.md` is
  silent.** A host repo with no commit-format guidance in `AGENTS.md` gets no format
  steer from the skill. Accepted: this is strictly better than imposing git-paw's
  Conventional-Commits format on an unrelated repo; an absent convention means the agent
  uses ordinary good judgment, which is the correct default for an un-opinionated host.
- **R4 — Existing skill-content tests reference the old Conventional-Commits prose.**
  Softening the format text could break tests that assert `feat(<scope>):`. Mitigation:
  the tasks include auditing existing skill-content/audit tests and updating any that
  pinned the removed prescription, so the suite stays green.

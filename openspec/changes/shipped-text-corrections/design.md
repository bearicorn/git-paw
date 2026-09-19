## Context

Each defect is shipped prose that drifted from behaviour or policy: a removed practice still
described, a message type that never existed, a version promise, and an error message frozen
at the release it was written in.

## Goals / Non-Goals

**Goals:** make every piece of shipped text true, and pin each with a content assertion so
it cannot drift back.

**Non-Goals:** rewriting the boot blocks or task prompts beyond these sentences; the
GP-49 question of whether bare sessions should support `add` at all (v0.18.0).

## Decisions

**Assert absence, not wording.** Tests check that the cherry-pick invitation and `agent.done`
are gone and that no version is promised — not a specific new sentence — so future edits stay
free while the specific falsehoods cannot return.

**Name the real completion mechanism.** For code changes the post-commit hook publishes
`agent.artifact`; the prompts should say "commit", which is what the agent can actually do.

## Risks / Trade-offs

- **Existing tests may pin the old text.** Update them deliberately; a test that pins a
  falsehood is the defect.

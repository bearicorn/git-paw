## Context

The prompt was specified in v0.5.0, when `stop` was framed as destructive. It is not: it
kills the CLI processes, but worktrees, branches and session state persist, and `start`
recovers. The implementation never shipped a prompt, and the prompt-matrix test asserts its
absence, so the specs and help text are the outliers.

## Goals / Non-Goals

**Goals:** one consistent contract — no prompt, no flag — across specs, `--help`, docs, code
and tests; the breaking flag removal visible in the changelog.

**Non-Goals:** changing what `stop` does; adding a prompt anywhere else; revisiting `purge`,
which is destructive and keeps its confirmation.

## Decisions

**Remove `--force` rather than hide it.** A hidden no-op keeps scripts working but leaves a
flag that means nothing in the public surface going into the v1.0.0 freeze. The maintainer
chose a clean break now, pre-freeze, with a changelog callout.

**Neutral log wording.** The stop scenario refers to "the session's broker log" rather than
`broker.log`, because `broker-repo-identity` makes the log per-session in the same release.

## Risks / Trade-offs

- **Scripts passing `--force` break.** Intended and called out. The failure is immediate and
  self-explanatory (an unexpected-argument error naming the flag), and the fix is deleting it.

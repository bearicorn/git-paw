## Context

Twelve-odd marker sets describing Claude Code's terminal UI are compiled into six Rust
modules and mirrored in `sweep.sh`. The algorithms around them are sound and CLI-neutral;
only the literals are vendor-specific. Two in-flight changes have already begun putting
per-CLI knobs on the `[clis.<name>]` seam — `unattended-boot-hardening` (permission-mode
flags) and `drive-loop-actuator-robustness` (`submit_delay_ms`). This change follows that
established seam rather than inventing a parallel one.

The distinction that governs the whole design: **prompt shape is not permission policy.**
A profile says what the terminal looks like. It must never say what an agent is allowed to
do.

## Goals / Non-Goals

**Goals:**

- One place to describe a CLI's terminal surface, readable by both Rust and the bundled
  helper script.
- Byte-identical behaviour when nothing is configured.
- Make adding a CLI a data task rather than a code change across six modules.

**Non-Goals:**

- Moving any security decision into data. The danger list, protected paths, worktree
  boundary, broad-grant rule and send gate stay compiled — permanently and by design.
- Per-CLI approval/permission *flags* (owned by `unattended-boot-hardening`) and
  submit-delay / nudge robustness (owned by `drive-loop-actuator-robustness`).
- Changing any detection algorithm.
- Shipping profiles for CLIs we cannot test. A wrong profile is worse than none, because
  it produces confident misdetection instead of an obvious fallback.

## Decisions

**D1 — Profile carries shape only; security stays compiled.** This is the load-bearing
boundary. If a profile could influence the danger list or broad-grant eligibility, an
agent that can write a config or worktree file could widen the gate that governs it —
a privilege-escalation path for the exact actor the gate exists to contain. The spec
states this as a requirement with adversarial scenarios rather than leaving it to
implementation discipline.

**D2 — Per-field fallback, not whole-profile fallback.** A partial profile falls back
field by field. *Considered and rejected:* all-or-nothing, where an incomplete profile
disables the default entirely — that turns a small authoring omission into silent
detection failure, which is exactly the class of bug this change exists to remove.

**D3 — One source read by both Rust and the shell.** Today's parity test asserts that two
independently maintained copies agree, which can only ever catch drift after it happens.
Reading one source makes drift unrepresentable, and the parity test becomes a check that
both consumers read it. `discover_supervisor_int` (`sweep.sh:158`) is the precedent for
the shell side sourcing configuration.

**D4 — Embed the Claude profile rather than shipping it as a loose file.** Preserves
single-binary distribution and guarantees a working fallback even with no filesystem
state. Matches how the bundled skills are already embedded via `include_str!`.

**D5 — Do not ship speculative profiles.** Ship Claude embedded, and the authoring
surface. A guessed Codex or Gemini profile would misdetect confidently where an absent
profile falls back visibly.

## Risks / Trade-offs

- **A wide refactor across six modules risks silent behaviour change.** → The specs pin
  extraction semantics and rate limiting as unchanged; the existing detection tests
  should pass without modification, which is the primary regression signal. Treat any
  test that needs editing as a red flag rather than a chore.
- **Indirection makes detection harder to read.** A future maintainer sees a lookup where
  a literal used to be. → Accepted; the profile is a single well-named surface, and the
  status quo is worse (the same literal in eight places).
- **The shell side gains a parsing dependency on the profile format.** → Keep the format
  trivially parseable from shell; prefer a flat shape over nested structure precisely so
  `_paw_common.sh` does not need a real parser.
- **Someone will eventually want a security rule in the profile "just for their setup".**
  → D1 is specified with adversarial scenarios so the answer is written down and testable
  rather than relitigated.

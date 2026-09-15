## Context

Three launch paths spawn worker CLIs: `cmd_supervisor`, `cmd_add`, and the two in
`commands/start.rs` (`cmd_start` and `cmd_start_with_specs`). The supervisor path grew
most of the hardening — it builds the prompt via `build_task_prompt`, submits it through
`submit_prompt_to_pane` (literal send, settle delay, separate `Enter`), and resolves
approval flags into `AttachContext.agent_flags`. The `start.rs` paths do none of those
three things.

The boot behaviour is not an oversight so much as an inherited contract. `boot-block`'s
*Manual mode boot block pre-fill* deliberately requires pre-fill-**without**-Enter, so a
human can paste their task after the boot instructions. That is correct for bare
`git paw start`. `cmd_start_with_specs` reuses it, but there is no human going to type
anything — the task is the spec — so the agent is left holding an unsubmitted prompt.

Meanwhile `cli-resolution` requires the opposite for the boot-injection path generally
("inject literally, then submit with a separate `Enter` after a settle delay"). The two
requirements contradict each other today; nothing fails because the paths that each
describes have not met until now.

The task-prompt gap was explicitly foreseen: the existing requirement closes with "This
requirement does NOT cover spec-content / task-prompt injection … in a future change, may
be augmented by a format-native apply skill invocation."

## Goals / Non-Goals

**Goals:**

- A `--specs` launch without supervisor mode produces agents that actually start work.
- Resolve the spec-level contradiction explicitly rather than leaving two requirements
  that disagree.
- Reuse the existing seams (`submit_prompt_to_pane`, `build_task_prompt`,
  `resolve_approval_flags`) rather than writing parallel implementations.
- Close the approval-flag bypass on every launch path at once.

**Non-Goals:**

- Changing bare `git paw start`'s pre-fill behaviour. The paste-your-task flow is
  documented and wanted; only its leakage into spec-driven launch is wrong.
- Adding a supervisor to the non-supervisor path. The point is that this path works
  *without* one.
- The `cmd_*` library-surface refactor (v0.18.0). This change moves or re-exports
  `build_task_prompt` only as far as reaching it from `commands/start.rs` requires.
- Deploying bundled helpers into worktrees — that is `bundled-helper-discovery`.

## Decisions

**Scope the manual-mode requirement rather than deleting it.** The tempting fix is to
make every path submit. That would break the documented bare-launch flow where the boot
block is a *preamble* the human completes. Instead the requirement gains an explicit
boundary: pre-fill-only is the bare human-driven path; spec-driven launches submit. Both
behaviours survive and the contradiction is gone.

**Reuse `submit_prompt_to_pane`, do not re-derive the mechanism.**
`cli-resolution` already requires split-send plus a per-CLI `submit_delay_ms`, and the
supervisor path already implements it. The fix is to call the same seam, which also means
the CLI-agnostic guarantee ("the mechanism SHALL contain no hardcoded CLI names") holds
for free.

**Inject the same task prompt the supervisor path builds.** Not a reduced variant: an
agent launched with `--specs` should receive exactly what it would under `--supervisor`,
minus the supervisor itself. Any divergence here becomes a second ad-hoc workflow, which
is the defect being fixed.

**Task prompt is injected even when the broker is disabled.** The boot block is
broker-specific and correctly suppressed without one; the agent's *assignment* is not.
Keeping them coupled is why a broker-less spec launch would otherwise stay inert.

**Fix the approval-flag bypass as an added requirement, not a modified one.**
`supervisor-config`'s *Permission flag mapping* already specifies the resolution chain
correctly — the bug is callers skipping it. An ADDED requirement stating that every
launch path must apply the resolved flags says precisely that, and avoids restating a
ninety-line requirement verbatim in a delta.

## Risks / Trade-offs

- **Behaviour change on a shipped path.** A `--specs` launch that someone currently
  completes by hand will now submit on its own. This is the intended fix, but it is a
  visible change for anyone who had adapted to the broken behaviour; it belongs in the
  changelog in those terms.
- **Workers suddenly get permission flags.** On a config with `agent_approval = "auto"`,
  non-supervisor workers will start accepting edits where previously they prompted. That
  is what the config always asked for, but the effective change in a consumer's session is
  real and should be called out in the release notes rather than buried.
- **`build_task_prompt` lives in `main.rs`.** Reaching it from `commands/start.rs` needs a
  move or a re-export. Keep that minimal — the proper library-surface split is v0.18.0,
  and doing it here would mix a behaviour fix with a refactor.
- **Spec-level contradiction may have other instances.** Two requirements disagreed and
  nothing caught it because their paths never met. Worth a note for the pre-freeze spec
  audit: contradiction detection across capabilities is not currently part of any gate.

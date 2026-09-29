## Context

`cli_unavailable_line` (`src/supervisor/drive.rs`) is a small function: take the embedded
Claude profile's `cli_unavailable_markers`, take the last `LIVE_PROMPT_TAIL` non-blank lines
of the capture, return the first line containing a marker. `sweep_pane` then exits the loop
if the pane's role is the supervisor, or otherwise records an escalation keyed with
`approval_dedup_key(agent_id, &capture)`.

Each of those three inputs — the window, the marker source, the dedup key — was borrowed
from the approval machinery, where it was designed for a different object. The window was
designed for a permission modal, and GP-31 shows it is too narrow even there. The marker
source was designed for Claude-specific *shape* checks; unavailability is meant to cover any
CLI. The dedup key was designed for a prompt, whose command slice identifies it; a banner has
no command slice, so `extract_command_slice` falls back to the last `Bash(...)` line anywhere
in the transcript.

## Goals / Non-Goals

**Goals:**

- See a banner that sits above CLI-drawn chrome.
- Detect any CLI's unavailability using that CLI's own markers.
- One escalation per (agent, banner), stable across sweeps.
- Report honestly when no orchestrator pane was watched.

**Non-Goals:**

- Threading a per-CLI profile through the whole loop. Sixteen call sites read the embedded
  profile directly; replacing them is v0.19.0's `CliProfile` work. This change resolves one
  field at one site.
- Detecting orchestrator CLI death without a supervisor pane. That needs the v0.18.0
  orchestrator model; here the summary only has to stop overclaiming.
- Widening the pane-0 / supervisor-role logic. v0.18.0 replaces it with a typed pane role.

## Decisions

**Reuse `live-prompt-tail-window`'s trailing-region helper.** That change is required to
expose its bounded-trailing-region logic as a reusable helper precisely so this scan can
adopt it. One definition of "the tail, allowing for chrome below it" keeps the two scans from
drifting — the drift is how this gap appeared in the first place.

**Resolve markers per pane, for this field only.** The pane's CLI is already recorded per
agent in the roster and session state; resolve `cli_unavailable_markers` from it with the
existing `resolve_prompt_profile`. Keeping the change to this field avoids turning a bug fix
into the v0.19.0 refactor.

**A dedicated dedup key.** Key on `(agent_id, matched marker)`. Not the banner line itself —
a reset time or countdown in the line would make the key unstable — and not
`approval_dedup_key`, whose command slice is unrelated to the banner.

**Honest summary over new detection.** Without a supervisor pane there is nothing in the
roster to watch for the orchestrator's own death. Inventing a way to watch it would mean
widening exactly the logic v0.18.0 deletes. Saying so in the summary is cheap and correct.

## Risks / Trade-offs

- **Depends on an unmerged helper.** If `live-prompt-tail-window` changes shape in review,
  this change follows it. Sequencing handles it: wave 3, after that change merges.
- **Per-CLI resolution needs the CLI at sweep time.** If an agent's CLI cannot be resolved
  (an old session file without it), fall back to the embedded default — today's behaviour —
  rather than failing the sweep. In practice this means a pane's *first* sweep of a run
  always resolves the embedded default: the loop reads the pane's CLI from the `/status`
  snapshot (`status_for_sweep`), which is empty until the pane has published at least one
  status, so the override only takes effect from that pane's second sweep onward. This
  applies uniformly to the orchestrator pane too — it is resolved the same way, from a
  `"supervisor"` `/status` row seeded with its configured CLI, not hardcoded to the embedded
  default — so a `[clis.<name>]` override on the orchestrator's own CLI is honoured once it
  has published.
- **Both edit `drive.rs`.** `idle-pane-nudge-submission` edits it in wave 2. Different
  functions, but wave 1 proved that clean textual merges can still fail to build; this lands
  after that one merges, and Gate 2 runs on the merged tree.

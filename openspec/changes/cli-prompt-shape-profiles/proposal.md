## Why

git-paw claims to orchestrate any agent CLI, but the *shape* of Claude Code's terminal
UI is hard-coded across roughly a dozen constants in eight files, plus bash mirrors in
`sweep.sh`. Verified sites:

| Marker set | Site |
|---|---|
| `CLI_READY_MARKERS` (incl. `"Welcome to Claude Code"`, `"Bypassing Permissions"`, `"│ >"`) | `src/tmux/readiness.rs:23-30` |
| `APPROVAL_MARKERS` | `src/supervisor/permission_prompt.rs:53` |
| `MID_RESPONSE_MARKERS` | `src/supervisor/drive.rs:1786` |
| `PROMPT_BOILERPLATE` | `src/supervisor/manual_approvals.rs:316` |
| file-prompt regex, `Bash command` / `Bash(` header, `do you want to`, `don't ask again` | `src/supervisor/auto_approve.rs:120, 387, 394-417` |
| mode markers (`accept edits`, `bypass permissions`, `❯ 1. yes`) | `src/coordination/inventory.rs:246-256` |
| context-bloat regex, `Pasted text #[0-9]` | `assets/scripts/sweep.sh:269, 1035` |

For a non-Claude CLI this is not cosmetic, it is broken behaviour. On Codex, an approval
marker matches but `Bash command` never does, so command extraction fails and the whole
capture is classified — narration text then substring-matches the danger list, producing
chronic false escalations, while `input_box_text` never finds `│ >` so stranded-input
recovery is dead code. On Gemini, `don't ask again` is absent so a three-option prompt is
read as two-option and a durable grant is never offered — or the wrong option index is
selected.

Two in-flight changes have already started per-CLI configuration from the other end —
`unattended-boot-hardening` (per-CLI permission-mode flags) and
`drive-loop-actuator-robustness` (`[clis.<name>].submit_delay_ms`). This change
consolidates the *marker* surface on the same principle rather than adding a third
scattered mechanism.

## What Changes

- Introduce a **per-CLI prompt-shape profile**: a data file describing how one agent
  CLI's terminal surface looks — readiness markers, approval markers, live-prompt
  markers, mid-response markers, the command-header form, the file-prompt pattern, the
  option-line form, the broad-grant marker, the input sigil, the mode markers, and the
  stream/bloat/paste-buffer markers.
- The Claude Code profile ships embedded as the built-in default, so **behaviour with no
  configuration is byte-identical to today**.
- Profiles are resolvable and overridable per CLI, consistent with the existing
  `[clis.<name>]` mechanism.
- `sweep.sh` reads the same profile rather than carrying its own bash copies of the
  regexes, so the shell and Rust views of a prompt cannot drift.
- Detection **algorithms** are unchanged — only the source of the literals moves.

**Explicitly out of scope (owned by in-flight changes):** per-CLI permission/approval
*flags* (`unattended-boot-hardening`) and per-CLI *submit delay* /
nudge-submission robustness (`drive-loop-actuator-robustness`). This change consumes the
`[clis.<name>]` seam those establish; it does not redefine it.

**Not breaking:** the embedded Claude profile is the fallback for every field, so an
existing installation with no profile configuration behaves exactly as before.

## Capabilities

### New Capabilities

- `cli-prompt-profiles`: the per-CLI prompt-shape profile — its field set, its
  resolution order, the embedded default, the fallback-per-field rule, and the
  requirement that Rust and `sweep.sh` read one source.

### Modified Capabilities

- `approval-command-safety`: permission-prompt detection and prompt-shape identification
  source their markers from the resolved per-CLI profile rather than from compiled
  constants. The detection algorithm, the classification outcomes, and the safety gates
  are unchanged.

## Impact

- **Code:** `src/tmux/readiness.rs`, `src/supervisor/permission_prompt.rs`,
  `src/supervisor/drive.rs`, `src/supervisor/manual_approvals.rs`,
  `src/supervisor/auto_approve.rs`, `src/coordination/inventory.rs` — each replaces its
  compiled constant with a profile lookup. `assets/scripts/sweep.sh` +
  `assets/scripts/_paw_common.sh` — read the profile (the `discover_supervisor_int`
  helper at `sweep.sh:158` is the precedent for sourcing config from shell).
- **Parity:** the existing Rust↔shell live-gate parity test (referenced at
  `sweep.sh:1243`) collapses from "assert two copies agree" to "assert one source is
  read by both" — a strictly stronger guarantee.
- **MUST NOT move into the profile:** the danger list, protected-path rules, worktree
  boundary resolution, the broad-grant / arbitrary-code-runner rule, and the TOCTOU
  approval send gate. Those are security decisions, not prompt shape; a user- or
  worktree-editable copy of any of them is a privilege-escalation path for the very
  agent being gated. The profile describes *what the terminal looks like*, never *what
  is allowed*.
- **Enum-variant ripple:** none.
- **Backward compatibility:** embedded Claude profile is the default for every field;
  absent configuration reproduces current behaviour exactly.
- **Docs:** a supported-CLIs / profile reference chapter, the configuration reference,
  and a short authoring guide for adding a CLI profile.

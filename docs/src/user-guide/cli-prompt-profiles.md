# CLI Prompt-Shape Profiles

git-paw's supervisor and auto-approve detectors recognise an agent CLI's
terminal surface — its "ready" banner, its permission-prompt wording, its
mid-response footer — by matching known markers against captured pane text.
A **prompt-shape profile** is the data that describes what one CLI's
terminal *looks like*. It never describes what that CLI is *allowed to do* —
see [What a profile is not](#what-a-profile-is-not) below.

git-paw ships one profile, embedded in the binary: **Claude Code**. Every
detector falls back to it, field by field, so an installation with no
profile configuration behaves exactly as it always has.

## Why this exists

Before profiles, the markers describing Claude Code's terminal were
compiled into half a dozen Rust modules plus a bash mirror in the bundled
`sweep.sh` helper. For a non-Claude CLI this was not cosmetic — an approval
marker that never matched meant command extraction silently failed, and a
three-option prompt with different wording was read as a two-option one. A
prompt-shape profile gives each of those literals one home, so adding a new
CLI's markers is a data change, not a code change across six files.

## Resolution order

For a given CLI name, each field resolves independently:

1. The field's value in that CLI's `[clis.<name>].prompt_profile` table, if
   set.
2. Otherwise, the embedded Claude Code default.

This is **per-field**, not per-profile: a `[clis.<name>].prompt_profile`
table that sets only `approval_markers` still gets the embedded default for
every other field (readiness markers, mode markers, and so on). A partial
profile can never silently disable detection for the fields it omits, and a
CLI with no `prompt_profile` table at all — including one git-paw has never
heard of — resolves the full embedded default rather than an empty profile.

## Fields

| Field | Matches | Used by |
|---|---|---|
| `readiness_markers` | Substrings that positively identify the CLI's interactive ready state (as opposed to a bare shell prompt), gating boot-block injection. | `tmux::readiness::classify_pane_readiness` (boot-prompt injection gate) |
| `approval_markers` | Substrings indicating the CLI is waiting for an approval decision. | `supervisor::permission_prompt::classify_capture` (poll-loop permission detection) |
| `live_prompt_markers` | The confirmation-question lead-in (`"do you want to"`) and the cancel footer (`"esc to cancel"`) that anchor a LIVE (not scrolled-away) permission prompt. | `supervisor::auto_approve::is_live_prompt`, the approval send-gate re-confirm |
| `mid_response_markers` | Substrings identifying a pane actively producing a response (its interrupt footer showing) rather than sitting at its input box. | `supervisor::drive::pane_is_mid_response` (unattended drive loop) |
| `prompt_boilerplate_markers` | Lower-cased substrings marking a captured line as prompt boilerplate (the question / choices) rather than the command awaiting a decision. | `supervisor::manual_approvals` pattern extraction |
| `command_header_markers` | Lead-in forms of a command-confirmation header — a whole-line header form (e.g. `"Bash command"`) or an inline-embedding form ending in `(` (e.g. `"Bash("`). | `supervisor::auto_approve::extract_command_slice` |
| `file_prompt_pattern` | Regex source matching the lead-in of a filesystem-operation permission prompt, capturing the target path. | `supervisor::auto_approve::extract_path_from_file_prompt` |
| `option_line_pattern` | Regex source matching a numbered option line (`"1. …"`, `"2) …"`) once leading decoration is stripped. | `supervisor::auto_approve::is_option_line` |
| `broad_grant_marker` | Substring identifying a durable ("don't ask again") broad-grant option in a multi-option prompt. | `supervisor::auto_approve::detect_prompt_shape` |
| `input_sigils` | Prefixes identifying the CLI's input-box sigil (e.g. `">"`, `"❯"`). | `supervisor::drive::input_box_text` (stranded-input recovery) |
| `mode_accept_edits_markers` | Substrings identifying an accept-edits / bypass-permissions mode footer. | `coordination::inventory::detect_mode` |
| `mode_interactive_markers` | Substrings identifying a visible interactive prompt. | `coordination::inventory::detect_mode` |
| `stream_error_markers` | Phrases indicating the CLI's API call failed mid-turn (transport error, timeout, disconnect). | the bundled `sweep.sh` helper's stuck-shape detector |
| `context_bloat_pattern` | Regex source matching a context-bloat clear/compact hint, capturing the token count in thousands. | the bundled `sweep.sh` helper's stuck-shape detector |
| `paste_buffer_pattern` | Regex source matching a paste-aware CLI's "pasted text" acknowledgement. | the bundled `sweep.sh` helper's stuck-shape detector |

Every marker-list field is matched as a case-insensitive substring, mirroring
the behaviour each site had before profiles existed. Pattern fields are
Rust `regex` crate syntax.

## Authoring a profile for a new CLI

Add a `prompt_profile` table under that CLI's `[clis.<name>]` entry,
setting only the fields that differ from Claude's terminal surface:

```toml
[clis.my-agent]
command = "my-agent"

[clis.my-agent.prompt_profile]
readiness_markers = ["my-agent ready"]
approval_markers = ["proceed? (y/n)"]
broad_grant_marker = "remember this choice"
```

Every field you omit falls back to the embedded Claude default (see
[Resolution order](#resolution-order)) — start with just the fields you know
differ, and add more once you observe a detection gap in practice.

### What a profile is not

A prompt-shape profile describes what a CLI's terminal **looks like**. It
has **no field** through which it could express what a command is
**allowed to do**. The following stay compiled, are never read from any
profile or config file, and cannot be widened by one:

- the danger-command list
- protected-path / worktree-boundary rules
- the arbitrary-code-runner / broad-grant eligibility rule
- the approval send gate

This boundary exists because a profile is configuration a coding agent's own
worktree can influence (via `.git-paw/config.toml`); if a profile could
alter a permission decision, an agent could widen the very gate meant to
constrain it. If you find yourself wanting a profile field to affect
*whether* a command is approved rather than *how the prompt looks*, that is
a sign that a compiled rule change is needed instead.

### Do not ship a guessed profile

git-paw ships only the Claude Code profile — the one CLI its detection is
tested against. A profile for a CLI nobody has verified against live output
is worse than no profile at all: it produces confident *misdetection*
instead of an obvious fallback to Claude's markers. If you author a profile
for another CLI, verify it against that CLI's actual terminal output before
relying on it, and expect to iterate.

## `sweep.sh` reads the same profile

The bundled supervisor helper (`sweep.sh`) resolves its stuck-shape and
live-prompt regexes via the internal `git paw __prompt-profile` command
rather than carrying its own copies of the markers — the same profile every
Rust detector reads. This means an authored profile applies to both the
in-process supervisor and the shell helper without being declared twice.

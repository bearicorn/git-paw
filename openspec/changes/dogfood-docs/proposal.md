## Why

The v0.19.0 and v0.20.0 dogfood runs hit five problems that need no code change, only text the
orchestrator or the operator reads at the right moment. Each one cost a stall or a detour:

- **DF-16:** oh-my-zsh's `tmux` plugin defines `tmux` as an alias in the orchestrator's shell.
  The orchestrator skill's `tmux display-message` / `list-panes` snippets ran the alias and failed.
  `command tmux` works. `sweep.sh` is unaffected, because a script does not load the interactive
  shell's aliases.
- **DF-25, DF-28:** the orchestrator's own CLI permission check vetoed or stalled a
  `sweep.sh approve` the human had sanctioned. `git paw setup-cli` already writes an ask rule for
  `approve`, but neither the skill nor the supervisor guide points the operator at it when this
  happens.
- **DV-02:** the orchestrator's permission classifier refused `git paw purge --force` on a stopped
  session that recorded no agents. The way out was `git paw start --specs …`, which already
  replaces such a session (v0.19.0), but nothing said so.
- **DF-10:** the `openspec` CLI, which OpenSpec workers run, sends telemetry by default.
  `OPENSPEC_TELEMETRY=0` turns it off, and the guide does not mention it.
- **DF-42:** `git paw remove`'s `Kept branch … not in main` line was correct, and it was the missed
  clue that a merge had not landed (DF-52). The docs present it only as branch safety, not as a
  signal.

## What Changes

- **Orchestrator skill** (`assets/agent-skills/orchestrator/SKILL.md`):
  - its `tmux` commands become `command tmux …`, with a one-line reason;
  - the "Who decides a permission prompt" section names the ask-rule remedy;
  - a stopped empty session is replaced by `git paw start` with a selection, not purged;
  - the merge loop reads a `Kept branch` line from `remove` as "did not land".
- **User guide:**
  - `supervisor.md`: the `command tmux` note and the approve ask-rule remedy;
  - `session-lifecycle.md`: "start replaces a stopped empty session, no purge needed", and the kept
    line as a signal;
  - `spec-driven-launch.md`: the OpenSpec telemetry opt-out.
- **Content tests** pin each addition, so a later skill or guide edit cannot drop it silently.

No behaviour change: no code, no config and no CLI surface.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `skill-agent`: ADDED four orchestrator-skill (and matching guide) requirements: `command tmux`,
  the approve ask rule, replacing rather than purging a stopped empty session, and a kept branch
  read as not landed.
- `spec-providers`: ADDED the OpenSpec telemetry opt-out in the spec-driven-launch guide.

## Impact

- **Assets:** `assets/agent-skills/orchestrator/SKILL.md`. A text-only edit. The skill is
  re-exported on the next supervised launch.
- **Docs:** `docs/src/user-guide/supervisor.md`, `docs/src/user-guide/session-lifecycle.md`,
  `docs/src/user-guide/spec-driven-launch.md`.
- **Tests:** new assertions in `tests/orchestrator_skill_content.rs`, plus a new
  `tests/docs_dogfood_notes.rs` for the guide text. The existing
  `orchestrator_skill_content.rs:263` substring check (`tmux display-message -t <session>:0.<pane>`)
  still matches inside `command tmux …`. The language-agnostic skill audit must still pass, so the
  skill names no agent CLI and no shell plugin.
- **Code:** none.
- **Security / safety:** the approve text steers toward an ask rule and away from an allow rule,
  which matches `setup-cli`'s grant table (`cli-setup` "Allowlist entries are path-scoped to
  git-paw's helper scripts"). The telemetry note reduces data that leaves the machine.
- **Cross-change:** `supervisor-vocabulary-split` (this release) renames "supervisor" in user-facing
  text. Whichever lands second rebases these sentences onto its wording; the content tests match
  phrases that do not contain "supervisor".

## Context

Line numbers refer to `main` at v0.20.0 (`e213707`). `sweep.sh` is
`assets/agent-skills/orchestrator/scripts/sweep.sh`.

**Addressing (DV-06).**
- `capture <pane>` takes a raw index and passes it straight into a tmux target (`sweep.sh:628-636`).
  It has no numeric check and no roster check.
- `approve <pane>` also takes an index:
  - it requires digits (:714-717), because tmux resolves `.` to the active pane;
  - it then requires the index to be a roster `pane_index` (:723-733);
  - then it takes the pane claim, re-confirms the live prompt and runs the option-1 guard (:738-791).
- `tell <agent>` takes an agent id and passes it to `git paw __send --agent=<id>` (:804-836).
- `__send` finds the agent in the roster by id (`src/commands/send.rs:161-166`). It finds the pane
  by the core owner rule, the live surface whose working directory lies in the agent's worktree
  (`surface_of`, :244-262). It never uses a stored index.
- The shell has its own path-to-agent mapping (`resolve_agent_for_path`, `sweep.sh:1465-1490`) and
  reads the roster's recorded `pane_index` (:1517-1531), which can go stale after `remove` / `add`.
- The broker's agent-id rule needs a separator (`supervisor` or `<prefix>[-/]<name>`, mirrored at
  `sweep.sh:863`), so no agent id is all digits.
- The spec pins `approve` to a numeric argument
  (`openspec/specs/broker-agent-helper/spec.md:622-656`).

**`wedged` for a started turn (DV-05).**
- `deliver` refuses an agent that is mid-response before it sends (`send.rs:189-191`), so the pane is
  idle when typing starts.
- `submit_and_verify` (`src/supervisor/drive.rs:2365-2391`) reports `Delivered` when the post-Enter
  capture passes `decide_recovery_step`. That accepts a mid-response pane as submitted
  (`nudge_submitted`, :2453-2465).
- `deliver` then takes **one** more capture (`send.rs:224`). It returns `Delivered` only if
  `turn_evidence` counts more lines containing the message's first 20 characters, or more
  paste-placeholder lines, than before the send (:82-91, :203, :220-234). Otherwise it returns
  `Wedged("the input emptied but the message does not show as a submitted turn")`.
- So a turn that started is reported `wedged` when either of these holds:
  - The message renders without its first 20 characters on one line: the CLI wraps, reflows or
    abbreviates the echoed turn.
  - An earlier copy of the same text scrolls out of the capture while the new one appears, so the
    count stays equal.
- The mid-response evidence that `submit_and_verify` already saw is discarded.
- The spec says an input that merely emptied SHALL be `wedged` (`broker-agent-helper/spec.md:911-921`).
  A pane that went from idle to mid-response has not merely emptied.

**`status` misses a booting worker (DV-07).**
- `cmd_status` (`sweep.sh:838-883`) prints one row per entry in the broker's `/status` `agents`.
- It reads the discovery roster (`roster_agent_ids`, :1498-1512) only to classify phantoms
  (:862-869).
- A roster agent that has not yet published `agent.status` gets no row. The orchestrator sees N-1
  workers and may think one failed to launch, or launch it again.

**Menu glyph literals.**
- The profile carries `menu_cursor_glyphs` (default `❯`, `›`, `>`) and `menu_footer_markers`
  (`src/config/prompt_profile.rs:114-119`, defaults :273-274).
- `menu_showing` strips borders only, then strips a profile glyph and requires an option line after
  it (`src/supervisor/screen.rs:20-46`).
- Remaining literals:
  - `permission_prompt.rs::strip_decoration` trims a compiled set that includes `'❯'`
    (`src/supervisor/permission_prompt.rs:77-102`, the glyph at :94). It deliberately does not
    trim `>` (:75-76).
  - `readiness.rs::find_option_digit` trims `'│' | '❯' | '●' | '•' | '*' | '·' | '>'`
    (`src/tmux/readiness.rs:26-44`).
  - `sweep.sh` prefixes its footer and option regexes with the class `[[:space:]│─╭╮╰╯├┤┐└┘┌⎿❯●•*·]`
    (:297, :313, :318).
  - `sweep.sh`'s `PROFILE_VARS` (:384-387) does not include the menu fields, although
    `git paw __prompt-profile --field menu_cursor_glyphs` prints them (`src/main.rs:781-782`).
- The single-source rule is `cli-prompt-profiles/spec.md:113-126` (*Rust and the supervisor helper
  script read one source*).
- `readiness.rs::looks_like_bare_shell` also checks `❯` (:124-136). That is a login-shell prompt
  sigil (starship, pure), not an agent CLI's menu, so it is out of scope.

## Goals / Non-Goals

**Goals:** the orchestrator can use the ids it reads anywhere; every old pane-index call keeps
working; a started turn is never reported `wedged`; `status` accounts for every roster agent; no
CLI menu glyph is compiled in outside the profile.

**Non-Goals:**
- Changing `approve`'s safety sequence or the claim.
- `snapshot` and `detect-stuck`, which iterate all panes and need no target.
- The box-drawing and bullet characters in the decoration sets.
- The shell-prompt sigil check in readiness.

## Decisions

### D1 — Accept both spellings; resolve in Rust
We considered moving everything to agent ids, or everything to pane indices. Pane indices are
unstable across `remove` / `add` and mean nothing in the broker's vocabulary, and the orchestrator's
inputs (`status`, broker events, `__send` refusals) all speak in ids. So ids are the primary
spelling. Pane indices stay accepted, because the skill text, the drive loop's escalation hand-off
("run `sweep.sh approve`") and operators' muscle memory use them, and dropping them would break
every existing call.

A digits-only argument is a pane index; anything else is an id. This is unambiguous because no
agent id is all digits.

Resolution is one hidden subcommand, `git paw __pane (--agent=<id> | --pane=<n>)`. It resolves the
session and roster the way `__send` does and prints `<agent_id> <pane_index>`. It exits 3 with
`refused: <reason>` when the id is not a roster coding agent, the agent has no live surface, or the
pane is owned by no roster agent.
- The id direction reuses `surface_of`.
- The pane direction reuses the same owner rule, `surface::resolve_agent`.

The shell keeps no second copy of the owner rule, and a stale roster `pane_index` is never used to
target keys.

How each verb uses it:
- **`approve <agent|pane>`:** an id resolves to its index first. Then the existing sequence runs
  unchanged on the numeric index: digits check, roster check, claim, live re-confirm, option-1
  guard.
- **`capture <agent|pane>`:** an id resolves to its index. A numeric index is captured as today,
  with no roster check: `capture` is read-only, and the dashboard is a legitimate thing to read. A
  non-numeric, non-id argument is now refused (exit 2) instead of reaching tmux.
- **`tell <agent|pane>`:** a numeric argument resolves to its agent id, which goes to
  `__send --agent=<id>`. `__send` itself keeps addressing by id only.

Each verb's result line names both, e.g. `approved feat-x (pane 2, option 1)`.

### D2 — A pane that went from idle to mid-response is a submitted turn
In `deliver`, after `SubmitResult::Delivered`, the evidence becomes "new message text, OR a new paste
placeholder, OR the pane shows a profile mid-response marker". The pane was verified idle before
the send (`send.rs:189`), so a mid-response marker afterwards is our turn running.

The check is polled. The evidence is re-read up to `DELIVERY_EVIDENCE_POLLS = 3` times, one settle
delay apart, before `wedged`. A CLI that echoes the turn a moment late is therefore not reported
`wedged`. Worst-case added latency is three settle delays (4.5 s at the 1.5 s default), and only on
the path that would otherwise report `wedged`.

An input that emptied with no turn evidence and no mid-response marker is still `wedged`, as the
spec requires.

### D3 — `status` merges the roster
`cmd_status` passes the roster to its Python block, which, after the broker rows, prints a row for
every roster `branch_id` absent from `/status`. The row has status `not-seen` and age `-`.

A broker that cannot be reached keeps today's failure. The `not-seen` rows answer "who has not
reported yet", not "is the broker up". `--all` still adds phantoms.

### D4 — Glyphs are stripped only before an option line
The compiled decoration sets keep borders and bullets and lose `❯` (and, in readiness, `>`). Each
reader then strips one profile `menu_cursor_glyphs` entry at the start of the trimmed line, but only
when an option line (the profile's `option_line_pattern`) follows. That is exactly `menu_showing`'s
rule, so the three readers and `__send` agree.

Requiring the option line is what keeps `>` safe: `strip_decoration` never eats the `>` of a
command line such as `> /dev/null`, because that is not followed by `1.`.

- `permission_prompt.rs` already receives the pane's compiled profile.
  `classify_pane_readiness` already receives the pane's `CliPromptProfile`
  (`src/tmux/readiness.rs:144-148`) and passes it on to `find_option_digit`. The readiness option
  check stays its own digit-plus-`.`/`)` test (:31-38), because dialog answering is compiled provider
  data, never profile-driven (:140-142).
- `sweep.sh` adds `MENU_CURSOR_GLYPHS_REGEX` to `PROFILE_VARS`, resolved per pane through
  `__prompt-profile --field menu_cursor_glyphs`, with every glyph regex-escaped. Its option regexes
  become `^<decoration>*(<glyphs>)?[[:space:]]*<option>`.
- The shell's compiled fallback is the embedded default (`❯|›|>`), used only when the binary cannot
  answer, as for every other profile field.

## Risks / Trade-offs

- [An orchestrator passes an id to an older git-paw without `__pane`] → `sweep.sh` and the binary
  ship together through `init` / `start`. A missing `__pane` is reported as exit 4, as `tell` does
  for a missing `__send`.
- [`not-seen` rows look like failures during a normal boot] → the status name says only that the
  broker has not heard from it. The skill text says a worker reads `not-seen` until its first
  publish.
- [Polling adds latency to `wedged`] → bounded at three settle delays, on the failure path only.
- [A profile glyph that is also a bullet the CLI prints before list items] → it is stripped only
  before an option line, which is what a menu row is.

## Migration Plan

One commit per decision, the suite green after each: `__pane` and D1 (approve, capture, tell), D2,
D3, D4 (Rust readers, then `sweep.sh`). Rollback is a revert. No persisted state changes.

## Open Questions

- DV-05's original capture was not kept. The failure mode above is inferred from the code. The
  regression test reproduces both inferred shapes, a reflowed echo and a scrolled-out earlier copy.
  If a later dogfood capture shows a third shape, add it to that test.
- DV-07's report does not say whether the booting worker was missing from `status` only, or also
  from `snapshot`, which lists panes by command and title (`sweep.sh:466-480`). This change fixes
  `status`. If `snapshot` also misses it, that is a follow-up.

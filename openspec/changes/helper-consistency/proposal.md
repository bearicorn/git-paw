## Why

The v0.20.0 build dogfood and the v0.19.0 release reviews found four places where the bundled
orchestrator helper, `sweep.sh`, disagrees with itself or with the binary it drives:

- **Two addressing schemes (DV-06).** `sweep.sh tell <agent>` takes an agent id, the slugified
  branch. `sweep.sh approve <pane>` and `sweep.sh capture <pane>` take a tmux pane index. The
  orchestrator reads ids from `status` and the broker, then has to look up the pane index by hand
  to approve or capture. Pane indices also shift after `remove` / `add` (DV-19), so a remembered
  index can point at another agent.
- **`tell` reported `wedged` for a turn that had started (DV-05).** After the submit actuator reports
  the input submitted, `__send` takes one more capture and reports `delivered` only if more lines
  carry the message's first 20 characters, or a paste placeholder, than before. A turn that started
  but renders the message differently, or pushes an earlier copy off the captured region, reads as
  `wedged`. The orchestrator then re-sends, and the agent gets the message twice.
- **`status` missed a booting worker (DV-07).** `sweep.sh status` prints only the agents the broker
  has heard from. A worker that has not yet published its first `agent.status` is absent from the
  output, although the session roster lists it.
- **Menu glyph literals remain.** `__send` reads the selection-menu cursor glyphs from the agent's
  prompt profile (`menu_cursor_glyphs`). Three other readers still hard-code `❯` in their decoration
  sets:
  - the permission-prompt detector;
  - the launch-readiness dialog answerer;
  - `sweep.sh`'s option-line regexes.

  A CLI whose menu cursor is `›` or an override glyph is read correctly by `__send` and misread by
  these three.

## What Changes

- **One addressing scheme, with two spellings.** Every `sweep.sh` verb that targets one coding agent
  (`tell`, `approve`, `capture`) accepts either an agent id or a numeric pane index. A digits-only
  argument is a pane index; anything else is an agent id. No agent id is digits only, because the
  broker's id rule requires a separator.
  - An agent id resolves to the pane its worktree owns, by the same owner rule `__send` uses,
    through a new hidden `git paw __pane` lookup.
  - A pane index resolves to the agent whose worktree owns that pane.
  - Each verb's output names both the agent and the pane.
  - Existing pane-index calls keep working unchanged.
- **A started turn is a delivered message.** `__send` also counts a turn as submitted when the pane,
  idle before the send, shows a mid-response marker after it. It re-reads the evidence for a bounded
  window before it reports `wedged`.
- **`status` shows every roster agent.** A roster agent the broker has not yet heard from gets a row
  with status `not-seen` and no age. A phantom (on the broker, not in the roster) is still listed
  separately.
- **Menu cursor glyphs come from the profile everywhere.** The permission-prompt detector, the
  readiness dialog answerer and `sweep.sh` read the cursor glyphs from the profile of the pane's
  CLI. They strip a glyph only where an option line follows it, as `menu_showing` does. Box-drawing
  borders and bullets stay compiled; they are terminal rendering, not a CLI's prompt shape.
  `sweep.sh` gains `menu_cursor_glyphs` in its per-pane profile fields.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `broker-agent-helper`:
  - MODIFIED *sweep.sh approve refuses pane 0*: the target may also be an agent id.
  - ADDED the id-to-pane resolution rule.
  - ADDED `capture` by agent id.
  - ADDED `tell` by pane index.
  - ADDED a started turn counts as delivered.
  - ADDED `status` lists roster agents the broker has not seen.
- `cli-prompt-profiles`:
  - ADDED compiled detectors take the menu cursor glyph from the profile.
  - ADDED the helper script takes the menu cursor glyph from the profile.

## Impact

- **Code:**
  - `src/commands/send.rs` (delivery evidence; a shared resolver for `__pane`);
  - a hidden `__pane` subcommand in `src/cli.rs` and `src/main.rs`;
  - `src/supervisor/permission_prompt.rs` (`strip_decoration` and the option-line readers take the
    profile's glyphs);
  - `src/tmux/readiness.rs` (`find_option_digit` takes the profile's glyphs).
- **Assets:** `assets/agent-skills/orchestrator/scripts/sweep.sh` (`approve`, `capture`, `tell`,
  `status`, `derive_option_regexes`, `PROFILE_VARS`), plus its tracked copy under
  `.git-paw/scripts/` when it is re-exported. The orchestrator skill text documents the dual
  addressing.
- **Tests:** one or more per scenario. The existing `approve` and `tell` suites stay unchanged and
  green.
- **Docs:** the orchestrator skill's `sweep.sh` reference and the user guide's helper table.
- **Enum-variant ripple:** none.
- **Security / safety:**
  - `approve` keeps its numeric check, the roster check before any key, the claim, the live
    re-confirm and the option-1 broad-grant guard.
  - An id is never passed to tmux as a target. Only the resolved numeric index is, so `.` and
    pattern targets stay impossible.
  - Stripping a glyph only before an option line keeps a leading `>` in command text from being
    eaten.
- **Cross-version dependencies:**
  - `supervisor-vocabulary-split` (this release) renames the root identity to `orchestrator`. The
    `status` row for the root keeps whichever identity the broker reports.
  - `wire-format-dtos`: `status` reads the broker's `/status` shape. Its new `not-seen` rows come
    from the discovery roster, not the wire.
  - `dashboard-usage-view` moves the roster into `git paw status`. This change only touches
    `sweep.sh status`.

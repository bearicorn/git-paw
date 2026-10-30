## Why

In the v0.17.0 dogfood, paragraph-length messages typed into a worker's pane landed in the CLI's
paste buffer unsubmitted, three times; the sender had to capture the pane and send a second `Enter`
by hand. The drive loop already has a submit-and-verify actuator for its own nudges
(`supervisor/drive.rs::send_nudge`: text → per-CLI settle delay → separate `Enter` → capture → bounded
`C-u` + re-type + `Enter`), and v0.17.0 `idle-pane-nudge-submission` makes its verification work for
wrapped and paste-placeholder renderings. But that actuator is private to the loop.

Everything else that types a message into a worker goes around it: the supervisor skill's `/tell`
`send-keys` delivery mode and its paste-buffer recovery advice are raw `tmux send-keys` performed by
the driving agent, and `sweep.sh` has no send verb at all (its verbs are `snapshot`, `capture`,
`approve`, `status`, …). After v0.18.0 the driving agent is the operator's external session, so every
message it sends to a worker is a raw keystroke with no settle delay, no verification, no per-pane
claim and no check that a permission prompt is not showing — where typed text can select an option.

## What Changes

- **`sweep.sh tell <agent> <message>`** — the orchestrator's one way to type a message into a worker.
  It delegates to a hidden `git paw __send`, which runs the drive loop's submit-and-verify actuator
  against that agent's surface with that agent's CLI profile (`cli-profile-threading`): per-CLI settle
  delay, separate `Enter`, verification, bounded recovery.
- **Guarded like an approval.** `__send` targets only a coding-agent surface from the session roster,
  takes that surface's exclusive per-pane claim, and refuses — sending nothing — when the pane shows a
  live permission prompt or when it is run from a linked worktree (so a coding agent cannot type into
  a peer's pane). The message travels on stdin, never in argv.
- **Never pre-approved.** v0.17.0 `git paw setup-cli`'s grant table lists `sweep.sh tell` as never
  granted and puts it on an `ask` rule, like `sweep.sh approve`: it types into another agent's input,
  so every call prompts.
- **One result line and exit status:** `delivered`, `wedged` (still unsubmitted after the bounded
  attempts) or `refused: <reason>`.
- **The skill uses it.** The supervisor/orchestrator skill's `send-keys` delivery and paste-buffer
  recovery guidance call `sweep.sh tell` instead of raw `tmux send-keys`.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `broker-agent-helper`: new `sweep.sh tell` verb and its hidden `git paw __send` backend.
- `supervisor-skill-discipline`: typed delivery to an agent goes through `sweep.sh tell`.
- `cli-setup`: *Allowlist entries are path-scoped to git-paw's helper scripts* — `sweep.sh tell` is
  listed as never granted and gets an `ask` rule. The delta starts from v0.18.0
  `agent-hook-and-tmux-guards`'s text (which builds on `orchestrator-read-surface`'s, which starts
  from v0.17.0 `cli-setup-consent`'s) and adds only `tell`.

## Impact

- **Code:** `src/supervisor/drive.rs` (factor the nudge actuator out as a reusable function over the
  terminal port, unchanged behaviour), new hidden subcommand in `src/cli.rs` + its handler,
  `assets/scripts/sweep.sh` (+ tracked `.git-paw/scripts/sweep.sh`), the bundled supervisor skill,
  `src/cli_setup/facts.rs` (grant table: `tell` never granted, `ask` rule).
- **CLI:** one hidden subcommand (not in `git paw --help`); no public surface change.
- **Enum-variant ripple:** none.
- **Security / safety:** the verb types operator-authored text only into roster coding-agent
  surfaces (never the dashboard or any non-agent surface), under the same per-pane claim
  `sweep.sh approve` and the drive loop's nudges take, so it cannot race either; it refuses while a
  permission prompt is live, so a message can never answer one; and it refuses from a linked
  worktree, so a coding agent cannot use it to steer a peer. It is never granted by `setup-cli`. Clearing (`C-u`) happens only for text the actuator itself typed, as
  `idle-pane-nudge-submission` requires. Message text goes on stdin so it does not appear in `ps`.
  Export-agnosticism: the new `sweep.sh` verb and skill text name no CLI or stack.
- **Cross-version dependencies:** requires v0.17.0 `idle-pane-nudge-submission` (the verification and
  recovery this verb reuses), `classifier-retirement` (the per-pane claim now guards explicit
  approvals and the loop's nudges and submits; the skill relays prompts) and `cli-setup-consent`
  (the grant table), v0.18.0 `terminal-backend-seam` (surface addressing), `orchestrator-skill-export`
  (the skill it edits; that change also modifies the `/tell` requirements, so this change adds a
  requirement rather than modifying them), `orchestrator-read-surface` and
  `agent-hook-and-tmux-guards` (their MODIFIED grant-table requirement, adding `sweep.sh events` and
  then the full never-granted `sweep.sh` list, is the base of this change's `cli-setup` delta and
  must archive first; re-diff against its archived text before archiving this change), and this cycle's
  `cli-profile-threading` (per-agent profile and settle delay). v0.20.0 `skill-package-deploy`
  modifies the same grant-table requirement after this change and must carry `tell`.
- **Upgrade:** a repository that ran `git paw setup-cli claude --allowlist` under v0.17.0 or v0.18.0
  lacks the `tell` ask rule; `git paw doctor` reports the setup state out of date, and the user
  re-runs `git paw setup-cli claude --undo` then `--allowlist`. Until then `sweep.sh tell` prompts as
  any ungranted command does; git-paw never edits the grants itself. Overlap check: v0.17.0
  `idle-pane-nudge-submission` covers only the loop's own nudges, not sends by the driving agent.
- **Dependencies:** none.

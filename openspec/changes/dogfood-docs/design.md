## Context

File:line references are as of `a450f46`.

- **DF-16:**
  - The orchestrator skill tells the agent to run raw tmux in two places:
    `assets/agent-skills/orchestrator/SKILL.md:306-308` (`tmux display-message -t <session>:0.<pane>
    -p '#{pane_current_path}'`, also named as the helper-missing escape hatch at `:330-333`) and
    `:1388` (`tmux list-panes -t <session>:0 -F …`).
  - The orchestrator runs these through its CLI's shell tool, which loads the operator's
    interactive shell configuration, aliases included.
  - `sweep.sh`'s own `tmux send-keys` calls (`assets/agent-skills/orchestrator/scripts/sweep.sh:788-790`)
    run in a non-interactive bash script, where aliases are not expanded. That is why the helper
    worked while the inline snippets did not.
  - `tests/orchestrator_skill_content.rs:263` pins `tmux display-message -t <session>:0.<pane>` as
    a substring.
- **DF-25, DF-28:**
  - The skill's "Who decides a permission prompt" (`SKILL.md:642-664`) tells the orchestrator to run
    `sweep.sh approve` only on the human's word. The supervisor guide's "The explicit approve action"
    (`docs/src/user-guide/supervisor.md:1161-1166`) says the same.
  - `git paw setup-cli` writes an explicit ask rule for `sweep.sh approve` at both helper paths
    (`docs/src/user-guide/cli-setup.md:83-86`). That rule makes the CLI ask the human rather than
    let its own classifier decide, but nothing connects it to the veto the operator sees.
- **DV-02:**
  - `src/commands/start.rs:1161` discards a stopped session that records no agents and launches the
    selection. The tmux-orchestration requirement "start never silently ignores a launch selection",
    scenario "A stopped empty session is replaced by the selection", specifies this, and
    `docs/src/user-guide/session-lifecycle.md:375-383` documents it.
  - Neither the guide nor the skill says this makes `purge` unnecessary. The skill never mentions
    `purge` at all.
- **DF-10:**
  - `docs/src/user-guide/spec-driven-launch.md:55-75` (OpenSpec format) tells users to run
    `openspec archive`, and OpenSpec workers run `/opsx:*` commands that call the `openspec` CLI.
  - git-paw performs no telemetry (`supervisor-learnings` "No-telemetry privacy guarantee"). The
    guide is silent on the `openspec` CLI's own telemetry.
- **DF-42:**
  - `src/commands/remove.rs:188-197` detaches the worktree through the purge helper, which prints
    `Kept branch <b> at <tip> (<reason>)` (`src/commands/purge.rs:531`).
  - `session-lifecycle.md:339-357` presents the line as branch safety.
  - The skill's merge loop already has an ancestor check (`SKILL.md:1568-1578`). The kept line is a
    second, later signal for the same failure, and it is the one the orchestrator actually sees
    when it cleans up.

## Goals / Non-Goals

**Goals:** each dogfood finding is answered where the reader is when they hit it, and a test keeps
it there.

**Non-Goals:**
- Any behaviour change, such as git-paw setting `OPENSPEC_TELEMETRY=0` itself in the worker
  environment, or `sweep.sh` resolving `tmux` through `command`. See Open Questions.
- Naming a specific CLI's rule syntax in the skill. The skill must pass the language-agnostic audit.
- DF-26 (laptop sleep), which is not docs-only.

## Decisions

### D1 — `command tmux`, not an absolute path
`command` is POSIX, bypasses both aliases and functions, and still finds tmux on `PATH` wherever
it is installed (Homebrew, apt, Nix). An absolute path would not be portable. `\tmux` bypasses an
alias but not a function. The skill states the reason once and does not name the plugin. The guide
may name oh-my-zsh as the common cause.

### D2 — Point at the ask rule `setup-cli` already writes
The remedy is "make sure the orchestrator session's settings carry an ask rule for
`sweep.sh approve` (run `git paw setup-cli <cli>`, or add it yourself)". It is never an allow rule,
because an allow would let any agent that reaches the main checkout's settings press keys in a
peer's pane. The supervisor guide may show the `claude` rule text, as `cli-setup.md` already does.
The skill stays CLI-neutral.

### D3 — DV-02 is documented as "no purge needed", not as a purge workaround
The guide sentence goes under "Refilling an emptied session", after the stopped-empty-session
bullet. The skill sentence goes where the skill tells the orchestrator how to start a wave. Neither
tells the operator to loosen the permission check that refused `purge`; that refusal was
reasonable for a destructive command.

### D4 — DF-42 joins the existing landed-merge check
The skill adds one paragraph after the ancestor check (`SKILL.md:1575-1578`): a later `Kept branch`
line from `git paw remove` means the same thing, so archive and publish nothing. The guide adds one
sentence to step 5. It also names the second cause seen in PK-14: the worker committed after the
merge.

### D5 — One content-test file for the guide text
`tests/docs_dogfood_notes.rs` reads the three guide pages and asserts each phrase, so a later
reorganisation of the pages fails loudly instead of dropping the note. Skill assertions go into the
existing `tests/orchestrator_skill_content.rs`.

## Risks / Trade-offs

- [`command tmux` reads as noise to a user without the alias] → one sentence explains it, and the
  prefix is harmless everywhere.
- [The telemetry variable's name or default changes upstream] → the guide links OpenSpec's own
  documentation of it. The test pins only the variable name.
- [Content tests over-pin prose] → each asserts a short phrase or token (`command tmux`, `ask`,
  `OPENSPEC_TELEMETRY=0`, `Kept branch`), not sentences.

## Migration Plan

Text and tests only. Ship in any order relative to the other v0.21.0 changes. The skill re-exports
on the next supervised launch. Rollback is a revert.

## Resolved Questions (2026-11-17)

- **`OPENSPEC_TELEMETRY=0` by default in workers:** no. The guide documents it; a default would be
  its own change.
- **`DO_NOT_TRACK=1`:** checked against OpenSpec's docs during implementation. The spec requires
  only `OPENSPEC_TELEMETRY=0`.

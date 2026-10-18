## Why

Three v0.17 dogfood findings (`.git-paw/v0.17.0-dogfood-findings.md`) show agents stepping around
git-paw's own guard rails without anything in git-paw noticing:

- **GP-57, GP-62 — hook bypass.** Two Opus workers tried `git -c core.hooksPath=.git/hooks commit`
  and `git -c core.hooksPath=.git-paw/hooks commit`. Neither directory holds git-paw's hooks, which
  live in the common git directory (`src/agents.rs:664-683` writes `pre-commit`, `post-commit` and
  `pre-push` there), so the commit would silently skip the branch-guard pre-commit hook and the
  post-commit artifact publish. `git … --no-verify` skips the same pre-commit hook and the pre-push
  block. The orchestrator declined both by hand.
- **GP-58 — self-approval through tmux.** A worker wrote a probe that captured its own pane and, on
  seeing a permission prompt, ran `tmux send-keys -t %1 1 Enter`, scheduled with
  `tmux run-shell -b`: an agent answering its own prompt from outside the CLI.

Since v0.17 git-paw approves nothing by keystroke and pre-approves only its own helpers through
`git paw setup-cli claude --allowlist`, whose single `permissions.ask` pin today is
`Bash(.git-paw/scripts/sweep.sh approve *)` (`src/cli_setup/facts.rs:159-165`, planned at
`:319-334`). The bundled worker skill describes the hooks (`assets/agent-skills/coordination.md:22-28`,
`:197-203`) but never says not to bypass them, and says nothing about tmux.

## What Changes

- **setup-cli pins hook bypass and tmux injection to "ask".** The allowlist item's grant table gains
  guard rules in `permissions.ask`, in the same `Bash(<command words> *)` form as the `approve` pin,
  so the CLI prompts every time even when a broader allow rule matches:
  - hook bypass: `Bash(git -c *)`, `Bash(git * --no-verify*)`, `Bash(git commit -n *)`;
  - tmux keystroke and command injection: `Bash(tmux send-keys *)`, `Bash(tmux send *)`,
    `Bash(tmux run-shell *)`, `Bash(tmux run *)`, `Bash(tmux pipe-pane *)`, `Bash(tmux pipep *)`.
  They are ask entries only; no allow entry names anything but a git-paw helper. Each form is
  confirmed against the CLI before it ships (the fact gate), like every other grant-table entry.
- **Undo removes the guard rules it wrote.** Undo's shape check accepts an ask value equal to a
  guard rule; without that, undo would report git-paw's own guard entries as forged and leave them.
- **The worker skill says it plainly.** `coordination.md` states that an agent never redirects or
  bypasses git hooks and never drives tmux, and that its own permission prompts are answered by the
  human or orchestrator, never by the agent.
- **Upgrade path.** A repository that applied the allowlist under an earlier version sees doctor's
  setup check report it out of date (it lacks the guard rules), then runs
  `git paw setup-cli claude --undo` and `--allowlist`. That converges once
  `setup-cli-adopted-entries` (archive position 2) makes doctor count entries already in the file.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `cli-setup`: *Allowlist entries are path-scoped to git-paw's helper scripts* gains the guard ask
  rules; *Undo removes exactly what git-paw added* recognizes them.
- `skill-agent`: new requirement that the coordination skill forbids bypassing git hooks and driving
  tmux.

## Impact

- **Predecessors (implementation order):** archive position 8.
  - After `orchestrator-read-surface` (3), which MODIFIES *Allowlist entries are path-scoped to
    git-paw's helper scripts* (twelve allow entries, `sweep.sh events`). This change's MODIFIED
    delta is written on that text and keeps its twelve allow entries unchanged.
  - After `setup-cli-adopted-entries` (2): the upgrade path above relies on doctor counting
    pre-existing entries. That change does not modify any `cli-setup` requirement, so *Undo removes
    exactly what git-paw added* is modified here on the released v0.17.0 text.
  - No other v0.18 change modifies either `cli-setup` requirement; the new `skill-agent`
    requirement is ADDED.
- **Code:** `src/cli_setup/facts.rs` (a `GUARD_ASK_RULES` table with the rationale per rule;
  `planned_ask_entries` appends it; the `CLAUDE` row's evidence comment names the fact-gate steps),
  `src/cli_setup/undo.rs` (`classify`: a guard-rule shape), `src/cli_setup/render.rs` golden,
  `assets/agent-skills/coordination.md`.
- **Tests:** `cli_setup::facts::tests` (ask entries include every guard; no allow entry begins
  `Bash(git` or `Bash(tmux`), `cli_setup::plan::tests` (guards planned, a guard already present is
  not claimed), `cli_setup::undo::tests` (recorded guards removed; a forged non-guard `git` ask left
  on disk), the render golden, a `skills.rs` content test for the new section, and the existing
  `lang_agnostic_skill_audit` over `coordination.md`.
- **Docs:** `docs/src/user-guide/cli-setup.md` (the guard rules, why each, the upgrade path),
  `docs/src/user-guide/security-posture.md` (what the guards do and do not stop),
  `docs/src/user-guide/coordination.md` (the rule).
- **Enum-variant ripple:** none.
- **Security / safety:** narrows what an agent can do without a prompt; it approves nothing and
  classifies nothing (detect-and-escalate only, the CLI does the prompting). An ask rule is a
  speed bump for the natural forms, not a boundary: tmux command abbreviations and global options
  (`tmux -L x send-keys`), other tmux command-running verbs, `--config-env`, environment-based git
  config and combined short flags are not matched, and a CLI mode that skips ask rules skips these
  too. The sandbox remains the boundary for a rogue agent. Export-agnosticism: the rules name git
  hooks and the tmux session, which are git-paw's own mechanisms, and no consumer tooling.
- **Dependencies:** none.

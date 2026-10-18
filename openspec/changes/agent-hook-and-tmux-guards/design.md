## Context

`git paw setup-cli claude --allowlist` writes, into the main checkout's
`.claude/settings.local.json`, path-scoped allow rules for git-paw's helpers, protection rules
(`Edit(**/.git-paw/scripts/**)` deny, `Edit(**/.git-paw/config.toml)` ask) and one ask pin,
`Bash(.git-paw/scripts/sweep.sh approve *)` (`src/cli_setup/facts.rs:159-205`, planned by
`planned_ask_entries`, `:319-334`). The file is read by every CLI session in the repository, workers
and root alike (fact F1a). The fact gate (`facts.rs:240-264`) showed that an ask rule wins over a
matching allow rule in `manual`, `acceptEdits` and `auto` (F4), and that the space form
`Bash(<words> *)` matches bare and argument-bearing invocations without bleeding into a longer verb
(F2a-F2c5).

Undo's shape check (`src/cli_setup/undo.rs:439-460`) accepts only helper-script `Bash(...)` values
and file-tool rules under `.git-paw/`, so a recorded `Bash(tmux send-keys *)` would be reported as
forged and left on disk.

## Goals / Non-Goals

**Goals:**
- The natural forms of hook bypass and tmux injection prompt a human in every session of a
  repository that applied the allowlist.
- Workers are told plainly not to do either.
- Undo stays exact: it removes the guards it wrote and nothing else.

**Non-Goals:**
- A boundary against a determined rogue agent (the sandbox is that; see Risks).
- Deny rules. An ask keeps legitimate rare uses possible (a human answers) and matches the existing
  `approve` pin.
- Enforcing anything in git-paw itself: no classifier, no keystroke filter. The CLI prompts.
- Broker-less sessions get no coordination skill (it is rendered only with the broker), so the
  instruction does not reach them; the ask rules still do.

## Decisions

### D1 — One `GUARD_ASK_RULES` table, same form as the `approve` pin

A data table in `facts.rs`, each row a value plus a one-line reason, appended by
`planned_ask_entries` after the `approve` pin and before the protection ask rule. Derived from no
input, like the rest of the grant table.

### D2 — `git -c *`, not a literal `core.hooksPath` key

git config names are case-insensitive (`core.hookspath=` works as well as `core.hooksPath=`), and
other `-c` keys run programs (`core.sshCommand`, `core.fsmonitor`, `alias.<x>=!…`), which defeat the
same guard another way. One rule on the `-c` option covers every spelling; a legitimate `-c` in an
agent session is rare and costs one prompt. `--no-verify` can follow any subcommand and appear
anywhere after it, hence `git * --no-verify*` (a mid-pattern wildcard — the fact gate must confirm
it); `git commit -n *` catches the short form in first position.

### D3 — tmux: the three injection verbs and their aliases

`send-keys`/`send`, `run-shell`/`run`, `pipe-pane`/`pipep`: the verbs GP-58 used or that execute or
type into a pane. A blanket `Bash(tmux *)` would also catch abbreviations, global options and other
command-running verbs, but would prompt every read-only tmux query in the root session; scoped to
the injection verbs as requested. Recorded as an open question.

### D4 — Fact gate before shipping

Each guard is confirmed against the CLI, as F4 confirmed the `approve` pin: the named invocation
prompts in `manual`, `acceptEdits` and `auto`, including when a broader allow rule (`Bash(git *)`,
`Bash(tmux *)`) is present, and an unrelated command (`git commit -m x`, `tmux list-panes`) does not.
The `CLAUDE` row's evidence comment records the result. A form that does not match as intended is
dropped from the table, never shipped unverified.

### D5 — Undo recognizes guards by exact value

`classify` gains a `Shape::GuardRule` for an ask-kind entry whose value equals a `GUARD_ASK_RULES`
value. Exact membership, not a `Bash(git …)` pattern, so a forged record cannot use the guard shape
to remove a user's own `git` or `tmux` ask rule. Guards are not protection entries: they are removed
whenever recorded, present and equal.

### D6 — Upgrade

A v0.17 or v0.18-ORS record lacks the guards, so doctor reports the allowlist out of date
(`setup-cli-adopted-entries` makes "applied" mean "present in the file"). `--undo` then
`--allowlist` (or `--allowlist` alone, which adds only the missing entries) converges. git-paw never
rewrites the file on its own.

## Risks / Trade-offs

- [Speed bump, not a wall] → abbreviations (`tmux send-k`), global options (`tmux -L x send-keys`),
  other command-running tmux verbs (`new-window <cmd>`, `if-shell`, `set-hook`), `git --config-env`,
  `GIT_CONFIG_*` environment variables and combined short flags (`git commit -anm`) are not matched.
  The rules catch the forms agents actually wrote; the skill states the policy; the sandbox is the
  boundary. Documented in `security-posture.md`.
- [A CLI mode that skips ask rules] → the guards hold only where the CLI honors ask rules; the fact
  gate records the modes checked.
- [Root-session friction] → the user's own session at the root also prompts on these forms. The
  bundled supervisor skill still teaches raw `tmux send-keys` for a typed reply into an agent pane
  (`assets/agent-skills/supervisor.md:220-225`, `:293-298`), so an orchestrator following it gets
  one prompt per reply — the same treatment `sweep.sh approve` already has. `orchestrator-skill-export`
  (18) should route typed replies through a helper or MCP verb, or say the prompt is expected.
- [False positives] → `git commit -m "explain --no-verify"` prompts once; acceptable for an ask.

## Open Questions

- Whether to widen tmux to `Bash(tmux *)` once the orchestrator reads only through helpers and MCP
  (after `orchestrator-skill-export`), so read-only tmux queries in the root no longer matter.

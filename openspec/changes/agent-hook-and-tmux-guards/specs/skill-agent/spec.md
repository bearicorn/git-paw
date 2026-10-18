## ADDED Requirements

### Requirement: Coordination skill SHALL forbid bypassing git hooks and driving tmux

The embedded `assets/agent-skills/coordination.md` skill SHALL contain a section that states plainly that the coding agent:

1. SHALL NOT redirect or bypass the git hooks git-paw installs — no per-invocation `core.hooksPath` override (`git -c core.hooksPath=…`), no `--no-verify` (or `git commit -n`), and no change to the repository's hook configuration — and SHALL commit with a plain `git commit` from its worktree root, fixing what a hook reports instead of skipping it;
2. SHALL NOT drive tmux — no `tmux send-keys`, `tmux run-shell`, `tmux pipe-pane` or any other tmux command aimed at the session's panes, its own included;
3. SHALL NOT answer, or arrange for anything to answer, its own permission prompts: a prompt is answered by the human or the orchestrator, and an agent blocked on one waits, or publishes a question if it needs a decision.

The section SHALL state why: the hooks are how git-paw guards the branch and reports commits, so a bypassed hook silently drops the branch guard and the commit report; and keystrokes sent into a pane from outside its CLI bypass the human and orchestrator gate entirely. The section SHALL note that `git paw setup-cli` pins these command forms to an always-ask rule, so trying them prompts a human. The wording SHALL stay project-agnostic: it SHALL name only git-paw's own mechanisms (its git hooks and the tmux session) and SHALL NOT name any consumer tool, command or convention.

#### Scenario: Coordination skill forbids hook bypass

- **WHEN** the embedded `coordination.md` skill is inspected
- **THEN** it SHALL contain the substrings `core.hooksPath` and `--no-verify`
- **AND** prose stating the agent must not redirect or bypass git hooks

#### Scenario: Coordination skill forbids driving tmux

- **WHEN** the embedded `coordination.md` skill is inspected
- **THEN** it SHALL contain the substrings `tmux send-keys` and `tmux run-shell`
- **AND** prose stating the agent must never drive tmux or answer its own permission prompts

#### Scenario: The section stays export-agnostic

- **WHEN** the rendered coordination skill is checked by the bundled-asset no-leak audit
- **THEN** the new section SHALL pass it

## 1. Orchestrator skill (`assets/agent-skills/orchestrator/SKILL.md`)

- [ ] 1.1 DF-16: `command tmux` at the pane-resolution command (~306), its escape-hatch mention (~332) and the `list-panes` inventory line (~1388); one sentence saying the prefix bypasses a shell alias or function named `tmux` (no CLI or plugin named)
- [ ] 1.2 DF-25/28: in "Who decides a permission prompt", one paragraph: if your own CLI refuses or stalls a `sweep.sh approve` the human sanctioned, tell the human to give the orchestrator session an ask rule for `sweep.sh approve` (`git paw setup-cli` writes one); never an allow rule
- [ ] 1.3 DV-02: where the skill starts a wave, a stopped session that records no agents is replaced by `git paw start` with the selection; do not purge it first
- [ ] 1.4 DF-42: after the landed-merge check (~1575), a `Kept branch` line from `git paw remove` means the branch did not land; archive and publish nothing for it

## 2. User guide

- [ ] 2.1 `supervisor.md`: the `command tmux` note (oh-my-zsh's `tmux` plugin as the usual cause), next to the pane-resolution and inventory commands
- [ ] 2.2 `supervisor.md` "The explicit approve action": the ask-rule remedy, linking `cli-setup.md`; may show the `claude` rule text
- [ ] 2.3 `session-lifecycle.md` "Refilling an emptied session": `git paw start --specs …` replaces a stopped empty session with no `purge`, which helps when an agent's permission check refuses `git paw purge --force`
- [ ] 2.4 `session-lifecycle.md` remove step 5: a `Kept branch` line for a branch you believed merged means the merge did not land, or the worker committed after it
- [ ] 2.5 `spec-driven-launch.md` OpenSpec format: the `openspec` CLI sends telemetry by default, git-paw sends none, and `OPENSPEC_TELEMETRY=0` set in the shell profile turns it off for every pane; link OpenSpec's own documentation

## 3. Tests

- [ ] 3.1 `tests/orchestrator_skill_content.rs`: `tmux_commands_bypass_shell_aliases`, `approve_step_names_an_ask_rule_not_an_allow_rule`, `a_stopped_empty_session_is_replaced_not_purged`, `a_kept_branch_on_remove_means_the_merge_did_not_land`
- [ ] 3.2 `tests/docs_dogfood_notes.rs`: `supervisor_guide_names_the_approve_ask_rule`, `session_lifecycle_replaces_a_stopped_empty_session_without_purge`, `session_lifecycle_reads_a_kept_branch_as_not_landed`, `spec_driven_launch_documents_the_openspec_telemetry_opt_out`
- [ ] 3.3 `tests/lang_agnostic_skill_audit.rs` still passes for every backend
- [ ] 3.4 Manual: in an interactive zsh with `alias tmux='tmux attach'` (or oh-my-zsh's plugin), the skill's `command tmux display-message …` line prints the pane path

## 4. Gates

- [ ] 4.1 `just check`; `mdbook build docs/`

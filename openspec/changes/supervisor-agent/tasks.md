## 1. Update coordination.md skill template

- [ ] 1.1 Add "Publish agent.status when you start working on a new file" instruction to `assets/agent-skills/coordination.md`
- [ ] 1.2 Add "Publish agent.status after editing or creating any file (populate modified_files)" instruction
- [ ] 1.3 Add "Publish agent.status after each git commit" instruction
- [ ] 1.4 Add `### Cherry-pick peer commits` section with the exact `git cherry-pick <commit>` command
- [ ] 1.5 Verify existing four operations (status, artifact, blocked, messages poll) are unchanged
- [ ] 1.6 Add "MUST NOT push to remote — commit to your worktree branch only" constraint

## 2. Extend WorktreeAssignment with inter-agent rules

- [ ] 2.1 Add `inter_agent_rules: Option<String>` field to `WorktreeAssignment` in `src/agents.rs`
- [ ] 2.2 Update `generate_worktree_section()` to append `## Inter-Agent Rules` subsection when field is `Some`
- [ ] 2.3 Rules section SHALL appear after skill content and before `<!-- git-paw:end -->`
- [ ] 2.4 When `inter_agent_rules = None`, output is identical to pre-change output (no regression)
- [ ] 2.5 Define `build_inter_agent_rules(branches: &[&str]) -> String` helper in `src/agents.rs` that generates the standard rules block with the branch list for file ownership

## 3. Implement cmd_supervisor() in src/main.rs

- [ ] 3.1 Add `pub async fn cmd_supervisor(args: &StartArgs, config: &PawConfig) -> Result<(), PawError>` function
- [ ] 3.2 Step 1: Load supervisor CLI from `config.supervisor.cli`, falling back to `config.default_cli`, error if neither is set
- [ ] 3.3 Step 2: Resolve branches from `--branches`, `--from-specs`, or spec scan
- [ ] 3.4 Step 3: Create worktrees for each branch using existing `git::ensure_worktree()`
- [ ] 3.5 Step 4: For each branch, call `agents::setup_worktree_agents_md()` with spec content, coordination skill (rendered), and inter-agent rules
- [ ] 3.6 Step 5: Build tmux session: pane 0 = `git-paw __dashboard`, panes 1-N = coding agents
- [ ] 3.7 Step 6: Inject `GIT_PAW_BROKER_URL` via `tmux set-environment -t <session>` before pane creation
- [ ] 3.8 Step 7: For each agent pane, construct launch command: `<cli_cmd> <approval_flags>` using `config::approval_flags()`
- [ ] 3.9 Step 8: Execute tmux session in detached mode
- [ ] 3.10 Step 9: Sleep ~2 seconds for pane boot
- [ ] 3.11 Step 10: Inject initial prompt per pane via `tmux send-keys -t <session>:<pane> "<prompt>" Enter`
- [ ] 3.12 Step 11: Resolve supervisor skill via `skills::resolve("supervisor")`, write rendered AGENTS.md to supervisor working dir
- [ ] 3.13 Step 12: Start supervisor CLI in foreground (blocking `std::process::Command::new(&supervisor_cli).spawn()?.wait()`)

## 4. Initial prompt derivation

- [ ] 4.1 If the agent has a spec, extract the spec title/description as the initial prompt
- [ ] 4.2 If no spec, use the default prompt: `"Begin your assigned task as described in AGENTS.md."`
- [ ] 4.3 Ensure the prompt is properly shell-escaped before passing to `tmux send-keys`

## 5. Unit tests

- [ ] 5.1 Test: `generate_worktree_section()` with `inter_agent_rules = Some(...)` includes `## Inter-Agent Rules`
- [ ] 5.2 Test: `generate_worktree_section()` with `inter_agent_rules = None` has no rules section (regression)
- [ ] 5.3 Test: `build_inter_agent_rules(branches)` output contains file ownership constraint
- [ ] 5.4 Test: `build_inter_agent_rules(branches)` output contains "MUST NOT push" constraint
- [ ] 5.5 Test: `build_inter_agent_rules(branches)` output contains "agent.status" proactive publishing requirement
- [ ] 5.6 Test: `build_inter_agent_rules(branches)` output contains "match spec field names exactly"
- [ ] 5.7 Test: Updated `coordination.md` embedded content contains `git cherry-pick`
- [ ] 5.8 Test: Updated `coordination.md` embedded content contains "after each commit"
- [ ] 5.9 Test: Updated `coordination.md` still contains all four original operations

## 6. Integration test (dry-run)

- [ ] 6.1 Test: `cmd_supervisor()` with `--dry-run` prints expected pane commands without executing tmux
- [ ] 6.2 Test: approval flags appear in dry-run output when `agent_approval = "full-auto"`

## 7. Quality gates

- [ ] 7.1 `cargo fmt` clean
- [ ] 7.2 `cargo clippy --all-targets -- -D warnings` clean
- [ ] 7.3 `cargo test` — all tests pass (new + existing)
- [ ] 7.4 `cargo doc --no-deps` — no warnings
- [ ] 7.5 `just check` — full pipeline green
- [ ] 7.6 Verify `worktree-agents-md` unit tests still pass (no regression from `inter_agent_rules` addition)

## 8. Handoff readiness

- [ ] 8.1 `cmd_supervisor()` is callable from `src/main.rs` start handler
- [ ] 8.2 `assets/agent-skills/coordination.md` is updated and embedded via `include_str!`
- [ ] 8.3 `WorktreeAssignment.inter_agent_rules` is documented with a doc comment
- [ ] 8.4 No changes to files outside `src/main.rs`, `src/agents.rs`, `assets/agent-skills/coordination.md`, and test files
- [ ] 8.5 Commit with message: `feat(supervisor): implement auto-start flow with agent launch and coordination rules`

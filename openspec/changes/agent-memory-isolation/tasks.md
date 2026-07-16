## 1. Protected-path derivation

- [x] 1.1 Implement the protected-path set derivation (`~/.claude`, `CLAUDE_CONFIG_DIR`, `[clis.<name>].settings_path` parents, `projects/**/memory` subtrees, repo-root `.claude/` + `.git-paw/` for embedded worktrees) with canonicalization and fail-closed syntactic matching; unit tests per spec scenarios

## 2. Classifier wiring

- [x] 2.1 Wire the protected-path rule into classification at danger-list precedence, covering filesystem-prompt paths (`extract_path_from_file_prompt`) and shell command-slice write targets; reads unaffected; tests: operator-memory write, settings append, in-worktree unaffected, read not matched, `..`-escape caught
- [x] 2.2 Mirror the rule in `assets/scripts/sweep.sh` classify (lockstep); `bash -n` after editing; extend `tests/sweep_sh_classify.rs` with a protected-path fixture

## 3. Skill guidance

- [x] 3.1 Add the memory-isolation section to `assets/agent-skills/coordination.md` (worktree-scoped artifacts, off-limits operator dirs, question-instead-of-write); engine/CLI-agnostic wording
- [x] 3.2 Add the out-of-worktree write violation procedure to `assets/agent-skills/supervisor.md` (scoped feedback, escalate on repeat)
- [x] 3.3 Update the pinned skill-content tests (`skills.rs`, `*_skill_content.rs`) for both files — grep for pinned literals before editing

## 4. Docs

- [x] 4.1 Coordination + supervisor mdBook chapters: memory isolation and the violation procedure
- [x] 4.2 Configuration reference: note that `settings_path` entries feed the protected-path set
- [x] 4.3 `mdbook build docs/` passes

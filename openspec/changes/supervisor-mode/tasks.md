## 1. Add --supervisor flag to CLI

- [ ] 1.1 Add `supervisor: bool` field to `StartArgs` struct in `src/cli.rs` with `#[arg(long, default_value_t = false)]`
- [ ] 1.2 Add `help` string: "Enable supervisor mode for this session"
- [ ] 1.3 Verify `start --supervisor` parses with `supervisor = true`
- [ ] 1.4 Verify `start` without `--supervisor` parses with `supervisor = false`
- [ ] 1.5 Verify `start --supervisor --cli claude --branches feat/a` parses all fields correctly

## 2. Implement supervisor mode resolution chain

- [ ] 2.1 In `src/main.rs` start handler, implement `resolve_supervisor_mode(args, config) -> Result<bool, PawError>`
- [ ] 2.2 Step 1: if `args.supervisor` is `true`, return `true` (no prompt)
- [ ] 2.3 Step 2: if `config.supervisor` is `Some(s)` and `s.enabled == true`, return `true` (no prompt)
- [ ] 2.4 Step 3: if `config.supervisor` is `Some(s)` and `s.enabled == false`, return `false` (no prompt)
- [ ] 2.5 Step 4: if `args.dry_run` is `true`, return `false` (skip prompt)
- [ ] 2.6 Step 5: if `config.supervisor` is `None`, prompt "Start in supervisor mode? (y/n)" using `dialoguer::Confirm` and return user's answer
- [ ] 2.7 Route to `cmd_supervisor()` if result is `true`, otherwise `cmd_start()`

## 3. Implement merge ordering

- [ ] 3.1 Add `pub fn build_dependency_graph(messages: &[(u64, BrokerMessage)]) -> HashMap<String, Vec<String>>` in `src/main.rs` or a new `src/supervisor.rs`
- [ ] 3.2 For each `agent.blocked` message, add edge `payload.from → agent_id` (B must merge before A)
- [ ] 3.3 Add `pub fn topological_merge_order<S: std::hash::BuildHasher>(graph: &HashMap<String, Vec<String>, S>, all_agents: &[String]) -> Vec<String>` — returns merge order, logs warning on cycles
- [ ] 3.4 When cycle detected, log a warning with branch names involved via `eprintln!` and return all branches in arbitrary order (graceful degradation)
- [ ] 3.5 Update spec documentation to reflect improved implementation (cycle warning + fallback instead of error return)
- [ ] 3.6 Integrate merge order into supervisor post-verification flow: iterate over ordered branches, run test command after each merge

## 4. Purge unmerged commit warning

- [ ] 4.1 In `src/main.rs` purge handler, resolve default branch via `git symbolic-ref refs/remotes/origin/HEAD`, fallback to `"main"`
- [ ] 4.2 For each worktree branch, run `git log <branch> --not <default> --oneline` and count output lines
- [ ] 4.3 Collect branches with non-zero commit counts
- [ ] 4.4 If any exist, print warning: "Warning: N branch(es) have unmerged commits:" followed by per-branch counts
- [ ] 4.5 If `--force` is set, print the warning but proceed without prompting
- [ ] 4.6 If not `--force`, prompt "Purge is irreversible. Continue? (y/N)" using `dialoguer::Confirm`
- [ ] 4.7 If user declines, print "Purge cancelled." and exit with code 0
- [ ] 4.8 If no unmerged commits exist, proceed without any warning (existing behavior)

## 5. Update git paw init for supervisor prompts

- [ ] 5.1 In `src/init.rs`, after existing init steps, check if config was already present; skip prompts if so
- [ ] 5.2 Prompt: "Enable supervisor mode by default?" using `dialoguer::Confirm`
- [ ] 5.3 If yes: prompt "Test command (e.g. 'just check', leave empty to skip):" using `dialoguer::Input`
- [ ] 5.4 If yes: append `[supervisor]\nenabled = true\n` (and `test_command = "..."` if non-empty) to config
- [ ] 5.5 If no: append `[supervisor]\nenabled = false\n` to config
- [ ] 5.6 Also add `.git-paw/session-summary.md` to `.gitignore` (alongside `.git-paw/logs/`)

## 6. Unit tests

- [ ] 6.1 Test: `start --supervisor` parses with `supervisor = true`
- [ ] 6.2 Test: `start` without flag parses with `supervisor = false`
- [ ] 6.3 Test: `resolve_supervisor_mode` with `--supervisor` flag returns `true` regardless of config
- [ ] 6.4 Test: `resolve_supervisor_mode` with `enabled = true` in config returns `true` without prompting
- [ ] 6.5 Test: `resolve_supervisor_mode` with `enabled = false` in config returns `false` without prompting
- [ ] 6.6 Test: `resolve_supervisor_mode` with `--dry-run` and no supervisor section returns `false` without prompting
- [ ] 6.7 Test: `build_dependency_graph` produces correct edges from a list of blocked messages
- [ ] 6.8 Test: `topological_merge_order` with chain A→B returns `[B, A]`
- [ ] 6.9 Test: `topological_merge_order` with no dependencies returns all branches
- [ ] 6.10 Test: `topological_merge_order` with cycle returns `Err` containing both cycle members
- [ ] 6.11 Test: purge with no unmerged commits produces no warning output
- [ ] 6.12 Test: purge warning lists branch names and commit counts
- [ ] 6.13 Test: init yes-to-supervisor writes `enabled = true` to config
- [ ] 6.14 Test: init no-to-supervisor writes `enabled = false` to config
- [ ] 6.15 Test: init adds `.git-paw/session-summary.md` to `.gitignore`
- [ ] 6.16 Test: `start --help` contains `--supervisor`

## 7. Quality gates

- [ ] 7.1 `cargo fmt` clean
- [ ] 7.2 `cargo clippy --all-targets -- -D warnings` clean
- [ ] 7.3 `cargo test` — all tests pass (new + existing)
- [ ] 7.4 `cargo doc --no-deps` — no warnings
- [ ] 7.5 `just check` — full pipeline green
- [ ] 7.6 Verify all existing CLI parsing tests still pass

## 8. Handoff readiness

- [ ] 8.1 `--supervisor` flag is documented in `start --help`
- [ ] 8.2 `resolve_supervisor_mode` is callable and tested
- [ ] 8.3 Merge ordering functions are in a testable module (not inlined in a handler)
- [ ] 8.4 Purge unmerged-commit check runs before any destructive operations
- [ ] 8.5 Modified files: `src/cli.rs`, `src/main.rs`, `src/init.rs` only (plus test files)
- [ ] 8.6 Commit with message: `feat(cli): add --supervisor flag, resolution chain, purge safety, and merge ordering`

## 1. Provision helpers at worktree setup

- [x] 1.1 In `attach_agent` (shared by `start` and `add`), after `create_worktree`, create `<worktree>/.git-paw/scripts/` and write the bundled `broker.sh` (broker enabled) and `docs-fetch.sh` (`docs_base_url` set) from the same embedded assets `git paw init` uses; `chmod +x` each
- [x] 1.2 Make it idempotent — always (re)write on attach so a reused worktree refreshes to the running binary's version

## 2. Tests

- [x] 2.1 After a worktree setup with broker enabled, `<worktree>/.git-paw/scripts/broker.sh` exists and is executable
- [x] 2.2 `docs-fetch.sh` is provisioned iff `docs_base_url` is configured
- [x] 2.3 Re-attaching an existing worktree refreshes the scripts without error (idempotence)

## 3. Docs

- [x] 3.1 Note in the coordination / agents-md chapter that helpers are auto-provisioned into worktrees (agents no longer copy them by hand)

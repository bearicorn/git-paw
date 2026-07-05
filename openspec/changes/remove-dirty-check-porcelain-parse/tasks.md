## 1. Robust parsing

- [ ] 1.1 Change `git::uncommitted_files` to run `git status --porcelain -z` and split on NUL, consuming the second path of rename/copy entries
- [ ] 1.2 Extend `is_managed_path` (`src/agents.rs`) to classify the `.git-paw/` subtree as git-paw-managed

## 2. Tests

- [ ] 2.1 Regression test: a newline-bearing path stays a single record (no phantom `**WARNING:` changed-file entry)
- [ ] 2.2 Test that `is_managed_path` classifies `.git-paw/`-subtree paths as managed

## 3. Integration

- [ ] 3.1 Rebase `fix/remove-dirty-check-flake` @ `38918e2` onto `feat/v0.10.0-specs` and mark these tasks complete

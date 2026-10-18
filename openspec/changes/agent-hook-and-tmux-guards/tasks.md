## 1. Fact gate

- [ ] 1.1 Against the installed CLI (same procedure as F4), confirm each guard form prompts in `manual`, `acceptEdits` and `auto`, also when `Bash(git *)` / `Bash(tmux *)` is allowed, and that `git commit -m x` and `tmux list-panes` do not; drop any form that does not match as intended; record the outcome in the `CLAUDE` row's evidence comment

## 2. Grant table and undo

- [ ] 2.1 `src/cli_setup/facts.rs`: `GUARD_ASK_RULES` (value + reason per rule, design D2/D3); `planned_ask_entries` appends them after the `approve` pin
- [ ] 2.2 Facts tests: `guard_rules_are_asked` (all nine present), `no_allow_entry_names_git_or_tmux`; existing allow-entry tests unchanged (twelve entries)
- [ ] 2.3 Plan tests: guards planned in the same file write; `a_guard_rule_already_present_is_not_planned`; update the render golden
- [ ] 2.4 `src/cli_setup/undo.rs::classify`: `Shape::GuardRule` for an ask entry equal to a `GUARD_ASK_RULES` value; not subject to the protection-retention rule
- [ ] 2.5 Undo tests: `recorded_guard_rules_are_removed`, `a_forged_non_guard_git_ask_is_reported_and_left_on_disk`; shell test: apply then undo leaves a pre-existing user `Bash(tmux send-keys *)` in place

## 3. Worker skill

- [ ] 3.1 `assets/agent-skills/coordination.md`: new section (next to "Commands you must not run") — never redirect or bypass git hooks, never drive tmux, never answer your own prompt; why; the setup-cli ask pins
- [ ] 3.2 `skills.rs` content tests for the two scenarios; the no-leak audit covers the section

## 4. Docs and gates

- [ ] 4.1 `docs/src/user-guide/cli-setup.md` (guard rules, reasons, upgrade path: doctor out of date → `--undo` → `--allowlist`), `docs/src/user-guide/security-posture.md` (what the guards stop and what they do not), `docs/src/user-guide/coordination.md`; `mdbook build docs/`
- [ ] 4.2 At archive: update the `cli-setup` `## Purpose` paragraph to mention the guard ask rules (a delta cannot carry it)
- [ ] 4.3 `cargo fmt`, `just lint`, `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast` (real exit code), `just deny`
- [ ] 4.4 Security and safety review (`security-and-safety-review`): ask-only, no allow outside helpers, undo exact, limits documented
- [ ] 4.5 `openspec validate agent-hook-and-tmux-guards --strict`

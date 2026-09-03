## 1. Sequencing precondition

- [ ] 1.1 Confirm `unattended-boot-hardening` has landed (each CLI's permission mode resolves to an explicit flag).
- [ ] 1.2 Confirm `worker-guidance-skill-export` has landed — it exports the replacement guidance as skills and edits the same supervisor-skill permission section this change corrects (D4).
- [ ] 1.3 Do not start the removal before both. Removing the machinery first would leave the guidance pointing at it.

## 2. Remove the inert seeding

- [ ] 2.1 `src/supervisor/curl_allowlist.rs` — remove the `allowed_bash_prefixes` writer and its merge logic.
- [ ] 2.2 `src/supervisor/dev_allowlist.rs` — remove the seeding path. Check whether the stack-preset data has a consumer beyond seeding before deleting the data along with the writer.
- [ ] 2.3 `src/supervisor/worktree_allowlist.rs` — remove per-worktree seeding and `ensure_claude_dir_excluded`.
- [ ] 2.4 `src/commands/supervisor.rs:469-475` and `src/commands/recover.rs:52-53` — remove the call sites.
- [ ] 2.5 For each deletion, check for a second caller before removing — `ensure_claude_dir_excluded` and the stack presets are the known adjacency risks.
- [ ] 2.6 Add no replacement mechanism: no config field, no strategy declaration, no seeding code (D2).

## 3. Verify the removal

- [ ] 3.1 Test: no permission grants are written into any CLI settings file at session start.
- [ ] 3.2 Test: no vendor-specific settings directory is created inside a provisioned worktree, and no exclusion entry is added for one.
- [ ] 3.3 Test: a pre-existing settings file is left byte-identical after a session start and is not deleted (D3).

## 4. Protect the adjacencies

- [ ] 4.1 Verify `core-memory-isolation` still derives its set from the parent directory of each configured `settings_path` (D5).
- [ ] 4.2 Test: a configured CLI's settings-path parent still contributes to the memory-isolation set.
- [ ] 4.3 Test: `[clis.<name>].settings_path` still parses and round-trips.

## 5. Correct the guidance

- [ ] 5.1 Remove the supervisor skill's claim that the seeded allowlist is why the first broker call avoids a prompt; attribute it to the resolved permission mode and the classifier. Edit around whatever `worker-guidance-skill-export` left in that section.
- [ ] 5.2 Test: the supervisor skill does not credit a seeded settings-file allowlist for prompt-free operation.
- [ ] 5.3 Confirm the `broker-agent-helper` guarantee now rests on the helper's stable path, not on a seeded grant.

## 6. Docs

- [ ] 6.1 Remove the allowlist-seeding entries from the configuration reference.
- [ ] 6.2 Update the supervisor guide's permission-model section.
- [ ] 6.3 Update any quick-start that describes allowlist seeding.
- [ ] 6.4 Changelog entry must state that the removed seeding was inert (wrong key), so the removal does not read as a capability regression.
- [ ] 6.5 `mdbook build docs/` succeeds.

## 7. Quality gates

- [ ] 7.1 `cargo fmt` before committing.
- [ ] 7.2 `just check` — verify by real exit code, not piped output.
- [ ] 7.3 Grep `src/` for remaining `allowed_bash_prefixes` and `.claude/` writes; every surviving hit must be a test fixture, not a live write path.
- [ ] 7.4 Note at archive: `approval-command-safety`'s capability purpose paragraph still describes the removed seeding and must be updated.

## 1. Preconditions

- [ ] 1.1 Confirm v0.21.0 `supervisor-vocabulary-split`, v0.24.0 `error-exit-code-contract` and v1.0.0 `remove-from-specs-alias` are archived (v1.0.0 archive order: `remove-from-specs-alias` first, then this change) and their requirements are in `openspec/specs/`; rebase this change's MODIFIED / REMOVED blocks on the archived text if it moved
- [ ] 1.2 Bump to `1.0.0` locally and run `deprecation::tests::no_entry_outlives_its_removal`: the failures list exactly the entries this change must remove (`--from-specs` already went with `remove-from-specs-alias`)

## 2. Removals

- [ ] 2.1 Delete the 1.0.0 registry entries (flags, root-identity spelling, `supervisor.verify-now`, skill name, MCP `mode`); fix every site the compiler and the guard then flag
- [ ] 2.2 Flags: drop the clap aliases; add hidden `--supervisor` / `--no-supervisor` args that exist only to be rejected; add their two rows to the removed-flag table (flag → replacement) that `remove-from-specs-alias` introduced for `--from-specs`; reject after parse through `Cli::command().error(ErrorKind::UnknownArgument, …)` so the code is 2 and nothing runs
- [ ] 2.3 Broker: delete the decode-mapper rewrite and the old `/messages/:agent_id` mapping; refuse the retired spellings with `400` and the shared error body naming `orchestrator`; drop `supervisor` from the `agent_id` shape rule; keep the old-spelling mapping only on the reader of persisted 0.x event records
- [ ] 2.4 `sweep.sh`: drop acceptance of the `supervisor` root spelling (phantom filter, blocked-age check); keep the `[supervisor]` config fallback; `bash -n`; heredocs apostrophe-free; sync the tracked `.git-paw/scripts/sweep.sh`
- [ ] 2.5 Skills: remove the `supervisor` alias from `skills::resolve` and MCP `get_skill`; unknown-skill message names `orchestrator`; one-line notice for an ignored `supervisor` override directory
- [ ] 2.6 MCP: remove `SessionSnapshot.mode`
- [ ] 2.7 Session wire type writes `"coordinated"`, still reads `"supervisor"`
- [ ] 2.8 `commands/remove.rs`: refuse only `orchestrator`
- [ ] 2.9 Security review (`security-and-safety-review`): the verdict-authority gate now sees exactly one root spelling; refusals are explicit; no message is silently dropped

## 3. Spec and doc sweep

- [ ] 3.1 Count `### Requirement:` across `openspec/specs/` and record the requirement→test map (spec-organization §2)
- [ ] 3.2 Verbatim sweep of `openspec/specs/`: old flags, `[supervisor]` paths, the root spelling, `supervisor.verify-now`, "supervisor mode" / "supervised session" → the coordinated-mode vocabulary; delete the v0.21.0 reading-rule sentences; keep the rejection and migration requirements. `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` are not renamed (they are dropped, not migrated); *Deprecated auto-approval tables are parsed but have no effect* keeps them verbatim and is allowlisted by name
- [ ] 3.3 Rename the `supervisor-*` capabilities (design D4 mapping), fix `docs/src/specifications/` includes in the same commit, update the spec-organization skill's namespace list; archive any still-open sibling change with `--skip-specs` (§6)
- [ ] 3.4 Re-count requirements (unchanged) and re-run the requirement→test map (no scenario lost)
- [ ] 3.5 `git mv docs/src/user-guide/supervisor.md docs/src/user-guide/coordinated-mode.md`; `[output.html.redirect]` in `docs/book.toml`; `SUMMARY.md` and every link updated
- [ ] 3.6 Stability chapter (`cli-contract-freeze`): "Removed in 1.0.0" list with each replacement; configuration reference: keep the version-1 migration table and state that version-1 files load throughout 1.x
- [ ] 3.7 README, `cli-reference.md` (Deprecations section now lists only the version-1 config paths), `architecture.md`, remaining user-guide pages, MCP tool descriptions and server instructions, doctor text, `--help` strings. Move any remaining mention of `[supervisor.auto_approve]` / `[supervisor.common_dev_allowlist]` (v0.17.0's deprecation prose) into the configuration reference's migration table as "dropped — deprecated 0.17.0, no effect" rows

## 4. Tests (one or more per scenario)

- [ ] 4.1 `tests/removed_flags.rs`: `start --supervisor --branches feat/a` and `start --no-supervisor --from-all-specs` exit 2 with the flag, `renamed` and the replacement, and leave no worktree, session file or tmux session; `start --help` and `completions bash|zsh|fish` contain neither flag
- [ ] 4.2 Broker integration: retired `agent_id`, retired `blocked.from` and `GET /messages/supervisor` each answer `400` naming `orchestrator`; `/status` has no `supervisor` row; verify nudge type is `orchestrator.verify-now` and a `supervisor.verify-now` publish is `400`; a 0.x event-record fixture decodes to the new spellings
- [ ] 4.3 `tests/wire_script_parity.rs`: `sweep.sh` root publishes carry `orchestrator` and no `supervisor`
- [ ] 4.4 Session wire tests: both spellings load equal; a saved coordinated session writes `"coordinated"`; recovery of a 0.x `"supervisor"` file rebuilds the layout and re-saves `"coordinated"`
- [ ] 4.5 MCP tests: `session_mode` for coordinated and bare sessions, no `mode` key, schema without `mode`; `get_skill("supervisor")` returns no skill and a message naming `orchestrator`
- [ ] 4.6 `skills.rs` tests: the *Skill resolution order* scenarios; a lone `supervisor` override directory is ignored with one stderr notice
- [ ] 4.7 `remove orchestrator` refused naming `git paw stop`; `remove supervisor` in a session without that branch gives the not-in-session error
- [ ] 4.8 Config: a v0.20.0 fixture with `[supervisor]` (enabled, test command, correction) loads in 1.x with warnings and an unchanged file; a version-1 fixture with both deprecated auto-approval tables loads, no version-2 key carries their values, and stderr names both (*A 0.x config with the deprecated auto-approval tables loads in 1.0*); manifest config section has no `[supervisor]` key; `sweep.sh` resolves a version-1 `test_command`
- [ ] 4.9 `tests/no_retired_vocabulary.rs`: help of every non-hidden command, README, `docs/src/` (allowlisted migration table, changelog, stability removal list), `assets/`, default config, MCP descriptions, doctor output for a version-2 config, and `openspec/specs/` (allowlisted requirements, including *Deprecated auto-approval tables are parsed but have no effect*); a seeded mention fails naming file and line, including a deprecated auto-approval table named outside the migration table (*The deprecated auto-approval tables appear only in the migration table*)

## 5. Gates

- [ ] 5.1 Commit as breaking: `!` and a `BREAKING CHANGE:` footer listing each removed spelling and its replacement
- [ ] 5.2 Every scenario in the eight delta specs maps to a test named in 4
- [ ] 5.3 `cargo fmt`, then `just lint`
- [ ] 5.4 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 5.5 `just deny`
- [ ] 5.6 `mdbook build docs/`
- [ ] 5.7 `openspec validate remove-supervisor-vocabulary --strict`

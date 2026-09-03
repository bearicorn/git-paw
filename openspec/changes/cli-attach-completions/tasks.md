## 1. `git paw attach`

- [ ] 1.1 Add an `Attach` variant to the `Commands` enum in `src/cli.rs` with `about` + `long_about` + examples
- [ ] 1.2 Dispatch: resolve the current repository's session (find-by-repo-path) and call `tmux::attach()`; actionable error when none is running/alive
- [ ] 1.3 Tests: `git paw attach` with no session exits with an actionable error (assert_cmd); attach resolves the repo's session name

## 2. `git paw completions <shell>`

- [ ] 2.1 Add `clap_complete` to `Cargo.toml` (approved) and to the `AGENTS.md` dependency table
- [ ] 2.2 Add a `Completions` variant taking a shell argument; dispatch generates the script via `clap_complete::generate` to stdout
- [ ] 2.3 Tests: `git paw completions bash` prints a non-empty script and exits 0; an unsupported shell errors actionably

## 3. Strike the phantom `resume`

- [ ] 3.1 Remove every `git paw resume` reference from README, mdBook, and the CLI reference; document reattach as `git paw attach` and revival as `git paw start`
- [ ] 3.2 Test/guard: `git paw resume` is rejected as unknown; a docs-parity check asserts no `resume` command reference remains

## 4. Docs

- [ ] 4.1 CLI reference lists `attach` and `completions` (the "lists every top-level subcommand" guard covers this)
- [ ] 4.2 README + mdBook document `attach` and installing completions
- [ ] 4.3 `mdbook build docs/` succeeds

## 5. Gates

- [ ] 5.1 `just check` green; no `unwrap()`/`expect()` in non-test code; public items documented
- [ ] 5.2 `just deny` clean (new `clap_complete` dependency passes licence/advisory checks)
- [ ] 5.3 Every spec scenario maps to a test

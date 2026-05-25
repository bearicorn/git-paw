# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] — v0.5.0

### Features

- *(start)* `git paw start` now rebases each existing agent branch onto the
  repository's default branch (whatever `origin/HEAD` tracks — typically
  `main`) before opening or reopening its worktree, so agents always start
  from current `main`. Brand-new branches created during the launch and
  branches already up to date with the default are unaffected. Resolves
  `MILESTONE.md` drift item 48 (agent worktree base divergence from main).
- *(cli)* New `--no-rebase` flag on `git paw start` opts out of the
  default-on rebase and reproduces the pre-v0.6 launch contract. Combines
  with all existing `start` flags (`--supervisor`, `--from-specs`,
  `--cli`, `--branches`, …).
- *(supervisor)* **Common dev-command allowlist preset** (default enabled).
  On every supervisor session start, git-paw seeds
  `.claude/settings.json::allowed_bash_prefixes` with a curated set of safe
  dev-loop prefix patterns so common commands bypass per-prompt approval.
  This is the v0.5.0 mitigation for drift 44 / drift 27; full per-CLI
  placement (Codex, Gemini, opencode, ...) ships in v1.0.0 hook-providers.

  **Preset patterns shipped in v0.5.0:**

  - Cargo: `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt`,
    `cargo check`, `cargo tree`, `cargo deny`, `cargo update`
  - Git (read): `git status`, `git log`, `git diff`, `git show`, `git fetch`
  - Git (write, non-destructive): `git commit`, `git push`, `git pull`,
    `git merge`, `git stash`, `git add`, `git restore`, `git rm`
  - Just: `just` (any recipe)
  - mdBook: `mdbook build`
  - OpenSpec: `openspec validate`, `openspec new`, `openspec archive`,
    `openspec list`, `openspec status`, `openspec instructions`
  - Search (read-only): `find`, `grep`, `sed -n`

  Destructive operations (`cargo install`, `cargo run`, `git rebase`,
  `git reset`, `git checkout`, `git push --force`, `sed` without `-n`,
  non-cargo package managers) are intentionally excluded.

  **Opt out:**

  ```toml
  [supervisor.common_dev_allowlist]
  enabled = false
  ```

  **Extend** with project-specific prefixes:

  ```toml
  [supervisor.common_dev_allowlist]
  extra = ["pnpm test", "deno fmt"]
  ```

  When the alt-config directory `~/.claude-oss/` already exists, the same
  preset is also written to `~/.claude-oss/settings.json`; the directory
  is never created by git-paw.

### Breaking Changes

- *(default)* Default behaviour of `git paw start` changed: existing agent
  branches are rebased onto the default branch at launch. Users who
  depended on the v0.4 / v0.5 no-rebase behaviour can pass `--no-rebase`.
  The dogfood evidence is that "no rebase" is the surprising default, not
  the safe one.
- *(api)* `git_paw::git::create_worktree` gains a third parameter
  `rebase_onto_main: bool`. Library consumers of `git-paw` as a crate
  must update call sites; the binary itself is the only known caller.

### Documentation

- Add `docs/src/user-guide/session-lifecycle.md` describing rebase-on-start,
  when to use `--no-rebase`, and conflict-handling.
- Update `README.md` "Smart session management" bullet to mention the new
  default.
- Update `git paw start --help` `long_about` with an example and a
  paragraph explaining when to use `--no-rebase`.

## [0.4.0] - 2026-05-06

### Features

- *(supervisor)* Auto-approve patterns
- *(supervisor)* Mode with merge loop, session summary, recovery, question forwarding
- *(cli,config,init,git)* Supervisor + force flags, supervisor config schema, branch handling
- *(dashboard)* Committed counter, prompt-inbox interactivity, message log panel, layout
- *(broker)* Hook injection, watcher, sticky terminal status, real uptime, verified/feedback messages
- *(skills)* Standardize agent-skill resolution to agentskills.io layout
- *(detect)* Expand auto-detection to cover 10 additional AI CLI tools

### Documentation

- Align README, mdBook, and AGENTS.md with v0.4.0 surface
- *(specs)* V0.4.0 OpenSpec changes, archive plan, and main-spec alignment

### Testing

- Behavioral integration and unit tests for v0.4.0

### Miscellaneous

- Deps, supervisor skill, gitignore + deny tuning, pre-push gate, ci fixes ([#50](https://github.com/bearicorn/git-paw/pull/50))
## [0.3.0] - 2026-04-10

### Features

- *(broker)* Wire broker into session lifecycle and update docs ([#43](https://github.com/bearicorn/git-paw/pull/43))
- Add dashboard, skills, and agent coordination
- *(broker)* Add HTTP broker with message types, delivery, and config

### CI/CD

- *(deps)* Switch dependabot to monthly and ignore cargo-dist actions
- *(deps)* Bump actions/deploy-pages from 4 to 5 (#40) ([#40](https://github.com/bearicorn/git-paw/pull/40))

### Miscellaneous

- Prepare v0.3.0 release

### Build

- *(deps)* Bump toml from 0.9.12+spec-1.1.0 to 1.1.2+spec-1.1.0 (#41) ([#41](https://github.com/bearicorn/git-paw/pull/41))
## [0.2.0] - 2026-04-08

### Features

- Add v0.2.0 spec-driven launch, init, logging, replay, and AGENTS.md integration ([#42](https://github.com/bearicorn/git-paw/pull/42))

### Miscellaneous

- Prepare v0.2.0 release
- Add v0.2.0 openspec change proposals and module stubs
## [0.1.0] - 2026-03-25

### Features

- Add CLI tool for parallel AI coding sessions across git worktrees
[0.4.0]: https://github.com/bearicorn/git-paw/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/bearicorn/git-paw/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/bearicorn/git-paw/compare/v0.1.0...v0.2.0


## 1. Repo-scope the default --stale sweep

- [ ] 1.1 Filter stale-receipt selection to the current repository by default (reuse session-by-repository resolution); do not touch other repositories' receipts
- [ ] 1.2 Tests: `--stale` in one repo leaves another repo's stale session intact; a same-repo stale session is still purged

## 2. Fail-safe live guarantee

- [ ] 2.1 Purge a receipt only when the probe positively confirms it stale; skip on live or liveness-indeterminate
- [ ] 2.2 Tests: a live session (any repo) is never purged; a liveness-indeterminate receipt is skipped

## 3. --all-repos opt-in

- [ ] 3.1 Add `--all-repos` to the `purge` subcommand (`src/cli.rs`); meaningful only with `--stale` (reject/inert otherwise)
- [ ] 3.2 Dispatch: `--stale --all-repos` broadens the sweep to every repository's stale receipts (still live-safe)
- [ ] 3.3 Tests: `--all-repos` without `--stale` errors/inert; `--stale --all-repos` sweeps across repos; live sessions intact

## 4. Docs

- [ ] 4.1 Purge chapter / CLI reference: document the current-repo default scope, `--all-repos`, and the fail-safe live guarantee
- [ ] 4.2 `mdbook build docs/` succeeds

## 5. Gates

- [ ] 5.1 `just check` green
- [ ] 5.2 Gate-5 safety: no unbounded cross-repo delete; live sessions never purged
- [ ] 5.3 Every spec scenario maps to a test

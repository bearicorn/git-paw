## Context

**What init manages.** `GITIGNORE_ENTRIES` lists every path git-paw writes under `.git-paw/` except
`config.toml`, including `.git-paw/skills/` and `.git-paw/scripts/` (`src/init.rs:18-37`).
`ensure_gitignore_entry` appends the missing ones by exact line match (`src/init.rs:991-1028`).
`run_init` creates the directories, migrates or writes the config, reconciles `.gitignore`, installs
every bundled package with its `.git-paw/scripts/` compatibility copies, and rewrites the AGENTS.md
pointer; with nothing to do it prints "Already initialized. Nothing to do." (`src/init.rs:93-208`).
`install_packages` writes a self-ignoring `.git-paw/skills/.gitignore` (`*`) so packages stay out of
`git add -A` even where `.gitignore` predates the entry (`src/init.rs:352-357`), but the
compatibility copies under `.git-paw/scripts/` get no such file (`src/init.rs:379-386`,
`COMPAT_SCRIPTS_DIR` at `:390-394`).

**Why tracked copies show up in workers (#27).** `.gitignore` does not untrack a file already in the
index. Every launch provisions the helpers into each agent worktree through the same writer
(`provision_worktree_helpers`, `src/init.rs:300-316`), so a repository that committed 0.19 copies
gets them overwritten with the 0.20 content in every worktree, and git reports them modified there.

**What doctor reports today.** `probe_hygiene` compares `.gitignore` lines with
`required_gitignore_entries` (`src/doctor.rs:2504-2514`); `check_hygiene` reports the gap with the
remedy `git paw init` (`src/doctor.rs:1472-1489`) and a stale orchestrator pointer with the same
remedy (`src/doctor.rs:1539-1548`). The Bundled-scripts group reports missing/stale helpers and
compatibility copies with the remedy `git paw init` (`preflight-diagnostics` *Bundled-scripts check*).
Nothing asks git whether a generated path is tracked; doctor already runs read-only git commands
through `CommandRunner` (`is_committed`, `src/doctor.rs:2145-2156`).

**Init on a broken config.** `run_init` resolves the placement with
`load_config(..).unwrap_or_default()` (`src/init.rs:137-139`), migrates the file text regardless
(`:141`), and renders the root packages from `load_config(..).unwrap_or_default()` (`:177`), so a
syntax error yields root skills rendered for the default broker URL and no gates.

## Goals / Non-Goals

**Goals:** one obvious upgrade command; generated helpers never reach a commit; no init output built
from defaults the user did not write.

**Non-Goals:** git-paw changing the index or making commits (untracking stays the user's commit);
a version stamp file recording which release initialised the repository (open question 2); doctor
`--fix` (still diagnose-only).

## Decisions

### D1 — "Generated" means init's ignore list, one source
`tracked_generated_files(repo_root)` runs `git ls-files -z -- <each GITIGNORE_ENTRIES path>` in the
main checkout and returns the tracked paths, grouped by entry. Doctor and init both call it, so the
list cannot drift from what init ignores. `config.toml` is not in the list, so it is never flagged.

### D2 — Doctor: one ⚠ row for tracked files, with the exact command
Detail: "N generated file(s) are tracked: <entries>". Remedy:
`git rm -r --cached <entries>` then commit, naming the entries (directories collapse to one
argument each). ⚠, not ✗: the session still runs; the risk is a commit.

### D3 — Doctor: an upgrade summary row on top of the individual ones
`check_hygiene` receives the count of findings that `git paw init` repairs (missing `.gitignore`
entries, missing or stale helpers and compatibility copies, stale orchestrator pointer) and, when it
is non-zero, adds one ⚠ "repository setup is behind this git-paw version (N items)" with the remedy
`git paw init`. The individual rows keep their own detail. Tracked files are not counted: init does
not fix them (D4).
*Alternative:* replace the individual rows with the summary — rejected; the rows say what is wrong.

### D4 — Init reports tracked files, never untracks
After reconciling, init prints the D2 command when `tracked_generated_files` is non-empty. It does not
run `git rm --cached`: that stages a deletion in the user's index, which is a commit decision.
Reporting does not count as a change for the "Already initialized" message.

### D5 — Compatibility directory self-ignores
`install_packages` writes `.git-paw/scripts/.gitignore` with `*` through `write_managed_file`, next
to the existing skills one. It covers repositories whose `.gitignore` lacks the entry and every agent
worktree (same writer). It does not untrack anything (D4 does the reporting).

### D6 — Init stops before rendering on a load error
When `.git-paw/config.toml` exists and the merged config fails to load, init creates the directories,
reconciles `.gitignore`, skips the config migration and the package render, prints the load error
with the file path, and exits non-zero. A missing config is not an error (fresh init path unchanged).

## Risks / Trade-offs

- [`git ls-files` on a huge repository] → it is limited to the `.git-paw/` pathspecs; cost is small.
- [A user who deliberately commits a generated file] → ⚠ only; the explicit-list design of
  `GITIGNORE_ENTRIES` already leaves any other file under `.git-paw/` alone.
- [Init exiting non-zero on a broken config breaks a script that ran `init` blindly] → the config is
  broken either way; the old behaviour wrote skills from defaults silently.

## Migration Plan

1. `tracked_generated_files` + doctor row + init report (D1, D2, D4).
2. Upgrade summary row (D3).
3. Compatibility `.gitignore` (D5) and init load-error stop (D6).
Each step is independent and reverts on its own.

## Open Questions

1. Should `git paw start` also print the tracked-files warning once per launch, since doctor is not
   always run (the #27 user found it through worker diffs)? Recommended; left out to keep launch
   output unchanged in this change.
2. Record the git-paw version that last ran `init` (for a "set up by 0.19" message)? Not needed for
   D3, which works from the observed state.

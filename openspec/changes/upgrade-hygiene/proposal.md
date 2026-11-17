## Why

Upgrading a repository from one git-paw release to the next works, but only for users who already
know the steps. An external user upgrading from 0.19 to 0.20 reported:

- **#24** — `.gitignore` was five entries short and stale helper copies remained. `git paw doctor`
  reported each as its own line in two groups, and `git paw init` fixed both, but nothing said
  "this repository is set up for an older git-paw; run `git paw init`".
- **#27** — the repository had committed the 0.19 copies of `.git-paw/scripts/*` before that path
  was ignored. Ignoring a path does not untrack it, so every launch rewrote the tracked copies in each
  worker's worktree, they showed up in the workers' changed files, and a worker could commit them.
  The user fixed it by untracking them by hand. Doctor does not look for tracked generated files.

A third gap sits on the same path: `git paw init` on a config that does not load falls back to the
built-in defaults and renders the root skills from them (`src/init.rs:137-139`, `:177`), the init-side
twin of the doctor fallback that `config-check-command` fixes.

## What Changes

- **Doctor flags tracked generated files.** A Hygiene ⚠ lists every tracked path under the
  directories and files `git paw init` ignores (everything git-paw writes under `.git-paw/` except
  `config.toml`), with the remedy `git rm -r --cached <paths>` and a commit.
- **Start warns once about tracked generated files.** Doctor is not always run; the #27 user found
  the problem through worker diffs. Each `git paw start` that launches or recovers a session prints
  one stderr warning with the same entries and `git rm -r --cached` command as the doctor row. A
  clean repository prints nothing, and the warning never changes the exit code.
- **Doctor names one upgrade command.** When any finding that `git paw init` repairs is present
  (missing `.gitignore` entries, missing or stale helpers or compatibility copies, a stale
  orchestrator pointer), doctor adds one Hygiene ⚠ "repository setup is behind this git-paw version"
  with the remedy `git paw init`. The individual findings stay.
- **Init reconciles an older setup in one run**, reporting each change, and a second run reports
  nothing to do (what `run_init` does today, now specified as the upgrade path).
- **Init reports tracked generated files** with the same `git rm -r --cached` command. It does not
  change the index: untracking is a commit the user makes.
- **Init self-ignores the compatibility directory.** `.git-paw/scripts/.gitignore` containing `*`,
  as `.git-paw/skills/.gitignore` already does, so new helper copies stay out of `git add -A` even
  where `.gitignore` predates the entry.
- **Init does not render from defaults on a config load error.** It still creates the directories
  and reconciles `.gitignore`, leaves `config.toml` untouched, writes no root package, and exits
  non-zero naming the error.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `preflight-diagnostics`: ADDED *Tracked generated files are flagged*; ADDED *Doctor names one
  command for an outdated repository setup*; ADDED *Start warns once about tracked generated
  files*.
- `core-init`: ADDED *Init reconciles a repository set up by an older version*; ADDED *Init reports
  tracked generated files without untracking them*; ADDED *Init self-ignores the compatibility
  scripts directory*; ADDED *Init does not render from defaults when the config fails to load*.

## Impact

- **Code:** `src/doctor.rs` (`probe_hygiene` gains tracked-file and upgrade-summary inputs;
  `check_hygiene` rows), `src/init.rs` (`install_packages` writes the compatibility `.gitignore`;
  `run_init` reports tracked files and stops before rendering on a load error),
  `src/commands/start.rs` (one stderr warning per launch or recovery), one shared
  `tracked_generated_files(repo_root)` read through `CommandRunner` (`git ls-files`).
- **Docs:** `docs/src/user-guide/doctor.md` (two Hygiene rows), an "Upgrading git-paw" section in
  the user guide (run `git paw doctor`, then `git paw init`, then untrack what doctor lists; `start`
  also warns about tracked files),
  CHANGELOG pointer to it.
- **Cross-version dependencies:** `config-check-command` (this release) — the doctor side of the
  "no defaults after a load error" rule; this change applies the same rule to `init`.
  `config-schema-versioning` — its `version = 1` backfill rides on the same init run; nothing here
  changes `migrate_existing_config`. `uniform-json-output` — the new rows are ordinary checks in the
  existing doctor document.

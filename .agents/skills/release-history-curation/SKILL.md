---
name: release-history-curation
description: Reshape a dogfood-built release branch into clean, changelog-worthy commits before the release PR. Use when a `feat/vX.Y.0-*` branch carries a messy history — many `--no-ff` per-change merges plus per-section, fixup, and up-front spec commits — that must become one conventional commit per change. Covers what "changelog-worthy" means against `cliff.toml`, the non-interactive content-preserving `read-tree` replay (the agent harness has no `git rebase -i`), the mandatory empty-`git diff` preservation gate, and per-commit buildability.
license: MIT
compatibility: git-paw
---

# Release History Curation

Use at release prep, when a feature branch built by dogfood (`feat/vX.Y.0-*`) carries a messy
history — many `--no-ff` per-change merges, per-section/fixup commits, and one up-front spec
commit — that must become a clean, linear, changelog-worthy history before the release PR. The
`git cliff` changelog is generated from these commit messages, so the shape of the history *is*
the release notes.

This reshapes history **content-preservingly**: the delivered tree never changes. Distributing or
backdating the curated commits (spreading them across dates or nights) is a separate operator
decision and is **out of scope** here.

## What "changelog-worthy" means

`git cliff` (`cliff.toml`) groups commits by their Conventional-Commit type and **skips `chore`**:

| Type | Rendered group |
|---|---|
| `feat` | Features |
| `fix` | Bug Fixes |
| `perf` | Performance |
| `refactor` | Refactor |
| `docs` | Documentation |
| `test` | Testing |
| `ci` | CI/CD |
| `style` | Miscellaneous |
| `chore` | *(skipped — invisible in the changelog)* |

The target is **one conventional commit per change** (per the AGENTS.md "one clean linear commit
that resolves the issue" convention), typed by what the change *delivers*:

- new capability → `feat`
- a defect fix → `fix`
- byte-identical relocation / removal of inert code → `refactor`
- docs- or security-posture-only → `docs` or `fix(user-guide)`

Messages are lowercase, no trailing period, scope from the AGENTS.md scope list (compound
`(a,b)` when a change cuts across scopes), and carry **no AI-assistant or task-tracker
references**. Cross-check `cliff.toml` so nothing changelog-worthy hides under `chore` and nothing
trivial is surfaced.

## Procedure

1. **Map the backbone.** `git log --oneline --first-parent <merge-base>..HEAD`. Each `--no-ff`
   merge is one change, in dependency order. Note any base commits *below* the first merge (the
   up-front spec authoring / prep). `<merge-base> = git merge-base main HEAD`.

2. **Cut a rescue ref before touching anything.** `git branch backup/<branch>-precuration HEAD`.
   A history rewrite is only safe with a recoverable original; keep this ref until the tag is
   pushed.

3. **Decide each commit's `type(scope): subject`** from its net diff *and* the change's intent
   (its `openspec/changes/<change>/proposal.md` "## Why"). Derive the scope from what the code
   touches, not the change's folder name.

4. **Reshape via a `read-tree` replay** — non-interactive, because the agent harness has no
   `git rebase -i` (it is TTY-gated). Snapshot each integration point's *whole tree* onto a fresh
   linear parent chain:

   ```bash
   git checkout --detach <merge-base>
   # base commit(s) below the first merge — e.g. the up-front specs:
   git read-tree <spec-base-commit>;  git commit -m "docs(specs): ..."
   # one commit per backbone merge, oldest -> newest:
   for M in <M1> <M2> ... <Mn>; do
     git read-tree "$M"                       # index := M's entire tree
     git commit -m "<type(scope): subject>"   # commits the index onto current HEAD
   done
   git reset --hard HEAD                       # resync the worktree (read-tree left it stale)
   ```

   Use whole trees (`read-tree`), never `git diff <a> <b> | git apply` — trees are binary- and
   deletion-safe and cannot fuzz. Each resulting commit's tree is *exactly* that merge's tree; the
   parent chain is clean and linear.

5. **Content-preservation gate (mandatory).** `git diff <old-tip> <new-tip>` MUST be empty —
   curation reshapes history, it never changes delivered code. If it is non-empty, a tree was
   mis-captured; stop and fix before proceeding. `git range-diff <merge-base>..<old-tip>
   <merge-base>..<new-tip>` gives a human-readable before/after.

6. **Buildability.** Every commit must build and release (AGENTS.md). Run `just check` at the new
   tip; spot-check intermediate commits (`git checkout <sha>` + `cargo check`/`just check`). Each
   commit corresponds to a real integration point, but a change that secretly depended on a *later*
   one will fail here — that exposes a mis-order to fix by reordering the replay or combining the
   pair. Preview the notes with `git cliff --tag vX.Y.Z --unreleased` (or `just changelog`).

7. **Move the branch.** `git branch -f <branch> <new-tip>; git checkout <branch>`. Keep the
   `backup/…` ref until `vX.Y.Z` is tagged.

## Guardrails

- **No `git rebase -i`** — TTY-gated in the agent harness. Use the `read-tree` replay (or
  `git rebase --onto` with a scripted `GIT_SEQUENCE_EDITOR`/`GIT_EDITOR`).
- **The empty `git diff <old> <new>` is non-negotiable.** A non-empty diff means the reshape
  altered the delivered code — recover from the rescue ref, do not ship it.
- **`chore` is invisible in the changelog** — never bury a feature or fix under it.
- **One clean commit per change.** Fold that change's spec authoring, fixups, and per-section
  dogfood commits into it; drop the `--no-ff` merge commits.
- **Recover, don't force through.** A botched curation is undone by
  `git branch -f <branch> backup/<branch>-precuration`.
- **Out of scope: distribution/backdating.** Spreading the curated commits across dates or nights
  is an operator decision, applied *after* curation; this skill only reshapes history.

## Worked shape

A release branch of 17 `--no-ff` per-change merges over one up-front `docs(specs)` commit curates
to **1 base `docs(specs)` commit + 17 impl commits** — each a `feat`/`fix`/`refactor` naming one
change — with `git diff <old-tip> <new-tip>` empty and `just check` green at the tip.

## Why

`README.md` is 403 lines. It has 39 Features bullets, three quick starts, a configuration example, a CLI table, a Platform table, a release procedure and a "How It Works" diagram. Most of it repeats the mdBook. Parts of it are wrong today:

| Section | Problem |
|---|---|
| How It Works | shows only sibling `../` worktrees (init writes `child`), and omits dashboard pane 0 and the `broker` window |
| Features | `[dashboard] show_message_log` is superseded by the broker log panel |
| Command table | `init` misses helper install, the orchestrator and cli-setup skill export, the `AGENTS.md` pointer and the config backfill; `approvals` reads as if git-paw approves (it lists escalated prompts and approves none); `mcp` misses the verdict and spec-authoring write tools; cells carry "(v0.x.0+)" tags |

A newcomer has to scroll past release mechanics to find out what the tool does. The README should be a ~110–125 line landing page in the maintainer's voice (the `tone-of-voice` skill, technical-docs rules: short, lists and tables, lead with the point), linking the docs for everything else.

## What Changes

- Rewrite `README.md` as a landing page with these parts:
  - title, tagline and badges
  - a 2-sentence "What it does" and a short demo
  - ~8 one-line Highlights, each linking a docs page
  - one Install block and one prerequisites line
  - one merged Quick start that links the three quick-start pages
  - a refreshed command table with no version tags, linking `cli-reference.md`
  - a 2-line Configuration pointer and a one-line Supported CLIs list plus `git paw add-cli`
  - a ~5-line Security note, Documentation links, Contributing and License
- The v0.19 README bullets for sandbox mode, provider families and `sweep.sh tell` are folded into the Highlights, not dropped.
- Move README-only facts into the docs **before** deleting them:
  1. the `CLAUDE.md` `@AGENTS.md` import tip → `docs/src/user-guide/agents-md.md`
  2. "`start --force` has no effect under supervisor mode" → `cli-reference.md` and `spec-driven-launch.md`
  3. optional: the "why no native Windows" rationale → `installation.md`; `cargo run -- start` → `contributing.md`
- Remove from the README what the docs already cover: most Features bullets, the approval-flags paragraph, the Platform table, Windows/WSL detail, `git paw` vs `git-paw`, the configuration example, the full CLI table, purge detail, the unattended-loop paragraphs, the deprecated-tables note, and the Releases section (it lives in `AGENTS.md`).
- Fix two v0.18 leftovers outside the README: the `git paw remove supervisor` sentence in `docs/src/cli-reference.md`, and the `/tell` doc comment in `src/coordination/tell.rs:23` ("typed in the supervisor pane"; there is no supervisor pane).
- Rewrite the README-content requirements so they hold for a landing page, and move the detail they pinned onto the docs pages that now hold it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `core-project-conventions`: "README features section enumerates the current user-facing surface" becomes a Highlights section that links each capability's page. New requirements cover the landing-page shape, moving README-only facts first, and the Security note.
- `cli-resolution`: "README Supported AI CLIs table matches `src/detect.rs`": the README carries a one-line list instead of a table, with the same binary set.
- `supervisor-launch`: "README Supervisor Mode quick start documents the flags": the flags are documented on the orchestrator quick-start page that the README links.
- `spec-providers`: "README documents `[specs] type` accepts all three backends" is removed. The README has no configuration excerpt; `core-configuration` already pins the `[specs]` example in the configuration reference.

## Impact

- **Docs**: `README.md` (rewrite); `docs/src/user-guide/agents-md.md`, `docs/src/cli-reference.md`, `docs/src/user-guide/spec-driven-launch.md`, `docs/src/quick-start-orchestrator.md`, optionally `docs/src/installation.md` and `docs/src/contributing.md` (receive the moved facts).
- **Code**: `src/coordination/tell.rs` (one doc comment; no behaviour change).
- **Tests that read the README**, updated with the requirements: `tests/readme_features.rs` (parses `## Features`; it moves to `## Highlights` and its >500-byte truncation guard is re-checked), `tests/supported_clis_docs_sync.rs` (parses a README table; it moves to the one-line list), `tests/security_posture_docs.rs` (needs `## Security`, `arbitrary code` and `sandbox`, which the note keeps), `tests/cli_dx_docs.rs` (the command table keeps `` `attach` `` and `` `completions` ``; no `paw resume`).
- **No struct or enum ripple**.
- **Export-agnosticism**: the README is not an exported asset. No `assets/` file changes.

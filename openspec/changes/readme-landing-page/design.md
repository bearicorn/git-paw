## Context

README sections today: Demo, What It Does, Features (L46-99), Platform Support, three Quick Starts
(Same CLI, Per-Branch CLI, Supervisor Mode), Installation, Usage (command table), Configuration (incl. the
`[specs]` excerpt), Supported AI CLIs, How It Works, Security, Contributing, Releases, License. README
content is pinned by: `core-project-conventions` (Features enumeration — no test enforces it),
`supervisor-launch` (supervisor quick start flags — no test), `spec-providers` (`[specs]` excerpt — no
test), `cli-resolution` (CLI table — no test), `core-security-posture` (disclaimer —
`tests/security_posture_docs.rs`), `cli-parsing` no-`resume` rule and `attach`/`completions` presence
(`tests/cli_dx_docs.rs`).

## Goals / Non-Goals

**Goals:** a short README that pitches, shows, installs, gets someone to a first session, and links out;
spec + guard so it stays that way.

**Non-Goals:** restructuring the mdBook; changing the docs site; badges policy.

## Decisions

- **D1 — Replace, don't weaken.** The enumeration requirement is REMOVED and a landing-page requirement
  ADDED, rather than MODIFIED in place, because its whole intent inverts (from "list everything" to
  "list little, link everything").
- **D2 — Guarded shape, not prose.** `tests/readme_landing_page.rs` asserts: the required sections exist
  (pitch, Demo, Installation, one Quick Start, command table, Supported AI CLIs, Security, Documentation
  link to the docs site); the Features list has at most 12 bullets and every bullet links into the docs;
  no `vX.Y` version stamp appears outside the Releases/Installation context; no `[supervisor.` config
  table name appears. The 12-bullet cap is a judgement call (today: 39) — easy to tune.
- **D3 — Move before cut.** Each README section removed is checked against the docs; content not already
  present moves to the matching chapter in the same change (content audit task).
- **D4 — Keep the command table.** One line per subcommand stays: it is landing-page-sized, the
  `cli_dx_docs` guard relies on it, and every new command adds one row.

## Risks / Trade-offs

- [Search engines / crates.io show the README] → the pitch, install and quick start stay; detail is one
  click away on the docs site.
- [Contributors keep adding feature bullets] → the guard caps the list; the doc-completeness skill says
  where detail goes.

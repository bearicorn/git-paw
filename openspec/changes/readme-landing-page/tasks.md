## 0. Preconditions

- [ ] 0.1 Land after the v0.18 relocation doc changes (`supervisor-pane-removal`, `orchestrator-skill-export`) and after this release's command additions (`finish`, `config check`) so the command table is final

## 1. Content audit (move before cut)

- [ ] 1.1 For every README section to be removed (Features detail, Per-Branch and Supervisor quick starts, Configuration excerpt, How It Works, Releases detail, Platform Support detail), find the docs page that already covers it; move anything missing into the matching chapter (`introduction.md`, `quick-start-*.md`, `configuration/README.md`, `architecture.md`, `contributing.md`, `installation.md`)
- [ ] 1.2 Confirm `mdbook build docs/` still passes after the moves

## 2. README rewrite

- [ ] 2.1 Rewrite `README.md`: pitch, demo, installation, one quick start, one-line-per-command table, Supported AI CLIs list, `## Security` disclaimer, documentation link; ≤ 12 linked capability bullets; no `[supervisor.` tables, no version-stamped feature text, no `paw resume`

## 3. Guard

- [ ] 3.1 `tests/readme_landing_page.rs`: required sections; ≤ 12 Features bullets each with a docs link; no `[supervisor.`; no Features bullet describing auto-approval of permission prompts (carries v0.17.0's guard); command-table rows == `git paw --help` subcommands (hidden ones excluded); keep `tests/security_posture_docs.rs` and `tests/cli_dx_docs.rs` green

## 4. Dev skill

- [ ] 4.1 `.agents/skills/doc-completeness/SKILL.md`: README rule becomes "command table row for every new command; feature bullet only for a new headline capability; detail goes to mdBook/references"; `tests/agent_skills_conform.rs` green

## 5. Gates

- [ ] 5.1 `just check`; `mdbook build docs/`

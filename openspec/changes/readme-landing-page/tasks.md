## 1. Move README-only facts into the docs first

- [ ] 1.1 `docs/src/user-guide/agents-md.md`: the `CLAUDE.md` `@AGENTS.md` import tip (first line `@AGENTS.md`, why it beats a symlink, `ln -s AGENTS.md GEMINI.md` for import-less CLIs, gitignore them)
- [ ] 1.2 `docs/src/cli-reference.md` and `docs/src/user-guide/spec-driven-launch.md`: `start --force` has no effect under supervisor mode
- [ ] 1.3 `docs/src/quick-start-orchestrator.md`: `--no-supervisor` and `start --force` in command-line examples, with the no-effect note
- [ ] 1.4 Optional: "why no native Windows" → `docs/src/installation.md`; `cargo run -- start` → `docs/src/contributing.md`
- [ ] 1.5 Check that every Features bullet removed from the README is covered by a `docs/src/` page (sandbox mode, provider families and `sweep.sh tell` included); add a line to the page where one is missing

## 2. Rewrite the README

- [ ] 2.1 Load the `tone-of-voice` skill; write in the maintainer's technical-docs voice
- [ ] 2.2 Sections in the order the spec gives, at most 130 lines (target 110–125)
- [ ] 2.3 ~8 Highlights, each linking a docs page: parallel worktrees + tmux; per-branch CLIs and provider families; spec-driven launch (OpenSpec / Spec Kit / Superpowers / Markdown); runtime isolation; orchestrator mode (your own agent drives via the exported skill, broker + dashboard, conflict detection, `sweep.sh tell`, learnings); `--unattended` drive loop that escalates and never approves; MCP server; doctor + recovery; sandboxed workers
- [ ] 2.4 One Install block (cargo / brew / shell installer) + one prerequisites line (tmux, Git 2.5+, macOS/Linux, Windows via WSL)
- [ ] 2.5 Merged Quick start: `git paw init`, `git paw`, `start --cli … --branches …`, the per-branch comment, `start --supervisor`, `start --unattended --from-all-specs`; link the three quick-start pages
- [ ] 2.6 Command table with no "(v0.x.0+)" tags, keeping `` `attach` `` and `` `completions` ``. Fix `init` (helper install, orchestrator + cli-setup skill export, `AGENTS.md` pointer, config backfill), `approvals` (lists escalated prompts; git-paw approves none) and `mcp` (verdict and spec-authoring write tools). Link `cli-reference.md`
- [ ] 2.7 2-line Configuration pointer; one-line Supported CLIs list as `` `<binary>` `` spans + `git paw add-cli`
- [ ] 2.8 ~5-line `## Security` note (arbitrary code, git-paw approves no prompt, the agent CLI's permission system decides, optional sandbox) linking `security-posture.md` and `sandbox.md`
- [ ] 2.9 Documentation links (mdBook, changelog, FAQ), Contributing, License
- [ ] 2.10 Drop: the How It Works diagram, `show_message_log`, the approval-flags paragraph, the Platform table, Windows/WSL detail, `git paw` vs `git-paw`, `cargo run`, the configuration example, the full CLI table, purge detail, the unattended-loop paragraphs, the deprecated-tables note, `## Releases`

## 3. Leftovers outside the README

- [ ] 3.1 `docs/src/cli-reference.md` `remove` section: keep that `git paw remove supervisor` is refused with a pointer to `git paw stop` (still true in `src/commands/remove.rs:65`); drop any wording that implies a supervisor pane (design.md, Risks)
- [ ] 3.2 `src/coordination/tell.rs:23`: reword "typed in the supervisor pane" to the orchestrator, which runs outside the session

## 4. Tests

- [ ] 4.1 `tests/readme_features.rs`: parse `## Highlights`; assert the README links plus the page substrings per the modified `core-project-conventions` scenarios
- [ ] 4.2 Tests for the new `core-project-conventions` scenarios: line budget and heading order, quick-start links and commands, no `(v0.` tags, superseded content gone, moved facts present, Security note content and links
- [ ] 4.3 `tests/supported_clis_docs_sync.rs`: collect the README's binaries from the Supported CLIs line (design.md decision 3)
- [ ] 4.4 Test for the `supervisor-launch` scenarios against `docs/src/quick-start-orchestrator.md`
- [ ] 4.5 `tests/security_posture_docs.rs` and `tests/cli_dx_docs.rs` pass unchanged

## 5. Quality gates

- [ ] 5.1 `cargo fmt`, then `just lint`
- [ ] 5.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 5.3 `just deny`
- [ ] 5.4 `mdbook build docs/`
- [ ] 5.5 `openspec validate readme-landing-page --strict`

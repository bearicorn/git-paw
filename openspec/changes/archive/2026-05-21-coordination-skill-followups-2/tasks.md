## 1. Coordination skill — `### Commit cadence` subsection

- [x] 1.1 In `assets/agent-skills/coordination.md`, insert a new subsection. Anchor: after the existing `### Working heartbeat` section (added by coordination-skill-followups) and before `### Stash hygiene`. Heading text: `### Commit cadence` (or `### Per-group commit cadence`).
- [x] 1.2 Content per the spec delta (`specs/agent-skills/spec.md` "Requirement: Coordination skill SHALL teach per-group commit cadence"): per-group default, ~10-file soft cap, split with `(part N of M)`, conventional-commit prefixes example, rationale.
- [x] 1.3 Add a behavioural test in `src/skills.rs::tests` named `coordination_skill_documents_commit_cadence`. Assert the embedded `coordination.md` content contains: (a) a heading naming the cadence concept (commit-cadence-style substring), (b) the substring `group` or `section`, (c) at least one conventional-commit example prefix (`feat(`, `fix(`, `docs(`, `test(`, or `chore(`).

## 2. Coordination skill — `### Terminal action: commit then publish, never archive` subsection

- [x] 2.1 In `assets/agent-skills/coordination.md`, insert a new subsection (anchor: near `### Messages you may receive` or `### Cherry-pick peer commits`). Heading text: `### Terminal action: commit then publish, never archive` (or substantively equivalent).
- [x] 2.2 Content: the coding agent's terminal action is `agent.artifact{status:"done"}` or the implicit `committed` event. The agent SHALL NOT invoke `/opsx:verify <change-id>` or `/opsx:archive <change-id>`. Rationale per spec delta. Name both skill names by literal string.
- [x] 2.3 Add a behavioural test `coordination_skill_forbids_opsx_verify_and_archive`. Assert the embedded `coordination.md` content contains: (a) literal `/opsx:verify`, (b) literal `/opsx:archive`, (c) prose like "off-limits" or "do not invoke" or "supervisor's job".
- [x] 2.4 Add a behavioural test `coordination_skill_names_terminal_action`. Assert the content contains `agent.artifact` and either `status: "done"` or `status: "committed"`.

## 3. Supervisor skill — `### Resolve pane to agent via pane_current_path` subsection

- [x] 3.1 In `assets/agent-skills/supervisor.md`, insert a new subsection (anchor: near or just before the existing `### Observe and drive a peer pane via tmux` section). Heading text: `### Resolve pane to agent via pane_current_path` (or substantively equivalent).
- [x] 3.2 Content: show the canonical tmux command (`tmux display-message -t paw-{{PROJECT_NAME}}:0.<pane> -p '#{pane_current_path}'`); explain that the output ends in `<project>-feat-<branch>`; warn against pane indices being alphabetical, CLI-argument-order, or inferred from `git paw status` / dashboard row order; recommend caching the mapping once per session.
- [x] 3.3 Add a behavioural test `supervisor_skill_documents_pane_current_path_resolution`. Assert the embedded `supervisor.md` content contains: (a) literal `tmux display-message`, (b) literal `pane_current_path`, (c) prose warning against `agent_id` alphabetical or CLI-argument-order assumptions.

## 4. mdBook mirror updates

- [x] 4.1 In `docs/src/user-guide/coordination.md`, mirror the two new coordination skill subsections (commit cadence + terminal action). Substantively identical content; cross-link to the bundled skill if the chapter uses an "embedded skill mirrors below" pattern.
- [x] 4.2 In `docs/src/user-guide/supervisor.md`, mirror the pane-current-path resolution subsection. Cross-link to the bundled `sweep.sh` (drift 68 §8c) where the `tmux display-message` invocation lives.
- [x] 4.3 `mdbook build docs/` clean.

## 5. Quality gates

- [x] 5.1 `cargo fmt` and `cargo clippy --all-targets -- -D warnings` clean.
- [x] 5.2 `just check` green (or equivalent test command per drift 67's gate templating).
- [x] 5.3 `openspec validate coordination-skill-followups-2 --strict` passes.
- [x] 5.4 `just deny` clean.

## 6. Release notes (in archive's release-notes.md, NOT CHANGELOG)

- [x] 6.1 Call out: coordination skill now teaches per-group commit cadence (~10-file soft cap, conventional-commit prefix per group).
- [x] 6.2 Call out: coordination skill explicitly forbids the coding agent from running `/opsx:verify` and `/opsx:archive` — both are supervisor-only.
- [x] 6.3 Call out: supervisor skill teaches `pane_current_path` as the canonical pane→agent resolution; pane indices are NOT alphabetical or CLI-argument-order.

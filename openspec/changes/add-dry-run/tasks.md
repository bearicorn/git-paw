## 1. Seams (behaviour-preserving)

- [ ] 1.1 Extract `add`'s resolution (session, status, branch/spec, worker, CLI validation, agent-id clash, agent cap) into a function returning an `AddPlan`; the real path consumes it unchanged; `tests/add_remove_e2e.rs` green
- [ ] 1.2 A pure `render_add_plan(&AddPlan) -> Vec<String>` reusing `worker_label`, `dry_run_launch_line` and `git::worktree_display_path`

## 2. Flag and dispatch

- [ ] 2.1 `src/cli.rs`: `Command::Add { dry_run }` with help text; parse tests (`add_dry_run_parses`, default false, with `--from-spec`)
- [ ] 2.2 `AddOptions.dry_run`; `main.rs` passes it; a dry run returns after the plan is built, before `SessionLock::acquire`
- [ ] 2.3 `add` long help: an example `git paw add feat/api --cli codex --dry-run`

## 3. JSON (after `uniform-json-output`)

- [ ] 3.1 `AddOutput` gains `dry_run` and `launch_command`; dry run fills `worktree_path` from `git::worktree_path_for`
- [ ] 3.2 Contract test: dry-run document keys; real add carries `dry_run: false`; refused dry run leaves stdout empty

## 4. Tests per scenario

- [ ] 4.1 E2E (isolated `HOME`/`XDG_DATA_HOME`, real repo + tmux): no session, stopped session, already-an-agent, unknown CLI each refuse with the real add's error
- [ ] 4.2 E2E: active plan lines, paused `Status:` line, profile worker line, and byte-identical `runs:` against `start --dry-run`
- [ ] 4.3 E2E: nothing changed (no worktree, session file bytes, pane count); a held `.add-remove.lock` does not block a dry run

## 5. Docs

- [ ] 5.1 `docs/src/cli-reference.md`: `add --dry-run`, the plan shape, the JSON keys
- [ ] 5.2 The orchestrator skill's `add` guidance mentions previewing with `--dry-run`

## 6. Gates

- [ ] 6.1 `cargo fmt`; `just check`; `just deny`; `mdbook build docs/`
- [ ] 6.2 `openspec validate add-dry-run --strict`

## 1. Parser and decomposition (`src/specs/speckit.rs`)

- [ ] 1.1 Task regexes accept an optional case-insensitive `[HUMAN]` token before or after `[P]`; `Task` gains `human: bool`
- [ ] 1.2 Parallel-ready rule: a `[P]` task is ready only with no open non-human non-`[P]` task before it and no other open task id of the phase in its description; others join the consolidated entry in `tasks.md` order
- [ ] 1.3 Human-only tasks produce and join no entry; a phase of only human tasks yields no entry and a "waiting on a human" report
- [ ] 1.4 Unit tables for 1.1–1.3; keep the existing decomposition tests green (inputs list `[P]` tasks first, or are noted)

## 2. Branch cap

- [ ] 2.1 `cap_segment(slug, 48)` after `branch_segment_slug` for `task/` and `phase/` (word-boundary cut, trailing `-`/`_` trimmed); `branch_segment_slug` itself unchanged
- [ ] 2.2 Refuse a scan whose capped branches collide, naming both ids, before any worktree is created
- [ ] 2.3 Tests: the long-description example, the unchanged short example, the collision

## 3. Wave planning

- [ ] 3.1 Pure `plan_wave(features, WaveMode) -> WavePlan { launch, held }`, unit-tested
- [ ] 3.2 `[specs] wave = "feature" | "all-features"` (default `feature`); config reference and commented init default
- [ ] 3.3 `SpecMode::All` applies `plan_wave` for Spec Kit only (CLI and MCP `from_all_specs`); held features and human-waiting features printed on stderr
- [ ] 3.4 e2e: three open features launch one; `all-features` launches three; `--specs 002-…` launches it with nothing held

## 4. Sibling prompt

- [ ] 4.1 `assets/task-prompts/speckit/parallel-siblings.md` (heading + no-import sentence); `your_task_section` appends the list for `[P]` entries with siblings
- [ ] 4.2 Tests: two siblings listed with branch; a lone `[P]` has no list

## 5. Conflict detector (`src/broker/conflict.rs`)

- [ ] 5.1 `forward_overlaps` and `in_flight_overlaps` drop `is_shared_task_tracking_path` paths beside the `.git-paw/` filter
- [ ] 5.2 Tests: forward and in-flight on `tasks.md` silent; a source overlap beside it still warns without naming `tasks.md`

## 6. Launch order (`src/commands/supervisor.rs`)

- [ ] 6.1 Open the session's service surface and start the broker host before the attach loop; register each agent as it attaches; a provisioning failure tears down session and host
- [ ] 6.2 Print `broker ready: <url>` on stderr once the host answers
- [ ] 6.3 e2e (isolated socket, ephemeral port, sleeping `on_create`): broker answers mid-provisioning; ready line precedes hook output; failing hook leaves no host

## 7. Docs and skill

- [ ] 7.1 Spec Kit user-guide chapter: one-feature waves, `[HUMAN]`, parallel-ready `[P]`, sibling list, branch cap, D6 note on old long branches
- [ ] 7.2 Conflict-detection chapter: the task-file exemption covers all three shapes
- [ ] 7.3 Orchestrator skill: run `--unattended` in the background and wait for `broker ready` before publishing
- [ ] 7.4 CHANGELOG: the default-wave and branch-name behaviour changes

## 8. Gates

- [ ] 8.1 `cargo fmt`, then `just lint`
- [ ] 8.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 8.3 `mdbook build docs/`
- [ ] 8.4 `openspec validate spec-kit-wave-planning --strict`

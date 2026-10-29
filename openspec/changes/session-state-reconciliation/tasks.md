## 1. Pure core

- [ ] 1.1 Add `src/session/reconcile.rs`: `Observations`, `Drift` (the ten kinds), `reconcile(record, discovery, observations)`, `Repair`, `repair_plan(drifts, observations)`; no I/O; generalize `tmux::session::agents_without_live_pane` into it (keep the `add`-path call working)
- [ ] 1.2 Repair-plan rules: gone → `stopped`; worktree-missing → drop the entry only when the terminal is gone; registration-stale → prune only when the dry-run set is a subset of git-paw-owned paths; discovery-stale → rewrite only when alive; temp files older than 60 s; everything else report-only

## 2. Observation gathering (shell)

- [ ] 2.1 Terminal facts through the v0.18 `TerminalBackend` (session liveness and surfaces with owning worktree); an unavailable backend gives an indeterminate observation
- [ ] 2.2 Version-control facts: `git worktree list --porcelain` and `git worktree prune --dry-run --verbose` (verify the output format against git ≥ 2.30; map `worktrees/<id>` to paths through the porcelain listing)
- [ ] 2.3 Broker facts through the broker client (`/status` reachability and `repo_root` per v0.17 `broker-repo-identity`); filesystem facts (discovery file, untracked `.git-paw/worktrees/*` dirs, `<session>.json.*.tmp` ages); newer-schema detection per `wire-format-dtos`

## 3. Doctor

- [ ] 3.1 Add the Session group (one check per drift kind; ids `session.<kind>`; the remedies from the spec; one informational ✓ when there is no session); slim Hygiene to `.gitignore` only
- [ ] 3.2 `src/cli.rs`: `doctor --fix` (help: "Repair git-paw's own session records (session file, discovery file, stale git-paw worktree registrations, leftover temp files); never deletes worktrees or branches or touches processes"); `long_about` example `git paw doctor --fix`
- [ ] 3.3 Apply the repair plan through `SessionStore::update` (re-read under the lock), `git::prune_worktrees` behind the subset gate, and temp-file deletion; a failed repair does not stop the rest
- [ ] 3.4 Re-run the Session group after repairs; print `fixed:` / `skipped:` / `failed:` lines; exit from the re-checked results; `--json` adds `repairs[{id, action, target, outcome, detail}]`

## 4. Tests (one or more per scenario)

- [ ] 4.1 Reconcile decision table: active+gone → gone drift; agents-exited → agents-exited drift (not gone); indeterminate → none; missing worktree vs missing surface; discovery mismatch; fully consistent → none; foreign broker; newer schema
- [ ] 4.2 E2E (isolated HOME, real tmux via the terminal backend): kill the terminal session externally, then `doctor` reports Session ⚠ with the `--fix` + `start` and `purge --stale` remedies; `doctor --fix` gives status `stopped` with worktrees and branches intact; then `start` rebuilds one surface per worktree
- [ ] 4.3 Hand-deleted worktree: reported naming the branch and path; dropped by `--fix` when the session is down; kept and reported when the session is live
- [ ] 4.4 Prune gate: a git-paw stale registration plus a user's own stale registration means no prune, the user registration is still listed, and the git-paw one stays reported
- [ ] 4.5 An untracked dirty dir under `.git-paw/worktrees/` is byte-unchanged after `--fix` and still reported
- [ ] 4.6 A live session with a deleted discovery file has it rewritten by `--fix` (one agent per worktree)
- [ ] 4.7 A live session with an unreachable broker: no process is spawned or signalled by `--fix` (assert the process table delta and the terminal surface list are unchanged); the drift stays reported
- [ ] 4.8 A foreign broker is ✗ with a non-zero exit; no session gives an informational ✓
- [ ] 4.9 Without `--fix`, doctor is read-only over a drifted session (snapshot `.git-paw/`, the session file and `git worktree list` before and after)
- [ ] 4.10 `--fix` output lists the applied repair and the re-check shows ✓; `--fix --json` `repairs` entries carry the five keys
- [ ] 4.11 Hygiene: a missing gitignore entry gives ⚠ with `git paw init`; a stale session is reported under Session, not Hygiene
- [ ] 4.12 Existing doctor suites (`tests/doctor_e2e.rs`, `src/doctor_tests.rs`, `tests/agent_pane_liveness_e2e.rs`) updated only where checks moved from Hygiene to Session

## 5. Docs

- [ ] 5.1 `docs/src/cli-reference.md`: `doctor --fix`, the Session group, and the exact repair set and its exclusions
- [ ] 5.2 User guide (doctor section and session lifecycle / crash recovery): "terminal killed or machine crashed → `git paw doctor --fix`, then `git paw start`"; how this differs from `purge --stale`
- [ ] 5.3 README CLI table if it lists doctor flags

## 6. Gates

- [ ] 6.1 Security and safety review per `.agents/skills/security-and-safety-review/SKILL.md` (first writing doctor mode; prune gate; no process control); record the outcome in the PR
- [ ] 6.2 `cargo fmt`, then `just lint`
- [ ] 6.3 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 6.4 `just deny`
- [ ] 6.5 `mdbook build docs/`
- [ ] 6.6 `openspec validate session-state-reconciliation --strict`

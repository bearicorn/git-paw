## 1. GP-02a — classifier normalizes leading assignment/wrapper prefixes

- [ ] 1.1 Extend `normalize_command` in `src/supervisor/auto_approve.rs` to strip a run of leading `NAME=value` assignments and a leading `env`/`nohup` wrapper before matching, composing with the existing trailing-probe/redirect strip
- [ ] 1.2 Ensure the danger-list / worktree-confinement / config-path / `.git/` rules run against the fully normalized command (no downgrade path)
- [ ] 1.3 Unit-table tests: `TMPDIR=… cargo test --lib`, `GIT_PAW_ALLOW_LIVE_SESSION=1 TMPDIR=… cargo test; echo exit=$?`, `env FOO=bar cargo build`, `nohup just check` all classify as their bare verb; `FOO=bar rm -rf /` still escalates as danger
- [ ] 1.4 Verify the shell classifier reached via `git paw __classify` (and any tracked `.git-paw/scripts/*.sh` copy) reflects the same normalization; add a parity assertion

## 2. GP-02b — git-paw managed helper scripts classify as safe

- [ ] 2.1 Add a classification rule in `auto_approve.rs` matching a normalized leading verb that resolves to a managed `.git-paw/scripts/{broker,sweep,docs-fetch}.sh` path (direct or `bash …`-invoked), reusing `is_managed_path()` from `src/agents.rs`
- [ ] 2.2 Preserve danger-list precedence: a chained danger op (`sweep.sh snapshot && rm -rf /`) still escalates
- [ ] 2.3 Unit-table tests for the three safe scenarios + the chained-danger escalation

## 3. GP-04b — writes under `.git/` escalate as danger

- [ ] 3.1 Add a danger rule in `auto_approve.rs` for filesystem prompts / command slices targeting a path inside a repository `.git/` directory (canonicalized, fail-closed on `..`/`~`), at danger-list precedence; reads unaffected; `git` subcommands unaffected
- [ ] 3.2 Unit-table tests: `echo … >> .git/info/exclude` and a write to `.git/config` escalate; `cat .git/config` does not match this rule
- [ ] 3.3 Verify parity in the `__classify` shell mirror

## 4. GP-04a — exclude managed bookkeeping from conflict overlap

- [ ] 4.1 In `src/broker/conflict.rs`, filter `is_managed_path()` entries out of each agent's modified-file set before computing overlap, in both forward-conflict and in-flight paths
- [ ] 4.2 Tests: a shared `.git-paw/config.toml` produces no conflict; a real `src/a.rs` overlap alongside `.git-paw/` still conflicts on `src/a.rs` only

## 5. GP-03a — per-CLI `Auto` permission flag

- [ ] 5.1 In `src/supervisor/config.rs`, map `("claude", Auto)` → `"--permission-mode acceptEdits"` in the built-in flag table; leave the override seam + fallback intact
- [ ] 5.2 Confirm `acceptEdits` against the live Claude `--permission-mode` enumeration (`default`/`acceptEdits`/`plan`/`bypassPermissions`); amend the spec row if upstream differs
- [ ] 5.3 Tests: `("claude", Auto)` resolves to `--permission-mode acceptEdits`; existing full-auto/override/fallback scenarios still pass

## 6. GP-03b — launch gate handles a first-run acceptance dialog

- [ ] 6.1 Extend the launch-readiness poll (tmux launch path) to recognise a first-run bypass/trust acceptance dialog as a distinct state and answer it (or fail the launch loudly), reusing the per-CLI readiness heuristic; conservative fallback for unrecognised CLIs
- [ ] 6.2 Ensure a pane blocked on an unanswered dialog is not reported a healthy running agent
- [ ] 6.3 Tests: dialog answered/failed rather than relaunched; stuck-on-dialog pane not reported healthy; unrecognised CLI falls back to prior behaviour

## 7. GP-03c — liveness reflects dead agent panes

- [ ] 7.1 Enrich the `is_tmux_alive` liveness probe feeding `effective_status` to require ≥1 live agent-pane CLI process, not mere session existence; tolerate the launch window (no flapping)
- [ ] 7.2 Tests: an all-panes-exited `Active` session resolves to `Stopped`; a session with ≥1 live pane stays `Active`
- [ ] 7.3 Confirm `git paw status` and `git paw doctor` report such a dead session as not-active (behavioural/e2e)

## 8. Docs

- [ ] 8.1 Configuration / security-posture reference: document the `.git/`-write danger rule and the managed-helper-script safe rule (GP-02b/04b)
- [ ] 8.2 Unattended-operation guide: document the deterministic per-CLI `Auto` permission mode and the launch liveness/acceptance-dialog gate (GP-03)
- [ ] 8.3 `mdbook build docs/` succeeds

## 9. Gates

- [ ] 9.1 `just check` (fmt + clippy + tests) green; no `unwrap()`/`expect()` in non-test code; public items documented
- [ ] 9.2 `just deny` clean
- [ ] 9.3 Gate-5 security review: `.git/`-write danger tightening verified; the managed-helper and prefix-normalization loosenings verified safe under the per-pane approval claim
- [ ] 9.4 Every spec scenario maps to a test (spec-audit traceability)

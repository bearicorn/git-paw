## 1. Selection seam (tmux byte-identical)

- [ ] 1.1 Rebase over `headless-backend-adapter`, `headless-drive-adaptation` and the archived v0.17.0–v1.0.0 changes named in the proposal (`cli-setup-consent` landed in v0.17.0; its consent is not a gate condition, design D11)
- [ ] 1.2 Core `BackendKind { Tmux, Headless }`; config wire key `default_backend` (validated values, top-level merge, serialized only when set, commented example in the init template); `--backend` value flag with help text and a long-about example
- [ ] 1.3 Backend factory: the only `match` on `BackendKind`; every lifecycle handler (`start`, `add`, `remove`, `recover`, `pause`, `stop`, `purge`, `status`, `attach`) and its v0.18.0 library form constructs the adapter from the session's recorded backend; `ensure_tmux_installed` only on the tmux path
- [ ] 1.4 Session wire type: optional `backend`; discovery wire type: optional `backend` and `headless_agents`, `agents: []` for headless
- [ ] 1.5 Tests: `--backend` parses both values, `zellij` exits 2 with nothing created, absent flag/key → tmux launch unchanged, help lists the flag; `default_backend` parse / invalid / repo-wins / round-trip; tmux session and discovery files byte-identical to pre-change fixtures; an older session file loads as tmux; a headless session file carries `"backend": "headless"`
- [ ] 1.6 Gate: `just check` green; the e2e, tmux and session-state suites pass unchanged

## 2. Eligibility gate and turn plan

- [ ] 2.1 Pure `headless_eligibility(record, level, matrix) -> Result<Eligible, Vec<Refusal>>` with conditions (a)–(d) and the matrix-evidence / user-declared rule; refusals carry the condition and the fixing key
- [ ] 2.2 Turn-plan composition in the one launch builder: executable, `args`, `headless_args`, sandbox arguments, approval flags, session-id (first) / resume (continuation); one session id recorded in `cli_session_id`
- [ ] 2.3 Wire the gate before any side effect on `start` (fresh and recovery), `add`, MCP `start_session` and `--dry-run`; print all refusals together; exit 1
- [ ] 2.4 Helper-call notice (design D11): for an admitted `auto` worker in a broker session, print one notice per CLI naming `git paw setup-cli <cli> --allowlist` when the setup fact row declares the item, else the CLI's own configuration / `[clis.<name>].args`; never refuse, never write settings. Tests: *The notice names setup-cli for claude* (settings file untouched), *No notice at full-auto*
- [ ] 2.5 Tests: the six-row eligibility table; the unverified-built-in refusal; a user-declared CLI launches with a notice; full-auto flags never appear at `auto`; the Claude-family plan (exact order) and the args/sandbox-once plan; a refused `start` creates no worktree/branch/session file/run state/broker; two refusals in one run; `add` refused before creating its worktree

## 3. Headless lifecycle

- [ ] 3.1 Launch: open session, session env, broker host via `spawn_service` + bounded `/status` wait, per worker `spawn_surface` + `stream_to` (capture on) + boot prompt as the first message; no dashboard surface, no readiness gate; next-step hint names `git paw status` and `git paw attach`; `--dry-run` prints backend, eligibility and turn plans
- [ ] 3.2 Revival on `start`: continuation to interrupted workers, broker host restarted if dead, then follow output when interactive (refuse to fork otherwise); lost run state → re-open and respawn with the continuation as the first message; conflicting `--backend` → exit 1 naming both
- [ ] 3.3 `remove` kills the surface before removing the worktree; `pause` stops the broker service only; `stop` closes the session and keeps the session file; `purge` closes and removes the run state; `add` to a paused session leaves the boot prompt as a draft
- [ ] 3.4 `status`: headless human output (backend line, per-worker activity); JSON `backend` and per-worktree `activity`; `terminal_alive` from the backend, `tmux_running` mirroring it; `attach` dispatch
- [ ] 3.5 E2E with a stub CLI and a `PATH` without `tmux` (`assert_cmd`, `tempfile`, `serial_test`): launch exits 0, both workers ran a boot turn, broker answers; capture holds `booted`; dry run creates nothing; revival resumes only the interrupted worker; conflicting backend refused with no tmux session created; lost run state resumes with the recorded id; stop leaves no process; pause frees the port while a turn finishes; remove stops the CLI before the worktree goes; human and JSON status of a headless session; tmux status byte-identical; `attach` shows labelled output and leaves the turn running

## 4. Doctor, sweep.sh, MCP, matrix

- [ ] 4.1 Doctor: tmux missing is ⚠ under `default_backend = "headless"`, ✗ otherwise; the headless eligibility check (✓ / ⚠ / ✗ with fixes), absent for tmux users; `--json` ids follow v0.21.0's naming
- [ ] 4.2 `assets/scripts/sweep.sh`: `snapshot`, `capture`, `approve`, `detect-stuck` read `backend` from the discovery file and refuse on `headless` (no tmux call, message names `git paw status` / `attach`); keep heredocs apostrophe-free and run `bash -n`; sync the tracked `.git-paw/scripts/sweep.sh` copy
- [ ] 4.3 MCP `start_session`: optional `backend` in the schema; same gate; MCP-level error listing refusals; omitted parameter unchanged
- [ ] 4.4 Support matrix: `headless-worker` status per row outside `derive_tier`; evidence rule in `required_evidence`; doctor tier report and docs-page parity cover it; capture the `claude` help-text fixture and the two attestations (continued conversation; refused action at `auto` ends the turn), recording `gap` with a note if either fails
- [ ] 4.5 Tests: doctor ⚠/✗ tmux rows, ineligible ✗ with the fix, user-declared ⚠, no check for tmux users; `sweep.sh capture` refuses on a headless discovery file and `status-publish` still works; MCP headless start, MCP refusal creates nothing, schema lists both values; tier unchanged by `headless-worker`, verified-without-evidence fails, docs-page mismatch fails

## 5. Docs and contract

- [ ] 5.1 New `docs/src/user-guide/headless.md` (selection, every gate condition with its key, the turn model, inbox wake, lifecycle differences, `auto` refusing everything outside the CLI policy and the helper-call notice with `git paw setup-cli`, sandboxed full-auto residual risks, inherited environment, native Windows unsupported); add to `SUMMARY.md`
- [ ] 5.2 `docs/src/cli-reference.md` (`--backend`, `attach` on headless), configuration reference (`default_backend`), `supported-clis.md` (headless column), README feature row and quick example, security posture chapter (headless section), `architecture.md` (backend factory); "tmux is a hard dependency" → "required for the tmux backend (the default)" in README/docs and AGENTS.md
- [ ] 5.3 Regenerate the v1.0.0 contract manifest with the additive entries (`start --backend` + values, `default_backend`, MCP `start_session.backend`, status JSON `backend` / `activity`, optional session and discovery fields); add the MODIFIED delta of *The coordinated-session vocabulary is frozen* broadening *worker* to "a coding agent in a worktree surface (a pane under tmux)"
- [ ] 5.4 Gate 4 (`doc-completeness`) and Gate 5 (`security-and-safety-review` against design.md S1–S7, `export-agnosticism` for the `sweep.sh` edit); `mdbook build docs/` succeeds
- [ ] 5.5 Every scenario maps to a named test; `just check` and `just deny` green; full suite green against the merge-base with `cargo test --no-fail-fast`; no `unwrap`/`expect` in non-test code; exit codes verified with `cmd; echo $?`, not through a pipe

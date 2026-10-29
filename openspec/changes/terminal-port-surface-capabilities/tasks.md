## 1. Port (seam first, tmux unchanged)

- [ ] 1.1 Rebase over the archived `terminal-backend-seam` and `broker-host-separation`, and over `headless-backend-adapter`; confirm the trait members this change touches (`SurfaceInfo.position`, `surface_at`, `foreground_process`, `prefill_text`, `relaunch_surface`, `answer_dialog`, `capture_tail`, `surfaces_are_interactive`)
- [ ] 1.2 `src/terminal/mod.rs`: a per-surface interactivity query; `SurfaceInfo.position: Option<usize>`; `surface_at` returns `Option<SurfaceId>`; doc comments say which members are interactive-only and that `capture_tail` returns output, a screen only for interactive surfaces
- [ ] 1.3 `TmuxBackend`: every surface interactive with `Some(position)`; no new tmux invocation
- [ ] 1.4 Headless adapter: every surface non-interactive with no position; `prefill_text` / `relaunch_surface` / `answer_dialog` return `PawError::Unsupported`; `foreground_process` returns `None`
- [ ] 1.5 `FakeBackend`: per-surface interactivity and position scripting; records the operations it refuses
- [ ] 1.6 Tests: "Capabilities are reported per surface", "A non-interactive surface refuses interactive-only operations", "tmux surfaces are interactive and positioned" (golden fixtures unchanged), "Surfaces of one session answer separately", the three interactivity scenarios carried from `headless-backend-adapter`
- [ ] 1.7 Gate: `just check` green; the tmux, e2e and drive-loop suites pass unmodified

## 2. Callers and resolver

- [ ] 2.1 Guard the launch-readiness gate (relaunch, dialog answer), the entry-point idle wait and submit, buffered-input submission, nudge verification and recovery, and the paused-session prefill on the target surface's interactivity; record a skip as not applicable
- [ ] 2.2 Screen predicates (idle markers, live-prompt gate, buffered input, mid-response, CLI-unavailable banner, bare interpreter) run only on interactive surfaces
- [ ] 2.3 `src/surface.rs`: a position-less surface resolves by owner to `Coding` or `Unknown`, never a reserved role; no reserved index derived from it
- [ ] 2.4 Tests: "The readiness gate skips a non-interactive surface", "An entry point is never typed into a non-interactive surface by the idle wait", "Interactive sessions are unchanged" (existing fake tests unmodified), "A position-less surface owned by a worktree is a coding surface", "A position-less surface with no owner match is unknown"

## 3. Guards and docs

- [ ] 3.1 Extend the port vocabulary audit with the new identifiers (`output`, `capability` concepts); keep the token ban
- [ ] 3.2 `docs/src/architecture-direction.md` Terminal row and `docs/src/architecture.md`: the port describes each surface by its capabilities; `mdbook build docs/` succeeds
- [ ] 3.3 Every scenario maps to a named test; `just check` and `just deny` green; full suite green against the merge-base with `cargo test --no-fail-fast`; no `unwrap`/`expect` in non-test code; exit codes verified with `cmd; echo $?`, not through a pipe

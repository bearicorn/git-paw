# Tasks — code-analysis-refactor

Source of record: the four `.git-paw/v0.13.0-wave3-code-analysis-*.md` analyses. Every task is
behavior-preserving. Standing constraint (all waves): the **Do-not-touch** list in `design.md`
stays byte-identical, and the **five-gate verification** below runs cold-start before a wave
lands. Verify a gate by its **real exit code**, never piped output (a trailing `| tail` masks a
failure).

## R0 — Baseline & triage (no code change)
- [ ] Capture a `just coverage` per-module baseline
- [ ] Confirm `just verify` (`GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`) green in a **clean cold-start env** (`just smoke` / `just smoke-container`) — NOT the live dogfood session (local-pass ≠ CI-pass)
- [ ] Split the combined findings into **refactor (behavior-preserving, this change)** vs **bugfix (behavior-changing → `path-injection-hardening` / `broker-runtime-hardening`)**
- [ ] Gate: baseline recorded; suite green cold; triage done

## R0 — Baseline & triage — DONE
- [x] Suite green cold-start confirmed (fresh-worktree full run: 2360 passed, the 1 fail is a known sweep.sh time-window flake, passes isolated — not a regression). Coverage baseline established during test-suite-consolidation. Refactor-vs-bugfix triage done (bugfixes routed to path-injection/broker-runtime; NOT folded here).

## R1 — Low-risk extractions behind the existing (non-PTY) net _(independent — start now)_
- [x] **Idiom/hygiene (D7) subset DONE** (each finding re-verified vs current code): dropped unused `anyhow` (+AGENTS.md row); deleted dead `detect.rs::resolve_command` (note preserved on `resolve_command_in`); removed the 2 vestigial `#[allow(dead_code)]` on tmux.rs; added the missing `///` on `agents.rs::inject_section_into_file`. **DEFERRED within R1:** the 6 logic-invariant `expect()→?` conversions (behavior-adjacent; dedicated per-site pass). [committed]
- [ ] `config.rs → config/{mod,supervisor,broker,dashboard,specs,cli,layout}.rs` — pure move + re-export; all `#[serde]` attrs / `Default` / `merged_with` **verbatim** (§ do-not-touch) — NEXT
- [ ] **GATING NOTE:** R2 (`main.rs` split — the headline conflict-hotspot win) is BLOCKED on `cli-interaction-e2e` (#2) finishing + the source-grep test removal; R3 (tmux runtime) also needs the dashboard CPU-leak fix. So #2 is the true unblocker for #6's high-value waves.
- [ ] `tmux.rs` builder (`command_strings` + argv builders) → `tmux/command.rs`; runtime path untouched
- [ ] `interactive.rs` resolver logic → module; live `dialoguer` impls untouched
- [ ] `dashboard.rs` draw/format helpers → module; event loop + SIGHUP untouched
- [ ] Introduce the `CommandRunner` trait + real runner (behavior-identical) as the seam type; wire the already-covered builder call sites through it
- [ ] Introduce `SessionName`/`BranchSlug`/`WorktreePath` newtypes at construction points — output **byte-identical** to current inline formatting (NO new sanitization)
- [ ] Idiom/hygiene (D7): drop unused `anyhow` + its `AGENTS.md` row (F1); delete dead `detect.rs::resolve_command` + allow (F3); remove the two vestigial `#[allow(dead_code)]` on `tmux.rs` public helpers; convert the 6 logic-invariant `expect()` → `?`/restructure (F2a); add the missing `///` on `agents.rs::inject_section_into_file`
- [ ] Gate: `just verify` green cold + `just deny`; coverage ≥ baseline per touched module; reviewer confirms the diff is pure move/re-export (no line traces to a behavior change)

## R2 — Unblock + split `main.rs` _(GATED: PTY net + source-grep removal)_
- [ ] **Precondition — W1 PTY net:** `cli-interaction-e2e` matrix covers the interactive / `--from-specs` / destructive-confirm dispatch and pane/prompt outcomes
- [ ] **Precondition — source-grep removal:** `test-suite-consolidation` has removed/replaced `tests/source_audit.rs`, `tests/cli_specs_tty_proceeds_to_picker.rs`, `tests/cli_from_specs_boot_block_failure.rs`, and the inline `main.rs` `include_str!` brace-walks — each confirmed to have a behavioral counterpart before deletion
- [ ] Split `main.rs → src/commands/*.rs` + thin `main`/`run` dispatch — leaf helpers first, then the tmux-orchestration `cmd_*` (`cmd_start`, `launch_spec_session`, `recover_*`, `restart_from_pause`, `attach_agent`) through the `CommandRunner` seam
- [ ] Add `CommandRunner`-mocked unit tests asserting the tmux/git **argv** of the `cmd_*` handlers without spawning real processes
- [ ] Enum-ripple guard: grep `BrokerMessage` / `SpecBackendKind` across `src/` — exhaustive matches stay co-located and compiler-caught
- [ ] Gate: `just verify` green cold + the PTY matrix green **serially**; coverage ≥ baseline; behavior parity via e2e (exit codes / stdout / `session.json` / panes), never source structure

## R3 — tmux runtime + remaining interactive/dashboard _(GATED: PTY net + dashboard CPU-leak fix)_
- [ ] **Precondition:** PTY net proven stable AND the dashboard CPU-leak fix (`fix/dashboard-cpu-leak`) landed
- [ ] Refactor the `tmux.rs` runtime path (send-keys / capture-pane parsing / pane-spawn — 59 `Command::new` sites) behind the `CommandRunner` seam
- [ ] Interactive live-prompt impls behind the PTY net
- [ ] Dashboard event loop (rebase on `fix/dashboard-cpu-leak`); SIGHUP `unsafe` path still untouched by this change
- [ ] Gate: PTY matrix + cold-start smoke green serially; manual dashboard smoke; coverage ≥ baseline

## R4 — Broker structural tidy _(DEFERRED post-freeze — optional, lowest priority)_
- [ ] Surgical, lock-discipline-preserving moves only in `broker/{delivery,mod,conflict}.rs`; NO reordering of lock / `.await` / `spawn`
- [ ] Extend `mcp_e2e.rs` to assert the thin query-module tool outputs **before** touching them
- [ ] Gate: `just verify` green cold + a cargo-mutants spot-check on `messages.rs`/`conflict.rs`/`delivery.rs` proving the behavioral net still kills the same mutants; serial e2e

## Standing do-not-touch constraint (every wave)
- [ ] Frozen serde surfaces byte-identical: `FileIntent` untagged, `AdvancedMain` flatten, `StatusPayload.message` no-`skip_serializing_if`, `SpecsConfig` `rename="type"`, `Session.created_at` serializer, `merged_with` default-as-"unset"
- [ ] `RepoSessionFile`/`RepoAgentEntry` shape unchanged (cross-process `sweep.sh`/`broker.sh` contract)
- [ ] Broker lock discipline preserved (no reorder of lock/`.await`/`spawn`); `clippy::await_holding_lock` stays clean
- [ ] Dashboard/SIGHUP `unsafe` path untouched
- [ ] `BrokerMessage`/`SpecBackendKind` variant sets unchanged
- [ ] No public CLI flag, config key, or broker wire shape changed

## Verification (five gates, at every wave — supervisor)
- [ ] Gate 1 — Testing: the wave's own tests pass (`cargo test --no-fail-fast` so the env-guard test can't mask failures)
- [ ] Gate 2 — Regression: full suite green vs the **merge-base**, cold-start; e2e serialized
- [ ] Gate 3 — Spec audit: every `code-architecture` scenario maps to a test (behavioral surface, `CommandRunner` argv assertions, re-export/compile checks); `openspec validate code-analysis-refactor --strict` passes
- [ ] Gate 4 — Doc audit: `docs/src/architecture.md` module table + subsystem sections match the new tree; `mdbook build docs/` passes; no `--help`/README/config-reference drift (no CLI/config surface moved)
- [ ] Gate 5 — Security: no secrets; least-privilege preserved; export-agnosticism intact; no frozen surface changed
- [ ] `cargo fmt` before every commit; `just check` + `just deny` green, confirmed by real exit code

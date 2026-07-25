# Design — code-analysis-refactor

## Context

Behavior-preserving refactor of `src/` into the patterns decided by the `code-standards`
skill, executed in test-gated waves before the v1.0.0 freeze. The four wave-3 analyses
(`.git-paw/v0.13.0-wave3-code-analysis-*.md`) are the source of record: the
principal-engineer lens sets the wave ordering, the do-not-touch list, and which refactors
are net-gated; the architect lens sets the module splits; the rust-expert lens sets the
idiom/dead-code subset; the oss-maintainer lens sets the surface-hygiene subset.

The governing premise (principal-engineer §0, §8): the code is defensively written and
well-pinned, so the payoff is **readability, not correctness** — bias conservative, land the
cheap high-ROI splits, and route the latent *bugs* to their own changes. "Raw LOC lies":
production sizes are `main.rs` ~4041 > `config.rs` ~1956 > `tmux.rs` ~1726; `skills.rs` prod
is only ~879 (its bulk is a test/asset block → `test-suite-consolidation`).

## Decisions

### D1 — Byte-identical observable surface is the gate, proven by the existing behavioral net

Every wave is a pure structure move. The contract: the CLI (exit codes, stdout/stderr), the
config TOML schema, the broker wire bytes, and on-disk `session.json` are **byte-identical
before and after**. The proof is the existing behavioral suite (30 e2e tests + serde
round-trip/back-compat tests) run against the merge-base before and after each wave — not the
source-grep tests, which assert *structure* and are removed (D5). "Reduce redundancy, never
reduce coverage": a wave that drops coverage on a real branch cut a sole guard — restore it.

### D2 — `CommandRunner` seam only where behavior is otherwise untestable

Introduce a `CommandRunner` trait (ports & adapters) so the tmux/git orchestration path
depends on an injectable seam: production wires the real runner (identical `std::process::Command`
behavior), tests inject a fake recording argv and returning scripted output. Per the NFR
conflict table (testability ↔ simplicity), the seam earns its keep only on the blind runtime
surface (`tmux.rs` runtime path — 59 `Command::new` sites, zero e2e today; the tmux-orchestration
`cmd_*` in `main.rs`). It is **not** retrofitted onto already-covered pure code. The seam is
behavior-preserving: the real runner does exactly what the inline calls do now.

### D3 — Domain newtypes are a construction seam, not new sanitization

`SessionName` / `BranchSlug` / `WorktreePath` centralize construction of injection-prone
strings at one point. In THIS change their `Display`/`as_str` output is **byte-identical to the
current inline `format!` output for every current input** — no space/dot sanitization is added,
because that is an observable behavior change (the CF1 session-name bug). The newtype is the
seam that `path-injection-hardening` later hardens with sanitize/quote-at-construction. Keeping
them separate honors "a behavior change is not a refactor" and the security↔back-compat rule
(security overrides compat only via an explicit versioned migration, never silently inside a
refactor).

### D4 — Module splits preserve re-exports and enum-ripple co-location

- `main.rs → src/commands/{start,add,remove,dashboard,supervisor,status,purge,replay,approvals,recover,…}.rs`;
  `main`/`run` keep only dispatch + arg wiring. Handlers already take plain args and return
  `Result<(), PawError>`, so moves are mechanical. `main.rs` is a `[[bin]]` — nothing depends on
  its internals, so the split is consumer-ripple-free.
- `config.rs → config/{mod,supervisor,broker,dashboard,specs,cli,layout}.rs`. `config` has fan-in
  61 (`crate::config`), so the split MUST re-export every type at the old path; all `#[serde]`
  attrs, field names, and `Default`/`merged_with` logic move **verbatim**.
- `tmux.rs → tmux/{command,session,readiness,layout}.rs`; `pub` paths re-exported from `tmux`.
- **Enum-variant ripple guard:** the `BrokerMessage` and `SpecBackendKind` exhaustive `match`es
  stay co-located (AGENTS.md hazard) so a missed arm is a compile error, not a silent inert
  variant. No split may scatter them across files.

### D5 — `main.rs` is doubly-gated; the source-grep tests are the unblocker

`main.rs`'s dispatch is behaviorally covered by black-box e2e for exit/stdout/`session.json`,
but three source-grep files + inline `include_str!("main.rs")` brace-walks
(`tests/source_audit.rs`, `tests/cli_specs_tty_proceeds_to_picker.rs`,
`tests/cli_from_specs_boot_block_failure.rs`, and `src/main.rs:4417+/:4474+`) are keyed to the
exact source structure and **false-positive-break on ANY split while giving zero runtime
protection**. And the tmux-orchestration `cmd_*` (pane/prompt wiring) are not fully asserted by
black-box e2e. So the split waits for **(a)** `test-suite-consolidation` deleting/replacing the
source-grep tests with behavioral counterparts, and **(b)** the `cli-interaction-e2e` PTY net
covering the interactive/from-specs/pane-wiring dispatch. Pure leaf helpers
(`resolve_supervisor_mode`, `config_to_custom_defs`, `expand_tilde`, `resolve_submit_delay_ms`)
can move earlier; the tmux-orchestration `cmd_*` wait for the net.

### D6 — Broker structural refactor is the trap → defer post-freeze (R4)

The broker has a strong behavioral net but load-bearing, partly-untested concurrency invariants
(single-threaded router tests, `role_gating = None`). A structural move there is Medium risk for
Low ROI. Any R4 work is surgical, lock-discipline-preserving (no reorder of lock/`.await`/`spawn`),
extends `mcp_e2e.rs` first, and gates on a cargo-mutants spot-check proving the net still kills
the same mutants. Recommendation: **post-freeze.**

### D7 — Idiom/hygiene is the behavior-preserving subset only

From the rust-expert + oss-maintainer lenses, only items with zero observable effect are in
scope: drop unused `anyhow` (F1); delete dead `detect.rs::resolve_command` + its allow (F3);
remove the two vestigial no-op `#[allow(dead_code)]` on `tmux.rs` public helpers; convert the
six logic-invariant `expect()` sites (`main.rs:2599/3879`, `speckit.rs:246/261`,
`learnings.rs:100`, `inventory.rs:399`) to `?`/restructure (F2a); add the one missing `///`
(`agents.rs::inject_section_into_file`). Explicitly **NOT** here: static-regex/lock-poison
`expect()` (accepted idiom — a docs decision, not code churn), `PawError` Display-string
normalization (observable), the S1 `#[serde(default)]` wire relax and the `#[doc(hidden)]`/MSRV
freeze-surface decisions (those are `wire-api-freeze-prep`, observable/semver-shaping).

## Do NOT touch (frozen for v1.0.0)

A "consistency cleanup" on any of these silently breaks a frozen contract. Leave byte-identical:

- **Silently-breaking serde:** `FileIntent` `#[serde(untagged)]`, `AdvancedMain` `#[serde(flatten)]`,
  `StatusPayload.message` with NO `skip_serializing_if` (v0.5.0 byte-compat), `SpecsConfig.spec_type`
  `#[serde(rename = "type")]`, `Session.created_at` custom `serialize_with`/`deserialize_with`, and
  `PawConfig::merged_with` default-as-"unset" merge semantics (Default values are load-bearing).
- **`RepoSessionFile`/`RepoAgentEntry`** cross-process shape (consumed by `sweep.sh`/`broker.sh`) —
  no field changes.
- **Broker lock discipline** — never hold a lock across `.await`; do not reorder lock/`.await`/`spawn`
  in `delivery.rs`/`mod.rs`/`watcher.rs`.
- **Dashboard/SIGHUP `unsafe` path** (`main.rs:3137-3170`, `dashboard.rs:61/118`) — untouched here;
  the CPU-leak fix lands on its own branch.
- **`BrokerMessage`/`SpecBackendKind` variant sets** — no add/remove (out of scope for a
  behavior-preserving refactor anyway).
- **Any public CLI flag, config key, or broker wire shape.**

## Risks

- **A "pure move" silently changes behavior** — mitigated by D1 (behavioral suite before/after vs
  merge-base) and the per-wave five-gate verification; reviewer confirms every changed line traces
  to a structure move, not a behavior change.
- **A split scatters an enum-ripple match** → a new variant later goes silently inert — mitigated by
  D4's co-location guard (grep the variant name across `src/` before finishing).
- **Removing the source-grep tests before their behavioral counterpart exists** drops real coverage —
  mitigated by sequencing R2 strictly after `test-suite-consolidation` confirms each counterpart
  (`e2e_supervisor_launch`, `cli_supervisor_non_tty`, `hook_integration`, `boot_block_integration`).
- **Broker structural work** — highest downside/lowest ROI; deferred (D6).

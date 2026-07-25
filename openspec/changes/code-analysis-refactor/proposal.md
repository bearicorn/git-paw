## Why

The v0.13.0 wave-3 code-analysis pass (four lenses:
`.git-paw/v0.13.0-wave3-code-analysis-{principal-engineer,architect,rust-expert,oss-maintainer}.md`)
found git-paw is **defensively written and unusually well-pinned** — 99.9% doc coverage,
no bare `unwrap()`, lint-enforced broker lock discipline, strong serde back-compat. The
remaining wins are **readability and merge-conflict-surface reduction, not correctness**:
the true production god-modules (`main.rs` ~4041 prod lines, then `config.rs` ~1956) mix
altitudes and are the repo's worst parallel-dogfood conflict hotspots, and the blind tmux/git
runtime path is unit-untestable because `std::process::Command` is called inline in logic.

This change is the **"apply" sibling of the already-authored `code-standards` skill** — it
executes the skill's decided patterns (the `CommandRunner` seam, domain newtypes, the
module-domain splits) as a **behavior-preserving refactor in test-gated waves**, before the
v1.0.0 freeze locks the observable surface. "Raw LOC lies": `skills.rs` looks largest but is
~90% a test/asset block — that is a `test-suite-consolidation` job, not a production split
here.

Behavior changes are explicitly NOT in scope. The latent path/session-name and broker
concurrency **bugs** the analyses surfaced ship as their own spec+test-gated changes
(`path-injection-hardening`, `broker-runtime-hardening`), each with a reproducing test —
never folded into a refactor wave.

## What Changes

- **Introduce a `code-architecture` capability** capturing the enforceable structural +
  regression contract a refactor must uphold: the public CLI / config / broker-wire surface
  stays byte-identical (proven by the existing behavioral suite before/after), process/tmux/git
  invocation becomes reachable through an injectable `CommandRunner` seam, injection-prone
  strings flow through a single newtype construction point, the codebase is organized into the
  enumerated domain modules, and the frozen serde/wire/lock/SIGHUP surfaces are not touched.
- **`CommandRunner` process-execution seam** (ports & adapters): logic depends on a
  `CommandRunner` trait; production uses the real runner, tests inject a fake that records argv
  and returns scripted output — making the tmux/git orchestration path unit-testable instead
  of e2e-only. Production behavior unchanged (the real runner shells out exactly as today).
- **Domain newtype seam** (`SessionName`, `BranchSlug`, `WorktreePath`): injection-prone values
  flow through one constructor whose output is **byte-identical to today's inline formatting**
  for current inputs. This establishes the seam that `path-injection-hardening` later hardens;
  it adds no sanitization here (that would be an observable behavior change).
- **Module-domain splits** (pure code-move + re-export, TOML/wire schema untouched):
  `main.rs → src/commands/*.rs` + thin dispatch; `config.rs → config/{mod,supervisor,broker,dashboard,specs,cli,layout}.rs`;
  `tmux.rs → tmux/{command,session,readiness,layout}.rs`; plus `interactive.rs` /
  `dashboard.rs` helper extractions. All old `crate::{config,tmux}::*` paths preserved via
  re-export.
- **Idiom / surface hygiene** (behavior-preserving subset only): drop the unused `anyhow`
  dependency (F1), delete the one genuinely-dead `detect.rs::resolve_command` (F3), remove the
  two vestigial no-op `#[allow(dead_code)]` on `tmux.rs` public helpers, convert the logic-invariant
  `expect()` sites to `?`/restructure (F2a), and add the one missing `///` on
  `agents.rs::inject_section_into_file`.
- **Wave ordering R0–R4 with a gate per wave** (see `tasks.md`): R1 low-risk extractions start
  now behind the existing net; `main.rs` split (R2) is **doubly-gated** on the W1 PTY net
  (`cli-interaction-e2e`) AND removal of the source-grep introspection tests (`test-suite-consolidation`);
  the tmux runtime path (R3) is gated on the PTY net + the dashboard CPU-leak fix; the broker
  structural tidy (R4) is **deferred post-freeze**.

## Capabilities

### New Capabilities
- `code-architecture`: the enforceable structural + regression contract for behavior-preserving
  refactors — byte-identical observable surface (CLI/config/wire), the `CommandRunner` injectable
  seam, the domain-newtype construction seam, the enumerated domain-module layout with preserved
  re-exports, the frozen do-not-touch surfaces, and the test-gated wave discipline.

### Modified Capabilities
_None._ This change adds no observable behavior, so it restates no existing product requirement.
The `code-architecture` contract governs *how* the code is structured; the CLI/config/wire specs
it protects are unchanged by construction.

## Impact

- **Code (structure only):** `src/main.rs` → new `src/commands/*.rs`; `src/config.rs` → `src/config/`
  tree; `src/tmux.rs` → `src/tmux/` tree; a `CommandRunner` trait + real/fake impls threaded through
  the tmux/git call sites; `SessionName`/`BranchSlug`/`WorktreePath` newtypes at the construction
  points. Every move preserves public re-exports so `main.rs` and the `assert_cmd` integration tests
  (separate crates consuming `git_paw::…`) compile unchanged.
- **Enum-variant ripple (must NOT scatter):** the `BrokerMessage` and `SpecBackendKind` exhaustive
  `match`es (AGENTS.md hazard) stay co-located after any split so the ripple stays compiler-caught.
  No variant is added or removed — a refactor never touches the variant set.
- **Cargo.toml / AGENTS.md:** remove the `anyhow` dependency line and its approved-set row (F1).
- **Do-not-touch (frozen for v1.0.0):** `FileIntent` untagged, `AdvancedMain` flatten,
  `StatusPayload.message` no-`skip_serializing_if`, `SpecsConfig` `rename="type"`,
  `Session.created_at` custom serializer, and `PawConfig::merged_with` default-as-"unset" merge
  semantics stay byte-identical; broker lock discipline (no reordering of lock/`.await`/`spawn`) is
  preserved; the `dashboard.rs`/`main.rs` SIGHUP `unsafe` path is untouched here (its CPU-leak fix
  lands separately on `fix/dashboard-cpu-leak`).
- **Gating dependencies:** `cli-interaction-e2e` (W1 PTY net) and the source-grep test removal in
  `test-suite-consolidation` gate R2/R3. R4 (broker) is recommended post-freeze.
- **Docs:** architecture docs (`docs/src/architecture.md` module table + subsystem sections) updated
  to the new module tree; `mdbook build docs/` must pass. No `--help` / README CLI / configuration-reference
  change (no CLI or config surface moves).

---
name: code-standards
description: git-paw's code patterns and refactor rules — how to structure, decouple, and refactor code so it stays testable and behavior-preserving. Use when implementing a feature, decoupling a module, refactoring, or reviewing/gating a change. Defines the injectable seams, domain module layout, error/idiom rules, and the frozen do-not-touch zones. Consulted by both the implementing agent (author-time) and the supervisor's review gate.
license: MIT
compatibility: git-paw (Rust, cargo, tmux)
---

# Code Standards

Use when **writing**, **decoupling**, **refactoring**, or **reviewing** git-paw code. Two
consumers: the **implementing agent** (write conformant code from the start) and the
**supervisor** (verify conformance at the review gate). See the `test-strategy` skill for the
test side.

## Why these standards — non-functional requirements

Every rule here serves a quality attribute; the standards are how git-paw resolves the tensions
between them. Its **defining** NFRs — because it auto-approves commands and runs unattended in real
repos, into a v1.0.0 freeze — are **security & safety**, **reliability under unattended operation**,
and **stability / back-compat**, above the usual maintainable/testable/documented. When NFRs
conflict, resolve by precedence: **Safety → Reliability → Contract stability → Internal quality →
DX**. Two rules fall straight out and bind every change: **security wins ties** (deny-by-default,
least privilege), and **security may override back-compat only via an explicit versioned migration,
never silently**. Full set, conflict table, and the review-gate test:
[references/non-functional-requirements.md](references/non-functional-requirements.md).

## Non-negotiable rules

- **No `unwrap()`/`expect()` in non-test code** — propagate with `?` via a `PawError` variant
  (thiserror). Accepted exceptions: static-regex compilation and lock-poison; document them,
  don't churn them to `?`.
- **Docs**: every public item has `///`; every module has `//!`.
- **Errors** go through `PawError`. Prefer a small constructor (`PawError::session(e)`) over
  repeating `map_err(|e| PawError::X(format!(…)))`.
- **Behavior-preserving refactors** touch structure only — never the observable CLI / config /
  wire / file surface. A behavior change is not a refactor (see below).

## Decoupling patterns — reach for these

- **Process-execution seam (ports & adapters).** Don't call `std::process::Command` inline in
  logic. Depend on a `CommandRunner` trait; production uses the real runner, tests inject a
  fake that records argv and returns scripted output. This is what makes tmux/git orchestration
  unit-testable instead of e2e-only.
- **Domain newtypes with smart constructors** for injection-prone strings — `SessionName`,
  `BranchSlug`, `WorktreePath`. Sanitize/quote once at construction so downstream code can't
  interpolate a raw untrusted string. (Kills the session-name / path-quoting bug class.)
- **Hidden-subcommand IPC seam** (`__dashboard`, `__classify`). When a shell helper needs Rust
  logic, expose `git paw __<verb>` and let the shell shell out — never re-implement the logic
  in bash. One source of truth; the export stays a thin client.
- **Command-handler modules + thin dispatch** — one file per command family under `commands/`;
  `main`/`run` only dispatch.

## Module domains — where code goes

- Keep subsystem code in its domain: `broker/`, `supervisor/`, `specs/`, `mcp/`, `dashboard/`,
  plus the planned splits `commands/`, `config/`, `tmux/{command,session,readiness,layout}`.
- Split a module when it mixes altitudes/responsibilities — but **raw LOC lies**: measure
  *production* lines (strip `#[cfg(test)]`), and extract giant inline test blocks into `#[path]`
  child files first.
- Keep `BrokerMessage` / `SpecBackendKind` exhaustive `match`es co-located so the variant ripple
  stays compiler-caught.

## Rust best practices

Follow the Rust API Guidelines — full checklist in
[references/rust-api-guidelines.md](references/rust-api-guidelines.md). What binds here:

- **Error types** — `PawError` is meaningful + well-behaved (`Error`/`Display`, `Send+Sync`); docs
  note Errors/Panics; examples use `?`, never `unwrap` (`C-GOOD-ERR`, `C-FAILURE`, `C-QUESTION-MARK`).
- **Newtypes + validation** — static distinctions via newtypes; validate arguments at the boundary
  (`C-NEWTYPE`, `C-VALIDATE`) — the injection-hardening seam.
- **Common traits + Debug** on public types; constructors are static inherent methods
  (`C-COMMON-TRAITS`, `C-DEBUG`, `C-CTOR`); conversions via `From`/`TryFrom`/`AsRef` (`C-CONV-TRAITS`).
- **v1.0.0 lib surface** — private struct fields, sealed traits, `#[doc(hidden)]` internals, pinned
  MSRV, stable public deps (`C-STRUCT-PRIVATE`, `C-SEALED`, `C-HIDDEN`, `C-METADATA`, `C-STABLE`).

## CLI & dev-tool best practices

Follow the CLI guidelines — full reference in
[references/cli-and-devtool-design.md](references/cli-and-devtool-design.md). What binds here:

- **stdout = primary/machine output; stderr = logs/errors/progress** — never mix. `--json` for
  machine surfaces; `--plain`/`--quiet` where useful.
- **Respect the TTY** — only prompt on a TTY; `--no-input` disables prompts; disable color off-TTY
  or under `NO_COLOR`.
- **Destructive ops confirm; `--force` bypasses**; non-TTY without `--force` errors with guidance.
- **Exit 0 / non-zero with distinct failure codes** (`PawError` mapping); **actionable errors**
  (what failed + how to fix — the `doctor` remedy lines).
- **Config precedence** flags > env > repo > user; **XDG** dirs. **Additive changes**; machine
  output is the stable contract; **no telemetry without consent**.
- **Dev-tool ergonomics** — self-diagnostics (`doctor`/`selftest`), single-binary distribution,
  minimal license-clean deps, deterministic/composable output, cross-platform care (macOS/Linux;
  Windows via WSL).

## Do NOT touch (frozen for v1.0.0)

- Silently-breaking serde: `FileIntent` untagged, `AdvancedMain` flatten, `StatusPayload.message`
  no-`skip_serializing_if`, `SpecsConfig` `rename="type"`, `Session.created_at` custom serializer,
  and the `merged_with` default-as-"unset" merge semantics.
- Broker lock discipline — never hold a lock across `.await`; don't reorder lock / await / spawn.
- The dashboard/SIGHUP `unsafe` path (a fix is pending on a branch — land that separately).
- Any public CLI flag, config key, or broker wire shape.

## Refactor rules

- **Findings-first**: report before removing. "Unused" may be public API, feature-gated, or
  CLI-only — check `pub` visibility and `tests/` first.
- **Surgical, test-gated waves**: `just check` green before and after; no drive-by reformatting
  of untouched code.
- **A behavior change (bug fix) is NOT a refactor** — it ships as its own spec+test-gated change
  with a reproducing test, never folded into a refactor wave.

## Review-gate checklist (supervisor)

- [ ] No new `unwrap`/`expect` in non-test code; errors via `PawError` + `?`.
- [ ] Public items documented (`///` / `//!`).
- [ ] Process / tmux / git calls go through the runner seam (or the diff justifies why not).
- [ ] Injection-prone strings use newtypes / are quoted + sanitized.
- [ ] No frozen serde / wire / CLI / config surface changed unless the change's spec says so.
- [ ] Refactor diffs are pure structure moves; any behavior change carries a reproducing test.
- [ ] `just check` + `just deny` green, verified by real exit code (not piped output).

---

This skill encodes **git-paw's own** standards. A consumer project supplies its own
`.agents/skills/code-standards` (or names one in config); git-paw's supervisor consults the
*project's* skill and imposes nothing.

## Context

AGENTS.md: "No `unwrap()` or `expect()` in non-test code — propagate errors with `?`". Nothing
enforces it. A trial clippy run (`-W clippy::unwrap_used -W clippy::expect_used`, with
`allow-unwrap-in-tests` / `allow-expect-in-tests` supplied via `CLIPPY_CONF_DIR`) over
`--all-targets` found:

- **20** findings in `src/` (all `expect`, no `unwrap`): 14 static-regex compiles, 4 code
  invariants, 2 clap guarantees — exactly the audit list.
- **0** additional findings in the library's own `#[cfg(test)]` code (the in-test exemption works
  for unit tests).
- **585** findings across **96** integration-test files under `tests/` — every one in a helper
  function outside a `#[test]` fn (e.g. `tests/helpers/mod.rs`), which clippy's in-test exemption
  does not cover because an integration-test crate carries no `#[cfg(test)]` attribute.

Also relevant: the `git-paw` `[[bin]]` target sets `test = false`, so `src/main.rs` and
`src/commands/*.rs` `#[cfg(test)]` code is neither compiled by `cargo test` nor by
`cargo clippy --all-targets`. Behavioural tests for the two clap sites must therefore be
integration tests.

## Goals / Non-Goals

**Goals:**
- Zero `unwrap`/`expect` in non-test library and binary code, except one audited helper.
- `just lint` fails on any new violation in `src/`.
- No behaviour change.

**Non-Goals:**
- Policing integration-test helpers under `tests/` (test code by intent; a panic there is a test
  failure, which is the desired behaviour).
- Other panic sources (`panic!`, `unreachable!`, indexing, `clippy::panic` /
  `indexing_slicing`) — separate, larger audit.
- Making regex accessors fallible (would ripple `Result` through `is_valid_agent_id`, the role
  guard, the Spec Kit parser and the classifier for a failure that cannot happen at runtime).

## Decisions

### D1 — Enable the lints at the crate roots, not in `[lints.clippy]`

Add `#![warn(clippy::unwrap_used, clippy::expect_used)]` to `src/lib.rs` (next to the existing
`#![warn(missing_docs)]`) and to `src/main.rs`, and create `clippy.toml`:

```toml
allow-unwrap-in-tests = true
allow-expect-in-tests = true
```

`[lints.clippy]` in `Cargo.toml` applies to every target, including the 96 integration-test crates,
where it would produce 585 findings that the in-test exemption cannot reach; silencing them would
mean an `#![allow]` in every test file or rewriting hundreds of test helpers — churn with no safety
value. Crate-root attributes scope the rule to exactly the code the convention names ("non-test
code"), and the lib-test target still benefits from the `clippy.toml` exemption. `warn` (not
`deny`) matches the existing `pedantic = "warn"` posture: `just lint`'s `-D warnings` makes it a hard
failure in CI while leaving local iteration unblocked.

Alternative considered: `[lints.clippy] unwrap_used = "warn"` + `#![allow(clippy::unwrap_used,
clippy::expect_used)]` at the top of each `tests/*.rs` — rejected (96-file churn, and every new
test file must remember the allow).

### D2 — Static regex policy: one helper, `&'static str`, one suppression, tested patterns

New private module `src/static_regex.rs` (module doc comment; declared `mod static_regex;` in
`src/lib.rs`):

```rust
/// Compiles a regex whose pattern is fixed at build time. …
#[allow(clippy::expect_used)] // pattern is a build-time constant; every caller's pattern is
                              // compiled by a unit test, so failure is unreachable at runtime
pub(crate) fn static_regex(pattern: &'static str) -> Regex {
    Regex::new(pattern).expect("static regex pattern compiles")
}
```

Each of the 14 sites keeps its existing `LazyLock` / `OnceLock` caching and replaces
`Regex::new(p).expect(..)` with `static_regex(p)`. The three prompt-profile sites pass
`claude_prompt_profile().<field>.as_str()` — `claude_prompt_profile()` returns
`&'static CliPromptProfile`, so the borrow is `'static`; a config-resolved profile (owned) cannot be
passed, which keeps runtime patterns out of the helper by type.

Each owning module adds a `static_regexes_compile` unit test that calls every regex accessor in that
module (forcing compilation), so a bad pattern fails `cargo test`. The helper module has its own
`compiles_a_static_pattern` test.

Alternatives considered: (a) a per-site `#[allow(clippy::expect_used)]` — 14 scattered suppressions,
each a precedent for the next; (b) fallible accessors — see Non-Goals; (c) `unwrap_or_else` to a
never-matching regex — turns a programming error into silent mis-classification (the role guard
and auto-approve classifier are safety-relevant), strictly worse than a test-time failure.

### D3 — Invariants: make the impossible state unrepresentable

Rather than `ok_or_else` into an error that can never be produced:

- `specs/speckit.rs`: the `ensure_implicit_phase` closure returns the current phase index
  (`usize`) — pushing the implicit phase when none is current — so callers bind `idx` directly and
  the two `current_phase_idx.expect(..)` calls disappear.
- `coordination/inventory.rs` `get_or_refresh`: take the snapshot, keep it when fresh, otherwise
  call `refresh()` (restoring the previous snapshot before returning `Err` on failure, preserving
  the documented "left untouched on failure" contract), and return the reference from
  `self.snapshot.insert(..)`. Drop the now-inaccurate `# Panics` doc section.
- `mcp/query/learnings.rs` `parse`: `if let Some(entries) = map.get_mut(section)` (the section is
  always inserted on its heading line; an absent key simply cannot occur and needs no branch).

### D4 — clap guarantees: error, not panic

`commands/add.rs`: `branch_arg.ok_or_else(|| PawError::BranchError("a branch is required unless
--from-spec is given".into()))?`. `commands/replay.rs`: `branch.ok_or_else(||
PawError::ReplayError("a branch is required unless --list is given".into()))?`. clap still rejects
these invocations first (exit 2, usage message); the `ok_or_else` is defence in depth should the
`required_unless_present` attribute ever drift. The integration tests assert the observable
contract (non-zero exit, no `panicked` on stderr).

## Risks / Trade-offs

- [A future contributor adds an `#[allow(clippy::expect_used)]` elsewhere] → the requirement makes
  the helper the sole exception; Gate 3 / code review checks for new suppressions (grep
  `allow(clippy::expect_used)` / `allow(clippy::unwrap_used)`).
- [`#[cfg(test)]` code in `src/main.rs` / `src/commands/` is not linted at all (bin `test =
  false`)] → pre-existing and out of scope; noted for a follow-up.
- [Upstream clippy changes how the in-test exemption treats integration crates] → crate-root
  scoping (D1) is independent of that behaviour.
- [Textual merge overlap with pending changes editing the same files] → each edit is a one-line
  substitution; resolve at rebase.

## Migration Plan

None — no behaviour, CLI, config, or wire change. Rollback is a plain revert.

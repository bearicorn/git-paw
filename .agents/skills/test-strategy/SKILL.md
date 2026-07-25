---
name: test-strategy
description: Decide the right test type for a git-paw behavior and write it so it survives refactors. Use when adding, reviewing, or consolidating tests — it maps each OpenSpec scenario or behavior to the correct layer (unit table / integration / e2e-PTY / asset-parity), enforces behavioral-only assertions, and lists the anti-patterns (source-grep, per-field batteries, brittle prose pins, fixed sleeps) to avoid.
license: MIT
compatibility: git-paw (Rust, cargo, tmux, OpenSpec)
---

# Test Strategy

Use this when you **add**, **review**, or **consolidate** a git-paw test. It answers two
questions: *what kind of test does this behavior need?* and *how do I write it so a later
refactor can't falsely break it?*

## Prime directive — behavioral only

Every test asserts **observable behavior**: inputs → outputs, public API contracts, error
conditions, and the CLI / wire / file / TOML surfaces. Never assert internal structure —
private fields, source layout, function call counts, or mock interactions. Every OpenSpec
scenario maps to at least one test. (This operationalizes the `AGENTS.md` standard; it does
not replace it.)

If a test would still pass after you *rename an internal function* but fail after you *move
it to another file*, it is testing structure, not behavior — rewrite or delete it.

## Step 1 — Pick the layer

Ask: **what is the smallest layer at which this behavior is observable?** Test there.

| The behavior is… | Layer | Where in git-paw |
|---|---|---|
| A pure / deterministic function — classifier rules, layout math, slugify, region-normalization, flag→value maps | **unit, table-driven** | `#[cfg(test)] mod tests {}` at the bottom of the module |
| A seam / boundary contract — config load+merge, session persist/recover, broker routing, an HTTP round-trip, git/worktree ops | **integration** | `tests/*.rs` with `assert_cmd` / `predicates` / `tempfile`; or the broker `oneshot` layer |
| A whole flow / process-wiring / interactive path — tmux orchestration, prompt gating, `launch → session.json → panes`, `--from-specs` | **e2e** | `assert_cmd` for exit/stdout/state; the **PTY harness** for interactive prompts |
| A shipped asset or prose contract — bundled skills, `sweep.sh`↔Rust classifier parity | **asset / parity** | `*_skill_content.rs`, `sweep_sh_*.rs` — assert stable anchors, never exact substrings |

Prefer the **lowest** layer that still observes the behavior: it is faster and less flaky.
Do **not** push pure logic up into e2e "to be realistic" — that just makes the suite slow
and brittle. The pyramid stays wide at the bottom (fast unit tables) with a thin behavioral
cap (integration + e2e).

## Step 2 — Write it robustly

**Unit (table-driven).** One test, many rows — never one test per variant/field/flag.
```rust
#[test]
fn classify_covers_each_command_class() {
    for (input, expected) in [
        ("cargo test", Safe),
        ("rm -rf /", Danger),
        // one row per behavioral rule — a new rule adds a row, not a test
    ] {
        assert_eq!(classify(input), expected, "input: {input}");
    }
}
```

**Integration.** Exercise the seam at its observable contract in a `tempfile` sandbox; assert
the written file / JSON / exit code, not internal calls. For the broker, prefer the `server.rs`
`oneshot` layer over raw TCP unless you are specifically testing lifecycle/concurrency.

**E2E / PTY.** For interactive prompts, drive the real binary in a detached tmux pane via the
shared PTY harness. Always: `#[serial]`, socket isolation (`helpers::TmuxTestEnv`), a
tmux-availability skip guard, and **poll-until-rendered** (never a fixed sleep as the sync
gate). Assert outcomes (config / `session.json` / panes); use `capture-pane` only to detect a
prompt and to synchronise — never to pixel-match.

**Asset / parity.** Assert **stable anchors** (a required key, a command name, a structural
marker) or keyword-sets — never a full exact substring of a bundled asset, which breaks on
any wording edit. `sweep_sh_*` guards the *shipped bash artifact*, a different artifact from
the Rust classifier — it is a sole guard, not a duplicate.

## Step 3 — Classifying an EXISTING test (consolidation)

When restructuring the suite, route every test to exactly one outcome:

- **delete** — tautologies (`derive` works, a getter round-trips, `assert!(file_I_wrote.exists())`).
- **table-ify** — one-test-per-{variant, field, flag} batteries → a single table test.
- **collapse to integration** — a unit test that only re-checks a contract the integration/HTTP
  layer already guards.
- **replace with e2e** — source-grep / brace-walk introspection tests → a real behavioral e2e
  (the PTY matrix supplies these for the interactive surface).
- **keep as unit** — fast pure-logic tables. The right tool; leave them.
- **protect as sole guard** — the only test covering a spec scenario, a bundled-asset parity
  test, or a prose-only scenario. Rewrite to stable anchors if brittle; **never delete**.

Rule: a removal that drops coverage on a real branch means a sole guard was cut — restore it
(as a table row if needed). Gate consolidation on coverage ≥ baseline and, for risky broker
cuts, a `cargo-mutants` spot-check.

## Anti-patterns — never write these

- **Source-grep / structure assertions** — `include_str!("../src/foo.rs")` + brace-walking, or
  "function X exists in file Y". Zero runtime protection; breaks on any refactor. Test behavior.
- **Per-field / per-variant batteries** — 14 near-identical getter tests. Use a table.
- **Tautologies** — asserting the language/`derive`/a pure fn's determinism.
- **Exact-substring prose pins** on bundled assets. Use stable anchors.
- **Fixed `sleep` as the sync gate** in e2e. Poll until the observable state appears.
- **Private-state / call-count / mock-interaction assertions.** Behavioral only.

## Robustness checklist (before committing a test)

- [ ] Asserts observable behavior, not internal structure.
- [ ] At the lowest layer that observes the behavior.
- [ ] Maps to an OpenSpec scenario (or documents the behavior it pins).
- [ ] Table-driven if it is one-per-{variant/field/flag}.
- [ ] e2e: `#[serial]`, socket-isolated, tmux-skip-guarded, poll-not-sleep.
- [ ] Does not duplicate a higher layer's guard; does not remove a sole guard.
- [ ] Verified by real exit code (`cmd; echo $?`), not piped output.

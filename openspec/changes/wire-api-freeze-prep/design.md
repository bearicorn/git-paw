# Design — wire-api-freeze-prep

## Context

The v1.0.0 freeze will lock the broker JSON wire format and the crate's public
API/semver surface. Some hardening is compatible to add before the freeze but
impossible to add compatibly after it. This change collects exactly those
now-or-never, relax-only items and deliberately excludes anything behavior-
changing or anything in the frozen serde "do-not-touch" set. It is scoped
tightly on purpose: four small, low-risk edits, one of which carries a spec
requirement.

## Decisions

### D1 — Relax-only: `#[serde(default)]` only, never `skip_serializing_if`

The three fields get `#[serde(default)]` and nothing else. `#[serde(default)]`
strictly widens what deserializes (absent array → empty vec) and leaves
serialization output byte-identical. Adding `skip_serializing_if =
"Vec::is_empty"` "for tidiness" was **rejected**: it would omit empty arrays on
the way out, changing the wire bytes and contradicting two frozen scenarios —
"Artifact payload with no exports" (which requires `exports` present as an empty
JSON array) and "Feedback with empty errors list is valid" (which requires
`"errors": []`). Relax-only means input-widening only; output stays as-is.

### D2 — The three fields, and why not five

Of the five required (non-`Option`) `Vec` fields on the broker wire that lack a
default, only three have **no non-empty contract** and get `#[serde(default)]`:

| Struct | Field | messages.rs | Non-empty contract? |
|---|---|---|---|
| `StatusPayload` | `modified_files` | :104 | none → **default** |
| `ArtifactPayload` | `exports` | :138 | none → **default** |
| `ArtifactPayload` | `modified_files` | :140 | none → **default** |
| `IntentPayload` | `files` | :293 | `EmptyIntentFiles` → **stays required** |
| `FeedbackPayload` | `errors` | :306 | `EmptyErrors` → **stays required** |

`StatusPayload.modified_files` is the highest realistic risk (a heartbeat
plausibly omits an empty list). All three defaulted fields accept an empty list
as valid, so defaulting genuinely widens what parses.

`IntentPayload.files` and `FeedbackPayload.errors` are deliberately **left
required** (no default). They carry intentional non-empty contracts
(`EmptyIntentFiles` at `messages.rs:48`, `EmptyErrors` at `:36`), so an absent
field is rejected either way — defaulting them would be pointless. Worse, it
would turn a clear serde "missing field `errors`" parse error into a
defaulted-then-"errors list must not be empty" validation error, and it would
enlarge the frozen-surface change beyond what the stability NFR wants at the
freeze ("minimize surface change"). Keeping them required means omitting the
field stays a clean missing-field parse error, and the non-empty validator still
rejects an explicit `[]`. Both behaviours are unchanged from today.

### D3 — Internal-library-API note over `#[doc(hidden)]`

Two options were on the table to signal "this library surface is not a stable
dependency API": (a) `#[doc(hidden)]` on the ~23 bin/test-only modules, or (b) a
crate-level `//!` doc note in `lib.rs` stating the library API is internal and
not covered by semver. **We pick (b)** — it is the lighter change (one edit, no
per-module churn), records the semver intent unambiguously, and does not risk
hiding an item that a future contributor legitimately wants on docs.rs. (a)
remains available post-note if a stronger signal is ever wanted; it is not
needed for the freeze. This is a documentation/intent task, not a spec
requirement, so it lives in `tasks.md`.

### D4 — MSRV = current stable

`edition = "2024"` already implies rustc ≥ 1.85, but the crate never declares its
floor. AGENTS.md states "MSRV: current stable"; the current stable toolchain is
1.97.0, so `rust-version = "1.97"`. This is behavior-preserving metadata that
lets `cargo`/docs.rs report a clear MSRV and lets the resolver pick MSRV-aware
dependency versions.

### D5 — `#![warn(missing_docs)]` with the one gap closed first

`#![warn(missing_docs)]` on `lib.rs` enforces the AGENTS.md `///` mandate going
forward. Under CI's clippy `-D warnings` a `warn`-level lint is effectively
denied, so this is a real guardrail, not a suggestion. The crate is at 99.9% doc
coverage with a single gap — `agents::inject_section_into_file` — which must get
a one-line `///` first, or the lint fails the build. `warn` (not `deny`) is used
per the audit's recommendation; CI escalates it. Scoped to the `lib.rs` public
API surface (the `main.rs` bin is not a semver surface).

## Non-goals

- No `skip_serializing_if` anywhere (D1) — that is output-changing, not
  relax-only.
- No touching of the frozen serde surfaces: `FileIntent` untagged, `AdvancedMain`
  flatten, `StatusPayload.message` no-skip null emission, `Session.created_at`
  serializer, config `merged_with` default semantics.
- No `#[doc(hidden)]` and no `pub` → `pub(crate)` demotions (the latter can break
  the bin/tests; out of scope for a relax-only change).
- No error-message normalization, dead-code removal, or module splits — those are
  separate v0.13.0 workstreams.

## Risks

- **Low.** All four items are relax-only or metadata/lint. The one wire change
  widens parsing without altering serialization, guarded by both new
  minimal-message tests and the existing byte-equivalence round-trip tests. The
  only way to regress a frozen scenario is to add `skip_serializing_if`, which D1
  explicitly forbids and the spec-audit gate re-checks.

# Rust API Guidelines (git-paw reference)

Condensed from the official [Rust API Guidelines checklist](https://rust-lang.github.io/api-guidelines/checklist.html).
git-paw publishes a **library crate** to crates.io (`src/lib.rs` re-exports modules), so the
future-proofing/stability items bind the *public* surface especially hard going into v1.0.0.
The binary-internal code is looser, but these still raise quality.

## Naming
- `C-CASE` — casing follows RFC 430 (types `CamelCase`, fns/vars `snake_case`, consts `SCREAMING`).
- `C-CONV` — conversions named `as_` (borrow, cheap), `to_` (expensive/owned), `into_` (consume).
- `C-GETTER` — getter is `field()` not `get_field()`; `field_mut()` for the mutable one.
- `C-ITER` — iterators expose `iter`, `iter_mut`, `into_iter`.
- `C-WORD-ORDER` — consistent word order across the API (e.g. `verb_noun` everywhere).

## Interoperability
- `C-COMMON-TRAITS` — eagerly derive/implement `Debug`, `Clone`, `PartialEq`/`Eq`, `Hash`, `Default`, `Copy` where it fits.
- `C-CONV-TRAITS` — conversions use `From`/`TryFrom`/`AsRef`/`AsMut`, not ad-hoc methods.
- `C-SERDE` — data structures on the wire/config implement `Serialize`/`Deserialize` (git-paw: config + broker messages).
- `C-GOOD-ERR` — **error types are meaningful and well-behaved**: implement `std::error::Error` + `Display` + `Debug`, are `Send + Sync + 'static`. git-paw: `PawError` via `thiserror`.
- `C-SEND-SYNC` — types are `Send`/`Sync` where feasible (matters for the async broker).

## Documentation
- `C-CRATE-DOC` / `C-EXAMPLE` — crate-level docs + a rustdoc example on public items.
- `C-QUESTION-MARK` — examples use `?`, never `unwrap`/`try!`.
- `C-FAILURE` — document a fn's Errors, Panics, and Safety.
- `C-METADATA` — `Cargo.toml` carries all common metadata (also flagged by the oss-maintainer audit: pin MSRV).
- `C-HIDDEN` — `#[doc(hidden)]` on internal items that are `pub` only for the binary/tests (directly addresses the "807 bare-`pub` items get semver-locked" finding).

## Predictability
- `C-CTOR` — constructors are static inherent methods (`Foo::new`, `Foo::with_…`).
- `C-NO-OUT` — return values, not out-parameters.
- `C-METHOD` — a fn with a clear receiver is a method on that type.

## Type safety (ties to our decoupling patterns)
- `C-NEWTYPE` — newtypes for static distinctions (our `SessionName`/`BranchSlug`/`WorktreePath`).
- `C-CUSTOM-TYPE` — arguments convey meaning through types, not bare `bool`/`Option` (prefer an enum over a positional `bool`).
- `C-BUILDER` — builders for complex construction.

## Dependability
- `C-VALIDATE` — **functions validate their arguments** (reject bad input at the boundary — pairs with the newtype smart-constructors).
- `C-DTOR-FAIL` — destructors never fail/panic.

## Debuggability
- `C-DEBUG` — all public types implement `Debug`; `C-DEBUG-NONEMPTY` — Debug output is not empty.

## Future-proofing (v1.0.0-critical for the lib surface)
- `C-SEALED` — seal traits you don't want downstream to implement.
- `C-STRUCT-PRIVATE` — public structs have private fields (expose via methods) so you can evolve them.
- `C-NEWTYPE-HIDE` — newtypes encapsulate implementation details.
- `C-STABLE` — **public dependencies of a stable crate must themselves be stable** (audit re-exported types).

## Necessities
- `C-PERMISSIVE` — crate + deps carry permissive licenses (git-paw: MIT; enforced by `cargo deny`; note the homegrown `src/dirs.rs` replacing the non-FOSS `dirs` crate).

## What binds git-paw most at the v1.0.0 freeze
`C-GOOD-ERR`, `C-HIDDEN` (+ MSRV per `C-METADATA`), `C-STRUCT-PRIVATE`, `C-SEALED`, `C-STABLE`,
`C-NEWTYPE`/`C-VALIDATE` (the injection-hardening seam), and `C-QUESTION-MARK`/`C-FAILURE`
(no `unwrap`/`expect`, documented failure modes).

# conflict-detector-fn-granularity Specification

## Purpose
TBD - created by archiving change conflict-detector-fn-granularity. Update Purpose after archive.
## Requirements
### Requirement: Optional regions field on agent.intent files

The `agent.intent` broker message variant SHALL accept each
`files` entry in one of two shapes: a plain string (the
v0.5.0 file-level form) OR an object `{ path: string,
regions?: Region[] }`. Both shapes SHALL be accepted within
the same `files` array (mixed entries permitted). Omitting
`regions` from an object entry SHALL be equivalent to using
the plain string form.

#### Scenario: String entry parses as file-level intent

- **WHEN** an intent message with `"files": ["src/main.rs"]`
  is published
- **THEN** the broker SHALL accept it and the file SHALL be
  treated as file-level (no regions declared)

#### Scenario: Object entry with regions parses correctly

- **WHEN** an intent message with
  `"files": [{ "path": "src/auth.rs",
    "regions": [{ "kind": "function", "name":
    "validate_token" }] }]` is published
- **THEN** the broker SHALL accept it and the file SHALL
  carry the declared regions

#### Scenario: Mixed string and object entries accepted

- **WHEN** an intent's `files` array contains both string and
  object entries
- **THEN** the broker SHALL accept the mixed shape, treating
  each entry per its own shape

### Requirement: Four region kinds

The system SHALL accept exactly four region kinds in v0.6.0:
`function { name }`, `class { name }`, `block { anchor }`,
and `range { start_line, end_line }`. Unknown `kind` values
SHALL cause the broker to reject the publish with a 400-
class error.

#### Scenario: Known kinds round-trip cleanly

- **WHEN** an intent is published with one region of each
  documented kind
- **THEN** the broker SHALL accept and route the message,
  preserving each region's structure

#### Scenario: Unknown kind is rejected loudly

- **WHEN** an intent publishes a region with `kind:
  "macro"` (not in the v0.6.0 set)
- **THEN** the broker SHALL reject the publish with a
  message identifying the offending region

### Requirement: Region-aware forward-conflict detection

The forward-conflict detector SHALL evaluate per-file
overlap as follows:

- When both intents declare regions for a shared file, the
  detector SHALL trigger only when at least one pair of
  regions intersects.
- When at least one intent omits regions for a shared file,
  the detector SHALL fall back to file-level conflict
  (v0.5.0 behaviour).
- Region intersection rules:
  - Same kind + matching `name` (for function/class/block)
    → intersect.
  - Two `range` regions with overlapping
    `[start_line, end_line]` intervals → intersect.
  - Cross-kind comparisons (named vs range) SHALL be
    treated as intersecting conservatively (we cannot
    resolve names to lines without source parsing).

#### Scenario: Non-overlapping functions in the same file do not conflict

- **GIVEN** intents A and B both naming `src/auth.rs`, with
  A declaring `function validate_token` and B declaring
  `function refresh_session`
- **WHEN** the forward-conflict detector runs
- **THEN** the detector SHALL NOT trigger a conflict

#### Scenario: Overlapping functions in the same file conflict

- **GIVEN** intents A and B both declaring
  `function validate_token` on `src/auth.rs`
- **WHEN** the detector runs
- **THEN** the detector SHALL trigger a forward-conflict
  warning identifying both branches and the intersecting
  function

#### Scenario: File-level fallback when regions omitted

- **GIVEN** intent A naming `src/auth.rs` with regions
  declared, and intent B naming `src/auth.rs` as a plain
  string (no regions)
- **WHEN** the detector runs
- **THEN** the detector SHALL trigger a file-level conflict
  (preserving v0.5.0 safety)

#### Scenario: Cross-kind comparison intersects conservatively

- **GIVEN** intent A declaring
  `function validate_token` on `src/auth.rs` and intent B
  declaring `range { start_line: 10, end_line: 50 }` on
  the same file
- **WHEN** the detector runs
- **THEN** the detector SHALL trigger a conflict and SHALL
  include a hint that the cross-kind comparison was
  conservative

#### Scenario: Overlapping ranges intersect

- **GIVEN** intent A declaring
  `range { 10, 30 }` and intent B declaring
  `range { 25, 45 }` on the same file
- **WHEN** the detector runs
- **THEN** the detector SHALL trigger a conflict naming the
  overlapping range

#### Scenario: Non-overlapping ranges do not intersect

- **GIVEN** intent A declaring
  `range { 10, 20 }` and intent B declaring
  `range { 30, 40 }` on the same file
- **WHEN** the detector runs
- **THEN** the detector SHALL NOT trigger a conflict on
  that file

### Requirement: Detector warning identifies intersecting regions

The detector SHALL name the intersecting regions explicitly
in any warning it produces (supervisor pane prose,
`agent.feedback` message, or learnings record) so consumers
can act on them. This applies whenever a region-level
conflict triggers.

#### Scenario: Warning enumerates the intersecting regions

- **GIVEN** a region-level conflict on two functions in
  `src/auth.rs`
- **WHEN** the warning is produced
- **THEN** the warning text SHALL list each intersecting
  region with its kind and name (or range)

### Requirement: Coordination skill teaches region declaration

The bundled `assets/agent-skills/coordination.md` SHALL
include guidance on when to declare regions, when to omit
them, and explicit language forbidding manufactured-narrow
regions to dodge conflict warnings.

#### Scenario: Skill prose covers when to declare and when to omit

- **WHEN** the forward-coordination section of
  coordination.md is read
- **THEN** the new region-declaration prose SHALL include
  both "declare when..." and "skip when..." guidance with
  at least two examples per direction

#### Scenario: Skill prose forbids dodging the detector

- **WHEN** the region-declaration prose is read
- **THEN** the prose SHALL contain explicit language
  warning against manufacturing narrow regions to avoid the
  forward-conflict warning, with a one-sentence rationale

### Requirement: Backwards compatibility with v0.5.0 publishers

The system SHALL treat v0.5.0 publishers (intents whose
`files` array contains only plain strings) byte-equivalently
to v0.5.0. The detector SHALL produce the same conflict
warnings v0.5.0 would for the same string-only inputs.

#### Scenario: v0.5.0 publisher round-trip matches v0.5.0

- **GIVEN** an intent published with `"files":
  ["src/foo.rs", "src/bar.rs"]` (v0.5.0 shape)
- **WHEN** the broker stores and emits the message AND the
  detector evaluates it
- **THEN** the routed message and the detector's behaviour
  SHALL match v0.5.0 byte-for-byte for the same inputs


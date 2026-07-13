## MODIFIED Requirements

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
    → intersect. Name matching SHALL compare NORMALIZED
    names: case-folded, trimmed, with separator characters
    (space, underscore, hyphen) collapsed to a single form,
    a trailing `()` stripped, and a leading declaration
    keyword (`fn`, `def`, `function`, `class`) stripped —
    so spelling variants of the same symbol intersect.
  - Named-vs-named comparisons across DIFFERENT kinds
    (function vs class vs block) with matching normalized
    names SHALL be treated as intersecting conservatively,
    with the same conservative-comparison hint as the
    named-vs-range rule.
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

#### Scenario: Spelling variants of the same symbol intersect

- **GIVEN** intent A declaring `function validate_token` and
  intent B declaring `function Validate Token()` on the same
  file
- **WHEN** the detector runs
- **THEN** normalization SHALL equate the two names and the
  detector SHALL trigger a forward-conflict warning

#### Scenario: Named regions of different kinds with the same name intersect conservatively

- **GIVEN** intent A declaring `function DEV_ALLOWLIST_PRESET`
  and intent B declaring `block DEV_ALLOWLIST_PRESET` on the
  same file
- **WHEN** the detector runs
- **THEN** the detector SHALL trigger a conflict and SHALL
  include a hint that the comparison was conservative

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

### Requirement: Coordination skill teaches region declaration

The bundled `assets/agent-skills/coordination.md` SHALL
include guidance on when to declare regions, when to omit
them, and explicit language forbidding manufactured-narrow
regions to dodge conflict warnings.

The region-declaration prose SHALL additionally instruct
agents to: declare region names using the CANONICAL symbol
spelling exactly as it appears in source; declare ALL
regions the work touches, including shared constant blocks,
import sections, and asset files (not only the headline
function); and RE-PUBLISH `agent.intent` when the work's
scope grows beyond the declared regions mid-task.

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

#### Scenario: Skill prose requires canonical names, full coverage, and re-publication

- **WHEN** the region-declaration prose is read
- **THEN** it SHALL instruct canonical source spelling for
  region names, declaring every touched region including
  shared blocks, and re-publishing `agent.intent` when scope
  grows

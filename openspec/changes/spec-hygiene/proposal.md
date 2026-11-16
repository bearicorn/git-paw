## Why

The v0.19.0 release reviews left two kinds of spec debt. They are spec-only today, but they make
the specs harder to trust as documentation:

- **Scenarios with no `Test:` pointer.** `worker-usage-budget` and `worker-friction-fixes` added
  scenarios whose tests exist but are not named in the spec. 33 such scenarios remain on main
  (others were given pointers later). One of them, "add on a genuinely stopped session is still
  refused", has no test at all.
- **Scenario titles and placement that strict validation will not let a MODIFIED block fix.**
  `openspec validate --strict` rejects a MODIFIED block that drops or renames a scenario. It also
  rejects a REMOVED and ADDED pair that share a requirement name. So `v019-release-review-fixes`
  and `architecture-conformance-fixes` left these as they were:
  - five `core-project-conventions` scenarios titled "README Features section …", when the section
    is `## Highlights` and one of them inspects the Commands section;
  - `core-configuration`'s "lists all three specs.type values", on a requirement that now names
    four;
  - two `code-architecture` scenarios ("An if-let on a spec-format variant is caught", "A match on a
    wrapped spec-format variant is caught") that test the spec-format dispatch rule owned by
    `spec-providers`, not how audits read source;
  - `git-operations`' "Creation-time rebase contract is unaffected", phrased relative to the change
    that introduced it.

## What Changes

- **`Test:` pointers** (MODIFIED, requirement text unchanged) on every scenario
  `worker-usage-budget` or `worker-friction-fixes` added that lacks one:
  - in `supervisor-unattended-operation`, `cli-provider`, `supervisor-config` and `skill-agent`
    (budget);
  - in `broker-agent-helper`, `skill-agent` (coordination), `boot-block` and
    `git-add-remove-branch` (friction fixes).

  The one scenario with no test gets a named test to write.
- **Renames and moves as REMOVED and ADDED.** Each affected requirement is removed and re-added
  under a new name, with the same body and retitled scenarios:
  - `core-project-conventions`: "README Highlights section links each capability family";
  - `core-configuration`: "Configuration reference documents every specs type value", whose two
    scenarios merge into one, because one test already covers both;
  - `code-architecture`: "Source audits check every production line", without the two spec-format
    scenarios;
  - `git-operations`: "Mid-session rebase runs in the worktree holding the branch".
- **The two spec-format scenarios move** into `spec-providers` "Per-format behaviour is reached
  only through the backend" (MODIFIED, adding them). `tests/source_audit.rs:447` already cites
  that requirement for the test.

No behaviour changes. One new test is written; everything else is spec text and code comments that
name the renamed requirements.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `supervisor-unattended-operation`: MODIFIED six requirements (`Test:` pointers only).
- `cli-provider`: MODIFIED three requirements (`Test:` pointers only).
- `supervisor-config`: MODIFIED "Budget caps in `[supervisor]`" (`Test:` pointers only).
- `skill-agent`: MODIFIED "Embedded coordination skill" and the two budget-step requirements
  (`Test:` pointers only).
- `broker-agent-helper`: MODIFIED "Helper publish subcommands" (`Test:` pointers only).
- `boot-block`: MODIFIED "Boot block asks for bare helper calls" (`Test:` pointers only).
- `git-add-remove-branch`: MODIFIED "add refills a live session that has no agents" (`Test:`
  pointers, one naming a new test).
- `core-project-conventions`: REMOVED and ADDED the README Highlights requirement (renamed and
  retitled).
- `core-configuration`: REMOVED and ADDED the specs-type reference requirement (renamed, with the
  two scenarios merged).
- `code-architecture`: REMOVED and ADDED the source-audit requirement (renamed, two scenarios
  moved out).
- `spec-providers`: MODIFIED "Per-format behaviour is reached only through the backend" (gains the
  two spec-format audit scenarios).
- `git-operations`: REMOVED and ADDED the mid-session rebase requirement (renamed, one scenario
  retitled).

## Impact

- **Specs:** the twelve capabilities above. Requirement and scenario counts:
  - `code-architecture` loses 2 scenarios and `spec-providers` gains them;
  - `core-configuration` goes from 2 scenarios to 1, the merge;
  - every other count is unchanged.
- **Tests:** one new test, `tests/emptied_session_refill_e2e.rs::add_on_a_stopped_session_is_refused_naming_start`.
  Comments that cite a renamed requirement are updated in `tests/readme_features.rs:71-130`,
  `tests/configuration_reference_docs.rs:167`, `tests/helpers/source_scan.rs:2` and
  `tests/source_audit.rs:497`. No assertion changes.
- **Code:** `src/git.rs:431` doc comment (prose; optional). No behaviour change.
- **Docs:** none. `docs/src/specifications/README.md` `{{#include}}`s each main spec, so it
  follows the specs on archive.
- **Cross-change:** `dx-error-cleanups` (this release) cites "Source audits read every
  production line" in its design. If it archives after this change, that citation must use the
  new name, "Source audits check every production line". No other open change modifies a
  requirement touched here.
- **Security / safety:** none.

## Why

`cargo deny check` fails on the v0.17.0 integration branch, so the release cannot ship:

```
error[unmaintained]: unsafe-libyaml is unmaintained
    ├ ID: RUSTSEC-2024-0449
```

The chain is `unsafe-libyaml v0.2.11` ← `serde_yaml v0.9.34+deprecated` ← git-paw. Both
crates are dead upstream: `unsafe-libyaml` was archived read-only on 2024-03-25, and
`serde_yaml` declares its own deprecation in its version string. No fix is coming, so an
ignore in `deny.toml` would be permanent.

`serde_yaml` has exactly **one** call site: `src/skills.rs` `parse_standardized_metadata`,
which deserialises a skill's frontmatter into `StandardizedSkillMetadata` — two required
string fields (`name`, `description`), two optional strings (`license`, `compatibility`),
and an optional `metadata` map. A YAML engine is far more than that needs.

This repository has already set the precedent: `dirs` was replaced by a homegrown
`src/dirs.rs` for a licensing reason, and AGENTS.md records it under "Notable exclusions —
do not re-add". The same treatment fits here, for a supply-chain reason.

## What Changes

- Replace `serde_yaml::from_str` with a small, dependency-free frontmatter parser scoped to
  the agentskills.io field set, and remove `serde_yaml` from `Cargo.toml`.
- The parser's supported subset SHALL be explicit: top-level `key: value` pairs with plain,
  single-quoted and double-quoted scalars; literal (`|`) and folded (`>`) block scalars;
  comments; and the one-level `metadata` map of string keys to string values. That covers
  every frontmatter block in the tree today — including keys git-paw does not consume,
  such as a nested `allowed-tools:` list, which SHALL be skipped together with their
  indented content.
- **No silent misparse.** A construct outside the subset in a field git-paw consumes SHALL
  be a validation error naming the field, never a best-effort value. This matters because
  the parser also reads **user-authored** override skills (`Source::User`), not only the
  embedded ones.
- AGENTS.md SHALL move `serde_yaml` from the approved-dependencies table to "Notable
  exclusions", with the reason and a do-not-re-add note, mirroring `dirs`.
- Incidental correction: AGENTS.md described `serde_yaml` as "Spec Kit frontmatter
  parsing"; its only use was skill frontmatter.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `core-project-conventions`: *AGENTS.md dependency table matches `Cargo.toml`* and
  *AGENTS.md dependency table SHALL list the dependencies and note `dirs` as intentionally
  absent* both currently **require** a `serde_yaml` row; they are amended to record it as an
  exclusion instead.

### Added Capabilities

- `skill-standardized`: a requirement defining the dependency-free frontmatter parser, its
  supported subset, and its no-silent-misparse rule.

## Impact

- `src/skills.rs` — `parse_standardized_metadata`; a new private parser (or a small
  sibling module) under the existing `SkillError::ValidationError` error path.
- `StandardizedSkillMetadata` keeps its public shape — `metadata` stays
  `Option<serde_json::Value>`, populated as a JSON object of strings — so no API churn.
- `Cargo.toml`, `Cargo.lock` — `serde_yaml` and `unsafe-libyaml` disappear. Confirm with
  `cargo tree -i unsafe-libyaml` (must report nothing) and `cargo deny check` (exit 0).
- `AGENTS.md` — dependency table and Notable exclusions.
- **No user-visible behaviour change** for any skill that parsed before, verified by a
  corpus test over every `SKILL.md` and embedded skill in the tree.
- **Release blocker**: this change is what makes `just deny` pass for v0.17.0.
- **Delta coordination**: neither `core-project-conventions` nor `skill-standardized` is
  touched by any other in-flight v0.17.0 change.

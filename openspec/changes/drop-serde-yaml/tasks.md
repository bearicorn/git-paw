## 1. Freeze parity fixtures FIRST (while serde_yaml is still present)

- [ ] 1.1 Collect every frontmatter input the parser meets: the three embedded skills in `assets/agent-skills/` and every `SKILL.md` under the repository's skill directories
- [ ] 1.2 Using the CURRENT `serde_yaml`-based parser, record each input's parsed `name`, `description`, `license`, `compatibility` and `metadata` as expected-value fixtures under `tests/`
- [ ] 1.3 Add hand-written fixtures for the subset's edge cases, also captured from the current parser: double-quoted scalar containing `: `, single-quoted scalar with `''`, folded and literal block scalars with each chomping indicator, trailing comments, trailing whitespace, an unconsumed key with a nested block sequence
- [ ] 1.4 Commit the fixtures before touching the parser — they are the only record of "before"

## 2. Dependency-free parser

- [ ] 2.1 Implement the parser for the subset defined in `specs/skill-standardized/spec.md`, returning the existing `SkillError::ValidationError` on failure
- [ ] 2.2 Skip unconsumed top-level keys together with their indented content
- [ ] 2.3 Reject, naming the field, any unsupported construct in a consumed field: flow collections, anchors and aliases, nested mappings inside `metadata`, tags
- [ ] 2.4 Populate `metadata` as a JSON object whose values are all strings; keep `StandardizedSkillMetadata`'s public shape
- [ ] 2.5 Keep the existing missing-`name` / missing-`description` checks and messages unchanged
- [ ] 2.6 No `unwrap()` / `expect()` outside tests

## 3. Remove the dependency

- [ ] 3.1 Replace the `serde_yaml::from_str` call and remove `serde_yaml` from `Cargo.toml`; refresh `Cargo.lock`
- [ ] 3.2 `cargo tree -i unsafe-libyaml` and `cargo tree -i serde_yaml` SHALL both report nothing
- [ ] 3.3 `cargo deny check > /tmp/deny.log 2>&1; echo $?` SHALL print 0 — check the real exit code, not piped output

## 4. Tests

- [ ] 4.1 The corpus parity test: every fixture from 1.2 and 1.3 parses to exactly its frozen expected values
- [ ] 4.2 Rejection tests for each unsupported construct in 2.3, each asserting the error names the field
- [ ] 4.3 Verify every scenario in `specs/skill-standardized/spec.md` and `specs/core-project-conventions/spec.md` maps to at least one test

## 5. Docs

- [ ] 5.1 In AGENTS.md, remove the `serde_yaml` row from the approved-dependencies table and add a "Notable exclusions" entry beside `dirs` stating the reason (deprecated upstream; unmaintained `unsafe-libyaml`, RUSTSEC-2024-0449) and that it must not be re-added
- [ ] 5.2 Document the supported frontmatter subset wherever the user guide describes custom or override skills, so users writing override skills know the bounds
- [ ] 5.3 Note in the changelog-facing commit body that override skills using YAML outside the subset now fail to load with a named-field error

## 6. Gates

- [ ] 6.1 `cargo fmt`, then `just lint`
- [ ] 6.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code
- [ ] 6.3 `just deny` passes
- [ ] 6.4 `mdbook build docs/`
- [ ] 6.5 `openspec validate drop-serde-yaml --strict`

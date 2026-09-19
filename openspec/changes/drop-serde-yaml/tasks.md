## 1. Freeze parity fixtures FIRST (while serde_yaml is still present)

- [x] 1.1 Collect every frontmatter input the parser meets: the three embedded skills in `assets/agent-skills/` and every `SKILL.md` under the repository's skill directories
- [x] 1.2 Using the CURRENT `serde_yaml`-based parser, record each input's parsed `name`, `description`, `license`, `compatibility` and `metadata` as expected-value fixtures under `tests/`
- [x] 1.3 Add hand-written fixtures for the subset's edge cases, also captured from the current parser: double-quoted scalar containing `: `, single-quoted scalar with `''`, folded and literal block scalars with each chomping indicator, trailing comments, trailing whitespace, an unconsumed key with a nested block sequence
- [x] 1.4 Commit the fixtures before touching the parser — they are the only record of "before"

## 2. Dependency-free parser

- [x] 2.1 Implement the parser for the subset defined in `specs/skill-standardized/spec.md`, returning the existing `SkillError::ValidationError` on failure
- [x] 2.2 Skip unconsumed top-level keys together with their indented content
- [x] 2.3 Reject, naming the field, any unsupported construct in a consumed field: flow collections, anchors and aliases, nested mappings inside `metadata`, tags
- [x] 2.4 Populate `metadata` as a JSON object whose values are all strings; keep `StandardizedSkillMetadata`'s public shape
- [x] 2.5 Keep the existing missing-`name` / missing-`description` checks and messages unchanged
- [x] 2.6 No `unwrap()` / `expect()` outside tests

## 3. Remove the dependency

- [x] 3.1 Replace the `serde_yaml::from_str` call and remove `serde_yaml` from `Cargo.toml`; refresh `Cargo.lock`
- [x] 3.2 `cargo tree -i unsafe-libyaml` and `cargo tree -i serde_yaml` SHALL both report nothing
- [x] 3.3 `cargo deny check > /tmp/deny.log 2>&1; echo $?` SHALL print 0 — check the real exit code, not piped output

## 4. Tests

- [x] 4.1 The corpus parity test: every fixture from 1.2 and 1.3 parses to exactly its frozen expected values
- [x] 4.2 Rejection tests for each unsupported construct in 2.3, each asserting the error names the field
- [x] 4.3 Verify every scenario in `specs/skill-standardized/spec.md` and `specs/core-project-conventions/spec.md` maps to at least one test

## 5. Docs

- [x] 5.1 In AGENTS.md, remove the `serde_yaml` row from the approved-dependencies table and add a "Notable exclusions" entry beside `dirs` stating the reason (deprecated upstream; unmaintained `unsafe-libyaml`, RUSTSEC-2024-0449) and that it must not be re-added
- [x] 5.2 Document the supported frontmatter subset wherever the user guide describes custom or override skills, so users writing override skills know the bounds
- [x] 5.3 Note in the changelog-facing commit body that override skills using YAML outside the subset now fail to load with a named-field error

## 6. Gates

- [x] 6.1 `cargo fmt`, then `just lint`
- [x] 6.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code (pre-existing `sweep_sh_stuck_detection` failures unrelated to this change — see note below)
- [x] 6.3 `just deny` passes
- [x] 6.4 `mdbook build docs/`
- [x] 6.5 `openspec validate drop-serde-yaml --strict`

**Note on 6.2:** the full suite has 3 pre-existing failures in
`tests/sweep_sh_stuck_detection.rs` (`repeated_detection_is_deduped`,
`permission_prompt_with_stale_heartbeat_publishes_stuck`,
`permission_marker_is_stuck_on_prompt_not_no_progress`) caused by a
case-sensitivity bug in `sweep.sh`'s stuck-prompt matching, unrelated to this
change and already fixed on the `live-prompt-tail-window` branch. Every other
test in the suite (library unit tests, all other integration files,
doctests) passes.

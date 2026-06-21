## 1. Bundled helper: `sweep.sh learn` subcommand

- [ ] 1.1 Add a `learn <category> <title> <body-json>` subcommand to `assets/scripts/sweep.sh` that builds an `agent.learning` payload (`agent_id="supervisor"`, passing `category`/`title`/`body` through) and POSTs it via the existing `publish()` helper + broker-URL discovery
- [ ] 1.2 Add `learn` to the subcommand list in the script's usage/help block and the header comment (alongside `status-publish`/`verified`/`feedback-gate`)
- [ ] 1.3 Verify `.git-paw/scripts/broker.sh`/`sweep.sh` are NOT both needed — `learn` stays on `sweep.sh` (supervisor-role helper); confirm no raw curl is introduced

## 2. Allowlist coverage (no broad curl grant)

- [ ] 2.1 In `src/supervisor/curl_allowlist.rs`, confirm the existing by-path grant for `.git-paw/scripts/sweep.sh` already covers `sweep.sh learn …`; if grants are by exact argv, generalise to a path prefix
- [ ] 2.2 Add a unit test asserting `sweep.sh learn` is permitted by the seeded allowlist AND that no `curl *` grant is added by this change

## 3. Renderer: "Tooling friction" section

- [ ] 3.1 In `src/broker/learnings.rs`, add a `tooling_friction` category constant and render its records under a `### Tooling friction` section (not the "Other learnings" fallback)
- [ ] 3.2 Apply the existing tolerant-rendering path (title + JSON body dump) when a `tooling_friction` body lacks the documented `friction` field
- [ ] 3.3 Unit test: a `tooling_friction` record renders under "Tooling friction" and NOT under "Other learnings"
- [ ] 3.4 Unit test: a malformed `tooling_friction` body renders as title + JSON under the section
- [ ] 3.5 Regression test: the four existing qualitative sections + the v0.5.0 deterministic sections render byte-for-byte unchanged; a genuinely unknown category still falls through to "Other learnings"

## 4. Supervisor skill: category + heuristic + helper routing

- [ ] 4.1 In `assets/agent-skills/supervisor.md`, document the `tooling_friction` category and its body shape (`friction`, `occurrences`, `suggestion`) alongside the existing four
- [ ] 4.2 Add the `tooling_friction` publish heuristic with an explicit "do not publish unless absorbed ≥2× this session" gate; name `friction` as the primary dedup identifier
- [ ] 4.3 Replace the raw-curl `agent.learning` publish example with `sweep.sh learn <category> <title> <body-json>`; state that the skill MUST NOT hand-roll raw curl for `agent.learning`

## 5. Supervisor skill: operational capture wiring

- [ ] 5.1 Add an opportunistic capture step to the continuous monitoring-loop / sweep section (§2), ordered AFTER approval clearing and stuck detection (terminal, non-blocking), directing the LLM to publish via `sweep.sh learn` when a category gate is met during the sweep
- [ ] 5.2 Add a session-end synthesis pass to the wind-down / final-summary section: reflect over the run and publish durable qualitative learnings via `sweep.sh learn`, deduped against in-session records by each category's primary identifier

## 6. Skill-content tests

- [ ] 6.1 Skill-content test (`src/skills.rs`): the embedded supervisor skill contains the `tooling_friction` category, its body fields, and the ≥2-occurrence gate
- [ ] 6.2 Skill-content test: the embedded supervisor skill references `sweep.sh learn` and contains NO raw `curl …/publish` example for `agent.learning`
- [ ] 6.3 Skill-content test: the continuous-sweep section includes the capture step (after approve/detect-stuck) AND the wind-down section includes the synthesis pass with dedup-by-primary-identifier wording

## 7. E2E + docs + quality gates

- [ ] 7.1 E2E test: publish `agent.learning category=tooling_friction` to a test broker → the aggregator's flushed `session-learnings.md` contains the record under "Tooling friction"
- [ ] 7.2 Update docs: configuration/learnings reference (and the supervisor user-guide chapter) to mention the `tooling_friction` category and `sweep.sh learn`; `mdbook build docs/` succeeds
- [ ] 7.3 `just check` (fmt + clippy + tests) and `just deny` pass; no `unwrap()`/`expect()` in non-test code; all new public items have doc comments

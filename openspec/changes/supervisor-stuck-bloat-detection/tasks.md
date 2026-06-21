## 1. Config fields

- [ ] 1.1 Add three optional fields to `SupervisorConfig` in `src/config.rs`: no-progress window seconds, context-bloat token threshold (thousands), and blocked-on-supervisor window seconds, each `#[serde(default, skip_serializing_if = "Option::is_none")]`
- [ ] 1.2 Add round-trip + default + pre-existing-config-loads unit tests for the new fields (assert absent → None, populated → match, None omits on serialize)
- [ ] 1.3 Extend the `git paw init` commented `[supervisor]` block to list the three new keys with example values

## 2. sweep.sh detection core

- [ ] 2.1 Add named marker regexes next to `STUCK_MARKERS_REGEX`: a stream-timeout/transport-error pattern and a `/clear to save <N>k tokens` pattern
- [ ] 2.2 Add threshold/window discovery in `sweep.sh` reading the new `[supervisor]` fields (defaults: no-progress ~1500s, context-bloat 250k, blocked-on-supervisor ~900s) via the existing TOML-discovery helper
- [ ] 2.3 Reorder `stuck_eval` classification so pane-marker shapes (stuck-on-prompt, stuck-stream-timeout, context-bloat) are evaluated BEFORE the no-progress heuristic; stuck-on-prompt path unchanged
- [ ] 2.4 Add the stuck-stream-timeout branch: marker present → publish `agent.status` `phase: "stuck-stream-timeout"` with `detail.captured_prompt`
- [ ] 2.5 Add the context-bloat branch: parse `N` from the clear hint, compare to threshold → publish `phase: "context-bloat"` with `detail` token figure; below threshold does not flag
- [ ] 2.6 Add the no-progress branch: snapshot `(checkbox_count, commit_count, timestamp)` per agent to `.git-paw/.sweep-progress`, compare to prior; both unchanged past window → `phase: "no-progress"`; first observation only records; movement in either counter clears
- [ ] 2.7 Add the blocked-on-supervisor branch: read the agent's `agent.blocked` stream, detect a supervisor-targeted block unanswered past the window → `phase: "blocked-on-supervisor"`
- [ ] 2.8 Extend dedup so each new shape keys on `(agent_id, shape)` in the dedup file (one publish per window per shape)
- [ ] 2.9 Keep `stuck-eval` fixture-drivable: new branches reachable via stdin capture + args so they are testable without tmux

## 3. Skill prose

- [ ] 3.1 Extend supervisor.md "Detecting stuck agents" to document all five shapes and the read-pane-before-classifying rule, retaining the inline-bash-reinvention prohibition
- [ ] 3.2 Extend supervisor.md "Stream-timeout recovery" error-shape subsection to cover a CODING AGENT's stream timeout (detected via sweep.sh, surfaced as `stuck-stream-timeout`) distinct from the supervisor's own
- [ ] 3.3 Add the "N re-verify cycles is not a stall" rule to supervisor.md (cite mcp-server 7 / dev-allowlist 6; judge stall by detected shapes, not cycle count)
- [ ] 3.4 Extend coordination.md "Context budget" to note proactive context-bloat flagging past the threshold and tie it to commit-before-compact
- [ ] 3.5 Re-run the no-language-leak audit against the updated supervisor.md and coordination.md

## 4. Tests

- [ ] 4.1 Test: stream-timeout marker in a pane is detected and publishes `phase: "stuck-stream-timeout"`
- [ ] 4.2 Test: context-bloat past threshold publishes `phase: "context-bloat"`; below threshold does not
- [ ] 4.3 Test: no-progress heartbeat (checkbox + commit both unchanged over the window, no marker) publishes `phase: "no-progress"`; movement in either counter does not; first observation does not
- [ ] 4.4 Test: a pane showing a permission marker is classified stuck-on-prompt, NOT no-progress, even with unchanged counters (read-pane rule)
- [ ] 4.5 Test: a supervisor-targeted `agent.blocked` unanswered past the window publishes `phase: "blocked-on-supervisor"`; a fresh one does not
- [ ] 4.6 Test: dedup — each shape publishes once per window per `(agent_id, shape)` across repeated sweeps
- [ ] 4.7 Test: skill prose assertions — five shapes + read-pane rule in supervisor.md, coding-agent stream-timeout case, "N re-verify cycles not a stall" rule, coordination.md proactive-bloat note

## 5. Quality gates

- [ ] 5.1 `just check` (fmt + clippy + tests) passes
- [ ] 5.2 `just deny` passes; no new dependencies added
- [ ] 5.3 `mdbook build docs/` succeeds; update user-guide/config-reference if the new `[supervisor]` keys are user-facing
- [ ] 5.4 `openspec validate supervisor-stuck-bloat-detection --strict` passes

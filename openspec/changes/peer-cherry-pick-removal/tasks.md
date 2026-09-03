## 1. Inter-agent rules (`src/agents.rs`)

- [ ] 1.1 Replace the *Cherry-pick peer artifacts* bullet in `build_inter_agent_rules` (`:172-175`) with a peer-dependency bullet: publish `agent.blocked` naming the peer and what is needed, then continue on unblocked work or wait; state that the agent MUST NOT cherry-pick a peer's commit into its own branch, and why (duplicates a commit the supervisor will later merge from the peer's own branch → divergent, unmergeable).
- [ ] 1.2 Point the same bullet at the `agent.advanced-main` discipline as the supported way to take on peer work after it lands on the base branch.
- [ ] 1.3 Substitute the agent's own id into the *Watch peer status* bullet (`:171`) so no literal `{{BRANCH_ID}}` reaches the sidecar. `generate_worktree_section` copies `inter_agent_rules` verbatim (`:219`), so the interpolation must happen in `build_inter_agent_rules` itself — do NOT route the string through `skills::render()`.
- [ ] 1.4 Reword the `inter_agent_rules` doc comment at `:135` (it currently lists "cherry-pick" as one of the rule categories).
- [ ] 1.5 Confirm `src/agents.rs:518` (branch-guard hook feedback text) is untouched — its cherry-pick instruction is self-correction for a mis-branched commit and is unrelated to peer grafting.

## 2. Bundled coordination skill (`assets/agent-skills/coordination.md`)

- [ ] 2.1 Replace the `### Cherry-pick peer commits` section (~`:511-527`) with a `### When you depend on a peer's work` section carrying the D1 escalate-and-continue rule and the D2 pointer to *When main advances*.
- [ ] 2.2 Reword the two "peers should cherry-pick" mentions in the `agent.artifact` / exports guidance (~`:438`, `:457`) so exports advertisement survives without implying the peer grafts the commit.
- [ ] 2.3 Verify the *Terminal action: commit then publish, never archive* section (~`:471`) still states that the supervisor cherry-picks and merges the worker's branch onto the release line — this supervisor-side language is correct and stays.
- [ ] 2.4 Grep the asset for `git cherry-pick` and confirm zero hits.

## 3. Tests

- [ ] 3.1 Invert `src/agents.rs:1866` `build_inter_agent_rules_contains_cherry_pick_reference` → assert the rules do NOT instruct peer cherry-picking, and DO instruct `agent.blocked` escalation. Rename accordingly.
- [ ] 3.2 Invert `src/agents.rs:1876` `embedded_coordination_contains_cherry_pick` → assert the embedded skill does NOT contain `git cherry-pick`. Rename accordingly.
- [ ] 3.3 Invert `src/skills.rs:1092` `coordination_skill_contains_cherry_pick_instructions` → assert absence, and add a positive assertion for the new peer-dependency section. Rename accordingly.
- [ ] 3.4 Add a test asserting the rendered worktree section contains no literal `{{BRANCH_ID}}` when `inter_agent_rules` is supplied (covers the boot-agents-md placeholder scenario).
- [ ] 3.5 Add a test asserting the supervisor-side integration language survives in the embedded skill, so the absence assertions in 3.2/3.3 cannot be satisfied by blanket deletion.
- [ ] 3.6 Grep `cherry` across `src/` and `tests/` and confirm every remaining hit is either the branch-guard hook (`agents.rs:518`) or supervisor-side prose.

## 4. Docs

- [ ] 4.1 Reword `docs/src/user-guide/coordination.md:507` ("announce named `exports` peers should cherry-pick") to match the new skill text. This page is a hand-maintained paraphrase, not an `{{#include}}`, so it requires an explicit edit.
- [ ] 4.2 Confirm `docs/src/user-guide/coordination.md:517` and `docs/src/user-guide/supervisor.md:331` (supervisor-side cherry-pick) are unchanged.
- [ ] 4.3 Run `mdbook build docs/` and confirm it succeeds.

## 5. Quality gates

- [ ] 5.1 `cargo fmt` before committing (hand-edited tests are frequently not rustfmt-clean).
- [ ] 5.2 `just check` — fmt + clippy + full test suite; verify by real exit code, not piped output.
- [ ] 5.3 Confirm no `unwrap()`/`expect()` introduced in non-test code and all touched public items keep doc comments.
- [ ] 5.4 Re-read the two edited authoring sites side by side and confirm they now agree — a worker reading both the sidecar rules and the skill must get one consistent instruction.

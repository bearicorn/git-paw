## Context

Every agent launched by `git-paw start --from-specs --supervisor` (and equivalent flows) receives a "boot block" at the head of its first prompt. The block is rendered by `git_paw::skills::build_boot_block` (`src/skills.rs:387`) from `assets/boot-block-template.md`, with `{{BRANCH_ID}}` and `{{GIT_PAW_BROKER_URL}}` substituted in. The template enumerates four coordination events the agent SHALL publish during its life: REGISTER (initial heartbeat), DONE (completion), BLOCKED (waiting on a peer), QUESTION (uncertainty escalation).

Section 2 (DONE) instructs the agent to publish `agent.artifact { status: "done" }` "when you complete your assigned task". It does not mention `git commit`. There is no language reserving the manual DONE event for any subset of tasks.

Separately, `git_paw::agents::build_post_commit_dispatcher_hook` (`src/agents.rs:382`) installs a `post-commit` hook in the main repo's shared `hooks/` directory. The hook reads `$GIT_DIR/paw-agent-id` (set per-worktree by `setup_worktree_agents_md`), computes `modified_files` from `git diff HEAD~1 --name-only`, and POSTs `agent.artifact { status: "committed", exports: [], modified_files: [...] }` to the broker. The hook is installed unconditionally; every commit by an agent worktree publishes a `committed` event automatically.

Both events are recognised as terminal statuses by `src/broker/delivery.rs::is_terminal_status` (`done | verified | blocked | committed`), and the dashboard renders both with distinct glyphs. The two events are wire-compatible — a downstream consumer can treat either as "agent reached a completion checkpoint".

The dogfood evidence is that the two paths are not interchangeable in practice:

| Agent (v0.5.0 dogfood) | Path taken | Outcome |
|---|---|---|
| `feat-governance-config` | Manual DONE | Reported done with 9 uncommitted files; supervisor merge failed (no commits to merge) |
| `feat-cross-format-spec-selection` | Manual DONE | Reported done with 12 uncommitted files; same failure mode |
| `feat-governance-context` | Commit → hook | Committed, hook fired, supervisor merge succeeded |
| `feat-forward-coordination` | Commit → hook | Committed, hook fired, supervisor merge succeeded |

Both failed agents had been instructed by the boot block and were obediently following it. The fix is to instruct them differently.

## Goals / Non-Goals

**Goals:**
- The boot block's DONE section leads with "commit your work via `git commit`" and explains that the post-commit hook will auto-publish `agent.artifact { status: "committed" }`.
- The manual `agent.artifact { status: "done" }` curl remains in the template but is explicitly scoped to code-less tasks (docs-only, planning, exploration) and explicitly forbidden when uncommitted changes exist.
- The four-section structure of the boot block (REGISTER / DONE / BLOCKED / QUESTION) is preserved so existing assertions about section count and order continue to hold.
- The change is content-only — no Rust code changes, no broker schema changes, no config changes.

**Non-Goals:**
- Removing the manual `done` event from the broker protocol. Code-less tasks (planning agents, learnings-mode summarisers, future docs-only flows) still need a way to report completion without a commit.
- Hardening the broker to reject `done` events when the worktree has uncommitted files. The broker has no view into the agent's worktree state at publish time, and that kind of cross-validation belongs in a future broker-side change, not in the boot block.
- Renaming `committed` → `done` (or vice versa) on the wire. The dashboard, delivery layer, and supervisor skill all reason about both statuses today; renaming would be a breaking change across the entire v0.5.0 surface for no semantic gain.
- Changing the post-commit hook payload. The hook already publishes the right shape; the bug is the agent-facing wording, not the wire format.
- Updating the supervisor skill (`assets/agent-skills/supervisor.md`). The supervisor already handles both `done` and `committed` events; clarifying the agent-facing instruction is sufficient. If the supervisor skill's wording on "what to verify" also conflates the two paths, that's a follow-up tracked under MILESTONE drift item 38 but not this change.

## Decisions

### D1. Keep the manual DONE event; just narrow its documented use case

**Choice:** The manual `agent.artifact { status: "done" }` curl stays in the boot block. The body of section 2 is rewritten to (a) lead with the commit-first instruction, (b) document the post-commit hook's auto-publish behaviour, and (c) describe the manual event as a code-less-task fallback with an explicit warning against using it when uncommitted changes exist.

**Why:**
- Code-less agents are a real category. Examples already shipping in v0.5.0: `learnings-mode` (writes a summary into broker stream, no worktree commits required), planning agents that produce only OpenSpec proposals via the supervisor's review channel, exploration agents that publish findings as `agent.question` follow-ups. Removing the manual `done` event would leave these without a completion signal.
- The two events publish different shapes downstream (`committed` carries the post-commit `modified_files` from `git diff HEAD~1`; manual `done` carries whatever the agent self-reports). Forcing every agent through the commit path would either require committing empty changesets (allowed but noisy) or require a code-less mode marker. Both are heavier than a wording tweak.
- The boot block is the only document the agent reads first; making it the single source of guidance keeps the four-event mental model intact for agents — REGISTER / DONE / BLOCKED / QUESTION still maps 1:1 to lifecycle phases. Section 2 just acquires a primary path and a fallback within the same section.

**Alternatives considered:**
- *Remove section 2 entirely* — Forces every agent through `git commit`, including code-less ones. Loses a real use case; requires inventing a "commit empty" convention. Rejected.
- *Split section 2 into 2a (commit) and 2b (manual done)* — Adds a fifth section visually, breaks the `Boot block contains all four essential events` scenario, and requires renumbering the BLOCKED/QUESTION sections. Cleaner to keep one section with two clearly-labelled paths inside it.
- *Make the manual `done` curl opt-in via a config flag (e.g. `[supervisor.allow_manual_done]`)* — Pushes the decision to operators who don't have the agent-side context to make it well. The wording change captures the right policy without adding a knob.
- *Have the broker reject `done` events with an empty `modified_files` array OR when the agent's branch has zero commits* — Defensive but architecturally wrong: the broker doesn't have a guaranteed view into worktree state, and `modified_files = []` is valid for code-less completions. Rejected.

### D2. Section heading stays `DONE: Task completion reporting`

**Choice:** Section 2's heading text is unchanged. Only the body is rewritten.

**Why:**
- The existing `boot-block-format` spec's "Standard boot block format" requirement enumerates the four sections by name (REGISTER, DONE, BLOCKED, QUESTION). Changing the heading would force a second MODIFIED requirement covering structure, and would invalidate the `boot_block_contains_all_four_essential_events` test as written.
- "DONE" is still semantically accurate — the section covers task completion. Both the commit path and the manual fallback are mechanisms for reaching DONE.
- The dogfood agents that failed did not fail because the heading misled them — they read the body and acted on its instruction. Fixing the body without the heading is the minimum viable change.

**Alternatives considered:**
- *Rename to `COMPLETE: Commit your work`* — More directive but breaks the four-event naming convention and requires updates throughout the supervisor skill and any user docs. Rejected.
- *Add a new section heading "2.1 Code path / 2.2 Code-less path"* — Adds visual complexity. Rejected; markdown sub-bullets inside section 2 are enough.

### D3. The manual curl remains in the template, not stripped

**Choice:** Section 2's body retains the existing `curl ... -d '{"type":"agent.artifact",...,"status":"done"...}'` command verbatim, presented as the fallback for code-less tasks. The curl is wrapped with a preceding warning ("Do NOT publish manual `done` when your worktree has uncommitted changes") and a follow-up instruction directing the agent to commit instead.

**Why:**
- Code-less agents need a copy-pasteable curl, same as any other coordination event. Removing it would force them to construct the JSON by hand, which is exactly the friction the boot block exists to eliminate.
- The pre-expanded curl pattern is consistent with REGISTER / BLOCKED / QUESTION (all four sections show a literal command). Removing one would create asymmetry.
- The risk that an agent reads only the curl and ignores the surrounding warning is real but mitigated by (a) leading the section with the commit-first instruction so the warning is encountered first, and (b) bolding the warning. Dogfood will confirm whether this is enough; if it isn't, a v0.6.0 follow-up can move the curl into a tutorial outside the boot block.

**Alternatives considered:**
- *Replace the manual curl with a link to docs* — Defeats the boot block's "everything copy-pasteable" design. Rejected.
- *Move the manual curl to a sibling skill file (e.g. `agent-skills/code-less-completion.md`) injected only for code-less branches* — Requires a per-branch boot block builder that takes a "code-less" flag, plus per-branch frontmatter in the spec entry. Out of scope; the wording change captures the policy without the plumbing.

### D4. Test the rendered boot block, not the template source

**Choice:** Skill-content tests in `src/skills.rs::tests` assert against the output of `build_boot_block("feat/test", "http://127.0.0.1:9119")`, not against `include_str!("../assets/boot-block-template.md")` directly.

**Why:**
- The rendered string is what agents actually see. Template-source assertions miss substitution bugs.
- `build_boot_block` is the only public surface for the template; existing tests (`boot_block_contains_all_four_essential_events`, `boot_block_substitutes_branch_id_placeholder`, etc.) follow the same pattern.
- Substring assertions against the rendered output are robust to whitespace/formatting tweaks in the template as long as the key phrases remain present.

## Risks / Trade-offs

- **[An agent reads the curl in section 2 and uses it without reading the surrounding commit-first instruction]** → Mitigation: the commit-first instruction comes first in the section body, and the warning against using manual `done` with uncommitted changes is bolded. If dogfood evidence shows agents still bypass the commit path, a follow-up can move the manual curl behind a per-branch code-less flag (D3 alternative).

- **[A code-less agent reads the section and assumes its task is "actually" code-bearing because the section leads with commit-first]** → The fallback paragraph explicitly enumerates code-less task types (docs-only, planning, exploration). The supervisor agent's monitoring loop can also publish a clarifying `agent.feedback` if a code-less agent reports working without committing — that's existing supervisor behaviour, not new.

- **[Template change breaks downstream tooling that scrapes the boot block]** → Search shows no scraper exists outside `src/skills.rs` and its tests. The spec's "Boot block uses consistent formatting" scenario asserts the four-section structure; that structure is preserved.

- **[mdBook docs go stale]** → Tasks call out a one-line review of `docs/src/architecture/boot-block.md` (if present) to align with the new wording. The chapter, if it exists, describes the four-event model at a high level and doesn't quote the DONE body verbatim.

- **[Other CLIs we don't dogfood yet (Codex, Gemini) read the wording differently]** → The wording change makes the contract more explicit, not less. Any CLI that auto-loads AGENTS.md and reads the boot block on launch sees the same disambiguated instruction. No regression.

## Migration Plan

This is a single-asset content edit with skill-content tests.

1. **Asset edit** in `assets/boot-block-template.md`: rewrite section `### 2. DONE: Task completion reporting`. Lead with the commit-first instruction. Follow with the bolded warning, then the fallback paragraph + manual curl for code-less tasks.
2. **Test additions** in `src/skills.rs::tests`:
   - `boot_block_done_section_leads_with_commit_instruction` — asserts the rendered block contains a substring like "commit your work" appearing before the manual `done` curl.
   - `boot_block_done_section_names_committed_status_published_by_hook` — asserts the rendered block mentions `agent.artifact { status: "committed" }` and references the post-commit hook.
   - `boot_block_done_section_scopes_manual_done_to_code_less_tasks` — asserts the rendered block lists code-less task examples (docs-only, planning, exploration) as the case where manual `done` is appropriate.
   - `boot_block_done_section_warns_against_manual_done_with_uncommitted_changes` — asserts the rendered block contains a bolded/emphasised warning against publishing manual `done` when uncommitted changes exist.
3. **Rollback** — revert the template edit and remove the new tests. The previous (ambiguous) wording is restored. Loss of clarity, but no functional regression in any other component.

No flag, no opt-in, no version gate. The new wording ships with v0.5.0 cleanup or as a point release.

## Open Questions

- *Should the supervisor skill's verification step also be reworded to prefer `committed` events over `done` events when both arrive for the same agent?* Probably yes for consistency, but tracked separately. The supervisor today treats both as terminal and runs `just check` either way; the difference matters only at merge time, where the supervisor already needs commits to merge regardless of event type. Captured as a v0.5.0 follow-up.

- *Should `agent.artifact { status: "done" }` with non-empty `modified_files` trigger a broker-side warning (since the agent is claiming files changed without committing them)?* Possibly useful diagnostic, but out of scope here. Captured as a v0.6.0 broker-hardening candidate.

- *Should we also clarify section 1 (REGISTER) to mention that the supervisor watches for `working` heartbeats?* MILESTONE drift item 37 covers heartbeat publishing and belongs in `forward-coordination`, not this change.

## Context

Four dogfood-discovered coordination-skill gaps (MILESTONE drift 54-57) that don't fit any existing in-flight change. The `forward-coordination` change is mid-implementation by an agent who has already published `agent.verified`; re-opening it would invalidate the verification. The four items are all asset-and-spec edits to `coordination.md` and `supervisor.md`. Bundling them avoids merge-conflict thrash between four micro-changes and keeps the OpenSpec history readable as "v0.5.0 coordination-skill follow-ups".

## Decisions

### D1 — Bundle vs split

**Decision:** Bundle all four drift items (54, 55, 56, 57) into one change rather than create four micro-changes.

**Rationale:**
- All four items edit the same two files (`assets/agent-skills/coordination.md`, `assets/agent-skills/supervisor.md`). Four parallel micro-changes would race on the same two files and produce textual merge conflicts on every pairwise merge.
- Each item is too small to justify its own proposal/design/specs/tasks set; the ceremony-to-substance ratio would be poor.
- The four items share a single theme — "lessons from the v0.5.0 dogfood that the coordination + supervisor skills should teach" — which reads coherently as a single change in the OpenSpec history.
- Roll-back granularity is not a concern: skill content is not load-bearing for the binary (no behavioural code depends on specific substrings outside of the test module).

**Alternatives considered:**
- *Four separate changes* — rejected for the reasons above.
- *Fold into `forward-coordination`* — rejected because that change is mid-implementation and has published `agent.verified`. Re-opening it would invalidate verification and force a full re-audit.
- *Defer to v0.6.0* — rejected because three of the four items (54, 56, 57) are dogfood-cost issues that compound every session; landing them in v0.5.0 is cheap and high-value.

### D2 — Order relative to `forward-coordination`

**Decision:** This change applies **after** `forward-coordination` archives. The four new subsections go in **net-new headings** in each skill file, never overlapping any heading that `forward-coordination` already creates or rewrites.

**Heading layout (reserved by this change):**

In `assets/agent-skills/coordination.md`:
- `### References & terminology` — appended after the existing `### Messages you may receive` section, before any trailing footer. New, not touched by `forward-coordination`.
- `### Stash hygiene` — appended after `### References & terminology`. New, not touched by `forward-coordination`.

In `assets/agent-skills/supervisor.md`:
- `### Supervisor publishes agent.intent for main-side work` — appended after the existing `### Conflict detection` section (which `forward-coordination` retains) and before `### Rules`. New, not touched by `forward-coordination`.
- `### Verify accept-edits commits before merge` — appended as a sub-bullet under the existing **Spec Audit Procedure** section (which `forward-coordination` retains) OR as a sibling subsection immediately after it. Implementer SHALL pick the sibling-subsection form for clarity. New, not touched by `forward-coordination`.

**Why explicit heading reservation:** `forward-coordination` rewrites large chunks of both files (the entire `Before you start editing` / `While you're editing` / `Working heartbeat` flow in `coordination.md`; the `Watch peer intents` pointer and `agent.question` answer step in `supervisor.md`). If this change rewrote any existing heading, the merge would be a conflicting rewrite. By targeting only **new** headings, the merge is purely additive — `git merge` sees a clean append, not a conflict.

**Rationale for ordering:** if both changes were in flight simultaneously, `forward-coordination` lands first because it is already verified. This change then rebases on top, picks up the new file content `forward-coordination` produced, and appends its four subsections. No spec-time interaction.

### D3 — Drift 54 placement (slugify docs)

**Decision:** Add a new `### References & terminology` subsection at the end of `coordination.md` that documents the two identifier forms and names `slugify_branch` as the conversion rule.

**Content sketch (illustrative, not literal):**
> Throughout the broker protocol, **two related forms** of agent identifier appear:
>
> - **Branch name** — the original git ref, e.g. `feat/no-supervisor-flag`. Used in `git checkout`, `git worktree`, and anywhere git itself is involved.
> - **`agent_id`** — the broker-side dashed slug, e.g. `feat-no-supervisor-flag`. Used in every `/publish` payload, `/messages/<id>` URL, and `agent.feedback`/`agent.question` `target` field.
>
> `agent_id` is the **slugified** form of the branch name. The conversion rule (`slugify_branch`):
> 1. Lowercase to ASCII.
> 2. Replace any character that isn't `[a-z0-9_]` with `-`.
> 3. Collapse consecutive `-` to a single `-`, trim leading/trailing `-`.
> 4. If the result is empty, fall back to `agent`.
>
> When you receive an `agent.feedback` or compose an `agent.question`, the `target` field MUST be the `agent_id` form. When you `git checkout` or reference a worktree, use the branch form.

**Why this placement:** end-of-file, after the operational sections (status, blocked, artifact, cherry-pick, messages), reads as a glossary/reference — matches the way readers actually use it (look up when confused, not at the start).

A brief reminder appears in `supervisor.md`'s `### Supervisor publishes agent.intent for main-side work` subsection (D4) where it would otherwise be ambiguous which form to use.

### D4 — Drift 55 placement (supervisor publishes agent.intent)

**Decision:** Add a new `### Supervisor publishes agent.intent for main-side work` subsection to `supervisor.md`, slotted after `### Conflict detection` and before `### Rules`.

**Content sketch:**
> When you (the supervisor) commit bug fixes, prep work, or other changes directly to `main` while coding agents are running in feat branches, those commits do not surface as broker events on the agents' side. Agents working off a stale `main` may produce incompatible commits without realising it.
>
> Before you edit `main`, publish an `agent.intent` from `agent_id = "supervisor"`:
> ```bash
> curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish \
>   -H "Content-Type: application/json" \
>   -d '{"type":"agent.intent","agent_id":"supervisor","payload":{"files":["path/one.rs","path/two.rs"],"summary":"<one-line summary>","valid_for_seconds":600,"scope":"main"}}'
> ```
> Cross-reference: see the agent-side `agent.intent` flow documented in `coordination.md` (`Before you start editing`).

**Why this placement:** it's a peer to `### Conflict detection` (also supervisor-driven, also coordination-protocol-shaped) and a cousin to the merge-orchestration steps in the numbered workflow. Placing it as a top-level subsection rather than burying it inside the workflow keeps it discoverable.

**Cross-reference to forward-coordination:** the `agent.intent` wire format is defined and validated by `forward-coordination` (capability `broker-messages`). This change only teaches a new caller (the supervisor itself) and reuses the existing payload shape. The optional `scope: "main"` field in the example is illustrative — it is **not** a required field in the wire format and SHALL NOT be tested by `broker-messages` scenarios. If `scope` is omitted, the payload remains valid.

### D5 — Drift 56 placement (accept-edits modified_files audit)

**Decision:** Add a new `### Verify accept-edits commits before merge` subsection to `supervisor.md`, slotted as a sibling section immediately after the existing **Spec Audit Procedure** section (which `forward-coordination` retains untouched).

**Content sketch:**
> Claude Code's `⏵⏵ accept edits` auto-mode (and equivalent in other CLIs) silently applies file edits without re-prompting. You lose real-time visibility into what the agent is editing. The fix is post-hoc: when you receive `agent.artifact { modified_files: [...] }` from such an agent, cross-reference the list against the change's owned-files / expected-files set:
>
> 1. Locate the change's `proposal.md` and read the **Impact / Code** section — that's the canonical list of files this change is allowed to touch.
> 2. Diff `modified_files` against that list. Files in `modified_files` but NOT in the proposal's expected set are out-of-scope edits.
> 3. For any out-of-scope edit, decide:
>    - If it is benign (whitespace, a typo fix in an adjacent line, etc.) — note it in the `agent.verified` message.
>    - If it is substantive — publish `agent.feedback` asking the agent to revert or justify.
>
> Do not auto-approve out-of-scope edits silently; that re-creates the visibility gap the accept-edits mode opened in the first place.

**Why this placement:** it sits in the post-test, pre-`agent.verified` zone, right next to Spec Audit, because it's another verification gate that runs at the same point in the workflow. Keeping it as a sibling subsection rather than a sub-step of Spec Audit makes the dependency on `modified_files` (which Spec Audit doesn't use) self-contained.

### D6 — Drift 57 placement (stash hygiene)

**Decision:** Add a new `### Stash hygiene` subsection to `coordination.md`, slotted after the new `### References & terminology` subsection (D3).

**Content sketch:**
> In a git-paw session, multiple worktrees and multiple agents may have produced stashes on overlapping branches. **Never blindly `git stash pop`** — the stash you pop may belong to a different agent, and popping it can either lose your in-flight work (if the pop conflicts) or contaminate your worktree with foreign changes.
>
> Three rules, in order:
> 1. **List first.** `git stash list`. Inspect every entry's branch and timestamp.
> 2. **Inspect before pop.** `git stash show -p stash@{N}` for the specific entry you want. Confirm the patch matches what you expect.
> 3. **Pop only your own.** Only pop entries you authored on the current worktree. If you cannot identify the author with confidence, leave the stash alone and ask the supervisor via `agent.question`.
>
> **Cautionary tale:** in a v0.5.0 dogfood session, an agent ran `git stash pop` without listing first; the popped stash was from an unrelated worktree, and the pop wiped the agent's in-progress changes. This is the failure mode the rules above prevent.

**Why this placement:** end-of-file, alongside the other reference content (D3). Stash hygiene is a "you'll need this when something goes wrong" topic; reference-style placement matches usage.

### D7 — Skill-content tests

**Decision:** Add 4-6 unit tests in `src/skills.rs::tests` asserting each new subsection's key substrings are present in the embedded skill content.

**Rationale:** consistent with the existing test pattern (see `coordination_skill_contains_cherry_pick`, `supervisor_skill_contains_spec_audit`, etc.). Substring assertions are cheap, stable, and catch the most common regression (someone accidentally deletes a subsection during a future edit).

**Test list:**
1. `coordination_skill_documents_slugify_terminology` — assert `slugify_branch` and `agent_id` are present near each other.
2. `coordination_skill_documents_stash_hygiene` — assert `git stash list`, `git stash show -p`, and substring like "pop only" are present.
3. `supervisor_skill_documents_main_side_intent` — assert `agent.intent` and `agent_id":"supervisor"` (or substantively equivalent) and `scope` are present.
4. `supervisor_skill_documents_accept_edits_audit` — assert `accept edits` and `modified_files` and `out-of-scope` (or substantively equivalent) are present.

Optional 5th-6th: assert each new heading is reachable as a top-level section (heading-line substring match).

### Alternatives

- **A1: Fold into `forward-coordination`.** Rejected — that change is already verified and mid-merge. Re-opening forces re-audit.
- **A2: Defer to v0.6.0.** Rejected — drift 57 is a data-loss pattern; landing the prevention guidance in the same release as the dogfood that surfaced it is the right call.
- **A3: Write a single combined "References" subsection covering drifts 54 and 57 together.** Rejected — slugify and stash hygiene are conceptually unrelated. Mashing them together makes both harder to find.
- **A4: Make drift 55 a wire-format extension (`scope: "main"` as a required field on `agent.intent`).** Rejected — that would be a `broker-messages` capability modification that does not belong in this change. Keep `scope` advisory and informational; if it later proves load-bearing, promote it via its own change.

### Risks

- **R1: Merge conflicts with `forward-coordination`.** Mitigated by D2's explicit heading reservation — every new subsection in this change targets a heading that does not exist in `forward-coordination`'s output. The merge is a clean append.
- **R2: Stale references if `forward-coordination` renames a section.** If `forward-coordination` renames `### Messages you may receive` (the section this change anchors after in `coordination.md`), the append still works — git diff matches on file end, not on the preceding section's heading text. But the implementer SHALL re-read both files after rebase to confirm placement still reads cleanly.
- **R3: Drift 55 example mentions a `scope` field that is not in the `agent.intent` wire format.** Mitigated by D4's note that `scope` is illustrative and SHALL NOT be tested by `broker-messages` scenarios. If a future change formalises `scope`, this skill content will already match.
- **R4: Tests become brittle if the skill content is reworded.** Mitigated by asserting on substring patterns (`slugify_branch`, `git stash list`, `accept edits`, `modified_files`) rather than exact prose. Future rewordings that preserve those keywords pass.

### Open questions

- **OQ1:** Should the supervisor-publishes-intent example include `scope: "main"` even though that field is not in the wire format? **Resolution:** yes, illustratively — it signals to peers and human readers that the supervisor is acting on `main`, not on a worktree, and does not affect validation because `agent.intent` payloads tolerate extra serde-rest fields. If a future change formalises `scope`, this content already matches.
- **OQ2:** Should stash hygiene cover the case where the supervisor (not the agent) initiated a stash? **Resolution:** out of scope. The supervisor's stash usage is one-off and ad-hoc; the agent-facing rules ("only pop your own") cover the dogfood pattern that motivated this change.

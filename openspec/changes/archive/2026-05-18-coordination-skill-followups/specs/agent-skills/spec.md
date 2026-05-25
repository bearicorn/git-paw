## MODIFIED Requirements

### Requirement: Embedded coordination skill

The embedded `coordination.md` skill content SHALL reflect the v0.5 state in which agents publish `agent.intent` before editing as the primary coordination signal, while `agent.status` publishing remains automated by the filesystem watcher and `agent.artifact` publishing remains automated by the post-commit git hook. The embedded content SHALL therefore:

1. NOT contain the legacy "MUST publish agent.status" instruction. Status publishing is automatic — agents do not curl `/publish` for `agent.status` themselves.
2. Include a note explaining that git-paw automatically publishes the agent's working status when the agent edits files and automatically publishes an `agent.artifact` when the agent runs `git commit`. The note SHALL state that agents only need to publish manually if they are blocked, want to announce explicit exports, or are signalling intent.
3. Retain the `agent.blocked` curl example as an opt-in operation for blocked agents.
4. Retain the `agent.artifact` curl example with `exports`, documented as the manual escape hatch when the agent wants to advertise specific exports beyond what the post-commit hook captures automatically.
5. Include a `### Cherry-pick peer commits` section that gives the exact `git cherry-pick` command an agent should run when a peer's `agent.artifact` arrives in the agent's inbox.
6. Include a `### Messages you may receive` section that documents the two supervisor-originated message variants:
   - `agent.verified` — the agent's work has been verified by the supervisor. No action required.
   - `agent.feedback` — the agent's work has issues. The `errors` field lists problems to fix; the agent SHALL address them and re-publish `agent.artifact`.
7. Continue to use `{{BRANCH_ID}}` and `{{GIT_PAW_BROKER_URL}}` placeholders, retaining the existing polling example `GET {{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}`.
8. Include a `### Before you start editing` section that instructs the agent to: (a) read its spec or task; (b) publish `agent.intent` listing the specific files it plans to touch with a one-line summary and a TTL; (c) poll once for warnings; (d) on overlap, decide whether to wait, split scope, or escalate via `agent.question`. The section SHALL include a `curl` example that publishes `agent.intent` with `files`, `summary`, and `valid_for_seconds`.
9. Include a `### While you're editing` section that instructs the agent to: (a) re-publish `agent.intent` if scope grows to include files not in the original list; (b) on seeing a peer's `agent.intent` for a file in the same module, send `agent.question` rather than racing. The section SHALL state explicitly that agents MUST NOT do pairwise check-ins on every change, MUST NOT wait for explicit go-ahead from peers when no conflict signal exists, and MUST NOT block on broker silence.
10. Include a `### Working heartbeat` section (or equivalent heading) that instructs the agent to publish a lightweight `agent.status` heartbeat with `status: "working"` every **5 tool uses** while actively working. The section SHALL:
    - State the cadence explicitly as "every 5 tool uses" (or substantively equivalent text naming the number 5).
    - Provide a `curl` example that publishes `agent.status` with `status: "working"` and `modified_files: []` (or the current dirty file list) so the broker treats it identically to watcher-driven status updates.
    - Explain *why* an agent-side heartbeat is needed in addition to the automatic filesystem watcher: the watcher cannot observe read-only tool uses (Read, Grep, Glob), permission-prompt waits, or LLM-only deliberation between tool calls, so `last_seen` would otherwise stay stale during active work.
    - Frame the cadence as a SHOULD (recommended floor): publishing more often is fine, publishing less often defeats the purpose.
11. Include a `### References & terminology` section (or equivalent heading) that documents the two related forms of agent identifier used throughout the broker protocol and names `slugify_branch` as the canonical conversion. The section SHALL:
    - Identify the **branch-name** form (e.g. `feat/no-supervisor-flag`) as the original git ref used in `git checkout`, `git worktree`, and other git-side operations.
    - Identify the **`agent_id`** form (e.g. `feat-no-supervisor-flag`) as the dashed slug used in every `/publish` payload, every `/messages/<id>` URL, and the `target` field of `agent.feedback` / `agent.question` payloads.
    - State explicitly that `agent_id` is the **slugified** form of the branch name, and name the conversion function as `slugify_branch`.
    - Describe the slugify rule's effect (lowercase, non-`[a-z0-9_]` chars become `-`, consecutive `-` collapse to one, empty fallback to `agent`) so a reader can predict the conversion without reading the source.
    - State which form to use in which context: the `agent_id` form in every broker payload `target` field; the branch-name form in git operations.
12. Include a `### Stash hygiene` section (or equivalent heading) that instructs the agent how to safely handle stashes in a multi-worktree environment. The section SHALL contain three rules in order:
    - **List before pop** — always run `git stash list` first.
    - **Inspect before pop** — use `git stash show -p stash@{N}` to inspect any candidate entry's patch contents before popping.
    - **Pop only your own** — only pop stash entries you authored on the current worktree; if authorship is uncertain, leave the stash alone and escalate via `agent.question` rather than risk a destructive pop.

    The section SHALL state that `git stash pop` SHOULD NOT be run blindly. The section MAY include a cautionary narrative referencing a real dogfood incident where a blind pop wiped in-flight work.

#### Scenario: Coordination skill documents automatic status publishing

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains text indicating that `agent.status` publishing is automatic
- **AND** it does NOT contain the substring "MUST publish agent.status"

#### Scenario: Coordination skill retains blocked and artifact curl examples

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a `curl` example for publishing `agent.blocked`
- **AND** it contains a `curl` example for publishing `agent.artifact`

#### Scenario: Coordination skill contains cherry-pick instructions

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains the substring `git cherry-pick`
- **AND** the cherry-pick guidance is reachable under a `Cherry-pick peer commits` heading or equivalent

#### Scenario: Coordination skill documents verification and feedback messages

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains the substring `agent.verified`
- **AND** it contains the substring `agent.feedback`
- **AND** it contains guidance describing how to handle feedback (fix the listed errors and re-publish `agent.artifact`)

#### Scenario: Coordination skill retains polling reference

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains `{{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}`

#### Scenario: Coordination skill contains Before you start editing section

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a heading `Before you start editing` (or equivalent)
- **AND** it contains a `curl` example that publishes `agent.intent`
- **AND** the `agent.intent` example includes `files`, `summary`, and `valid_for_seconds` payload fields

#### Scenario: Coordination skill contains While you're editing section

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a heading `While you're editing` (or equivalent)
- **AND** it instructs the agent to re-publish `agent.intent` when scope grows
- **AND** it instructs the agent to use `agent.question` (not pairwise blocking) when a peer's intent overlaps

#### Scenario: Coordination skill rejects pairwise over-coordination patterns

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains explicit guidance that agents MUST NOT perform pairwise check-ins on every change
- **AND** it contains explicit guidance that agents MUST NOT wait for go-ahead from peers when no conflict signal exists
- **AND** it contains explicit guidance that agents MUST NOT block on broker silence

#### Scenario: Coordination skill instructs working heartbeat every five tool uses

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a heading or sub-section naming the working heartbeat (e.g. `Working heartbeat`, `Heartbeat`, or equivalent)
- **AND** it contains the literal cadence text `every 5 tool uses` (or substantively equivalent text naming the number `5`)
- **AND** it contains a `curl` example that publishes an `agent.status` message with `status` set to `"working"`

#### Scenario: Coordination skill explains why heartbeats supplement the filesystem watcher

- **WHEN** the embedded coordination skill's working-heartbeat section is inspected
- **THEN** it explains that the filesystem watcher does not see read-only tool uses, permission-prompt waits, or LLM-only deliberation
- **AND** it states that the heartbeat keeps `last_seen` fresh during active work that does not touch files

#### Scenario: Coordination skill documents agent_id vs branch slugify rule

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a heading or sub-section naming references / terminology (e.g. `References & terminology`, `Terminology`, or equivalent)
- **AND** it contains the substring `agent_id`
- **AND** it contains the substring `slugify_branch`
- **AND** it instructs the agent to use the `agent_id` (dashed) form in broker `target` fields and the branch-name (slashed) form in git operations
- **AND** it describes the slugify rule sufficiently for a reader to predict the conversion (lowercase, non-`[a-z0-9_]` chars to `-`, collapse, fallback to `agent` on empty)

#### Scenario: Coordination skill documents stash hygiene rules

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a heading or sub-section naming stash hygiene (e.g. `Stash hygiene`, `Stash safety`, or equivalent)
- **AND** it contains the substring `git stash list`
- **AND** it contains the substring `git stash show -p`
- **AND** it instructs the agent to pop only stash entries the agent authored on the current worktree
- **AND** it states that `git stash pop` SHOULD NOT be run blindly (or substantively equivalent language)

### Requirement: Embedded supervisor skill

The embedded supervisor skill SHALL include a "Spec Audit Procedure" section that instructs the supervisor to verify implementation matches spec before publishing `agent.verified`. The procedure SHALL include:

- How to locate spec files for a given change
- How to extract WHEN/THEN scenarios from spec files
- How to search the codebase for matching tests
- How to verify struct fields, function signatures, and types match SHALL/MUST requirements
- How to compile gaps into an `agent.feedback` error list
- When to publish `agent.verified` (no gaps) vs `agent.feedback` (gaps found)

The spec audit SHALL run after the test command passes and before `agent.verified` is published.

The embedded supervisor skill SHALL ALSO include a "Watch peer intents" pointer that informs the supervisor that `agent.intent` messages arrive in its inbox alongside other peer events. The pointer SHALL state that programmatic conflict-warning logic is not part of this release and that the supervisor MAY inspect intents and prompt agents via `agent.feedback` or `agent.question` if it spots overlap manually. The pointer is intentionally advisory — full conflict-detection algorithms are owned by the `conflict-detection` change.

The embedded supervisor skill SHALL ALSO include a first-class workflow step covering how to answer an asking agent's `agent.question`. The step SHALL require the supervisor to:

1. Publish the `agent.feedback` response to the broker as before.
2. **ALSO** send the answer text to the asking agent's pane via `tmux send-keys -t paw-{{PROJECT_NAME}}:0.<pane-index> "<answer>" Enter`, because v0.5.0 agents do not poll their inbox for `agent.feedback` responses (drift 34). The instruction SHALL state this rationale explicitly so future contributors understand the workaround is transitional and slated for relaxation when MCP-mediated inbox access lands in v0.6.0.
3. **ALSO** apply the existing paste-buffer recovery sub-case when the answer text is long enough to land in a paste buffer on a paste-aware CLI — i.e. after the `tmux send-keys` of the answer, the supervisor SHALL inspect the pane and, if a paste-buffer indicator is present, send a follow-up `Enter` keystroke to submit the buffered content. This re-uses the paste-buffer recovery action already documented under stall detection; the supervisor skill SHALL cross-reference it (e.g. "see the paste-buffer recovery sub-case under stall detection") rather than duplicate the full recovery text.

The embedded supervisor skill SHALL ALSO include a "Supervisor publishes agent.intent for main-side work" section (or equivalent heading) instructing the supervisor agent to publish `agent.intent` from `agent_id = "supervisor"` whenever it is about to commit changes directly to `main` while coding agents are running. The section SHALL:

- Explain the visibility gap it closes: supervisor commits to `main` are not surfaced as broker events, and agents working in feat branches may produce commits incompatible with a freshly-advanced `main` if they are not notified.
- Provide a `curl` example with `type` = `agent.intent`, `agent_id` = `"supervisor"`, and a payload containing `files`, `summary`, and `valid_for_seconds`. The example MAY include an optional `scope: "main"` field, which is illustrative and not a required wire-format field.
- Cross-reference the agent-side `agent.intent` flow documented in `coordination.md` (the `Before you start editing` section) so the reader understands the supervisor reuses the same wire format.

The embedded supervisor skill SHALL ALSO include a "Verify accept-edits commits before merge" section (or equivalent heading) instructing the supervisor agent how to audit `agent.artifact { modified_files }` against the change's expected file set when the underlying agent ran in an auto-accept-edits mode that bypasses per-edit prompts. The section SHALL:

- Explain the visibility gap auto-accept-edits modes create: once enabled, file edits silently apply without re-prompting and the supervisor cannot watch each edit in real time.
- Instruct the supervisor to locate the change's `proposal.md` and read its expected file list (typically under an `Impact` / `Code` section) before inspecting an `agent.artifact`.
- Instruct the supervisor to diff `modified_files` against the expected list; files in `modified_files` but absent from the expected list SHALL be flagged as out-of-scope edits.
- Specify the supervisor's response: benign out-of-scope edits (whitespace, adjacent typo fixes) MAY be noted in `agent.verified`; substantive out-of-scope edits SHALL be raised via `agent.feedback` asking the agent to revert or justify.
- State that out-of-scope edits SHALL NOT be silently auto-approved.

#### Scenario: Supervisor skill contains spec audit procedure

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains the substring `Spec Audit`
- **AND** it contains instructions to read `openspec/changes/` spec files
- **AND** it contains instructions to grep for matching tests
- **AND** it contains instructions to verify field names match spec

#### Scenario: Spec audit runs after tests, before verified

- **WHEN** the embedded supervisor skill workflow is inspected
- **THEN** the spec audit step appears after the test command step
- **AND** the spec audit step appears before the `agent.verified` publish step

#### Scenario: Supervisor skill mentions agent.intent

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains the substring `agent.intent`
- **AND** it contains a heading or section titled `Watch peer intents` (or equivalent)
- **AND** it indicates that automatic conflict-warning logic is not part of this release

#### Scenario: Supervisor skill instructs tmux-send-keys alongside agent.feedback answers

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains explicit guidance that, when answering an `agent.question`, the supervisor MUST publish `agent.feedback` AND ALSO send the answer to the asking agent's pane via `tmux send-keys`
- **AND** it contains the substring `tmux send-keys` in the section covering `agent.question` answers
- **AND** it states the rationale (agents do not poll their inbox for `agent.feedback` responses)

#### Scenario: Supervisor skill cross-references paste-buffer recovery for long answers

- **WHEN** the supervisor skill's `agent.question` answer step is inspected
- **THEN** it instructs the supervisor to apply the paste-buffer recovery sub-case when the answer text is long enough to land in a paste buffer
- **AND** it instructs the supervisor to follow up with an `Enter` keystroke after the answer if a paste-buffer indicator is present

#### Scenario: Supervisor skill instructs publishing agent.intent for main-side work

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or section indicating the supervisor publishes `agent.intent` for main-side work (e.g. `Supervisor publishes agent.intent for main-side work`, `Publish intent before editing main`, or equivalent)
- **AND** it contains a `curl` example whose `type` field is `agent.intent` and whose `agent_id` field is `"supervisor"`
- **AND** the example's payload includes `files`, `summary`, and `valid_for_seconds`
- **AND** the section explains the visibility gap (agents in feat branches do not see supervisor commits to `main` without an explicit broker event)

#### Scenario: Supervisor skill cross-references the agent-side intent flow

- **WHEN** the supervisor-publishes-intent section is inspected
- **THEN** it cross-references the agent-side `agent.intent` flow in `coordination.md` (e.g. names the `Before you start editing` section or substantively equivalent)
- **AND** it states that the supervisor reuses the same wire format defined for coding agents

#### Scenario: Supervisor skill instructs accept-edits modified_files audit

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or section instructing the supervisor to verify `accept edits`-mode commits before merge (e.g. `Verify accept-edits commits before merge` or equivalent)
- **AND** it instructs the supervisor to locate the change's expected file list from `proposal.md`
- **AND** it instructs the supervisor to diff `agent.artifact { modified_files }` against the expected list
- **AND** it instructs the supervisor to flag out-of-scope edits via `agent.feedback`

#### Scenario: Supervisor skill forbids silent auto-approval of out-of-scope accept-edits changes

- **WHEN** the supervisor accept-edits audit section is inspected
- **THEN** it states that out-of-scope edits SHALL NOT be silently auto-approved
- **AND** it instructs the supervisor to publish `agent.feedback` for substantive out-of-scope edits asking the agent to revert or justify

### Requirement: Supervisor skill — paste-buffer recovery in stall detection

The embedded `supervisor.md` skill SHALL include a paste-buffer recovery sub-case under its existing stall-detection section. The sub-case SHALL instruct the supervisor agent that when a peer agent's `last_seen` has not advanced (or, at launch time, before any heartbeat has arrived) AND a `tmux capture-pane` of that peer's pane shows a paste-buffer indicator, the supervisor SHALL send a recovery `tmux send-keys -t <target> Enter` to submit the buffered content.

The sub-case SHALL:

1. Identify itself as an additional stall-detection case alongside the existing "idle prompt → likely done" and "thinking/waiting → prompt to self-report" cases.
2. List at least one known paste-buffer indicator pattern. The list SHALL include Claude Code's `Pasted text #N` (where `N` is a number) and SHALL be presented as illustrative-not-exhaustive so the supervisor agent can apply judgment to indicators on other CLIs.
3. Specify the recovery action as `tmux send-keys -t <pane> Enter` (a single Enter keystroke to the stuck pane).
4. State that the recovery action is safe-by-default — on a non-paste-aware CLI or a misclassified pane, the extra Enter either produces a benign blank prompt or is ignored.
5. Frame indicator detection as lenient: if a pane shows long buffered text in the input area without a follow-up response, the supervisor SHOULD attempt the recovery even if the literal indicator string is not on the listed-patterns list.
6. State that paste-buffer recovery SHALL also be applied **proactively at launch time** — the supervisor agent SHALL NOT wait for the `last_seen`-based stall threshold before inspecting agent panes for paste-buffer state. Coding-agent boot prompts are frequently long enough on paste-aware CLIs (e.g. Claude Code v2.1.x) to land in a paste buffer immediately, and waiting 30+ seconds for stall detection wastes the agents' productive time.

#### Scenario: Supervisor skill mentions paste-buffer recovery

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or sub-section identifying paste-buffer recovery (e.g. `paste-buffer`, `paste buffer`, or equivalent under stall detection)

#### Scenario: Supervisor skill names a known paste-buffer indicator

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it mentions Claude Code's `Pasted text #N` indicator pattern (or substantively equivalent text)

#### Scenario: Supervisor skill specifies the recovery action

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it instructs the supervisor agent to use `tmux capture-pane` to inspect the suspected pane
- **AND** it instructs the supervisor agent to send `tmux send-keys -t <pane> Enter` to recover
- **AND** it states that the Enter keystroke is safe-by-default (no-op or benign blank prompt on non-paste-aware CLIs)

#### Scenario: Supervisor skill frames indicator detection as lenient

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it instructs the supervisor agent to apply judgment rather than match a closed list of indicator patterns
- **AND** it covers the heuristic case of long buffered text in the input area without a follow-up response

#### Scenario: Supervisor skill instructs proactive paste-buffer recovery at launch

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it explicitly instructs the supervisor agent to perform a paste-buffer-recovery sweep proactively at launch (before any `last_seen`-based stall threshold elapses)
- **AND** it explains the rationale (long boot prompts land in paste buffers immediately on paste-aware CLIs)

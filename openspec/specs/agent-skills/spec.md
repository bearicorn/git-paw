# agent-skills Specification

## Purpose
TBD - created by archiving change skill-templates. Update Purpose after archive.
## Requirements
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
13. NOT hardcode a specific commit-MESSAGE format as a mandatory convention. The "Commit cadence" section SHALL defer message format to the host project's injected `AGENTS.md` (e.g. "follow the project's commit-message conventions; see the project's `AGENTS.md`") rather than prescribe Conventional Commits (`feat(<scope>):`, `fix(<scope>):`, …) as the required format. A Conventional-Commits prefix MAY still appear as an illustrative example, but the prose SHALL NOT state that the agent MUST use that format — message format is a per-project convention, not a git-paw-bundled-skill rule.

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

#### Scenario: Coordination skill defers commit-message format to the project AGENTS.md

- **WHEN** the embedded coordination skill's "Commit cadence" section is inspected
- **THEN** it instructs the agent to follow the host project's commit-message conventions and references the project's `AGENTS.md`
- **AND** it does NOT state that the agent MUST use a specific commit-message format (e.g. it does NOT prescribe Conventional Commits as mandatory)

### Requirement: Skill resolution order

The system SHALL provide a function `pub fn resolve(skill_name: &str) -> Result<SkillTemplate, SkillError>` that locates the skill template by name using the following order, returning the first match:

1. The user override file at `<config_dir>/git-paw/agent-skills/<skill-name>.md` where `<config_dir>` is the result of `dirs::config_dir()`
2. The embedded default for that skill name shipped in the binary

If neither the user override nor an embedded default exists for the requested name, the function SHALL return `Err(SkillError::UnknownSkill { name })`.

The returned `SkillTemplate` SHALL include a `source: Source` field indicating whether the content came from a user override or the embedded default, for diagnostic purposes.

#### Scenario: Resolve falls back to embedded when no user override exists

- **GIVEN** no user override file exists for the `coordination` skill
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::Embedded`

#### Scenario: User override is preferred over embedded default

- **GIVEN** a file at `<config_dir>/git-paw/agent-skills/coordination.md` containing the text `custom user content`
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::User`
- **AND** the template content equals `custom user content`

#### Scenario: Unknown skill name returns an error

- **WHEN** `skills::resolve("nonexistent")` is called
- **AND** no user override exists for `nonexistent`
- **AND** no embedded default exists for `nonexistent`
- **THEN** the result is `Err(SkillError::UnknownSkill { name: "nonexistent" })`

### Requirement: User override directory may be absent

The system SHALL treat a missing user override directory as a normal condition equivalent to "no override available", not as an error. Specifically:

- If `dirs::config_dir()` returns `None`, the system SHALL skip the user override lookup and proceed to the embedded default
- If the directory `<config_dir>/git-paw/agent-skills/` does not exist, the system SHALL skip the user override lookup and proceed to the embedded default
- If the specific file `<config_dir>/git-paw/agent-skills/<skill-name>.md` does not exist, the system SHALL skip the user override lookup and proceed to the embedded default

#### Scenario: Missing user config directory falls through to embedded

- **GIVEN** `dirs::config_dir()` is unable to determine a config directory
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::Embedded`

#### Scenario: Missing agent-skills subdirectory falls through to embedded

- **GIVEN** `<config_dir>/git-paw/` exists but `<config_dir>/git-paw/agent-skills/` does not
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::Embedded`

#### Scenario: Missing skill file falls through to embedded

- **GIVEN** `<config_dir>/git-paw/agent-skills/` exists but contains no `coordination.md`
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::Embedded`

### Requirement: Unreadable user override is a hard error

When a user override file exists but cannot be read (permission denied, I/O error, invalid UTF-8), the system SHALL return `Err(SkillError::UserOverrideRead { path, source })` rather than silently falling back to the embedded default. This makes misconfigured overrides visible to the user instead of hidden behind a working default.

#### Scenario: Permission denied on user override returns an error

- **GIVEN** a file at `<config_dir>/git-paw/agent-skills/coordination.md` with read permissions removed
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Err(SkillError::UserOverrideRead { .. })`
- **AND** the error message identifies the path of the unreadable file

#### Scenario: Invalid UTF-8 in user override returns an error

- **GIVEN** a file at `<config_dir>/git-paw/agent-skills/coordination.md` containing non-UTF-8 bytes
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Err(SkillError::UserOverrideRead { .. })`

### Requirement: Skill template rendering

The `render()` function SHALL accept a `project: &str` parameter and a
`test_command: Option<&str>` parameter and substitute `{{PROJECT_NAME}}` with
the project name and `{{TEST_COMMAND}}` with the supervisor's configured
test command, alongside the existing `{{BRANCH_ID}}` and `{{GIT_PAW_BROKER_URL}}`
substitutions.

The function signature SHALL be:

```rust
pub fn render(
    template: &SkillTemplate,
    branch: &str,
    broker_url: &str,
    project: &str,
    test_command: Option<&str>,
) -> String
```

When `test_command` is `Some(cmd)`, every occurrence of `{{TEST_COMMAND}}` in
the template SHALL be replaced with the string `cmd`. When `test_command` is
`None`, every occurrence of `{{TEST_COMMAND}}` SHALL be replaced with the
literal string `"(not configured)"` so the rendered prose remains readable
and the unknown-placeholder warning path is not triggered.

#### Scenario: PROJECT_NAME placeholder is substituted

- **GIVEN** a `SkillTemplate` whose content contains `paw-{{PROJECT_NAME}}`
- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119", "my-app", None)` is called
- **THEN** the resulting string contains `paw-my-app`
- **AND** the resulting string contains no `{{PROJECT_NAME}}`

#### Scenario: Both BRANCH_ID and PROJECT_NAME substituted

- **GIVEN** a template containing both `{{BRANCH_ID}}` and `{{PROJECT_NAME}}`
- **WHEN** `render(template, "feat/http-broker", "url", "git-paw", None)` is called
- **THEN** the output contains `feat-http-broker` and `git-paw`
- **AND** no `{{BRANCH_ID}}` or `{{PROJECT_NAME}}` placeholders remain

#### Scenario: TEST_COMMAND placeholder is substituted when test_command is Some

- **GIVEN** a `SkillTemplate` whose content contains `run {{TEST_COMMAND}} after merge`
- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119", "proj", Some("just check"))` is called
- **THEN** the resulting string contains `run just check after merge`
- **AND** the resulting string contains no `{{TEST_COMMAND}}`

#### Scenario: TEST_COMMAND placeholder substitutes a literal when test_command is None

- **GIVEN** a `SkillTemplate` whose content contains `run {{TEST_COMMAND}} after merge`
- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119", "proj", None)` is called
- **THEN** the resulting string contains `run (not configured) after merge`
- **AND** the resulting string contains no `{{TEST_COMMAND}}`
- **AND** no `{{TEST_COMMAND}}` placeholder warning is written to standard error

### Requirement: Unknown placeholder warning

The system SHALL detect when the rendered output contains any `{{...}}` substring that was not consumed by substitution. When such a substring is found, the system SHALL emit a warning to the standard error stream identifying the unsubstituted placeholder. The presence of such a placeholder SHALL NOT cause `render` itself to fail; the rendering completes and the warning is informational.

This protects users who add typos like `{{GIT_PAW_BROKER_URL}}` (incorrect double-curly form) to their override files.

#### Scenario: Unknown placeholder triggers a warning

- **GIVEN** a `SkillTemplate` whose content contains the literal text `url={{UNKNOWN_PLACEHOLDER}}`
- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119", "proj")` is called
- **THEN** the function returns a string still containing `{{UNKNOWN_PLACEHOLDER}}`
- **AND** a warning has been written to standard error mentioning `UNKNOWN_PLACEHOLDER`

#### Scenario: No warning when only known placeholders are present

- **GIVEN** a `SkillTemplate` whose content contains only `{{BRANCH_ID}}`, `{{PROJECT_NAME}}`, and `{{GIT_PAW_BROKER_URL}}`
- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119", "proj")` is called
- **THEN** no warning is written to standard error

### Requirement: SkillTemplate value type

The system SHALL define a public type `SkillTemplate` with at least these fields:

- `name: String` — the skill name (e.g. `"coordination"`)
- `content: String` — the unrendered template content
- `source: Source` — an enum with at least the variants `Embedded` and `User`

`SkillTemplate` SHALL derive `Debug` and `Clone`. The `Source` enum SHALL derive `Debug`, `Clone`, `Copy`, `PartialEq`, and `Eq`.

#### Scenario: SkillTemplate from embedded source has correct fields

- **WHEN** `skills::resolve("coordination")` is called with no user override
- **THEN** the returned `SkillTemplate` has `name == "coordination"`
- **AND** `source == Source::Embedded`
- **AND** `content` is non-empty

#### Scenario: SkillTemplate is cloneable

- **GIVEN** a `SkillTemplate` returned by `skills::resolve`
- **WHEN** `template.clone()` is called
- **THEN** the clone has identical `name`, `content`, and `source` fields

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

### Requirement: Supervisor skill — proactive permission-prompt handling at launch

The embedded `supervisor.md` skill SHALL include explicit guidance for the supervisor agent's initial launch-time monitoring sweep. The guidance SHALL:

1. State that immediately after attaching to the session, the supervisor SHALL inspect every coding-agent pane via `tmux capture-pane`.
2. Classify what each pane is showing into the categories:
   - `paste-buffer state` → apply the paste-buffer recovery sub-case (above).
   - `permission prompt` (e.g. `This command requires approval`, `Do you want to proceed?`) → classify the pending command and act per the safe-command policy (below).
   - `working / esc to interrupt` → leave alone, agent is doing its thing.
   - `idle / ? for shortcuts` → agent has finished or never started; investigate.
3. For panes showing a permission prompt, classify the pending command:
   - **Safe-by-pattern**: matches the existing auto-approve safe-command whitelist (`curl http://127.0.0.1:<broker_port>/...`, `cargo fmt|clippy|test|build`, `git commit`, `git push`, plus any `safe_commands` from `[supervisor.auto_approve]` in config). The supervisor SHALL select the "Yes, and don't ask again" option (typically `Down` + `Enter`) so the pattern is permanently allowed for that agent.
   - **Confined-to-worktree**: file edits / reads / `git -C <worktree>` operations within the agent's own worktree are safe; select "Yes, allow all edits" or equivalent (`Down` + `Enter`).
   - **Unknown / wider scope**: anything else SHALL escalate via `agent.question` to the dashboard, not be auto-approved.
4. State that this proactive sweep complements (does NOT replace) the existing `[supervisor.auto_approve]` background poll thread; the proactive sweep handles permissions that appear within the first few seconds of launch when the poll thread's stall threshold has not yet elapsed.

#### Scenario: Supervisor skill mentions proactive launch-time permission sweep

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or section instructing the supervisor to inspect every pane immediately after attaching (e.g. "launch-time sweep", "initial pane inspection", or equivalent)

#### Scenario: Supervisor skill enumerates the four pane categories

- **WHEN** the launch-time sweep section is inspected
- **THEN** it identifies the four pane categories: paste-buffer state, permission prompt, working, and idle
- **AND** it maps each category to a default action

#### Scenario: Supervisor skill describes the safe-command auto-approve heuristic

- **WHEN** the launch-time sweep section is inspected
- **THEN** it instructs the supervisor to recognise `curl http://127.0.0.1:` as a broker call and approve via "Yes, and don't ask again"
- **AND** it states that the "don't ask again" option is preferred so future broker calls auto-allow without further intervention
- **AND** it mentions that file edits and `git -C <worktree>` operations confined to the agent's own worktree are also safe-by-default

#### Scenario: Supervisor skill escalates unknown permission prompts

- **WHEN** the launch-time sweep section is inspected
- **THEN** it instructs the supervisor to escalate via `agent.question` (rather than auto-approve) for any permission prompt that does not match the safe-command or confined-to-worktree patterns

#### Scenario: Supervisor skill says proactive sweep complements the auto-approve thread

- **WHEN** the launch-time sweep section is inspected
- **THEN** it explicitly states that the proactive sweep complements (does NOT replace) the existing `[supervisor.auto_approve]` background poll thread
- **AND** it explains the rationale (proactive sweep covers the first-few-seconds window before the poll thread's stall threshold elapses)

### Requirement: Supervisor skill — corrected agent.verified curl example

The embedded `supervisor.md` skill's `curl` example for publishing `agent.verified` SHALL use payload field names that match the wire format defined in `broker-messages`. Specifically, the example SHALL include the substrings `verified_by` and `message` in the payload, and SHALL NOT include the substrings `target`, `result`, or `notes` (the v0.4.0 mistakes that did not match the validated wire format).

The `agent_id` field at the top level of the example SHALL be the *recipient* (the agent being verified), per the existing v0.4 convention for supervisor-originated messages. The skill text SHALL clarify this so users do not put `"supervisor"` in `agent_id` (which would route the verification to the supervisor's own inbox).

#### Scenario: Supervisor skill verified example contains correct payload fields

- **WHEN** the embedded `supervisor.md` skill is inspected
- **THEN** the `agent.verified` `curl` example contains the substring `verified_by`
- **AND** the example contains the substring `message`
- **AND** the example does NOT contain the substrings `target`, `result`, or `notes` as payload field names

#### Scenario: Supervisor skill verified example clarifies agent_id semantics

- **WHEN** the embedded `supervisor.md` skill is inspected
- **THEN** the surrounding text or comment indicates that the top-level `agent_id` is the recipient (the agent being verified), not the sender

### Requirement: Supervisor skill — corrected agent.feedback curl example

The embedded `supervisor.md` skill's `curl` example for publishing `agent.feedback` SHALL use payload field names that match the wire format defined in `broker-messages`. Specifically, the example SHALL include the substrings `from` and `errors` in the payload, with `errors` shown as a JSON array of strings, and SHALL NOT include the substrings `target` or `message` as payload field names (the v0.4.0 mistakes).

The `agent_id` field at the top level of the example SHALL be the *recipient* (the agent receiving feedback), per the existing v0.4 convention for `agent.feedback` delivery. The skill text SHALL clarify this so users do not put `"supervisor"` in `agent_id`.

#### Scenario: Supervisor skill feedback example contains correct payload fields

- **WHEN** the embedded `supervisor.md` skill is inspected
- **THEN** the `agent.feedback` `curl` example contains the substring `from`
- **AND** the example contains the substring `errors`
- **AND** the example shows `errors` as a JSON array (contains `[` and `]` brackets within the example body)
- **AND** the example does NOT contain the substring `target` as a payload field name
- **AND** the `agent.feedback` example does NOT contain `message` as a payload field name (it's a Verified-payload field, not Feedback)

#### Scenario: Supervisor skill feedback example clarifies agent_id semantics

- **WHEN** the embedded `supervisor.md` skill is inspected
- **THEN** the surrounding text or comment indicates that the top-level `agent_id` for `agent.feedback` is the recipient (the agent receiving feedback), not the sender

### Requirement: Supervisor skill prose references correct field names

The embedded `supervisor.md` skill's prose surrounding the curl examples (workflow steps, audit notes, etc.) SHALL reference payload field names that match the wire format. Specifically:

- References to publishing `agent.verified` SHALL describe the payload as containing `verified_by` (the sender, typically `"supervisor"`) and `message` (the optional summary). References SHALL NOT use `result` or `notes` as field names.
- References to publishing `agent.feedback` SHALL describe the payload as containing `from` and `errors`. References SHALL NOT use `target` or `message` (singular) as Feedback payload field names.

#### Scenario: Workflow prose references verified_by, not result/notes

- **WHEN** the embedded `supervisor.md` skill's workflow prose is inspected
- **THEN** references to the `agent.verified` payload structure use `verified_by` and/or `message`
- **AND** the workflow prose does NOT instruct setting `result:"pass"` or `notes:""` as part of the verified payload

#### Scenario: Workflow prose references errors, not message, for feedback

- **WHEN** the embedded `supervisor.md` skill's workflow prose is inspected
- **THEN** references to `agent.feedback` payload describe the `errors` field (a list of strings)
- **AND** the workflow prose does NOT instruct setting `message:"..."` as the feedback payload (the `message` field belongs to Verified, not Feedback)

### Requirement: Supervisor skill — Governance verification sub-step

The embedded `supervisor.md` skill SHALL include a "Governance verification" section (or sub-section within the existing Spec Audit Procedure) instructing the supervisor agent how to handle governance documents. The section SHALL include the following content:

1. **Activation condition** — the section's instructions apply only when the boot prompt's "Governance documents" section is present (i.e. at least one `[governance]` path is configured). When the boot-prompt section is absent, the supervisor SHALL skip governance reading entirely.
2. **Ordering** — governance reading runs as a sub-step *inside* the existing Spec Audit Procedure (step 7 in the supervisor flow), NOT as a separate flow step. The skill SHALL state this explicitly.
3. **Per-doc examples** — the section SHALL provide brief examples of what to look for per doc type (DoD walk against branch state, ADR drift detection in the diff, security checklist walk, test-strategy proportion check, constitution conformance check). The examples SHALL be illustrative, not exhaustive rubrics. The skill SHALL state that the supervisor agent applies judgment given the project's conventions.
4. **Findings flow through `agent.feedback`** — the section SHALL state that governance findings are surfaced as standard `agent.feedback` errors, mixed in with spec-audit findings. There is NO governance-specific tag prefix, NO `[governance-gate:<doc>]` token, NO separate broker variant.
5. **Missing-doc handling** — the section SHALL instruct the supervisor that a configured path with no readable file is a finding (added to the `agent.feedback` errors list) but NOT a separate failure type.
6. **No gating semantics** — the skill SHALL NOT instruct the supervisor to consult any `[governance.gates]` table, since that table does not exist. The skill SHALL NOT use the language of "gating" or "blocking on governance failures" — governance findings are audit findings, treated like any other.

The section SHALL be inserted in `supervisor.md` within or immediately after the existing Spec Audit Procedure.

#### Scenario: Supervisor skill mentions Governance verification

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains the substring `Governance verification` (or equivalent heading)
- **AND** it states that the section's instructions apply only when the boot prompt's "Governance documents" section is present

#### Scenario: Supervisor skill specifies the ordering

- **WHEN** the embedded supervisor skill's flow is inspected
- **THEN** the governance reading is described as a sub-step of the existing Spec Audit Procedure
- **AND** is NOT presented as a separate workflow step (no "step 7.5" framing)

#### Scenario: Supervisor skill provides per-doc examples

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains illustrative examples for DoD walks, ADR drift, security checklist walks, test-strategy checks, and constitution conformance
- **AND** the skill states the examples are illustrative, not exhaustive rubrics
- **AND** the skill states the supervisor agent applies judgment given the project's conventions

#### Scenario: Supervisor skill states findings flow through agent.feedback

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it states that governance findings are reported as `agent.feedback` errors (alongside other audit findings)

#### Scenario: Supervisor skill does NOT introduce governance-specific tag

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it does NOT contain the substring `[governance-gate:`
- **AND** it does NOT introduce a tag prefix or categorisation token specific to governance findings

#### Scenario: Supervisor skill does NOT reference governance gates

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it does NOT contain the substring `[governance.gates]`
- **AND** it does NOT instruct the supervisor to consult per-doc gate flags
- **AND** it does NOT use the language of "gating" or "blocking on governance failures"

#### Scenario: Supervisor skill instructs missing-doc handling

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it instructs the supervisor that a configured path pointing at a non-existent file becomes a finding in the `agent.feedback` errors list
- **AND** it does NOT treat missing files as a distinct failure type

### Requirement: Coordination skill — Spec Kit consolidated worktree behaviour

The embedded `coordination.md` skill SHALL include a "When working in a Spec Kit consolidated worktree" sub-section that activates when the agent's worktree branch begins with `phase/`. The sub-section SHALL instruct the agent to:

1. Read the ordered list of tasks provided in the boot prompt and treat them as a sequential to-do list (no parallelism within the consolidated worktree — the non-`[P]` marker in Spec Kit means tasks share files or context).
2. Work through tasks in the order given. After completing each task, flip its `- [ ]` checkbox to `- [x]` in the worktree's local `tasks.md`. The agent MAY commit the writeback alongside the task's code change or as a separate commit; the choice is the agent's.
3. Publish `agent.intent` covering the union of files for the next 1–2 tasks rather than re-publishing for every task — `valid_for_seconds` SHALL be set generously (e.g. equal to expected runtime for the remaining tasks) since the agent owns the consolidated set.
4. Publish `agent.done` (the existing `agent.artifact` with terminal status) only after every task in the listed set shows `- [x]` in `tasks.md`.

The sub-section SHALL state that for `[P]` (single-task) worktrees this guidance does not apply — `[P]` worktrees are scoped to one task and follow the standard "before/while editing" coordination pattern.

#### Scenario: Coordination skill mentions Spec Kit consolidated behaviour

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a heading or section referring to Spec Kit consolidated worktrees (or `phase/...` branches)
- **AND** it instructs the agent to work through listed tasks sequentially

#### Scenario: Coordination skill mentions tasks.md writeback

- **WHEN** the embedded coordination skill is inspected
- **THEN** it instructs the agent to flip `- [x]` in `tasks.md` per task as it completes
- **AND** it states that the writeback can be committed alongside the task's code or as a separate commit

#### Scenario: Coordination skill states agent.done timing for consolidated worktrees

- **WHEN** the embedded coordination skill is inspected
- **THEN** it instructs the agent to publish `agent.done` only after all listed tasks show `- [x]`

#### Scenario: Coordination skill clarifies that [P] worktrees follow standard pattern

- **WHEN** the embedded coordination skill is inspected
- **THEN** it clarifies that `[P]` (single-task) worktrees do not require sequential-list handling
- **AND** the standard "before/while editing" pattern applies to `[P]` worktrees

### Requirement: Supervisor skill — interactive user input

The embedded `supervisor.md` skill SHALL include a "When the user types in your pane" section instructing the supervisor agent how to handle user input that arrives while the autonomous monitoring loop is running. The section SHALL:

1. State that the supervisor pane is interactive — the user can type questions or directives at any point during the session.
2. Distinguish three cases of user input and map each to existing mechanisms:
   - **Status question** ("how's X going?", "what are the agents working on?") — answer conversationally using `curl /status`, `curl /messages/supervisor`, and `tmux capture-pane`. Do NOT publish; just respond.
   - **Directive** ("ask X to use bcrypt", "tell Y to skip the migration") — publish `agent.feedback` to the named agent (or use `tmux send-keys` for low-stakes nudges) AND confirm the action conversationally to the user.
   - **Judgment-call ask** ("should we merge feat-a before feat-b?") — apply the supervisor's normal escalation rules; only publish `agent.question` to the dashboard if the call is genuinely ambiguous beyond what the user just provided. Otherwise answer conversationally with the supervisor's reasoning.
3. State that the autonomous loop continues alongside user input — if the supervisor is mid-spec-audit when the user asks something, finish the current step, then respond, then resume the loop.
4. The mechanisms (curl, tmux capture-pane, agent.feedback, tmux send-keys, agent.question) are unchanged from the existing supervisor skill; the addition is *when to use which* in response to user input.

#### Scenario: Supervisor skill mentions interactive user input

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or section identifying user input handling (e.g. `When the user types in your pane`)

#### Scenario: Supervisor skill names the three input cases

- **WHEN** the embedded supervisor skill's user-input section is inspected
- **THEN** it identifies the status-question case and maps it to `/status` / `/messages/supervisor` / `tmux capture-pane`
- **AND** it identifies the directive case and maps it to `agent.feedback` / `tmux send-keys`
- **AND** it identifies the judgment-call case and maps it to the supervisor's normal escalation (with `agent.question` only when ambiguous)

#### Scenario: Supervisor skill states the autonomous loop continues alongside user input

- **WHEN** the embedded supervisor skill's user-input section is inspected
- **THEN** it explicitly states that the autonomous monitoring loop continues alongside user input
- **AND** it instructs the supervisor to finish the current step before responding

### Requirement: Supervisor skill — Merge orchestration

The embedded `supervisor.md` skill SHALL include a "Merge orchestration" section that replaces the v0.4 Rust `run_merge_loop` function. The section SHALL instruct the supervisor agent to perform merge orchestration via existing mechanisms (curl, shell, git, the configured `test_command`) and SHALL cover:

1. **When to merge** — once all expected agents have published `agent.verified` (or after the user explicitly asks the supervisor to merge).
2. **Compute merge order from `agent.blocked` events** — read `curl /messages/supervisor` (or the broker's message log via the dashboard's view) to find `agent.blocked` events. For each `agent.blocked` from X with `payload.from = Y`, treat as edge "X depends on Y". Topologically sort the dependency graph; agents with no incoming edges merge first.
3. **Per-branch merge loop**:
   - Checkout main: `git checkout main`.
   - Fast-forward merge: `git merge --ff-only feat/<branch>`. Never create merge commits.
   - On non-FF / conflict: SKIP the merge for this branch; publish `agent.feedback` to the branch's agent listing the conflict and asking them to rebase or resolve. Continue with the next branch.
   - On FF success: run the configured `test_command` (from `[supervisor].test_command`). On failure: `git reset --hard <previous-HEAD>` to revert; publish `agent.feedback` listing the regression. On success: continue to the next branch.
4. **Cycle handling** — if the dependency graph has cycles, publish `agent.question` to the dashboard surfacing the cycle and asking the user how to proceed. Do NOT merge any branch in the cycle until the user resolves it.
5. **Final summary** — when all eligible branches are merged (or skipped), publish a final `agent.status` from `agent_id = "supervisor"` summarising what was merged, what was skipped, and any regressions encountered.

#### Scenario: Supervisor skill mentions merge orchestration

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or section identifying merge orchestration (e.g. `Merge orchestration`)

#### Scenario: Supervisor skill names the trigger condition

- **WHEN** the embedded supervisor skill's merge-orchestration section is inspected
- **THEN** it states that merge orchestration runs when all expected agents have published `agent.verified` (or on user request)

#### Scenario: Supervisor skill describes the topological-order computation

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the supervisor to read `agent.blocked` messages
- **AND** it states that an `agent.blocked` from X with `payload.from = Y` defines a dependency edge (X depends on Y)
- **AND** it instructs the supervisor to topologically sort the resulting dependency graph

#### Scenario: Supervisor skill describes the per-branch merge + test loop

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the supervisor to use `git merge --ff-only`
- **AND** it instructs the supervisor to run the configured `test_command` after each successful merge
- **AND** it instructs the supervisor to revert (`git reset --hard`) and publish `agent.feedback` on test failure
- **AND** it instructs the supervisor to skip and publish `agent.feedback` on merge conflict / non-FF

#### Scenario: Supervisor skill describes cycle handling

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the supervisor to escalate via `agent.question` when the dependency graph has cycles
- **AND** it instructs the supervisor NOT to merge cycle members until the user resolves the cycle

#### Scenario: Supervisor skill describes the final summary

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the supervisor to publish a final `agent.status` summary after merge orchestration completes
- **AND** the summary covers what was merged, what was skipped, and any regressions

### Requirement: Supervisor skill — five verification gates

The embedded `supervisor.md` skill SHALL enumerate the supervisor agent's verification sub-flow as **five** explicit first-class gates, run in the order below before any `agent.verified` message is published for a coding-agent branch. The Workflow section's steps 4-7 (or equivalent) SHALL be restructured so each gate has its own heading and prose. Findings from any gate flow through `agent.feedback`; the `errors` array entries SHALL each begin with a bracketed gate-name prefix (e.g. `[doc audit]`, `[security audit]`) so the recipient agent can route fixes to the right gate.

The five gates SHALL be, in order:

1. **Testing** — the supervisor SHALL run the configured `{{TEST_COMMAND}}` inside the agent's worktree and capture full output. Failures here block all downstream gates.

2. **Regression analysis** — the supervisor SHALL diff the testing output against the baseline (the test outcome on `main` recorded before any agent was launched). Previously-passing tests that now fail SHALL be reported as regressions. Pure additions (new tests that did not exist on baseline) are not regressions.

3. **Spec audit** — for each `### Requirement:` and `#### Scenario:` block under `openspec/changes/<change>/specs/`, the supervisor SHALL verify (a) the implementation contains the SHALL/MUST behaviour the requirement describes, (b) at least one test exercises each scenario's WHEN/THEN. Gaps SHALL be reported as `[spec audit] <requirement-name>: <gap description>` errors.

4. **Doc audit** — the supervisor SHALL verify the documentation surfaces named in the change's `Impact` section have been updated. The doc surfaces in scope are: mdBook chapters under `docs/src/`, top-level `README.md`, `AGENTS.md`, the relevant `--help` text accessed via the binary, and rustdoc on changed public items. The existing governance-verification sub-step (DoD, ADRs, security checklist, test strategy, constitution docs) remains in scope as an **input source** for this gate — its findings are doc-audit findings tagged `[doc audit]`. Doc-audit gaps SHALL be reported as `[doc audit] <surface>: <gap description>` errors.

5. **Security audit** — the supervisor SHALL review the diff for the OWASP-relevant patterns called out in the project's `CLAUDE.md` (command injection, XSS, SQL injection, path traversal, unvalidated external input flowing into `Command::new(...)` or filesystem writes, secret leakage in logs/error messages) and for any new `unwrap()` / `expect()` calls outside test code (project-wide rule, also from `CLAUDE.md`). On doc/text-only changes this gate is normally a fast noop. Findings SHALL be reported as `[security audit] <category>: <issue>` errors.

The Workflow's `Verify or feedback` step (currently §7) SHALL be updated so the published `agent.verified` message's `message` field enumerates the outcome of all five gates (e.g. `"all five gates clean: testing OK, no regressions, spec audit clean, doc audit clean, security audit clean"`). On any gate failing, `agent.feedback` SHALL be published instead, with each error entry tagged by gate name as described above.

The existing governance-verification sub-step in the Spec Audit Procedure section is preserved and explicitly referenced as a doc-audit input source — it is not deleted, and the per-doc examples (DoD, ADRs, security.md, test-strategy.md, constitution.md) remain valuable guidance.

#### Scenario: Supervisor skill enumerates five verification gates

- **WHEN** the embedded supervisor skill's Workflow section is inspected
- **THEN** it lists exactly five first-class verification gates in this order: Testing, Regression analysis, Spec audit, Doc audit, Security audit
- **AND** each gate has its own heading or sub-section (not buried inside another gate)

#### Scenario: Supervisor skill specifies gate-name prefixes in feedback errors

- **WHEN** the embedded supervisor skill's `agent.feedback` example or guidance is inspected
- **THEN** the example or prose makes clear that errors in the `errors` array begin with a bracketed gate-name prefix (e.g. `[doc audit]`, `[security audit]`, `[spec audit]`, `[regression]`, `[testing]`)

#### Scenario: Supervisor skill defines the doc-audit surfaces

- **WHEN** the embedded supervisor skill's Doc audit gate is inspected
- **THEN** it enumerates the doc surfaces in scope: mdBook chapters under `docs/src/`, `README.md`, `AGENTS.md`, `--help` text, and rustdoc on changed public items
- **AND** it cross-references the change's `Impact` section as the authoritative driver of which surfaces apply per audit

#### Scenario: Supervisor skill defines the security-audit categories

- **WHEN** the embedded supervisor skill's Security audit gate is inspected
- **THEN** it enumerates the OWASP categories from `CLAUDE.md`: command injection, XSS, SQL injection, path traversal, unvalidated external input, secret leakage in logs/errors
- **AND** it also calls out the project-wide rule against new `unwrap()` / `expect()` outside test code

#### Scenario: Supervisor skill preserves the governance-verification sub-step as a doc-audit input source

- **WHEN** the embedded supervisor skill is inspected after this requirement is implemented
- **THEN** the existing governance-verification sub-step (with DoD, ADR, security.md, test-strategy.md, constitution.md examples) is still present
- **AND** the prose explicitly cross-references it from the Doc audit gate as an input source

#### Scenario: Supervisor skill's verified-message enumerates all five gates

- **WHEN** the embedded supervisor skill's `agent.verified` example or guidance for the message-field content is inspected
- **THEN** the example or prose instructs the supervisor to enumerate the outcomes of all five gates in the message field (not only "tests pass" or "spec audit clean")

### Requirement: Supervisor skill — continuous-sweep absorption doctrine

The embedded `supervisor.md` skill SHALL codify that absorbing routine inner-agent permission prompts is the supervisor agent's continuous responsibility, not a launch-time-only activity. Specifically:

1. The skill's "Workflow" section's `Watch` step (currently §2) SHALL state that on every monitoring-loop iteration, the supervisor agent applies the launch-time safe-command policy (currently §1.5) to every coding-agent pane — not only at attach time. The wording SHALL make clear that the proactive sweep runs on every iteration, while the reactive `[supervisor.auto_approve]` background poll thread remains a fallback for when the supervisor agent is offline or its iteration cadence is too slow.

2. The skill's "Rules" section SHALL contain a bullet stating that the supervisor agent absorbs routine approvals as part of its job, and that the human is the escalation audience for non-routine decisions. The bullet SHALL enumerate routine cases (dev-essential prompts: `git commit`, `cargo test|build|fmt|clippy`, `mdbook build`, `git stash`, `git restore`, common shell reads like `awk`, `grep`, `python3 -c '...'`) and non-routine cases (cross-agent conflicts requiring design judgement, scope/spec decisions, destructive operations outside an agent's own worktree, anything novel or surprising). The framing SHALL be that the supervisor is the rubber-stamp gate and the human is only invoked when judgement is required.

3. The skill SHALL NOT delete the existing launch-time sweep guidance (§1.5) or the `[supervisor.auto_approve]` poll-thread description; the continuous-iteration guidance complements both. The intent is that all three mechanisms (launch sweep, continuous sweep, reactive poll thread) cover the full lifecycle: launch sweep handles the first-few-seconds window before the supervisor's monitoring loop starts iterating, the continuous sweep covers steady-state operation, and the poll thread is a fallback when the supervisor agent itself is unavailable.

#### Scenario: Supervisor skill instructs continuous-iteration safe-command sweep

- **WHEN** the embedded supervisor skill's `Watch` step (or equivalent monitoring-loop section) is inspected
- **THEN** it explicitly instructs the supervisor agent to sweep every coding-agent pane on every iteration and apply the safe-command policy from the launch-time sweep section
- **AND** the wording SHALL distinguish the continuous sweep from the launch-time sweep and the reactive poll thread (all three coexist; the continuous sweep is the steady-state mechanism)

#### Scenario: Supervisor skill Rules section codifies routine-approval absorption

- **WHEN** the embedded supervisor skill's `Rules` section is inspected
- **THEN** it contains a bullet stating that the supervisor agent absorbs routine approvals and the human is the escalation audience
- **AND** the bullet enumerates routine dev-essential prompt categories (cargo / git / mdbook / shell-read families)
- **AND** the bullet enumerates non-routine cases that SHALL be escalated (cross-agent conflicts, scope/spec decisions, destructive ops outside a worktree, anything novel)

#### Scenario: Supervisor skill keeps the existing launch-time sweep and poll-thread guidance intact

- **WHEN** the embedded supervisor skill is inspected after this requirement is implemented
- **THEN** the existing §1.5 launch-time pane sweep guidance is still present
- **AND** the existing `[supervisor.auto_approve]` background poll thread description is still present
- **AND** the new continuous-sweep guidance complements (does NOT replace) either of the above

### Requirement: Coordination skill SHALL teach per-group commit cadence

The embedded `assets/agent-skills/coordination.md` skill SHALL contain a section (heading text approximately "Commit cadence" or "Per-group commit cadence") that instructs the coding agent to commit after completing each numbered task group (e.g. `## 1.`, `## 2.`) in the change's `tasks.md`. The section SHALL state:

1. The default unit of commit is the task GROUP, not the individual task. After all `- [ ]` items in a group are `- [x]`, the agent SHALL commit before starting the next group.
2. The agent SHALL NOT accumulate more than approximately ten uncommitted files at a time. If a single group's implementation produces more uncommitted files than that, the agent SHALL split into multiple commits using suffixes like `(part 1 of 2)`.
3. The commit-MESSAGE format SHALL defer ENTIRELY to the host project's injected `AGENTS.md` rather than be mandated, defaulted, or illustrated by the bundled skill. The section SHALL instruct the agent to follow the project's own commit-message conventions (e.g. "follow the project's commit-message conventions; see the project's `AGENTS.md`"). The bundled skill SHALL NOT present a Conventional-Commits prefix (`feat(<scope>):`, `fix(<scope>):`, …) as git-paw's example, default, or recommendation — Conventional Commits is git-paw's OWN repo convention (it belongs in git-paw's `AGENTS.md`, not in the asset the binary exports to every consumer). Any commit example the section needs (e.g. to demonstrate the `(part N of M)` split mechanism) SHALL use a FORMAT-NEUTRAL subject with no convention-specific prefix. This is the bundled-skill side of the "what the binary exports vs what is git-paw-repo-specific" separation, and keeps the requirement consistent with the "Embedded coordination skill" requirement (item 13) and the `lang-agnostic-skills` convention-neutrality audit.
4. The rationale: per-group commits protect against agent crashes, conflict mediation, and `/clear` resets losing unbounded work; they also map cleanly to the post-commit hook's `agent.artifact{status:committed}` event sequence the supervisor uses for verification.

#### Scenario: Coordination skill names the per-group cadence

- **WHEN** the embedded `coordination.md` skill is inspected
- **THEN** the content SHALL contain a heading naming the commit-cadence concept (e.g. "Commit cadence", "Per-group commit cadence", or substantively equivalent)
- **AND** the section's body SHALL mention the GROUP grain (i.e. the substring "group" or "section" appears at least once)
- **AND** SHALL name the ~10-file soft cap on uncommitted work

#### Scenario: Coordination skill defers commit-message format to the project AGENTS.md

- **WHEN** the commit-cadence section is inspected
- **THEN** it SHALL instruct the agent to follow the host project's commit-message conventions and SHALL reference the project's `AGENTS.md` as the source of the format rules
- **AND** it SHALL NOT state that the agent MUST use a specific commit-message format (it SHALL NOT prescribe Conventional Commits as the mandatory format)
- **AND** it SHALL NOT present a Conventional-Commits prefix (e.g. `feat(<scope>):`) as git-paw's example, default, or recommendation — any commit example shown SHALL use a format-neutral subject with no convention-specific prefix

#### Scenario: Per-group cadence and releasable-unit discipline remain unchanged

- **WHEN** the commit-cadence section is inspected
- **THEN** it SHALL still instruct the agent to commit per task group with the approximately-ten-uncommitted-file soft cap and `(part N of M)` split guidance
- **AND** the releasable-unit / `git commit --amend` fixup discipline (defined by the "Coordination skill — releasable-unit commit discipline with amend fixups" requirement) SHALL remain present and unaffected by this change

### Requirement: Coordination skill SHALL forbid the coding agent from invoking `/opsx:verify` and `/opsx:archive`

The embedded `coordination.md` skill SHALL contain a section explaining that the coding agent's terminal action is `agent.artifact { status: "done" }` (or the implicit `committed` event auto-published by the post-commit hook). The section SHALL explicitly state that the coding agent SHALL NOT invoke `/opsx:verify <change-id>` or `/opsx:archive <change-id>`, naming both skill names so the rule is unambiguous.

The rationale SHALL be stated:

1. Verification is the supervisor's responsibility (the five-gate framework codified in `supervisor-as-pane-followups`).
2. Archiving happens during the supervisor's cherry-pick + merge flow on the release branch (per the AGENTS.md release procedure), NOT on the agent's feature branch.

#### Scenario: Coordination skill explicitly names `/opsx:verify` and `/opsx:archive` as off-limits

- **WHEN** the embedded `coordination.md` skill is inspected
- **THEN** the content SHALL contain the literal substring `/opsx:verify`
- **AND** the literal substring `/opsx:archive`
- **AND** prose stating both are NOT the coding agent's responsibility (e.g. "do not invoke", "off-limits", "supervisor's job", or substantively equivalent)

#### Scenario: Coordination skill names the terminal action

- **WHEN** the terminal-action section is inspected
- **THEN** the content SHALL mention `agent.artifact` with `status: "done"` OR `status: "committed"` as the coding agent's final wire-format publish

### Requirement: Supervisor skill SHALL teach `pane_current_path` for pane→agent resolution

The embedded `assets/agent-skills/supervisor.md` skill SHALL contain a section (heading text approximately "Resolve pane to agent" or "Pane→agent mapping") that teaches the supervisor agent the canonical resolution command:

```bash
tmux display-message -t paw-{{PROJECT_NAME}}:0.<pane> -p '#{pane_current_path}'
```

The section SHALL warn explicitly that:

1. Pane indices are NOT sorted alphabetically by `agent_id`.
2. Pane indices are NOT in the CLI-argument order from `git paw start --specs A B C`.
3. The mapping SHALL NOT be inferred from `git paw status` output (which is sorted alphabetically by the broker) or from the dashboard's row order (same).

The output's basename ends in `<project>-feat-<branch>`, giving the authoritative `agent_id`. The supervisor agent SHOULD cache the mapping once per session.

#### Scenario: Supervisor skill names the `pane_current_path` resolution command

- **WHEN** the embedded supervisor skill is inspected
- **THEN** the content SHALL contain the literal substring `tmux display-message` and `pane_current_path`
- **AND** the section SHALL warn against assuming pane order from `agent_id` alphabetical or from CLI-argument order

#### Scenario: Supervisor skill warns against `git paw status` ordering as a mapping source

- **WHEN** the pane-resolution section is inspected
- **THEN** the section SHALL contain prose explicitly noting that `git paw status` ordering or dashboard row order SHALL NOT be used as the pane→agent mapping source

### Requirement: Skill template SHALL substitute six gate-command placeholders

`src/skills.rs::render` SHALL substitute the following placeholder strings in the supervisor skill template based on `[supervisor]` config values:

| Placeholder                  | Source field                              | When `None`         |
| ---------------------------- | ----------------------------------------- | ------------------- |
| `{{TEST_COMMAND}}`           | `[supervisor].test_command`               | `(not configured)`  |
| `{{LINT_COMMAND}}`           | `[supervisor].lint_command`               | `(not configured)`  |
| `{{BUILD_COMMAND}}`          | `[supervisor].build_command`              | `(not configured)`  |
| `{{DOC_BUILD_COMMAND}}`      | `[supervisor].doc_build_command`          | `(not configured)`  |
| `{{SPEC_VALIDATE_COMMAND}}`  | `[supervisor].spec_validate_command`      | `(not configured)`  |
| `{{FMT_CHECK_COMMAND}}`      | `[supervisor].fmt_check_command`          | `(not configured)`  |
| `{{SECURITY_AUDIT_COMMAND}}` | `[supervisor].security_audit_command`     | `(not configured)`  |

The substitution SHALL be plain string replacement (every occurrence of the placeholder in the template is replaced with the rendered value).

`{{CHANGE_ID}}` is NOT substituted by `render` — it passes through into the rendered skill verbatim. The supervisor agent SHALL substitute it at verification time using the change name it is currently auditing.

#### Scenario: Test command substitution with Some(value)

- **GIVEN** a skill template containing the literal string `Run {{TEST_COMMAND}} on the worktree.`
- **AND** a `SupervisorConfig` with `test_command = Some("just check")`
- **WHEN** `render` is called
- **THEN** the rendered output SHALL contain `Run just check on the worktree.`
- **AND** SHALL NOT contain the literal `{{TEST_COMMAND}}`

#### Scenario: Test command substitution with None

- **GIVEN** the same template
- **AND** a `SupervisorConfig` with `test_command = None`
- **WHEN** `render` is called
- **THEN** the rendered output SHALL contain `Run (not configured) on the worktree.`

#### Scenario: All six new placeholders are substituted

- **GIVEN** a template containing each of the six new placeholders (`{{LINT_COMMAND}}`, `{{BUILD_COMMAND}}`, `{{DOC_BUILD_COMMAND}}`, `{{SPEC_VALIDATE_COMMAND}}`, `{{FMT_CHECK_COMMAND}}`, `{{SECURITY_AUDIT_COMMAND}}`) at least once
- **AND** a `SupervisorConfig` with corresponding fields set to `Some("CMD-N")` for each
- **WHEN** `render` is called
- **THEN** every placeholder SHALL be replaced with its corresponding `CMD-N` value
- **AND** the rendered output SHALL NOT contain any of the original `{{...}}` placeholder strings

#### Scenario: CHANGE_ID placeholder passes through unrendered

- **GIVEN** a `spec_validate_command = Some("openspec validate {{CHANGE_ID}} --strict")` in `SupervisorConfig`
- **AND** a template containing `Run {{SPEC_VALIDATE_COMMAND}} for the change being audited.`
- **WHEN** `render` is called
- **THEN** the rendered output SHALL contain `Run openspec validate {{CHANGE_ID}} --strict for the change being audited.`
- **AND** the `{{CHANGE_ID}}` substring SHALL still appear verbatim in the rendered output (it is NOT a render-time placeholder)

### Requirement: Supervisor skill prose SHALL use placeholders, not hardcoded commands

The embedded `assets/agent-skills/supervisor.md` SHALL reference gate commands exclusively via the placeholder names listed above, not via the hardcoded `just`, `cargo`, `mdbook`, or `openspec` invocations. The gate prose SHALL state explicitly that when a placeholder renders as `(not configured)`, the agent SHALL skip the tooling invocation but the gate's manual review (e.g. spec scenario coverage check, OWASP-category diff scan) still applies.

#### Scenario: Rendered skill uses placeholders for all five gates

- **GIVEN** the embedded supervisor skill rendered with all six new `[supervisor]` fields set to `Some("CMD-N")`
- **WHEN** the rendered content is inspected
- **THEN** the content SHALL contain a `Gate 1`/`Testing` section that uses `CMD-N` (the substituted `{{TEST_COMMAND}}`)
- **AND** the content SHALL contain a `Doc audit` section that uses the substituted `{{DOC_BUILD_COMMAND}}`
- **AND** the content SHALL contain a `Security audit` section that uses the substituted `{{SECURITY_AUDIT_COMMAND}}`
- **AND** the content SHALL NOT contain a hardcoded `just check`, `mdbook build`, `cargo audit`, `cargo clippy`, `openspec validate`, or `cargo fmt --check` string outside of placeholder context

#### Scenario: Not-configured gates render as graceful skip prose

- **GIVEN** the embedded supervisor skill rendered with all six new fields set to `None`
- **WHEN** the rendered content is inspected
- **THEN** the content SHALL contain the literal phrase `(not configured)` in each of the five gate sections
- **AND** the content SHALL contain prose stating that gates whose commands render as `(not configured)` SHALL skip the tooling invocation but still apply manual review

#### Scenario: Hardcoded git-paw command audit

- **WHEN** the embedded supervisor skill template (pre-render) is read from `assets/agent-skills/supervisor.md`
- **THEN** no occurrence of `just check`, `cargo test`, `cargo clippy`, `cargo audit`, `cargo fmt --check`, `mdbook build` SHALL appear in the gate prose (those literal strings only appear inside the `{{...}}` placeholder definitions or in fenced code blocks demonstrating EXAMPLE values like `# [supervisor]\n# test_command = "just check"`)

### Requirement: `git paw init` SHALL install the bundled sweep helper

`src/init.rs::run_init` SHALL write the embedded `assets/scripts/sweep.sh` (referenced via `include_str!`) to `<repo>/.git-paw/scripts/sweep.sh` and set executable permissions (`0o755` on Unix). The write SHALL be additive: if the file already exists, `git paw init` overwrites it with the bundled version.

The script SHALL be generalized — no hardcoded session name, repo parent path, broker port, or test command. The script SHALL:

- Read the session name from `<repo>/.git-paw/sessions/*.json` (the most recently modified entry when multiple exist).
- Read the broker URL from `<repo>/.git-paw/config.toml` `[broker].port` (default 9119), constructing `http://127.0.0.1:<port>`.
- Read the test command from `<repo>/.git-paw/config.toml` `[supervisor].test_command`. When unset, commands that depend on the test command SHALL no-op gracefully with a message.
- Detect the project root via `git rev-parse --show-toplevel` from inside the script's cwd.

#### Scenario: `git paw init` writes the sweep helper

- **GIVEN** a fresh git repository with no `.git-paw/` directory
- **WHEN** `git paw init` is invoked
- **THEN** the file `<repo>/.git-paw/scripts/sweep.sh` SHALL exist
- **AND** the file SHALL be executable (mode `0o755` on Unix)
- **AND** the file content SHALL be byte-identical to the embedded `assets/scripts/sweep.sh`

#### Scenario: `git paw init` overwrites an existing sweep.sh

- **GIVEN** a repo where `.git-paw/scripts/sweep.sh` already exists with modified content
- **WHEN** `git paw init` is invoked again
- **THEN** the file SHALL be overwritten with the bundled embedded content
- **AND** the file SHALL remain executable

#### Scenario: sweep.sh reads session name from session JSON

- **GIVEN** `<repo>/.git-paw/sessions/paw-myproject.json` exists with `session_name: "paw-myproject"`
- **WHEN** `.git-paw/scripts/sweep.sh status` is invoked from the repo root
- **THEN** the script SHALL query `tmux` against session `paw-myproject` (not the hardcoded `paw-git-paw`)

#### Scenario: sweep.sh reads broker port from config

- **GIVEN** `<repo>/.git-paw/config.toml` contains `[broker]\nport = 9200`
- **WHEN** `.git-paw/scripts/sweep.sh status` is invoked
- **THEN** the script SHALL curl `http://127.0.0.1:9200/status` (not the hardcoded 9119)

#### Scenario: sweep.sh status filters phantom agents

- **GIVEN** the broker `/status` returns agents `[supervisor, feat-x, a, <agent-id>]`
- **WHEN** `.git-paw/scripts/sweep.sh status` is invoked
- **THEN** the rendered output SHALL include rows only for `supervisor` and `feat-x`
- **AND** the output SHALL include a trailing line naming the filtered phantoms (e.g. `phantoms (use --all to show): a, <agent-id>`)
- **AND** invoking `.git-paw/scripts/sweep.sh status --all` SHALL include every agent in the rendered output and SHALL NOT print the phantoms summary line

### Requirement: Supervisor skill SHALL reference the bundled sweep helper

The embedded `assets/agent-skills/supervisor.md` skill SHALL invoke `.git-paw/scripts/sweep.sh <subcommand>` for the operations covered by the helper instead of raw tmux + curl pipelines. The supervisor pane's cwd is the repo root by construction, so the relative path resolves directly. The subcommands taught by the skill SHALL include at minimum: `snapshot`, `capture <pane>`, `approve <pane>`, `status`, `worktrees-status`, `inbox`, `feedback-gate <agent> <gate> <msg>`, `verified <agent> <msg>`, `status-publish <msg>`.

The skill MAY retain a single curl example for the supervisor's initial `agent.status` self-registration, because the script's session-name discovery depends on the session JSON existing — which it does not on the very first publish in a fresh session. All subsequent broker interactions in the skill SHALL go through the helper.

The skill SHALL NOT contain `for p in ... ; do tmux capture-pane ...` style loops over pane indices. Those loops trip Claude CLI per-pattern approval prompts on every sweep iteration and defeat the helper's purpose.

#### Scenario: Rendered supervisor skill references sweep.sh

- **WHEN** the embedded supervisor skill is inspected
- **THEN** every example invoking `tmux capture-pane` across multiple panes SHALL use `.git-paw/scripts/sweep.sh snapshot` or `.git-paw/scripts/sweep.sh capture <pane>` instead
- **AND** every example publishing `agent.verified`, `agent.feedback`, or `agent.status` SHALL use the corresponding `sweep.sh <subcommand>` form
- **AND** no `for p in <list>; do tmux capture-pane ...` loop SHALL appear in the rendered skill content

#### Scenario: Rendered supervisor skill does not contain phantom-prone curl placeholders

- **WHEN** the embedded supervisor skill is inspected
- **THEN** the skill SHALL NOT contain the literal string `<agent-id>` or `<your question>` or `<your specific question>` inside any documented curl payload `agent_id` or payload-text field
- **AND** any remaining placeholder syntax in examples SHALL use clearly-broken forms like `__FILL_IN__` so accidental submission produces an obvious error rather than phantom agents

### Requirement: Skill-content tests SHALL cover all rendered subsections named in archived prompt-submit-fix scenarios

Three new tests in `src/skills.rs::tests` SHALL assert behaviour of the rendered supervisor skill content that the `prompt-submit-fix` archive's `agent-skills/spec.md` scenarios required but did not cover with dedicated tests:

1. **Launch-time-sweep proactive instruction** — assert the rendered skill contains prose instructing the supervisor agent to inspect every pane immediately after attaching (i.e. before any stall threshold elapses).
2. **Escalation via `agent.question` for unknown prompts** — assert the launch-sweep section instructs escalation via `agent.question` for permission prompts that do not match the safe-command or confined-to-worktree patterns.
3. **"Complements not replaces" cross-reference** — assert the launch-sweep section explicitly states the proactive sweep complements (does NOT replace) the `[supervisor.auto_approve]` background poll thread.

#### Scenario: Launch-sweep proactive instruction test exists

- **WHEN** the test module in `src/skills.rs` is inspected
- **THEN** a behavioural test (e.g. `supervisor_skill_documents_proactive_launch_sweep`) SHALL exist
- **AND** SHALL assert the rendered supervisor skill content contains prose tied to the first-few-seconds-after-attach window

#### Scenario: Unknown-prompt escalation test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist asserting the rendered skill instructs `agent.question` escalation for unknown permission prompts

#### Scenario: Complements-not-replaces cross-reference test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist asserting the rendered skill contains "complements" / "does NOT replace" language tying the launch sweep to the `[supervisor.auto_approve]` poll thread

### Requirement: Dashboard input-handling SHALL be tested for the supervisor-as-pane removed-inbox scenarios

Three new unit tests in `src/dashboard.rs::tests` SHALL cover dashboard input-handling scenarios that the `supervisor-as-pane-followups` archive's `dashboard/spec.md` required:

1. **Tab key is ignored** — pressing `KeyCode::Tab` SHALL NOT alter any state.
2. **Printable characters do not enter a buffer** — `KeyCode::Char('a')` and `KeyCode::Char(' ')` SHALL leave no buffer state behind.
3. **Layout collapses to non-inbox shape when `show_message_log = false`** — the layout-builder helper's Vec<Constraint> SHALL be exactly `[title, table, status]` (3 chunks, no prompts/input chunks).

#### Scenario: Tab-key-ignored test exists

- **WHEN** the test module in `src/dashboard.rs` is inspected
- **THEN** a behavioural test SHALL exist asserting `KeyCode::Tab` does not alter any dashboard state

#### Scenario: Printable-char-ignored test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist asserting `KeyCode::Char('a')` and space leave no buffer state

#### Scenario: Layout-collapse test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist asserting the layout chunks are exactly `[title, table, status]` when `show_message_log = false`

### Requirement: Source-audit tests SHALL cover cmd_supervisor non-self-publish and dashboard no-phantom-row

Two new tests in `tests/source_audit.rs` SHALL close the archived `supervisor-as-pane-followups` scenarios:

1. **`cmd_supervisor` does not self-publish** — grep `src/main.rs::cmd_supervisor`'s body for `publish_to_broker_http` AND `build_status_message("supervisor"`; assert zero matches inside the function.
2. **Dashboard renders no supervisor row pre-bootstrap** — fixture-driven: render `Snapshot { agents: vec![] }` (i.e. the broker has no entries because the supervisor pane hasn't published yet); assert the output contains no `supervisor` substring and no divider row.

#### Scenario: cmd_supervisor source-audit test exists

- **WHEN** `tests/source_audit.rs` is inspected
- **THEN** a behavioural test SHALL exist greping `cmd_supervisor` for `publish_to_broker_http` and `build_status_message("supervisor"` substrings
- **AND** asserting both grep counts are zero

#### Scenario: No-phantom-row test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist rendering an empty-agents snapshot and asserting the output contains no `supervisor` substring

### Requirement: SpecEntry backend-tag tests SHALL cover scan() returns

Six new tests SHALL assert that the `SpecBackend::scan()` implementations populate `SpecEntry.backend` correctly per the archived `openspec-apply-boot-prompt` change:

- **OpenSpec backend (3 tests in `src/specs/openspec.rs::tests`):**
  - Single-entry scan returns `backend == SpecBackendKind::OpenSpec`.
  - Multi-entry scan: every returned entry has `backend == SpecBackendKind::OpenSpec`.
  - Backend tag is independent of `paw_cli` or `owned_files` frontmatter.
- **Markdown backend (3 tests in `src/specs/markdown.rs::tests`):**
  - Single-entry scan returns `backend == SpecBackendKind::Markdown`.
  - Multi-entry scan: every returned entry has `backend == SpecBackendKind::Markdown`.
  - Backend tag is applied AFTER filtering out non-pending entries.

#### Scenario: OpenSpec scan tags every entry with the OpenSpec backend

- **WHEN** the test module in `src/specs/openspec.rs` is inspected
- **THEN** a behavioural test SHALL exist that calls `scan()` on a fixture with at least 2 changes and asserts every returned `SpecEntry` has `backend == SpecBackendKind::OpenSpec`

#### Scenario: Markdown scan tags every entry with the Markdown backend

- **WHEN** the test module in `src/specs/markdown.rs` is inspected
- **THEN** a behavioural test SHALL exist that calls `scan()` on a fixture with at least 2 pending entries and asserts every returned `SpecEntry` has `backend == SpecBackendKind::Markdown`

### Requirement: BrokerMessage envelope tests SHALL cover the seven-variant enumeration and question-no-from-field absence

Two new tests in `src/broker/messages.rs::tests` SHALL close the archived `spec-corrections-v0-5-0` scenarios:

1. **Envelope enumerates all seven wire-format type values** — iterate the seven `BrokerMessage` variants, serialize each, assert the JSON's `"type"` field equals the spec'd discriminator string (`agent.status`, `agent.artifact`, `agent.blocked`, `agent.verified`, `agent.feedback`, `agent.question`, `agent.intent`).
2. **`QuestionPayload` omits the `from` field** — serialize a `QuestionPayload` instance, assert the resulting JSON does NOT contain a `"from"` key.

#### Scenario: Envelope-enumerates-seven test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist that serializes each of the seven `BrokerMessage` variants and asserts the `"type"` field matches the corresponding spec'd discriminator string

#### Scenario: Question-no-from-field test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist that serializes a `QuestionPayload` and asserts the resulting JSON does NOT contain a `"from"` key

### Requirement: Skill-content tests SHALL cover paste-buffer cross-ref and git-paw-status-ordering warning

Two new tests in `src/skills.rs::tests` SHALL close the archived `coordination-skill-followups` and `coordination-skill-followups-2` scenarios:

1. **Paste-buffer cross-ref in send-keys-alongside-feedback section** — assert the rendered supervisor skill's tmux-send-keys-alongside-`agent.feedback` section cross-references paste-buffer recovery for long answers.
2. **`git paw status` ordering warning in pane-resolution section** — assert the rendered supervisor skill's `pane_current_path` section contains the substring `git paw status` AND prose forbidding using its alphabetical order as a pane→agent mapping source.

#### Scenario: Paste-buffer cross-ref test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist asserting the rendered supervisor skill's send-keys-alongside-feedback section mentions paste-buffer recovery

#### Scenario: git-paw-status warning test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist asserting the rendered supervisor skill's pane-resolution section contains `git paw status` substring AND prose forbidding using its order as a mapping source

### Requirement: `config-test-isolation`'s "None preserves" scenario SHALL be annotated as a documented exception

`tests/config_integration.rs` (or `src/config.rs::tests`, whichever owns `load_config` test fixtures) SHALL contain a doc-comment block explaining that the "None preserves platform-default user-config resolution" scenario from `config-test-isolation`'s archived spec has no dedicated test BY DESIGN. The block SHALL state the rationale: a dedicated test would either pollute the dev machine's real config directory or require brittle process-global env-var manipulation. The block SHALL note that the scenario is exercised behaviourally by every existing production call site (which all pass `None`).

The doc comment SHALL be discoverable via grep for the scenario name (`None preserves platform-default`) so future audits find it.

#### Scenario: Documented-exception comment exists

- **WHEN** the test file(s) for `load_config` are inspected
- **THEN** a doc-comment block SHALL exist mentioning the literal substring `None preserves platform-default`
- **AND** the comment SHALL explain why a dedicated test is intentionally omitted

### Requirement: Coordination skill — stand-by after final commit

The embedded `coordination.md` skill SHALL include a positive "stand by after your
final commit" protocol that tells the coding agent what to do *instead* of reaching
for verification or archive once its work is committed. This complements the existing
`opsx-role-gating` forbidden-commands block (which forbids `/opsx:verify` and
`/opsx:archive` from a coding-agent worktree) by supplying the actionable next step.
The protocol SHALL:

1. Instruct the agent that after its final commit it SHALL **stand by**: rely on the
   automatic `agent.artifact { status: "committed" }` publish (or publish a manual
   `agent.artifact { status: "done" }` for code-less work) and then **wait**.
2. State that while standing by the agent SHALL NOT run `/opsx:verify` or
   `/opsx:archive` — these are supervisor-only — and SHALL cross-reference the
   role-gating guidance rather than restate the full forbidden-commands list.
3. State what the agent waits *for*: an `agent.verified`, `agent.feedback`, or
   further `agent.intent` from the supervisor; on `agent.feedback` the agent fixes the
   listed errors and re-publishes `agent.artifact`.

#### Scenario: Coordination skill instructs standing by after the final commit

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains explicit guidance to stand by (wait) after the final commit rather than proceed to verification or archive
- **AND** it states that the agent waits for `agent.verified`, `agent.feedback`, or further `agent.intent` from the supervisor

#### Scenario: Stand-by protocol forbids self-verify and self-archive

- **WHEN** the stand-by guidance is inspected
- **THEN** it states the agent SHALL NOT run `/opsx:verify` or `/opsx:archive` while standing by
- **AND** it cross-references the supervisor-only / role-gating guidance rather than re-deriving its own enforcement

### Requirement: Coordination skill — releasable-unit commit discipline with amend fixups

The embedded `coordination.md` skill's "Commit cadence" section SHALL teach
releasable-unit commit discipline so agents stop producing micro-commit noise that the
supervisor must hand-squash at release. The section SHALL:

1. State that each commit MUST build and pass its own gates on its own — a commit is a
   releasable unit, not a checkpoint of in-progress work.
2. State that a small follow-up to the commit the agent *just* made (one that has not
   yet been verified and that the agent has not moved past) SHOULD be folded into that
   commit with `git commit --amend` rather than landed as a separate `fix typo` /
   `address feedback` micro-commit.
3. State the narrowing caveat: the agent SHALL NOT `--amend` an already-verified commit
   or an earlier commit from a previous group — amend applies ONLY to the
   most-recent, not-yet-verified commit.
4. Tie the rationale to git-paw's orchestration model: per-commit verification treats
   each committed artifact as a verification boundary, and the supervisor curates the
   release changelog from these commits, so a clean releasable-unit history avoids the
   manual squashes seen in prior release cycles.

#### Scenario: Commit cadence requires each commit to be a releasable unit

- **WHEN** the embedded coordination skill's "Commit cadence" section is inspected
- **THEN** it states that each commit must build and pass its own gates on its own (a releasable unit)

#### Scenario: Commit cadence prefers amend for just-made-commit fixups

- **WHEN** the "Commit cadence" section is inspected
- **THEN** it instructs the agent to fold a small follow-up to the just-made commit in with `git commit --amend` rather than add a separate micro-commit

#### Scenario: Commit cadence forbids amending verified or earlier commits

- **WHEN** the "Commit cadence" section is inspected
- **THEN** it states the agent SHALL NOT `git commit --amend` an already-verified commit or an earlier commit from a previous group


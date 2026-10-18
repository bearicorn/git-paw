## ADDED Requirements

### Requirement: Orchestrator skill is the supervisor skill relocated

The bundled skill formerly addressed to a supervisor pane SHALL be the orchestrator skill, shipped as `assets/agent-skills/orchestrator.md` and addressed to an agent that drives the session from outside the tmux session at the repository root. Every requirement in this and any other capability phrased as applying to "the embedded supervisor skill" or `supervisor.md` SHALL be read as applying to the orchestrator skill unchanged. The skill SHALL NOT assume it runs inside a pane of the session: it SHALL refer to the operator's interaction surface as "your session", SHALL NOT contain the phrases "your pane" or "own pane", and SHALL describe pane 0 as the dashboard. The skill SHALL name `.git-paw/scripts/sweep.sh verified`, `feedback-gate` and `conflict-ack` as the only path for publishing verdicts. The skill SHALL carry no session-specific value: it SHALL contain neither the broker URL nor the `{{GIT_PAW_BROKER_URL}}` placeholder, and SHALL NOT embed the tmux session name (`paw-{{PROJECT_NAME}}`); it SHALL reach the broker only through the bundled helpers and the MCP tools (which discover the running session) and SHALL name `.git-paw/AGENTS.local.md` as where the session's name and broker URL are recorded.

#### Scenario: Skill carries no pane-bound self-reference

- **WHEN** the rendered orchestrator skill is inspected
- **THEN** it SHALL NOT contain the substrings `your pane` or `own pane`
- **AND** every mention of pane 0 SHALL identify it as the dashboard

#### Scenario: Skill carries no session-specific value

- **WHEN** the orchestrator skill template and its rendered `SKILL.md` are inspected
- **THEN** neither SHALL contain `{{GIT_PAW_BROKER_URL}}`, an `http://` broker URL, or `paw-{{PROJECT_NAME}}`
- **AND** the skill SHALL name `.git-paw/AGENTS.local.md` as the place to read the session name and broker URL

#### Scenario: Existing supervisor-skill content requirements still hold

- **WHEN** the skill-content tests that assert the five gates, spec audit, merge orchestration, stuck detection, stream-timeout recovery, qualitative learnings, the permission ladder (the CLI's own permission system, then relay to the human, then an explicit `sweep.sh approve` on the human's instruction) and the per-CLI setup section are run against the orchestrator skill
- **THEN** they SHALL pass unchanged apart from the asset path and the assertions that this change's MODIFIED requirements replace (no `curl` and no session name in the skill, the roster read when needed, the unattended merge rule)

#### Scenario: Verdicts route through the helper

- **WHEN** the orchestrator skill's verification, feedback and conflict-handling sections are inspected
- **THEN** they SHALL name `sweep.sh verified`, `sweep.sh feedback-gate` and `sweep.sh conflict-ack` as the verdict path
- **AND** SHALL NOT show a raw `curl` posting `agent.verified` or `agent.feedback`

### Requirement: Orchestrator skill reads the session through the event surface

The orchestrator skill SHALL instruct the orchestrator to follow the session by blocking on the broker's event cursor — the MCP `wait_for_events` tool or the bundled `sweep.sh events` helper, carrying the returned `last_seq` forward — rather than by periodically capturing and diffing pane contents, and SHALL explain why: a sampled capture can miss a prompt that is answered and replaced between two samples. It SHALL instruct the orchestrator to treat a `stalled` wave reported by those reads (or by `git paw status`) as a signal to inspect the session. It SHALL state that the in-process drive loop, when running, escalates every live permission prompt to the broker and approves none, and that the orchestrator relays each escalated prompt to the human and runs `sweep.sh approve <pane>` only on the human's explicit instruction. It SHALL instruct the orchestrator, when an unattended session's drive loop has exited on its heartbeat, to re-run `git paw start --unattended` at the repository root, which re-attaches a drive loop to the live session without relaunching it.

#### Scenario: Skill teaches re-attaching the drive loop after a heartbeat exit

- **WHEN** the orchestrator skill's monitoring section is inspected
- **THEN** it SHALL instruct re-running `git paw start --unattended` after the drive loop exits on its heartbeat, to re-attach a drive loop to the live session

#### Scenario: Skill teaches the cursor read

- **WHEN** the orchestrator skill's monitoring section is inspected
- **THEN** it SHALL name `wait_for_events` or `sweep.sh events` and the carried-forward cursor
- **AND** SHALL NOT instruct a fixed-interval pane-capture diff as the primary way to detect events

#### Scenario: Skill acts on a stalled wave

- **WHEN** the monitoring section is inspected
- **THEN** it SHALL instruct the orchestrator to investigate when the wave is reported stalled

#### Scenario: Skill relays escalated prompts read from events

- **WHEN** the monitoring section is inspected
- **THEN** it SHALL state that the drive loop escalates permission prompts and approves none
- **AND** SHALL instruct the orchestrator to relay each escalated prompt to the human and to run `sweep.sh approve` only on the human's explicit instruction

## MODIFIED Requirements

### Requirement: Supervisor skill — permission decisions belong to the agent CLI and the human

The embedded orchestrator skill SHALL document who decides a worker's permission prompt, in this order:

1. The worker's CLI-native permission system runs first: its resolved permission mode and its own allow rules. git-paw adds no rules of its own.
2. Any prompt the CLI raises is relayed to the human. Under `--unattended` the drive loop escalates it to the broker (and the exit summary and the manual-decision log), where the orchestrator reads it through the event surface. Otherwise the orchestrator's own sweep finds it.
3. The human decides. The orchestrator carries out an approval with `sweep.sh approve <pane>` only on the human's explicit instruction.

The skill SHALL state that `sweep.sh approve` answers a prompt once (option 1) and never grants a durable "don't ask again". It SHALL state that a prompt which should not recur belongs in the agent CLI's own allow rules, configured by the user. The skill SHALL NOT describe any command, command family or helper path as safe to approve, SHALL NOT reference a git-paw command classifier, and SHALL NOT enumerate any toolchain's verbs as approvable.

#### Scenario: The orchestrator skill documents the permission ladder

- **WHEN** the orchestrator skill is rendered
- **THEN** it SHALL describe the order: the CLI's own permission system, then relay to the human, then an explicit `sweep.sh approve` on the human's instruction
- **AND** it SHALL state that `sweep.sh approve` answers once and never grants "don't ask again"

#### Scenario: Unattended escalations are read from the broker, not a pane

- **WHEN** the orchestrator skill's permission section is inspected
- **THEN** it SHALL state that under `--unattended` the drive loop escalates prompts to the broker
- **AND** it SHALL NOT state that the drive loop hands prompts to a pane of the orchestrator

#### Scenario: The orchestrator skill names no command as safe

- **WHEN** the rendered orchestrator skill is inspected
- **THEN** it SHALL NOT contain `git paw __classify`, a dev-command allowlist enumeration, or an instruction to absorb routine approvals
- **AND** it SHALL NOT instruct the orchestrator to select a "don't ask again" option

### Requirement: Skill resolution order

The system SHALL provide a function `pub fn resolve(skill_name: &str) -> Result<SkillTemplate, SkillError>` that locates the skill template by name using the following order, returning the first match:

1. A standardized override directory `.agents/skills/<skill-name>/` containing `SKILL.md`, looked up from the process's current directory and up to four of its ancestors (this is the only override location production callers read in this release: `resolve` consults no user config directory, and the flat `<config_dir>/git-paw/agent-skills/<skill-name>.md` file is not read)
2. The embedded default for that skill name shipped in the binary

The name `supervisor` SHALL be an accepted alias of `orchestrator`: `resolve("supervisor")` SHALL return the orchestrator template, and an override directory named either `orchestrator` or `supervisor` (at the location above) SHALL apply to both names, an `orchestrator` directory winning over a `supervisor` directory when both exist.

If neither the user override nor an embedded default exists for the requested name, the function SHALL return `Err(SkillError::UnknownSkill { name })`.

The returned `SkillTemplate` SHALL include a `source: Source` field indicating whether the content came from an override (`Source::AgentsStandard` for the location above) or the embedded default, for diagnostic purposes.

#### Scenario: Resolve falls back to embedded when no user override exists

- **GIVEN** no user override file exists for the `coordination` skill
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::Embedded`

#### Scenario: Standardized override is preferred over embedded default

- **GIVEN** a repository root containing `.agents/skills/coordination/SKILL.md` with valid frontmatter and the body `custom user content`
- **WHEN** `skills::resolve("coordination")` is called from that repository root
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::AgentsStandard`
- **AND** the template content equals `custom user content`

#### Scenario: supervisor is an alias of orchestrator

- **GIVEN** no user override for either name
- **WHEN** `skills::resolve("supervisor")` and `skills::resolve("orchestrator")` are called
- **THEN** both SHALL return `Ok` with identical embedded content

#### Scenario: A legacy supervisor override applies to the orchestrator name

- **GIVEN** a repository root containing `.agents/skills/supervisor/SKILL.md` and no `.agents/skills/orchestrator/`
- **WHEN** `skills::resolve("orchestrator")` is called from that repository root
- **THEN** the result SHALL have `source = Source::AgentsStandard` and the override's content

#### Scenario: An orchestrator override wins over a supervisor override

- **GIVEN** a repository root containing both `.agents/skills/orchestrator/SKILL.md` and `.agents/skills/supervisor/SKILL.md`
- **WHEN** `skills::resolve("supervisor")` is called from that repository root
- **THEN** the result SHALL carry the `orchestrator` override's content

#### Scenario: Unknown skill name returns an error

- **WHEN** `skills::resolve("nonexistent")` is called
- **AND** no user override exists for `nonexistent`
- **AND** no embedded default exists for `nonexistent`
- **THEN** the result is `Err(SkillError::UnknownSkill { name: "nonexistent" })`

### Requirement: Supervisor skill — interactive user input

The embedded orchestrator skill SHALL include a "When the user types in your session" section instructing the orchestrator how to handle operator input that arrives while its monitoring loop is running. The section SHALL:

1. State that the operator can type questions or directives to the orchestrator at any point during the session.
2. Distinguish three cases of user input and map each to existing mechanisms:
   - **Status question** ("how's X going?", "what are the agents working on?") — answer conversationally using the broker `/status` and `/messages/supervisor` reads (via the helper or MCP tools) and a pane capture through `sweep.sh capture`. Do NOT publish; just respond.
   - **Directive** ("ask X to use bcrypt", "tell Y to skip the migration") — publish `agent.feedback` to the named agent via `sweep.sh feedback-gate` (or a `/tell` routing for low-stakes nudges) AND confirm the action conversationally.
   - **Judgment-call ask** ("should we merge feat-a before feat-b?") — apply the orchestrator's normal escalation rules; only publish `agent.question` if the call is genuinely ambiguous beyond what the operator just provided. Otherwise answer conversationally with the reasoning.
3. State that the monitoring loop continues alongside user input — finish the current step, then respond, then resume.
4. The mechanisms are unchanged; the addition is *when to use which* in response to user input.

#### Scenario: Skill mentions interactive user input

- **WHEN** the embedded orchestrator skill is inspected
- **THEN** it contains a heading or section identifying user input handling (e.g. `When the user types in your session`)
- **AND** SHALL NOT contain the heading `When the user types in your pane`

#### Scenario: Skill names the three input cases

- **WHEN** the embedded skill's user-input section is inspected
- **THEN** it identifies the status-question case and maps it to `/status` / `/messages/supervisor` reads and a pane capture
- **AND** it identifies the directive case and maps it to `agent.feedback` via the helper
- **AND** it identifies the judgment-call case and maps it to the normal escalation (with `agent.question` only when ambiguous)

#### Scenario: Skill states the loop continues alongside user input

- **WHEN** the embedded skill's user-input section is inspected
- **THEN** it explicitly states that the monitoring loop continues alongside user input
- **AND** it instructs the orchestrator to finish the current step before responding

### Requirement: Supervisor skill SHALL teach `pane_current_path` for pane→agent resolution

The embedded orchestrator skill (`assets/agent-skills/orchestrator.md`, formerly `supervisor.md`) SHALL contain a section (heading text approximately "Resolve pane to agent" or "Pane→agent mapping") that teaches the orchestrator the canonical resolution command, naming the tmux session by the session name recorded in `.git-paw/AGENTS.local.md` (written here as `<session>`), since the exported skill carries no session-specific value:

```bash
tmux display-message -t <session>:0.<pane> -p '#{pane_current_path}'
```

The section SHALL warn explicitly that:

1. Pane indices are NOT sorted alphabetically by `agent_id`.
2. Pane indices are NOT in the CLI-argument order from `git paw start --specs A B C`.
3. The mapping SHALL NOT be inferred from `git paw status` output (which is sorted alphabetically by the broker) or from the dashboard's row order (same).

The section SHALL teach that the output is resolved to an agent by matching it, as a full path, against the worktree paths the session records: the `worktree_path` entries of the `agents` roster in the per-repo session file (`.git-paw/sessions/<session>.json`, or the same roster as read through the bundled helpers or the MCP session tools), where the matching entry's `branch_id` is the authoritative `agent_id`. The section SHALL NOT teach deriving the `agent_id` from the directory's basename or from any project-prefixed naming shape, because the worktree directory name depends on the configured worktree placement (a child worktree carries no project prefix, and its slug need not equal the `agent_id`). The skill text SHALL stay project-agnostic (no project-specific directory names). The orchestrator SHALL read the roster when it needs the pane of an agent rather than building a pane map once and reusing it for the session, and SHALL in any case re-read it after an agent is added or removed, since the roster changes and tmux renumbers panes on layout changes.

#### Scenario: Supervisor skill names the `pane_current_path` resolution command

- **WHEN** the embedded orchestrator skill is inspected
- **THEN** the content SHALL contain the literal substring `tmux display-message` and `pane_current_path`
- **AND** the section SHALL warn against assuming pane order from `agent_id` alphabetical or from CLI-argument order

#### Scenario: Supervisor skill warns against `git paw status` ordering as a mapping source

- **WHEN** the pane-resolution section is inspected
- **THEN** the section SHALL contain prose explicitly noting that `git paw status` ordering or dashboard row order SHALL NOT be used as the pane→agent mapping source

#### Scenario: Supervisor skill resolves panes against the session's recorded worktree paths

- **WHEN** the pane-resolution section is inspected
- **THEN** it SHALL name the per-repo session file `.git-paw/sessions/` and its `agents` roster's `worktree_path` and `branch_id` fields as the mapping source
- **AND** it SHALL NOT contain a basename-shape rule such as `<project>-feat-<branch>` or an example basename carrying a project prefix

#### Scenario: Supervisor skill re-resolves after the roster changes

- **WHEN** the pane-resolution section is inspected
- **THEN** it SHALL state that the roster is re-read after `git paw add` or `git paw remove`

#### Scenario: Skill reads the roster when needed instead of caching a session-long map

- **WHEN** the pane-resolution section is inspected
- **THEN** it SHALL instruct reading the roster when the pane of an agent is needed
- **AND** it SHALL NOT instruct building a pane map once and reusing it for the rest of the session

### Requirement: Supervisor skill SHALL reference the bundled sweep helper

The embedded orchestrator skill (`assets/agent-skills/orchestrator.md`, formerly `supervisor.md`) SHALL invoke `.git-paw/scripts/sweep.sh <subcommand>` for the operations covered by the helper instead of raw tmux pipelines or raw HTTP requests. The orchestrator runs at the repository root, so the relative path resolves directly. The subcommands taught by the skill SHALL include at minimum: `snapshot`, `capture <pane>`, `approve <pane>`, `status`, `worktrees-status`, `inbox`, `events`, `feedback-gate <agent> <gate> <msg>`, `verified <agent> <msg>`, `conflict-ack <agent-a> <agent-b> <file>`, `status-publish <msg>`, `answer <agent> <text>` and `advanced-main <merged-branch> <summary>`.

The skill SHALL NOT contain the token `curl` anywhere — no raw read or `/publish` example, no instruction to type a `curl` command into a worker's pane, and no prose suggesting one. Every broker read SHALL go through `sweep.sh status`, `sweep.sh inbox`, `sweep.sh events` or the MCP read tools; every publish SHALL go through a `sweep.sh` verb (`status-publish`, `verified`, `feedback-gate`, `conflict-ack`, `learn`, `answer`, `advanced-main`) or the bundled `broker.sh` with `--agent supervisor` (`question`, `intent`); an instruction given to a stalled worker SHALL name the worker's own `broker.sh` verb (for example `.git-paw/scripts/broker.sh blocked`). The orchestrator's own first `agent.status` SHALL use `sweep.sh status-publish`: the orchestrator drives a session that is already running, so the session file the helper reads exists.

The skill SHALL NOT contain `for p in ... ; do tmux capture-pane ...` style loops over pane indices. Those loops trip Claude CLI per-pattern approval prompts on every sweep iteration and defeat the helper's purpose.

#### Scenario: Rendered supervisor skill references sweep.sh

- **WHEN** the embedded orchestrator skill is inspected
- **THEN** every example invoking `tmux capture-pane` across multiple panes SHALL use `.git-paw/scripts/sweep.sh snapshot` or `.git-paw/scripts/sweep.sh capture <pane>` instead
- **AND** every example publishing `agent.verified`, `agent.feedback`, or `agent.status` SHALL use the corresponding `sweep.sh <subcommand>` form
- **AND** no `for p in <list>; do tmux capture-pane ...` loop SHALL appear in the rendered skill content

#### Scenario: Rendered supervisor skill does not contain phantom-prone placeholders

- **WHEN** the embedded orchestrator skill is inspected
- **THEN** the skill SHALL NOT contain the literal string `<agent-id>` or `<your question>` or `<your specific question>` as an agent-id or message argument of any documented helper invocation
- **AND** any remaining placeholder syntax in examples SHALL use clearly-broken forms like `__FILL_IN__` so accidental submission produces an obvious error rather than phantom agents

#### Scenario: The rendered orchestrator skill contains no curl

- **WHEN** the orchestrator skill is rendered for every spec backend and the no-backend case
- **THEN** no rendering SHALL contain the token `curl`

#### Scenario: A stalled worker is told to use its own helper

- **WHEN** the stall-detection section's message to a stalled worker is inspected
- **THEN** it SHALL instruct the worker to run `.git-paw/scripts/broker.sh blocked`
- **AND** it SHALL NOT contain a raw HTTP request

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
2. **ALSO** send the answer text to the asking agent's pane via `tmux send-keys -t <session>:0.<pane-index> "<answer>" Enter` (`<session>` being the session name recorded in `.git-paw/AGENTS.local.md`), because v0.5.0 agents do not poll their inbox for `agent.feedback` responses (drift 34). The instruction SHALL state this rationale explicitly so future contributors understand the workaround is transitional and slated for relaxation when MCP-mediated inbox access lands in v0.6.0.
3. **ALSO** apply the existing paste-buffer recovery sub-case when the answer text is long enough to land in a paste buffer on a paste-aware CLI — i.e. after the `tmux send-keys` of the answer, the supervisor SHALL inspect the pane and, if a paste-buffer indicator is present, send a follow-up `Enter` keystroke to submit the buffered content. This re-uses the paste-buffer recovery action already documented under stall detection; the supervisor skill SHALL cross-reference it (e.g. "see the paste-buffer recovery sub-case under stall detection") rather than duplicate the full recovery text.

The embedded supervisor skill SHALL ALSO include a "Supervisor publishes agent.intent for main-side work" section (or equivalent heading) instructing the supervisor agent to publish `agent.intent` from `agent_id = "supervisor"` whenever it is about to commit changes directly to `main` while coding agents are running. The section SHALL:

- Explain the visibility gap it closes: supervisor commits to `main` are not surfaced as broker events, and agents working in feat branches may produce commits incompatible with a freshly-advanced `main` if they are not notified.
- Provide an example publishing it through the bundled helper — `.git-paw/scripts/broker.sh --agent supervisor intent <summary> <files> [ttl]` — which sends `type` = `agent.intent`, `agent_id` = `"supervisor"`, and a payload containing `files`, `summary`, and `valid_for_seconds`. The example SHALL NOT be a raw HTTP request.
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
- **AND** it contains a `.git-paw/scripts/broker.sh --agent supervisor intent` example
- **AND** the example supplies the summary, the file list and the TTL the helper sends as `summary`, `files` and `valid_for_seconds`
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

### Requirement: Supervisor skill — Merge orchestration

The embedded orchestrator skill (`assets/agent-skills/orchestrator.md`, formerly `supervisor.md`) SHALL include a "Merge orchestration" section that replaces the v0.4 Rust `run_merge_loop` function. The section SHALL instruct the orchestrator to perform merge orchestration via existing mechanisms (the bundled helpers or the MCP tools, shell, git, the configured `test_command` — never a raw `curl`) and SHALL cover:

1. **When to merge** — once all expected agents have published `agent.verified` (or after the user explicitly asks the orchestrator to merge). In an unattended run (the session context carries the drive-loop directive) the orchestrator SHALL merge as soon as this trigger holds, fast-forward only, without first asking the human, and SHALL hold a branch back and escalate only on a conflict (a non-fast-forward or conflicting branch, or a dependency cycle among the branches), an exhausted correction cycle, or a regression: a conflicting branch is skipped and a regressing merge reverted, each with `agent.feedback` to the branch's agent (item 3); a dependency cycle (item 4) and a branch whose correction budget is exhausted are escalated with `agent.question`; every held-back branch is named in the final summary (item 5). In an attended run the orchestrator MAY confirm the merge order with the human before merging. The skill SHALL NOT carry an unconditional rule to ask the human before every merge.
2. **Compute merge order from `agent.blocked` events** — read the broker's message log through `sweep.sh inbox` / `sweep.sh events` or the MCP read tools to find `agent.blocked` events. For each `agent.blocked` from X with `payload.from = Y`, treat as edge "X depends on Y". Topologically sort the dependency graph; agents with no incoming edges merge first.
3. **Per-branch merge loop**:
   - Checkout main: `git checkout main`.
   - Fast-forward merge: `git merge --ff-only feat/<branch>`. Never create merge commits.
   - On non-FF / conflict: SKIP the merge for this branch; publish `agent.feedback` to the branch's agent listing the conflict and asking them to rebase or resolve. Continue with the next branch.
   - On FF success: run the configured `test_command` (from `[supervisor].test_command`). On failure: `git reset --hard <previous-HEAD>` to revert; publish `agent.feedback` listing the regression. On success: continue to the next branch.
4. **Cycle handling** — if the dependency graph has cycles, publish `agent.question` (through `broker.sh --agent supervisor question`) surfacing the cycle and asking the user how to proceed. Do NOT merge any branch in the cycle until the user resolves it.
5. **Final summary** — when all eligible branches are merged (or skipped), publish a final `agent.status` from `agent_id = "supervisor"` summarising what was merged, what was skipped, and any regressions encountered.

#### Scenario: Supervisor skill mentions merge orchestration

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or section identifying merge orchestration (e.g. `Merge orchestration`)

#### Scenario: Supervisor skill names the trigger condition

- **WHEN** the embedded supervisor skill's merge-orchestration section is inspected
- **THEN** it states that merge orchestration runs when all expected agents have published `agent.verified` (or on user request)

#### Scenario: An unattended merge follows the trigger without asking first

- **WHEN** the merge-orchestration section is inspected
- **THEN** it SHALL state that, in an unattended run, merging proceeds once every expected agent is verified, fast-forward only, without asking the human first
- **AND** it SHALL name the only escalations: a conflict (non-fast-forward or dependency cycle), an exhausted correction cycle, and a regression
- **AND** the skill SHALL NOT contain an unconditional "ask the human before merging" rule

#### Scenario: An attended merge may confirm with the human

- **WHEN** the merge-orchestration section is inspected
- **THEN** it SHALL state that in an attended run the orchestrator may confirm the merge order with the human before merging

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

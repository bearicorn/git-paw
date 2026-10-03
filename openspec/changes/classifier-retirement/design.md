## Context

Three components send approval keystrokes today because git-paw's classifier said a command is
safe:

1. **The `--unattended` drive loop.** `src/supervisor/drive.rs`: `sweep_pane` → `classify_prompt` →
   `send_approval`. It prefers the durable option 2 through
   `auto_approve::select_option_index`, and covers pane 0.
2. **The attended dashboard thread.** `commands::supervisor::spawn_auto_approve_thread` →
   `poll::tick_from_status` → `drive_outcomes` → `approve::auto_approve_pane` →
   `approval_gate::approval_send_gate`. It runs inside `__dashboard`, is gated by
   `dashboard_should_auto_approve`, and is disabled under `--unattended` through
   `GIT_PAW_UNATTENDED`.
3. **`sweep.sh approve <pane>`.** A human or the orchestrator runs it. It still asks
   `git paw __classify --resolve-option` which option to press, so it can pick the durable
   "don't ask again" option.

`src/supervisor/auto_approve.rs` (3.2k lines, about half of them tests) mixes two concerns. One
is prompt detection, which several features need. The other is the command classifier, which
this change retires. The classifier also drives `ProtectedPaths` (memory isolation),
`src/supervisor/dev_allowlist.rs` (stack presets), `AutoApproveConfig::effective_whitelist`,
`{{DEV_ALLOWLIST_PRESET}}` in the supervisor skill, the `auto_approved` learnings counter, and
the skill doctrine that teaches the orchestrator to "absorb routine approvals".

The user's decision (2026-10-19) is to retire the classifier in v0.17.0. The proposal lists the
false-SAFE evidence. Pre-approving git-paw's own helper scripts is a separate, consented change
(`cli-setup-consent`). The scoping memo `retirement-scope.md` (sections A, B, C, D, F and G) was
checked against the code at `bfc882a`. Its file and line citations were accurate to within a
few lines. It missed `tests/e2e_supervisor_stop.rs` (an auto-approve stop test) and the tracked
repo-local `.git-paw/config.toml`, which declares both deprecated tables; both are covered
below.

## Goals / Non-Goals

**Goals:**

- No git-paw process sends approval keystrokes on its own initiative. The only approval path is
  an explicit `sweep.sh approve <pane>`, which always presses option `1`.
- Every live permission prompt is detected and escalated the same way, whatever the command text.
  This covers the broker, the orchestrator hand-off, the exit summary, the manual-decision log
  and a learning. The rest of the wave keeps moving.
- Prompt detection keeps working unchanged. That covers the live-prompt gate, input-box parsing
  for `submit_buffered_input` (the GP-22/23 Enter workaround), the trailing-region anchor used by
  cli-unavailable detection, stuck detection, and command-slice extraction for dedup, escalation
  text and learnings.
- Old configurations keep loading and round-tripping. The deprecated tables warn once and do
  nothing.
- Docs and bundled skills state truthfully what git-paw does and does not decide.

**Non-Goals:**

- Seeding or writing any permission rule into an agent CLI. That is `cli-setup-consent`.
- A new exit condition for an unattended run whose agents are all waiting on prompts. That is a
  follow-up; see Risks.
- Repurposing the protected-path set as seeded deny rules (memo E.4).
- Renaming or merging the `approval-*` capabilities. See D10.
- Removing durable grants that 0.16.0 already wrote into a CLI's settings.

## Decisions

### D1 — No automatic approver survives; `sweep.sh approve` presses `1` and checks it first

The drive loop and the dashboard thread lose their approve paths. The send gate, the claim and
the live-prompt re-confirm stay, because `sweep.sh approve` still types into a pane.

`sweep.sh approve <pane>` keeps the pane-0 refusal, the claim, and the fresh capture checked by
`is_live_capture`. It then sends `1` and `Enter` as two separate keystrokes. Before sending, it
looks for the numbered option-1 line in the same capture. It sends nothing when that line is
missing, or when it contains the resolved profile's `broad_grant_marker`. The marker is read with
`discover_prompt_profile_field broad_grant_marker`, with an apostrophe-free fallback.

*Why the guard:* "never a durable grant" should be checked, not assumed. Option 1 is "Yes" for
every Claude prompt shape we know, but the memo flags that this is unverified for other CLIs.
The guard also gives `broad_grant_marker` a consumer. Without one it would be a dead config key,
and config keys are a frozen surface.

*Alternative rejected:* keep `__classify --resolve-option` only to pick the option. That keeps a
classifier on the approve path, which is the thing being retired.

### D2 — The orchestrator relays prompts; it approves only on the human's explicit word

The brief says `sweep.sh approve` survives "as an explicit human/orchestrator-directed action".
The specs separate the mechanism from the doctrine:

- **Mechanism** (`approval-auto`, `broker-agent-helper`): a human or the orchestrator may run
  `sweep.sh approve`. No git-paw process ever runs it.
- **Doctrine** (`skill-agent`, `supervisor-skill-discipline`, `boot-block`,
  `supervisor-autonomous-orchestrator`): the bundled skill tells the orchestrator to relay every
  permission prompt to the human, and to run `sweep.sh approve` only when the human explicitly
  says so. The drive loop's hand-off for a permission prompt asks for a relay, not a decision.

*Why:* an orchestrator that approves on its own reading of the command is an LLM classifier. It
reads untrusted pane text, so it is exposed to prompt injection, and git-paw's bundled skill
would be the thing teaching it to approve. That contradicts the decision. **This interpretation
needs the user's confirmation.** If the orchestrator should be allowed to approve on its own
judgment, only the four doctrine requirements change. The mechanism stays as specified.

### D3 — The dashboard thread is deleted, not kept as a forward-only loop (D-2)

The following are deleted: `spawn_auto_approve_thread` and its classifier wiring struct
(`commands/supervisor.rs`), the spawn block and `dashboard_should_auto_approve` (`main.rs`), the
`UNATTENDED_ENV` / `GIT_PAW_UNATTENDED` producer and consumer, all of `poll.rs` except the status
fetcher, `stall.rs`, `approve::{auto_approve_pane, ApprovalRequest, approval_keystrokes}`, and
`approval_gate::approval_send_gate`. The send gate's only Rust caller was `auto_approve_pane`;
`sweep.sh` mirrors it in shell.

`poll::{AgentStatusRow, fetch_status_over_http}` move into `drive.rs`, which is their only
production consumer. They stay `pub` because `tests/unattended_drive_e2e.rs` imports
`fetch_status_over_http`. `KeyDispatcher` / `TmuxKeyDispatcher` move to a new
`src/supervisor/keys.rs`, because the drive loop's nudges use them, and `approve.rs` is deleted.

*Trade-off:* an attended session no longer writes the manual-decision log, and nothing in git-paw
code watches attended panes. The orchestrator's `sweep.sh detect-stuck` finds stuck-on-prompt
agents. The skill makes it relay them to the human.

### D4 — Prompt detection moves into `permission_prompt.rs`; `auto_approve.rs` goes away

`src/supervisor/permission_prompt.rs` is already "permission-prompt detection" and already owns
`capture_pane` and the approval-marker scan. The detection half of `auto_approve.rs` moves there,
so there is one detection module named for what it does. Once the classifier half is deleted,
`auto_approve.rs` no longer exists.

| Item (`auto_approve.rs` unless noted) | Fate |
|---|---|
| `LIVE_PROMPT_TAIL`, `LIVE_PROMPT_BLOCK`, `LIVE_PROMPT_TRAILING_BOUND`, `LIVE_PROMPT_FOOTER_MARKERS`, `is_footer_line`, `live_prompt_markers_at_tail`, `is_live_prompt`, `anchor_on_trailing_region` | move |
| `is_rule_line`, `strip_input_sigil`, `is_idle_input_box_line` | move |
| `strip_decoration`, `option_line_regex`, `is_option_line`, `is_confirmation_boundary`, `is_bash_command_header`, `extract_command_slice`, `file_prompt_regex`, `extract_path_from_file_prompt` | move |
| `permission_prompt::PermissionType`, `classify_capture*`, `detect_permission_prompt*` | collapse to `has_approval_marker(captured, profile) -> bool` (+ a capture-and-scan wrapper if a caller remains) |
| `READ_MOSTLY_VERBS`, `default_safe_commands`, `is_safe_command`, `is_path_inside_worktree`, `resolve_for_boundary`, `is_worktree_file_op`, `is_worktree_git_op`, `is_worktree_dev_test_op`, managed-script helpers, `normalize_command` and its strip helpers, `DANGER_BASE`, `os_addendum`, `is_dangerous`, the scratch-`rm` family, `ProtectedPaths` and its helpers, `is_protected_path_violation`, `is_git_dir_write`, `PromptShape`, `detect_prompt_shape`, `leading_verb`, `is_arbitrary_code_runner`, `verb_is_read_mostly`, `select_option_index` | delete |
| `src/supervisor/dev_allowlist.rs`, `skills::render_dev_allowlist_preset` | delete |
| `approval_gate::{live_prompt_in_tail, approval_dedup_key}`, `claim.rs`, `manual_approvals.rs` | keep |

Visibility is kept on the move (`pub(crate)` stays `pub(crate)`). The moved unit tests go with
the code, including `static_regexes_compile`. The move is section 1 of `tasks.md`: a pure
structure move, gated on an unchanged test count and a green `sweep_sh_live_gate_parity`, before
any behaviour changes (code-standards, *A behavior change is NOT a refactor*).

*Alternative:* a new `supervisor/prompt_detect.rs` (memo A.2). It was rejected because it would
leave two detection modules side by side, with `capture_pane` and the marker scan split from the
gate that uses them.

### D5 — Escalation shape: one verdict, relay wording, existing dedup

- **Verdict label `permission-prompt`.** It replaces `danger` and `unknown`, and it says what the
  escalation is without implying the answer should be yes. The other verdicts
  (`cli-unavailable`, `nudge-wedged`, `correction-*`, `unmerged`) are unchanged.
- **The escalation command** is `dedup_shape(capture)`: the command slice, or the file-prompt
  path when there is no command header. That is the value the escalation already used.
- **`Escalation::question()`** for `permission-prompt` says the agent is waiting on a permission
  prompt that needs a human decision, that git-paw does not approve prompts, and which command
  it is. **`handoff_text()`** asks the orchestrator to relay the prompt to the human and to run
  `sweep.sh approve` only on the human's instruction. The text paraphrases the command into one
  capped line, as today. **`friction_learning_body`** drops "could not auto-approve".
- **Dedup is unchanged.** `approval_dedup_key` keys on `(agent_id, command)` within the existing
  ~5-minute `DedupWindow`. The brief's "escalated once (deduped)" is read as once per dedup
  window: a prompt still unanswered after the window is escalated again, as a reminder. If the
  user wants strictly once per prompt (until it clears), that needs a different key lifetime.
  See Open Questions.
- **Outcome.** Any escalation makes the outcome `escalated-for-review` (existing behaviour,
  `drive.rs` wind-down). No exit condition is added.
- **Pane 0.** A pane-0 prompt is escalated as agent `supervisor`. `hand_to_orchestrator` already
  declines to type into a pane that shows a live prompt (`orchestrator_ready`), so a pane-0
  prompt is carried by the broker and the summary only.

### D6 — The manual-decision log is written by the drive loop (absorbed change)

This absorbs `drive-loop-manual-decision-log`, keeping its D1 and D2:

- **D1:** a `ManualDecisionSink` trait on `DriveDeps`, a production `RecorderManualDecisionSink`
  that publishes the returned first-seen learning, and a `NoopManualDecisions` /
  `RecordingManualDecisions` test pair.
- **D2:** the call goes after `dedup.should_emit` and before `record_escalation`, so one
  escalation produces exactly one record and banners never do.

Its D3 had a shared `for_session` constructor for two callers. With the dashboard thread gone
there is one caller (`drive_unattended_loop`), so the resolution (`manual_approvals_log` default
on, `learnings`, `cli`) can be inlined there. Keeping a `for_session` helper is acceptable if it
reads better, but it is not required. The cut parts of that change (*Drive-loop auto-approvals
are not logged*, and the auto-approve half of its task 1.3) have no subject any more.

`git paw approvals`' `Suggestion::BundledPresetCandidate` becomes `Suggestion::UserAllowlist`,
rendered `user allowlist`, because there is no bundled preset to promote into. This changes the
`--json` `suggested_target` value and is listed as breaking.

### D7 — Config: deprecate-and-parse with one warning per file per process

- `AutoApproveConfig` and `CommonDevAllowlistConfig` keep every field and type.
  `ApprovalLevelPreset` stays as a parse type. `effective_whitelist`, `resolved()` (and its clamp
  warning), `approve_worktree_writes()` and the uses of `MIN_STALL_THRESHOLD_SECONDS` are
  deleted. The doc comments say "Deprecated: parsed for compatibility; no effect".
- `SupervisorConfig::common_dev_allowlist` becomes `Option<CommonDevAllowlistConfig>` with
  `#[serde(default, skip_serializing_if = "Option::is_none")]`. Today it serializes even when
  absent, so any save of a `[supervisor]` config writes a default table. After this change,
  "declared" is the same as `is_some()`. `merged_with` replaces `[supervisor]` as a whole table
  (`overlay.supervisor.or(self.supervisor)`), so the frozen merge semantics are untouched.
- **Warning.** `load_config_file` already prints `warning: <path>: …` for the `[worktree]` stride.
  The deprecation warning uses the same channel. A pure `deprecated_table_warning(&PawConfig) ->
  Option<String>` names the declared tables, which keeps it unit-testable. A process-wide
  `OnceLock<Mutex<HashSet<PathBuf>>>` prints it once per file per process, because several
  subcommands load the configuration more than once. Suggested text:
  `warning: <path>: [supervisor.auto_approve], [supervisor.common_dev_allowlist] are deprecated
  and have no effect (git-paw no longer approves permission prompts); remove them`.
- **Known effect.** A config that 0.16.0 saved will carry a default
  `[supervisor.common_dev_allowlist]` table the user never wrote, and will warn until the table
  is removed. That is acceptable: the warning tells the user exactly what to delete.
- **Defaults.** `git paw init` drops both commented stanzas and the "stack-neutral built-ins …"
  comment. This repository's own tracked `.git-paw/config.toml` declares both tables. It is
  updated in the same change; the working tree there has uncommitted local edits, so the
  implementer edits only those two tables.

### D8 — Protected paths and the learnings counter are deleted, not repurposed

`ProtectedPaths` existed only so the classifier would refuse to auto-approve writes into operator
territory. With no automatic approval, every such prompt already reaches a human. Turning the set
into seeded deny rules is memo E.4, which belongs with `cli-setup-consent` or later. Keeping it
unused is dead code. `[clis.<name>].settings_path` stays parse-only; its doc comment drops the
`ProtectedPaths` reference.

The learnings aggregator's `auto_approved` branch (`observe()`), `record_auto_approve`,
`extract_command_class` and the permission counter and threshold lose their only producer and
are deleted. The `### Permission patterns` heading and the rendering of `permission_pattern`
records arriving as `agent.learning` stay; the *v0.5.0 sections unchanged* scenario still holds.

### D9 — Spec mechanics

- **REMOVED + ADDED instead of RENAMED** where a requirement's name carries the retired concept:
  *Prompt-free operation is attributed to the permission mode and classifier*, *Settings path
  remains available to memory isolation*, *Escalation of risky and unknown prompts is
  non-blocking*, and *Supervisor skill — tiered permission model and safe-command policy*. The
  new names say what the requirement now guarantees, and REMOVED+ADDED validates and archives
  without the rename-then-modify ordering subtleties. An archive dry-run on a scratch copy of
  `openspec/` (after `bundled-helper-discovery`) applied cleanly: +7 / ~40 / −48. A full
  `openspec validate --all --strict` of the result passed.
- **Purposes.** `openspec archive` does not rewrite `## Purpose`, so the archiver must replace
  these by hand. Leaving them would break *Purpose is real* and Gate-3. Proposed text:
  - `approval-command-safety`: "Detects agent-CLI permission prompts non-invasively via
    `tmux capture-pane` and the resolved CLI prompt profile, reporting only whether an approval
    marker is present. git-paw makes no safety decision about an agent's command. Whether it runs
    is decided by the agent CLI's own permission system or the human. Prompt-free operation is
    attributed to that system, never to a git-paw classifier."
  - `approval-auto`: "The approval-send discipline that remains after the classifier's retirement.
    The shared live-prompt gate decides whether a prompt is on screen. Approval keystrokes reach a
    pane only through an explicit `sweep.sh approve`, which re-confirms the prompt on a fresh
    capture, refuses pane 0 and answers once. Escalation reuses the existing broker variants."
  - `supervisor-unattended-operation`: "Runs an in-process drive loop under
    `git paw start --unattended` that keeps a supervisor wave moving: polling about every 15
    seconds, sweeping every pane, escalating each live permission prompt for a human decision
    without blocking the wave, recovering buffered input, detecting completion and CLI
    unavailability, and exiting with a per-agent summary. It approves nothing."
  - `approval-pattern-surfacing`: "Records every permission prompt the drive loop escalates for a
    human decision to a per-session JSONL log. Provides `git paw approvals` to aggregate those
    patterns by frequency, with a hint for which of the agent CLI's own allowlists (project or
    user) could absorb them. First-seen patterns can emit a `permission_pattern` learning. The
    channel is opt-out via config."
  - `core-memory-isolation`, `core-security-posture`, `core-lang-agnostic`, `supervisor-config`,
    `cli-prompt-profiles`: remove the clauses that name the classifier, the protected-path
    enforcement, `{{DEV_ALLOWLIST_PRESET}}`, the common dev allowlist, and the broad-grant
    eligibility rule.
  - Then run `tests/spec_purpose_backfilled.rs`, `tests/specifications_page_lists_every_capability.rs`
    and `mdbook build docs/`. The Specifications index blurbs in `docs/src/specifications/README.md`
    are updated in tasks section 10.
- **D10 — no capability rename now.** After this change `approval-auto` holds the send discipline
  and `approval-command-safety` holds detection plus the "no safety decision" rule. Better names,
  such as merging both into one `approval-prompt-handling`, are a pure reorganization. That belongs
  in a later release under `spec-organization` §2, after `cli-setup-consent` settles where
  *No permission grants are seeded into agent CLI settings files* lives.
- **Removed constant-derived-prose rule.** `core-lang-agnostic` *Agent-facing prose is authored in
  bundled assets* drops its "constant-derived prose stays compiled" paragraph and scenario. The
  rendered dev allowlist was the only instance, so the scenario would have no test. Re-add the
  rule when a new instance appears.

### D11 — The seeding requirement is left to `cli-setup-consent`

*No permission grants are seeded into agent CLI settings files* (`approval-command-safety`) is not
touched, per the brief. Its last sentence ("Prompt-free agent operation is provided by the CLI's
own resolved permission mode together with git-paw's command classifier …") becomes stale once
this change lands. `cli-setup-consent`'s MODIFIED delta of that requirement must drop the
classifier clause. Its earlier draft (`7f01f322`, on `feat/v0.19.0-specs`) still carries it, and
also ADDs *CLI setup invocations escalate as danger*, which has no danger list to hang on any
more. After both changes land, the requirement stays in `approval-command-safety`, which is
where its scenarios' tests look. It can move into the `cli-setup` capability at the D10 rename.
If `cli-setup-consent` does not ship in v0.17, the archiver of this change should file the
one-sentence fix as a follow-up rather than leave it stale.

## Cross-change coordination

Pending changes checked (excluding the four dropped changes and the absorbed one), by exact
requirement name, per capability:

| Change | Shared capability | Overlap with this change's deltas | Verdict |
|---|---|---|---|
| `bundled-helper-discovery` | `broker-agent-helper`, `supervisor-learnings` | **MODIFIED *Helper publish subcommands*** (both). Its ADDED requirements are different names. | **Fixed order: `bundled-helper-discovery` archives first.** This delta is that delta's text verbatim, except the two classifier clauses this change retires (paragraph "Each helper SHALL be invocable …" and the last scenario's classifier/seeding bullets). Every one of its SHALLs and scenarios is kept. If `bundled-helper-discovery` changes that requirement again before archiving, rebase this delta on its final text. |
| `production-panic-free` | `core-project-conventions`, `core-ci-hygiene` | none (it ADDs different requirements) | Archives after this change. It must adjust: the two static regexes it lists in `src/supervisor/auto_approve.rs` (`file_prompt_regex`, `option_line_regex`) live in `src/supervisor/permission_prompt.rs`. Its *Every static pattern compiles under test* pointer `supervisor::auto_approve::tests::static_regexes_compile` becomes `supervisor::permission_prompt::tests::static_regexes_compile`. Its "14 static compiles" count is unchanged; both regexes move and none is deleted. |
| `broker-repo-identity` (being reworked) | `broker-runtime` | none. This change MODIFIES only *Start flow conditionally creates dashboard pane* and REMOVES *Auto-approve thread location in dashboard subprocess*. It does not touch *Purge flow cleans up broker log*, *Stop flow shuts down broker via pane 0 exit*, the pause-flow requirements, *Stale broker detection*, *GET /status …*, *Dashboard subcommand starts broker and dashboard*, or `core-error-handling`. None of those mentions auto-approval. | no conflict |
| `stop-without-confirmation` | `broker-runtime`, `cli-parsing` | none (*Stop flow …*, *Stop subcommand* vs this change's *Start flow …*, *--unattended flag*) | no conflict |
| `broker-listener-origin-guard` | `broker-runtime`, `core-security-posture` | none (ADDED only) | no conflict |
| `cli-launch-identity` | `boot-block`, `cli-resolution` | none (*End-to-end boot registration* is not in its delta) | no conflict. The memo's `--settings` design is not specced here. |
| `supervisor-boot-wiring` | `boot-block`, `skill-agent`, `supervisor-learnings` | none (different requirement names) | no conflict. Code-level: both edit `assets/agent-skills/supervisor.md`; merge by hunk. |
| `status-and-picker-truth` | `supervisor-launch` | none (ADDED only) | no conflict |
| `conflict-escalation-persistence` | — | — | no shared capability |
| `drive-loop-manual-decision-log` | `approval-pattern-surfacing` | MODIFIED *Manual-decision log file* | **Absorbed.** The orchestrator deletes the directory once this change lands; it must not archive. |
| `classifier-compound-segments`, `broad-grant-scope-safety`, `approval-send-identity-check`, `approval-pane-map-refresh` | `approval-*`, `supervisor-unattended-operation` | MODIFY requirements this change REMOVES (*Whitelist …*, *Prefix matching …*, *Classifier normalizes a trailing exit-code probe …*, *Broad grant …*, *Option-index selection …*, *Worktree-confined file operations …*, *git-paw managed helper-script invocations …*, *Auto-approve classifier-safe prompts*) | **Dropped.** They must be deleted before this change archives, or their archive would fail on removed requirements. `approval-send-identity-check` MODIFIES *Approval keystrokes require a re-confirmed live prompt*, which this change leaves untouched. |
| `cli-setup-consent` (authored in parallel, not yet in `openspec/changes/` here) | likely `approval-command-safety`, `skill-agent`, possibly `broker-agent-helper`, `cli-resolution`, `mcp-agent-docs` | Must not MODIFY *Helper publish subcommands*, *End-to-end boot registration*, *Bundled docs-fetch skill and helper*, *Prompt-free operation is attributed to the agent CLI's permission system*, or the new skill-agent requirement, unless it archives after this change and bases its text on these deltas. It owns *No permission grants are seeded …* (D11). | Check when it lands. |

## Risks / Trade-offs

- **[Unattended runs stall more often]** Every command the CLI's own permission system does not
  allow now waits for a human. Under `agent_approval = "auto"` (Claude `acceptEdits`), the
  `broker.sh` boot publish, `git commit` and the test command all prompt. → The docs make it
  explicit that a prompt-free run needs the CLI's own allow rules (or `git paw setup-cli` once
  `cli-setup-consent` lands), or `full-auto` inside the documented sandbox. Escalations are
  deduped, logged and summarised. The wave exits `escalated-for-review`, or at the 25-minute
  heartbeat, rather than hanging silently. "Exit early when every live agent is blocked on a
  prompt" is the named follow-up. It also removes the premise of v0.24's
  `unattended-heartbeat-progress` ("an approval dispatched by the loop counts as progress").
- **[The orchestrator's own pane stalls]** The loop no longer clears pane-0 prompts, and the
  hand-off into a prompting pane is suppressed. → The pane-0 escalation goes to the broker and the
  summary. The docs recommend `[supervisor] approval = "full-auto"`, or the CLI's own rules for
  `sweep.sh` / `broker.sh`, for the orchestrator.
- **[Memory isolation loses its mechanical check]** The protected-path danger rule is gone. →
  Every write prompt into operator territory already reaches a human. The coordination and
  supervisor doctrine stays. The sandbox chapter, and deny rules in the CLI's own settings, are
  the mechanical options.
- **[Durable-grant residue]** 0.16.0's classifier selected "Yes, and don't ask again" for safe
  and read-mostly prompts. Claude persisted those grants into the repository-root
  `.claude/settings.local.json`, which every worktree and the user's own sessions share. This
  repository's file holds 281 entries, including broad ones such as `Bash(git-paw purge:*)`.
  Retirement stops creating them and removes none. → The release notes call it out (tasks
  section 12). A read-only `doctor` advisory is a possible follow-up. Never auto-delete.
- **[Attended sessions lose the decision log]** See D3. → Accepted. `git paw approvals` reads
  unattended runs' logs.
- **[Existing tests cannot "pass unchanged"]** AGENTS.md §6 cannot hold, because many tests pin
  the old approve behaviour. → Each is deleted or rewritten in the section that changes its
  behaviour, with the scenario it now covers named in tasks section 9.
- **[Lost work]** `backup/classifier-compound-segments-r3` (about 4.4k unmerged lines) is
  abandoned. Its holes table is reused as test fixtures for *Command text cannot steer the
  outcome*.
- **[Large diff]** The change touches 24 capabilities, so a reviewer may miss a SHALL. →
  Sections are ordered so each is a buildable commit, and section 8 ends with a grep gate for the
  retired symbols.

## Migration Plan

1. Users with `[supervisor.auto_approve]` / `[supervisor.common_dev_allowlist]` see one
   deprecation warning per file and per process. Their configs load and save unchanged. They can
   delete the tables at their leisure.
2. Users who relied on `--unattended` approving routine commands move those commands into their
   agent CLI's own allow rules (or `git paw setup-cli`), or run `full-auto` inside a sandbox.
3. Users who scripted `git paw __classify` or `sweep.sh classify` (both internal) must stop. The
   commands fail as unknown.
4. The release notes, carried by the change's `BREAKING CHANGE:` footer, which feeds git-cliff,
   list the four breaking surfaces and the durable-grant residue.
5. **Rollback:** revert the change. The config tables round-trip throughout, so a revert restores
   their effect with no data loss.

## Open Questions

1. **D2:** may the orchestrator approve a prompt on its own judgment? The specs say no: it relays,
   and approves only on the human's explicit instruction.
2. **D5:** "escalated once": once per ~5-minute dedup window (as specified), or once per prompt
   until it clears?
3. **D6:** the `suggested_target` JSON value rename (`bundled preset candidate` →
   `user allowlist`). Is a breaking JSON change acceptable, or should the old value stay with new
   help text?
4. **D1:** the option-1 durable-grant guard in `sweep.sh approve`. It was added beyond the brief
   to make "never durable" checked; confirm or drop.

## Context

Nothing in `src/` reads token usage or cost. The pieces that make it possible already exist: each
Claude-family worker is launched with a git-paw-assigned session id (`--session-id <uuid>`, recorded
as `WorktreeEntry.cli_session_id`), each CLI now has a provider record, and the drive loop sweeps on
a fixed cadence. A breach is answered by its own policy, `[supervisor] budget_on_exhausted`
(`escalate` | `abandon`), separate from the correction loop's `on_exhausted`.

The concrete usage report of any CLI is a third-party fact the code does not encode. For Claude
Code it is expected to be the session transcript under the CLI's configuration directory, keyed by
session id, with per-message token counts.

Verified against Claude Code 2.1.322 (2026-11-10): the report is
`<config dir>/projects/<cwd slug>/<session id>.jsonl`, the config dir being a variant's
`settings_path` parent, else `CLAUDE_CONFIG_DIR`, else `~/.claude`, and the slug the worktree path
with every non-alphanumeric character replaced by `-`. Subagent turns go to
`<session id>/subagents/agent-*.jsonl` beside it. Each `assistant` line carries `message.usage`
(`input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens`); one
response is split over several lines repeating the same `message.id` and usage, so each id counts
once. No cost figure is written, so a `budget_max_cost` cap is reported unenforceable for this
family. The adapter falls back to any project directory holding `<session id>.jsonl` if the slug
rule changes.

## Goals / Non-Goals

**Goals:**

- Per-worker and per-tier token usage, and cost where the CLI itself reports it.
- A cap that stops git-paw from adding spend and surfaces the breach, for unattended runs.
- Honest "unknown" everywhere a figure cannot be attributed.

**Non-Goals:**

- A price table. Shipping per-model prices would go stale and couple git-paw to vendors' pricing;
  cost is shown only when the CLI reports it.
- Stopping a worker mid-turn, killing a pane, or any hard enforcement inside the CLI.
- Attributing the operator's own driving session.
- A dashboard view (the dashboard's future is an open v0.18.0 question).

## Decisions

**D1 — Facet shape.** `CliProvider::usage(&UsageQuery) -> UsageReport` where the query carries the
session id, worktree path and resolved config directory, and the report is `Known { input, output,
cache_read, cache_write, cost: Option }` or `Unknown(reason)`. The provider supplies a pure parser
from report text to figures; a thin adapter locates and reads the report. Default: `Unknown`.

**D2 — Roles.** `worker` (each coding agent; grouped by CLI id, which is how a tier is configured
after `cli-provider-record`) and `orchestrator` (each headless one-shot the drive loop starts; it is
assigned a session id when its provider has a `session_id_args` template, and the loop keeps those
ids for the run). The operator's session is out of git-paw's view; outputs say "excludes your own
session".

**D3 — Where it shows.** `git paw status --json`: a `usage` object (or `null`) per worktree and a
`usage_total` over known figures, additive. The drive loop exit summary: a usage section by role and
by CLI id. Human `git paw status` stays byte-identical, preserving the existing session-state rule.

**D4 — Breach handling has its own policy, `budget_on_exhausted`.** Review round 1 split it from
`[supervisor.correction] on_exhausted`, so tuning correction cannot end a run on the budget. The
loop re-reads usage on a slower cadence than the sweep (every few sweeps, parsing only appended
report lines) and keeps each session's last known figures, so the total never goes down. On breach
it publishes one `budget-exhausted` escalation review item, then stops initiating spend: no
re-engagement or correction, no one-shots, no branch refreshes or their notifications. A human's
text left in a pane is still submitted. Permission prompts need no change: since v0.17.0
`classifier-retirement` the loop escalates every prompt and answers none, so a worker stops at its
next prompt regardless of the budget. `escalate` keeps the loop running so
completion is still observed; `abandon` exits with outcome `budget-exhausted` and a non-zero status.
Interrupting workers was rejected: it risks losing uncommitted work and needs a per-CLI interrupt
contract nobody has verified. Consequence, documented: a worker that does not reach a prompt
(`full-auto`, or only edits and commands its CLI's own permission mode and allow rules let through)
keeps spending until its current task ends.

**D5 — Unknown is not zero.** At loop start, with a cap set, the loop prints which workers' usage is
unknown and, for `budget_max_cost` when not every worker CLI reports cost, escalates once that it cannot be evaluated. Totals are labelled
"of known usage". A cap is never silently treated as satisfied because figures are missing.

**D6 — Skill step.** The exported orchestrator skill's budget step tells the driving agent to read
`git paw status --json` usage before opening a new wave or dispatching new work, to stop and report
to the operator on breach, and never to stop or kill a worker itself. It names no CLI or vendor.

## Risks / Trade-offs

- **Report format churn** → the parser is per provider and fixture-tested; a parse failure yields
  `Unknown(reason)`, never a wrong number and never a failed command.
- **Stale figures** → reports are read when asked; a mid-turn worker's latest message may be missing.
  Acceptable for a guardrail that acts between turns.
- **Privacy** → transcripts contain conversation text. The adapter reads only the usage fields it
  needs and never logs or stores the rest.
- **Exit summary edited twice** → v0.18.0 `supervisor-pane-removal` also modifies it; this change
  lands later and rebases its delta onto that text (both add independent lines).

## Spec traceability

| Spec | Scenario | Test |
|---|---|---|
| cli-provider | A known report yields figures | `agent_cli::usage::tests::a_known_report_yields_the_sums_counting_each_response_once` |
| cli-provider | A generic CLI's usage is unknown | `agent_cli::usage::tests::a_generic_cli_is_unknown_with_a_reason_naming_the_cli` |
| cli-provider | A session without an assigned id is unknown | `agent_cli::usage::tests::a_session_without_an_assigned_id_is_unknown` |
| cli-provider | A malformed report yields unknown | `agent_cli::usage::tests::a_malformed_report_is_unknown_not_a_partial_total` |
| cli-provider | An unknown reason carries no report content | `agent_cli::usage::tests::a_malformed_report_is_unknown_not_a_partial_total`, `agent_cli::usage::tests::the_adapter_skips_a_record_still_being_written_but_not_a_malformed_one` |
| session-state | A worker with a usage report shows figures | `tests/status_json_usage.rs::a_worker_with_a_usage_report_shows_its_token_totals` |
| session-state | Unknown usage is null and counted | `tests/status_json_usage.rs::an_unknown_worker_is_null_and_counted_in_the_total` |
| session-state | Human-readable status is unchanged by usage | `tests/status_json_usage.rs::human_status_is_byte_identical_with_and_without_usage` |
| supervisor-config | Absent caps load as no cap | `config::supervisor::tests::absent_budget_caps_load_as_no_cap_and_are_not_written_back` |
| supervisor-config | A zero cap is rejected | `config::supervisor::tests::a_zero_or_negative_cap_is_rejected_naming_the_field` |
| supervisor-config | Caps round-trip | `config::supervisor::tests::budget_caps_round_trip` |
| supervisor-unattended-operation | A breach is escalated once | `supervisor::drive::tests::budget_guard::usage_is_read_only_every_n_sweeps_and_breaches_once`, `supervisor::drive::tests::budget_guard::a_breach_under_escalate_escalates_once_and_sends_nothing` |
| supervisor-unattended-operation | A breach under escalate stops initiated spend | `supervisor::drive::tests::budget_guard::a_breach_under_escalate_escalates_once_and_sends_nothing` |
| supervisor-unattended-operation | A breach starts no orchestrator invocation and touches no worker | `supervisor::drive::tests::budget_guard::a_breach_under_escalate_escalates_once_and_sends_nothing`, `supervisor::drive::tests::budget_guard::a_breach_under_abandon_exits_budget_exhausted_without_touching_a_pane` |
| supervisor-unattended-operation | No cap leaves the loop unchanged | `supervisor::drive::tests::budget_guard::no_cap_leaves_actions_and_outcome_unchanged` |
| supervisor-unattended-operation | A breach under abandon ends the run | `supervisor::drive::tests::budget_guard::a_breach_under_abandon_exits_budget_exhausted_without_touching_a_pane`, `tests/unattended_drive_e2e.rs::a_budget_breach_under_abandon_exits_non_zero_and_leaves_the_worker_running` |
| supervisor-unattended-operation | Unknown workers are named at start | `supervisor::drive::tests::budget_guard::the_start_report_names_a_worker_with_unknown_usage` |
| supervisor-unattended-operation | An unenforceable cost cap is reported and escalated once | `supervisor::drive::tests::budget_guard::the_start_report_says_a_cost_cap_cannot_be_evaluated` |
| supervisor-unattended-operation | Summary reports outcome and escalations | `supervisor::drive::tests::summary_reports_outcome_states_and_escalations` |
| supervisor-unattended-operation | A completed wave is not reported as working | `supervisor::drive::tests::completed_wave_resolves_terminal_state_not_working`, `supervisor::drive::tests::two_done_agents_all_merged_reports_completed` |
| supervisor-unattended-operation | Finished workers with unmerged branches are not reported as merged | `supervisor::drive::tests::two_done_agents_none_merged_reports_escalated_for_review` |
| supervisor-unattended-operation | An undeterminable merge comparison counts as unmerged | `supervisor::drive::tests::comparison_error_counts_as_unmerged_with_reason` |
| supervisor-unattended-operation | Unmerged entries never reach a surface | `supervisor::drive::tests::two_done_agents_none_merged_reports_escalated_for_review` |
| supervisor-unattended-operation | The usage section separates roles and names unknowns | `supervisor::drive::tests::budget_guard::the_usage_section_separates_roles_and_names_unknowns` |
| skill-agent | The rendered skill carries the budget step | `tests/orchestrator_budget_skill_content.rs::the_rendered_skill_carries_the_budget_step` |
| skill-agent | The budget step is vendor-agnostic | `tests/orchestrator_budget_skill_content.rs::the_budget_step_is_vendor_agnostic` |
| cli-provider | A relocated variant's reports are found | `agent_cli::usage::tests::the_adapter_reads_the_session_and_its_subagents_from_the_relocated_dir` |
| supervisor-config | The budget policy is independent of the correction policy | `config::supervisor::tests::budget_on_exhausted_round_trips_independently_of_the_correction_policy` |
| supervisor-config | Doctor warns about a cost cap no worker CLI can satisfy | `doctor::tests::the_probe_flags_a_cost_cap_over_a_claude_fleet`, `doctor::tests::a_cost_cap_no_worker_cli_reports_is_a_warning_not_a_failure` |
| supervisor-unattended-operation | A rewritten report is read again from its start | `agent_cli::usage::tests::a_report_rewritten_at_the_same_length_is_read_again_from_its_start` |
| supervisor-unattended-operation | An unevaluable cost cap does not change the outcome | `supervisor::drive::tests::budget_guard::an_unevaluable_cost_cap_leaves_a_clean_wave_completed` |
| supervisor-unattended-operation | The token cap counts every token kind | `supervisor::drive::tests::budget_guard::the_token_cap_counts_cache_tokens_too`, `tests/status_json_usage.rs::a_worker_with_a_usage_report_shows_its_token_totals` |
| supervisor-unattended-operation | A removed, swapped or unknown worker keeps its spend | `supervisor::drive::tests::budget_guard::the_total_never_goes_down_when_a_worker_leaves_swaps_or_turns_unknown` |
| supervisor-unattended-operation | A re-read parses only appended records | `agent_cli::usage::tests::a_cached_re_read_parses_only_the_appended_lines` |
| supervisor-unattended-operation | A breach still submits a human's buffered text but refreshes no branch | `supervisor::drive::tests::budget_guard::after_a_breach_buffered_input_is_submitted_but_no_branch_is_refreshed` |

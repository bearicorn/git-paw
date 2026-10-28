## Context

`config/prompt_profile.rs` already has the right data model: `CliPromptProfile` (shape only, no
permission field), `CliPromptProfileOverride` with per-field fallback, and `resolve_prompt_profile`.
What is missing is plumbing. The detectors were moved onto profile *literals* by the
`cli-prompt-profiles` change but kept calling the embedded default directly, and two regexes are
compiled once into process-wide `OnceLock`s from it (`file_prompt_regex`, `option_line_regex`), so
they cannot vary per pane even in principle.

Where a profile is resolved today it is resolved for the wrong CLI: `cmd_prompt_profile` resolves
`[supervisor].cli` → `default_cli` once for the session (v0.17.0 `classifier-retirement` deletes the
other session-level resolution, the dashboard auto-approve wiring in `cmd_dashboard`); only the
launch gate resolves the pane's own CLI, and it does so from the leading token of the CLI identity
(v0.17.0 `cli-launch-identity`), not of the launch command.
The drive loop knows each agent's CLI only from the broker `/status` row (`worker_cli`), and that
`cli` field is in `StatusPayload`, which agents publish themselves.

One more Claude-only compiled fact sits next to the detectors: `readiness::DIALOG_MARKERS` holds
Claude Code's bypass-acceptance and folder-trust dialogs. (The other, `ProtectedPaths`, is deleted
by v0.17.0 `classifier-retirement` together with the classifier it fed.)

After v0.17.0 git-paw answers no permission prompt on its own: every live prompt is escalated, and
`sweep.sh approve` presses option `1` only when a human or the orchestrator runs it. The detectors
this change threads therefore decide only *whether* a live prompt is on a surface and *what* to
escalate (command slice, file-prompt path), plus readiness, input-box, mid-response,
CLI-unavailability and mode inference — never whether a command may run.

## Goals / Non-Goals

**Goals:**

- Every detector reads the profile of the CLI on the surface it inspects.
- Byte-identical decisions when no override is configured.
- Decisions callable as pure functions over recorded text, per CLI (v0.23.0 conformance suite).
- Provider-owned first-run dialogs, so adding a provider is one place.

**Non-Goals:**

- New prompt profiles for other CLIs. Only Claude has one; others resolve the embedded default until
  the v0.23.0 audit records their real shapes.
- Changing any detection algorithm or window size.
- Proactively writing a CLI's trust marker before launch (see Decisions, D5).

## Decisions

**D1 — Thread `&CliProfile`, decide purely.** Each detector gains a profile parameter and the
remaining no-profile wrappers (`classify_pane_readiness`, and any capture-and-scan wrapper v0.17.0
kept around `has_approval_marker`) are deleted once their callers are migrated, so the compiler
lists every site. Following `docs/src/architecture-direction.md`, the decision functions take
`(profile, captured text, observed inputs)` and return data; capture and keystrokes stay in the
shell. The drive loop's escalation path (`dedup_shape`, `extract_command_slice` and the file-prompt
path match) gains the profile, so the command an escalation names is cut with the pane's own
shapes.

**D2 — Resolve once per agent from the launch record.** The drive loop builds an
`agent id → CliProfile` map from the session's `WorktreeEntry.cli` (git-paw wrote it at launch) when
it starts, and looks a surface up by its agent. An agent added later is resolved
on first sight; an agent with no recorded CLI resolves `generic`. The broker row's `cli` stays
display-only. Alternative — keep using `/status` — rejected: an agent could publish a `cli` naming a
profile whose markers misread its own pane.

**D3 — Compile patterns at resolution.** `CliProfile` holds compiled `Regex` values for the pattern
fields. Config load compiles every override pattern and fails with
`[clis.<name>].prompt_profile.<field>: <regex error>`. The embedded default's patterns are compiled
once and shared. This removes the two process-wide `OnceLock`s and the `.expect` on an overridable
input.

**D4 — Helper per pane.** `sweep.sh` already knows each pane's agent and CLI from the discovery
roster (`agents[].cli`); it resolves each field once per distinct CLI via
`git paw __prompt-profile --cli <id> --field …`. That includes the fields `sweep.sh approve` reads
for its live-prompt re-confirm and its option-1 guard (`broad_grant_marker`, read today through
`discover_prompt_profile_field`), so the guard checks the target pane's own durable-grant wording.
With no roster CLI it calls without `--cli`, which keeps today's session-CLI fallback.

**D5 — First-run dialogs: reactive, provider-owned.** `DIALOG_MARKERS` moves into the provider layer
as compiled data (answering an acceptance dialog is a permission decision, so it is not
profile-overridable). Today every pane is checked for Claude's dialogs; to keep that, they become the
embedded default set that a provider without dialogs of its own inherits — so a `claude-oss` entry
without `family` keeps having its trust dialog answered. The roadmap item "trust-folder pre-acceptance" — writing the CLI's trust marker
before launch — is not specced: v0.16.0 already answers the trust dialog at launch (GP-03b); the
marker lives in the CLI's own state file, whose format is unverified and which a running CLI may
rewrite concurrently; and `approval-command-safety` (*No permission grants are seeded into agent CLI
settings files*, as v0.17.0 `cli-setup-consent` words it) forbids git-paw writing a trust marker
except through the explicit, consented `git paw setup-cli`, whose `--trust` item no CLI declares
yet. Revisit only if the v0.23.0 audit finds a CLI whose dialog cannot be answered reactively, and
then as a `--trust` fact row for `setup-cli`, not as a launch-time write.

## Risks / Trade-offs

- **Per-pane resolution changes override behaviour** → a session whose `[supervisor].cli` override was
  silently applied to every pane now applies it only to that CLI's panes. That is the bug being
  fixed; the changelog says so.
- **Signature churn across `drive.rs` and `permission_prompt.rs`** → done seam-first: add the
  parameter with the embedded default at every call site (byte-identical, suite green), then switch
  callers to the per-agent map, then delete the wrappers.
- **v0.17.0 restructuring** → `classifier-retirement` moves the detectors out of `auto_approve.rs`
  into `permission_prompt.rs` and rewrites the drive loop's prompt path to detect-and-escalate; this
  lands after it and after v0.18.0's backend seam, and Gate 2 runs on the merged tree. Task 1.1
  re-greps the sites rather than trusting the pre-v0.17.0 list.
- **Regex compile cost** → once per distinct CLI per process; negligible next to a pane capture.

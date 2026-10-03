## Why

git-paw decides that an agent's shell command is "safe" and then types the approval into the
agent's pane. That decision cannot be made reliably by pattern-matching command text. Three Gate-5
security reviews of the in-flight `classifier-compound-segments` hardening each found new
false-SAFE classes. Every capture below classifies `safe` on the shipped 0.16.0 classifier and on
the branch tip, so git-paw would approve it, often with a durable "don't ask again" grant:

- **Boundary poisoning.** A command block of `ls` followed by `rm -rf /important # [y/N]` is safe.
  The approval markers are matched with `contains()`, so a marker inside the command text moves
  the command-slice boundary.
- **`$'…'` quoting.** `git reset $'--hard' HEAD~1` and `find . $'-delete'`.
- **Environment injection.** `export GIT_SSH_COMMAND=id; git fetch`.
- **The `git` family is a leaky denylist.** `stash`, `update-ref -d`, `reflog expire` +
  `gc --prune=now`, `worktree add /tmp/x`, `credential fill`, `clone -u` / `--template` / `-c`,
  `fetch` / `pull --upload-pack=`, `bisect run`, `grep -O`, `difftool -x`, `archive --exec`,
  `commit -n`, and git-paw's own `git paw stop` and `git paw purge`.
- **Code execution through a read-mostly verb.** `awk 'BEGIN{system("id")}'`.
- **Secret reads.** `cat ~/.ssh/id_rsa`, `cat ~/.aws/credentials`.

Each fix opens the next hole, so the classifier is retired rather than hardened again.
Permission decisions for general commands belong to each agent CLI's own permission system:
the launch approval flags git-paw already maps, the CLI's native modes and allow rules, and a
sandbox. git-paw keeps the part it does well, which is detecting a live prompt and getting it in
front of a human without stalling the rest of the wave. Pre-approving git-paw's own helper
scripts is a separate, consented change (`cli-setup-consent`, `git paw setup-cli`) and is not
specified here.

## What Changes

- **BREAKING:** No git-paw component approves a permission prompt on its own. The `--unattended`
  drive loop's classify-then-approve path (`sweep_pane` → `classify_prompt` → `send_approval`) is
  replaced by detect-and-escalate. Every live permission prompt is escalated once per dedup
  window to the broker, the orchestrator pane (when present), the exit summary and the
  manual-decision log, and the rest of the wave keeps moving. There is no new exit condition.
- **BREAKING:** `--unattended` no longer auto-approves anything. Docs state that a truly
  unattended run needs the agent CLI's own allow rules, or `full-auto` inside a sandbox.
- The attended dashboard auto-approve thread is deleted: `spawn_auto_approve_thread`,
  `src/supervisor/poll.rs`, `approve::auto_approve_pane`, `src/supervisor/stall.rs`,
  `dashboard_should_auto_approve` and the `GIT_PAW_UNATTENDED` signal.
- **BREAKING:** The hidden `git paw __classify` subcommand and the `sweep.sh classify` verb are
  removed.
- `sweep.sh approve <pane>` survives only as an explicit action that a human or the orchestrator
  runs. It always presses option `1` (one-time Yes). It never consults a classifier and can never
  select a durable "don't ask again" option: it sends nothing when option 1 is missing or reads as
  a durable grant. The pane-0 refusal, the per-pane claim and the fresh-capture live-prompt
  re-confirm stay.
- **BREAKING (config):** `[supervisor.auto_approve]` (`enabled`, `safe_commands`,
  `stall_threshold_seconds`, `approval_level`, `approve_worktree_writes`) and
  `[supervisor.common_dev_allowlist]` (`enabled`, `stacks`, `extra`) still parse and round-trip,
  so old configs load. They have no effect, and each configuration file that declares them gets
  one deprecation warning per process naming the tables. Both are removed from the
  `git paw init` default config. `effective_whitelist`, the dev-allowlist presets
  (`src/supervisor/dev_allowlist.rs`) and the `{{DEV_ALLOWLIST_PRESET}}` skill placeholder go.
- `src/supervisor/auto_approve.rs` is split, not simply deleted. Prompt detection moves into
  `src/supervisor/permission_prompt.rs`: the live-prompt gate, input-box and rule-line parsing,
  the trailing-region anchor, and command-slice extraction. The classifier half is deleted: the
  whitelist, danger list, scratch-path exception, normalizers, protected-path and `.git/` rules,
  managed-helper rule, prompt shape and option selection. `PermissionType` collapses to "an
  approval marker is present".
- Absorbs the re-scoped `drive-loop-manual-decision-log`. The drive loop records every live
  prompt it escalates for a human decision in
  `.git-paw/sessions/<session>.manual-approvals.jsonl`. It writes one record per escalation and
  honours the opt-out and the first-seen learning. The `git paw approvals` promotion hint names
  the agent CLI's own permission rules (`project allowlist` / `user allowlist`) instead of a
  bundled preset. **BREAKING (JSON):** `suggested_target` value `bundled preset candidate`
  becomes `user allowlist`.
- The bundled supervisor and coordination skills stop teaching the orchestrator to absorb routine
  approvals. The orchestrator relays every permission prompt to the human and runs
  `sweep.sh approve` only when the human tells it to. The boot-context drive-loop directive and
  the orchestrator hand-off text change accordingly.
- The learnings aggregator's `auto_approved` counter (the *Permission-pattern signal*) goes,
  because nothing produces that status any more.
- Docs (`--help`, README, mdBook, configuration reference, architecture, security posture) are
  rewritten to say plainly what git-paw does and does not decide.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `approval-command-safety`: removes every classifier requirement (whitelist, danger list,
  scratch exception, normalizers, protected-path and `.git/` rules, managed-helper rule, stack
  presets, `__classify`). Adds *git-paw makes no safety decision about an agent's command*.
  Re-attributes prompt-free operation to the agent CLI's permission system, and keeps
  `settings_path` parse-only. The requirement *No permission grants are seeded into agent CLI
  settings files* is left untouched (owned by `cli-setup-consent`).
- `approval-auto`: removes the auto-approval keystroke, option-index, broad-grant, worktree
  pre-approval, stall-integration, logging and `approve_worktree_writes` requirements. Adds
  *Approval keystrokes are sent only by an explicit approve action*. Re-scopes the live-prompt
  gate, the pane-0 exclusion and the broker-variant reuse to detection and escalation.
- `supervisor-unattended-operation`: removes the sole-approver, auto-approve and durable-grant
  requirements. Every live prompt is escalated without blocking. Pane 0 is swept for escalation,
  CLI unavailability and buffered input only. The per-pane claim now guards the loop's
  nudge and submit keystrokes and the explicit `sweep.sh approve`.
- `approval-pattern-surfacing`: the manual-decision log is written by the drive loop for every
  escalated live prompt (absorbs `drive-loop-manual-decision-log`). The promotion hint targets
  the CLI's own rules.
- `supervisor-config`: the `[supervisor.auto_approve]` / `[supervisor.common_dev_allowlist]`
  requirements are replaced by one deprecated-but-parsed requirement. The permission-flag text no
  longer mentions a classifier.
- `broker-runtime`: removes *Auto-approve thread location in dashboard subprocess*; rewords the
  dashboard-pane requirement.
- `supervisor-launch`: removes *Auto-approve common command classes*; the user-guide chapter's
  allowlist section becomes an agent-permissions section.
- `cli-parsing`: `--unattended` help text.
- `boot-block`: the drive-loop directive in the supervisor boot context.
- `supervisor-skill-discipline`: escalation-first doctrine with no approving on judgment.
- `supervisor-autonomous-orchestrator`: permission-prompt hand-offs ask the orchestrator to relay
  the prompt to the human, not to decide it.
- `skill-agent`: removes the absorption doctrine, the tiered `__classify` model, and the
  bookkeeping test requirement. The launch-time sweep escalates every permission prompt.
- `broker-agent-helper`: `sweep.sh approve` sends option `1` + `Enter` and never consults a
  classifier. Helper paths are nameable by an exact-path permission rule.
- `supervisor-learnings`: removes the `auto_approved` permission-pattern signal; the
  `sweep.sh learn` scenario no longer relies on a classifier.
- `cli-prompt-profiles`: the compiled-only security set shrinks to the live-prompt gate, the send
  gate and the approve guard.
- `core-memory-isolation`: removes the classifier-enforced protected-path set; keeps the doctrine.
- `core-security-posture`: the disclaimer's control table names the CLI's permission system and
  the sandbox, not a classifier.
- `core-lang-agnostic`: drops `{{DEV_ALLOWLIST_PRESET}}`.
- `cli-resolution`: boot registration no longer credits a classifier.
- `mcp-agent-docs`: the docs-fetch helper is installed at an exact path that a permission rule
  can name.
- `core-init`: the default config omits the deprecated tables.
- `core-configuration`: the configuration reference documents the deprecation.
- `core-project-conventions`: the README Features list drops `[supervisor.auto_approve]`.
- `core-ci-hygiene`: the direct-tmux scenario names a test that still exists.

## Impact

- **Code removed:** the classifier half of `src/supervisor/auto_approve.rs`;
  `src/supervisor/poll.rs` (after moving `AgentStatusRow` + `fetch_status_over_http` into
  `drive.rs`); `src/supervisor/stall.rs`; `src/supervisor/dev_allowlist.rs`;
  `approve::{auto_approve_pane, ApprovalRequest, approval_keystrokes}`;
  `approval_gate::approval_send_gate`; `drive::{PromptVerdict, classify_prompt,
  prompt_command_slice, first_whitelist_match, send_approval}` and `AlertSink::log_approval`;
  `DriveConfig` / `DriveRunOptions` `whitelist`, `approve_worktree_writes`, `protected_paths`;
  `commands::supervisor::spawn_auto_approve_thread` and its wiring in `src/main.rs`;
  `Command::Classify` / `cmd_classify`; `AutoApproveConfig::{effective_whitelist, resolved}`;
  `ProtectedPaths`; `skills::render_dev_allowlist_preset`; the learnings `auto_approved` counter.
- **Code moved or kept:** prompt detection moves to `src/supervisor/permission_prompt.rs`.
  `KeyDispatcher` / `TmuxKeyDispatcher` move to `src/supervisor/keys.rs`, and `approve.rs` is
  deleted. `claim.rs`, `approval_gate::{live_prompt_in_tail, approval_dedup_key}`,
  `manual_approvals.rs` and the drive loop's nudge, buffered-input and cli-unavailable paths
  stay.
- **Shell:** `assets/scripts/sweep.sh` loses `classify` and `run_classifier`. `approve` sends `1` +
  `Enter` after its own live gate and option-1 guard. `assets/scripts/broker.sh` and
  `docs-fetch.sh` header comments are reworded.
- **Config:** both tables are kept as parse-only structs. `common_dev_allowlist` becomes
  `Option<…>` with `skip_serializing_if`, so a save never writes an undeclared table.
- **Enum ripple:** none for `BrokerMessage` or `SpecBackendKind`. `PermissionType` is deleted, and
  its consumers (`poll.rs`, `approve.rs`, `commands/supervisor.rs`,
  `tests/auto_approve_integration.rs`) are deleted with it.
- **Tests:** classifier suites are deleted (`tests/classify_subcommand.rs`,
  `tests/sweep_sh_classify.rs`, `tests/auto_approve_integration.rs`,
  `tests/prompt_profile_security_boundary.rs`, `tests/memory_isolation_integration.rs`, and the
  unit tests in `auto_approve.rs`, `poll.rs`, `approve.rs`, `stall.rs` and `dev_allowlist.rs`).
  Approve-path suites are rewritten to escalate. New tests cover escalation, deprecation and
  old-config loading. AGENTS.md §6 "existing tests pass unchanged" cannot hold for this change;
  the behaviour change is intentional and specified here.
- **Cross-change coordination** (details in design.md "Cross-change coordination"):
  - Archive order: `bundled-helper-discovery` → `classifier-retirement`. This change MODIFIES
    *Helper publish subcommands* on top of that change's delta.
  - `production-panic-free` archives after this change and must rename its
    `supervisor::auto_approve` static-regex sites and test path.
  - `cli-setup-consent` owns *No permission grants are seeded into agent CLI settings files*.
  - `drive-loop-manual-decision-log` is absorbed. `classifier-compound-segments`,
    `broad-grant-scope-safety`, `approval-send-identity-check` and `approval-pane-map-refresh` are
    dropped, and nothing here depends on them.
- **Release notes:** call out the durable grants that 0.16.0's classifier may already have
  written into the repository-root `.claude/settings.local.json` through "don't ask again"
  options. Retirement stops new ones but removes none.

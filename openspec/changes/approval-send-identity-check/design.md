## Context

Two in-binary approvers send approval keystrokes. Both decide from one capture and send after
another. Traced on `feat/v0.17.0-specs` @ 546df0a:

| Step | Unattended drive loop (`src/supervisor/drive.rs`) | Dashboard poll loop (`src/supervisor/poll.rs`, `approve.rs`, `approval_gate.rs`) |
|---|---|---|
| 1. Capture | `sweep_pane`: `deps.capturer.capture` | `drive_outcomes`: `inspect_with_profile` (capture #1, prompt kind), then `inspector.capture` (capture #2) |
| 2. Classify | `classify_prompt(&capture, …)` → `PromptVerdict::Safe { option_index, matched }` | `normalize_command(extract_command_slice(..))`, `is_terminal_danger`, the safe-rule chain, all on capture #2 |
| 3. Option index | `select_option_index(detect_prompt_shape(captured), &slice, true)`: 2-option → 1; 3-option → 2 unless an arbitrary-code runner. After v0.17 `broad-grant-scope-safety` it is chosen by the option's *text*. | same function |
| 4. Audit | `deps.alerts.log_approval` → `BrokerAlertSink` → `publish_to_broker_http` | `auto_approve_pane` → `publish_to_broker_http` |
| 5. Claim | `send_approval`: `PaneClaim::try_acquire` | `auto_approve_pane`: `PaneClaim::try_acquire` |
| 6. Re-confirm | `send_approval`: fresh capture, `live_prompt_in_tail` | `approval_send_gate`: pane-0 refusal, fresh capture #3, `live_prompt_in_tail` |
| 7. Send | `approval_keystrokes(option_index)`: the digit, then `Enter` | same |

Step 6 checks only that *a* prompt is live. Nothing compares the step-6 capture with the step-1
capture. Three things look like they narrow the window, but none closes it:

- **The claim** (`claim.rs`) excludes other approvers, but only from step 5. Classification happens
  before it is taken.
- **The dedup keys** never run on the send path. `approval_dedup_key` (agent + command slice) dedups
  drive-loop *escalations*. `dedup_shape` (command slice, else the file-op target path) is the
  escalation's displayed `command`. The poll loop dedups nothing.
- **The window is long.** Step 4 is an HTTP round trip. `publish_to_broker_http` allows a 500 ms
  connect plus 2 s write and 2 s read timeouts, so steps 2 to 6 can take seconds.

`send_approval` skipping the pane-0 refusal is required (the drive loop clears pane 0's own safe
prompts) and is not changed here.

## Goals / Non-Goals

**Goals:**
- Keys go only to the prompt that was classified, with the option index resolved for it.
- One pure function, used by both approvers, decides "same prompt?". The two send paths cannot
  drift apart.
- The change only narrows: a defect causes a missed approval, never an extra one.

**Non-Goals:**
- `sweep.sh approve`. It is operator-directed and carries no earlier verdict (see R4).
- Classifier changes. The identity follows the classifier's extractors, whatever they become.
- Escalation dedup (Open Question 3), the audit message's timing (Open Question 1), and nudge /
  buffered-input sends. The latter already re-check, on a fresh capture, that no prompt is live
  (`idle-pane-nudge-submission`).

## Decisions

### D1 — The identity is every prompt-derived input the decision read, taken by the classifier's own extractors

`PromptIdentity` is built from a capture by a pure function in `approval_gate.rs`. It has three parts:

- **command**: the prompted command slice exactly as captured (`extract_command_slice`, before
  normalisation). This is the same text `dedup_shape` keys on. After v0.17
  `classifier-compound-segments` it covers every line of a header-form prompt.
- **file target**: `extract_path_from_file_prompt`, the extractor the classifier's protected-path,
  `.git/`-write and worktree-file-op rules read. v0.17 `broad-grant-scope-safety` (GP-56) moves it
  to the header path.
- **options**: the numbered option lines after the prompt's last header, with decoration and the
  selection caret removed (`strip_decoration`). These fix the prompt's shape and the text of the
  option the index selects.

When a prompt has neither a command nor a file target, the command part is the trimmed capture.
That is the text the classifier fell back to, so it is the text the verdict depended on.

Rejected alternatives:
- `approval_dedup_key` alone. It has no file target, so every file-op prompt on an agent gets the
  same key, and it has no options.
- Whole-capture equality. Any redraw elsewhere on screen would abort every send.
- The normalised slice (`prompt_command_slice`). Equal raw slices always give equal normalised
  slices, so the raw slice is the stricter key.

Reusing the classifier's extractors means the identity changes whenever what the classifier reads
changes. Its precision is bounded by the classifier's view (see R3).

### D2 — One pure re-confirm decision

`reconfirm_prompt(expected: &PromptIdentity, fresh_capture: &str) -> Reconfirm` returns `Cleared`
when `live_prompt_in_tail` fails, `Changed` when the prompt is live but its identity differs, and
`Unchanged` otherwise. Both `approval_send_gate` and `drive::send_approval` call it on their single
pre-send capture, so liveness and identity are judged on the same capture. It is plain data in and
plain data out (functional core), with table tests over captured text.

Rejected: caching verdicts by identity and re-using them. That is the carry-over this change exists
to prevent.

### D3 — The expected identity comes from the classified capture, carried as data

- **Drive loop:** `sweep_pane` builds the identity from the same `capture` it passes to
  `classify_prompt`, and passes it to `send_approval`.
- **Poll loop:** `drive_outcomes` builds it from capture #2 (the one it classifies), threads it
  through `dispatch_approval` into a new `ApprovalRequest` field, and on into `approval_send_gate`.

Capture #1 only supplies the `PermissionType` kind, which can turn a send into a no-op (`Unknown`)
but never start one. Its own small window can therefore only suppress an approval.

### D4 — A mismatch aborts; the next sweep starts over

On `Changed` nothing is sent and the claim drops by RAII. The approver reports not-sent:
- `GateOutcome::PromptChanged` makes `auto_approve_pane` return `Ok(false)`, so the tick outcome is
  not `Approved`.
- `send_approval` returns `Ok(false)`.

The verdict is discarded. The next sweep classifies whatever is live then, with no memory of the
aborted decision. There is no re-classification inside the same sweep. That would make the sweep
re-entrant, and it saves at most one poll interval, only in the rare case where the prompt changed.

### D5 — Where the check lives, before and after v0.18

- **Pulled into v0.17:** two call sites, `approval_send_gate` and `drive::send_approval`. The
  latter skips the gate, both for pane 0 and as GP-46. The shared function is what keeps them
  aligned.
- **After v0.18:** `supervisor-pane-removal` D3 routes `send_approval` through the shared gate, so
  the check sits in one place. `drive-loop-functional-core`'s `ApproveIfStillLive { surface, keys }`
  effect gains `expected: PromptIdentity`, and its effect-log outcomes (sent / skipped by claim /
  prompt cleared) gain "prompt changed". `terminal-backend-seam` changes where the capture comes
  from, not the pure function.

### D6 — Spec placement avoids the v0.18 deltas

- ADD the rule to `approval-auto`.
- MODIFY that capability's gate requirement, whose "sent when live" scenario would otherwise say
  "send" for a changed prompt.
- MODIFY `supervisor-unattended-operation`'s *Auto-approve classifier-safe prompts* for the drive
  loop.

*Auto-approval keystroke sequence* is deliberately left alone: v0.18 `supervisor-pane-removal`
modifies it, and its "the prompt has cleared" clause already reads as "the classified prompt is no
longer live" once a changed prompt counts as the classified one clearing.

## Threat analysis (security-and-safety-review)

- **Asset.** The link between a classifier verdict, including the option it selected (possibly a
  permanent grant), and the prompt the keys actually land on.
- **Actors (safety).**
  - A rogue, mistaken or prompt-injected agent that queues tool calls: a safe one first, a
    dangerous one behind it.
  - A second actor answering the first prompt inside the window: a human typing in the pane, the
    orchestrator's `sweep.sh approve`, or the CLI itself. The CLI can do this through a permanent
    grant written by an earlier approval that resolves a queued prompt, or through a hook.
- **Actors (security).** Instructions injected into what an agent reads reduce to the rogue-agent
  case. This change adds no new external input.
- **Impact today.** The danger-list escalation is bypassed for the replacement prompt, so anything
  it asks for gets approved: writes outside the worktree, irreversible git, exfiltration. When the
  verdict chose option 2 and the replacement is a 3-option prompt, the CLI records a permanent
  "don't ask again" grant for an unvetted command. That persistence outlives the session. (A 2-option
  replacement fails safe only by accident: option 2 is "No".)
- **Mitigation.** The identity check runs on the same fresh capture as the liveness check, under
  the claim, immediately before the send. The danger-list, worktree confinement and the send-gate
  are kept and strengthened. No allowlist is widened. The keys sent are unchanged (digit, then
  `Enter`). The identity is compared, never typed, so no untrusted text reaches tmux. No secrets,
  no new dependencies, no `unsafe`.
- **Residual risks.**
  - **R1.** The gap between the re-confirm capture and `send-keys` is two tmux process spawns
    (tens of ms). tmux has no compare-and-send. The claim keeps other approvers out of this gap,
    but not a human typing or the CLI changing state on its own.
  - **R2 (to verify).** It is unverified whether the CLI acts on the option digit alone. If it
    does, the trailing `Enter` could land on a queued next prompt and pick its default option. This
    check cannot reach that; it is a separate follow-up if confirmed.
  - **R3.** Two prompts with equal command, target and options are treated as the same prompt. For
    a file-op prompt, the body (diff preview) is not part of the identity. The worktree-file-op rule
    judges a write by its target. But for a prompt with no command header, the danger-list runs over
    the whole capture, so a same-target edit whose body newly matches a danger pattern would reuse a
    safe verdict inside the window. Accepted: this needs a same-target prompt to replace the
    original inside the window, and the authorization decision (write to this in-worktree path) is
    the same.
  - **R4.** `sweep.sh approve` is outside this change. It goes capture → `git paw __classify
    --resolve-option` → send with no re-capture, and the operator's decision was made from an
    earlier look at the pane. Follow-up: let the operator pass the expected command.

## Risks / Trade-offs

- [A false mismatch on every sweep would leave a safe prompt unapproved and unescalated, a silent
  stall] → The identity excludes the parts that change while a modal is up (footer, caret, anything
  above the last header). A test takes two separately produced captures of one live modal and
  requires equal identities. Open Question 2 covers escalating after repeated aborts.
- [The no-header fallback uses the whole capture, so it is sensitive to redraws] → It applies only
  to prompts with neither a command header nor a file target, which the compiled Claude-profile
  loop does not model. It fails closed.
- [Approval latency] → One extra poll interval, only when the prompt really changed.
- [Public API ripple] → New `ApprovalRequest` field, new `approval_send_gate` parameter, new
  `GateOutcome` variant. Affected: in-crate callers and `tests/auto_approve_integration.rs`. Worth
  noting for v0.18 `cmd-library-surface`.

## Migration Plan

None. Nothing is persisted and there is no config or wire format. Rollback is a revert.

## Open Questions

1. Should an aborted send publish a follow-up status, so the audit does not show an `auto_approved`
   that never happened? Recommendation: not here. The same over-report already exists for cleared
   prompts and lost claims, and "Logged before keystrokes" constrains any fix, so fix all three
   together.
2. Should a pane be escalated after K consecutive identity mismatches? Recommendation: no, until a
   false mismatch is seen.
3. Should escalation dedup key on this identity? `approval_dedup_key` has no file target, so two
   different file-op escalations on one agent within 5 minutes collapse into one alert.
   Recommendation: a separate small follow-up, since it changes escalation behavior, not sending.
4. To verify against Claude Code: does the option digit alone confirm the selection (R2)?

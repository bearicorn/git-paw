## Why

Across eight dogfood waves the supervisor has been hand-classifying every
captured permission prompt into "escalate to the human" vs "auto-approve",
re-deriving the same rules each time. That tribal knowledge is concrete and
battle-tested, but it lives in the supervisor's head, not in the product. The
worst failure it caused: a v0.8.0 unattended run stalled on the agent's own
`git commit` prompt — the agent had finished the work but could not land it.
For unattended operation (v0.9.0, F2-full) the classifier IS the brain of the
loop: it must be a deterministic, reviewable, tested artifact, not an LLM's
ad-hoc judgement re-invented per session.

## What Changes

- Add a **curated danger-list** to the safe-command classifier: an explicit
  escalate-to-human set (`rm -rf`/`rm -fr`, `git push`, `--force`,
  `reset --hard`, `git rebase`, branch-switching `git checkout `, `branch -D`,
  `git worktree remove`, `clean -fd`/`clean -fdx`, `sudo `, `mkfs`, `dd if=`,
  `> /dev/`, `chmod -R`, `chown -R`, `pkill`/`kill`) plus a small per-OS
  addendum (macOS `diskutil`, `/Volumes/…` deletes, `rm -rf ~/Library/…`;
  Linux `mkfs*`, `/dev/sd*`, `/dev/nvme*`). A danger match ALWAYS escalates,
  overriding any allowlist match.
- Add a **scratch-path exception**: `rm -rf`/`rm -fr` is NOT escalated when its
  only target is repo/OS scratch (`/tmp/paw-*`, `/private/tmp/paw-*`,
  `$TMPDIR`-rooted `paw-*`, `.git-paw/tmp/...`), including
  `rm -rf "$VAR"` where `$VAR` resolves to such a path — those auto-approve.
- **Pre-approve worktree-confined `git add` / `git commit`** so an unattended
  agent can land its own work. This is the single highest-value rule from the
  F2 evidence.
- Add the **read-mostly verb allowlist** for auto-approve
  (`curl cat ls grep rg git echo sed awk find wc head tail jq mkdir touch
  openspec just export tmux env`), and the **broad-grant rule**: the
  "don't ask again for: X" / "Yes, and don't ask again" option is taken ONLY
  when X is in that allowlist AND is NOT an arbitrary-code runner. For
  arbitrary-code runners (`python`, `bash -c`, `sh -c`, `eval`, `node`,
  ` -c `) the classifier uses one-time "Yes" and NEVER the permanent broad
  grant.
- Add a **live-prompt gate**: the classifier acts only when the prompt footer
  (`Esc to cancel`) appears in the last ~4 non-blank lines of the capture, so
  a supervisor merely narrating about a pane cannot trip a phantom approval.
- Define **option-index selection** for the keystroke step: 2-option Yes/No →
  option 1 (Yes); 3-option Yes / Yes-don't-ask / No → option 2 only when the
  broad-grant rule allows, else option 1.

## Capabilities

### New Capabilities

(none — this productizes existing behaviour; all deltas land on existing specs)

### Modified Capabilities

- `safe-command-classification`: add the curated danger-list (with per-OS
  addenda), make a danger match an unconditional escalate that overrides
  allowlist matches, add the `rm -rf` scratch-path exception, and the
  read-mostly verb allowlist as the safe-verb basis.
- `automatic-approval`: add the live-prompt gate as a precondition for firing,
  the worktree-confined `git add`/`git commit` pre-approval, the
  arbitrary-code one-time-only (no broad grant) rule, and the 2-/3-option
  index-selection semantics.

## Impact

- Classifier logic consumed by the supervisor poll/sweep loop and, per
  v0.9.0 F2-full, by the `unattended-drive-loop` capability (the drive loop is
  the consumer; this change is the decision function it calls).
- Touches the supervisor auto-approve module and the bundled
  `assets/scripts/sweep.sh` helper (danger-list, scratch exception, live-prompt
  gate, option-index selection).
- No new dependencies. `regex` (already approved) backs the danger/scratch/
  arbitrary-code pattern matching. Per-OS addenda are compiled for
  macOS/Linux only (Windows is WSL = Linux).
- Backward compatible: when `[supervisor.auto_approve] enabled = false` the
  classifier never fires; the existing whitelist, worktree-write, and
  manual-decision-log behaviour is unchanged for prompts that don't hit a new
  rule.

## Context

The supervisor auto-approve path already exists and is well-specified:

- `safe-command-classification` — a prefix-matched whitelist of safe command
  classes; anything outside it is `Unknown` and left for the human.
- `automatic-approval` — sends approval keystrokes for safe classes, logs to
  the broker, and is driven from stall detection.
- `auto-approve-file-edits` — worktree-confined write/edit/create/delete
  prompts classify safe-by-pattern after path canonicalization.
- `approval-configuration` — `[supervisor.auto_approve]` config table.
- `approval-pattern-surfacing` — logs every forwarded (manual-decision) prompt.
- `dev-command-allowlist` — seeds Claude's OWN `allowed_bash_prefixes` so
  routine dev commands don't even prompt.

What is NOT yet in the specs is the curated escalate-vs-approve judgement the
supervisor has been applying by hand across eight dogfood waves: the explicit
*danger* set, the `rm -rf` scratch exception, the worktree-confined
`git add`/`git commit` pre-approval, the live-prompt gate, and the
arbitrary-code "never broadly grant" rule. This change productizes exactly that
judgement so it is deterministic, reviewable, and tested — the decision
function the `unattended-drive-loop` capability calls when it drives an agent
with no human in the loop.

This change does NOT introduce the drive loop itself, the Claude allowlist
seeder, or the keystroke-dispatch mechanism — those exist. It refines the
*decision* the existing dispatch consumes.

## Goals / Non-Goals

**Goals:**

- Encode the curated danger-list as the authoritative escalate set, with a
  danger match overriding any allowlist/safe-by-pattern match.
- Add a per-OS escalate addendum (shared base + macOS/Linux addendum).
- Add the `rm -rf` scratch-path exception (repo-local `.git-paw/tmp/` and OS
  scratch incl. macOS's `/private/tmp` symlink and `$TMPDIR`).
- Pre-approve worktree-confined `git add` / `git commit` (the F2 keystone).
- Gate all firing on a LIVE prompt to kill phantom approvals.
- Restrict the permanent broad grant to allowlisted, non-arbitrary-code verbs.
- One test per scenario; behavioral assertions only.

**Non-Goals:**

- The unattended drive loop's polling/scheduling (separate capability).
- Changing keystroke transport (`tmux send-keys`) or broker logging — reused.
- Auto-detecting a repo's toolchain (the dev-allowlist spec already forbids
  this; unchanged here).
- Windows-native rules — Windows is WSL, which classifies as Linux.

## Decisions

### D1 — Danger-list overrides allowlist (escalate wins ties)

The classifier evaluates the **danger-list FIRST**. A danger match returns
escalate even if a later allowlist or worktree-write rule would have matched.
Rationale: `git push` starts with the allowlisted verb `git`, and
`rm -rf .git-paw/tmp/x` starts with the allowlisted-ish `rm`; without
danger-first, an allowlist hit could mask a destructive command. Alternatives
considered: scoring/precedence weights (too subtle to audit); allowlist-first
with a danger veto (equivalent, but harder to reason about). Danger-first is
the simplest auditable order.

### D2 — Match the prompted COMMAND slice, not surrounding narration

Classification runs against the command text *between* the
`Bash command`/`Bash(` header and the confirmation question — NOT the whole
pane capture. A supervisor narrating "I will not run `rm -rf /`" in prose must
not be classified as a danger prompt, and equally a real `git push` prompt must
be read from the command line, not from a summary the agent printed earlier.

### D3 — `rm -rf` scratch exception by resolved target

`rm -rf`/`rm -fr` escalates UNLESS every target it removes is repo/OS scratch:
`/tmp/paw-*`, `/private/tmp/paw-*` (macOS symlinks `/tmp`→`/private/tmp`),
`$TMPDIR`-rooted `paw-*`, or a path under `.git-paw/tmp/`. The exception also
covers `rm -rf "$VAR"` when `$VAR` is bound (in the captured environment or a
preceding `VAR=…` assignment on the same prompt) to such a path. If ANY target
falls outside the scratch set, the whole command escalates. Rationale: the
unattended loop legitimately cleans its own scratch dirs every wave; forcing a
human approval there defeats the purpose, but a single non-scratch target is a
hard stop. We prefer the repo-local `.git-paw/tmp/` (OS-independent) but the
whitelist must still cover the OS temp dirs because existing tooling writes
there.

### D4 — Worktree-confined `git add` / `git commit` pre-approval

`git add` and `git commit` auto-approve when the agent's cwd resolves inside
its worktree root (reusing the canonicalize-then-`starts_with` check from
`auto-approve-file-edits`). This is additive to the existing worktree-write
rule, which only covered file write/edit/create/delete prompts, not the VCS
staging/commit prompts. F2 evidence: the v0.8.0 dogfood proved the COMMIT step
itself stalls the loop. Note `git push` is NOT here — it is in the danger-list
(D1) and always escalates.

### D5 — Live-prompt gate

The classifier acts only when the footer marker `Esc to cancel` is present in
the last ~4 non-blank lines of the pane capture. Off-screen or scrolled-away
prompts, and prose that merely mentions a command, do not fire. Rationale:
prior dogfood produced phantom approvals when the supervisor's own narration
about a pane was mistaken for a live prompt. The "last ~4 non-blank lines" bound
is the observable signal that the prompt is the active foreground UI.

### D6 — Option-index selection

- 2-option Yes/No → option **1** (Yes).
- 3-option Yes / Yes-and-don't-ask-again / No → option **2**
  (the broad grant) ONLY when the broad-grant rule (D7) permits; otherwise
  option **1** (one-time Yes).

### D7 — Broad grant only for allowlisted, non-arbitrary-code verbs

The "don't ask again for: X" / option-2 broad grant is taken ONLY when X's verb
is in the read-mostly allowlist (`curl cat ls grep rg git echo sed awk find wc
head tail jq mkdir touch openspec just export tmux env`) AND X is NOT an
arbitrary-code runner. Arbitrary-code runners — `python`, `bash -c`, `sh -c`,
`eval`, `node`, or any command containing a bare ` -c ` code-string flag — get
a one-time "Yes" only, never a permanent grant. Rationale: a permanent grant on
`python` or `bash -c` is effectively a permanent grant on *anything*, since the
code string is unbounded; one-time approval keeps each arbitrary-code
invocation individually visible.

### D8 — Per-OS escalate addendum (shared base + macOS/Linux)

The danger-list is a shared base plus a small compile-time per-OS addendum:

- macOS: `diskutil`, deletes targeting `/Volumes/…`, `rm -rf ~/Library/…`.
- Linux (and WSL): `mkfs*`, raw block devices `/dev/sd*`, `/dev/nvme*`.

Built as `base + os_addendum()` so the base stays portable and only the
genuinely OS-specific destructive surface is gated per platform. Alternative
(one flat list) rejected: it would gate macOS device paths on Linux and vice
versa, with no value and confusing audits.

## Risks / Trade-offs

- [A too-broad danger-list blocks legitimate unattended work] → Mitigations:
  the scratch exception (D3) and the worktree `git add`/`git commit`
  pre-approval (D4) carve out the high-frequency legitimate cases proven in
  dogfood; everything else escalating is the safe default for unattended mode.
- [`$VAR` resolution for the scratch exception is best-effort] → If a variable
  cannot be resolved to a concrete path, the command is treated as
  non-scratch and escalates (fail-safe, not fail-open).
- [Live-prompt gate could miss a real prompt that scrolled past 4 lines] →
  Acceptable: a missed prompt escalates to the human (the existing
  manual-decision path), which is the safe direction. A false *approval* is the
  outcome we must never produce.
- [Arbitrary-code one-time approvals add friction in unattended mode] →
  Intended: the alternative (permanent broad grant on `python`/`bash -c`) is an
  unbounded-code grant we explicitly refuse.

## Migration Plan

Additive and gated by the existing `[supervisor.auto_approve] enabled` flag.
With the feature disabled, behaviour is identical to v0.8.0. No config schema
change is required for the default rules; existing configs load unchanged. The
new rules apply automatically when auto-approve is enabled.

## Open Questions

- Should the read-mostly allowlist be user-extensible like
  `safe_commands`, or fixed? Current decision: it composes with the existing
  configurable `safe_commands` whitelist rather than introducing a new config
  surface (no new field this change).

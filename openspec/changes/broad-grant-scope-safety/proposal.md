## Why

The option resolver selects a prompt's permanent broad grant by **index**, without
reading what that option actually grants. Claude Code's option 2 is not one thing: for a
simple command it is a narrow prefix grant naming the exact command
(`Yes, and don't ask again for: cargo test --bin git-paw recover::`), but for a
**compound** command it is a scope-wide grant instead
(`Yes, and don't ask again for similar commands in /…/worktrees/<branch>`). The resolver
takes option 2 in both cases.

A scope-wide grant permanently allowlists a *class* of commands for an entire worktree on
the strength of one safe instance. Three instances were observed across two agents in a
single dogfood wave, and the trigger is systematic rather than incidental: agents issue
compound commands constantly (`cargo fmt && cargo test`, `cmd; echo ---; cmd`), and the
CLI cannot form a single narrow prefix grant for a multi-statement command, so it offers
the directory-wide one. Every compound command is therefore an opportunity to silently
widen the worktree's permanent allowlist, invisibly to the operator.

The existing arbitrary-code carve-out does not cover this. It refuses the broad grant for
`bash -c` / `python -c` code strings, but `timeout 1 bash /tmp/script.sh` is not a code
string — it slipped through and was resolved to a directory-wide grant, which had to be
overridden by hand.

## What Changes

- The broad-grant rule SHALL gain a **scope** condition independent of its existing
  classification condition: the auto-approver SHALL select a permanent grant option only
  when that option's text grants a **specific command**, and SHALL NEVER select one that
  grants a directory, a path prefix, or a class of commands — whatever the verdict on the
  command itself.
- Option selection SHALL be resolved from the option's **text**, not from its position in
  the list. An option whose grant scope cannot be determined SHALL be treated as
  scope-wide and refused.
- When the only permanent-grant option offered is scope-wide, the auto-approver SHALL
  fall back to the one-time "Yes" option rather than escalating — the command itself
  already classified safe, so the wave keeps moving under the narrower approval.
- The bundled `sweep.sh approve` helper SHALL apply the same scope rule, so the helper and
  the in-tool approver cannot disagree.
- The scope determination SHALL NOT be profile-sourced. `cli-prompt-profiles` requires
  that "Security decisions are never profile-sourced", and the breadth of a permanent
  permission grant is exactly such a decision — a consumer must not be able to widen what
  auto-approves by editing a profile field. The rule lives in the classifier.
- **Managed-script trust (folded in, same classifier, same security review):** the
  managed helper-script rule SHALL trust only the exact `<worktree>/.git-paw/scripts/<name>`
  location. Today `is_managed_script_path` (`src/supervisor/auto_approve.rs`) trusts any
  token whose basename is `broker.sh` / `sweep.sh` / `docs-fetch.sh` and whose **relative**
  path passes `agents::is_managed_path` — true for anything starting `.git-paw/` — with no
  `..` normalisation and no content check. So `bash .git-paw/../../tmp/sweep.sh` and
  `bash .git-paw/tmp/sweep.sh` (an agent-writable scratch directory) classify `safe` today
  (verified with `git paw __classify`), and `drive.rs::classify_prompt` labels them
  `managed-script`. The path SHALL be normalised, any `..` component refused, and the
  resolved path SHALL be exactly `<worktree>/.git-paw/scripts/<name>`; the classifier
  SHOULD also compare the file's content with the embedded asset.
- GP-51 (`/dev/null` discard redirects anywhere) was folded in here originally; it now
  lives in `classifier-compound-segments`, which owns the normaliser and the redirect
  analysis and lands first.

- **GP-56 (folded in): file prompts are classified on a bare file name.** Claude Code's
  question line abbreviates the target (`Do you want to make this edit to tasks.md?`); the
  real path is on the `Edit file` header line (`../../../openspec/changes/x/tasks.md`). The
  classifier extracts the question-line name, resolves it inside the worktree, and returns
  `safe` — so every out-of-worktree edit classifies safe. Verified in the v0.17.0 dogfood: a
  worker's edit of the main checkout's `tasks.md` classified `safe 1`. Take the path from the
  header and fail closed when it cannot be found.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `approval-auto`: *Broad grant restricted to allowlisted non-arbitrary-code verbs* gains
  the grant-scope condition; *Option-index selection for Yes/No prompts* requires
  text-based rather than index-based resolution of the broad-grant option;
  *Worktree-confined file operations classify as safe* takes the target from the file header
  and fails closed (GP-56).
- `approval-command-safety`: *git-paw managed helper-script invocations classify as safe*
  trusts only the exact `<worktree>/.git-paw/scripts/<name>` location (normalised path,
  `..` refused, SHOULD content-check against the embedded asset).

## Impact

- `src/supervisor/auto_approve.rs` — broad-grant rule and option resolution
  (`--resolve-option` path used by `git paw __classify`); `is_managed_script_path` /
  `is_managed_script_invocation` (managed-script trust; stop delegating the decision to
  `agents::is_managed_path`, whose subtree-wide answer is right for `remove` but too broad
  here); `src/init.rs`'s embedded script constants (`BROKER_SCRIPT`, `SWEEP_SCRIPT`,
  `DOCS_FETCH_SCRIPT`) if the content check is implemented.
- `src/supervisor/drive.rs::classify_prompt` and `src/supervisor/poll.rs` — the
  `managed-script` rule they call inherits the tightening; no call-site change.
- **Depends on `classifier-compound-segments` (wave 3, lands first)** for per-segment
  evaluation of compound commands. This change (wave 4) reasons about the *grant* a
  compound command is offered; that change makes the classifier evaluate *every segment*
  of it, and applies the managed-script rule per segment.
- `assets/scripts/sweep.sh` (and the repo-root `.git-paw/scripts/sweep.sh` copy, which is
  gitignored and written by `git paw init`) — the helper
  reads the resolved option from `__classify --resolve-option`, so the fix is centralised,
  but the helper's own option parsing must be checked for the same assumption.
- **Security-relevant**: this narrows what is auto-approved. It cannot widen the
  allowlist; the failure mode of a bug here is extra prompts, not extra grants.
- **Delta coordination**: modifies different `approval-auto` requirements than
  `live-prompt-tail-window`, and touches no `cli-prompt-profiles` requirement (the already
  archived `drive-loop-exit-summary-truth` touched those). In `approval-command-safety` it
  modifies only the managed helper-script requirement, which no other pending change
  touches; `classifier-compound-segments` and `retire-stale-requirements` modify other
  requirements of that capability. All archive independently.

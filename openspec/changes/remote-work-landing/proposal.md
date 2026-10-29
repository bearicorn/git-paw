## Why

With a remote surface, mixed sessions and the Jules adapter in place, a remote worker still cannot
safely do useful work: nothing decides which branches may go remote, nothing brings the vendor's
patch back, and nothing stops recovery from starting a second session. This change closes the
loop. The unit of work stays the local worktree and branch: the vendor session starts from the
branch's pushed base, and its final patch is checked, applied and committed on the agent's branch.
Everything after that commit (verify, gates, verdicts, merge) is unchanged. It also makes remote
workers selectable and documents them.

## What Changes

- **The remote eligibility gate**, before anything is created (like v1.3.0's headless gate), every
  refusal reported at once, exit 1. A remote worker is admitted only when all of these hold:
  - `[remote] vendor` is set and the broker is enabled;
  - the worker comes from a spec, whose task prompt becomes the vendor session's prompt;
  - `origin` is on GitHub;
  - the base branch on `origin` equals the local tip (`git ls-remote`, read-only);
  - the worker's branch has no local commits, and an existing worktree holds no uncommitted work;
  - the key is set and the vendor sees the repository (an access check that starts no work);
  - limits and any cool-down allow one more session.

  **git-paw never pushes.** The gate, the limits and the cool-down apply only to creating a
  session: re-attaching, adoption, answers and the verify continuation are never refused by them
  (Fleet's final review, I-1).
- **Creation order.** The local session (worktree, broker registration, session file) is up before
  the vendor session is created, so a local failure never orphans a vendor session. The prompt is
  the task plus a bundled, agnostic addendum; no boot block, broker URL or helper path leaves the
  machine.
- **Patch checks**, before the patch touches the worktree, as a pure function. A patch is refused
  when it has:
  - a path under `.git/` or `.git-paw/`, an absolute path or a `..` component;
  - a `.gitmodules` file;
  - a symlink (`120000`) or submodule (`160000`) mode;
  - more than 5 MB.

  Paths come from git's own parse (`git apply --numstat -z`) and from the header lines, and names
  are compared without regard to case.
- **Apply:** `git apply --index`, then `--3way --index`. It requires a clean worktree and never
  cleans one. On failure it restores the worktree exactly; a failed replacement puts the earlier
  landing back.
- **Commit and announce:**
  - the commit goes on the worker's branch with the vendor's suggested message, the repository's
    identity, and its hooks;
  - git-paw then publishes `agent.artifact` (`done`) on the worker's behalf, so verification,
    gates and merge are unchanged;
  - no push, no pull request.
- **Patch choice that holds either way:**
  - the newest unlanded patch by creation time;
  - landed only if it is based on the session's starting commit and covers every path an earlier
    patch changed; otherwise git-paw escalates instead of guessing (open question 3);
  - no patch at completion escalates (open question 4);
  - a failed session lands nothing: a partial patch it created is escalated with the failure
    (`remote-failed`), never applied or committed.
- **One verify continuation.** The first `agent.feedback` after a landing goes to the same vendor
  session as one message with the failing output (last 8,000 characters, key masked). The next
  failure escalates `remote-verify-failed`. A failed verification never creates a second vendor
  session.
- **Recovery** (stopped), **restart** (paused) and **revival** by `git paw start` re-attach to the
  recorded vendor session with a fresh time cap. A session created but never recorded is adopted
  by its title. An interrupted landing completes once, and a verification a stop interrupted is
  run again. Without the key, the worker stays remote and its session stays recorded.
- **Doctor** reports remote readiness without starting work. The **security posture** documents
  remote workers. A **remote-workers chapter** is added.

## Capabilities

### New Capabilities

- `remote-worker-landing`: the remote eligibility gate, creation after the local launch, patch
  checks, apply, commit and announce, patch choice, the one-continuation rule, recovery without a
  second session, and the user chapter.

### Modified Capabilities

- `preflight-diagnostics`: the remote readiness check (added).
- `core-security-posture`: the posture documents remote workers (added).

## Impact

- **MODIFIED requirement headers:** none.
- **Builds on:**
  - this release: `remote-surface-adapter` (surface, follow, run state, `WorktreeEntry.remote`,
    assets); `mixed-backend-sessions` (`--worker-backend`, the per-worker gate pass, routing,
    status); `jules-remote-vendor` (adapter, ledger, limits, cool-down, key stripping for git).
  - v1.3.0 `headless-backend-selection` (the gate pattern and its refusal format).
  - v0.18.0 (locked): `broker-host-separation` (registration, host lifecycle);
    `orchestrator-escalation-handoff` (escalations reach the orchestrator);
    `drive-loop-functional-core` (gate feedback is published as `agent.feedback`).
  - v0.17.0: `supervisor-correction-loop` (gate feedback), `supervisor-branch-refresh` (may move a
    remote worker's branch; the three-way apply covers it), `git-add-remove-branch` (worktree
    creation, uncommitted-work safety, unmerged-branch retention), `git-hook-injection` (hooks run
    on the landing commit).
  - v0.21.0 `start-resume-summary` (a remote worker's resume line).
  - v1.0.0 `cli-contract-freeze` (additive manifest entries).
- **Code:** `src/remote/landing.rs` (gate decision, patch check, patch choice, continuation:
  pure), the follow's completion and feedback handling, launch ordering in `start` / `add`,
  recovery, `src/doctor.rs`, docs.
- **Enum-variant ripple:** none. The follow's `BrokerMessage` match gains the `Feedback` arm.
- **Contract:** additive. The `remote` value of `--worker-backend` becomes admissible; new
  escalation verdict labels (`remote-patch-refused`, `remote-patch-conflict`,
  `remote-worktree-dirty`, `remote-patch-ambiguous`, `remote-branch-moved`, `remote-no-patch`,
  `remote-verify-failed`) travel on the existing `agent.question` path with no wire change.
- **Security / safety:** untrusted patches, untrusted vendor text, local execution of vendor code
  by verification, data sent to the vendor. Threats in design.md.
- **Dependencies:** none. `jules-remote-vendor`'s transport is the system `curl` through
  `CommandRunner` (decided), which adds no crate.
- **Version notes:** "v1.6.0" is a working number for this release and appears only in prose.

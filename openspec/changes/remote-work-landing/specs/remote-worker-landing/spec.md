## Purpose

Defines how a remote worker's work enters and leaves git-paw. A remote worker is admitted only when
its vendor session can start from exactly what the local repository holds, because git-paw never
pushes. The session is created only after the local session is up. The patch it hands back is
treated as untrusted: it is checked, applied and committed on the worker's own branch, and announced
as the worker's work, so verification, gates, verdicts and merge run unchanged. A failed
verification earns one continuation message to the same session before it is escalated, and
recovery re-attaches to the recorded session instead of starting a second one. This capability also
documents remote workers for users.

## ADDED Requirements

### Requirement: A remote worker launches only when it passes the remote eligibility gate

Before creating any worktree, branch, session file, run state, broker or vendor session for a launch or an attach that includes a remote worker, the system SHALL decide for every remote worker for which a vendor session is to be created whether it may launch, and SHALL admit it only when all of these hold: (a) `[remote] vendor` names a vendor; (b) the session's broker is enabled; (c) the worker is launched from a spec, whose task prompt becomes the vendor session's prompt; (d) the repository's `origin` is a GitHub repository, given as an `https://github.com/`, `git@github.com:` or `ssh://git@github.com/` URL; (e) the base branch the worker's branch is created from exists on `origin`, and its commit there, read with `git ls-remote` and no command that writes, equals the local base branch's tip; (f) the worker's branch does not exist locally, or its tip equals that base tip and its worktree, when one exists, holds no uncommitted change and no untracked file outside ignored paths; (g) the vendor's API key is set; (h) the vendor confirms that the repository is visible to it, which starts no work; and (i) the vendor's daily and concurrency limits and any cool-down allow one more session. When any remote worker is refused, the command SHALL create nothing, SHALL print every refused worker with each failed condition and what would satisfy it, and SHALL exit with code 1. git-paw SHALL NOT push, create a branch on `origin`, or change `origin` to satisfy a condition.

#### Scenario: An unpushed base is refused

- **GIVEN** a GitHub `origin` whose `main` is one commit behind the local `main`, and a remote worker launched from a spec onto a new branch from `main`
- **WHEN** the launch runs
- **THEN** it SHALL exit with code 1 naming the worker, the base branch `main` and that its local tip is not on `origin`
- **AND** no worktree, branch, session file or vendor session SHALL exist afterwards
- **AND** no `git push` SHALL have been run

#### Scenario: A branch with local commits is refused

- **GIVEN** an existing local branch `feat/r` one commit ahead of the pushed base
- **WHEN** a launch makes `feat/r` a remote worker
- **THEN** it SHALL exit with code 1 naming `feat/r` and that it has local commits

#### Scenario: A worktree holding uncommitted work is refused

- **GIVEN** an existing local branch `feat/r` at the pushed base tip whose worktree holds an uncommitted change to `README.md`
- **WHEN** a launch makes `feat/r` a remote worker
- **THEN** it SHALL exit with code 1 naming `feat/r` and that its worktree holds uncommitted changes
- **AND** the change to `README.md` SHALL be intact and no vendor session SHALL have been created

#### Scenario: A repository not on GitHub is refused

- **GIVEN** a repository whose `origin` is `https://gitlab.example/team/demo.git`
- **WHEN** a launch includes a remote worker
- **THEN** it SHALL exit with code 1 naming that `origin` is not a GitHub repository

#### Scenario: A remote worker needs the broker and a spec

- **GIVEN** `[broker] enabled = false`
- **WHEN** `git paw start --branches feat/r --worker-backend feat/r=remote` runs
- **THEN** it SHALL exit with code 1 naming both the disabled broker and the missing spec

#### Scenario: An eligible worker is admitted

- **GIVEN** a GitHub `origin` whose `main` equals the local `main`, `[remote] vendor = "jules"`, the broker enabled, the key set, a fake vendor that reports the repository visible, an empty ledger and a remote worker launched from a spec onto a new branch
- **WHEN** the launch's gate runs
- **THEN** the worker SHALL be admitted

### Requirement: Picking up a worker's own vendor session is never gated as a new one

The remote eligibility gate, the vendor's daily and concurrency limits and its cool-down SHALL apply only where a vendor session is to be created. Re-attaching to a worker's recorded vendor session or adopting one by its title, on recovery, restart from pause or revival by `git paw start`, and sending a followed session an answer or its one verify continuation, SHALL NOT be refused by any of them and SHALL NOT count as a new session against the limits. They SHALL proceed even when the worker's branch holds a landed commit, the base branch on `origin` has moved, the ledger is at a limit or a cool-down is active.

#### Scenario: Recovery re-attaches whatever the gate would say

- **GIVEN** a stopped session whose remote worker records vendor session `sessions/abc123` and whose branch holds the commit of a landed patch, a ledger at the daily task limit and an active cool-down
- **WHEN** `git paw start` recovers it
- **THEN** it SHALL NOT refuse the worker
- **AND** the follow SHALL read `sessions/abc123`
- **AND** the fake SHALL have recorded no session creation

#### Scenario: The verify continuation is sent during a cool-down

- **GIVEN** a landed patch, a ledger at the daily task limit and an active cool-down
- **WHEN** the follow processes the first `agent.feedback` for the worker
- **THEN** the fake SHALL have recorded exactly one message, to the worker's own vendor session
- **AND** it SHALL have recorded no session creation

### Requirement: The vendor session is created from the task after the local session is up

For each admitted remote worker the launch SHALL create its worktree and branch at the base tip like any other worker's, register it with the broker host, and save the session, and only then spawn its remote surface, so a failure of the local launch never leaves a vendor session running. The vendor session's prompt SHALL be the worker's task prompt followed by the bundled remote task addendum. The worker SHALL receive no boot prompt, and no coordination instructions, helper-script path, broker URL or path outside the repository SHALL be sent to the vendor. The session's title SHALL name the git-paw session, the worker's branch and a suffix unique to the creation, and SHALL be recorded before the vendor session is created. When creating the vendor session fails, the launch SHALL print the worker, the vendor's reason with any key masked, and that `git paw start` retries it, SHALL leave the rest of the session running, and SHALL exit with code 1.

#### Scenario: The session is saved before the vendor session is created

- **GIVEN** a launch with one tmux worker and one admitted remote worker on the fake vendor
- **WHEN** the launch runs
- **THEN** the session file SHALL have been saved, listing both workers, before the fake recorded the session creation

#### Scenario: The prompt is the task and the addendum only

- **GIVEN** an admitted remote worker launched from a spec whose task prompt contains `Implement the export`
- **WHEN** its vendor session is created
- **THEN** the recorded prompt SHALL contain `Implement the export` followed by the remote task addendum's content
- **AND** it SHALL contain no broker URL and no path to a helper script

#### Scenario: A failed creation leaves the rest running

- **GIVEN** a launch with one tmux worker and one admitted remote worker whose fake vendor creation fails with a network error
- **WHEN** the launch runs
- **THEN** it SHALL exit with code 1 naming the remote worker and that `git paw start` retries it
- **AND** the tmux worker's pane SHALL be running

### Requirement: A patch is checked before it touches the worktree

Before applying a vendor session's patch, git-paw SHALL refuse it, touching nothing, when its size exceeds 5 MB; when any path it names, on either side of a rename or copy, is absolute, has a `..` component, has a component equal to `.git` or `.git-paw`, or is a `.gitmodules` file at any depth, each name compared without regard to case because a case-insensitive filesystem treats `.GIT` as `.git`; or when any of its header lines creates, deletes or changes a file with mode `120000` (a symbolic link) or `160000` (a submodule). The paths SHALL be taken both from git's own parse of the patch (`git apply --numstat -z`) and from the patch's header lines, and the patch SHALL be refused when either finds a forbidden path. The check SHALL be a pure function of the patch text and that parse. A refused patch SHALL be escalated on the worker's behalf (`remote-patch-refused`) with the reason and the vendor session's URL, and SHALL NOT be applied.

#### Scenario: Forbidden patches are refused

- **WHEN** the check is called for patches that write `.git/hooks/post-checkout`, write `.git-paw/scripts/broker.sh`, write `../outside.txt`, write `/etc/hosts`, add `.gitmodules`, add a file with mode `120000`, add a `160000` entry, rename `src/a.rs` to `.git/config`, and exceed 5 MB
- **THEN** each SHALL be refused with a reason naming the path or the rule

#### Scenario: Refused names match whatever their case

- **WHEN** the check is called for patches that write `.GIT/config`, write `.Git-Paw/scripts/broker.sh` and add `.GitModules`
- **THEN** each SHALL be refused with a reason naming the path

#### Scenario: A refused patch changes nothing

- **GIVEN** a worker's worktree at commit `C` with a clean `git status`, and a vendor patch that writes `.git/hooks/pre-commit`
- **WHEN** the session completes
- **THEN** HEAD SHALL still be `C` and `git status` SHALL be clean
- **AND** one `remote-patch-refused` escalation SHALL have been published with the session's URL

#### Scenario: An ordinary patch passes

- **WHEN** the check is called for a patch that modifies `src/lib.rs` and adds `docs/notes.md`
- **THEN** it SHALL pass

### Requirement: A safe patch is applied with --index, then --3way, and a failed apply leaves the worktree as it was

git-paw SHALL apply a patch that passed the check to the worker's worktree with `git apply --index`, and, when that fails, with `git apply --3way --index`. Before applying it SHALL require the worktree to hold no uncommitted change and no untracked file outside ignored paths, and otherwise SHALL land nothing and escalate `remote-worktree-dirty`, so it never discards a person's work. When both attempts fail, git-paw SHALL restore the worktree's index and files to the commit it started from and remove every file the attempts created, leaving `git status` clean and HEAD unchanged, and SHALL escalate `remote-patch-conflict` with the vendor session's URL. git-paw SHALL NOT run, on a worker's worktree, any command that discards a change or removes a file it did not itself create, such as a hard reset over uncommitted changes or a clean of untracked or ignored files.

#### Scenario: A patch applies directly

- **GIVEN** a clean worktree at the commit the session started from and a patch made against it
- **WHEN** the patch is landed
- **THEN** it SHALL be applied with `git apply --index` alone

#### Scenario: The three-way apply covers a moved branch

- **GIVEN** a clean worktree whose branch was refreshed onto a newer base commit that changes lines inside the patch's context, so `git apply --index` fails
- **WHEN** the patch is landed
- **THEN** it SHALL be applied with `git apply --3way --index`

#### Scenario: A conflicting patch leaves the worktree clean

- **GIVEN** a clean worktree at commit `C` and a patch that applies neither directly nor three-way
- **WHEN** the patch is landed
- **THEN** HEAD SHALL be `C`, `git status` SHALL be clean and no file the attempts created SHALL remain
- **AND** one `remote-patch-conflict` escalation SHALL have been published

#### Scenario: A dirty worktree is not touched

- **GIVEN** a remote worker's worktree holding an uncommitted change to `README.md`
- **WHEN** its session completes with a safe patch
- **THEN** the change to `README.md` SHALL be intact and nothing SHALL be applied
- **AND** one `remote-worktree-dirty` escalation SHALL have been published

#### Scenario: A failed apply removes only its own files

- **GIVEN** a clean worktree at commit `C` holding the ignored file `build/out.log`, and a patch that applies neither directly nor three-way
- **WHEN** the patch is landed
- **THEN** `build/out.log` SHALL still exist with its content unchanged

### Requirement: A landed patch is committed on the worker's branch and announced as the worker's work

After a successful apply, git-paw SHALL commit the staged change on the worker's branch with the patch's suggested commit message, control characters removed, or, when there is none, a fixed message naming the vendor session. The commit SHALL use the repository's configured git identity and SHALL run the repository's hooks; a commit that fails, for example because a hook refuses it, SHALL be handled as a failed apply. After the commit, git-paw SHALL publish on the worker's behalf an `agent.artifact` with status `done` and the paths the patch changed as its modified files, so verification, gates, verdicts and merge proceed exactly as for a local worker. git-paw SHALL NOT push the branch or open a pull request.

#### Scenario: The commit carries the suggested message

- **GIVEN** a safe patch whose suggested commit message is `Add the export command`
- **WHEN** it is landed
- **THEN** the worker's branch SHALL gain exactly one commit whose message is `Add the export command`

#### Scenario: The landing is announced and nothing is pushed

- **GIVEN** a safe patch that modifies `src/export.rs`
- **WHEN** it is landed
- **THEN** the broker log SHALL hold an `agent.artifact` from the worker with status `done` and modified file `src/export.rs`
- **AND** no `git push` SHALL have been run

#### Scenario: A refusing hook leaves the worktree clean

- **GIVEN** a repository whose pre-commit hook refuses every commit
- **WHEN** a safe patch is landed
- **THEN** HEAD SHALL be unchanged and `git status` SHALL be clean
- **AND** one `remote-patch-conflict` escalation SHALL have been published

### Requirement: Only a patch that covers the session's whole change is landed

When a vendor session completes, git-paw SHALL choose the newest patch event by creation time that it has not landed before and — after a verify continuation — that was created after the continuation was sent. Because it is not yet confirmed whether a session's newest patch carries the session's whole change, git-paw SHALL land that patch only when its base commit is the commit the session started from and it changes every path that an earlier patch event of the same session changed; otherwise it SHALL land nothing and escalate `remote-patch-ambiguous` with the vendor session's URL. When a newer patch replaces one already landed, git-paw SHALL, before applying it, return the worker's branch to the commit the earlier patch was applied on, and only when the branch's tip is still the commit that landing made; otherwise it SHALL land nothing and escalate `remote-branch-moved`. A completed session with no patch to land SHALL be escalated `remote-no-patch` with the URL. git-paw SHALL land patches only from a session that has completed: when a vendor session fails, git-paw SHALL apply and commit none of the patches it has not landed before, however complete they look, and the session's `remote-failed` escalation SHALL state, when the session created such a patch, that this partial patch was not landed. The choice SHALL be a pure function of the session's state, its patch events, the landing record and the continuation record.

#### Scenario: The newest patch by creation time is chosen

- **GIVEN** a completed session whose listing returns a patch created at 10:05 before a patch created at 10:01 that changes a subset of its paths, both based on the starting commit
- **WHEN** the patch is chosen
- **THEN** the 10:05 patch SHALL be landed

#### Scenario: A newest patch that drops an earlier path is ambiguous

- **GIVEN** a completed session whose earlier patch changes `a.rs` and `b.rs` and whose newest patch changes only `b.rs`
- **WHEN** the patch is chosen
- **THEN** nothing SHALL be landed
- **AND** one `remote-patch-ambiguous` escalation SHALL have been published with the session's URL

#### Scenario: A newer patch replaces the landed one

- **GIVEN** a branch whose tip is the commit git-paw made for the session's first patch, and a newer patch created after the verify continuation
- **WHEN** the session completes again
- **THEN** the branch SHALL hold one commit for the newer patch on top of the commit the first patch was applied on, and no commit for the first patch

#### Scenario: A completed session without a patch is escalated

- **GIVEN** a completed session with no patch event
- **WHEN** the follow processes the completion
- **THEN** one `remote-no-patch` escalation SHALL have been published with the session's URL
- **AND** nothing SHALL be committed

#### Scenario: A failed session's partial patch is escalated, not landed

- **GIVEN** a worker's worktree at commit `C` with a clean `git status`, and a vendor session that created a patch based on the starting commit and then reported the failed state
- **WHEN** the follow processes the failure
- **THEN** HEAD SHALL still be `C` and `git status` SHALL be clean
- **AND** one `remote-failed` escalation SHALL have been published with the session's URL, stating that the session's patch was not landed
- **AND** no `agent.artifact` from the worker SHALL have been published

### Requirement: A failed replacement puts the earlier landing back

When a newer patch is to replace one already landed, git-paw SHALL run the patch check, the coverage rule of patch choice and the clean-worktree precondition on it, and SHALL record the earlier landing commit, before it moves the worker's branch; a newer patch that any of these refuses SHALL leave the branch where it is. When, after the branch has moved, the newer patch applies neither directly nor three-way, or its commit fails, git-paw SHALL return the branch to the earlier landing commit, leaving HEAD, the index and the files exactly as they were before the replacement began, and SHALL escalate as for a failed apply. Recovery that completes an interrupted replacement SHALL follow the same rule, so the earlier landing is never lost.

#### Scenario: A refused replacement leaves the earlier landing in place

- **GIVEN** a branch whose tip is landing commit `L1` for the session's first patch, and a newer patch created after the verify continuation that adds `.gitmodules`
- **WHEN** the session completes again
- **THEN** HEAD SHALL still be `L1` and `git status` SHALL be clean
- **AND** one `remote-patch-refused` escalation SHALL have been published

#### Scenario: A replacement that does not apply puts the earlier landing back

- **GIVEN** a branch whose tip is landing commit `L1`, and a newer patch created after the verify continuation that applies neither directly nor three-way on the commit `L1` was applied on
- **WHEN** the session completes again
- **THEN** HEAD SHALL be `L1`, `git status` SHALL be clean and no file the attempts created SHALL remain
- **AND** one `remote-patch-conflict` escalation SHALL have been published

### Requirement: A failed verify gets one continuation message, then escalates

After git-paw has landed a vendor session's patch, the first `agent.feedback` addressed to the worker SHALL be sent to the same vendor session as one continuation message: the bundled continuation framing followed by the feedback's errors, cut to their last 8,000 characters, with the vendor key masked. git-paw SHALL record the continuation as sent before sending it, SHALL restart the follow's time cap when it is sent, and SHALL then land the next patch the session creates after it. Every later `agent.feedback` for the same vendor session, and any `agent.feedback` that arrives before a patch of the session was landed, SHALL NOT be sent; git-paw SHALL instead escalate `remote-verify-failed` on the worker's behalf with the vendor session's URL. When the session, having reported a state other than completed after the continuation, completes again with no newer patch, git-paw SHALL escalate `remote-no-patch`; while the session still reports the completion that preceded the continuation, git-paw SHALL keep following it. git-paw SHALL NOT create a second vendor session for a worker on account of a failed verification.

#### Scenario: The first failure is sent once, with its output

- **GIVEN** a landed patch and an `agent.feedback` for the worker whose errors hold 20,000 characters ending in `test export_roundtrip failed`
- **WHEN** the follow processes the feedback
- **THEN** the fake SHALL have recorded exactly one message to the same session, holding the continuation framing and the last 8,000 characters of the errors, including `test export_roundtrip failed`

#### Scenario: The second failure escalates

- **GIVEN** a session that has already been sent its continuation
- **WHEN** another `agent.feedback` for the worker arrives
- **THEN** no message SHALL be sent
- **AND** one `remote-verify-failed` escalation SHALL have been published with the session's URL
- **AND** the fake SHALL have recorded no session creation

#### Scenario: The vendor has not picked up the continuation yet

- **GIVEN** a session sent its continuation that still reports the completion that preceded it
- **WHEN** the follow reads it twice
- **THEN** no escalation SHALL have been published and the follow SHALL continue

#### Scenario: A completion after the continuation without a newer patch escalates

- **GIVEN** a session sent its continuation that reported working and then completed with no patch created after the continuation
- **WHEN** the follow processes the completion
- **THEN** one `remote-no-patch` escalation SHALL have been published

### Requirement: Recovery and restart re-attach to the recorded vendor session and never create a second

Recovery of a stopped session, restart of a paused session and revival of a live session by `git paw start` SHALL re-attach every remote worker that records a vendor session — re-reading its state and resuming its follow with a fresh time cap, including a follow that had ended — and SHALL create no vendor session for it. For a remote worker whose follow state records a title but no vendor session, git-paw SHALL look the session up by that title and adopt it when found, and only when none is found SHALL create one, after the remote eligibility gate passes again. When re-attaching, git-paw SHALL complete a landing that was interrupted: a patch recorded as landing whose commit is not on the worker's branch SHALL be checked and applied again from a clean worktree, and no patch SHALL be committed twice. The resume summary SHALL describe each remote worker as re-attaching to its vendor session, with its URL.

#### Scenario: Recovery re-attaches and creates nothing

- **GIVEN** a stopped session whose remote worker records vendor session `sessions/abc123`
- **WHEN** `git paw start` recovers it
- **THEN** the fake SHALL have recorded no session creation
- **AND** the follow SHALL read `sessions/abc123`

#### Scenario: A session created but not recorded is adopted by title

- **GIVEN** a remote worker whose follow state records a title and no vendor session, and a fake vendor that lists a session with that title
- **WHEN** `git paw start` recovers the session
- **THEN** git-paw SHALL adopt the listed session
- **AND** the fake SHALL have recorded no session creation

#### Scenario: An interrupted landing completes once

- **GIVEN** a remote worker whose follow state records a patch as landing and whose branch does not hold its commit, with a clean worktree
- **WHEN** the session is recovered
- **THEN** the branch SHALL hold exactly one commit for that patch

#### Scenario: A follow ended by the cap is resumed

- **GIVEN** a live session whose remote worker's follow ended at the time cap
- **WHEN** `git paw start` revives the session
- **THEN** the follow SHALL read the worker's vendor session again with a fresh time cap

### Requirement: A verification interrupted by a stop is run again on recovery

From the moment git-paw announces a landed patch until an `agent.verified` or `agent.feedback` for the worker is observed, the worker's landing SHALL be recorded as verifying. A gate run for a remote worker that a `stop` or `pause` interrupts SHALL publish no verdict for the worker, so the interruption is never taken for a failed verification, and the landing SHALL stay verifying. When recovery, restart from pause or revival re-attaches a remote worker whose landing is verifying and whose branch tip is still the landing commit, git-paw SHALL announce the landing again, with the same `agent.artifact` of status `done`, so the landed patch is verified again. It SHALL send no continuation and create no vendor session on that account, and SHALL NOT escalate `remote-no-patch` because the session's newest patch has already been landed.

#### Scenario: A stop during verification publishes no verdict

- **GIVEN** an unattended session whose remote worker's landed patch is being verified by a gate run
- **WHEN** `git paw stop` runs before the gate run ends
- **THEN** no `agent.feedback` and no `agent.verified` for the worker SHALL have been published
- **AND** the fake SHALL have recorded no message to the worker's vendor session

#### Scenario: Recovery verifies the landed patch again

- **GIVEN** a stopped session whose remote worker's landing is verifying, whose branch tip is the landing commit of the patch created after the verify continuation, and whose vendor session reports completed with that patch as its newest
- **WHEN** `git paw start` recovers the session
- **THEN** the broker log SHALL hold a new `agent.artifact` from the worker with status `done` and the landed patch's paths
- **AND** no `remote-no-patch` escalation SHALL have been published
- **AND** the fake SHALL have recorded no message and no session creation

### Requirement: A re-attach without the vendor key keeps the worker's session recorded

When recovery, restart from pause or revival re-attaches a remote worker while the vendor's key is unset or malformed, git-paw SHALL still recover the session with that worker as a remote worker recording the same vendor session, SHALL create no vendor session for it, and SHALL NOT move its task to another worker or backend. The worker's follow SHALL end at once as on an authentication failure, publishing one `remote-auth` escalation that names the key variable, carries the session's URL and holds no part of any key, and the resume summary SHALL describe the worker as not followed for want of the key. The ledger SHALL keep counting the session, and a later recovery or restart from pause with the key set SHALL re-attach to it.

#### Scenario: Recovery without the key keeps the worker remote

- **GIVEN** a stopped session whose remote worker `feat-r` records vendor session `sessions/abc123`, and `JULES_API_KEY` unset
- **WHEN** `git paw start` recovers the session
- **THEN** the session file SHALL still record `feat-r` as a remote worker with vendor session `sessions/abc123`
- **AND** one `remote-auth` escalation naming `JULES_API_KEY` and carrying the session's URL SHALL have been published
- **AND** the fake SHALL have recorded no session creation

#### Scenario: A later recovery with the key re-attaches

- **GIVEN** that session stopped again, and `JULES_API_KEY` set
- **WHEN** `git paw start` recovers it
- **THEN** the follow SHALL read `sessions/abc123`
- **AND** the fake SHALL have recorded no session creation

### Requirement: The remote-workers chapter documents selection, the gate, landing and the limits

The user guide SHALL include a remote-workers chapter that documents: selecting a remote worker (`--worker-backend <branch>=remote`, `git paw add --worker-backend remote`, `[remote] vendor`); every eligibility condition and what satisfies it, including pushing the base branch yourself because git-paw never pushes; what is sent to the vendor (the task prompt, relayed answers and failing verify output); the follow (questions escalated, answers sent once per question, no plan approval, the time cap, no cancel, printed URLs); landing (the patch checks, the apply order, the commit, the announcement, no pull request); the one-continuation rule; recovery; the daily and concurrency limits, the ledger and the cool-down; the key's handling and its residual exposure; the vendor's reviewed terms facts and the date they were read; which proof-of-concept questions remain open; and that only repositories on GitHub and only the Jules vendor are supported.

#### Scenario: The chapter covers the gate and the no-push rule

- **WHEN** the remote-workers chapter is read
- **THEN** it SHALL name each eligibility condition and what satisfies it
- **AND** it SHALL state that git-paw never pushes and never opens a pull request

#### Scenario: The chapter is built

- **WHEN** `mdbook build docs/` runs
- **THEN** it SHALL succeed with the remote-workers chapter listed in `SUMMARY.md`

## Why

The manual loop `init → start → work → ?` has no last step. Supervised sessions integrate work through
the orchestrator's merge procedure (raw `git checkout main && git merge --ff-only …` in the skill), and
the only git-paw command that ends an agent is `git paw remove`, which deletes the worktree without
integrating anything. `git paw finish <branch>` closes the loop: check the worktree is committed,
integrate the branch into a target branch, then tear the agent down. It is also the primitive the
external orchestrator (v0.18) and the GP-09 merge queue (`finish-merge-queue`) build on.

## What Changes

- New `git paw finish <branch> [--into <target>] [--merge-commit | --ff-only]` subcommand.
- **Refuses unclean work**: uncommitted changes (git-paw's own managed files excepted, as in `remove`),
  or a worktree whose `HEAD` is not `<branch>` (detached, mid-rebase). No `--force`: finish never
  discards work.
- **Target**: `--into` names it; default is the repository's default branch (single resolver). A target
  that does not exist is created at the branch tip, so work can be moved onto a fresh branch (for a PR).
- **Merge policy**: fast-forward only by default. `[finish] strategy = "merge-commit"` or
  `--merge-commit` creates a merge commit instead; `--ff-only` overrides the config back. A branch that
  cannot be integrated under the policy is refused with **exit code 3** and nothing changes.
- **Integration never disturbs another checkout**: when the target is checked out (normally at the repo
  root) the merge runs there and requires that checkout to be clean; otherwise the ref is advanced with a
  compare-and-swap update. No push — PR creation stays out of scope.
- **Teardown after success**: close the agent's surface and deregister it if it belongs to the live
  session, run `on_remove`, remove the worktree, and delete the branch only if git-paw created it and it
  is now contained in the target.
- **Announces the move**: integrating into the default branch with a live broker publishes the existing
  `agent.advanced-main` event, so the drive loop's branch refresh reacts exactly as it does today.
- **`--json`** under the shared `cli-machine-output` contract (`schema_version`, one object, no
  escapes): target, strategy, broker notification, and a `branches` array of per-branch outcomes
  (shaped as an array so the merge queue extends it additively). A policy refusal prints the document
  and exits 3; any other failure leaves stdout empty. `finish` joins the JSON roster.
- **Safety**: `git paw finish` runs only from the main working tree (never from an agent worktree), so
  even a human-approved prompt in an agent pane cannot integrate the agent's own work. git-paw
  approves no prompt (v0.17.0), so an agent's `git paw finish` prompt reaches a human like any other;
  `git paw doctor`'s broad-grant advisory lists a durable `git paw finish` grant in the project's CLI
  settings.

## Capabilities

### New Capabilities

- `git-branch-finish`: the `git paw finish` command — preconditions, target resolution, merge policy,
  integration mechanics, teardown, broker announcement, exit status.

### Modified Capabilities

- `preflight-diagnostics`: **Doctor lists broad durable grants in the project's CLI settings** (added
  by v0.17.0 `cli-setup-consent`; this delta copies its text) gains `finish` among git-paw's
  state-changing subcommands (shape 5).
- `core-error-handling`: **Exit codes distinguish cancellation from errors** (written on v0.17.0
  `cli-setup-consent`'s text: `UserCancelled` and `NotAttempted` exit 2) gains exit code 3 for an
  integration refused by the merge policy.
- `cli-machine-output` (introduced by this release's `uniform-json-output`): **The JSON roster is fixed
  and guarded** gains `finish`.

## Impact

- **Code:** new `src/commands/finish.rs` (use case) + a pure `decide_integration` in core; `src/git.rs`
  gains ref helpers (`is_ancestor`, `update_ref_cas`, `check_ref_format`, `worktree_for_branch`,
  `merge_in_worktree`, `merge_commit_without_checkout`) behind `CommandRunner`; `src/cli.rs`
  (`Command::Finish`); `src/main.rs` dispatch; `src/error.rs` (`PawError::IntegrationRefused`,
  `exit_code::INTEGRATION_REFUSED = 3`); `src/config/` (`FinishConfig`, `[finish] strategy`);
  the doctor broad-grant advisory's git-paw subcommand list (from `cli-setup-consent`); teardown
  reuses `detach_worktree` with the comparison ref parameterised (it is the default branch today).
- **Enum-variant ripple:** none — `BrokerMessage::AdvancedMain` already exists; no new variant.
  `SpecBackendKind` untouched.
- **Docs:** `--help`, README command table, `docs/src/cli-reference.md` (incl. the `finish --json`
  document), `docs/src/user-guide/session-lifecycle.md`
  (manual loop ends in `finish`), configuration reference (`[finish]`), init default config (commented
  `[finish]`), the exported orchestrator skill's merge section may call `finish` (done in
  `finish-merge-queue`).
- **Cross-version dependencies:**
  - v0.17 `classifier-retirement` — removes the classifier danger-list this change once extended
    (*Curated danger-list escalates to human*), so no danger-list delta remains; every permission
    prompt is escalated to a human. v0.17 `cli-setup-consent` — adds `PawError::NotAttempted` (exit 2)
    to *Exit codes distinguish cancellation from errors* and adds *Doctor lists broad durable grants
    in the project's CLI settings*; both deltas here are written on its text and must archive after
    it. If a v0.18–v0.20 change also modifies the broad-grant requirement, rebase this delta on that
    text.
  - v0.17 `purge-unmerged-branch-safety` — finish reuses its teardown rule (delete a created branch
    only when merged) and generalises the comparison ref from "default branch" to "integration target".
    Must rebase over it.
  - v0.18 `cmd-library-surface` — finish is written as a library use case (`git_paw::commands`,
    repo + config in, typed outcome out, no printing); `terminal-backend-seam` — the agent's surface is
    closed through `TerminalBackend`, never a pane index; `supervisor-pane-removal` — after it every
    session is pane-less, so finish depends on how that change resolves GP-49 (`remove`/`add` refuse bare
    sessions today); finish SHALL NOT inherit that refusal. `broker-verdict-authority` — `advanced-main`
    is not an authority verb; finish runs at the root so it holds the root credential anyway.
  - v0.21 (this release) `uniform-json-output` (other half) — creates the `cli-machine-output`
    capability this change MODIFIES (roster + `finish`) and whose contract `finish --json` follows;
    it must be archived **before** this change. Its typed output module hosts `FinishReport`.
- **Security / safety:** see design.md — every git write is local, compare-and-swap or inside a clean
  checkout, never forced; root-only; durable grants flagged by doctor; no new dependency.

## Rebase note (2026-10-19)

This change uses the pre-v0.21 supervisor vocabulary. If it archives after
`supervisor-vocabulary-split`, the user-facing text it adds (flags, config keys, help, docs, broker
wire values) SHALL use the new names (`--coordinated` / `--bare`, `[orchestrator]` / `[workers]` /
`[drive]`, `default_mode`, root identity `orchestrator`). Its deltas are not rewritten: they archive
against today's names, which that change's reading rule maps and v1.0.0's
`remove-supervisor-vocabulary` sweep renames. Old terms used here:

- `agent.advanced-main { from: "supervisor", … }` published by `finish` (design) →
  `from: "orchestrator"`
- "Supervised sessions" / "supervised or not" (proposal, design) → coordinated sessions /
  coordinated or bare
- "a top-level table (not under `[supervisor]`)" for `[finish]` (design) → not under
  `[orchestrator]` / `[workers]` / `[drive]`
- capability names `supervisor-branch-refresh`, `supervisor-pane-removal` → kept until the 1.0.0
  rename (`drive-branch-refresh`)

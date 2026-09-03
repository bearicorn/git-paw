## Why

The flagship `--unattended` autonomous orchestrator (v0.14.0) cannot reach its
first tool call on a fresh consumer. The first external-project dogfood
(peer-4-poker, Spec Kit, 2026-08-31) proved a fresh unattended run dies at boot:
several defects mask each other, so fixing any one alone still leaves the wave
dead at t=0. This change lands the classifier, launch, and conflict-detector
fixes (GP-02/03/04); the agnostic per-CLI allowlist-seeding fix (GP-01) is a
sibling change (`cli-native-allowlist-seeding`).

## What Changes

- **GP-02 — the classifier escalates git-paw's OWN bundled boot commands.**
  `.git-paw/scripts/{broker,sweep,docs-fetch}.sh …` classify as `unknown`, so the
  unattended loop escalates them with nothing to resolve them — every pane stalls
  on its own boot-time helper call. Two parts: (a) unconditionally treat
  invocations of git-paw's managed helper scripts as safe (these are files git-paw
  itself authors); (b) extend command normalization to strip a run of leading
  `VAR=value` assignments plus `env`/`nohup` wrappers before prefix-matching (the
  dogfood emitted `TMPDIR=… cargo test`, `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test`,
  `env VAR=…`, `nohup … &`, varying the prefix each time so a per-command grant
  never keeps up).
- **GP-03 — full-auto / auto are non-deterministic, and dead panes report
  healthy.** `full-auto` → `--dangerously-skip-permissions` opens a blocking
  first-run acceptance dialog (defaulting to "No, exit") on a config dir that never
  accepted bypass, so every pane exits within seconds — yet status still reports
  the session active. `auto` appends no flags, so the pane inherits the config
  dir's default (often manual). Three fixes: (a) resolve `Auto` to an explicit
  permission-mode flag per CLI (rather than no flag) so both levels are
  deterministic; (b) a launch-readiness gate detects the first-run acceptance
  dialog and answers it or fails loudly; (c) effective status stops reporting a
  session healthy once its panes have died.
- **GP-04 — conflict detector false-positives on the shared untracked `.git-paw/`,
  and a `.git/` mutation was auto-approved.** Every pairwise agent combination
  reports `in-flight-conflict` because each worktree's `git status` lists the
  shared untracked `.git-paw/`. Worse, the supervisor "resolved" it by
  auto-approving `echo '.git-paw/' >> .git/info/exclude`. Two fixes: exclude
  git-paw's own managed bookkeeping from the detector's overlap set; and the
  classifier treats writes under `.git/` as non-safe (**security**).

No new commands, flags, or config fields. Additive/behavioral; existing configs
and sessions load and behave unchanged. Not breaking.

## Capabilities

### New Capabilities

<!-- None — this change modifies existing behavior only. -->

### Modified Capabilities

- `approval-command-safety`: command normalization additionally strips leading
  assignment / `env` / `nohup` prefixes before matching (GP-02a); git-paw's managed
  helper-script invocations classify as safe (GP-02b); writes under `.git/` escalate
  as danger (GP-04b).
- `broker-conflict-detection`: in-flight overlap detection excludes git-paw's own
  managed bookkeeping paths, so a shared untracked `.git-paw/` never fabricates a
  conflict (GP-04a).
- `supervisor-config`: the approval-level → CLI-flag mapping resolves `Auto` to an
  explicit permission-mode flag for CLIs that support one (rather than no flag),
  keeping the mapping per-CLI (GP-03a).
- `tmux-orchestration`: the launch-readiness gate detects a first-run bypass /
  trust acceptance dialog and answers it or fails loudly, so a `full-auto` pane
  cannot silently exit at boot (GP-03b).
- `session-state`: effective status combines file state with a liveness probe that
  detects panes/CLIs that have died, so a session whose panes all exited is not
  reported active (GP-03c).

## Impact

- **Code:** `src/supervisor/auto_approve.rs` (`normalize_command` leading-prefix
  strip, managed-path classification, `.git/`-write danger; reuse
  `is_managed_path()` from `src/agents.rs`); `src/supervisor/config.rs` (per-CLI
  approval-level flag mapping); the launch-readiness gate in the tmux/launch path;
  `src/session/…` effective-status liveness probe; `src/broker/conflict.rs`
  (overlap-set filter). Mirrored shell logic in `assets/scripts/*.sh` where the
  classifier is reflected must stay in sync (tracked-`sweep.sh` drift hazard).
- **Enum-variant ripple:** none. No `BrokerMessage` or `SpecBackendKind` variant is
  added or removed; the GP-03c liveness fix refines the existing effective-status
  probe rather than adding a `SessionStatus` variant.
- **Export-agnosticism:** GP-02b keys on git-paw's own `.git-paw/scripts/*.sh`
  helper paths (git-paw authors these — not a consumer-toolchain assumption); GP-04b
  is a generic safety rule; GP-03a keeps the flag mapping per-CLI (a CLI without a
  permission-mode flag maps `Auto` to nothing). No git-paw toolchain verbs are
  hard-coded as universally safe.
- **Security:** GP-04b tightens the classifier (a `.git/` mutation is no longer
  auto-approvable); GP-02 loosens it only for git-paw's own authored helper scripts
  and for prefix-normalized forms of already-safe commands. The per-pane approval
  claim (v0.14.0) keeps broadened auto-approval race-safe. Gate-5 security review
  required.
- **Docs:** the configuration / security-posture reference notes the `.git/`-write
  danger rule (GP-04b); the unattended-operation guide notes the deterministic
  per-CLI permission-mode mapping and the launch liveness gate.

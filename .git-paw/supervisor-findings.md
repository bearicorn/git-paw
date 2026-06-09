# Wave-1.5 dogfood findings — 2026-05-31

Session: `paw-git-paw` (supervisor on `feat/v0.6.0-specs`).
Acting human: Claude (driving the supervisor, not the agents).
Binary: freshly built from `feat/v0.6.0-specs` HEAD (0a35f5a),
includes v0.5.0 `prompt-submit-fix`.

Agents (4): session-bugfixes-v0-6-x, auto-approve-scope-v0-6-x,
per-commit-verification-v0-6-x, supervisor-pane-affordances-v0-6-x.

Role map (capture-derived, NOT pane_current_path):
- pane 0 (135x32): supervisor
- pane 1 (136x32): dashboard
- pane 2 (118x23): agent
- pane 3 (67x23): agent
- pane 4 (42x23): agent
- pane 5 (42x23): agent

## Findings

### W15-1. Boot prompts injected but NOT submitted for claude-oss (BLOCKER)

All five CLI panes (supervisor + 4 agents) showed the full boot
block sitting in the claude-oss input box, unsubmitted, 90s+
after launch. Broker reported 0 agents the whole time.

The binary includes the v0.5.0 `prompt-submit-fix` (single-Enter
launch + paste-buffer recovery), which was verified against
`claude`. It does not submit for `claude-oss` — either claude-oss
collapses the paste to `[Pasted text #N]` and needs the extra
Enter that the fix's timing doesn't deliver, or claude-oss's
submit keybinding differs. Net: a fresh supervisor session with
`[supervisor] cli = "claude-oss"` is dead on arrival until
something sends the submit key.

Impact: wave-2 (and any claude-oss session) can't start without
manual submit intervention. This is the single most important
v0.6.0 dogfood finding so far — it's a launch blocker for the
primary dogfood CLI.

Recommended response: extend `prompt-submit-fix` with a
per-CLI submit profile; claude-oss needs (TBD: confirm one vs
two Enters / different keybinding). Candidate: fold into a
`prompt-submit-claude-oss` v0.6.x change or extend the existing
fix's per-CLI handling.

### W15-2. pane_current_path unreliable for role mapping

`tmux list-panes -F '#{pane_current_path}'` reported pane 1's
CWD as the `session-bugfixes` worktree, but pane 1 actually
runs the dashboard (the `__dashboard` subprocess inherited a
worktree CWD). Role mapping by pane_current_path would have
mis-scoped feedback. Had to derive roles from pane *content*
instead. Reinforces the existing
`feedback_verify_pane_to_agent_mapping` memory but adds: the
dashboard pane also breaks the path heuristic.

### W15-3. Supervisor's own permission prompts have no auto-approver

The supervisor is itself a claude-oss agent and hits the same
`Do you want to proceed?` permission prompts (e.g. a read-only
`find`). The `[supervisor.auto_approve]` subsystem watches the
*coding agents*, not the supervisor — so nothing auto-approves
the supervisor's own prompts. The human must approve them
directly in pane 0. Expected (human supervises the supervisor),
but worth noting: a long unattended supervisor session will
stall on its own first non-allowlisted command. The
common-dev-allowlist preset covers cargo/git/etc. but not a
broad `find /`. Digit-key selection (e.g. send "2") confirms
without a separate Enter.

### W15-4. POSITIVE: supervisor self-recovered stuck agents

Without any human directive, the supervisor inspected the
panes, diagnosed that the 4 coding agents had unsubmitted boot
prompts, and sent Enter to each to submit them. It recovered
the W15-1 blocker on its own — strong signal that the
supervisor-as-pane model + skill handle the stuck-unsubmitted
case once the supervisor itself is running. (The human still
had to submit the *supervisor's* own boot prompt — W15-1 +
W15-3.)

### W15-5. Supervisor used a `for p in 2 3 4 5` loop (simple_expansion drift)

The recovery command was `for p in 2 3 4 5; do tmux send-keys
-t paw-git-paw:0.$p Enter; done` — the `$p` expansion trips
the claude-oss `simple_expansion` permission gate (known
drift `feedback_avoid_loop_pane_capture`). The supervisor
should use the bundled `sweep.sh` (which does explicit
per-pane sends) instead of an ad-hoc loop. Confirms the
v0.6.x `stuck-prompt-detection` / sweep-usage direction is
needed, and that the supervisor skill still reaches for inline
loops.

### W15-6. Agents stall on boot-time broker-curl permission prompt (claude-oss)

After the supervisor submitted the agents' boot prompts, all
four agents booted, read AGENTS.md, and hit the
`curl ... /publish` permission prompt ("Yes / Yes don't ask
again for: curl * / No") on their first broker publish. Only
the first agent got past it (registered); panes 3/4/5 sat on
the curl prompt. v0.5.0's curl-allowlist seeding writes
`~/.claude-oss/settings.json` only when that dir pre-exists,
and the per-worktree `.claude/settings.json` seeding may not
cover claude-oss — so the boot curl isn't pre-allowed for
claude-oss agents. Observing whether the auto-approve thread
(30s stall threshold, `curl http://127.0.0.1:` is safe) or the
supervisor sweep clears them.

### W15-7. Auto-approve can't help UNREGISTERED agents (chicken-and-egg)

After 2+ minutes (well past the 30s stall threshold), neither
the auto-approve thread nor the supervisor cleared the boot
curl prompts on panes 3/4/5. Root cause: the auto-approve poll
thread iterates over *broker-registered agents* and checks
their `last_seen`. These three agents never registered —
they're stuck on the curl prompt that happens BEFORE their
first publish. So auto-approve has no agent record to poll →
never scrapes their panes → never approves. The agent can't
register until it clears the curl prompt, but auto-approve
won't act until the agent is registered. This is a structural
gap that drift F (broker as a pane-keyed stall detector, not
agent-record-keyed) would close — the detector must watch
panes that have NO broker presence yet.

### W15-8. Broker curls should be agent-skill helper scripts (user direction, 2026-05-31)

Rather than agents constructing raw
`curl -s -X POST http://127.0.0.1:9119/publish -d '{...}'`
(verbose, trips the `curl *` permission gate on every distinct
invocation, costs tokens), the broker publish/poll operations
should ship as **executable agent-skill helpers** — e.g. a
bundled `paw-publish <type> ...` script the skill instructs
agents to call. Benefits:
- ONE allowlistable path (`Bash(paw-publish*)`) instead of
  matching curl patterns → kills the boot-time prompt.
- Fewer tokens per publish.
- Deterministic payload shaping (no hand-rolled JSON).
Allowlist seeding for the helper path lands in v1.0.0's
Per-CLI Broker-Curl Allowlist Seeding + Skills-package-
executable-helpers features. This reframes W15-6's fix:
not "seed curl patterns" but "wrap curls in a helper, then
allowlist the helper." Captured as a v1.0.0 direction;
relates to MILESTONE v1.0.0 "Skills package executable
helpers, not just markdown."

### W15-9. POSITIVE: supervisor cleared curl prompts the auto-approve thread couldn't (drift F validated)

The supervisor used `sweep.sh capture` to scrape panes 2-5,
saw the agents stuck on the broker-curl prompt, and approved
each — getting all 4 coding agents registered. This is exactly
the case W15-7 showed the auto-approve thread CANNOT handle
(unregistered agents have no broker record to poll). The
supervisor succeeded because it scrapes panes directly rather
than keying off broker records. Direct evidence for drift F:
pane-keyed detection (supervisor / broker-as-stall-detector)
catches what agent-record-keyed detection misses. The
supervisor reached for `sweep.sh` (good) but still wrapped its
sends in `for p` loops that trip simple_expansion every time
(W15-5) — each one needed a human "1 Enter".

## Boot sequence summary (all 5 agents live)

Human interventions required to boot a claude-oss supervisor
session: 1× submit supervisor boot prompt (W15-1); 4× approve
the supervisor's own loop/find/grep commands (W15-3/5,
simple_expansion). The supervisor then self-recovered all 4
coding agents (submit + curl approval). Net: the session
cannot boot unattended on claude-oss today, but the
supervisor CAN drive agent recovery once it is itself running
and approved.

### W15-10. Conflict detector false-positive on src/config.rs (validates fn-granularity) + GOOD agent coordination

The forward-conflict detector fired: feat-supervisor-pane-affordances
and feat-auto-approve-scope both declared `agent.intent` for
`src/config.rs`. The supervisor relayed an `agent.feedback`
forward-conflict warning. BUT the two edits are in different
regions — pane-affordances adds a new `[layout]` struct/field;
auto-approve-scope edits the `auto_approve` struct. File-level
detection can't tell them apart → false positive. This is
exactly the case the wave-2 `conflict-detector-fn-granularity`
change fixes (region-aware intent). Direct validation.

POSITIVE: the pane-affordances agent handled it well — instead
of blindly proceeding or blindly blocking, it published a
reasoned `agent.question` ("my change is additive + different
region, OK to proceed or wait for their config.rs commit?").
Good forward-coordination behaviour from the v0.5.0 skill.
The cost: the agent stalls waiting for an answer that a
region-aware detector would have made unnecessary.

### W15-11. No clean shape to ANSWER an agent.question (feedback requires `errors`)

Tried to send the pane-affordances agent a positive go-ahead
via `agent.feedback`; the broker rejected it:
`invalid message JSON: missing field errors`. `agent.feedback`
is schema-locked to `{from, errors[]}` — there is no
message type for "answer a question with non-error guidance."
A positive answer must be shoehorned into the `errors` array.
Relates to drift E (supervisor→user channel) and the
`agent.question` asymmetry: questions have no symmetric
answer variant. Candidate: an `agent.answer` /
`supervisor.reply` shape, or relax feedback to allow a
`message` without `errors`.

### W15-12. File-edit prompts are a distinct shape from command prompts (drift C confirmed)

The agents stalled silently on `Do you want to make this edit
to <file>?` prompts — a DIFFERENT string than the
`Do you want to proceed?` command prompt. Any approver (the
auto-approve subsystem, a sweep helper, or my drive script)
that only matches "proceed" misses every file edit. The
v0.5.0 auto-approve covers shell commands, not file edits, so
these sit forever (the agents made 0 implementation commits
while blocked). Picking option 2 ("Yes, allow all edits during
this session") flips the pane to `⏵⏵ accept edits on`, which
stops future edit prompts — but that is the same accept-edits
mode implicated in the wave-1 wrong-branch commit (drift D).
So: W15-12 (file-edit prompts need handling — wave-2
`auto-approve-file-edits`) and drift D (accept-edits enables
cross-worktree commits — wave-2 `worktree-branch-guard`) are
coupled: the safe way to grant accept-edits is to ALSO have
the branch guard. Both are in the wave-1.5 spec set.

### W15-13. A blind pane-approver must NOT touch the supervisor pane

My acting-human drive loop approved prompts on ALL CLI panes
including pane 0 (the supervisor). Sending "1"/"2"+Enter into
the supervisor pane while it was mid-command interrupted it
("Interrupted · What should Claude do instead?"). Lesson: any
auto-approver / sweep that clears agent prompts must scope to
the AGENT panes only; the supervisor pane is driven by the
human (and the supervisor drives itself). This also applies to
the real auto-approve subsystem if it ever send-keys — it must
exclude the supervisor's own pane. Fixed the loop to panes
2-5 only.

### W15-14. Per-section commit cadence drift (auto-approve-scope hit ~16 uncommitted)

auto-approve-scope committed once early (8d093cc) then
accumulated ~16 uncommitted files without committing again,
while the other three agents committed in smaller increments.
Matches the known `feedback_per_section_commit_cadence` drift:
opsx agents don't reliably commit between sections; the
supervisor is supposed to nudge when uncommitted >~10. With
the supervisor flaky this session, no nudge fired. Risk:
large uncommitted working set is loss-prone (esp. with
accept-edits on). Reinforces that the supervisor's
commit-cadence nudge is load-bearing.

### W15-15. Dashboard CLI column empty for coding agents

Broker `/status` shows `"cli": ""` for all four coding agents;
only the supervisor row has `"cli": "claude-oss"`. The agents'
boot-time `agent.status` publish (the curl in the boot block)
doesn't carry the CLI, so the dashboard renders a blank CLI
column for every agent. This is the agent-side twin of v0.5.0
drift 31 (supervisor cli empty), which `supervisor-as-pane-
followups` fixed only for the supervisor row. Fix: include the
agent's CLI in its boot-status publish (boot block / AGENTS.md
injection knows the CLI), or have the broker resolve it from
the session JSON (now that W15/session-json-location writes
per-agent cli + pane_index).

### W15-16. Phantom agent from feedback `from` field (broker roster harvest)

A `"human"` agent appeared in the roster (`status:"feedback"`,
no heartbeats, last_seen frozen). Root cause: a published
`agent.feedback` with `payload.from:"human"` (the acting-human
answering an agent.question) — the broker harvested that `from`
identity into the agent roster as if it were a real agent. The
roster should be built only from agents that publish
`agent.status` heartbeats, not from `from`/`target` identity
fields on feedback/question messages. Otherwise any
supervisor- or human-originated feedback spawns a phantom
roster row (relates to drift 30 phantom-row family). Cosmetic
but misleading on the dashboard; no clean delete path once
created.

### W15-17. POSITIVE: supervisor runs per-agent gates on directive; gates pass; dedup approver holds

On the directive, the supervisor began per-commit verification
(drift-G behaviour, directed): cd into
feat/per-commit-verification-v0-6-x and ran cargo test on the
new code. The agent's implementation passed its own gate —
`nudge_appears_in_message_log_after_artifact`,
`nudge_does_not_overwrite_committing_agent_record`,
`committed_artifact_suppresses_nudge_when_disabled` all ok.
So the verify-now nudge (the per-commit-verification change's
own feature) is implemented + tested + green. The dedup
supervisor-pane approver (sup-approve.sh) cleared 3 prompts
with zero interrupts — the W15-13 double-send fix holds. Net:
the full loop works once the boot blockers (W15-1/6/7) are
cleared — agents implement, supervisor verifies per-agent,
gates pass. The session's friction is entirely in
boot/approval, not in the core implement→verify cycle.

### W15-18. Supervisor uses `cd <worktree> && git` (untrusted-hooks warning + cwd-leak risk)

During verification the supervisor ran `cd <agent-worktree> &&
git ...`, which trips a claude-oss safety prompt: "This command
changes directory before running git, which can execute
untrusted hooks from the target directory." Two problems: (a)
it's a distinct prompt shape that stalls the supervisor; (b)
cd-ing into another worktree is the exact cwd-leak that caused
wave-1's wrong-branch commit (drift D) — if any such command
mutates (commit/merge), it lands on the wrong branch. The
supervisor skill should mandate `git -C <path> ...` (no cd) for
all cross-worktree git, which avoids both the warning and the
cwd-leak. Reinforces worktree-branch-guard + a skill-prose fix.

### W15-19. META: prompt-approver dedup must not key on prompt boilerplate

My first dedup supervisor-approver keyed on the prompt's last
6 lines to avoid double-sends (W15-13). That failed: every
Yes/No prompt ends in identical boilerplate ("Do you want to
proceed? / 1. Yes / 2. No / Esc to cancel"), so after the first
approval the signature matched for every subsequent distinct
command and they were all deduped → supervisor stalled 3+ min.
Correct approach: approve when a prompt is present, then WAIT
for the prompt to clear (poll until gone) before scanning
again — one approval per prompt instance, no content dedup.
This is a direct lesson for the real stuck-prompt-detection /
auto-approve design (W15-6/7, drift F): prompt dedup is subtle;
dedup on the command/agent identity or use wait-for-clear, NOT
on the prompt text (which is boilerplate-dominated).

### W15-20. POSITIVE: thorough, correct five-gate verification (per-commit-verification VERIFIED)

The supervisor published a complete five-gate agent.verified for
feat/per-commit-verification: testing (fmt/lint/build + 1122
unit + 9 new scenario tests pass), regression, spec audit
(openspec validate --strict, scenario-to-test mapping), doc
audit (just docs builds), security (just audit). Crucially it
CORRECTLY classified the single test failure as the
cold-start live-session guard tripping on the dogfood tmux
session — environmental, not a code regression. That guard is
the wave-1 cold-start-ci-parity work doing its job. The
verify-now-nudge feature the agent implemented is green. End-
to-end loop proven: implement -> commit (own branch) -> five-
gate verify -> verified. With both approvers fixed (agent
drive loop + supervisor wait-for-clear), the session runs
autonomously; remaining friction is purely the boot/approval
layer (W15-1/6/7/12/13/19).

### W15-21. CAPSTONE: agents context-exhausted, idle without finishing (validates coordination-context-budget)

True end-state of the drive: only feat/per-commit-verification
fully completed + five-gate verified. The other three agents
went IDLE at claude-oss's "new task? /clear to save" prompt
having consumed enormous context — pane 3 = 352.7k tokens,
pane 4 = 244.5k, pane 5 = 189.3k — WITHOUT finishing their
tasks (session-bugfixes 10/54 tasks done, 1 commit, 5
uncommitted). They stopped because each agent runs ONE
autonomous turn then waits; with no compaction discipline they
ballooned context and effectively stalled. This is the single
strongest validation of the wave-2 `coordination-context-budget`
change: agents need explicit compact/clear/commit-then-summarise
moments or they exhaust context mid-change. It also shows the
supervisor must RE-ENGAGE idle agents (`/opsx:apply <change>`
per the per-section-commit-cadence + re-engagement memory) to
push them through all tasks — a single boot prompt does not
carry a 40-task change to completion. Driving further requires
/clear + /opsx:apply re-engagement cycles (context lost), which
is itself v0.7.0 skill-reload/context-budget territory.

## Wave-1.5 drive outcome

- per-commit-verification: DONE + VERIFIED (all five gates).
- auto-approve-scope: 5 commits, substantial impl, not verified.
- supervisor-pane-affordances: 1 commit, not verified.
- session-bugfixes: 1 commit (Bug 1 / --specs fix), 10/54 tasks,
  context-exhausted.
- Integration branch (feat/v0.6.0-specs) NEVER contaminated
  (drift-D held the entire run).
- 21 findings W15-1..W15-21 captured. The implement->verify loop
  is proven; ALL residual friction is boot/approval/context-
  budget, not the core cycle.

### W15-22. Parallel skill-editing changes conflict at merge; conflict detector missed skills.rs overlap

Merging auto-approve-scope after per-commit-verification hit a
real conflict in src/skills.rs — both changes appended skill-
content tests + skill prose to the bundled supervisor skill at
the same location. Two issues:
(a) The forward-conflict detector warned on src/config.rs
    overlap (W15-10) but NOT on src/skills.rs overlap, even
    though both agents' intents listed skills.rs. Either the
    intents under-declared skills.rs or the detector missed it
    — worth auditing. Region-aware detection
    (conflict-detector-fn-granularity) wouldn't help here since
    it's the same region (the test module + skill sections).
(b) Resolving it is the supervisor's merge-orchestration job
    (topological order + conflict resolution). With the
    supervisor context-exhausted/menu-stuck this session, the
    acting-human had to choose: hand-resolve risky Rust source
    on the live integration branch, or abort + preserve on
    branch. Chose abort — branch preservation already prevents
    rework; the proper merge happens next wave with a healthy
    supervisor.
Lesson: multiple changes touching the bundled skills (very
common in this cycle — most changes add a supervisor.md/
coordination.md section + a skills.rs test) WILL serialize at
merge. The supervisor's merge-orchestration must own ordered,
conflict-resolving merges; the human shouldn't hand-surgery
the integration branch mid-session.

## Wave-2 dogfood (2026-06-02) — claude-oss boot fix verification

Session paw-git-paw, 3 preserved branches (session-bugfixes, auto-approve-scope,
supervisor-pane-affordances) on the agnostic-fixed binary.

- **W2-1 BOOT FIX CONFIRMED.** All 3 coding agents registered with the broker
  hands-off (status=working) within ~40s — the split-send (1500ms default) submit
  AND the config-driven `[clis.claude-oss].settings_path` curl-seeding both worked.
  Resolves claude-oss-launch deferred tasks 2.4/3.4/5.1/5.5. Zero manual Enter,
  zero curl approval for the agent panes.
- **W2-2 supervisor pane CLI launch ate by oh-my-zsh.** Pane 0 runs in the repo
  root under zsh+oh-my-zsh; the periodic `[oh-my-zsh] Would you like to update?
  [Y/n]` prompt consumed the first `c` of `claude-oss` → `command not found:
  laude-oss` → CLI never launched → boot block fell through to the shell (garbled
  `done` syntax errors). Agent panes (worktree dirs) were unaffected. Fix idea:
  send the CLI-launch keystroke defensively (clear line first, or disable shell
  auto-update prompts in the launched pane env, e.g. DISABLE_AUTO_UPDATE).
- **W2-3 recovery path explodes panes + corrupts session JSON.** `git paw start`
  recovering a STOPPED 4-worktree supervisor session created 10-11 panes (7 halving
  splits) and hit "no space for new pane"; the failed start then rewrote the session
  JSON to 0 worktrees. Fresh `--branches` launch builds the correct N+2 panes. The
  recovery rebuild is the buggy path. (Headless canvas also bumped 200x50→480x140 to
  fit multi-agent layouts when detached.)
- **W2-4 auto_approve did not clear agent boot prompts.** All 3 agents sat ~5min on
  safe permission prompts (read specs, cargo audit, git log) that the
  `[supervisor.auto_approve]` thread did not approve — reinforces W15-7/drift-F.
- **W2-5 supervisor used an inline `for p in 1 2 3 4` loop despite the skill.** Even
  with supervisor-skill-discipline merged into its AGENTS.md, the supervisor built
  the forbidden simple_expansion loop to capture panes (claude-oss won't offer
  "don't ask again" for `$p`, so it re-prompts each time). Prose-only hardening is
  fragile → drift-F (auto-approve inversion) is the durable fix.
- **W2-6 agents use `cd <dir> && git` (cwd-leak / untrusted-hooks).** Pane 4 agent
  repeatedly triggered claude-oss's untrusted-hooks warning via cd-before-git — the
  W15-18 anti-pattern, but agent-side (supervisor-skill-discipline only covers the
  supervisor). Consider extending the git -C discipline to coordination.md.

## Wave-2 cold verification (2026-06-02)

- **W2-7 (CRITICAL) — supervisor five-gate verification false-positived.** The
  supervisor verified auto-approve-scope as five-gate PASS, but cold the branch
  has a real failing test (`terminal_status_integration::watcher_working_tick_
  cannot_downgrade_committed_status`). Root cause: default `cargo test` is
  fail-fast across binaries — it aborts at the FIRST failing binary. The
  no-tmux-server guard test (`helpers::tests::guard_returns_when_no_tmux_server`,
  tripped by an unrelated `paw-*` session) lives in an early-alphabetical
  integration binary, so `cargo test`/`just check` STOP THERE and never run the
  later suites (incl. terminal_status). The supervisor saw "only the guard
  fails" and called PASS — without realizing ~40 suites never executed.
  FIX: the five-gate testing step MUST run `cargo test --no-fail-fast` (and/or
  `GIT_PAW_ALLOW_LIVE_SESSION=1` to neutralize the guard) so an env-guard
  failure can't mask real downstream failures. Update supervisor.md.
- **W2-8 — auto-approve-scope has a stale test contradicting its own new spec.**
  `status-republish-on-write` intentionally relaxes committed-stickiness
  (committed→working on a post-commit write within 60s TTL), but the pre-existing
  `watcher_working_tick_cannot_downgrade_committed_status` still asserts committed
  stays sticky against a write. The change added the feature + a burst test but
  never reconciled the old test. Not a feature bug — an un-updated test. Blocks
  merge until reconciled (update the test to expect committed→working within TTL,
  or remove it as superseded).
- **Cold merge-readiness:** session-bugfixes ✅ (49 suites, 0 fail), supervisor-
  pane-affordances ✅ (45 suites, 0 fail), auto-approve-scope ❌ (W2-8). All three:
  fmt/clippy -D/deny/audit clean; lib green. Clean-env-only gates (coverage,
  manual dogfood) still outstanding for all.

## Wave-2 tooling-hardening (2026-06-03)

- W2-7/W2-2/W2-3 implemented + committed on feat/v0.6.0-specs (no-fail-fast
  verification + just verify recipe; CLI-launch prompt-suppression + line
  clear; recovery stale-session teardown + headless canvas).
- **W2-9 (flaky test) — `sweep_sh_stuck_detection::paste_buffer_capture_
  publishes_paste_variant` is flaky under parallel load** (broker-publish /
  timing race; passes 3/3 in isolation, failed once in a full `just verify`).
  From auto-approve-scope-v0-6-x. Ironic vs W2-7: a flaky test under
  `--no-fail-fast` parallel load is exactly what undermines trustworthy
  verification. Fix: widen the test's broker-poll window / add a bounded
  wait-for-publish. Follow-up.

## Wave-3 validation dogfood (2026-06-03) — new-binary boot + a CONTAMINATION bug

Fresh `git paw start --supervisor --specs coordination-context-budget,
supervisor-stream-timeout-recovery` on the W2-fixed binary.

- **W3-VALIDATED:** supervisor pane booted claude-oss HANDS-OFF (W2-2 fix:
  DISABLE_AUTO_UPDATE + C-u — no oh-my-zsh keystroke-eat); supervisor roster
  row showed `cli=claude-oss` (seeding fix); exactly 4 panes, no explosion
  (W2-3); `--specs` under `--supervisor` created both spec branches
  (session-bugfixes fix). All four wave-2 fixes confirmed live.
- **W3-1 (CRITICAL) — first agent pane is created in the REPO ROOT, not its
  worktree, under `--specs` supervisor mode.** Pane 2 (the FIRST coding
  agent, coordination-context-budget) had `pane_current_path` = repo root;
  pane 3 (second agent) was correctly in its worktree. The first agent
  therefore edited + COMMITTED its work to the integration branch
  (feat/v0.6.0-specs), not its own branch — the exact drift-D wrong-branch
  contamination. Recovered: cherry-picked the 2 stray commits onto
  feat/coordination-context-budget, reset integration to its prior HEAD.
  Root-cause candidate: the supervisor build's first "agent area" pane is
  split from pane 0 (supervisor, repo root) and the `-c <first_agent.worktree>`
  is not taking (or is lost in the pane-1/pane-2 swap). Second+ agents (their
  own `split-window -c <worktree>`) are correct. MUST fix before the next
  supervisor dogfood; blocks unattended `--specs` supervisor runs.

## Wave-4 (3-feature) dogfood (2026-06-04)

- W4-VALIDATED (new cli model live): supervisor row showed `cli=claude-oss`
  from the prefill (not a self-reported guess); only the supervisor row showed
  until agents published (publisher-only rows, no phantom idle rows); all 3
  agents launched in their own worktrees (W3-1). dashboard-broker-log verified
  (1/3) within the first babysitter window; agent-learning-variant +
  advanced-main-event committed full impl+tests, pending supervisor verify.
- **W4-1 — finished agents draft `/opsx:archive` themselves.** Both completed
  agents (agent-learning-variant, advanced-main-event) sat with "archive this
  change with /opsx:archive" in their input box. Agents MUST NOT archive
  (supervisor-only, post-verification + post-cherry-pick per AGENTS.md). The
  opsx:apply flow appears to suggest archiving at task-completion. Reinforces
  the `opsx-role-gating` change (gate verify/archive so coding agents can't
  invoke them). Cleared the stray inputs to prevent a premature worktree-local
  archive.

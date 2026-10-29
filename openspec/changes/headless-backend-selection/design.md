## Context

At the code this cycle builds on:

- **Launch.** Every launch path builds the worker command with v0.19.0's one builder,
  `CliProfile::launch_command`: executable, `args`, sandbox arguments (`worker-sandbox-mode`),
  approval flags for the effective level, then session-id or resume arguments. It also refuses a
  native sandbox combined with a sandbox-disabling row.
- **Built-in rows.**
  - The headless one-shot form is v0.18.0's `headless_args`, with built-in rows `claude` → `-p`
    and `codex` → `exec`. That change appends approval flags *after* the headless form.
  - Session-id and resume templates have a built-in row for the `claude` family only.
  - Approval flags have built-in rows for `claude` (`auto`: `--permission-mode acceptEdits`,
    `full-auto`: `--dangerously-skip-permissions`), `codex` (`auto`:
    `--sandbox workspace-write`, `full-auto`: `--dangerously-bypass-approvals-and-sandbox`),
    `agy` (full-auto) and `qwen` (full-auto).
- **Approval in tmux.** Since v0.17.0 (`classifier-retirement`) git-paw approves nothing.
  Prompt-free operation is attributed to the CLI's own permission system: the approval flags, its
  native modes and allow rules (`approval-command-safety` *Prompt-free operation is attributed to
  the agent CLI's permission system*). Every other live prompt is escalated to a human. Under
  headless there is no prompt to escalate (`headless-drive-adaptation`).
- **tmux is required everywhere today.** `tmux::ensure_tmux_installed()` is called by `start`,
  `add`, `remove` and `recover`. Doctor reports a missing tmux as ✗ (`preflight-diagnostics`
  *Environment checks*).
- **Persisted roster.** `RepoAgentEntry.pane_index` is a required `usize`. v0.21.0 freezes its name,
  type and meaning in discovery-file schema 1, and v0.18.0 says a future backend persists its own
  handle additively, never by repurposing these integers.
- **Status JSON** (v0.21.0) carries `terminal_alive` and the compatibility alias `tmux_running`,
  which "SHALL carry the same value".
- **The vocabulary.** v1.0.0 freezes `default_mode`, `[orchestrator]`, `[workers]` and `[drive]`,
  and defines a *worker* as "a coding agent in a pane".
- **The support matrix** (v0.23.0) derives tiers from twelve lifecycle steps. Its `headless` step
  covers the one-shot (stdin in, exit). v1.0.0 forbids removing a first-class CLI within the
  major.
- **CLI setup.** v0.17.0's `cli-setup-consent` added `git paw setup-cli`: run explicitly by the
  user, it writes least-privilege grants for git-paw's helper scripts into the main checkout's
  `.claude/settings.local.json` (claude only). Launches never write settings.

## Goals / Non-Goals

**Goals:**
- An opt-in, additive selector; tmux stays the default and byte-identical.
- A launch gate that makes "headless" never mean "unsupervised and unconfined".
- Fail before anything is created, with every reason at once.
- Session state, status, attach, doctor, the matrix and the docs all tell the truth about a headless
  session.

**Non-Goals:**
- Mixed-backend sessions, switching backend on recovery, a public dashboard command for headless
  sessions (Open Questions).
- New per-CLI facts. Only rows the code encodes count as built-in; everything else is
  user-declared or "to verify by the v0.23.0 audit".
- Native Windows, a PTY backend, other multiplexers.

## Decisions

### D1 — `start --backend <tmux|headless>` and top-level `default_backend`

The selector is a value flag, so a future backend is an added value rather than a new flag. The
key sits at the top level beside `default_cli` and `default_mode` because it is a per-session
default, like them. It merges as an ordinary top-level scalar, with the repository value winning.

Resolution: `--backend` → `default_backend` → `tmux`. The config default applies only to new
sessions. An existing session always uses its recorded backend.

*Alternatives:*
- `--headless` (boolean): closes the door on a third backend.
- `--terminal` / `default_terminal`: "terminal = headless" reads as a contradiction.
- `[workers] backend`: the backend also hosts the broker service and owns the session, not only
  the workers.

"Backend" is not otherwise user-visible: spec formats are `--specs-format` / `[specs] type`.

### D2 — One backend per session, for the session's life

`Session.backend` is recorded at launch. `add`, recovery, `pause`, `stop`, `purge`, `status` and
`attach` construct the adapter from it, through one factory, the only `match` on `BackendKind`.

A `--backend` that differs from an existing session's backend fails with an error naming both.
Mixed sessions would need a composite backend. Switching on recovery is plausible, because resume
is keyed by the CLI session id, but it is left for later.

### D3 — The eligibility gate is the safety rule

`headless_eligibility(record, effective_level, matrix) -> Result<Eligible, Vec<Refusal>>` is pure
and runs for every planned worker before any side effect. Refusals are collected, not
short-circuited, and printed together with the condition and the fixing key, with exit code 1.

| # | Condition | Fixing key | Why |
|---|---|---|---|
| a | headless form, session-id template and resume template all resolve | `[clis.<name>].headless_args` / `session_id_args` / `resume_args` | Every later message must continue the same conversation; a CLI that cannot resume would start blank on every nudge and answer. Today only the `claude` family has all three built in. `codex` has `exec` but no encoded resume or assignable id, so it is refused until the audit or the user supplies them. |
| b | the effective worker level is not `manual` | `[workers] approval` | `manual` means a human approves each action; there is no one to ask. |
| c | that level resolves non-empty approval flags for the CLI | `[clis.<name>].approval_args` | These flags *are* the declared non-interactive policy. Without them the CLI runs in its default mode, whose non-interactive behaviour git-paw cannot know. |
| d | at `full-auto`, `sandbox` is `native` or `wrapper` | `[clis.<name>].sandbox` | No prompts and nobody watching leave the sandbox as the only containment. |
| e | if (a) rests only on the family's built-in rows, that family's matrix row records `headless-worker` as verified or gap | — (the audit), or override a template | No built-in claim without evidence (the v0.23.0 rule). An override makes the CLI *user-declared*: allowed, with a launch notice saying git-paw has not verified it. |

git-paw never raises a level, never adds a skip-permissions flag, never infers a sandbox, and
never reorders these checks to skip one. v0.19.0's refusal of a native sandbox combined with a
sandbox-disabling row still applies through the builder.

*Alternatives:*
- **Warn only**, as v0.19.0 D4 does for tmux. Rejected: a warning is invisible in a detached run,
  and headless has no installed base to break.
- **Allow only `full-auto` plus a sandbox.** Rejected: `auto` is a legitimate and *stricter*
  policy. Everything outside the CLI's own policy is refused, because no prompt is shown and
  git-paw approves nothing.
- **Infer eligibility from the family.** Rejected for the same reason v0.19.0 D3 rejects
  inference.

### D4 — Turn-plan order: the headless form before the flags

The order is the executable, `args`, `headless_args`, sandbox arguments, approval flags, then
session-id (first) or resume (continuation) arguments. That matches v0.18.0's one-shot (approval
flags appended after the headless form), and a subcommand-style form (`exec`) needs to come
early.

For `claude` the built-in plan is `<exe> [args] -p [approval] --session-id <id>`, and the
continuation is `… --resume <id>`. Whether `-p` combines with `--session-id` and with `--resume`
is to be verified by the audit (D10). For a subcommand form, whether flags may sit between
`exec` and a `resume` subcommand is also to verify.

### D5 — Persisted state: add, never repurpose

- `Session.backend: Option<BackendKind>`, with `serde(default, skip_serializing_if)`, written only
  for headless. A tmux session file is byte-identical.
- In the discovery file, a top-level `backend` (absent means tmux) and `headless_agents[]` with
  `branch_id`, `worktree_path` and `cli`. A headless session writes `agents: []`.

`agents` is what `sweep.sh` uses for pane targeting, and a headless session has no panes, so the
empty list is accurate and `pane_index` keeps its type and meaning. Omitting `pane_index` (a type
change) and writing a placeholder (repurposing) were both rejected. Readers that ignore unknown
keys are unaffected.

### D6 — The headless launch

1. Run the eligibility gate.
2. Create the worktrees.
3. `open_session`; `set_session_env`.
4. Start the broker host with `spawn_service` and wait (bounded) for `/status`, when the broker is
   enabled.
5. For each worker: `spawn_surface` with the turn plan, `stream_to` the record (when capture is
   on), then the boot prompt as text plus `Enter`.
6. Save the session and the discovery file.
7. Print `git paw status` and `git paw attach` as the next steps.

There is no dashboard surface, no readiness gate (nothing to wait for), and no tmux requirement.
`--dry-run` prints the backend, each worker's eligibility and each turn plan, and creates nothing.

`add` to a paused session writes the boot prompt as a draft. `pending_boot_prompt` stays as
today, and restart-from-pause submits it.

### D7 — Revival, attach, status

**`start` on an existing headless session** (the analogue of reattach / recover):

1. Send the continuation message to every `Interrupted` worker.
2. Recreate the broker service if it is not alive.
3. Then, interactively, follow output as `attach` does. Non-interactively, refuse as today's rule
   requires.

A session whose run state is gone but whose receipt is active reads stale, and `start` recovers
it: it re-opens the session and respawns surfaces whose first message is the continuation (the
conversation lives in the CLI's transcript under `cli_session_id`).

**`attach`** follows workers' output.

**`status`, human output.** Headless sessions add a `Backend: headless` line and each agent's
activity; tmux output is unchanged.

**`status --json`** adds `backend` (`"tmux"` / `"headless"`) and per-worktree `activity` (`null`
for tmux). `terminal_alive` is the backend's session liveness, and `tmux_running` mirrors it as
v0.21.0's alias rule requires. That a headless session's `tmux_running` is `true` is documented:
the field is a deprecated alias of `terminal_alive`.

### D8 — Doctor and `sweep.sh`

**Doctor.**
- Under `default_backend = "headless"`, a missing tmux is ⚠ ("only the headless backend is
  usable"); otherwise it stays ✗.
- A new *Headless* check per configured worker CLI renders D3's result: ✓ eligible, ⚠
  user-declared, ✗ with each refusal. It runs only when `default_backend = "headless"`, so a tmux
  user's report is unchanged.

**`sweep.sh`.** Its `snapshot`, `capture`, `approve` and `detect-stuck` subcommands read the
discovery file's `backend` and exit non-zero with a message naming `git paw status` / `git paw
attach` when it is `headless`, instead of failing on a missing tmux session. Its heredocs stay
apostrophe-free, and `bash -n` is run after the edit.

### D9 — Contract and vocabulary

All entries are additive and recorded in the v1.0.0 manifest:

- `start --backend` and its values `tmux` / `headless`;
- `default_backend`;
- MCP `start_session.backend`;
- `status --json` `backend` / `activity`;
- the optional session-file and discovery-file fields.

No exit code changes. The frozen definition "a *worker* is a coding agent in a pane" becomes
inaccurate. At apply time a MODIFIED delta of v1.0.0's *The coordinated-session vocabulary is
frozen* broadens it to "a coding agent git-paw runs in a worktree surface — a pane under tmux".
This widens the meaning without narrowing any entry, so the freeze allows it. AGENTS.md and the
docs change "tmux is a hard dependency" to "tmux is required for the tmux backend (the default)".

### D10 — The matrix records `headless-worker` outside the tiers

A thirteenth tier step would make every first-class CLI unverified, and so demote it, which the
freeze forbids. So each row carries `headless-worker` (verified / gap / unverified) beside the
twelve steps, not in `derive_tier`. Its evidence:

- a fixture: help text containing every token of the turn plans at `auto` and `full-auto`;
- an attestation that a two-turn run continued the conversation;
- an attestation that, at `auto`, an action outside the CLI's policy was refused and the turn
  exited instead of waiting for input;
- an attestation of what the CLI prints on a usage-limit or login failure in non-interactive
  mode, which feeds the unavailable markers.

The Supported AI CLIs page and doctor show the fact, and the docs-page parity test covers it. The
claude row is attested as part of this change's tasks.

### D11 — `git paw setup-cli` is a launch notice, not gate condition (f)

`cli-setup-consent` shipped in v0.17.0, not v0.19.0, and it records no consent for passing
permission flags: its consent is the user running `git paw setup-cli`, which writes helper grants
into the CLI's settings. Nothing about a headless launch depends on it, so the gate gains no
condition (f). What it does answer is the question an earlier draft left open: at `auto`, a CLI's
policy may refuse the helper-script or MCP calls workers use to publish status. The launch does not
refuse; it prints one notice per such CLI naming `git paw setup-cli <cli> --allowlist` where the
setup fact row declares the item (today `claude`), and the CLI's own configuration or
`[clis.<name>].args` otherwise. The launch never writes settings; only that explicit command does
(`approval-command-safety` *No permission grants are seeded into agent CLI settings files*).

*Alternative rejected:* refusing `auto` in a broker session without a setup record. It would make
a consented, optional setup step mandatory, and a CLI without a setup row could never run headless
at `auto`.

## Risks / Trade-offs

Threat analysis (`security-and-safety-review`), policy level. The adapter-level threats are in
`headless-backend-adapter`.

- **S1 — A rogue or mistaken worker at headless `auto`.** → It can do only what its CLI's policy
  allows, and git-paw approves nothing on its behalf. That is the same as tmux `auto` since
  v0.17.0, except that on tmux an action outside the policy prompts and is escalated to a human,
  while headless refuses it. Residual: the CLI's own policy semantics
  (for example, whether an edit-accepting mode can write the shared `.git` through the worktree's
  gitdir), to verify per CLI. The trade-off is documented: test and build commands are refused
  unless the user's own CLI configuration allows them.
- **S2 — A rogue worker at headless `full-auto`.** → A declared sandbox is required. The v0.19.0
  residuals remain: network egress, shared `.git` refs, pushes with the user's credentials, and
  secret reads without read-hardening. Headless adds two: nobody is watching, and work continues
  after the terminal closes. Mitigations:
  - the posture chapter recommends the read-hardening tier for headless;
  - budget caps stop new turns;
  - no-progress and the heartbeat surface stalls;
  - `stop` ends everything.
- **S3 — A false sandbox declaration.** → Declared, not verified. Doctor shows the mode, and the
  docs say what a declaration does and does not prove (as in v0.19.0).
- **S4 — Untrusted config.** → `args`, `approval_args`, templates and `sandbox_args` are trusted
  config under the same model as `[clis.*].command` and `[worktree.hooks]`. The gate cannot defend
  against a user who declares a permissive override, and it does not try to.
- **S5 — Bypassing the gate.** → The MCP `start_session`, `add`, recovery and the dry run all call
  the same pure decision through the library surface, and an e2e test asserts each refuses an
  ineligible worker.
- **S6 — Environment inheritance.** → Turns inherit the environment of the process that starts
  their runner: the user's shell, the broker host, or the MCP server for an MCP-started session.
  Documented. Nothing is persisted (adapter D11).
- **S7 — An unattended wave continues while the operator is away.** → That is the point of the
  feature, and it is bounded by the budget caps, the heartbeat and the escalation hand-off.
  `git paw status` shows every busy worker.

Other trade-offs:

- **`tmux_running` is `true` for a headless session.** → A documented alias of `terminal_alive`.
  The alternative would break v0.21.0's alias rule.
- **No dashboard for headless sessions.** → `status`, `attach` and the MCP read tools cover the
  roster and output. The broker log stream is in the session record.

## Migration Plan

1. `BackendKind`, `default_backend`, the `--backend` flag (rejected for now by the gate with "not
   yet available"), the factory, and the session and discovery fields; tmux byte-identical.
2. The pure eligibility gate plus the turn-plan composition; table tests.
3. The headless launch, add, remove, pause, stop, purge, revival, status and attach paths;
   end-to-end tests with a stub CLI and no tmux on PATH.
4. Doctor, `sweep.sh`, the MCP parameter, the matrix fact and the claude attestation.
5. Docs, manifest regeneration, the vocabulary delta.

Rollback: revert. Headless sessions created meanwhile need this release to stop or purge (older
binaries ignore `backend` and would treat them as tmux sessions whose tmux session is gone, which
means stale). The changelog says so.

## Open Questions

- **Switching backend on recovery.** It is possible because resume is per CLI session.
  *Recommendation:* later, on demand.
- **Mixed-backend sessions.** *Recommendation:* no, until a use case appears.
- **A public dashboard for headless sessions.** This is downstream of v0.18.0's dashboard-survival
  question. *Recommendation:* no new command here.
- **`attach --agent <branch>`.** *Recommendation:* add it only if interleaved output proves noisy
  in dogfood.

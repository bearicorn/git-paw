## Why

`headless-backend-adapter` and `headless-drive-adaptation` make a headless worker possible, but
nothing selects one. Selecting it is also a safety decision. On tmux, an action outside the
agent CLI's permission policy raises a prompt, and git-paw escalates it to a human (since v0.17.0
git-paw approves nothing). A headless worker has no prompts, so nothing is escalated and the only
thing that decides what it may do is its CLI's own non-interactive permission policy. git-paw must therefore refuse to run a worker headless unless it knows the
following about it:

- that the CLI has a non-interactive form at all;
- that its conversation can be continued between turns;
- that its CLI was given a declared permission policy;
- that a skip-permissions worker is sandboxed.

This change adds the opt-in selection with that gate, records the backend in session state, and
routes every lifecycle command through it. It also documents the feature. tmux stays the default,
and nothing changes for a user who does not opt in.

## What Changes

- **Selection.** `git paw start --backend <tmux|headless>` and a top-level `default_backend` key
  (`"tmux"` | `"headless"`; absent means tmux). The key merges repository-over-global like
  `default_mode`. MCP `start_session` gains an optional `backend` parameter. The backend is fixed
  for a session's life:
  - `add`, recovery and every lifecycle command use the recorded backend;
  - a `--backend` that contradicts an existing session is an error.
- **Eligibility gate, before anything is created.** For every worker, a pure decision over its
  resolved CLI record and effective approval level checks four things:
  - (a) the CLI resolves a headless form, a session-id template and a resume template;
  - (b) the worker approval level is not `manual`;
  - (c) that level resolves non-empty approval flags for the CLI — its declared non-interactive
    permission policy;
  - (d) at `full-auto`, the CLI declares `sandbox = "native"` or `"wrapper"`.

  When (a) rests only on the family's built-in rows, the v0.23.0 support matrix must record the
  new `headless-worker` fact as verified or gap. When any of the three templates comes from the
  user's `[clis.<name>]`, the CLI is user-declared and launches with a notice. A `git paw
  setup-cli` record (v0.17.0) is not a gate condition.
- **Helper-call notice.** A worker at `auto` in a broker session launches with a notice that its
  CLI's policy may refuse the helper or MCP calls it publishes status with, naming
  `git paw setup-cli <cli> --allowlist` where the CLI supports it. It never refuses the launch and
  never writes settings.

  Any refusal aborts the launch, with exit code 1 and every refused worker listed, together with
  the condition and the key that would fix it. No worktree, branch, session file, run state or
  broker is created. git-paw never raises a worker's level or adds a skip-permissions flag for
  headless.
- **The turn plan comes from the one launch builder.** In order: the executable, `args`,
  `headless_args`, sandbox arguments, approval flags, then the session-id arguments (first turn)
  or the resume arguments (later turns). The headless form goes before the flags, as in v0.18.0's
  one-shot.
- **The headless launch.**
  - No tmux is required.
  - The broker host runs as a detached service.
  - No dashboard surface is created.
  - There is no readiness wait: capture is attached first, and the boot prompt is the first
    message.
  - `start` on an existing headless session revives it: it sends the continuation message to
    every interrupted worker, then follows the output when interactive.
  - `stop` and `purge` close the session; `pause` stops the broker host.
- **Session state (additive).**
  - `Session.backend` is written only for headless.
  - The discovery file gains `backend` and `headless_agents`. For a headless session `agents` is
    empty, so `pane_index` keeps its meaning.
  - `status` shows the backend and per-worker activity for headless sessions. tmux human output
    is byte-identical.
  - `status --json` adds `backend` and per-worktree `activity`.
- **`attach`** follows workers' output read-only for a headless session.
- **`sweep.sh`'s terminal subcommands** (`snapshot`, `capture`, `approve`, `detect-stuck`) refuse
  on a headless session and point at `git paw status` / `attach`. Its broker subcommands are
  unchanged.
- **Doctor.**
  - A missing `tmux` is ⚠, not ✗, when `default_backend = "headless"`.
  - A new check reports each configured worker CLI's headless eligibility with the same decision
    the launch uses.
- **Support matrix.** A `headless-worker` fact per row, outside tier derivation (so no
  first-class CLI is demoted). It is published on the Supported AI CLIs page and in doctor.
- **Docs.**
  - a new `user-guide/headless.md`;
  - `--help`, README, the CLI reference, the configuration reference and `supported-clis.md`;
  - the security posture chapter (the headless threat model);
  - the stability manifest records the additive entries.

## Capabilities

### New Capabilities

<!-- None (`session-headless-backend` is created by `headless-backend-adapter`). -->

### Modified Capabilities

- `session-headless-backend`: the eligibility gate and permission policy, matrix evidence for
  built-in facts, the auto-level helper-call notice, turn-plan composition, the headless launch,
  revival, lifecycle dispatch, the `sweep.sh` refusal.
- `cli-parsing`: `start --backend` (added); *Attach subcommand* (modified: follows output for a
  headless session).
- `core-configuration`: `default_backend` (added).
- `session-state`: the session records its backend; headless discovery-file entries; status and
  status JSON for headless sessions (all added).
- `preflight-diagnostics`: *Environment checks* (modified: tmux ⚠ under a headless default); the
  headless eligibility check (added).
- `cli-support-matrix`: the `headless-worker` fact outside tier derivation (added; capability
  created by v0.23.0 `cli-support-audit`).
- `mcp-server`: `start_session` accepts a backend (added).
- `core-security-posture`: the posture documents headless workers (added).

## Impact

- **Builds on (must rebase over):**
  - v1.3.0: `headless-backend-adapter` and `headless-drive-adaptation`.
  - v0.18.0:
    - `terminal-backend-seam`: one adapter is chosen at construction, from the session's
      backend.
    - `broker-host-separation`: the host is a service.
    - `cmd-library-surface`: lifecycle handlers.
    - `mcp-orchestrator-write-tools`: `start_session`.
    - `orchestrator-escalation-handoff`: `headless_args`.
    - `supervisor-pane-removal` / `pane-less-session-parity`: the discovery file's
      coding-agents-only `agents` list.
  - v0.19.0:
    - `cli-provider-record`: the one launch builder, families, `args`.
    - `worker-sandbox-mode`: `sandbox`, `sandbox_args`, and the refusal of sandbox-disabling
      rows.
    - `worker-usage-budget`: unchanged under headless.
  - v0.17.0 worker session id / `--resume` (`cli-resolution` *Per-CLI session-id and resume
    argument templates*).
  - v0.17.0 `classifier-retirement`: git-paw approves nothing and escalates every live prompt on
    tmux; the posture and threat analysis here start from that. v0.17.0 `cli-setup-consent`:
    `git paw setup-cli` and its per-CLI setup fact table (carried by v0.19.0's provider record),
    which the helper-call notice names; its consent is not a gate condition (design D11).
  - v0.21.0:
    - `supervisor-vocabulary-split`: `default_mode`'s placement and merge rule, mirrored by
      `default_backend`.
    - `uniform-json-output`: the status JSON fields `terminal_alive` / `tmux_running`.
    - `wire-format-dtos`: the session and discovery wire types gain the optional fields.
  - v0.22.0 `session-record-capture`: capture through `stream_to`.
  - v0.23.0 `cli-support-audit`: the matrix, evidence rules and docs-page parity.
  - v1.0.0:
    - `cli-contract-freeze`: every new flag, key, value, MCP parameter and JSON field is an
      additive manifest entry. Its *coordinated-session vocabulary* defines a worker as "a coding
      agent in a pane"; at apply time, broaden it to "a coding agent in a worktree surface (a
      pane under tmux)" with a MODIFIED delta (design.md D9).
    - `layer-isolation`: the backend is orthogonal to layers.
- **Requirements modified that earlier pending changes also touch:**
  - v0.18.0 `supervisor-pane-removal` modifies *Per-repo JSON shape matches sweep.sh
    expectations*. This change leaves that requirement alone and adds a separate one.
  - Neither *Attach subcommand* nor *Environment checks* is modified by any pending change.
- **Code:**
  - `src/cli.rs` (`--backend`), `src/config/` (`default_backend`).
  - A new pure `headless_eligibility` in the core and a backend factory.
  - `src/commands/{start,add,remove,recover,pause,stop,status,attach}.rs` and their v0.18.0
    library forms.
  - `src/session.rs` (wire types), `src/doctor.rs`, the v0.23.0 support module, the MCP
    `start_session` tool, and `assets/scripts/sweep.sh`.
- **Enum-variant ripple:** none (`BrokerMessage` and `SpecBackendKind` are untouched). The new
  `BackendKind` (`Tmux`, `Headless`) is closed and matched only in the backend factory and the
  config / wire mappers.
- **Contract:** additive only.
  - New: `start --backend` and its values; `default_backend`; the MCP `start_session.backend`
    parameter; `status --json` `backend` / `activity`; the optional session-file and
    discovery-file fields.
  - No exit code changes: refusal is `1`, and a bad value is clap's `2`.
- **Security / safety:** the launch gate is the change's purpose. The threat analysis is in
  design.md.
- **Dependencies:** none.
